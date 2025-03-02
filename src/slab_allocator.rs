// src/slab_allocator.rs

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::marker::PhantomData;
use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB
};

// Define the fixed sizes we'll support in our slab allocator
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
// Heap configuration
pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 512 * 1024; // 512 KiB

struct Slab {
    block_size: usize,
    free_blocks: NonNull<FreeBlock>,
    blocks_count: usize,
    // Add PhantomData to make NonNull Send/Sync
    _phantom: PhantomData<FreeBlock>,
}

unsafe impl Send for Slab {}
unsafe impl Sync for Slab {}

impl Slab {
    const fn new() -> Self {
        Slab {
            block_size: 0,
            free_blocks: NonNull::dangling(),
            blocks_count: 0,
            _phantom: PhantomData,
        }
    }

    fn init(&mut self, block_size: usize, heap_start: usize, heap_size: usize) {
        self.block_size = block_size;
        let blocks_count = heap_size / block_size;
        self.blocks_count = blocks_count;
        
        if blocks_count == 0 {
            self.free_blocks = NonNull::dangling();
            return;
        }
        
        // Initialize free block list
        let mut current_block = heap_start as *mut FreeBlock;
        
        for i in 0..blocks_count - 1 {
            unsafe {
                // Initialize the current block's next pointer
                (*current_block).next = NonNull::new(
                    (heap_start + (i + 1) * block_size) as *mut FreeBlock
                ).unwrap();
                
                // Move to the next block
                current_block = (heap_start + (i + 1) * block_size) as *mut FreeBlock;
            }
        }
        
        // Set the last block's next pointer to dangling
        unsafe {
            (*current_block).next = NonNull::dangling();
        }
        
        self.free_blocks = NonNull::new(heap_start as *mut FreeBlock).unwrap();
    }
    
    fn allocate(&mut self) -> Option<NonNull<u8>> {
        if self.free_blocks.as_ptr() == NonNull::dangling().as_ptr() {
            return None; // No free blocks available
        }
        
        // Take the first free block
        let block = self.free_blocks;
        unsafe {
            self.free_blocks = (*block.as_ptr()).next;
        }
        
        Some(NonNull::new(block.as_ptr() as *mut u8).unwrap())
    }
    
    fn deallocate(&mut self, ptr: NonNull<u8>) {
        let block = NonNull::new(ptr.as_ptr() as *mut FreeBlock).unwrap();
        unsafe {
            (*block.as_ptr()).next = self.free_blocks;
            self.free_blocks = block;
        }
    }
}

// Free block structure for linked list
struct FreeBlock {
    next: NonNull<FreeBlock>,
}

// Make FreeBlock safe to share between threads
unsafe impl Send for FreeBlock {}
unsafe impl Sync for FreeBlock {}

// Slab allocator structure with tracking for heap regions
pub struct SlabAllocator {
    slabs: [Mutex<Slab>; BLOCK_SIZES.len()],
    slab_heap_regions: [Mutex<(usize, usize)>; BLOCK_SIZES.len()], // (start, end) for each slab region
    fallback_allocator: Mutex<linked_list_allocator::Heap>,
}

// Explicitly implement Send and Sync for SlabAllocator
unsafe impl Send for SlabAllocator {}
unsafe impl Sync for SlabAllocator {}

impl SlabAllocator {
    // Create a new empty slab allocator
    pub const fn new() -> Self {
        const EMPTY_SLAB: Mutex<Slab> = Mutex::new(Slab::new());
        const EMPTY_REGION: Mutex<(usize, usize)> = Mutex::new((0, 0));
        SlabAllocator {
            slabs: [EMPTY_SLAB; BLOCK_SIZES.len()],
            slab_heap_regions: [EMPTY_REGION; BLOCK_SIZES.len()],
            fallback_allocator: Mutex::new(linked_list_allocator::Heap::empty()),
        }
    }
    
    // Initialize the allocator with a given heap area
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        // Split the heap into equal parts for each slab size
        let slab_heap_size = heap_size / (BLOCK_SIZES.len() + 1); // +1 for fallback allocator
        let mut current_heap_start = heap_start;
        
        // Initialize each slab with its portion of the heap
        for (i, &block_size) in BLOCK_SIZES.iter().enumerate() {
            // Store the region bounds
            *self.slab_heap_regions[i].lock() = (current_heap_start, current_heap_start + slab_heap_size);
            
            // Initialize the slab
            self.slabs[i].lock().init(block_size, current_heap_start, slab_heap_size);
            current_heap_start += slab_heap_size;
        }
        
        // Initialize fallback allocator with remaining space
        let remaining_size = heap_size - (slab_heap_size * BLOCK_SIZES.len());
        if remaining_size > 0 {
            // Fix: Pass usize directly instead of *mut u8
            unsafe {
                self.fallback_allocator.lock().init(current_heap_start, remaining_size);
            }
        }
    }
    
    // Find the appropriate slab for a given layout
    fn find_slab_index(&self, layout: &Layout) -> Option<usize> {
        // Consider both size and alignment requirements
        let required_block_size = layout.size().max(layout.align());
        BLOCK_SIZES.iter()
            .position(|&size| size >= required_block_size)
    }
}

// Implement the global allocator trait
unsafe impl GlobalAlloc for SlabAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Try to find a fitting slab
        if let Some(index) = self.find_slab_index(&layout) {
            if let Some(ptr) = self.slabs[index].lock().allocate() {
                return ptr.as_ptr();
            }
        }
        
        // If no slab fits or all slabs are full, use fallback allocator
        self.fallback_allocator.lock().allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Find which region this pointer belongs to
        let ptr_addr = ptr as usize;
        
        // Check each slab region
        for i in 0..BLOCK_SIZES.len() {
            let (region_start, region_end) = *self.slab_heap_regions[i].lock();
            
            if ptr_addr >= region_start && ptr_addr < region_end {
                // Found the slab this pointer belongs to
                // Wrap unsafe functions in unsafe blocks
                unsafe {
                    self.slabs[i].lock().deallocate(NonNull::new_unchecked(ptr));
                }
                return;
            }
        }
        
        // If not in any slab region, use fallback allocator
        // Wrap unsafe functions in unsafe blocks
        unsafe {
            self.fallback_allocator.lock().deallocate(
                NonNull::new_unchecked(ptr),
                layout
            );
        }
    }
}

// Define global allocator instance
#[global_allocator]
static ALLOCATOR: SlabAllocator = SlabAllocator::new();

// Maps the virtual heap pages to physical frames
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), &'static str> {
    // Map heap pages to physical frames
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // Allocate and map frames for the heap
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or("Failed to allocate frame for heap")?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        
        // Fix: Handle the Result without using ? operator
        unsafe {
            match mapper.map_to(page, frame, flags, frame_allocator) {
                Ok(tlb) => tlb.flush(),
                Err(_) => return Err("Failed to map page"),
            }
        }
    }

    // Initialize the allocator
    unsafe {
        ALLOCATOR.init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

// Heap debugging function - prints the status of the allocator
pub fn print_heap_status() {
    // Calculate used blocks for each slab size
    for (i, &size) in BLOCK_SIZES.iter().enumerate() {
        // Remove unnecessary unsafe block
        let (start, end) = *ALLOCATOR.slab_heap_regions[i].lock();
        let blocks_total = (end - start) / size;
        
        // Count free blocks (approximation since we don't store count)
        let mut free_count = 0;
        // Remove unnecessary unsafe block
        let mut current = ALLOCATOR.slabs[i].lock().free_blocks.as_ptr();
        while current != NonNull::<FreeBlock>::dangling().as_ptr() {
            free_count += 1;
            // Need to keep this unsafe because we're dereferencing a raw pointer
            current = unsafe { (*current).next.as_ptr() };
        }
        
        let used_blocks = blocks_total - free_count;
        
        crate::println!("Slab size {}: {}/{} blocks used ({}%)", 
            size, 
            used_blocks, 
            blocks_total,
            (used_blocks as f64 / blocks_total as f64 * 100.0) as usize
        );
    }
    
    // Print fallback allocator stats (if available)
    // Note: linked_list_allocator doesn't expose stats directly
    crate::println!("Fallback allocator: stats not available");
}