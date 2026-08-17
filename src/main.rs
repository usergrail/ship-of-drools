// Must run as administrator (or hold SeDebugPrivilege).

#[allow(dead_code)]
mod diagnostic;
mod error;
mod memory;
mod pointer_chain;
mod process;
mod scanner;

use std::io::{self, Write};

const LB:    &str = "\x1b[94m";
const PU:    &str = "\x1b[95m";
const DIM:   &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

fn print_banner(handle: windows::Win32::Foundation::HANDLE, module_base: u64, container: u64) {
    let squids   = read_target(handle, container, 0xA8, 0x80, 0x3F8);
    let dollars  = read_target(handle, container, 0xA8, 0x80, 0x6C8);
    let harpoons = read_static4(handle, module_base, 0x0325_59E8, 0x190, 0x1E0, 0xE8);

    let sq_str = squids  .map_or("?".into(), |v| v.to_string());
    let sd_str = dollars .map_or("?".into(), |v| v.to_string());
    let hp_str = harpoons.map_or("?".into(), |v| v.to_string());

    print!("\x1b[2J\x1b[H");
    println!(" ⌜                                                      ⌝");
    println!(" {LB}   ██████╗ ██████╗  ██████╗  ██████╗ ██╗     ███████╗{RESET}");
    println!(" {LB}   ██╔══██╗██╔══██╗██╔═══██╗██╔═══██╗██║     ██╔════╝{RESET}");
    println!(" {LB}   ██║  ██║██████╔╝██║   ██║██║   ██║██║     ███████╗{RESET}");
    println!(" {LB}   ██║  ██║██╔══██╗██║   ██║██║   ██║██║     ╚════██║{RESET}");
    println!(" {LB}   ██████╔╝██║  ██║╚██████╔╝╚██████╔╝███████╗███████║{RESET}");
    println!(" {LB}   ╚═════╝ ╚═╝  ╚═╝ ╚═════╝  ╚═════╝ ╚══════╝╚══════╝{RESET}");
    println!(" ⌞                                                      ⌟");
    println!(" {DIM}            - A masterpiece made by grail. -{RESET}");
    println!();
    println!(" ⌜                                                      ⌝");
    println!("   {DIM}Current Values:{RESET}");
    println!();
    println!("   Squids:         {PU}{sq_str}{RESET}");
    println!("   Sand Dollars:   {PU}{sd_str}{RESET}");
    println!("   Harpoons:       {PU}{hp_str}{RESET}");
    println!();
    println!("   {DIM}Commands:{RESET}");
    println!();
    println!("   {LB}print{RESET}      | {DIM}Print current values{RESET}");
    println!("   {LB}set{RESET}        | {DIM}Set values{RESET}");
    println!("   {LB}exit{RESET}       | {DIM}Quit{RESET}");
    println!(" ⌞                                                      ⌟");
    println!();
}

fn prompt(msg: &str) -> String {
    print!("{msg}");
    io::stdout().flush().unwrap();
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
    s.trim().to_string()
}

fn main() {
    print!("\x1b]0;ShipOfFools.exe \u{2502} Attached\x07");
    let args: Vec<String> = std::env::args().collect();

    let pid = match process::find_pid("ShipOfFools.exe") {
        Ok(p) => p,
        Err(e) => { eprintln!("[-] {e}"); std::process::exit(1); }
    };
    let proc = match process::open_process(pid) {
        Ok(p) => p,
        Err(e) => { eprintln!("[-] {e}"); std::process::exit(1); }
    };
    let module_base = match process::get_module_base(proc.pid, "GameAssembly.dll") {
        Ok(b) => b,
        Err(e) => { eprintln!("[-] {e}"); std::process::exit(1); }
    };

    if args.len() >= 3 && args[1] == "scan" {
        let target: u32 = match args[2].parse() {
            Ok(v) => v,
            Err(_) => { eprintln!("[-] scan requires an integer argument"); std::process::exit(1); }
        };

        let container = match pointer_chain::resolve_prefix(proc.handle, module_base, true) {
            Ok(c) => c,
            Err(e) => { eprintln!("[-] prefix: {e}"); std::process::exit(1); }
        };
        println!("[*] Pass 1 — from squid-token container 0x{container:016X}");
        let n1 = scanner::scan3(proc.handle, container, target, "container");
        println!("[*] Pass 1 found {n1} match(es)\n");

        let statics: &[u64] = &[
            0x0325_5A68,
            0x0327_80F0,
            0x0327_0D08,
            0x0327_3440,
            0x0325_59E8,
            0x0327_8068,
        ];
        let mut n2 = 0usize;
        for &soff in statics {
            println!("[*] Pass 2 — dll+0x{soff:08X}");
            n2 += scanner::scan_from_static(proc.handle, module_base, soff, target);
        }
        println!("[*] Pass 2 found {n2} match(es)");
        return;
    }

    let container = match pointer_chain::resolve_prefix(proc.handle, module_base, false) {
        Ok(c) => c,
        Err(e) => { eprintln!("[-] {e}"); std::process::exit(1); }
    };

    loop {
        print_banner(proc.handle, module_base, container);
        let input = prompt(" > ");
        match input.as_str() {
            "exit" => break,
            "print" => {}
            "set" => {
                let sq = prompt(&format!("  Set Squids to {DIM}(blank to skip){RESET}: "));
                if let Ok(v) = sq.parse::<u32>() {
                    write_target(proc.handle, container, 0xA8, 0x80, 0x3F8, v);
                }
                let sd = prompt(&format!("  Set Sand Dollars to {DIM}(blank to skip){RESET}: "));
                if let Ok(v) = sd.parse::<u32>() {
                    write_target(proc.handle, container, 0xA8, 0x80, 0x6C8, v);
                }
                let hp = prompt(&format!("  Set Harpoons to {DIM}(blank to skip){RESET}: "));
                if let Ok(v) = hp.parse::<u32>() {
                    write_static4(proc.handle, module_base, 0x0325_59E8, 0x190, 0x1E0, 0xE8, v);
                }
            }
            _ => {}
        }
    }
}

fn read_static4(handle: windows::Win32::Foundation::HANDLE, module_base: u64, s: u64, h1: u64, h2: u64, h3: u64) -> Option<u32> {
    let p0 = memory::try_read_u64(handle, module_base + s).filter(|&p| memory::is_valid_user_ptr(p))?;
    let p1 = memory::try_read_u64(handle, p0 + h1).filter(|&p| memory::is_valid_user_ptr(p))?;
    let p2 = memory::try_read_u64(handle, p1 + h2).filter(|&p| memory::is_valid_user_ptr(p))?;
    Some(memory::try_read_u64(handle, p2 + h3)? as u32)
}

fn write_static4(handle: windows::Win32::Foundation::HANDLE, module_base: u64, s: u64, h1: u64, h2: u64, h3: u64, value: u32) {
    let p0 = match memory::try_read_u64(handle, module_base + s).filter(|&p| memory::is_valid_user_ptr(p)) {
        Some(p) => p, None => { eprintln!("[-] static read failed"); return; }
    };
    let p1 = match memory::try_read_u64(handle, p0 + h1).filter(|&p| memory::is_valid_user_ptr(p)) {
        Some(p) => p, None => { eprintln!("[-] h1 read failed"); return; }
    };
    let p2 = match memory::try_read_u64(handle, p1 + h2).filter(|&p| memory::is_valid_user_ptr(p)) {
        Some(p) => p, None => { eprintln!("[-] h2 read failed"); return; }
    };
    let addr = p2 + h3;
    let before = memory::try_read_u64(handle, addr).unwrap_or(0) as u32;
    match memory::write_u32(handle, addr, value) {
        Ok(()) => println!("{DIM}  0x{addr:016X}  {before} → {value}{RESET}"),
        Err(e) => eprintln!("[-] write failed: {e}"),
    }
}

fn read_target(handle: windows::Win32::Foundation::HANDLE, container: u64, b: u64, s: u64, t: u64) -> Option<u32> {
    let branch = memory::try_read_u64(handle, container + b).filter(|&p| memory::is_valid_user_ptr(p))?;
    let obj    = memory::try_read_u64(handle, branch + s).filter(|&p| memory::is_valid_user_ptr(p))?;
    Some(memory::try_read_u64(handle, obj + t)? as u32)
}

fn write_target(handle: windows::Win32::Foundation::HANDLE, container: u64, b: u64, s: u64, t: u64, value: u32) {
    let branch = match memory::read_ptr(handle, container + b, 5) {
        Ok(p) => p,
        Err(e) => { eprintln!("[-] +{b:#X}: {e}"); return; }
    };
    let obj = match memory::read_ptr(handle, branch + s, 6) {
        Ok(p) => p,
        Err(e) => { eprintln!("[-] +{b:#X}→+{s:#X}: {e}"); return; }
    };
    let addr = obj + t;
    let before = memory::try_read_u64(handle, addr).unwrap_or(0) as u32;
    match memory::write_u32(handle, addr, value) {
        Ok(()) => println!("{DIM}  0x{addr:016X}  {before} → {value}{RESET}"),
        Err(e) => eprintln!("[-] write failed: {e}"),
    }
}


