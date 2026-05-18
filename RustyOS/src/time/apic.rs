use crate::console_print;
use core::sync::atomic::Ordering;
use crate::{console_println, memory, serial, HHDM_OFFSET};
use crate::{msr, println};
use crate::time::tsc;

enum LapicCommand {
    Eoi = 0xB0,
    SpuriousInterruptVector = 0xF0,
    LvtTimer = 0x320,
    InitialCount = 0x380,
    CurrentCount = 0x390,
    DivideConfig = 0x3E0,
}

static mut TICKS_RATE_MS: u32 = 0;
fn write(command: LapicCommand, value: u32) {
    let base = get_base();
    unsafe {
        core::ptr::write_volatile((base + command as u64) as *mut u32, value);
    }
}

fn read(command: LapicCommand) -> u32 {
    let base = get_base();
    unsafe { core::ptr::read_volatile((base + command as u64) as *const u32) }
}

fn enable() {
    let mut apic_base = msr::read(msr::IA32_APIC_BASE);
    apic_base |= 1 << 11; // set the APIC Global Enable bit
    let apic_physical_base = apic_base & 0xFFFFF000;
    let apic_virtual_base = apic_physical_base + HHDM_OFFSET.load(Ordering::Relaxed) as u64;
    memory::vmm::map_page(memory::vmm::get_kernel_pml4t(), apic_virtual_base, apic_physical_base, 0x3); // Present + RW
    msr::write(msr::IA32_APIC_BASE, apic_base);
}

fn get_base() -> u64 {
    (msr::read(msr::IA32_APIC_BASE) & 0xFFFFF000) + HHDM_OFFSET.load(Ordering::Relaxed) as u64
}

fn calibrate(ms: u32, divider: u32) {
    write(LapicCommand::DivideConfig, divider);
    write(LapicCommand::InitialCount, 0xFFFFFFFF);
    let start = tsc::read_micros();
    while tsc::read_micros() - start < ms as u64 * 1000 {
        core::hint::spin_loop();
    }
    let elapsed_ticks = 0xFFFFFFFF - read(LapicCommand::CurrentCount);
    let ticks_per_ms = elapsed_ticks / ms;
    unsafe {
        TICKS_RATE_MS = ticks_per_ms;
    }
}

pub fn init() {
    enable();
    calibrate(10, 0x3); // 10 ms, divide by 16
    console_println!("APIC timer calibrated: {} ticks/ms", unsafe { TICKS_RATE_MS });

    // Enable APIC
    write(LapicCommand::SpuriousInterruptVector, read(LapicCommand::SpuriousInterruptVector) | 0x100); // set bit 8 to enable APIC

    write(LapicCommand::SpuriousInterruptVector, (read(LapicCommand::SpuriousInterruptVector) & 0xFFFFFF00) | 0xFF); // vector 0xFF, masked = 0

    let mut lvt = read(LapicCommand::LvtTimer);
    // set vector 0x30, unmask, oneshot mode
    lvt = (lvt & 0xFFFFFF00) | 0x30; // vector 0x30
    lvt &= !(1 << 16); // unmask
    lvt &= !(1 << 17); // oneshot mode
    write(LapicCommand::InitialCount, 0); // stop timer for now
    write(LapicCommand::LvtTimer, lvt);
    write(LapicCommand::DivideConfig, 0x3); // divide by 16
}

pub fn start_timer(ms: u32) {
    let ticks = unsafe { TICKS_RATE_MS * ms };
    write(LapicCommand::InitialCount, ticks);
}

pub fn eoi() {
    write(LapicCommand::Eoi, 0);
}

pub fn reset() {
    // set count to 0 to stop the timer (in case it's still running)
    write(LapicCommand::InitialCount, 0);
}