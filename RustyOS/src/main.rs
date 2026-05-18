#![no_std]
#![no_main]
#![no_builtins]
extern crate alloc;

use core::sync::atomic::{AtomicU64, Ordering};
use crate::framebuffer::init_global_console;

mod limine_imp;
mod serial;
mod io;
mod memory;
mod idt;
mod gdt;
mod time;
mod cpuid;
mod msr;
mod process;
mod framebuffer;
mod user;

mod font;


pub static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    serial::init();
    println!("PANIC: {}", info.message());

    // find if CONSOLE is initialized
    let console_initialized = framebuffer::is_console_initialized();
    if console_initialized {
        console_println!("PANIC: {}", info.message());
    }
    loop {
        unsafe {
            core::arch::asm!("cli; hlt", options(noreturn))
        }
    }
}

fn idle_process() -> ! {
    console_println!("Idle process started");
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() {
    serial::init();
    println!("RustyOS boot...");

    assert!(limine_imp::BASE_REVISION.is_supported(), "Limine base revision not supported");

    let hhdm_offset= limine_imp::HHDM_OFFSET_REQUEST.response().unwrap().offset;
    HHDM_OFFSET.store(hhdm_offset, Ordering::Relaxed);

    let memmap = limine_imp::FRAMEBUFFER_REQUEST_MEMORY_MAP.response().unwrap();
    let kernel_address = limine_imp::KERNEL_ADDRESS_REQUEST.response().unwrap();

    memory::base::init(memmap);
    memory::vmm::init(kernel_address, memmap);
    memory::heap_kernel::init();

    init_global_console();
    console_println!("Framebuffer console initialized");

    gdt::init();

    idt::pic::remap(0x20, 0x28);
    idt::pic::mask_all();
    idt::init();

    time::pit::init();
    time::tsc::init();
    time::apic::init();

    // activate interrupts
    unsafe {
        core::arch::asm!("sti");
    }

    process::syscall::init();

    console_println!("System initialization complete, starting processes...");


    console_println!("Creating processes...");
    let idle = process::create_process(idle_process as *const () as u64, "idle", 0x4000);
    console_println!("Idle process created with PID {}", unsafe { (*idle).pid });
    let ramdisk = limine_imp::get_ram_disk().expect("No RAM disk module found");
    let user_proc = process::loader::create_process_from_elf(ramdisk, "user_process").expect("Failed to create user process from ELF");
    user::map_user_process(kernel_address, unsafe {(*user_proc).pml4t});
    console_println!("User process created with PID {} -> entry point: {:p}", unsafe { (*user_proc).pid }, user::user_entry as *const () as *const u8);
    process::scheduler::init();
    process::scheduler::start();
}