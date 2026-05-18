use limine::request::ExecutableAddressResponse;
use crate::memory;

#[unsafe(link_section = ".user_text")]
#[unsafe(no_mangle)]
pub extern "C" fn user_entry() -> ! {
    sys_debug_write(b"Hello from user space!\n".as_ptr(), 25);
    let pid = sys_get_pid();
    sys_debug_write(b"PID: ".as_ptr(), 5);
    let mut buf = [0u8; 20];
    let mut n = 0;
    let mut temp = pid;
    while temp > 0 {
        buf[n] = b'0' + (temp % 10) as u8;
        temp /= 10;
        n += 1;
    }
    for i in 0..n / 2 {
        let tmp = buf[i];
        buf[i] = buf[n - 1 - i];
        buf[n - 1 - i] = tmp;
    }
    sys_debug_write(buf.as_ptr(), n as u64);
    sys_exit(0);
}

/*
RAX numéro syscall
RDI arg1
RSI arg2
RDX arg3
R10 arg4
R8 arg5
R9 arg6
RAX valeur de retour
 */
#[unsafe(link_section = ".user_text")]
fn syscall(num: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") num,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            in("r10") arg4,
            in("r8") arg5,
            in("r9") arg6,
            lateout("rax") ret,
        );
    }
    ret
}

#[unsafe(link_section = ".user_text")]
fn sys_exit(status: u64) -> ! {
    syscall(1, 0, 0, 0, 0, 0, 0);
    loop {}
}

#[unsafe(link_section = ".user_text")]
fn sys_sleep(ms: u64) {
    syscall(2, ms, 0, 0, 0, 0, 0);
}

#[unsafe(link_section = ".user_text")]
fn sys_debug_write(msg_ptr: *const u8, msg_len: u64) {
    syscall(3, msg_ptr as u64, msg_len, 0, 0, 0, 0);
}

#[unsafe(link_section = ".user_text")]
fn sys_get_pid() -> u64 {
    syscall(4, 0, 0, 0, 0, 0, 0)
}


unsafe extern "C" {
    static user_text_start: u8;
    static user_text_end: u8;
}

pub fn map_user_process(kernel: &ExecutableAddressResponse, pml4t: *mut u64) {
    let mut start: u64;
    let mut end: u64;
    unsafe {
        start = &raw const user_text_start as u64;
        end = &raw const user_text_end as u64;
    }
    let size = end - start;
    let num_pages = (size + 0xFFF) / 0x1000;
    for i in 0..num_pages {
        let offset = kernel.physical_base as i64 - kernel.virtual_base as i64;
        let virt = start + i * 0x1000;
        let phys = (virt as i64 + offset) as u64;
        memory::vmm::map_page(pml4t, virt, phys, 0x7); // Present + RW + User
    }
}