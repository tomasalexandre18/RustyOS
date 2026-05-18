#![no_std]
#![no_main]
#![no_builtins]
extern crate alloc;

use alloc::string::ToString;

unsafe extern "C" {
    fn main() -> u64;
}

pub mod syscall;
mod memory;

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    syscall::debug_write(info.to_string().as_ptr(), info.to_string().len() as u64);
    syscall::exit(1);
}

// entrypoint
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    memory::init();
    unsafe {
        syscall::exit(main());
    }
}