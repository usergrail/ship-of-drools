use crate::memory;
use std::collections::{HashSet, VecDeque};
use windows::Win32::Foundation::HANDLE;

const MAX_EXPORT_NAMES: u64 = 100_000;
const MAX_ASSEMBLIES: u64 = 4096;
const MAX_IMAGE_TYPES: u64 = 200_000;

pub fn find_export(handle: HANDLE, base: u64, name: &str) -> Option<u64> {
    let e_lfanew = memory::try_read_u32(handle, base + 0x3c)? as u64;
    let opt_hdr = base.checked_add(e_lfanew)?.checked_add(24)?;
    if memory::try_read_u16(handle, opt_hdr)? != 0x20b {
        return None;
    }
    let exp_rva = memory::try_read_u32(handle, opt_hdr + 0x70)? as u64;
    if exp_rva == 0 {
        return None;
    }
    let dir = base.checked_add(exp_rva)?;
    let num_names = (memory::try_read_u32(handle, dir + 0x18)? as u64).min(MAX_EXPORT_NAMES);
    let functions = base.checked_add(memory::try_read_u32(handle, dir + 0x1c)? as u64)?;
    let names = base.checked_add(memory::try_read_u32(handle, dir + 0x20)? as u64)?;
    let ordinals = base.checked_add(memory::try_read_u32(handle, dir + 0x24)? as u64)?;
    for i in 0..num_names {
        let name_rva = memory::try_read_u32(handle, names.checked_add(i * 4)?)? as u64;
        if !cstr_eq(handle, base.checked_add(name_rva)?, name.as_bytes()) {
            continue;
        }
        let ordinal = memory::try_read_u16(handle, ordinals.checked_add(i * 2)?)? as u64;
        let fn_rva = memory::try_read_u32(handle, functions.checked_add(ordinal * 4)?)? as u64;
        return Some(follow_thunks(handle, base.checked_add(fn_rva)?));
    }
    None
}

fn cstr_eq(handle: HANDLE, addr: u64, target: &[u8]) -> bool {
    memory::read_bytes_for_dump(handle, addr, target.len() + 1).is_some_and(|b| {
        b.len() == target.len() + 1 && &b[..target.len()] == target && b[target.len()] == 0
    })
}

fn read_cstr(handle: HANDLE, addr: u64) -> Option<String> {
    let b = memory::read_bytes_for_dump(handle, addr, 256)?;
    let end = b.iter().position(|&v| v == 0)?;
    std::str::from_utf8(&b[..end]).ok().map(str::to_owned)
}

fn rel_target(next: u64, rel: i32) -> Option<u64> {
    let target = (next as i64).checked_add(rel as i64)?;
    (target > 0).then_some(target as u64)
}

fn follow_thunks(handle: HANDLE, mut addr: u64) -> u64 {
    let mut seen = HashSet::new();
    for _ in 0..8 {
        if !seen.insert(addr) {
            break;
        }
        let Some(b) = memory::read_bytes_for_dump(handle, addr, 6) else {
            break;
        };
        if b.len() >= 5 && b[0] == 0xe9 {
            let rel = i32::from_le_bytes([b[1], b[2], b[3], b[4]]);
            if let Some(target) =
                rel_target(addr + 5, rel).filter(|&p| memory::is_valid_user_ptr(p))
            {
                addr = target;
                continue;
            }
        }
        if b.len() >= 6 && b[0] == 0xff && b[1] == 0x25 {
            let rel = i32::from_le_bytes([b[2], b[3], b[4], b[5]]);
            if let Some(target) = rel_target(addr + 6, rel)
                .and_then(|slot| memory::try_read_u64(handle, slot))
                .filter(|&p| memory::is_valid_user_ptr(p))
            {
                addr = target;
                continue;
            }
        }
        break;
    }
    addr
}

fn probe_rip_refs(handle: HANDLE, fn_addr: u64, limit: usize) -> Vec<u64> {
    let Some(b) = memory::read_bytes_for_dump(handle, fn_addr, limit) else {
        return Vec::new();
    };
    let mut refs = Vec::new();
    for i in 0..b.len().saturating_sub(6) {
        if (b[i] & 0xf8) == 0x48 && matches!(b[i + 1], 0x8b | 0x8d) && (b[i + 2] & 0xc7) == 0x05 {
            let rel = i32::from_le_bytes([b[i + 3], b[i + 4], b[i + 5], b[i + 6]]);
            if let Some(target) = rel_target(fn_addr + i as u64 + 7, rel) {
                refs.push(target);
            }
        }
    }
    refs
}

fn probe_code_graph_rip_refs(handle: HANDLE, root: u64) -> Vec<u64> {
    const BYTES: usize = 128;
    const MAX_DEPTH: u8 = 3;
    const MAX_FUNCTIONS: usize = 256;
    let mut queue = VecDeque::from([(root, 0u8)]);
    let mut seen = HashSet::new();
    let mut refs = Vec::new();
    while let Some((addr, depth)) = queue.pop_front() {
        if seen.len() >= MAX_FUNCTIONS || !seen.insert(addr) {
            continue;
        }
        refs.extend(probe_rip_refs(handle, addr, BYTES));
        if depth == MAX_DEPTH {
            continue;
        }
        let Some(b) = memory::read_bytes_for_dump(handle, addr, BYTES) else {
            continue;
        };
        for i in 0..b.len().saturating_sub(4) {
            if !matches!(b[i], 0xe8 | 0xe9) {
                continue;
            }
            let rel = i32::from_le_bytes([b[i + 1], b[i + 2], b[i + 3], b[i + 4]]);
            if let Some(target) =
                rel_target(addr + i as u64 + 5, rel).filter(|&p| memory::is_valid_user_ptr(p))
            {
                let target = follow_thunks(handle, target);
                refs.extend(probe_rip_refs(handle, target, BYTES));
                queue.push_back((target, depth + 1));
            }
        }
    }
    refs.sort_unstable();
    refs.dedup();
    refs
}

fn probe_field_off64(handle: HANDLE, fn_addr: u64) -> Option<u64> {
    let b = memory::read_bytes_for_dump(handle, fn_addr, 64)?;
    for i in 0..b.len().saturating_sub(2) {
        if b[i] != 0x48 || b[i + 1] != 0x8b {
            continue;
        }
        let modrm = b[i + 2];
        if modrm & 7 != 1 {
            continue;
        }
        match modrm >> 6 {
            0 => return Some(0),
            1 if i + 3 < b.len() => {
                let d = b[i + 3] as i8 as i64;
                if d >= 0 {
                    return Some(d as u64);
                }
            }
            2 if i + 6 < b.len() => {
                let d = i32::from_le_bytes([b[i + 3], b[i + 4], b[i + 5], b[i + 6]]);
                if d >= 0 {
                    return Some(d as u64);
                }
            }
            _ => {}
        }
    }
    None
}

fn probe_field_off32(handle: HANDLE, fn_addr: u64) -> Option<u64> {
    let b = memory::read_bytes_for_dump(handle, fn_addr, 64)?;
    for i in 0..b.len().saturating_sub(1) {
        let (modrm_at, disp_at) = if b[i] == 0x8b {
            (i + 1, i + 2)
        } else if i + 2 < b.len() && b[i] == 0x48 && b[i + 1] == 0x63 {
            (i + 2, i + 3)
        } else {
            continue;
        };
        let modrm = b[modrm_at];
        if modrm & 7 != 1 {
            continue;
        }
        match modrm >> 6 {
            0 => return Some(0),
            1 if disp_at < b.len() => {
                let d = b[disp_at] as i8 as i64;
                if d >= 0 {
                    return Some(d as u64);
                }
            }
            2 if disp_at + 3 < b.len() => {
                let d = i32::from_le_bytes([
                    b[disp_at],
                    b[disp_at + 1],
                    b[disp_at + 2],
                    b[disp_at + 3],
                ]);
                if d >= 0 {
                    return Some(d as u64);
                }
            }
            _ => {}
        }
    }
    None
}

fn probe_field_off32_graph(handle: HANDLE, root: u64) -> Option<u64> {
    if let Some(off) = probe_field_off32(handle, root) {
        return Some(off);
    }
    let b = memory::read_bytes_for_dump(handle, root, 96)?;
    for i in 0..b.len().saturating_sub(4) {
        if !matches!(b[i], 0xe8 | 0xe9) {
            continue;
        }
        let rel = i32::from_le_bytes([b[i + 1], b[i + 2], b[i + 3], b[i + 4]]);
        let Some(target) = rel_target(root + i as u64 + 5, rel) else {
            continue;
        };
        if let Some(off) = probe_field_off32(handle, follow_thunks(handle, target)) {
            return Some(off);
        }
    }
    None
}

pub fn get_domain(handle: HANDLE, base: u64) -> Option<u64> {
    let f = find_export(handle, base, "il2cpp_domain_get")?;
    let mut refs = probe_rip_refs(handle, f, 64);
    refs.extend(probe_code_graph_rip_refs(handle, f));
    refs.into_iter()
        .filter_map(|var| memory::try_read_u64(handle, var))
        .find(|&p| memory::is_valid_user_ptr(p))
}

pub fn find_image(handle: HANDLE, base: u64, assembly_name: &str) -> Option<u64> {
    let image_off = probe_field_off64(
        handle,
        find_export(handle, base, "il2cpp_assembly_get_image")?,
    )?;
    let f = find_export(handle, base, "il2cpp_domain_get_assemblies")?;
    for reference in probe_code_graph_rip_refs(handle, f) {
        for vector in [Some(reference), memory::try_read_u64(handle, reference)]
            .into_iter()
            .flatten()
        {
            let Some(begin) = memory::try_read_u64(handle, vector) else {
                continue;
            };
            let Some(end) = memory::try_read_u64(handle, vector + 8) else {
                continue;
            };
            if !memory::is_valid_user_ptr(begin) || end < begin || (end - begin) % 8 != 0 {
                continue;
            }
            let count = (end - begin) / 8;
            if count == 0 || count > MAX_ASSEMBLIES {
                continue;
            }
            if let Some(image) = scan_assemblies(handle, begin, count, image_off, assembly_name) {
                return Some(image);
            }
        }
    }
    None
}

fn scan_assemblies(
    handle: HANDLE,
    data: u64,
    count: u64,
    image_off: u64,
    wanted: &str,
) -> Option<u64> {
    for i in 0..count {
        let Some(assembly) = memory::try_read_u64(handle, data + i * 8) else {
            continue;
        };
        let Some(image) = memory::try_read_u64(handle, assembly + image_off) else {
            continue;
        };
        let Some(name_ptr) = memory::try_read_u64(handle, image) else {
            continue;
        };
        let Some(name) = read_cstr(handle, name_ptr) else {
            continue;
        };
        let stem = name.strip_suffix(".dll").unwrap_or(&name);
        if stem.eq_ignore_ascii_case(wanted) || name.eq_ignore_ascii_case(wanted) {
            return Some(image);
        }
    }
    None
}

pub fn find_class(
    handle: HANDLE,
    base: u64,
    image: u64,
    namespace: &str,
    class_name: &str,
) -> Option<u64> {
    let get_class = find_export(handle, base, "il2cpp_image_get_class")?;
    let count_off = probe_field_off32_graph(
        handle,
        find_export(handle, base, "il2cpp_image_get_class_count")?,
    )?;
    let name_off = probe_field_off64(handle, find_export(handle, base, "il2cpp_class_get_name")?)?;
    let namespace_off = probe_field_off64(
        handle,
        find_export(handle, base, "il2cpp_class_get_namespace")?,
    )?;
    let count = memory::try_read_u32(handle, image + count_off)? as u64;
    if count == 0 || count > MAX_IMAGE_TYPES {
        return None;
    }

    let first = if let Some(holder_off) = probe_field_off64(handle, get_class) {
        let holder = memory::try_read_u64(handle, image + holder_off)?;
        memory::try_read_u32(handle, holder)? as u64
    } else {
        let start_off = probe_field_off32(handle, get_class)?;
        memory::try_read_u32(handle, image + start_off)? as u64
    };

    for reference in probe_code_graph_rip_refs(handle, get_class) {
        for table in [memory::try_read_u64(handle, reference), Some(reference)]
            .into_iter()
            .flatten()
        {
            if !memory::is_valid_user_ptr(table) {
                continue;
            }
            if let Some(klass) = scan_classes(
                handle,
                table,
                first,
                count,
                name_off,
                namespace_off,
                namespace,
                class_name,
            ) {
                return Some(klass);
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn scan_classes(
    handle: HANDLE,
    table: u64,
    first: u64,
    count: u64,
    name_off: u64,
    namespace_off: u64,
    wanted_ns: &str,
    wanted_name: &str,
) -> Option<u64> {
    for i in 0..count {
        let slot = first.checked_add(i)?.checked_mul(8)?.checked_add(table)?;
        let Some(klass) = memory::try_read_u64(handle, slot) else {
            continue;
        };
        let Some(name_ptr) = memory::try_read_u64(handle, klass + name_off) else {
            continue;
        };
        if read_cstr(handle, name_ptr).as_deref() != Some(wanted_name) {
            continue;
        }
        let Some(ns_ptr) = memory::try_read_u64(handle, klass + namespace_off) else {
            continue;
        };
        if read_cstr(handle, ns_ptr).as_deref() == Some(wanted_ns) {
            return Some(klass);
        }
    }
    None
}

pub fn static_fields(handle: HANDLE, base: u64, klass: u64) -> Option<u64> {
    let off = probe_field_off64(
        handle,
        find_export(handle, base, "il2cpp_class_get_static_field_data")?,
    )?;
    memory::try_read_u64(handle, klass + off).filter(|&p| memory::is_valid_user_ptr(p))
}

pub fn field_offset(handle: HANDLE, base: u64, klass: u64, wanted: &str) -> Option<u64> {
    let get_fields = find_export(handle, base, "il2cpp_class_get_fields")?;
    let name_off = probe_field_off64(handle, find_export(handle, base, "il2cpp_field_get_name")?)?;
    let value_off = probe_field_off32(
        handle,
        find_export(handle, base, "il2cpp_field_get_offset")?,
    )?;
    let code = memory::read_bytes_for_dump(handle, get_fields, 192)?;

    let mut pointer_offsets = Vec::new();
    let mut count_offsets = Vec::new();
    let mut strides = Vec::new();
    for i in 0..code.len() {
        if i + 6 < code.len()
            && (code[i] & 0xf8) == 0x48
            && code[i + 1] == 0x8b
            && code[i + 2] >> 6 == 2
        {
            let d = u32::from_le_bytes([code[i + 3], code[i + 4], code[i + 5], code[i + 6]]) as u64;
            pointer_offsets.push(d);
        }
        if i + 6 < code.len() && code[i] == 0x0f && code[i + 1] == 0xb7 && code[i + 2] >> 6 == 2 {
            let d = u32::from_le_bytes([code[i + 3], code[i + 4], code[i + 5], code[i + 6]]) as u64;
            count_offsets.push(d);
        }
        if i + 3 < code.len()
            && code[i] == 0x48
            && code[i + 1] == 0x83
            && code[i + 2] >> 3 & 7 == 0
            && code[i + 3] != 0
        {
            strides.push(code[i + 3] as u64);
        }
        if i + 3 < code.len()
            && code[i] == 0x48
            && code[i + 1] == 0xc1
            && code[i + 2] >> 3 & 7 == 4
            && code[i + 3] < 16
        {
            strides.push(1u64 << code[i + 3]);
        }
    }
    pointer_offsets.sort_unstable();
    pointer_offsets.dedup();
    count_offsets.sort_unstable();
    count_offsets.dedup();
    strides.sort_unstable();
    strides.dedup();

    for pointer_off in pointer_offsets {
        let Some(fields) = memory::try_read_u64(handle, klass + pointer_off)
            .filter(|&p| memory::is_valid_user_ptr(p))
        else {
            continue;
        };
        for &count_off in &count_offsets {
            let Some(count) = memory::try_read_u16(handle, klass + count_off).map(u64::from) else {
                continue;
            };
            if count == 0 || count > 4096 {
                continue;
            }
            for &stride in &strides {
                for index in 0..count {
                    let field = fields.checked_add(index.checked_mul(stride)?)?;
                    let Some(name_ptr) = memory::try_read_u64(handle, field + name_off) else {
                        continue;
                    };
                    if read_cstr(handle, name_ptr).as_deref() == Some(wanted) {
                        return Some(memory::try_read_u32(handle, field + value_off)? as u64);
                    }
                }
            }
        }
    }
    None
}
