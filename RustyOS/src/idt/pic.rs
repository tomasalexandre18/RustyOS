use crate::io::{io_wait, outb};

static mut mask1: u8 = 0xFF;
static mut mask2: u8 = 0xFF;

enum PicCommand {
    Init = 0x10,
    Icw4 = 0x01,
    PicEoi = 0x20,
}

enum PicPort {
    Pic1Cmd = 0x20,
    Pic1Data = 0x21,
    Pic2Cmd = 0xA0,
    Pic2Data = 0xA1,
}

pub fn mask_all() {
    unsafe {
        mask1 = 0xFF;
        mask2 = 0xFF;
    }
    unsafe {
        outb(PicPort::Pic1Data as u16, mask1);
        outb(PicPort::Pic2Data as u16, mask2);
    }
}

pub fn remap(offset1: u8, offset2: u8) {
    unsafe {
        // save masks
        mask1 = crate::io::inb(PicPort::Pic1Data as u16);
        mask2 = crate::io::inb(PicPort::Pic2Data as u16);

        // start initialization
        outb(PicPort::Pic1Cmd as u16, PicCommand::Init as u8 | PicCommand::Icw4 as u8);
        io_wait();
        outb(PicPort::Pic2Cmd as u16, PicCommand::Init as u8 | PicCommand::Icw4 as u8);
        io_wait();

        // set vector offsets
        outb(PicPort::Pic1Data as u16, offset1);
        io_wait();
        outb(PicPort::Pic2Data as u16, offset2);
        io_wait();

        // configure cascading
        outb(PicPort::Pic1Data as u16, 0x04);
        io_wait();
        outb(PicPort::Pic2Data as u16, 0x02);
        io_wait();

        // set mode
        outb(PicPort::Pic1Data as u16, PicCommand::Icw4 as u8);
        io_wait();
        outb(PicPort::Pic2Data as u16, PicCommand::Icw4 as u8);
        io_wait();

        // restore masks
        outb(PicPort::Pic1Data as u16, mask1);
        outb(PicPort::Pic2Data as u16, mask2);
    }
}

pub fn set_mask(irq: u8) {
    unsafe {
        if irq < 8 {
            mask1 |= 1 << irq;
            outb(PicPort::Pic1Data as u16, mask1);
        } else {
            mask2 |= 1 << (irq - 8);
            outb(PicPort::Pic2Data as u16, mask2);
        }
    }
}

pub fn clear_mask(irq: u8) {
    unsafe {
        if irq < 8 {
            mask1 &= !(1 << irq);
            outb(PicPort::Pic1Data as u16, mask1);
        } else {
            mask2 &= !(1 << (irq - 8));
            outb(PicPort::Pic2Data as u16, mask2);
        }
    }
}

pub fn eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            outb(PicPort::Pic2Cmd as u16, PicCommand::PicEoi as u8);
        }
        outb(PicPort::Pic1Cmd as u16, PicCommand::PicEoi as u8);
    }
}