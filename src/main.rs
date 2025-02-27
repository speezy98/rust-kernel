#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_kernel::println;
use x86_64::VirtAddr;
use alloc::{boxed::Box, vec::Vec};

// Define the kernel entry point with bootloader
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    println!("Hello from Rust Kernel!");
    
    // Initialize memory subsystems
    rust_kernel::init(boot_info);
    
    println!("Heap initialized, performing basic heap test...");
    
    // Test heap allocation
    let heap_value = Box::new(42);
    println!("heap_value at {:p} = {}", heap_value, *heap_value);
    
    // Create a vector
    let mut vec = Vec::new();
    for i in 0..10 {
        vec.push(i);
    }
    println!("Created a vector: {:?}", vec);
    
    #[cfg(test)]
    test_main();
    
    println!("Kernel initialization complete!");
    rust_kernel::hlt_loop();
}

/// This function is called on panic.
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    rust_kernel::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info)
}