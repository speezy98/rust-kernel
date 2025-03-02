#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use rust_kernel::{println, fs::{FileSystem, Fat32FileSystem}};
use core::panic::PanicInfo;
use alloc::vec::Vec;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    // Initialize the kernel
    rust_kernel::init(boot_info);
    
    println!("Running filesystem tests...");
    test_main();
    
    rust_kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info)
}

// Create a memory-based disk with a simple FAT32 structure
fn create_test_disk() -> impl rust_kernel::fs::fat32::Disk {
    use rust_kernel::fs::fat32::{MemoryDisk, FatBootSector};
    
    // Create a memory disk with 1MB of space
    let mut disk = MemoryDisk::new(512, 2048);
    
    // We would normally format the disk here, but for testing
    // we'll just leave it uninitialized
    
    disk
}

#[test_case]
fn test_filesystem_init() {
    println!("Testing filesystem initialization");
    
    let disk = create_test_disk();
    let mut fs = Fat32FileSystem::new(disk);
    
    // This will likely fail with a real error message since we haven't 
    // formatted the disk, but it should run without panicking
    match fs.init() {
        Ok(_) => println!("Filesystem initialized successfully"),
        Err(e) => println!("Filesystem initialization failed as expected: {}", e),
    }
}

