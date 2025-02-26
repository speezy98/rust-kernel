// memory/mod.rs
use crate::println;
use x86_64::{
    structures::paging::{
        PageTable, OffsetPageTable, PhysFrame, Size4KiB, 
        page_table::{FrameError, PageTableEntry, PageTableIndex}, PageTableFlags
    },
    PhysAddr, VirtAddr,
    registers::control::Cr3,
};

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
    for &index in &table_indexes {
        // Convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };
        
        // Read the page table entry and update `frame`
        let entry = &table[index];
        
        if !entry.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }
        
        // Check if this entry is a huge page (either 2MiB or 1GiB)
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Huge pages must be supported for 2MiB and 1GiB pages
            return Some(TranslateResult {
                frame: match frame_from_page_table_entry(entry.clone(), &table_indexes, index) {
                    Ok(frame) => frame,
                    Err(_) => return None,
                },
                flags: entry.flags(),
            });
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
        frame: frame,
        flags: PageTableFlags::empty(), // Actual flags would be retrieved from the entry
    })
}

/// Convert a page table entry for a huge page into a normal 4KiB frame
fn frame_from_page_table_entry(
    _entry: PageTableEntry, 
    _indexes: &[PageTableIndex; 4], 
    _level_index: PageTableIndex
) -> Result<PhysFrame<Size4KiB>, FrameError> {
    // Implementation depends on page size (4KiB, 2MiB, or 1GiB)
    // This is simplified; would need more details for complete implementation
    unimplemented!("Huge page translation not yet implemented")
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

#[cfg(test)]
mod tests {
    use super::*;
    use bootloader::bootinfo::BootInfo;
    
    #[test_case]
    fn test_identity_mapping() {
        // This assumes bootloader has set up identity mapping
        // for part of the physical memory
        let boot_info = unsafe { &*(0x1000 as *const BootInfo) };
        let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
        
        // Test a known identity-mapped address (usually boot code)
        let virt_addr = VirtAddr::new(0x1000);
        if let Some(phys_addr) = unsafe { virt_to_phys(virt_addr, phys_mem_offset) } {
            assert_eq!(phys_addr.as_u64(), 0x1000);
        } else {
            panic!("Identity mapping test failed: address not mapped");
        }
    }
    
    #[test_case]
    fn test_kernel_mapping() {
        // Test an address in the kernel's address space
        let boot_info = unsafe { &*(0x1000 as *const BootInfo) };
        let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
        
        // This is a simplification - you'd need to know a valid kernel address
        extern "C" {
            static _start: u8;
        }
        
        let kernel_addr = VirtAddr::new(&_start as *const u8 as u64);
        if let Some(phys_addr) = unsafe { virt_to_phys(kernel_addr, phys_mem_offset) } {
            println!("Kernel start mapped to physical address: {:?}", phys_addr);
            // Can't assert exact address, but it should be less than physical memory size
            assert!(phys_addr.as_u64() < 0x1_0000_0000); // Less than 4GB
        } else {
            panic!("Kernel mapping test failed: address not mapped");
        }
    }
    
    #[test_case]
    fn test_unmapped_address() {
        // Test an address that should not be mapped
        let boot_info = unsafe { &*(0x1000 as *const BootInfo) };
        let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
        
        // Very high address that should not be mapped
        let unmapped_addr = VirtAddr::new(0xFFFF_FFFF_FFFF_0000);
        let result = unsafe { virt_to_phys(unmapped_addr, phys_mem_offset) };
        assert!(result.is_none());
    }
}