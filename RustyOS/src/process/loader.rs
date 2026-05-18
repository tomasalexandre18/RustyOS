use core::sync::atomic::Ordering;
use crate::serial;
use crate::{println, HHDM_OFFSET};
use crate::memory::base::{alloc_frame, kmemset};
use crate::memory::vmm::map_page;
use crate::process::{create_user_process, Process};

#[repr(C)]
pub struct ElfHeader {
    pub magic:      [u8; 4],
    pub class:      u8,
    pub endian:     u8,
    pub version:    u8,
    pub os_abi:     u8,
    pub abi_ver:    u8,
    pub padding:    [u8; 7],
    pub e_type:     u16,
    pub machine:    u16,
    pub e_version:  u32,
    pub e_entry:    u64,
    pub e_phoff:    u64,
    pub e_shoff:    u64,
    pub e_flags:    u32,
    pub e_ehsize:   u16,
    pub e_phentsize:u16,
    pub e_phnum:    u16,
    pub e_shentsize:u16,
    pub e_shnum:    u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
pub struct ProgramHeader {
    pub p_type:   u32,
    pub p_flags:  u32,
    pub p_offset: u64,
    pub p_vaddr:  u64,
    pub p_paddr:  u64,
    pub p_filesz: u64,
    pub p_memsz:  u64,
    pub p_align:  u64,
}

pub const PT_LOAD: u32 = 1;
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub fn create_process_from_elf<'a>(elf_data: &[u8], name: &str) -> Result<*mut Process, &'a str> {
    let elf_header = unsafe { &*(elf_data.as_ptr() as *const ElfHeader) };
    if elf_header.magic != ELF_MAGIC {
        return Err("Invalid ELF magic");
    }

    let phdrs = unsafe {
        core::slice::from_raw_parts(
            elf_data.as_ptr().add(elf_header.e_phoff as usize) as *const ProgramHeader,
            elf_header.e_phnum as usize,
        )
    };

    let process = create_user_process(elf_header.e_entry, name, 0x4000);
    for phdr in phdrs {
        if phdr.p_type == PT_LOAD {
            let intra_offset = phdr.p_vaddr & 0xFFF;   // offset dans la première page
            let page_vaddr_base = phdr.p_vaddr & !0xFFF; // vaddr page-alignée
            // ELF garantit: p_offset & 0xFFF == p_vaddr & 0xFFF
            // donc p_offset - intra_offset est le "début fichier" de la fenêtre de pages

            let total_size = intra_offset + phdr.p_memsz;
            let num_pages = (total_size + 0xFFF) / 0x1000;

            for i in 0..num_pages {
                let page = alloc_frame().expect("OOM");
                let virt = page_vaddr_base + i * 0x1000;  // toujours aligné
                map_page(unsafe {(*process).pml4t}, virt, page, 0x7);

                let hhdm_ptr = (page + HHDM_OFFSET.load(Ordering::Relaxed) as u64) as *mut u8;
                unsafe {
                    core::ptr::write_bytes(hhdm_ptr, 0, 0x1000);
                }

                // fenêtre de cette page dans l'espace "étendu" [0, num_pages*0x1000)
                let win_start = i * 0x1000;
                let win_end   = win_start + 0x1000;
                // la donnée fichier occupe [intra_offset, intra_offset + p_filesz)
                let data_end  = intra_offset + phdr.p_filesz;

                let copy_begin = intra_offset.max(win_start);
                let copy_end   = data_end.min(win_end);

                if copy_begin < copy_end {
                    let copy_size   = (copy_end - copy_begin) as usize;
                    let dest_off    = (copy_begin - win_start) as usize;
                    let src_off     = (copy_begin - intra_offset) as usize; // offset depuis p_offset
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            elf_data.as_ptr().add(phdr.p_offset as usize + src_off),
                            hhdm_ptr.add(dest_off),
                            copy_size,
                        );
                    }
                }
            }
        }
    }

    Ok(process)
}