use windows::Win32::{
    Foundation::HANDLE,
    System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory},
};

use crate::error::{MemError, MemResult};

const USER_SPACE_MIN: u64 = 0x0000_0000_0001_0000;
const USER_SPACE_MAX: u64 = 0x0000_7FFF_FFFF_FFFF;

pub fn is_valid_user_ptr(address: u64) -> bool {
    address >= USER_SPACE_MIN && address <= USER_SPACE_MAX
}

pub fn read_ptr(handle: HANDLE, address: u64, step: usize) -> MemResult<u64> {
    if address == 0 {
        return Err(MemError::NullPointer { step, at_address: 0 });
    }
    if !is_valid_user_ptr(address) {
        return Err(MemError::NonCanonical { step, address });
    }

    let mut buf = [0u8; 8];
    let mut bytes_read: usize = 0;

    let outcome = unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            buf.as_mut_ptr() as *mut _,
            8,
            Some(&mut bytes_read),
        )
    };

    if let Err(e) = outcome {
        return Err(MemError::ReadFailed {
            address,
            hresult: e.code().0,
            bytes_read,
            bytes_requested: 8,
        });
    }
    if bytes_read != 8 {
        return Err(MemError::ReadFailed {
            address,
            hresult: 0,
            bytes_read,
            bytes_requested: 8,
        });
    }

    let value = u64::from_le_bytes(buf);
    if value == 0 {
        return Err(MemError::NullPointer { step: step + 1, at_address: address });
    }
    Ok(value)
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
    unsafe {
        WriteProcessMemory(
            handle,
            address as *mut _,
            buf.as_ptr() as *const _,
            4,
            None,
        )
    }
    .map_err(MemError::WinApi)
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
