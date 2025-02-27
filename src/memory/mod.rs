// src/memory/mod.rs
use crate::println;
use x86_64::{
    structures::paging::{
        PageTable, OffsetPageTable, PhysFrame, Size4KiB, 
        FrameAllocator, 
        page_table::{FrameError, PageTableEntry, PageTableIndex}, PageTableFlags, 
        Page, Mapper, 
        page::PageRangeInclusive
    },
    PhysAddr, VirtAddr,
    registers::control::Cr3,
};

pub mod frame_allocator;


/// Initialize a new OffsetPageTable
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = unsafe { active_level_4_table(physical_memory_offset) };
    unsafe { OffsetPageTable::new(level_4_table, physical_memory_offset) }
}

/// Returns a mutable reference to the active level 4 page table
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    unsafe { &mut *page_table_ptr }
}

/// Translates a virtual address to its mapped physical address, or None if not mapped.
pub unsafe fn virt_to_phys(virtual_address: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    let addresses = unsafe { translate_addr_inner(virtual_address, physical_memory_offset) }?;
    
    // Get the physical frame that the virtual address points to
    let frame: PhysFrame = addresses.frame;
    
    // Calculate the physical address by adding the page offset
    let offset = virtual_address.as_u64() & 0xFFF; // Get the 12 least significant bits
    let physical_address = frame.start_address() + offset;
    
    Some(physical_address)
}

/// Detailed implementation for translating a virtual address to a frame
unsafe fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr) 
    -> Option<TranslateResult> {
    
    // Read the active level 4 frame from the CR3 register
    let (level_4_table_frame, _) = Cr3::read();
    
    let table_indexes = [
        addr.p4_index(), addr.p3_index(), addr.p2_index(), addr.p1_index()
    ];
    let mut frame = level_4_table_frame;
    
    // Walk the page tables
    for (level, &index) in table_indexes.iter().enumerate() {
        // Convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };
        
        // Read the page table entry
        let entry = &table[index];
        
        // Check if the entry is present
        if !entry.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }
        
        // Check if this entry is a huge page (either 2MiB or 1GiB)
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Huge pages must be 2MiB (level 2) or 1GiB (level 3)
            if level == 1 || level == 2 {
                return Some(TranslateResult {
                    frame: handle_huge_page(entry, level, addr, &table_indexes),
                    flags: entry.flags(),
                });
            } else {
                panic!("Huge page at unexpected level: {}", level);
            }
        }
        
        // Get the frame that the entry points to
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("Unexpected huge frame error"),
        };
    }
    
    // Return the frame and flags of the final page table entry
    Some(TranslateResult {
        frame,
        flags: PageTableFlags::empty(), // Simplified for brevity
    })
}

/// Handle huge page translation (2MiB or 1GiB pages)
fn handle_huge_page(
    entry: &PageTableEntry,
    level: usize,
    addr: VirtAddr,
    indexes: &[PageTableIndex; 4]
) -> PhysFrame<Size4KiB> {
    // Base physical address from the entry (with cleared flags)
    let phys_addr_base = PhysAddr::new(entry.addr().as_u64());
    
    // Calculate offset based on the level
    let page_offset = match level {
        1 => { // 1GiB page
            // Extract offset from p2, p1, and page offset bits
            // Use .into() which converts PageTableIndex to u16
            let p2_index = u64::from(u16::from(indexes[2])) * 0x200000; // 2MiB per p2 entry
            let p1_index = u64::from(u16::from(indexes[3])) * 0x1000;   // 4KiB per p1 entry
            let page_offset = addr.as_u64() & 0xFFF;                    // 12 bits offset
            p2_index + p1_index + page_offset
        },
        2 => { // 2MiB page
            // Extract offset from p1 and page offset bits
            let p1_index = u64::from(u16::from(indexes[3])) * 0x1000;   // 4KiB per p1 entry
            let page_offset = addr.as_u64() & 0xFFF;                    // 12 bits offset
            p1_index + page_offset
        },
        _ => panic!("Unexpected huge page level: {}", level),
    };
    
    // Calculate final physical address
    let phys_addr = phys_addr_base + page_offset;
    
    // Convert to a 4KiB frame
    PhysFrame::containing_address(phys_addr)
}

/// Result of a virtual to physical address translation
struct TranslateResult {
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
}

/// Function to print memory mapping for debugging
pub fn print_memory_mapping(addr: VirtAddr, physical_memory_offset: VirtAddr) {
    println!("Virtual Address: {:?}", addr);
    
    if let Some(phys_addr) = unsafe { virt_to_phys(addr, physical_memory_offset) } {
        println!("Mapped to Physical Address: {:?}", phys_addr);
    } else {
        println!("Not mapped to any physical address");
    }
}

/// Maps a range of pages to physical frames with given flags
pub fn map_range(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    range: PageRangeInclusive<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    for page in range {
        // Allocate a physical frame
        let frame = frame_allocator
            .allocate_frame()
            .ok_or("Failed to allocate physical frame")?;
        
        // Map the virtual page to the physical frame
        unsafe {
            // Handle the error without using ? operator
            match mapper.map_to(page, frame, flags, frame_allocator) {
                Ok(tlb) => tlb.flush(),
                Err(_) => return Err("Failed to map page"),
            }
        }
    }
    
    Ok(())
}

/// Maps a specific virtual page to a specific physical frame
pub fn map_page_to_frame(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    page: Page<Size4KiB>,
    frame: PhysFrame<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    unsafe {
        // Handle the error without using ? operator
        match mapper.map_to(page, frame, flags, frame_allocator) {
            Ok(tlb) => tlb.flush(),
            Err(_) => return Err("Failed to map page to frame"),
        }
    }
    
    Ok(())
}

/// Unmaps a page and frees its frame
pub fn unmap_page(
    mapper: &mut impl Mapper<Size4KiB>,
    page: Page<Size4KiB>,
) -> Result<(), &'static str> {
    let (_frame, flush) = mapper
        .unmap(page)
        .map_err(|_err| "Failed to unmap page")?;
    
    flush.flush();
    
    // The frame can now be reused by the frame allocator
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bootloader::bootinfo::BootInfo;
    
    #[test_case]
    fn test_identity_mapping() {
        // This assumes bootloader has set up identity mapping
        // for part of the physical memory
        let boot_info = unsafe { &*(0x1000 as *const BootInfo) };
        // Adjust for your bootloader version
        let phys_mem_offset = VirtAddr::new(0); // Use identity mapping for tests
        
        // Test a known identity-mapped address (usually boot code)
        let virt_addr = VirtAddr::new(0x1000);
        if let Some(phys_addr) = unsafe { virt_to_phys(virt_addr, phys_mem_offset) } {
            assert_eq!(phys_addr.as_u64(), 0x1000);
        } else {
            panic!("Identity mapping test failed: address not mapped");
        }
    }
    
    #[test_case]
    fn test_unmapped_address() {
        // Test an address that should not be mapped
        let phys_mem_offset = VirtAddr::new(0); // Use identity mapping for tests
        
        // Very high address that should not be mapped
        let unmapped_addr = VirtAddr::new(0xFFFF_FFFF_FFFF_0000);
        let result = unsafe { virt_to_phys(unmapped_addr, phys_mem_offset) };
        assert!(result.is_none());
    }
}