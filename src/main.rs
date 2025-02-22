#![no_std]  // On n'utilise pas la bibliothèque standard
#![no_main] // On n'utilise pas le point d'entrée standard

use core::panic::PanicInfo;

// Point d'entrée du kernel
static HELLO: &[u8] = b"Hello World!";

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}

// Gestionnaire de panique
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}