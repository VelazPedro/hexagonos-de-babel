//! Links the application icon into the Windows executable. The MSVC linker
//! takes a `.res` file directly, which is simple enough to write here without
//! any resource compiler or extra crate.
//!
//! The icon itself is drawn by the game: see `src/render/icon.rs`.

use std::error::Error;
use std::path::PathBuf;

const ICON: &str = "assets/icon/hexagonos.ico";
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// Explorer shows the first group of icons; the window loads it by this number.
const GROUP_ID: u16 = 1;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON}");
    let target = |key: &str| std::env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") != "windows" || target("CARGO_CFG_TARGET_ENV") != "msvc" {
        return Ok(());
    }
    let ico = std::fs::read(ICON)?;
    let path = PathBuf::from(std::env::var("OUT_DIR")?).join("icon.res");
    std::fs::write(&path, resources(&ico).ok_or("the icon file is damaged")?)?;
    println!("cargo:rustc-link-arg-bins={}", path.display());
    Ok(())
}

/// One entry of a `.res` file: a header naming it by numbers, then its data
/// padded to four bytes.
fn push(out: &mut Vec<u8>, kind: u16, name: u16, data: &[u8]) {
    out.extend((data.len() as u32).to_le_bytes());
    // The size of this header.
    out.extend(32u32.to_le_bytes());
    // 0xFFFF announces a number in place of a text.
    for value in [0xffff, kind, 0xffff, name] {
        out.extend(value.to_le_bytes());
    }
    // Versions, flags and language may all stay zero.
    out.extend([0u8; 16]);
    out.extend(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// Turns an `.ico` into the resources Windows expects: one per image, plus the
/// group that lists them.
fn resources(ico: &[u8]) -> Option<Vec<u8>> {
    let word = |at: usize| Some(u16::from_le_bytes(ico.get(at..at + 2)?.try_into().ok()?));
    let long = |at: usize| Some(u32::from_le_bytes(ico.get(at..at + 4)?.try_into().ok()?) as usize);
    let count = word(4)?;
    let mut out = Vec::new();
    // Every `.res` file opens with an empty entry.
    push(&mut out, 0, 0, &[]);
    // The group repeats the file's directory, with a resource number in place
    // of each file offset.
    let mut group = ico.get(..6)?.to_vec();
    for i in 0..count {
        let entry = 6 + 16 * usize::from(i);
        let (length, offset) = (long(entry + 8)?, long(entry + 12)?);
        let id = i + 1;
        push(&mut out, RT_ICON, id, ico.get(offset..offset + length)?);
        group.extend(ico.get(entry..entry + 12)?);
        group.extend(id.to_le_bytes());
    }
    push(&mut out, RT_GROUP_ICON, GROUP_ID, &group);
    Some(out)
}
