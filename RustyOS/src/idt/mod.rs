pub mod pic;

use crate::console_print;
use crate::{console_println, process, serial, time};
use core::arch::global_asm;
use core::include_str;
use crate::{println};
use crate::gdt::set_kernel_stack;

global_asm!(include_str!("idt_stubs.s"));

global_asm!(include_str!("isr_common.s"));

include!("idt_isrs.rs");

#[repr(C, align(16))]
#[derive(Copy, Clone)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

#[repr(C, packed)]
struct IdtPointer {
    #[allow(dead_code)]
    limit: u16,
    base: u64,
}

struct Idt {
    entries: [IdtEntry; 256],
}

static mut IDT: Idt = Idt {
    entries: [IdtEntry {
        offset_low: 0,
        selector: 0,
        ist: 0,
        type_attr: 0,
        offset_mid: 0,
        offset_high: 0,
        zero: 0,
    }; 256],
};


static mut IDT_PTR: IdtPointer = IdtPointer {
    limit: (size_of::<Idt>() - 1) as u16,
    base: 0
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TrapFrame {
    // poussés par isr_common (ordre inverse des push)
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9:  u64,
    pub r8:  u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    // poussés par les stubs
    pub int_no:    u64,
    pub error_code: u64,

    // poussés automatiquement par le CPU
    pub rip:    u64,
    pub cs:     u64,
    pub rflags: u64,
    pub rsp:    u64,
    pub ss:     u64,
}

// impl null function init for TrapFrame to silence
impl TrapFrame {
    // display impl for TrapFrame for debugging

    pub fn display_console(&self) {
        console_println!("TrapFrame {{");
        console_println!("  r15: {:#018x}", self.r15);
        console_println!("  r14: {:#018x}", self.r14);
        console_println!("  r13: {:#018x}", self.r13);
        console_println!("  r12: {:#018x}", self.r12);
        console_println!("  r11: {:#018x}", self.r11);
        console_println!("  r10: {:#018x}", self.r10);
        console_println!("  r9 : {:#018x}", self.r9);
        console_println!("  r8 : {:#018x}", self.r8);
        console_println!("  rbp: {:#018x}", self.rbp);
        console_println!("  rdi: {:#018x}", self.rdi);
        console_println!("  rsi: {:#018x}", self.rsi);
        console_println!("  rdx: {:#018x}", self.rdx);
        console_println!("  rcx: {:#018x}", self.rcx);
        console_println!("  rbx: {:#018x}", self.rbx);
        console_println!("  rax: {:#018x}", self.rax);
        console_println!("  int_no: {:#018x}", self.int_no);
        console_println!("  error_code: {:#018x}", self.error_code);
        console_println!("  rip: {:#018x}", self.rip);
        console_println!("  cs: {:#018x}", self.cs);
        console_println!("  rflags: {:#018x}", self.rflags);
        console_println!("  rsp: {:#018x}", self.rsp);
        console_println!("  ss: {:#018x}", self.ss);
        console_println!("}}");
    }

    pub fn display_serial(&self) {
        println!("TrapFrame {{");
        println!("  r15: {:#018x}", self.r15);
        println!("  r14: {:#018x}", self.r14);
        println!("  r13: {:#018x}", self.r13);
        println!("  r12: {:#018x}", self.r12);
        println!("  r11: {:#018x}", self.r11);
        println!("  r10: {:#018x}", self.r10);
        println!("  r9 : {:#018x}", self.r9);
        println!("  r8 : {:#018x}", self.r8);
        println!("  rbp: {:#018x}", self.rbp);
        println!("  rdi: {:#018x}", self.rdi);
        println!("  rsi: {:#018x}", self.rsi);
        println!("  rdx: {:#018x}", self.rdx);
        println!("  rcx: {:#018x}", self.rcx);
        println!("  rbx: {:#018x}", self.rbx);
    }

    pub fn new() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            int_no: 0,
            error_code: 0,
            rip: 0,
            cs: 0,
            rflags: 0,
            rsp: 0,
            ss: 0,
        }
    }

    pub fn copy_from(&mut self, other: &TrapFrame) {
        self.r15 = other.r15;
        self.r14 = other.r14;
        self.r13 = other.r13;
        self.r12 = other.r12;
        self.r11 = other.r11;
        self.r10 = other.r10;
        self.r9  = other.r9;
        self.r8  = other.r8;
        self.rbp = other.rbp;
        self.rdi = other.rdi;
        self.rsi = other.rsi;
        self.rdx = other.rdx;
        self.rcx = other.rcx;
        self.rbx = other.rbx;
        self.rax = other.rax;
        self.int_no = other.int_no;
        self.error_code = other.error_code;
        self.rip = other.rip;
        self.cs  = other.cs;
        self.rflags = other.rflags;
        self.rsp = other.rsp;
        self.ss  = other.ss;
    }
}

pub fn init() {
    unsafe {
        IDT_PTR.base = core::ptr::addr_of!(IDT) as u64;
    }

    ISR_TABLE.iter().enumerate().for_each(|(i, &isr)| {
        let offset = isr as u64;
        let entry = IdtEntry {
            offset_low: offset as u16,
            selector: 0x08, // code segment selector
            ist: 0,
            type_attr: 0x8E, // interrupt gate, present
            offset_mid: (offset >> 16) as u16,
            offset_high: (offset >> 32) as u32,
            zero: 0,
        };
        unsafe {
            IDT.entries[i] = entry;
        }
    });

    unsafe {
        core::arch::asm!(
        "lidt [{}]",
        in(reg) &raw const IDT_PTR,
        options(nostack, preserves_flags)
        );
    }
}

#[unsafe(no_mangle)]
extern "C" fn isr_dispatch(tf: &mut TrapFrame) {
    unsafe {
        if !process::scheduler::SCHEDULER.current.is_null() {
            set_kernel_stack((*process::scheduler::SCHEDULER.current).process_stack_top as u64);
        }
    }
    if tf.int_no >= 0x20 && tf.int_no <= 0x2F {
        console_println!("Hardware interrupt: int_no={:#x}", tf.int_no);

        pic::eoi((tf.int_no - 0x20) as u8);
        return;
    }

    if tf.int_no == 0x30 {
        // APIC spurious interrupt, can be ignored for now
        time::apic::reset(); // reset if the interrupt is generated by syscall

        process::scheduler::schedule(tf);
        time::apic::eoi();
        return;
    }

    tf.display_console();
    tf.display_serial();

    // check if current process is not null and is a user process, if so, kill it and print a message
    unsafe {
        if !process::scheduler::SCHEDULER.current.is_null() {
            let mut current = process::scheduler::SCHEDULER.current;
            if (*current).state.eq(&process::ProcessState::Running) {
                println!("Killing process pid {} due to unhandled interrupt", (*current).pid);
                process::scheduler::kill_current();
            }
            current = (*current).next;

            panic!("user process killed due to unhandled interrupt: int_no={:#x}, error_code={:#x}, rip={:#x}, cs={:#x}, rflags={:#x}, rsp={:#x}, ss={:#x}",
                tf.int_no, tf.error_code, tf.rip, tf.cs, tf.rflags, tf.rsp, tf.ss
            );
            process::scheduler::schedule(tf);
            return;
        }
    }

    // Unhandled interrupt Panic for now
    if tf.int_no == 14 {
        panic!("Page fault at address {:#x}, error code {:#x}, rip={:#x}",
            unsafe {
                let cr2: u64;
                core::arch::asm!("mov {}, cr2", out(reg) cr2);
                cr2
            },
            tf.error_code,
            tf.rip
        );
    }
    panic!("Unhandled interrupt: int_no={:#x}, error_code={:#x}, rip={:#x}, cs={:#x}, rflags={:#x}, rsp={:#x}, ss={:#x}",
        tf.int_no, tf.error_code, tf.rip, tf.cs, tf.rflags, tf.rsp, tf.ss
    );
}