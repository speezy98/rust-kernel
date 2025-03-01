#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use rust_kernel::{println, memory};
use core::panic::PanicInfo;
use x86_64::VirtAddr;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    // Initialize the kernel
    rust_kernel::init(boot_info);
    
    println!("Running memory tests...");
    test_main();
    
    rust_kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info)
}

#[test_case]
fn test_virt_to_phys() {
    // Test the virtual to physical address translation
    // VGA buffer is a known mapped address
    let virt_addr = VirtAddr::new(0xb8000);
    let phys_mem_offset = VirtAddr::new(0xb8000);
    
    let phys_addr = unsafe { memory::virt_to_phys(virt_addr, phys_mem_offset) };
    
    // The VGA buffer should be mapped to a physical address
    assert!(phys_addr.is_some());
    
    // In identity mapping, the physical address should be the same
    assert_eq!(phys_addr.unwrap().as_u64(), 0xb8000);
}

#[test_case]
fn test_slab_allocator() {
    use rust_kernel::slab_allocator;
    
    // Allocate memory of different sizes
    let small = alloc::boxed::Box::new([0u8; 16]);
    let medium = alloc::boxed::Box::new([0u8; 256]);
    let large = alloc::boxed::Box::new([0u8; 4096]);
    
    // Make sure they don't overlap (basic sanity check)
    let small_ptr = small.as_ptr() as usize;
    let medium_ptr = medium.as_ptr() as usize;
    let large_ptr = large.as_ptr() as usize;
    
    assert!(small_ptr != medium_ptr);
    assert!(small_ptr != large_ptr);
    assert!(medium_ptr != large_ptr);
    
    // Print allocator status for debugging
    slab_allocator::print_heap_status();
}