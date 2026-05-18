fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{}/kernel.ld", manifest);
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rustc-link-arg=-static");
    // println!("cargo:rustc-link-arg=--wrap=rust_begin_unwind");

    let mut asm = String::new();
    for i in 0..256usize {
        let has_err = matches!(i, 8 | 10 | 11 | 12 | 13 | 14 | 17);
        if !has_err {
            asm.push_str(&format!(".global isr{0}\nisr{0}:\npush 0\npush {0}\njmp isr_common\n", i));
        } else {
            asm.push_str(&format!(".global isr{0}\nisr{0}:\npush {0}\njmp isr_common\n", i));
        }
    }
    std::fs::write("src/idt/idt_stubs.s", asm).unwrap();

    let mut rs = String::new();

    rs.push_str("unsafe extern \"C\" {\n");
    for i in 0..256usize {
        rs.push_str("    #[allow(dead_code)]\n");
        rs.push_str(&format!("    fn isr{}();\n", i));
    }
    rs.push_str("}\n\n");

    rs.push_str("#[allow(dead_code)]\npub static ISR_TABLE: [unsafe extern \"C\" fn(); 256] = [\n");
    for i in 0..256usize {
        rs.push_str(&format!("    isr{},\n", i));
    }
    rs.push_str("];\n");

    // add common handler
    rs.push_str("\n\n");
    rs.push_str("unsafe extern \"C\" {\n");
    rs.push_str("    #[allow(dead_code)]\n");
    rs.push_str("    fn isr_common();\n");
    rs.push_str("}\n");

    std::fs::write("src/idt/idt_isrs.rs", rs).unwrap();
}