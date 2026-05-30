//! Client-side clipboard implementation
//!
//! This module provides clipboard reading and writing capabilities
//! for the RDP Client. Uses platform-native APIs.

use anyhow::Result;

/// Clipboard content type (shared with common)
#[derive(Debug, Clone)]
pub enum ClipboardContent {
    Text(String),
}

impl From<&str> for ClipboardContent {
    fn from(s: &str) -> Self {
        ClipboardContent::Text(s.to_string())
    }
}

impl From<String> for ClipboardContent {
    fn from(s: String) -> Self {
        ClipboardContent::Text(s)
    }
}

/// Clipboard reader for the client
pub struct ClipboardReader;

impl ClipboardReader {
    /// Create a new clipboard reader
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Read text content from the system clipboard
    ///
    /// # Platform support
    /// - Windows: Uses `GetClipboardData` with `CF_UNICODETEXT`
    /// - Linux: Uses `xclip -selection clipboard -o` or `wl-paste`
    pub fn read_text(&self) -> Result<Option<String>> {
        #[cfg(target_os = "windows")]
        {
            self.read_text_windows()
        }
        #[cfg(target_os = "linux")]
        {
            self.read_text_linux()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(anyhow::anyhow!(
                "Clipboard reading is not supported on this platform"
            ))
        }
    }

    #[cfg(target_os = "windows")]
    fn read_text_windows(&self) -> Result<Option<String>> {
        use windows::Win32::UI::Clipboard::*;
        use windows::Win32::Foundation::{GlobalLock, GlobalUnlock};
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        unsafe {
            if OpenClipboard(None).is_err() {
                return Ok(None);
            }

            let handle = GetClipboardData(CF_UNICODETEXT.0);
            if handle.is_err() {
                CloseClipboard();
                return Ok(None);
            }

            let hglobal = handle.unwrap();
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                GlobalUnlock(hglobal);
                CloseClipboard();
                return Ok(None);
            }

            // Count wide characters until null terminator
            let mut len = 0;
            let mut p = ptr as *const u16;
            while *p != 0 {
                len += 1;
                p = p.add(1);
            }

            let wstring: Vec<u16> = std::slice::from_raw_parts(ptr as *const u16, len).to_vec();
            GlobalUnlock(hglobal);
            CloseClipboard();

            let os_string = OsString::from_wide(&wstring);
            Ok(Some(os_string.to_string_lossy().into_owned()))
        }
    }

    #[cfg(target_os = "linux")]
    fn read_text_linux(&self) -> Result<Option<String>> {
        use std::process::Command;

        // Try wl-paste first (Wayland)
        let output = Command::new("wl-paste")
            .arg("--no-newline")
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
            }
        }

        // Fall back to xclip (X11)
        let output = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .arg("-o")
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
            }
        }

        // Fall back to xsel
        let output = Command::new("xsel")
            .arg("--clipboard")
            .arg("--output")
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
            }
        }

        Ok(None)
    }
}

/// Clipboard writer for the client
pub struct ClipboardWriter;

impl ClipboardWriter {
    /// Create a new clipboard writer
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Write text content to the system clipboard
    ///
    /// # Platform support
    /// - Windows: Uses `SetClipboardData` with `CF_UNICODETEXT`
    /// - Linux: Uses `xclip -selection clipboard` or `wl-copy`
    pub fn write_text(&self, content: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.write_text_windows(content)
        }
        #[cfg(target_os = "linux")]
        {
            self.write_text_linux(content)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(anyhow::anyhow!(
                "Clipboard writing is not supported on this platform"
            ))
        }
    }

    #[cfg(target_os = "windows")]
    fn write_text_windows(&self, content: &str) -> Result<()> {
        use windows::Win32::UI::Clipboard::*;
        use windows::Win32::Foundation::{
            GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock, GHND,
        };
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        unsafe {
            if OpenClipboard(None).is_err() {
                return Err(anyhow::anyhow!("Failed to open clipboard"));
            }

            if EmptyClipboard().is_err() {
                CloseClipboard();
                return Err(anyhow::anyhow!("Failed to empty clipboard"));
            }

            // Convert to wide string with null terminator
            let os_str = OsStr::new(content);
            let wide: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0)).collect();

            // Allocate global memory
            let bytes = (wide.len() * 2) as u32;
            let hglobal = GlobalAlloc(GHND, bytes as usize);
            if hglobal.is_err() {
                CloseClipboard();
                return Err(anyhow::anyhow!("Failed to allocate memory"));
            }

            let hglobal = hglobal.unwrap();
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                GlobalFree(hglobal);
                CloseClipboard();
                return Err(anyhow::anyhow!("Failed to lock memory"));
            }

            // Copy wide string to global memory
            std::ptr::copy(wide.as_ptr(), ptr as *mut u16, wide.len());
            GlobalUnlock(hglobal);

            if SetClipboardData(CF_UNICODETEXT.0, hglobal).is_err() {
                GlobalFree(hglobal);
                CloseClipboard();
                return Err(anyhow::anyhow!("Failed to set clipboard data"));
            }

            CloseClipboard();
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    fn write_text_linux(&self, content: &str) -> Result<()> {
        use std::process::{Command, Stdio};

        // Try wl-copy first (Wayland)
        let result = Command::new("wl-copy")
            .arg("--type")
            .arg("text/plain")
            .arg(content)
            .status();

        if let Ok(status) = result {
            if status.success() {
                return Ok(());
            }
        }

        // Fall back to xclip (X11)
        let mut child = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .arg("-i")
            .stdin(Stdio::piped())
            .spawn()?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(content.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            return Ok(());
        }

        // Fall back to xsel
        let mut child = Command::new("xsel")
            .arg("--clipboard")
            .arg("--input")
            .stdin(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(content.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            return Ok(());
        }

        Err(anyhow::anyhow!("Failed to write to clipboard"))
    }
}

impl Default for ClipboardReader {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl Default for ClipboardWriter {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
