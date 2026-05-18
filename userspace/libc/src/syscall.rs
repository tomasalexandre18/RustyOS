
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
        out("rcx") _,
        out("r11") _,
        options(nostack),
        );
    }
    ret
}

pub fn exit(status: u64) -> ! {
    syscall(1, status, 0, 0, 0, 0, 0);
    loop {}
}

pub fn sleep(ms: u64) {
    syscall(2, ms, 0, 0, 0, 0, 0);
}

pub fn debug_write(msg_ptr: *const u8, msg_len: u64) {
    syscall(3, msg_ptr as u64, msg_len, 0, 0, 0, 0);
}

pub fn get_pid() -> u64 {
    syscall(4, 0, 0, 0, 0, 0, 0)
}

pub fn sbrk(pages: u64) -> *mut u8 {
    syscall(5, pages, 0, 0, 0, 0, 0) as *mut u8
}

#[repr(C)]
pub struct FramebufferInfo {
    pub addr:   *mut u8,  // adresse virtuelle du FB mappé
    pub width:  u32,
    pub height: u32,
    pub pitch:  u32,
    pub bpp:    u8,
}

impl FramebufferInfo {
    pub fn default() -> FramebufferInfo {
        FramebufferInfo {
            addr: core::ptr::null_mut(),
            width: 0,
            height: 0,
            pitch: 0,
            bpp: 0,
        }
    }
}

pub fn get_framebuffer(info: *mut FramebufferInfo) -> i64 {
    syscall(6, info as u64, 0, 0, 0, 0, 0) as i64
}

