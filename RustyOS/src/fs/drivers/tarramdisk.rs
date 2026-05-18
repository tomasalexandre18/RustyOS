use alloc::boxed::Box;
use alloc::vec::Vec;

pub struct Ramdisk {
    pub data: &'static [u8],
}

pub struct TarEntry {
    pub name: &'static str,
    pub offset: usize,  // offset dans le ramdisk vers les données
    pub size: usize,
}

pub struct TarFs {
    pub ramdisk: &'static Ramdisk,
    pub entries: &'static [TarEntry],  // table construite au mount
}