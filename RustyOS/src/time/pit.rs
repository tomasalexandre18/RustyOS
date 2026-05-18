use crate::io::outb;

pub fn init() {
    // set PIT in pooling mode (No interrupts)
    unsafe {
        // 1193180 is the PIT input clock frequency
        // set mode 2 (rate generator) for channel 0
        outb(0x43, 0x34); // Channel 0, Access mode: lobyte/hibyte, Mode 2
        outb(0x40, 0xFF); // Divisor low byte (max value for 18.2 Hz)
        outb(0x40, 0xFF); // Divisor high byte (max value for 18.2 Hz)
    }
}

pub fn read() -> u16 {
    // read the current value of the PIT counter
    let low: u8;
    let high: u8;
    unsafe {
        outb(0x43, 0x00); // Latch command for channel 0
        low = crate::io::inb(0x40); // Read low byte
        high = crate::io::inb(0x40); // Read high byte
    }
    ((high as u16) << 8) | (low as u16)
}