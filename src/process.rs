use windows::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW,
            Process32NextW, MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
            PROCESS_VM_READ, PROCESS_VM_WRITE,
        },
    },
};

use crate::error::{MemError, MemResult};

pub struct ProcessHandle {
    pub handle: HANDLE,
    pub pid: u32,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub fn find_pid(process_name: &str) -> MemResult<u32> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if let Err(e) = Process32FirstW(snap, &mut entry) {
            let _ = CloseHandle(snap);
            return Err(MemError::WinApi(e));
        }

        loop {
            let name = wchar_to_string(&entry.szExeFile);
            if name.eq_ignore_ascii_case(process_name) {
                let pid = entry.th32ProcessID;
                let _ = CloseHandle(snap);
                return Ok(pid);
            }
            if Process32NextW(snap, &mut entry).is_err() {
                break;
            }
        }

        let _ = CloseHandle(snap);
        Err(MemError::ProcessNotFound(process_name.to_owned()))
    }
}

pub fn open_process(pid: u32) -> MemResult<ProcessHandle> {
    unsafe {
        let handle = OpenProcess(
            PROCESS_VM_READ | PROCESS_QUERY_INFORMATION | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL(0),
            pid,
        )?;
        Ok(ProcessHandle { handle, pid })
    }
}

pub fn get_module_base(pid: u32, module_name: &str) -> MemResult<u64> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid)?;

        let mut entry: MODULEENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;

        if let Err(e) = Module32FirstW(snap, &mut entry) {
            let _ = CloseHandle(snap);
            return Err(MemError::WinApi(e));
        }

        loop {
            let name = wchar_to_string(&entry.szModule);
            if name.eq_ignore_ascii_case(module_name) {
                let base = entry.modBaseAddr as u64;
                let _ = CloseHandle(snap);
                return Ok(base);
            }
            if Module32NextW(snap, &mut entry).is_err() {
                break;
            }
        }

        let _ = CloseHandle(snap);
        Err(MemError::ModuleNotFound(module_name.to_owned()))
    }
}

fn wchar_to_string(s: &[u16]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}
