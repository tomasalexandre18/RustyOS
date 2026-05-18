use core::fmt;
use crate::io::{inb, outb};

const COM1: u16 = 0x3F8;

pub fn init() {
    unsafe {
        // Disable all interrupts
        outb(COM1 + 1, 0x00);
        // Enable DLAB (set baud rate divisor)
        outb(COM1 + 3, 0x80);
        // Set divisor to 3 (lo byte) 38400 baud
        outb(COM1 + 0, 0x03);
        //                  (hi byte)
        outb(COM1 + 1, 0x00);
        // 8 bits, no parity, one stop bit
        outb(COM1 + 3, 0x03);
        // Enable FIFO, clear them, with 14-byte threshold
        outb(COM1 + 2, 0xC7);
        // IRQs enabled, RTS/DSR set
        outb(COM1 + 4, 0x0B);
    }
}

pub fn write_byte(byte: u8) {
    unsafe {
        // Wait for the serial port to be ready
        while (inb(COM1 + 5) & 0x20) == 0 {}
        outb(COM1, byte);
    }
}

pub fn write_str(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

pub struct Serial;

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(serial::Serial, $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! debug_balise {
    () => {{
        use core::fmt::Write;
        let _ = write!(serial::Serial, "[DEBUG] {}:{}\n", file!(), line!());
    }};
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(serial::Serial, "[DEBUG] {}:{} - ", file!(), line!());
        let _ = write!(serial::Serial, $($arg)*);
        let _ = write!(serial::Serial, "\n");
    }};
}