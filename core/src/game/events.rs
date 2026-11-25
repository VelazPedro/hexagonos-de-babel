//! Input the simulation consumes and presentation events it produces.

use super::vec::V2;
use crate::babel::rarity::Rarity;

/// Device-independent controls for one simulation step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Desired movement; length is clamped to 1.
    pub movement: V2,
    /// World position the mouse points at, if a mouse is in use.
    pub aim_at: Option<V2>,
    /// Right-stick direction, if used.
    pub aim_dir: Option<V2>,
    pub attack_pressed: bool,
    pub attack_held: bool,
    pub dash: bool,
    pub interact: bool,
    pub ranged: bool,
    pub torch: bool,
    pub tonic: bool,
    pub lamp: bool,
    pub read: bool,
    /// +1 / -1 to cycle the active tool.
    pub cycle_tool: i32,
}

impl Input {
    /// Keeps the one-frame presses of `other` so they are not lost.
    pub fn absorb_presses(&mut self, other: &Input) {
        self.attack_pressed |= other.attack_pressed;
        self.dash |= other.dash;
        self.interact |= other.interact;
        self.torch |= other.torch;
        self.tonic |= other.tonic;
        self.lamp |= other.lamp;
        self.read |= other.read;
        self.cycle_tool += other.cycle_tool;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Step,
    StepWater,
    Swing,
    SwingHeavy,
    Charge,
    Hit,
    HitArmor,
    EnemyDie,
    EliteDie,
    Dash,
    Hurt,
    Death,
    HarvestWood,
    HarvestStone,
    HarvestMetal,
    HarvestGlass,
    HarvestPlant,
    NodeBreak,
    Pickup,
    DropCommon,
    DropRare,
    DropEpic,
    DropLegendary,
    DropCanonical,
    Craft,
    Build,
    LevelUp,
    SkillLearn,
    ReadPage,
    WordFound,
    LegendaryFanfare,
    Inscribe,
    LampLight,
    TorchPlace,
    Teleport,
    Stairs,
    Door,
    ChestOpen,
    UiMove,
    UiConfirm,
    UiBack,
    UiError,
    Throw,
    Explosion,
    BossRoar,
    Freeze,
    Poison,
    Burn,
    Locked,
    Heal,
    Victory,
}

impl Sfx {
    pub const ALL: [Sfx; 51] = [
        Sfx::Step,
        Sfx::StepWater,
        Sfx::Swing,
        Sfx::SwingHeavy,
        Sfx::Charge,
        Sfx::Hit,
        Sfx::HitArmor,
        Sfx::EnemyDie,
        Sfx::EliteDie,
        Sfx::Dash,
        Sfx::Hurt,
        Sfx::Death,
        Sfx::HarvestWood,
        Sfx::HarvestStone,
        Sfx::HarvestMetal,
        Sfx::HarvestGlass,
        Sfx::HarvestPlant,
        Sfx::NodeBreak,
        Sfx::Pickup,
        Sfx::DropCommon,
        Sfx::DropRare,
        Sfx::DropEpic,
        Sfx::DropLegendary,
        Sfx::DropCanonical,
        Sfx::Craft,
        Sfx::Build,
        Sfx::LevelUp,
        Sfx::SkillLearn,
        Sfx::ReadPage,
        Sfx::WordFound,
        Sfx::LegendaryFanfare,
        Sfx::Inscribe,
        Sfx::LampLight,
        Sfx::TorchPlace,
        Sfx::Teleport,
        Sfx::Stairs,
        Sfx::Door,
        Sfx::ChestOpen,
        Sfx::UiMove,
        Sfx::UiConfirm,
        Sfx::UiBack,
        Sfx::UiError,
        Sfx::Throw,
        Sfx::Explosion,
        Sfx::BossRoar,
        Sfx::Freeze,
        Sfx::Poison,
        Sfx::Burn,
        Sfx::Locked,
        Sfx::Heal,
        Sfx::Victory,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberKind {
    Enemy,
    Echo,
    Player,
    Heal,
    Burn,
    Poison,
}

/// Screens the simulation asks the interface to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRequest {
    /// Choose a station for a home building plot.
    Build(u8),
    Crafting,
    Scriptorium,
    Chest,
    Reader,
    /// Fast travel between lit return lamps.
    Lamps,
}

#[derive(Clone, Debug)]
pub enum GameEvent {
    Sound(Sfx),
    Particles {
        pos: V2,
        z: f32,
        color: [u8; 3],
        count: u16,
        speed: f32,
    },
    Number {
        pos: V2,
        amount: i32,
        kind: NumberKind,
    },
    /// Floating text over the world ("¡Esquiva!", "+3 Papel").
    Floating {
        pos: V2,
        text: String,
        color: [u8; 3],
    },
    Shake(f32),
    Message(String),
    Banner {
        title: String,
        subtitle: String,
        color: [u8; 3],
    },
    /// Full-screen flash for a Legendary or Canonical discovery.
    Flash(Rarity),
    LevelUp(u32),
    Open(ScreenRequest),
    /// A new site has been loaded.
    SiteEntered,
    Autosave,
    PlayerDied,
    /// The Catalog of catalogs has fallen.
    Ending,
}
