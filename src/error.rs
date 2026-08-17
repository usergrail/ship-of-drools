use std::fmt;

#[derive(Debug)]
pub enum MemError {
    ProcessNotFound(String),
    ModuleNotFound(String),
    ReadFailed {
        address: u64,
        hresult: i32,
        bytes_read: usize,
        bytes_requested: usize,
    },
    NullPointer {
        step: usize,
        at_address: u64,
    },
    NonCanonical { step: usize, address: u64 },
    WinApi(windows::core::Error),
}

impl fmt::Display for MemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessNotFound(n) => write!(f, "process not found: {n}"),
            Self::ModuleNotFound(n) => write!(f, "module not found: {n}"),
            Self::ReadFailed { address, hresult, bytes_read, bytes_requested } => write!(
                f,
                "ReadProcessMemory at 0x{address:016X} failed \
                 (HRESULT=0x{hresult:08X}); got {bytes_read}/{bytes_requested} bytes"
            ),
            Self::NullPointer { step, at_address } => write!(
                f,
                "null pointer at step {step} (read from 0x{at_address:016X})"
            ),
            Self::NonCanonical { step, address } => write!(
                f,
                "non-canonical address 0x{address:016X} at step {step}"
            ),
            Self::WinApi(e) => write!(f, "WinAPI: {e}"),
        }
    }
}

impl std::error::Error for MemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::WinApi(e) = self { Some(e) } else { None }
    }
}

impl From<windows::core::Error> for MemError {
    fn from(e: windows::core::Error) -> Self {
        Self::WinApi(e)
    }
}

pub type MemResult<T> = Result<T, MemError>;
