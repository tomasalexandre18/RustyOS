use core::sync::atomic::Ordering;
use crate::serial;
use crate::{println, HHDM_OFFSET};
use limine::request::MemmapRespData;
static mut BITMAP: *mut u8 = core::ptr::null_mut();
static mut BITMAP_SIZE: u64 = 0;

fn frame_to_bitmap_index(frame: u64) -> (usize, u8) {
    if frame / 8 >= unsafe { BITMAP_SIZE } {
        panic!("Frame number {} out of bitmap bounds", frame);
    }
    let byte_index = (frame / 8) as usize;
    let bit_index = (frame % 8) as u8;
    (byte_index, bit_index)
}

fn frame_to_physical_addr(frame: u64) -> u64 {
    frame * 0x1000
}

fn physical_addr_to_frame(addr: u64) -> u64 {
    addr / 0x1000
}
fn set_frame(frame: u64, used: bool) {
    unsafe {
        let (byte_index, bit_index) = frame_to_bitmap_index(frame);
        if used {
            *BITMAP.add(HHDM_OFFSET.load(Ordering::Relaxed) as usize + byte_index) |= 1 << bit_index;;
        } else {
            *BITMAP.add(HHDM_OFFSET.load(Ordering::Relaxed) as usize + byte_index) &= !(1 << bit_index);
        }
    }
}

fn get_frame(frame: u64) -> bool {
    unsafe {
        let (byte_index, bit_index) = frame_to_bitmap_index(frame);
        (*BITMAP.add(HHDM_OFFSET.load(Ordering::Relaxed) as usize + byte_index) & (1 << bit_index)) != 0
    }
}
fn set_frame_used(frame: u64) {
    set_frame(frame, true);
}

fn set_frame_free(frame: u64) {
    set_frame(frame, false);
}

pub fn alloc_frame() -> Option<u64> {
    for frame in 0..(unsafe { BITMAP_SIZE } * 8) {
        if !get_frame(frame) {
            set_frame_used(frame);
            return Some(frame_to_physical_addr(frame));
        }
    }
    None
}

#[allow(dead_code)]
pub fn free_frame(frame: u64) {
    set_frame_free(physical_addr_to_frame(frame));
}

pub fn init(memmap: &MemmapRespData) {
    let free_frames = memmap.entries().iter()
        .filter(|entry| entry.type_ == 0); // type 0 means available RAM

    let last_free_addr = free_frames.clone().map(|entry| entry.base + entry.length).max().unwrap_or(0);

    // calculate the space needed for the bitmap (0x0 to last_free_addr, divided by 0x1000 for frame size, divided by 8 for bits in a byte)
    let bitmap_size = (last_free_addr / 0x1000 + 7) / 8; // round up to nearest byte

    // find a free region for the bitmap (we can just use the first free region that is large enough)
    let bitmap_addr = free_frames.clone().find_map(|entry| {
        if entry.length >= bitmap_size {
            Some(entry.base)
        } else {
            None
        }}).expect("No free memory region large enough for bitmap");

    unsafe {
        BITMAP = bitmap_addr as *mut u8;
        BITMAP_SIZE = bitmap_size;
    }

    // set all frames as used in the bitmap
    for frame in 0..(last_free_addr / 0x1000) {
        set_frame_used(frame);
    }

    // mark available frames as free in the bitmap
    for entry in memmap.entries() {
        if entry.type_ == 0 {
            let start_frame = entry.base / 0x1000;
            let end_frame = (entry.base + entry.length) / 0x1000;
            for frame in start_frame..end_frame {
                set_frame_free(frame);
            }
        }
    }

    // mark the bitmap itself as used
    let bitmap_start_frame = bitmap_addr / 0x1000;
    let bitmap_end_frame = (bitmap_addr + bitmap_size + 0xFFF) / 0x1000;
    for frame in bitmap_start_frame..bitmap_end_frame {
        set_frame_used(frame);
    }
}

pub fn kmemset(ptr: *mut u8, value: u8, count: usize) {
    unsafe {
        for i in 0..count {
            ptr.add(i).write_volatile(value);
        }
    }
}