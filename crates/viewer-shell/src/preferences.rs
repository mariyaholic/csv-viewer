// SPDX-License-Identifier: Apache-2.0
//! Appearance only. Never store file paths, searches, source data or clipboard text.
//! Read before creating HWNDs and atomically save after the window closes.
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub version: u32,
    pub theme: String,
    pub window: Option<Window>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            theme: "paper".into(),
            window: None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub maximized: bool,
}
#[derive(Clone, Copy)]
pub struct Area {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Window {
    fn valid(&self) -> bool {
        (320..=16384).contains(&self.width)
            && (240..=16384).contains(&self.height)
            && (48..=768).contains(&self.dpi)
    }
    /// Screen coordinates (not WINDOWPLACEMENT workspace coordinates): avoid taskbar creep.
    /// Keep negative monitor coordinates; clamp removed monitors and rescale size, not origin.
    pub fn fit(self, work: Area, dpi: u32) -> Self {
        let dpi = dpi.clamp(96, 768);
        let old_dpi = self.dpi.clamp(48, 768);
        let space_w = (i64::from(work.right) - i64::from(work.left)).clamp(1, i64::from(i32::MAX));
        let space_h = (i64::from(work.bottom) - i64::from(work.top)).clamp(1, i64::from(i32::MAX));
        let width = (u64::from(self.width) * u64::from(dpi) / u64::from(old_dpi))
            .clamp(1, space_w as u64) as u32;
        let height = (u64::from(self.height) * u64::from(dpi) / u64::from(old_dpi))
            .clamp(1, space_h as u64) as u32;
        Self {
            x: i64::from(self.x).clamp(
                i64::from(work.left),
                i64::from(work.left) + space_w - i64::from(width),
            ) as i32,
            y: i64::from(self.y).clamp(
                i64::from(work.top),
                i64::from(work.top) + space_h - i64::from(height),
            ) as i32,
            width,
            height,
            dpi,
            maximized: self.maximized,
        }
    }
}
fn checked(mut p: Preferences) -> Result<Preferences, String> {
    if p.version != 1 {
        return Err("Saved appearance uses an unsupported version".into());
    }
    if !crate::THEMES.iter().any(|t| t.id == p.theme) {
        p.theme = "paper".into();
    }
    p.window = p.window.filter(Window::valid);
    Ok(p)
}
pub fn file_in(root: &Path, title: &str) -> Option<PathBuf> {
    let id = match title {
        "CSV Viewer" => "csv-viewer",
        "Parquet Viewer" => "parquet-viewer",
        _ => return None,
    };
    if root.as_os_str().is_empty() {
        return None;
    }
    Some(root.join(id).join("desktop.json"))
}
pub fn load(path: &Path) -> Result<Preferences, String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(e) => return Err(format!("Cannot read saved appearance: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 {
        return Err("Saved appearance exceeds 16 KiB".into());
    }
    checked(serde_json::from_slice(&bytes).map_err(|e| format!("Invalid saved appearance: {e}"))?)
}
#[cfg(windows)]
mod save_lock {
    use std::{ffi::c_void, ptr};
    type Handle = isize;
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(attributes: *const c_void, owner: i32, name: *const u16) -> Handle;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn ReleaseMutex(handle: Handle) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }
    pub struct Guard(Handle);
    impl Guard {
        pub fn acquire() -> Result<Self, String> {
            // Session-local and shared by both viewers. No file paths or user data
            // enter the object name; abandoned owners are recoverable after a crash.
            let name: Vec<u16> = "Local\\MariaViewerAppearanceSaveV1"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe {
                let handle = CreateMutexW(ptr::null(), 0, name.as_ptr());
                if handle == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                match WaitForSingleObject(handle, 2000) {
                    0 | 0x80 => Ok(Self(handle)),
                    code => {
                        CloseHandle(handle);
                        Err(format!("Appearance save is busy or unavailable ({code:#x}); previous settings were preserved"))
                    }
                }
            }
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }
}
pub fn save(path: &Path, p: &Preferences) -> Result<(), String> {
    let p = checked(p.clone())?;
    let parent = path.parent().ok_or("Saved appearance needs a directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let _save_lock = save_lock::Guard::acquire()?;
    // Same-directory temporary file + atomic replace; no truncated JSON on a failed write.
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&p).map_err(|e| e.to_string())?;
    tmp.write_all(&bytes).map_err(|e| e.to_string())?;
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    let mut attempt = 0;
    loop {
        match tmp.persist(path) {
            Ok(file) => {
                drop(file);
                return Ok(());
            }
            // Windows can briefly reject replacement while another closing window
            // finishes its rename/handle close. Retry the SAME complete temp file;
            // never delete the destination or fall back to an in-place write.
            Err(e)
                if cfg!(windows)
                    && attempt < 4
                    && e.error
                        .raw_os_error()
                        .is_some_and(|code| matches!(code, 5 | 32 | 33)) =>
            {
                tmp = e.file;
                std::thread::sleep(std::time::Duration::from_millis(10 << attempt));
                attempt += 1;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}
