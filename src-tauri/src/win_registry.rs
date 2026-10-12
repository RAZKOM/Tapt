//! Minimal Windows registry access (`RegGetValueW` / `RegSetKeyValueW`).
//!
//! Used for the machine HWID (`HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`)
//! and for the trial's second persistence copy under `HKCU`. Every call is
//! best-effort: failures come back as `None` / `false`, never a panic.

#![cfg(windows)]

use std::ffi::c_void;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    RegGetValueW, RegSetKeyValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_QWORD,
    RRF_RT_REG_QWORD, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY,
};

/// Predefined root keys we use.
#[derive(Debug, Clone, Copy)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

impl Hive {
    fn hkey(self) -> HKEY {
        match self {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Read a `REG_SZ` value. Always reads the 64-bit view so a 32-bit build
/// sees the same `MachineGuid` as a 64-bit one.
pub fn read_string(root: Hive, subkey: &str, value: &str) -> Option<String> {
    let subkey_w = wide(subkey);
    let value_w = wide(value);
    let flags = RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY;

    let mut size: u32 = 0;
    // SAFETY: valid NUL-terminated wide strings; a null data pointer asks
    // only for the required size in bytes.
    let rc = unsafe {
        RegGetValueW(
            root.hkey(),
            subkey_w.as_ptr(),
            value_w.as_ptr(),
            flags,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if rc != ERROR_SUCCESS || size == 0 {
        return None;
    }

    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    let mut size_bytes = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `size_bytes` writable bytes.
    let rc = unsafe {
        RegGetValueW(
            root.hkey(),
            subkey_w.as_ptr(),
            value_w.as_ptr(),
            flags,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut c_void,
            &mut size_bytes,
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16(&buf[..len]).ok()
}

/// Read a `REG_QWORD` value.
pub fn read_u64(root: Hive, subkey: &str, value: &str) -> Option<u64> {
    let subkey_w = wide(subkey);
    let value_w = wide(value);
    let mut out: u64 = 0;
    let mut size = std::mem::size_of::<u64>() as u32;
    // SAFETY: `out` is 8 writable bytes and `size` says so.
    let rc = unsafe {
        RegGetValueW(
            root.hkey(),
            subkey_w.as_ptr(),
            value_w.as_ptr(),
            RRF_RT_REG_QWORD,
            std::ptr::null_mut(),
            &mut out as *mut u64 as *mut c_void,
            &mut size,
        )
    };
    (rc == ERROR_SUCCESS).then_some(out)
}

/// Write a `REG_QWORD` value, creating `subkey` if needed.
pub fn write_u64(root: Hive, subkey: &str, value: &str, data: u64) -> bool {
    let subkey_w = wide(subkey);
    let value_w = wide(value);
    // SAFETY: `data` is 8 readable bytes for the duration of the call.
    let rc = unsafe {
        RegSetKeyValueW(
            root.hkey(),
            subkey_w.as_ptr(),
            value_w.as_ptr(),
            REG_QWORD,
            &data as *const u64 as *const c_void,
            std::mem::size_of::<u64>() as u32,
        )
    };
    rc == ERROR_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_guid_is_readable() {
        let guid = read_string(
            Hive::LocalMachine,
            r"SOFTWARE\Microsoft\Cryptography",
            "MachineGuid",
        );
        assert!(guid.is_some_and(|g| g.len() >= 32));
    }

    #[test]
    fn qword_round_trip_creates_key() {
        let subkey = r"Software\Razkom\TaptTest";
        assert!(write_u64(Hive::CurrentUser, subkey, "Probe", 1234));
        assert_eq!(read_u64(Hive::CurrentUser, subkey, "Probe"), Some(1234));
        assert_eq!(read_u64(Hive::CurrentUser, subkey, "Missing"), None);
        let subkey_w = wide(subkey);
        // SAFETY: valid NUL-terminated wide string.
        unsafe {
            windows_sys::Win32::System::Registry::RegDeleteKeyW(
                HKEY_CURRENT_USER,
                subkey_w.as_ptr(),
            );
        }
    }
}
