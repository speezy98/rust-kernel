#![no_std]  // On n'utilise pas la bibliothèque standard
#![no_main] // On n'utilise pas le point d'entrée standard


mod vga_buffer;

use core::panic::PanicInfo;


#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    #[warn(clippy::empty_loop)]
    loop {}
}

// Gestionnaire de panique
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}