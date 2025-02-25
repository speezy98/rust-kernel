#![no_std]  // On n'utilise pas la bibliothèque standard
#![no_main] // On n'utilise pas le point d'entrée standard
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod vga_buffer;
mod serial;

use core::panic::PanicInfo;



pub trait Testable {
    fn run(&self) -> ();
}

// Gestionnaire de panique
#[cfg(not(test))] 
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info);
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}

}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    #[cfg(test)]
    test_main();
    #[allow(clippy::empty_loop)]
    loop {}
}


