use std::fmt;

#[derive(Debug)]
pub enum MemError {
    ProcessNotFound(String),
    ModuleNotFound(String),
    NonCanonical { step: usize, address: u64 },
    WinApi(windows::core::Error),
}

impl fmt::Display for MemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessNotFound(n) => write!(f, "process not found: {n}"),
            Self::ModuleNotFound(n) => write!(f, "module not found: {n}"),
            Self::NonCanonical { step, address } => {
                write!(f, "non-canonical address 0x{address:016X} at step {step}")
            }
            Self::WinApi(e) => write!(f, "WinAPI: {e}"),
        }
    }
}

impl std::error::Error for MemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::WinApi(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl From<windows::core::Error> for MemError {
    fn from(e: windows::core::Error) -> Self {
        Self::WinApi(e)
    }
}

pub type MemResult<T> = Result<T, MemError>;
