use windows::Win32::{
    Foundation::HANDLE,
    System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory},
};

use crate::error::{MemError, MemResult};

const USER_SPACE_MIN: u64 = 0x0000_0000_0001_0000;
const USER_SPACE_MAX: u64 = 0x0000_7FFF_FFFF_FFFF;

pub fn is_valid_user_ptr(address: u64) -> bool {
    (USER_SPACE_MIN..=USER_SPACE_MAX).contains(&address)
}

pub fn try_read_u64(handle: HANDLE, address: u64) -> Option<u64> {
    if !is_valid_user_ptr(address) {
        return None;
    }
    let mut buf = [0u8; 8];
    let mut bytes_read: usize = 0;
    unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            buf.as_mut_ptr() as *mut _,
            8,
            Some(&mut bytes_read),
        )
        .ok()?;
    }
    if bytes_read != 8 {
        return None;
    }
    Some(u64::from_le_bytes(buf))
}

pub fn write_u32(handle: HANDLE, address: u64, value: u32) -> MemResult<()> {
    if !is_valid_user_ptr(address) {
        return Err(MemError::NonCanonical { step: 0, address });
    }
    let buf = value.to_le_bytes();
    unsafe { WriteProcessMemory(handle, address as *mut _, buf.as_ptr() as *const _, 4, None) }
        .map_err(MemError::WinApi)
}

pub fn try_read_u32(handle: HANDLE, address: u64) -> Option<u32> {
    if !is_valid_user_ptr(address) {
        return None;
    }
    let mut buf = [0u8; 4];
    let mut bytes_read: usize = 0;
    unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            buf.as_mut_ptr() as *mut _,
            4,
            Some(&mut bytes_read),
        )
        .ok()?;
    }
    if bytes_read != 4 {
        return None;
    }
    Some(u32::from_le_bytes(buf))
}

pub fn try_read_u16(handle: HANDLE, address: u64) -> Option<u16> {
    if !is_valid_user_ptr(address) {
        return None;
    }
    let mut buf = [0u8; 2];
    let mut bytes_read: usize = 0;
    unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            buf.as_mut_ptr() as *mut _,
            2,
            Some(&mut bytes_read),
        )
        .ok()?;
    }
    if bytes_read != 2 {
        return None;
    }
    Some(u16::from_le_bytes(buf))
}

pub fn read_bytes_for_dump(handle: HANDLE, address: u64, count: usize) -> Option<Vec<u8>> {
    if !is_valid_user_ptr(address) {
        return None;
    }
    let mut buf = vec![0u8; count];
    let mut bytes_read: usize = 0;
    unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            buf.as_mut_ptr() as *mut _,
            count,
            Some(&mut bytes_read),
        )
        .ok()?;
    }
    if bytes_read == 0 {
        return None;
    }
    buf.truncate(bytes_read);
    Some(buf)
}
