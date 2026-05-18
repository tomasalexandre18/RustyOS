use crate::fs::mount::Mount;

pub struct Inode {
    pub size: usize,
    pub fs_data: *const (),  // pointeur opaque vers TarEntry ou autre
}

pub struct FileOps {
    pub read: fn(file: &mut File, buf: &mut [u8]) -> isize,
    pub write: fn(file: &mut File, buf: &[u8]) -> isize,
    pub close: fn(file: &mut File),
    pub seek: fn(file: &mut File, offset: usize) -> bool,
}

pub struct File {
    pub inode: &'static Inode,
    pub ops: &'static FileOps,
    pub offset: usize,
}

static mut MOUNT_TABLE: [Option<&'static Mount>; 4] = [None; 4];

pub fn mount(mount: &'static Mount) -> bool {
    unsafe {
        for slot in MOUNT_TABLE.iter_mut() {
            if slot.is_none() {
                *slot = Some(mount);
                return true;
            }
        }
        false // table full
    }
}

pub fn unmount(mountpoint: &str) -> bool {
    unsafe {
        for slot in MOUNT_TABLE.iter_mut() {
            if let Some(m) = slot {
                if m.mountpoint == mountpoint {
                    slot.unwrap().fs.unmount(); // call unmount on the filesystem for cleanup
                    
                    *slot = None;
                    return true;
                }
            }
        }
        false // not found
    }
}

pub fn get_mount_for_path(path: &str) -> Option<&'static Mount> {
    unsafe {
        for mount in MOUNT_TABLE.iter() {
            if let Some(m) = mount {
                if path.starts_with(m.mountpoint) {
                    return Some(*m);
                }
            }
        }
        None
    }
}