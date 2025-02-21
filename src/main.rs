#![no_std]  // On n'utilise pas la bibliothèque standard
#![no_main] // On n'utilise pas le point d'entrée standard

use core::panic::PanicInfo;

// Point d'entrée du kernel
#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}

// Gestionnaire de panique
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}