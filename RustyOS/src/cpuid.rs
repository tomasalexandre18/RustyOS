pub fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let mut eax: u32;
    let mut ebx: u32;
    let mut ecx: u32;
    let mut edx: u32;
    unsafe {
        core::arch::asm!(
        "push rbx",
        "cpuid",
        "mov {ebx_out:e}, ebx",
        "pop rbx",
        inlateout("eax") leaf => eax,
        ebx_out = lateout(reg) ebx,
        lateout("ecx") ecx,
        lateout("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}