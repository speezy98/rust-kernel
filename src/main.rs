#![no_std]  // On n'utilise pas la bibliothèque standard
#![no_main] // On n'utilise pas le point d'entrée standard


mod vga_buffer;

use core::panic::PanicInfo;

// Gestionnaire de panique
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some numbers: {} {}", 42, 1.337).unwrap();

    loop {}
}

