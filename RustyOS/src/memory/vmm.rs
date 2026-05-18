use core::sync::atomic::Ordering;
use crate::serial;
use limine::request::{ExecutableAddressResponse, MemmapRespData};
use crate::{debug_balise, HHDM_OFFSET};
use crate::memory::base::{alloc_frame, kmemset};

static mut PML4T_KERNEL: *mut u64 = core::ptr::null_mut();
static mut PML4T_KERNEL_MAPPING_COPY: *mut u64 = core::ptr::null_mut();

unsafe extern "C" {
    pub static kernel_end: u64;
}

pub fn init_kernel_mapping(kernel: &ExecutableAddressResponse) {
    unsafe {
        PML4T_KERNEL_MAPPING_COPY = alloc_frame().expect("Failed to allocate frame for PML4T copy") as *mut u64;
        let copy_virt = (PML4T_KERNEL_MAPPING_COPY as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u8;
        kmemset(copy_virt, 0, 4096);
    };

    get_or_create_table(unsafe { PML4T_KERNEL_MAPPING_COPY }, 256, 0x3); // allocate PDPT for 0xFFFF800000000000 in advance for kernel allocation

    let start_kernel = kernel.virtual_base;
    let end_kernel;

    #[allow(unused_unsafe)]
    unsafe {
        end_kernel = &raw const kernel_end as u64;
    }

    let end_kernel_aligned = (end_kernel + 0xFFF) & !0xFFF;
    let offset = kernel.physical_base - kernel.virtual_base;
    for addr in (start_kernel..end_kernel_aligned).step_by(0x1000) {
        let phys = (addr as i64 + offset as i64) as u64;
        map_page(unsafe {PML4T_KERNEL_MAPPING_COPY}, addr , phys, 0x3); // Present + RW
    }
}

pub fn copy_kernel_mapping(pml4t: *mut u64) {
    unsafe {
        let copy_virt = (PML4T_KERNEL_MAPPING_COPY as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        let pml4_virt = (pml4t as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        for i in 0..512 {
            let entry = copy_virt.add(i).read();
            pml4_virt.add(i).write(entry);
        }
    }
}

pub fn init(kernel: &ExecutableAddressResponse, memmap: &MemmapRespData) {
    unsafe {
        PML4T_KERNEL = alloc_frame().expect("Failed to allocate frame for PML4T") as *mut u64;
    }

    init_kernel_mapping(kernel);
    copy_kernel_mapping(unsafe {PML4T_KERNEL});

    for entry in memmap.entries() {
        let start = entry.base & !0xFFF; // align down
        let end = (entry.base + entry.length + 0xFFF) & !0xFFF; // align up

        for phys in (start..end).step_by(0x1000) {
            let virt = phys + HHDM_OFFSET.load(Ordering::Relaxed) as u64;
            map_page(unsafe {PML4T_KERNEL}, virt, phys, 0x3); // Present + RW
        }
    }

    load_pml4t(unsafe {PML4T_KERNEL});
}

pub fn load_pml4t(pml4t: *mut u64) {
    unsafe {
        core::arch::asm!(
        "mov cr3, {}",
        in(reg) pml4t as u64,  // adresse PHYSIQUE de la PML4
        options(nostack)
        );
    }
}

fn get_or_create_table(table: *mut u64, index: usize, flags: u64) -> *mut u64 {
    unsafe {
        let table_virt = (table as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        let entry = table_virt.add(index);
        if (*entry) & 1 == 0 {
            let new_table = alloc_frame().expect("Failed to allocate frame");
            kmemset((new_table + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u8, 0, 4096);
            *entry = new_table | flags | 0x3;
        } else {
            // table existe déjà — OR les flags pour ne pas rétrograder
            *entry |= flags;
        }
        ((*entry) & 0x000FFFFFFFFFF000) as *mut u64
    }
}

pub fn map_page(pml4t: *mut u64, virt: u64, phys: u64, flags: u64) {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index   = (virt >> 21) & 0x1FF;
    let pt_index   = (virt >> 12) & 0x1FF;

    let pdpt = get_or_create_table(pml4t, pml4_index as usize, flags);
    let pd   = get_or_create_table(pdpt, pdpt_index as usize, flags);
    let pt   = get_or_create_table(pd, pd_index as usize, flags);

    unsafe {
        let pt_virt = (pt as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        pt_virt.add(pt_index as usize).write(phys | flags);
    }
}

pub fn get_new_pml4t() -> *mut u64 {
    let new_pml4t = alloc_frame().expect("Failed to allocate frame for new PML4T") as *mut u64;
    kmemset((new_pml4t as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u8, 0, 4096);
    copy_kernel_mapping(new_pml4t);
    new_pml4t
}

#[allow(dead_code)]
pub fn virt_to_phys(pml4: *mut u64, virt: u64) -> Option<u64> {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4_virt = (pml4 as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        if (*pml4_virt.add(pml4_index as usize)) & 1 == 0 {
            return None;
        }
        let pdpt = ((*pml4_virt.add(pml4_index as usize)) & 0x000FFFFFFFFFF000) as *mut u64;

        let pdpt_virt = (pdpt as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        if (*pdpt_virt.add(pdpt_index as usize)) & 1 == 0 {
            return None;
        }
        let pd = ((*pdpt_virt.add(pdpt_index as usize)) & 0x000FFFFFFFFFF000) as *mut u64;

        let pd_virt = (pd as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        if (*pd_virt.add(pd_index as usize)) & 1 == 0 {
            return None;
        }
        let pt = ((*pd_virt.add(pd_index as usize)) & 0x000FFFFFFFFFF000) as *mut u64;

        let pt_virt = (pt as u64 + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u64;
        if (*pt_virt.add(pt_index as usize)) & 1 == 0 {
            return None;
        }
        Some(((*pt_virt.add(pt_index as usize)) & 0x000FFFFFFFFFF000) + (virt & 0xFFF))
    }
}

pub fn get_kernel_pml4t() -> *mut u64 {
    unsafe { PML4T_KERNEL }
}