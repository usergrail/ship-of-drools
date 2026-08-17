use windows::Win32::Foundation::HANDLE;

use crate::{error::MemResult, memory::read_ptr};

const STATIC_OFFSET: u64 = 0x030B_5818;
const PREFIX: &[u64] = &[0xB8, 0x8, 0x10, 0x170];

pub fn resolve_prefix(handle: HANDLE, module_base: u64, verbose: bool) -> MemResult<u64> {
    let root_addr = module_base + STATIC_OFFSET;
    if verbose { println!("[chain] root addr   0x{root_addr:016X}"); }

    let mut ptr = read_ptr(handle, root_addr, 0)?;
    if verbose { println!("[chain] step 0      0x{ptr:016X}"); }

    for (i, &offset) in PREFIX.iter().enumerate() {
        let src = ptr + offset;
        ptr = read_ptr(handle, src, i + 1)?;
        if verbose {
            println!(
                "[chain] step {step}  +0x{offset:<5X}  @ 0x{src:016X}  →  0x{ptr:016X}",
                step = i + 1
            );
        }
    }

    if verbose { println!("[chain] container   0x{ptr:016X}"); }
    Ok(ptr)
}
