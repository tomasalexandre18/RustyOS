use std::collections::HashMap;
use std::fs;
use std::process::Command;



fn build(_args: Vec<String>) {
    // build RustyOS cargo project
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--package")
        .arg("RustyOS")
        .arg("--target")
        .arg("x86_64-unknown-none")
        .current_dir("RustyOS")
        .status()
        .expect("Failed to execute cargo build");
    if !status.success() {
        eprintln!("cargo build failed");
        std::process::exit(1);
    }
}

fn build_userspace(_args: Vec<String>) {
    // build userspace/bin/test_user_process cargo project
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--package")
        .arg("test_user_process")
        .current_dir("userspace/bin/test_user_process")
        .status()
        .expect("Failed to execute cargo build for userspace project");
    if !status.success() {
        eprintln!("cargo build failed for userspace project test_user_process");
        std::process::exit(1);
    }
}

fn make_ramdisk(_args: Vec<String>) {
    // copy target/x86_64-unknown-none/release/test_user_process to iso_root/boot/ramdisk.elf
    fs::create_dir_all("iso_root/boot").unwrap();
    fs::copy("target/x86_64-unknown-none/release/test_user_process",
             "iso_root/boot/ramdisk.elf").unwrap();
}

fn iso(args: Vec<String>) {
    let xorriso = std::env::var("XORRISO").unwrap_or("C:/msys64/usr/bin/xorriso.exe".to_string());

    build(args.to_owned());
    build_userspace(args.to_owned());

    fs::create_dir_all("iso_root/boot").unwrap();
    fs::create_dir_all("iso_root/EFI/BOOT").unwrap();

    fs::copy("target/x86_64-unknown-none/release/RustyOS",
             "iso_root/boot/kernel").unwrap();
    fs::copy("limine/limine-bios.sys",
             "iso_root/boot/limine-bios.sys").unwrap();
    fs::copy("limine/limine-bios-cd.bin",
             "iso_root/boot/limine-bios-cd.bin").unwrap();
    fs::copy("limine/BOOTX64.EFI",
             "iso_root/EFI/BOOT/BOOTX64.EFI").unwrap();
    fs::copy("limine.conf",
             "iso_root/boot/limine.conf").unwrap();

    make_ramdisk(args.to_owned());

    let status = Command::new(xorriso)
        .arg("-as")
        .arg("mkisofs")
        .arg("-o")
        .arg("rustyos.iso")
        .arg("-b")
        .arg("boot/limine-bios-cd.bin")
        .arg("-c")
        .arg("boot/boot.cat")
        .arg("-no-emul-boot")
        .arg("-boot-load-size")
        .arg("4")
        .arg("-boot-info-table")
        .arg("iso_root")
        .status()
        .expect("Failed to execute xorriso");
    if !status.success() {
        eprintln!("xorriso failed");
        std::process::exit(1);
    }

    // limine installer
    let status = Command::new("limine/limine.exe")
        .arg("bios-install")
        .arg("rustyos.iso")
        .status()
        .expect("Failed to execute limine installer");
    if !status.success() {
        eprintln!("limine installer failed");
        std::process::exit(1);
    }
    fs::remove_dir_all("iso_root").unwrap();
}



fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 1 {
        eprintln!("must specify xtask script");
        std::process::exit(1);
    }

    let task_name = &args[0];

    let task: HashMap<&str, fn(Vec<String>)> = HashMap::from([
        ("build", build as fn(Vec<String>)),
        ("iso", iso as fn(Vec<String>))
    ]);

    if !task.contains_key(task_name.as_str()) {
        eprintln!("Invalid task name: {}", task_name);
        eprintln!("Available tasks:");
        for key in task.keys() {
            eprintln!("  - {}", key);
        }
        std::process::exit(1);
    }

    let function = task.get(task_name.as_str()).unwrap();

    let fn_args = args[1..].to_vec();
    function(fn_args);
}