
#![no_std]
#![no_main]

mod lang_items;
mod sbi;
mod console;

core::arch::global_asm!(include_str!("entry.asm"));

use crate::sbi::shutdown;


fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    });
}



#[no_mangle]
pub fn rust_main() -> ! {
    println!("hello world!");
    clear_bss();
    shutdown();
}