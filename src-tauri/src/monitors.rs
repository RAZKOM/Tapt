//! Enumerate connected displays for the settings UI and resolve the
//! overlay window’s fullscreen geometry on Windows.
//! Non-Windows builds return a single synthetic entry so the frontend
//! stays consistent.

use serde::{Deserialize, Serialize};

use crate::config::OverlayMonitorRect;

/// One connected display — mirrors the `MonitorInfo` interface in `src/types.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub device_name: String,
}

impl MonitorInfo {
    pub fn to_overlay_rect(&self) -> OverlayMonitorRect {
        OverlayMonitorRect {
            left: self.left,
            top: self.top,
            width: self.width,
            height: self.height,
        }
    }
}

#[cfg(windows)]
fn enumerate_monitors_windows() -> Vec<MonitorInfo> {
    use windows_sys::Win32::Foundation::{BOOL, FALSE, LPARAM, RECT, TRUE};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

    let mut out: Vec<MonitorInfo> = Vec::new();

    unsafe extern "system" fn callback(
        hmon: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let out = &mut *(data as *mut Vec<MonitorInfo>);
        let mut mi: MONITORINFOEXW = std::mem::zeroed();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(
            hmon,
            &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO,
        ) == FALSE
        {
            return TRUE;
        }
        let r = mi.monitorInfo.rcMonitor;
        let w = (r.right - r.left).max(0) as u32;
        let h = (r.bottom - r.top).max(0) as u32;
        let is_primary = (mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;

        let end = mi
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(mi.szDevice.len());
        let device_name = String::from_utf16_lossy(&mi.szDevice[..end]);

        out.push(MonitorInfo {
            left: r.left,
            top: r.top,
            width: w,
            height: h,
            is_primary,
            device_name,
        });

        TRUE
    }

    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(callback),
            &mut out as *mut Vec<MonitorInfo> as LPARAM,
        );
    }

    out
}

/// Enumerate monitors in virtual-screen coordinates (Windows) or a stub elsewhere.
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    #[cfg(windows)]
    {
        let mut monitors = enumerate_monitors_windows();
        monitors.sort_by_key(|m| (m.left, m.top, m.device_name.clone()));
        monitors
    }
    #[cfg(not(windows))]
    {
        vec![MonitorInfo {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
            device_name: "Display".to_string(),
        }]
    }
}

/// Preferred overlay bounds when the user has not picked a specific display.
pub fn primary_monitor_rect() -> Option<OverlayMonitorRect> {
    let monitors = enumerate_monitors();
    monitors
        .iter()
        .find(|m| m.is_primary)
        .or_else(|| monitors.first())
        .map(MonitorInfo::to_overlay_rect)
}

/// IPC name must match `invoke("list_monitors")` in the settings UI.
#[tauri::command]
pub fn list_monitors() -> Result<Vec<MonitorInfo>, String> {
    Ok(enumerate_monitors())
}
