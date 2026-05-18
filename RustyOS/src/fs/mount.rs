use crate::fs::vfs::File;

pub struct Mount {
    pub mountpoint: &'static str,
    pub fs: &'static dyn Filesystem,
}

unsafe impl Sync for Mount {
    // This is safe because Mount only contains references to static data, and does not allow mutation.
}

pub trait Filesystem {
    fn open(&self, path: &str) -> Option<File>;
    fn read(&self, file: &mut File, buf: &mut [u8]) -> isize;
    fn write(&self, file: &mut File, buf: &[u8]) -> isize;
    fn close(&self, file: &mut File);
    fn seek(&self, file: &mut File, offset: usize) -> bool;

    fn unmount(&self) {
        // default implementation does nothing, can be overridden by filesystems that need cleanup
    }
}