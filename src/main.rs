#![no_std]  // On n'utilise pas la bibliothèque standard
#![no_main] // On n'utilise pas le point d'entrée standard


mod vga_buffer;

use core::panic::PanicInfo;

// Gestionnaire de panique
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");
    panic!("Some panic message");
    loop {}
}

