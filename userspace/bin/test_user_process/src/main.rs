#![no_std]
#![no_main]
#![no_builtins]

extern crate libc;
extern crate alloc;

use alloc::format;
use alloc::string::ToString;
use libc::syscall::{get_framebuffer, FramebufferInfo};

#[unsafe(no_mangle)]
fn main() -> u64 {
    libc::syscall::debug_write("Hello from user process!\n".as_ptr(), "Hello from user process!\n".len() as u64);

    let mut fb = FramebufferInfo::default();
    let result = get_framebuffer(&mut fb);
    if result != 0 {
        let error_msg = "Failed to get framebuffer info: error code ".to_string() + &result.to_string() + "\n";
        libc::syscall::debug_write(error_msg.as_ptr(), error_msg.len() as u64);
        return 1;
    }
    
    let msg = format!("Framebuffer info - Address: {:p}, Width: {}, Height: {}, Pitch: {}, BPP: {}\n",
        fb.addr, fb.width, fb.height, fb.pitch, fb.bpp);
    libc::syscall::debug_write(msg.as_ptr(), msg.len() as u64);

    // write a square in the bottom right corner of the screen
    if !fb.addr.is_null() {
        let fb_slice = unsafe { core::slice::from_raw_parts_mut(fb.addr, (fb.height * fb.pitch) as usize) };
        let square_size = 50;
        let square_color = 0x00FF00;
        for y in (fb.height - square_size)..fb.height {
            for x in (fb.width - square_size)..fb.width {
                let offset = (y * fb.pitch + x * (fb.bpp as u32 / 8)) as usize;
                if offset + 3 < fb_slice.len() {
                    fb_slice[offset] = (square_color & 0xFF) as u8;
                    fb_slice[offset + 1] = ((square_color >> 8) & 0xFF) as u8;
                    fb_slice[offset + 2] = ((square_color >> 16) & 0xFF) as u8;
                    if fb.bpp == 32 {
                        fb_slice[offset + 3] = 0xFF; // alpha channel
                    }
                }
            }
        }
    }

    0
}
