#[repr(C)]
#[derive(Copy, Clone)]
struct GdtEntry {
    limit_low:  u16,
    base_low:   u16,
    base_mid:   u8,
    access:     u8,
    granularity: u8,
    base_high:  u8,
}

#[repr(C, packed)]
struct TssDescriptor {
    limit_low:  u16,
    base_low:   u16,
    base_mid:   u8,
    access:     u8,   // 0x89 = present, TSS available
    granularity: u8,
    base_high:  u8,
    base_upper: u32,
    reserved:   u32,
}

#[repr(C, packed)]
pub struct Tss {
    reserved0:  u32,
    pub(crate) rsp0:       u64,  // stack kernel ring 0
    rsp1:       u64,
    rsp2:       u64,
    reserved1:  u64,
    ist1:       u64,  // interrupt stack table (optionnel pour l'instant)
    ist2:       u64,
    ist3:       u64,
    ist4:       u64,
    ist5:       u64,
    ist6:       u64,
    ist7:       u64,
    reserved2:  u64,
    reserved3:  u16,
    iomap_base: u16,
}

#[repr(C)]
struct Gdt {
    entries: [GdtEntry; 6], // 6 entries: null, kernel code, kernel data, user code, user data
    tss_desc: TssDescriptor, // 16 bytes
}

static mut TSS: Tss = Tss {
    reserved0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    reserved1: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    reserved2: 0,
    reserved3: 0,
    iomap_base: 0,
};



static mut GDT: Gdt = Gdt {
    entries: [GdtEntry {
        limit_low: 0,
        base_low: 0,
        base_mid: 0,
        access: 0,
        granularity: 0,
        base_high: 0,
    }; 6],
    tss_desc: TssDescriptor {
        limit_low: 0,
        base_low: 0,
        base_mid: 0,
        access: 0,
        granularity: 0,
        base_high: 0,
        base_upper: 0,
        reserved: 0,
    },
};

pub fn set_kernel_stack(stack_top: u64) {
    unsafe {
        TSS.rsp0 = stack_top;
    }
}

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

static mut GDT_PTR: GdtPointer = GdtPointer {
    limit: (size_of::<Gdt>() - 1) as u16,
    base: 0,
};

pub fn init() {
    unsafe {
        GDT_PTR.base = &raw const GDT as u64;
    }

    unsafe {
        // Null descriptor
        GDT.entries[0] = GdtEntry {
            limit_low: 0,
            base_low: 0,
            base_mid: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        };

        // Kernel code segment
        GDT.entries[1] = GdtEntry {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access: 0x9A, // Present, Ring 0, Code Segment
            granularity: 0xAF, // 4K granularity, 64-bit code segment
            base_high: 0,
        };

        // Kernel data segment
        GDT.entries[2] = GdtEntry {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access: 0x92, // Present, Ring 0, Data Segment
            granularity: 0xCF, // 4K granularity
            base_high: 0,
        };

        // User code segment
        GDT.entries[5] = GdtEntry {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access: 0xFA, // Present, Ring 3, Code Segment
            granularity: 0xAF, // 4K granularity, 64-bit code segment
            base_high: 0,
        };

        // User data segment
        GDT.entries[3] = GdtEntry {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access: 0xF2, // Present, Ring 3, Data Segment
            granularity: 0xCF, // 4K granularity
            base_high: 0,
        };

        GDT.entries[4] = GdtEntry {
            limit_low: 0xFFFF,
            base_low: 0,
            base_mid: 0,
            access: 0xF2, // Present, Ring 3, Data Segment
            granularity: 0xCF, // 4K granularity
            base_high: 0,
        };
        // TSS entry
        GDT.tss_desc = TssDescriptor {
            limit_low: size_of::<Tss>() as u16 - 1,
            base_low: 0,
            base_mid: 0,
            access: 0x89, // Present, TSS available
            granularity: 0,
            base_high: 0,
            base_upper: 0,
            reserved: 0,
        };
        GDT.tss_desc.base_low = (&raw const TSS as u64 & 0xFFFF) as u16;
        GDT.tss_desc.base_mid = ((&raw const TSS as u64 >> 16) & 0xFF) as u8;
        GDT.tss_desc.base_high = ((&raw const TSS as u64 >> 24) & 0xFF) as u8;
        GDT.tss_desc.base_upper = (&raw const TSS as u64 >> 32) as u32;
        GDT.tss_desc.access = 0x89; // Present, TSS available
        GDT.tss_desc.granularity = 0; // byte granularity
        GDT.tss_desc.limit_low = (size_of::<Tss>() - 1) as u16;

    }

    unsafe {
        core::arch::asm!(
            "lgdt [{}]",
            in(reg) &raw const GDT_PTR,
            options(nostack, readonly)
        );

        core::arch::asm!(
            "push 0x08",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            out("rax") _
        );

        // Load TSS
        core::arch::asm!(
            "mov ax, 0x30", // TSS segment selector (5th entry, index 4, RPL 0)
            "ltr ax",
            options(nostack)
        );
    }
}