#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_kernel::println;
use rust_kernel::task;
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
    
    // Spawn some test tasks
    task::spawn("task1", task1);
    task::spawn("task2", task2);
    
    println!("Tasks created, starting scheduler...");
    
    // Start the scheduler
    task::yield_task();
    
    #[cfg(test)]
    test_main();
    
    println!("Kernel initialization complete!");
    rust_kernel::hlt_loop();
}

// Example task function
fn task1() -> ! {
    let id = task::current_task_id();
    println!("Task 1 (ID: {}) started", id);
    
    let mut counter = 0;
    loop {
        println!("Task 1: counter = {}", counter);
        counter += 1;
        
        // Yield to other tasks after 5 iterations
        if counter % 5 == 0 {
            println!("Task 1: yielding");
            task::yield_task();
        }
        
        // Slow down the task
        for _ in 0..10_000_000 {
            core::hint::spin_loop();
        }
    }
}

// Another example task function
fn task2() -> ! {
    let id = task::current_task_id();
    println!("Task 2 (ID: {}) started", id);
    
    let mut counter = 0;
    loop {
        println!("Task 2: counter = {}", counter);
        counter += 1;
        
        // Yield to other tasks after 3 iterations
        if counter % 3 == 0 {
            println!("Task 2: yielding");
            task::yield_task();
        }
        
        // Slow down the task
        for _ in 0..5_000_000 {
            core::hint::spin_loop();
        }
    }
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