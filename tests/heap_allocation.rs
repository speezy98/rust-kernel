#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use rust_kernel::{println, memory, slab_allocator};
use x86_64::VirtAddr;
use alloc::{boxed::Box, vec::Vec, rc::Rc};
use rust_kernel::memory::frame_allocator::BootInfoFrameAllocator;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    // Initialize the kernel
    rust_kernel::init(boot_info);
    
    println!("Running allocator tests...");
    test_main();
    
    println!("Tests completed!");
    rust_kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info)
}

#[test_case]
fn test_simple_allocation() {
    println!("Running test_simple_allocation");
    let heap_value_1 = Box::new(42);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 42);
    assert_eq!(*heap_value_2, 13);
}

#[test_case]
fn test_large_vec() {
    // Allocate a large vector to test dynamic memory allocation
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn test_many_boxes() {
    // Allocate many small boxes to test the slab allocator
    for i in 0..100 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[test_case]
fn test_drop_cleanup() {
    // Test that memory is properly reclaimed after dropping
    let mut vec = Vec::new();
    
    // First allocation
    for i in 0..100 {
        vec.push(Box::new([i; 1000]));
    }
    
    // Drop all allocations
    drop(vec);
    
    // Should be able to allocate again
    let mut vec2 = Vec::new();
    for i in 0..100 {
        vec2.push(Box::new([i; 1000]));
    }
    
    // Verify contents
    for (i, boxed) in vec2.iter().enumerate() {
        assert_eq!(boxed[0], i as u64);
    }
}

#[test_case]
fn test_fragmentation_resistance() {
    // Test resistance to fragmentation by alternating allocations of different sizes
    let mut small_boxes = Vec::new();
    let mut large_boxes = Vec::new();
    
    // Allocate alternating sizes
    for i in 0..10 {
        small_boxes.push(Box::new([i as u8; 10]));
        large_boxes.push(Box::new([i as u8; 1000]));
    }
    
    // Drop small allocations
    drop(small_boxes);
    
    // Should be able to allocate small objects again
    let mut new_small_boxes = Vec::new();
    for i in 10..20 {
        new_small_boxes.push(Box::new([i as u8; 10]));
    }
    
    // Verify no corruption occurred
    for (i, large_box) in large_boxes.iter().enumerate() {
        assert_eq!(large_box[0], i as u8);
    }
    
    for (i, small_box) in new_small_boxes.iter().enumerate() {
        assert_eq!(small_box[0], (i + 10) as u8);
    }
}

#[test_case]
fn test_reference_counting() {
    // Test reference counting with Rc
    let rc_val = Rc::new(42);
    let rc_clone = rc_val.clone();
    
    assert_eq!(*rc_val, 42);
    assert_eq!(*rc_clone, 42);
    assert_eq!(Rc::strong_count(&rc_val), 2);
    
    drop(rc_val);
    assert_eq!(Rc::strong_count(&rc_clone), 1);
}