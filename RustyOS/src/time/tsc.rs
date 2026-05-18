use crate::console_print;
use crate::{console_println, serial};
use crate::cpuid::cpuid;
use crate::println;
use crate::time::pit;

static mut TSC_FREQUENCY: u64 = 0;

fn get_tsc_freq_from_cpuid() -> Option<u64> {
    let (eax, ebx, ecx, edx) = cpuid(0x15);
    if eax != 0 && ebx != 0 && ecx != 0 {
        let tsc_freq = (ebx as u64 * 1000000) / eax as u64;
        return Some(tsc_freq);
    }
    // Fallback to leaf 0x16 if 0x15 is not supported

    let (eax, ebx, ecx, edx) = cpuid(0x16);
    if eax != 0 && ebx != 0 && ecx != 0 {
        return Some(ecx as u64 * ebx as u64 / eax as u64);
    }

    let (_, _, ecx, _) = cpuid(0x1);
    if ecx & (1 << 31) != 0 {
        // On est dans une VM, vérifier leaf max
        let (eax, _, _, _) = cpuid(0x40000000);
        if eax >= 0x40000010 {
            let (eax, _, _, _) = cpuid(0x40000010);
            if eax != 0 {
                return Some(eax as u64 * 1000); // Hz
            }
        }
    }
    None
}

fn calibrate_tsc() {
    // cpuid is not used for now
    /*
    if let Some(freq) = get_tsc_freq_from_cpuid() {
        unsafe {
            TSC_FREQUENCY = freq;
        }
        return;
    }
    // Fallback: measure TSC frequency using PIT

     */
    console_println!("Calibrating TSC frequency using PIT...");
    let count1 = pit::read() as u64;
    let tsc1 = rdtsc();

    // attendre ~10ms = ~11932 ticks PIT
    loop {
        let count2 = pit::read() as u64;
        // gérer le wrap-around du compteur
        let delta = if count1 >= count2 { count1 - count2 } else { count1 + (0xFFFF - count2) };
        if delta >= 11932 {
            let tsc2 = rdtsc();
            let tsc_delta = tsc2 - tsc1;
            unsafe {
                TSC_FREQUENCY = tsc_delta * 1193180 / delta;
            }
            return;
        }
    }
}

pub fn init() {
    calibrate_tsc();
    console_println!("TSC frequency: {} Hz", unsafe { TSC_FREQUENCY });
}

fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") low, out("edx") high);
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn read_seconds() -> f64 {
    let tsc = rdtsc();
    unsafe { tsc as f64 / TSC_FREQUENCY as f64 }
}

pub fn read_millis() -> u64 {
    let tsc = rdtsc();
    unsafe { tsc * 1000 / TSC_FREQUENCY }
}

pub fn read_micros() -> u64 {
    let tsc = rdtsc();
    unsafe { tsc * 1_000_000 / TSC_FREQUENCY }
}