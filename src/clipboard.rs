//! The system clipboard, to pass a room code along without retyping it. Only
//! used when the player presses a button for it. Elsewhere than Windows it
//! does nothing.

/// A room code and an address fit many times over.
const LIMIT: usize = 200;

#[cfg(windows)]
mod system {
    use super::LIMIT;

    const UNICODE_TEXT: u32 = 13;
    const MOVEABLE: u32 = 2;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenClipboard(owner: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> isize;
        fn SetClipboardData(format: u32, memory: isize) -> isize;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
        fn GlobalFree(memory: isize) -> isize;
        fn GlobalLock(memory: isize) -> *mut u16;
        fn GlobalUnlock(memory: isize) -> i32;
        fn GlobalSize(memory: isize) -> usize;
    }

    pub fn read() -> Option<String> {
        // SAFETY: the clipboard is closed on every path once opened. The text
        // is read only within the size the system reports for its block.
        unsafe {
            if OpenClipboard(0) == 0 {
                return None;
            }
            let memory = GetClipboardData(UNICODE_TEXT);
            let mut text = None;
            if memory != 0 {
                let start = GlobalLock(memory);
                if !start.is_null() {
                    let room = (GlobalSize(memory) / 2).min(LIMIT);
                    let units = std::slice::from_raw_parts(start, room);
                    let end = units.iter().position(|&u| u == 0).unwrap_or(room);
                    text = Some(String::from_utf16_lossy(&units[..end]));
                    GlobalUnlock(memory);
                }
            }
            CloseClipboard();
            text
        }
    }

    pub fn write(text: &str) -> bool {
        let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
        // SAFETY: the block is exactly as large as what is copied into it.
        // Once handed to the clipboard it belongs to the system; otherwise it
        // is freed here.
        unsafe {
            if OpenClipboard(0) == 0 {
                return false;
            }
            let memory = GlobalAlloc(MOVEABLE, units.len() * 2);
            let mut done = false;
            if memory != 0 {
                let start = GlobalLock(memory);
                if !start.is_null() {
                    std::ptr::copy_nonoverlapping(units.as_ptr(), start, units.len());
                    GlobalUnlock(memory);
                    done = EmptyClipboard() != 0 && SetClipboardData(UNICODE_TEXT, memory) != 0;
                }
                if !done {
                    GlobalFree(memory);
                }
            }
            CloseClipboard();
            done
        }
    }
}

/// The text in the clipboard, shortened to what a code could be.
pub fn read() -> Option<String> {
    #[cfg(windows)]
    {
        system::read().map(|t| t.chars().take(LIMIT).collect())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn write(text: &str) -> bool {
    #[cfg(windows)]
    {
        system::write(text)
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        false
    }
}
