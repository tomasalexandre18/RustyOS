pub mod scheduler;
pub mod syscall;

pub mod loader;

use core::sync::atomic::Ordering;
use crate::{memory, serial, HHDM_OFFSET};
use crate::idt::TrapFrame;
use crate::memory::heap_kernel::kmalloc;


pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

impl ProcessState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessState::Ready => "Ready",
            ProcessState::Running => "Running",
            ProcessState::Blocked => "Blocked",
            ProcessState::Terminated => "Terminated",
        }
    }

    // equality check for ProcessState
    pub fn eq(&self, other: &ProcessState) -> bool {
        matches!((self, other),
            (ProcessState::Ready, ProcessState::Ready) |
            (ProcessState::Running, ProcessState::Running) |
            (ProcessState::Blocked, ProcessState::Blocked) |
            (ProcessState::Terminated, ProcessState::Terminated))
    }
}

pub struct Process {
    pub pid: u64,
    pub name: [u8; 32],
    pub state: ProcessState,
    pub trap_frame: TrapFrame,  // sauvegardé au context switch
    pub process_stack_base: *mut u8,
    pub process_stack_top: *mut u8,
    pub next: *mut Process,     // liste chaînée
    pub pml4t: *mut u64,        // pour les processus utilisateurs (kernel processes will use the kernel PML4T)
    pub kernel_stack_page: *mut u8,      // pour les processus utilisateurs, page de pile en kernel space pour les IRQ depuis Ring 3
    pub heap_start: u64,          // adresse virtuelle de début du heap (pour les processus utilisateurs)
    pub heap_end: u64,            // adresse virtuelle de fin du heap (pour les processus utilisateurs)
    pub fd_table: FdTable,        // table de descripteurs de fichiers (pour les processus utilisateurs)
}

pub struct FdTable {
    pub files: [Option<u64>; 64],  // taille fixe pour l'instant
}

impl FdTable {
    pub fn insert(&mut self, file: u64) -> Option<usize> {
        for (i, slot) in self.files.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                return Some(i);
            }
        }
        None // table full
    }
    pub fn get(&mut self, fd: usize) -> Option<&mut u64> {
        if fd < self.files.len() {
            self.files[fd].as_mut()
        } else {
            None
        }
    }
    pub fn close(&mut self, fd: usize) -> bool {
        if fd < self.files.len() && self.files[fd].is_some() {
            self.files[fd] = None;
            true
        } else {
            false
        }
    }
}


pub static mut PROCESS_LIST_HEAD: *mut Process = core::ptr::null_mut();
pub fn create_process(entry: u64, name: &str, stack_size: usize) -> *mut Process {
    let proc = kmalloc(size_of::<Process>()) as *mut Process;
    if proc.is_null() {
        panic!("create_process: kmalloc failed for Process struct");
    }
    unsafe {
        (*proc).pid = scheduler::next_pid();
        let mut name_bytes = [0u8; 32];
        let bytes = name.as_bytes();
        let len = if bytes.len() > 32 { 32 } else { bytes.len() };
        for i in 0..len {
            name_bytes[i] = bytes[i];
        }
        (*proc).name = name_bytes;
        (*proc).state = ProcessState::Ready;
        (*proc).trap_frame = TrapFrame::new(); // will be set when the process is scheduled
        (*proc).process_stack_base = kmalloc(stack_size) as *mut u8;
        if (*proc).process_stack_base.is_null() {
            panic!("create_process: kmalloc failed for kernel stack");
        }
        (*proc).process_stack_top = (((*proc).process_stack_base as usize + stack_size) & !0xF) as *mut u8;
        (*proc).next = core::ptr::null_mut();
        // set trap frame
        (*proc).trap_frame.rip = entry as u64;
        (*proc).trap_frame.rsp = (*proc).process_stack_top as u64;
        (*proc).trap_frame.rflags = 0x202; // IF=1, bit 9
        (*proc).trap_frame.cs = 0x08; // kernel code segment selector
        (*proc).trap_frame.ss = 0x10; // kernel data segment selector

        (*proc).pml4t = memory::vmm::get_kernel_pml4t();
        (*proc).kernel_stack_page = core::ptr::null_mut(); // not used for kernel processes
        
        // save the proc
        if PROCESS_LIST_HEAD.is_null() {
            PROCESS_LIST_HEAD = proc;
        } else {
            let mut current = PROCESS_LIST_HEAD;
            while !(*current).next.is_null() {
                current = (*current).next;
            }
            (*current).next = proc;
        }
    }
    proc
}

pub fn create_user_process(entry: u64, name: &str, stack_size: usize) -> *mut Process {
    let proc = kmalloc(size_of::<Process>()) as *mut Process;
    if proc.is_null() {
        panic!("create_user_process: kmalloc failed for Process struct");
    };
    unsafe {
        (*proc).pid = scheduler::next_pid();
        let mut name_bytes: [u8; 32] = [0; 32];
        let bytes = name.as_bytes();
        let len = if bytes.len() > 32 { 32 } else { bytes.len() };
        for i in 0..len {
            name_bytes[i] = bytes[i];
        }
        (*proc).name = name_bytes;
        (*proc).state = ProcessState::Ready;
        (*proc).trap_frame = TrapFrame::new();

        (*proc).process_stack_base = kmalloc(stack_size) as *mut u8;
        if (*proc).process_stack_base.is_null() {
            panic!("create_user_process: kmalloc failed for kernel stack");
        }
        (*proc).process_stack_top = (((*proc).process_stack_base as usize + stack_size) & !0xF) as *mut u8;
        (*proc).pml4t = memory::vmm::get_new_pml4t(); // new PML4T for user process
        let kernel_stack_virt = kmalloc(stack_size) as u64;
        let user_stack_virt: u64 = 0x00007fff00000000;

        let stack_pages = (stack_size + 0xFFF) / 0x1000;
        for i in 0..stack_pages {
            let phys = (kernel_stack_virt + (i * 0x1000) as u64) - HHDM_OFFSET.load(Ordering::Relaxed) as u64;
            let virt = user_stack_virt + (i * 0x1000) as u64;
            memory::vmm::map_page((*proc).pml4t, virt, phys, 0x7); // Present + RW + User
        }

        let user_stack_top = user_stack_virt + stack_size as u64;
        (*proc).trap_frame.rsp = user_stack_top & !0xF;

        (*proc).next = core::ptr::null_mut();

        (*proc).heap_start = 0x0000_0010_0000_0000; // start of user heap
        (*proc).heap_end = (*proc).heap_start; // initially empty heap
        // TrapFrame Ring 3
        (*proc).trap_frame.rip = entry as u64;
        (*proc).trap_frame.rflags = 0x202;
        (*proc).trap_frame.cs = 0x2B; // 0x28 | 3  ← User CS64
        (*proc).trap_frame.ss = 0x23; // 0x20 | 3  ← User SS

        // stack kernel for handling IRQs from Ring 3
        (*proc).kernel_stack_page = kmalloc(0x1000) as *mut u8;
        if (*proc).kernel_stack_page.is_null() {
            panic!("create_user_process: kmalloc failed for kernel stack");
        }

        if PROCESS_LIST_HEAD.is_null() {
            PROCESS_LIST_HEAD = proc;
        } else {
            let mut current = PROCESS_LIST_HEAD;
            while !(*current).next.is_null() {
                current = (*current).next;
            }
            (*current).next = proc;
        }
        
    }
    proc
}
