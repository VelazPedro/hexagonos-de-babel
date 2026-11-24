//! Options and key bindings, stored next to the save file.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Remappable keyboard actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Attack,
    Dash,
    Interact,
    Ranged,
    Torch,
    Tonic,
    Lamp,
    Read,
    NextTool,
    Inventory,
    Skills,
    Glossary,
    Map,
    Catalog,
}

impl Action {
    pub const ALL: [Action; 18] = [
        Action::Up,
        Action::Down,
        Action::Left,
        Action::Right,
        Action::Attack,
        Action::Dash,
        Action::Interact,
        Action::Ranged,
        Action::Torch,
        Action::Tonic,
        Action::Lamp,
        Action::Read,
        Action::NextTool,
        Action::Inventory,
        Action::Skills,
        Action::Glossary,
        Action::Map,
        Action::Catalog,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Up => "Arriba",
            Action::Down => "Abajo",
            Action::Left => "Izquierda",
            Action::Right => "Derecha",
            Action::Attack => "Atacar / recolectar",
            Action::Dash => "Esquivar",
            Action::Interact => "Interactuar",
            Action::Ranged => "Lanzar páginas",
            Action::Torch => "Plantar antorcha",
            Action::Tonic => "Beber tónico",
            Action::Lamp => "Lámpara de retorno",
            Action::Read => "Leer el libro",
            Action::NextTool => "Cambiar herramienta",
            Action::Inventory => "Inventario",
            Action::Skills => "Habilidades",
            Action::Glossary => "Glosario",
            Action::Map => "Mapa",
            Action::Catalog => "Catálogo",
        }
    }

    pub fn default_key(self) -> &'static str {
        match self {
            Action::Up => "W",
            Action::Down => "S",
            Action::Left => "A",
            Action::Right => "D",
            Action::Attack => "J",
            Action::Dash => "Espacio",
            Action::Interact => "E",
            Action::Ranged => "K",
            Action::Torch => "T",
            Action::Tonic => "Q",
            Action::Lamp => "L",
            Action::Read => "R",
            Action::NextTool => "F",
            Action::Inventory => "I",
            Action::Skills => "H",
            Action::Glossary => "G",
            Action::Map => "M",
            Action::Catalog => "C",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Action::Attack => "attack",
            Action::Dash => "dash",
            Action::Interact => "interact",
            Action::Read => "read",
            Action::Lamp => "lamp",
            Action::Map => "map",
            Action::Skills => "skills",
            _ => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 0..=10
    pub master: u8,
    pub music: u8,
    pub effects: u8,
    /// 0 = automatic (largest integer scale that fits).
    pub pixel_scale: u8,
    pub fullscreen: bool,
    pub screen_shake: bool,
    pub keys: BTreeMap<Action, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            master: 8,
            music: 6,
            effects: 8,
            pixel_scale: 0,
            fullscreen: false,
            screen_shake: true,
            keys: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn key(&self, action: Action) -> &str {
        self.keys
            .get(&action)
            .map_or_else(|| action.default_key(), String::as_str)
    }

    /// Binds a key, swapping with whichever action already used it.
    pub fn bind(&mut self, action: Action, key: &str) {
        let previous = self.key(action).to_string();
        let owner = Action::ALL
            .into_iter()
            .find(|&a| a != action && self.key(a) == key);
        if let Some(other) = owner {
            self.keys.insert(other, previous);
        }
        self.keys.insert(action, key.to_string());
    }

    /// A broken options file falls back to defaults.
    pub fn load(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn store(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())
            .map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}

/// Per-user data directory.
pub fn data_dir() -> PathBuf {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = var("HEXBABEL_SAVE_DIR") {
        return PathBuf::from(dir);
    }
    let base = if cfg!(windows) {
        var("APPDATA").map(PathBuf::from)
    } else {
        var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("HexagonosDeBabel")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebinding_swaps_instead_of_duplicating() {
        let mut s = Settings::default();
        s.bind(Action::Dash, "E");
        assert_eq!(s.key(Action::Dash), "E");
        assert_eq!(s.key(Action::Interact), "Espacio");
    }
}
