use core::{fmt, ptr};
use core::fmt::Arguments;
use limine::framebuffer::Framebuffer;
use crate::font;
use crate::font::PSF1Font;
use crate::limine_imp::FRAMEBUFFER_REQUEST;

fn get_framebuffer_info() -> &'static [&'static Framebuffer] {
    unsafe { FRAMEBUFFER_REQUEST.response().unwrap().framebuffers() }
}

pub fn get_framebuffer() -> Option<&'static Framebuffer> {
    let framebuffers = get_framebuffer_info();
    if framebuffers.is_empty() {
        return None;
    }
    // find best fit 1080p / 720p 32bpp RGBA framebuffer
    framebuffers.iter().find(|fb| fb.bpp == 32).copied()
        .or_else(|| framebuffers.first().copied())
}

pub fn clear_framebuffer(fb: &Framebuffer, color: u32) {
    let pixels = fb.address() as *mut u32;
    for i in 0..(fb.width * fb.height) as isize {
        unsafe { *pixels.offset(i) = color };
    }
}

pub struct Console {
    framebuffer: &'static Framebuffer,
    cursor_x: usize,
    cursor_y: usize,
    max_cursor_x: usize,
    font: PSF1Font,
    fg_color: u32,
    bg_color: u32,
}

impl Console {
    pub fn new(framebuffer: &'static Framebuffer, font: PSF1Font) -> Self {
        Self {
            framebuffer,
            cursor_x: 0,
            cursor_y: 0,
            max_cursor_x: framebuffer.width as usize / 8, // assuming 8px wide font
            font,
            fg_color: 0xFFFFFFFF, // white
            bg_color: 0x00000000, // transparent black
        }
    }

    pub fn write_char(&mut self, c: char) {
        if c == '\n' {
            self.cursor_x = 0;
            self.cursor_y += self.font.char_height(); // move cursor down by character height
            return;
        }

        // check if y exceeds framebuffer height, if so scroll up
        if self.cursor_y + self.font.char_height() > self.framebuffer.height as usize {
            let line_size = self.framebuffer.width as usize * self.font.char_height() * 4; // 4 bytes per pixel
            unsafe {
                let fb_ptr = self.framebuffer.address() as *mut u8;
                // move framebuffer content up by one line
                ptr::copy(fb_ptr.add(line_size), fb_ptr, (self.framebuffer.width as usize * (self.framebuffer.height as usize - self.font.char_height()) * 4));
                // clear the last line
                for i in (self.framebuffer.width as usize * (self.framebuffer.height as usize - self.font.char_height()) * 4)..(self.framebuffer.width as usize * self.framebuffer.height as usize * 4) {
                    ptr::write(fb_ptr.add(i), self.bg_color as u8);
                }
            }
            self.cursor_y -= self.font.char_height(); // move cursor up by one line
        }

        let glyph = self.font.get_glyph(c);
        for row in 0..self.font.char_height() {
            let bits = glyph[row];
            for col in 0..8 {
                let color = if (bits >> (7 - col)) & 1 == 1 { self.fg_color } else { self.bg_color };
                let x = self.cursor_x + col;
                let y = self.cursor_y + row;
                if x < self.framebuffer.width as usize && y < self.framebuffer.height as usize {
                    let pixel_offset = (y * self.framebuffer.width as usize + x) * 4; // 4 bytes per pixel
                    unsafe {
                        let pixel_ptr = (self.framebuffer.address() as *mut u32).add(pixel_offset / 4);
                        *pixel_ptr = color;
                    }
                }
            }
        }


        self.cursor_x += 8; // move cursor right by character width
        if self.cursor_x >= self.max_cursor_x * 8 {
            self.cursor_x = 0;
            self.cursor_y += self.font.char_height(); // move cursor down by character height
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

pub static mut CONSOLE: Option<Console> = None;

pub fn init_global_console() {
    if let Some(fb) = get_framebuffer() {
        clear_framebuffer(fb, 0x00000000); // clear to transparent black
        let font = font::load_font();
        unsafe {
            CONSOLE = Some(Console::new(fb, font));
        }
    }
}

// macro to write formatted text to the console
#[macro_export]
macro_rules! console_print {
    ($($arg:tt)*) => {
        unsafe {
            if let Some(ref mut c) = $crate::framebuffer::CONSOLE {
                let _ = core::fmt::Write::write_fmt(c, format_args!($($arg)*));
            }
        }
    };
}

#[macro_export]
macro_rules! console_println {
    () => {
        console_print!("\n");
    };
    ($($arg:tt)*) => {
        console_print!("{}\n", format_args!($($arg)*));
    };
}

pub fn is_console_initialized() -> bool {
    unsafe { ptr::addr_of!(CONSOLE).read().is_some() }
}