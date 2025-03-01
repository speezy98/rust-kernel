
use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{
        FrameAllocator, PhysFrame, Size4KiB,
    },
    PhysAddr,
};

/// A frame allocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// # Safety
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
    
    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // Get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        
        // Map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        
        // Transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        
        // Create PhysFrame objects
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
    
    /// Returns the number of usable frames available.
    pub fn available_frames(&self) -> usize {
        self.usable_frames().count()
    }
    
    /// Returns the total memory size in bytes.
    pub fn total_memory_size(&self) -> u64 {
        let mut total = 0;
        for region in self.memory_map.iter() {
            total += region.range.end_addr() - region.range.start_addr();
        }
        total
    }
    
    /// Returns the usable memory size in bytes.
    pub fn usable_memory_size(&self) -> u64 {
        let mut total = 0;
        for region in self.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                total += region.range.end_addr() - region.range.start_addr();
            }
        }
        total
    }
    
    /// Prints memory map information for debugging.
    pub fn print_memory_map(&self) {
        crate::println!("Memory map:");
        for region in self.memory_map.iter() {
            crate::println!("  {:?}: {:#x} - {:#x} ({} bytes)",
                region.region_type,
                region.range.start_addr(),
                region.range.end_addr(),
                region.range.end_addr() - region.range.start_addr()
            );
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

/// A frame allocator that always returns `None`.
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        None
    }
}

/// A simple frame allocator that keeps track of allocated frames in a bitmap.
pub struct BitmapFrameAllocator {
    // We'll use a bitmap to track which frames are allocated (1 = allocated, 0 = free)
    bitmap: &'static mut [u8],
    // Start address of the memory region
    start_frame_number: usize,
    // Number of frames in the region
    frames_count: usize,
}

impl BitmapFrameAllocator {
    /// Creates a new bitmap frame allocator.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the bitmap covers all allocatable frames,
    /// and that it's initialized correctly (all zeros).
    pub unsafe fn new(
        bitmap: &'static mut [u8],
        start_frame_number: usize,
        frames_count: usize,
    ) -> Self {
        BitmapFrameAllocator {
            bitmap,
            start_frame_number,
            frames_count,
        }
    }
    
    /// Marks a frame as allocated
    fn mark_frame_allocated(&mut self, frame_number: usize) {
        let rel_frame = frame_number - self.start_frame_number;
        if rel_frame >= self.frames_count {
            return; // Out of range
        }
        
        let byte_index = rel_frame / 8;
        let bit_index = rel_frame % 8;
        
        self.bitmap[byte_index] |= 1 << bit_index;
    }
    
    /// Marks a frame as free
    fn mark_frame_free(&mut self, frame_number: usize) {
        let rel_frame = frame_number - self.start_frame_number;
        if rel_frame >= self.frames_count {
            return; // Out of range
        }
        
        let byte_index = rel_frame / 8;
        let bit_index = rel_frame % 8;
        
        self.bitmap[byte_index] &= !(1 << bit_index);
    }
    
    /// Checks if a frame is allocated
    fn is_frame_allocated(&self, frame_number: usize) -> bool {
        let rel_frame = frame_number - self.start_frame_number;
        if rel_frame >= self.frames_count {
            return false; // Out of range
        }
        
        let byte_index = rel_frame / 8;
        let bit_index = rel_frame % 8;
        
        (self.bitmap[byte_index] & (1 << bit_index)) != 0
    }
    
    /// Find the first free frame
    fn find_free_frame(&self) -> Option<usize> {
        for frame_number in self.start_frame_number..(self.start_frame_number + self.frames_count) {
            if !self.is_frame_allocated(frame_number) {
                return Some(frame_number);
            }
        }
        None
    }
}

unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(frame_number) = self.find_free_frame() {
            self.mark_frame_allocated(frame_number);
            let frame_addr = (frame_number * 4096) as u64;
            Some(PhysFrame::containing_address(PhysAddr::new(frame_addr)))
        } else {
            None
        }
    }
}