//! Versioned RON saves with atomic writes and a backup copy.

use crate::game::profile::{Profile, SAVE_VERSION};
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LoadOutcome {
    NoSave,
    Loaded(Box<Profile>),
    /// The main file was unusable; the backup was loaded instead.
    Recovered(Box<Profile>, String),
    /// Neither file could be used. The message is meant for the player.
    Failed(String),
}

pub fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("ron.bak")
}

fn temp_path(path: &Path) -> PathBuf {
    path.with_extension("ron.tmp")
}

#[derive(Deserialize)]
struct Header {
    version: u32,
}

pub fn parse(text: &str) -> Result<Profile, String> {
    let header: Header = ron::from_str(text).map_err(|_| "el archivo está dañado".to_string())?;
    if header.version != SAVE_VERSION {
        return Err(format!(
            "es de otra versión del juego (formato {}, se espera {SAVE_VERSION})",
            header.version
        ));
    }
    ron::from_str(text).map_err(|_| "el archivo está dañado".to_string())
}

fn load_file(path: &Path) -> Result<Profile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("no se pudo leer ({e})"))?;
    parse(&text)
}

pub fn load(path: &Path) -> LoadOutcome {
    let backup = backup_path(path);
    if !path.exists() && !backup.exists() {
        return LoadOutcome::NoSave;
    }
    match load_file(path) {
        Ok(p) => LoadOutcome::Loaded(Box::new(p)),
        Err(main_error) => match load_file(&backup) {
            Ok(p) => LoadOutcome::Recovered(
                Box::new(p),
                format!("La partida guardada {main_error}. Se cargó la copia de respaldo."),
            ),
            Err(_) => LoadOutcome::Failed(format!(
                "La partida guardada {main_error} y no hay una copia de respaldo válida."
            )),
        },
    }
}

/// Renames an unusable save and its backup so that a new game cannot destroy
/// them. Existing set-aside files are never overwritten.
pub fn set_aside(path: &Path) -> std::io::Result<()> {
    for file in [path.to_path_buf(), backup_path(path)] {
        if !file.exists() {
            continue;
        }
        let mut n = 1;
        let target = loop {
            let mut name = file.clone().into_os_string();
            name.push(format!(".apartado{n}"));
            let candidate = PathBuf::from(name);
            if !candidate.exists() {
                break candidate;
            }
            n += 1;
        };
        std::fs::rename(&file, target)?;
    }
    Ok(())
}

/// Writes to a temporary file, keeps the previous good save as backup and
/// renames the temporary file into place.
pub fn store(path: &Path, profile: &Profile) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = ron::ser::to_string_pretty(profile, ron::ser::PrettyConfig::new().depth_limit(4))
        .map_err(std::io::Error::other)?;
    let tmp = temp_path(path);
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    // Only a readable save may replace the backup.
    if path.exists() && load_file(path).is_ok() {
        std::fs::copy(path, backup_path(path))?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Resource;
    use crate::world::deltas::SiteDelta;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hexbabel-save-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn sample() -> Profile {
        let mut p = Profile::new(1234);
        p.player.add_resource(Resource::Iron, 7);
        let mut delta = SiteDelta::default();
        delta.depleted_nodes.insert(3);
        p.world.deltas.insert("w:1:0:0".into(), delta);
        p
    }

    #[test]
    fn roundtrip_and_backup_recovery() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("save.ron");
        assert!(matches!(load(&path), LoadOutcome::NoSave));
        let first = sample();
        store(&path, &first).expect("store");
        match load(&path) {
            LoadOutcome::Loaded(p) => assert_eq!(*p, first),
            other => panic!("{other:?}"),
        }
        let mut second = first.clone();
        second.player.level = 9;
        store(&path, &second).expect("store");
        std::fs::write(&path, "(version: 1, garbage").expect("corrupt");
        match load(&path) {
            LoadOutcome::Recovered(p, warning) => {
                assert_eq!(*p, first);
                assert!(warning.contains("respaldo"));
            }
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unusable_saves_are_set_aside_not_destroyed() {
        let dir = temp_dir("aside");
        let path = dir.join("save.ron");
        std::fs::write(&path, "(version: 99)").expect("write");
        std::fs::write(backup_path(&path), "(version: 98)").expect("write");
        set_aside(&path).expect("set aside");
        store(&path, &sample()).expect("store");
        store(&path, &sample()).expect("store");
        let kept = std::fs::read_to_string(dir.join("save.ron.apartado1")).expect("kept");
        assert_eq!(kept, "(version: 99)");
        assert!(dir.join("save.ron.bak.apartado1").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn other_versions_and_total_corruption_do_not_crash() {
        let dir = temp_dir("version");
        let path = dir.join("save.ron");
        std::fs::write(&path, "(version: 99, world_seed: 1)").expect("write");
        match load(&path) {
            LoadOutcome::Failed(m) => assert!(m.contains("otra versión"), "{m}"),
            other => panic!("{other:?}"),
        }
        std::fs::write(&path, [0u8, 159, 146, 150]).expect("write");
        assert!(matches!(load(&path), LoadOutcome::Failed(_)));
        let _ = std::fs::remove_dir_all(dir);
    }
}
