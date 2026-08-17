use windows::Win32::Foundation::HANDLE;

use crate::memory::{is_valid_user_ptr, try_read_u64};

pub fn scan3(handle: HANDLE, start: u64, target: u32, label: &str) -> usize {
    let mut l1: Vec<(u64, u64)> = Vec::new();
    for o1 in (0u64..=0x400).step_by(8) {
        if let Some(p) = try_read_u64(handle, start + o1) {
            if is_valid_user_ptr(p) {
                l1.push((o1, p));
            }
        }
    }

    let mut l2: Vec<(u64, u64, u64)> = Vec::new();
    for (o1, p1) in &l1 {
        for o2 in (0u64..=0x800).step_by(8) {
            if let Some(p) = try_read_u64(handle, p1 + o2) {
                if is_valid_user_ptr(p) {
                    l2.push((*o1, o2, p));
                }
            }
        }
    }

    let mut found = 0usize;
    for (o1, o2, p2) in &l2 {
        for o3 in (0u64..=0x800).step_by(8) {
            if let Some(raw) = try_read_u64(handle, p2 + o3) {
                if raw as u32 == target {
                    println!(
                        "[+] {label}+0x{o1:X}→+0x{o2:X}→+0x{o3:X} = {target}"
                    );
                    found += 1;
                }
            }
        }
    }
    found
}

pub fn scan_from_static(
    handle: HANDLE,
    module_base: u64,
    static_off: u64,
    target: u32,
) -> usize {
    let addr = module_base + static_off;
    let p0 = match try_read_u64(handle, addr) {
        Some(v) if is_valid_user_ptr(v) => v,
        _ => return 0,
    };
    let label = format!("dll+0x{static_off:08X}→*");
    scan3(handle, p0, target, &label)
}
