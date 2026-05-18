use core::{
    mem::size_of,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
use crate::memory::heap_kernel::{kfree, kmalloc};

pub struct KBox<T> {
    ptr: NonNull<T>,
}

impl<T> KBox<T> {
    pub fn new(val: T) -> Option<Self> {
        let raw = kmalloc(size_of::<T>()) as *mut T;
        let ptr = NonNull::new(raw)?; // retourne None si null
        unsafe { ptr.as_ptr().write(val); }
        Some(KBox { ptr })
    }
}

impl<T> Deref for KBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for KBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> Drop for KBox<T> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.ptr.as_ptr()); // appelle le destructeur de T
            kfree(self.ptr.as_ptr() as *mut u8);
        }
    }
}