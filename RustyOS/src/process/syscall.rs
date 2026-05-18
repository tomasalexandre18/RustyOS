use crate::console_print;
use crate::{console_println, HHDM_OFFSET};
use crate::{debug_balise, memory, print, process, serial};
use core::arch::{asm, global_asm};
use core::sync::atomic::Ordering;
use crate::{msr, println};
use crate::framebuffer::get_framebuffer;
use crate::process::scheduler::schedule;

const MSR_STAR: u32 = 0xC0000081;
const MSR_LSTAR: u32 = 0xC0000082;
const MSR_SFMASK: u32 = 0xC0000084;
const MSR_EFER: u32 = 0xC0000080;

const MSR_KERNEL_GS_BASE: u32 = 0xC0000102;

#[repr(C)]
struct KernelGsData {
    _pad: u64,        // offset 0x00
    kernel_rsp: u64,  // offset 0x08
    user_rsp: u64,    // offset 0x10
}

static mut KERNEL_GS_DATA: KernelGsData = KernelGsData {
    _pad: 0,
    kernel_rsp: 0,
    user_rsp: 0,
};

global_asm!(
    r#"
    .global syscall_handler
    syscall_handler:
    swapgs

    mov qword ptr gs:[0x10], rsp
    mov rsp, qword ptr gs:[0x08]

    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15

    mov rdi, rsp
    call syscall_handler_rust

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax

    mov rsp, qword ptr gs:[0x10]
    swapgs
    sysretq
    "#
);

unsafe extern "C" {
    fn syscall_handler();
}

#[repr(C)]
pub struct FramebufferInfo {
    pub addr:   *mut u8,  // adresse virtuelle du FB mappé
    pub width:  u32,
    pub height: u32,
    pub pitch:  u32,
    pub bpp:    u8,
}

const USER_FB_MAP_START: u64 = 0x0000_7000_0000_0000; // adresse virtuelle de départ pour le mapping du framebuffer dans l'espace utilisateur

#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler_rust(tf: &mut crate::idt::TrapFrame) {
    // pour l'instant on ne gère que le syscall numéro 1 : exit
    let num = tf.rax;
    let arg1 = tf.rdi;
    let arg2 = tf.rsi;
    let arg3 = tf.rdx;
    let arg4 = tf.r10; // 4ème argument passé dans r10 selon la convention de syscall
    let arg5 = tf.r8;  // 5ème argument passé dans r8
    let arg6 = tf.r9;  // 6ème argument passé dans r9
    console_println!("syscall_handler_rust called with num={}, arg1={}, arg2={}, arg3={}, arg4={}, arg5={}, arg6={})", num, arg1, arg2, arg3, arg4, arg5, arg6);


    // rcx fix bits 48-63 to 0 to avoid issues with syscall/sysret
    if tf.rcx >= 0x0000_8000_0000_0000 {
        // adresse non-canonique ou kernel → SIGSEGV / error
        console_println!("syscall_handler_rust: invalid rcx value {:#x}, returning error", tf.rcx);
        
    }
    match num {
        1 => {
            // exit
            process::scheduler::kill_current();
            call_schdule();
            loop {}
        }
        2 => {
            // sleep
            let ms = arg1;
            process::scheduler::sleep_current(ms);
            call_schdule(); // call schedule to switch to the next process
            tf.rax = 0; // return 0 for success
        },
        3 => {
            // write to debug console [DEBUG - PROCESS - PID={}]: message
            let pid = process::scheduler::current_pid();
            let msg_ptr = arg1 as *const u8;
            let msg_len = arg2 as usize;
            let msg_slice = unsafe { core::slice::from_raw_parts(msg_ptr, msg_len) };
            if let Ok(msg_str) = core::str::from_utf8(msg_slice) {
                console_println!("[DEBUG - PROCESS - PID={}]: {}", pid, msg_str);
            } else {
                console_println!("[DEBUG - PROCESS - PID={}]: <invalid utf-8 message>", pid);
            }
            tf.rax = 0; // return 0 for success
        },
        4 => {
            // get_pid
            tf.rax = process::scheduler::current_pid();
        },
        5 => {
            // sbrk

            let pages = arg1;
            let proc = process::scheduler::current_process();
            if proc.is_null() {
                panic!("Je veux crever: syscall sbrk called with no current process");
            }
            unsafe {
                let mut new_heap_end = (*proc).heap_end;
                for _ in 0..pages {
                    let page = memory::base::alloc_frame().expect("sbrk: failed to allocate page");
                    memory::vmm::map_page((*proc).pml4t , new_heap_end, page, 0x7); // Present + RW + User
                    (*proc).heap_end += 0x1000; // 4KB
                    new_heap_end += 0x1000;
                }
                tf.rax = new_heap_end
            }
        },
        6 => {
            let framebuffer_info_ptr = arg1 as *mut FramebufferInfo;
            if framebuffer_info_ptr.is_null() {
                tf.rax = 1; // error code for null pointer
                return;
            }
            let framebuffer_kernel = get_framebuffer();
            if framebuffer_kernel.is_none() {
                tf.rax = 2; // error code for no framebuffer
                return;
            }

            let fb_info = framebuffer_kernel.unwrap();

            let addr_phys = fb_info.address() as u64 - HHDM_OFFSET.load(Ordering::Relaxed) as u64; // convertir adresse virtuelle du framebuffer en adresse physique
            let addr_start = addr_phys;
            console_println!("Mapping framebuffer at physical address {:#x}, size {} bytes", addr_start, fb_info.size());
            let addr_end = addr_start + fb_info.size() as u64;

            for addr in (addr_start..addr_end).step_by(0x1000) {
                memory::vmm::map_page(unsafe {(*process::scheduler::current_process()).pml4t}, USER_FB_MAP_START + (addr - addr_start) as u64, addr as u64, 0x7); // Present + RW + User
            }
            unsafe {
                (*framebuffer_info_ptr).addr = USER_FB_MAP_START as *mut u8;
                (*framebuffer_info_ptr).width = fb_info.width as u32;
                (*framebuffer_info_ptr).height = fb_info.height as u32;
                (*framebuffer_info_ptr).pitch = fb_info.pitch as u32;
                (*framebuffer_info_ptr).bpp = fb_info.bpp as u8;
            }
            tf.rax = 0; // success
        },
        _ => {
            console_println!("Unknown syscall number {}", num);
            tf.rax = u64::MAX; // return error code for unknown syscall
        }
    }
    console_println!("syscall_handler_rust returning with rax={}", tf.rax);
}

fn call_schdule() {
    unsafe {
        asm!("int 0x30", options(nostack, preserves_flags)); // interrupt 0x30 is used for scheduling, it will call the scheduler to switch to the next process
    }
}

pub fn init() {
    // activer SCE (SysCall Enable) dans EFER
    let efer = msr::read(MSR_EFER);
    msr::write(MSR_EFER, efer | 1);

    // STAR : bits 47:32 = CS kernel (0x08), bits 63:48 = CS user (0x18)
    // au SYSCALL : CS = STAR[47:32], SS = STAR[47:32]+8
    // au SYSRET  : CS = STAR[63:48]+16, SS = STAR[63:48]+8
    msr::write(MSR_STAR, (0x0008u64 << 32) | (0x0018u64 << 48));

    // LSTAR : adresse du handler
    msr::write(MSR_LSTAR, syscall_handler as *const () as u64);

    // SFMASK : masque IF au minimum (désactive les interruptions pendant le handler)
    msr::write(MSR_SFMASK, 0x200);
}

pub fn set_kernel_stack(kernel_rsp: u64) {
    unsafe {
        KERNEL_GS_DATA.kernel_rsp = kernel_rsp;
        // update the GS base to point to our TSS data structure
        let gs_base = &raw const KERNEL_GS_DATA as u64;
        msr::write(MSR_KERNEL_GS_BASE, gs_base);
    }
}
