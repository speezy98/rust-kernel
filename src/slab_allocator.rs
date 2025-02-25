// Soon lib.rs
#![no_std]
#![feature(const_mut_refs)]
#![feature(allocator_api)]

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use spin::Mutex;

// Define the fixed sizes we'll support in our slab allocator
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

struct Slab {
    block_size: usize,
    free_blocks: NonNull<FreeBlock>,
    blocks_count: usize,
}

impl Slab {
    const fn new() -> Self {
        Slab {
            block_size: 0,
            free_blocks: NonNull::dangling(),
            blocks_count: 0,
        }
    }

    fn init(&mut self, block_size: usize, heap_start: usize, heap_size: usize) {
        self.block_size = block_size;
        let blocks_count = heap_size / block_size;
        self.blocks_count = blocks_count;
        
        // Initialize free block list
        let mut current_block = heap_start as *mut FreeBlock;
        for _ in 0..blocks_count {
            unsafe {
                (*current_block).next = NonNull::new(current_block.add(1)).unwrap();
                current_block = current_block.add(1);
            }
        }
        
        // Set the last block's next pointer to null
        unsafe {
            (*current_block.sub(1)).next = NonNull::dangling();
        }
        
        self.free_blocks = unsafe { NonNull::new(heap_start as *mut FreeBlock).unwrap() };
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

// Slab allocator structure
pub struct SlabAllocator {
    slabs: [Mutex<Slab>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl SlabAllocator {
    // Create a new empty slab allocator
    pub const fn new() -> Self {
        const EMPTY_SLAB: Mutex<Slab> = Mutex::new(Slab::new());
        SlabAllocator {
            slabs: [EMPTY_SLAB; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }
    
    // Initialize the allocator with a given heap area
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        // Split the heap into equal parts for each slab size
        let slab_heap_size = heap_size / BLOCK_SIZES.len();
        let mut current_heap_start = heap_start;
        
        // Initialize each slab with its portion of the heap
        for (i, &block_size) in BLOCK_SIZES.iter().enumerate() {
            self.slabs[i].lock().init(block_size, current_heap_start, slab_heap_size);
            current_heap_start += slab_heap_size;
        }
        
        // Initialize fallback allocator with remaining space
        let remaining_size = heap_size - (slab_heap_size * BLOCK_SIZES.len());
        self.fallback_allocator.init(current_heap_start as *mut u8, remaining_size);
    }
    
    // Find the appropriate slab for a given layout
    fn find_slab_index(&self, layout: &Layout) -> Option<usize> {
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
        self.fallback_allocator.allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Find which slab this pointer belongs to
        if let Some(index) = self.find_slab_index(&layout) {
            let slab_start = self.slabs[index].lock().free_blocks.as_ptr() as usize;
            let slab_end = slab_start + self.slabs[index].lock().blocks_count * self.slabs[index].lock().block_size;
            let ptr_addr = ptr as usize;
            
            if ptr_addr >= slab_start && ptr_addr < slab_end {
                self.slabs[index].lock().deallocate(NonNull::new_unchecked(ptr));
                return;
            }
        }
        
        // If not in any slab, use fallback allocator
        self.fallback_allocator.deallocate(
            NonNull::new_unchecked(ptr),
            layout
        );
    }
}

// Define global allocator instance
#[global_allocator]
static ALLOCATOR: SlabAllocator = SlabAllocator::new();

// Initialization function to be called at startup
pub fn init_heap() {
    const HEAP_START: usize = 0x_4444_4444_0000;
    const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
    
    unsafe {
        // Ensure virtual memory is mapped before initializing
        // This would call your virt_to_phys mapping function
        ALLOCATOR.init(HEAP_START, HEAP_SIZE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    
    #[test_case]
    fn test_small_allocations() {
        // Initialize allocator
        init_heap();
        
        // Allocate and deallocate many small blocks
        let mut v = Vec::new();
        for i in 0..1000 {
            v.push(i);
        }
        assert_eq!(v.iter().sum::<u64>(), 499500);
    }
    
    #[test_case]
    fn test_large_allocation() {
        // Initialize allocator
        init_heap();
        
        // Allocate a large block that will use the fallback allocator
        let large_vec = vec![0u8; 10000];
        assert_eq!(large_vec.len(), 10000);
    }
    
    #[test_case]
    fn test_multiple_allocations() {
        // Initialize allocator
        init_heap();
        
        // Create multiple allocation vectors of different sizes
        let mut small = Vec::new();
        let mut medium = Vec::new();
        let mut large = Vec::new();
        
        for i in 0..100 {
            small.push(i);
            medium.push(vec![i; 10]);
            large.push(vec![i; 100]);
        }
        
        assert_eq!(small.len(), 100);
        assert_eq!(medium.len(), 100);
        assert_eq!(large.len(), 100);
        
        // Free memory by clearing vectors
        small.clear();
        medium.clear();
        large.clear();
        
        // Allocate again to check if memory is properly reused
        let mut v = Vec::new();
        for i in 0..1000 {
            v.push(i);
        }
        assert_eq!(v.len(), 1000);
    }
}