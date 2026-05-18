use core::arch::asm;
use crate::{println, serial, time};
use crate::debug_balise;
use crate::gdt::{set_kernel_stack};
use crate::idt::TrapFrame;
use crate::memory::vmm::kernel_end;
use crate::process::{syscall, Process, PROCESS_LIST_HEAD};
use crate::time::apic::start_timer;

pub struct Scheduler {
    pub head: *mut Process,
    pub current: *mut Process,
    pub next_pid: u64,
}

pub static mut SCHEDULER: Scheduler = Scheduler {
    head: core::ptr::null_mut(),
    current: core::ptr::null_mut(),
    next_pid: 1,
};

pub fn next_pid() -> u64 {
    unsafe {
        let pid = SCHEDULER.next_pid;
        SCHEDULER.next_pid += 1;
        pid
    }
}

pub fn init() {
    unsafe {
        SCHEDULER.head = PROCESS_LIST_HEAD;
        SCHEDULER.current = PROCESS_LIST_HEAD;
    }
}

pub fn start() -> ! {
    start_timer(1);
    loop {
        core::hint::spin_loop(); // the timer will lunch the first Scheduler interrupt
    }
}

pub fn schedule(tf: &mut TrapFrame) {
    unsafe {
        if SCHEDULER.current.is_null() {
            return;
        }
        // save current process state
        if (*SCHEDULER.current).state.eq(&crate::process::ProcessState::Running) {
            (*SCHEDULER.current).trap_frame.copy_from(tf);
            (*SCHEDULER.current).state = crate::process::ProcessState::Ready;
        }
        // find next ready process
        let mut next = (*SCHEDULER.current).next;
        while !next.is_null() && (*next).state.eq(&crate::process::ProcessState::Ready) == false {
            next = (*next).next;
        }
        if next.is_null() {
            next = SCHEDULER.head; // wrap around to the head of the list
            while !next.is_null() && (*next).state.eq(&crate::process::ProcessState::Ready) == false {
                next = (*next).next;
            }
        }
        if next.is_null() {
            return; // no ready process found
        }

        SCHEDULER.current = next;
        set_kernel_stack((*SCHEDULER.current).process_stack_top as u64);

        // context switch to the next process by loading its trap frame into the provided tf
        *tf = core::ptr::read_volatile(&(*SCHEDULER.current).trap_frame);
        // set cr3 to the process's page table
        crate::memory::vmm::load_pml4t((*SCHEDULER.current).pml4t);

        (*SCHEDULER.current).state = crate::process::ProcessState::Running;

        syscall::set_kernel_stack((*SCHEDULER.current).kernel_stack_page as u64 + 0x1000); // set kernel stack to the top of the kernel stack page

        // relunch the timer for the next scheduling event
        start_timer(30);
    }
}

pub(crate) fn kill_current() {
    unsafe {
        if SCHEDULER.current.is_null() {
            return;
        }
        kill_process_pid((*SCHEDULER.current).pid);
    }
}

fn kill_process_pid(pid: u64) {
    unsafe {
        let mut current = SCHEDULER.head;
        while !current.is_null() {
            if (*current).pid == pid {
                (*current).state = crate::process::ProcessState::Terminated;
                println!("Process {} killed", pid);
                return;
            }
            current = (*current).next;
        }
    }
}

pub fn scheduler_next() {
    unsafe {
        asm!("int 0x30", options(nostack, preserves_flags));
    }
}

pub fn sleep_current(ms: u64) {
    unsafe {
        if SCHEDULER.current.is_null() {
            return;
        }
        (*SCHEDULER.current).state = crate::process::ProcessState::Blocked;
    }
    scheduler_next();
}

pub fn current_pid() -> u64 {
    unsafe {
        if SCHEDULER.current.is_null() {
            return 0;
        }
        (*SCHEDULER.current).pid
    }
}

pub fn current_process() -> *mut Process {
    unsafe {
        SCHEDULER.current
    }
}