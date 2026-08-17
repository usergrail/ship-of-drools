use windows::Win32::Foundation::HANDLE;

use crate::memory::{read_bytes_for_dump, try_read_u64};

// ── Offsets derived from the six known pointer-chain tails ──────────────────
//
// These are NOT stable across sessions.  They are compiled here only as
// starting points for the scan.  The goal of this module is to find a
// property (ID, type tag, element count, string) that consistently
// distinguishes the correct child so we can replace these hardcoded paths
// with a runtime search.

/// First-level offsets from the container object.
const BRANCH_OFFSETS: &[u64] = &[0xA8, 0xC0];

/// Second-level offsets seen across the six recorded tails.
const SECOND_OFFSETS: &[u64] = &[0xB8, 0xC8, 0x148, 0x160, 0x418];

/// Terminal offsets seen in the recorded tails.  The value at these addresses
/// is the actual target data — show as many interpretations as possible.
const TAIL_OFFSETS: &[u64] = &[0x38, 0x354, 0x444, 0x524, 0x538];

/// Bytes to dump from each object header — enough to see vtable ptr, fields,
/// possible type-tag strings, or element counts.
const DUMP_SIZE: usize = 0x80;

// ─────────────────────────────────────────────────────────────────────────────

/// Walks every combination of known branch/second/tail offsets beneath
/// `container` and prints all readable values and raw memory.
///
/// Look for:
///   • A value that is constant across runs at the same branch (ID, tag, count).
///   • A readable ASCII/UTF-16 string near the object header.
///   • A vtable pointer at offset 0x0 that resolves to a known class.
///   • An integer at a small offset (0x8–0x20) that differs between children.
pub fn inspect_children(handle: HANDLE, container: u64) {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║  DIAGNOSTIC  container = 0x{container:016X}                  ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");

    println!("\n── container raw dump ──────────────────────────────────────────────");
    dump_region(handle, container, 0x100);

    for &b_off in BRANCH_OFFSETS {
        let branch_addr = container + b_off;
        let branch_ptr = match try_read_u64(handle, branch_addr) {
            Some(0) => {
                println!("\n  [container+0x{b_off:X}] → NULL");
                continue;
            }
            Some(v) => v,
            None => {
                println!("\n  [container+0x{b_off:X}] → <unreadable>");
                continue;
            }
        };

        println!("\n  [container+0x{b_off:X}] → 0x{branch_ptr:016X}");
        println!("  ── branch dump ─────────────────────────────────────────────────");
        dump_region(handle, branch_ptr, DUMP_SIZE);

        for &s_off in SECOND_OFFSETS {
            let second_addr = branch_ptr + s_off;
            let second_ptr = match try_read_u64(handle, second_addr) {
                Some(0) => {
                    println!("\n    [branch+0x{s_off:X}] → NULL");
                    continue;
                }
                Some(v) => v,
                None => {
                    println!("\n    [branch+0x{s_off:X}] → <unreadable>");
                    continue;
                }
            };

            println!("\n    [branch+0x{s_off:X}] → 0x{second_ptr:016X}");
            println!("    ── object dump ───────────────────────────────────────────────");
            dump_region(handle, second_ptr, DUMP_SIZE);

            println!("    ── tail reads ────────────────────────────────────────────────");
            for &t_off in TAIL_OFFSETS {
                let tail_addr = second_ptr + t_off;
                match try_read_u64(handle, tail_addr) {
                    Some(v) => {
                        let lo_u32 = (v & 0xFFFF_FFFF) as u32;
                        let as_f32 = f32::from_bits(lo_u32);
                        let as_i32 = lo_u32 as i32;
                        println!(
                            "      [obj+0x{t_off:X}] = 0x{v:016X} \
                             u64={v}  i32={as_i32}  f32={as_f32:.4}"
                        );
                    }
                    None => println!("      [obj+0x{t_off:X}] = <unreadable>"),
                }
            }
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════════════");
    println!("END DIAGNOSTIC");
    println!("═══════════════════════════════════════════════════════════════════════");
    println!();
    println!("What to look for in the output above:");
    println!("  1. A value that is CONSTANT at a given branch/second offset across restarts");
    println!("     → that is a candidate discriminator (type ID, object ID, role enum).");
    println!("  2. ASCII text in the raw dumps (e.g. class name, entity name).");
    println!("  3. A pointer-sized value at offset 0x0 (vtable) that is the same for all");
    println!("     children of the same type but different between types.");
    println!("  4. An integer at 0x8–0x20 that is unique per child (entity index).");
    println!("  5. Which branch (0xA8 vs 0xC0) always leads to the same second-level");
    println!("     offset across both sessions.");
}

fn dump_region(handle: HANDLE, base: u64, size: usize) {
    let Some(bytes) = read_bytes_for_dump(handle, base, size) else {
        println!("    <unreadable region at 0x{base:016X}>");
        return;
    };
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let addr = base + (i * 16) as u64;
        // Insert an extra space after the 8th byte for readability.
        let hex: String = chunk
            .iter()
            .enumerate()
            .map(|(j, b)| {
                if j == 8 {
                    format!(" {b:02X} ")
                } else {
                    format!("{b:02X} ")
                }
            })
            .collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("    {addr:016X}  {hex:<57} {ascii}");
    }
}
