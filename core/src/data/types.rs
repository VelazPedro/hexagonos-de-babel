use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Resource {
    Paper,
    Leather,
    Fiber,
    Ink,
    Oil,
    Stone,
    Iron,
    Brass,
    Silver,
    Crystal,
    Ash,
    Salt,
    Bone,
    Vitral,
}

impl Resource {
    pub const ALL: [Resource; 14] = [
        Resource::Paper,
        Resource::Leather,
        Resource::Fiber,
        Resource::Ink,
        Resource::Oil,
        Resource::Stone,
        Resource::Iron,
        Resource::Brass,
        Resource::Silver,
        Resource::Crystal,
        Resource::Ash,
        Resource::Salt,
        Resource::Bone,
        Resource::Vitral,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Resource::Paper => "Papel",
            Resource::Leather => "Cuero",
            Resource::Fiber => "Fibra",
            Resource::Ink => "Tinta",
            Resource::Oil => "Aceite",
            Resource::Stone => "Piedra",
            Resource::Iron => "Hierro",
            Resource::Brass => "Latón",
            Resource::Silver => "Plata",
            Resource::Crystal => "Cristal",
            Resource::Ash => "Ceniza",
            Resource::Salt => "Sal",
            Resource::Bone => "Hueso",
            Resource::Vitral => "Vitral",
        }
    }

    pub fn color(self) -> [u8; 3] {
        match self {
            Resource::Paper => [246, 232, 196],
            Resource::Leather => [176, 108, 62],
            Resource::Fiber => [126, 206, 88],
            Resource::Ink => [92, 72, 180],
            Resource::Oil => [232, 176, 52],
            Resource::Stone => [150, 156, 178],
            Resource::Iron => [120, 128, 142],
            Resource::Brass => [222, 160, 70],
            Resource::Silver => [200, 224, 236],
            Resource::Crystal => [110, 226, 236],
            Resource::Ash => [128, 112, 128],
            Resource::Salt => [250, 214, 226],
            Resource::Bone => [236, 228, 208],
            Resource::Vitral => [240, 110, 180],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolKind {
    Knife,
    Hammer,
    Lamp,
    Lens,
}

impl ToolKind {
    pub const ALL: [ToolKind; 4] = [
        ToolKind::Knife,
        ToolKind::Hammer,
        ToolKind::Lamp,
        ToolKind::Lens,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ToolKind::Knife => "Cuchilla",
            ToolKind::Hammer => "Martillo",
            ToolKind::Lamp => "Lámpara",
            ToolKind::Lens => "Lente",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

pub const MAX_TIER: u8 = 5;

pub fn tier_name(tier: u8) -> &'static str {
    match tier {
        0 => "Rústico",
        1 => "de Hierro",
        2 => "de Latón",
        3 => "de Plata",
        4 => "de Cristal",
        _ => "Áureo",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Shelf,
    SealedCodex,
    Vine,
    GoldVine,
    Rock,
    Railing,
    BrokenLamp,
    InkWell,
    AshPile,
    SaltCrust,
    BonePile,
    BrassVein,
    SilverVein,
    Mirror,
    VitralShard,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeDef {
    pub kind: NodeKind,
    pub name: String,
    pub tool: ToolKind,
    pub min_tier: u8,
    pub hits: u32,
    /// `(resource, min, max)` rolled from the node id, so the same node always yields the same.
    pub yields: Vec<(Resource, u32, u32)>,
    pub sprite: String,
    pub xp: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slot {
    Weapon,
    Armor,
    Boots,
    Lamp,
    Charm,
}

impl Slot {
    pub const ALL: [Slot; 5] = [
        Slot::Weapon,
        Slot::Armor,
        Slot::Boots,
        Slot::Lamp,
        Slot::Charm,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Weapon => "Arma",
            Slot::Armor => "Abrigo",
            Slot::Boots => "Calzado",
            Slot::Lamp => "Farol",
            Slot::Charm => "Amuleto",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Slot> {
        match tag {
            "weapon" => Some(Slot::Weapon),
            "armor" => Some(Slot::Armor),
            "boots" => Some(Slot::Boots),
            "lamp" => Some(Slot::Lamp),
            "charm" => Some(Slot::Charm),
            _ => None,
        }
    }
}

/// Every numeric bonus an item, inscription, skill or relic can grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Stat {
    Damage,
    Defense,
    MaxHp,
    MaxEnergy,
    EnergyRegen,
    HpRegen,
    MoveSpeed,
    LightRadius,
    BurnChance,
    PoisonChance,
    SlowChance,
    Lifesteal,
    Knockback,
    Reach,
    Magnet,
    XpBonus,
    Yield,
    HarvestPower,
    EchoChance,
    DarkDamage,
    DashCooldown,
    IFrames,
    ComboDamage,
    InscribeDiscount,
    CraftDiscount,
    LampDiscount,
    ReadBonus,
    Size,
}

impl Stat {
    pub fn label(self) -> &'static str {
        match self {
            Stat::Damage => "Daño",
            Stat::Defense => "Defensa",
            Stat::MaxHp => "Vida máx.",
            Stat::MaxEnergy => "Energía máx.",
            Stat::EnergyRegen => "Regen. energía",
            Stat::HpRegen => "Regen. vida",
            Stat::MoveSpeed => "Velocidad",
            Stat::LightRadius => "Radio de luz",
            Stat::BurnChance => "Quemadura",
            Stat::PoisonChance => "Veneno",
            Stat::SlowChance => "Escarcha",
            Stat::Lifesteal => "Robo de vida",
            Stat::Knockback => "Empuje",
            Stat::Reach => "Alcance",
            Stat::Magnet => "Imán",
            Stat::XpBonus => "Experiencia",
            Stat::Yield => "Rendimiento",
            Stat::HarvestPower => "Recolección",
            Stat::EchoChance => "Golpe doble",
            Stat::DarkDamage => "Daño en sombra",
            Stat::DashCooldown => "Recarga de esquiva",
            Stat::IFrames => "Invulnerabilidad",
            Stat::ComboDamage => "Daño de combo",
            Stat::InscribeDiscount => "Inscripción barata",
            Stat::CraftDiscount => "Fabricación barata",
            Stat::LampDiscount => "Lámparas baratas",
            Stat::ReadBonus => "Saber de lectura",
            Stat::Size => "Tamaño del golpe",
        }
    }

    /// Whether values are fractions shown as percentages.
    pub fn is_percent(self) -> bool {
        !matches!(
            self,
            Stat::Damage | Stat::Defense | Stat::MaxHp | Stat::MaxEnergy | Stat::HpRegen
        )
    }
}

/// Word effect granted by a tag: `stat += amount * rarity multiplier`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectDef {
    pub tag: String,
    pub name: String,
    pub stat: Stat,
    pub amount: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotDef {
    pub slot: Slot,
    pub base_noun: String,
    pub base: Vec<(Stat, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Consumable {
    Torch,
    Tonic,
    PageAmmo,
    Index,
}

impl Consumable {
    pub const ALL: [Consumable; 4] = [
        Consumable::Torch,
        Consumable::Tonic,
        Consumable::PageAmmo,
        Consumable::Index,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Consumable::Torch => "Antorcha",
            Consumable::Tonic => "Tónico de tinta",
            Consumable::PageAmmo => "Páginas afiladas",
            Consumable::Index => "Índice",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelicDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub stats: Vec<(Stat, f32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemsFile {
    pub nodes: Vec<NodeDef>,
    pub effects: Vec<EffectDef>,
    pub slots: Vec<SlotDef>,
    pub relics: Vec<RelicDef>,
    /// Light radius in tiles for lamp tiers 0..=5.
    pub lamp_radius: Vec<f32>,
    /// Harvest damage per swing for tool tiers 0..=5.
    pub tool_power: Vec<u32>,
    /// Materials the hammer adds to each thing it breaks, by its tier.
    pub hammer_extra: Vec<u32>,
    /// Damage each whetstone adds for good.
    pub whetstone_damage: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Station {
    Binder,
    Forge,
    Alembic,
    Scriptorium,
    Chest,
}

impl Station {
    pub const ALL: [Station; 5] = [
        Station::Binder,
        Station::Forge,
        Station::Alembic,
        Station::Scriptorium,
        Station::Chest,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Station::Binder => "Mesa de encuadernador",
            Station::Forge => "Forja",
            Station::Alembic => "Alambique de tinta",
            Station::Scriptorium => "Scriptorium",
            Station::Chest => "Cofre",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipeOutput {
    Tool(ToolKind, u8),
    /// The whetstone of this level, made in order like the tools.
    Whetstone(u8),
    Item(Consumable, u32),
    Material(Resource, u32),
    Build(Station),
    /// Not crafted at a station: paid on the spot to light a return lamp.
    ReturnLamp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub station: Option<Station>,
    pub cost: Vec<(Resource, u32)>,
    pub output: RecipeOutput,
    #[serde(default)]
    pub xp: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Branch {
    Reader,
    Explorer,
    Artisan,
    Fighter,
}

impl Branch {
    pub const ALL: [Branch; 4] = [
        Branch::Reader,
        Branch::Explorer,
        Branch::Artisan,
        Branch::Fighter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Branch::Reader => "Lector",
            Branch::Explorer => "Explorador",
            Branch::Artisan => "Artesano",
            Branch::Fighter => "Combatiente",
        }
    }
}

/// Something only a station improvement can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Perk {
    /// Extra materials from what the hammer breaks, within its limit.
    HammerExtra,
    /// How much faster the hammer breaks things.
    HammerPower,
    /// Added to the share of damage a thrown page deals.
    PageDamage,
    /// Chance of a thrown page setting fire.
    PageBurn,
    /// Energy a thrown page no longer costs.
    PageThrift,
    /// How much farther torches light.
    TorchReach,
    /// Health per second beside a torch.
    TorchWarmth,
    /// Damage per second to enemies beside a torch.
    TorchFire,
    /// Added to the share of health a tonic gives back.
    TonicHeal,
    /// Energy a tonic also gives.
    TonicSpirit,
    /// More tonics from each batch.
    TonicBatch,
    /// More ink from each distilling.
    InkBatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Stat(Stat),
    Perk(Perk),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradePrice {
    pub station: Station,
    /// What levels I, II and III cost.
    pub levels: Vec<Vec<(Resource, u32)>>,
}

/// One thing a station can improve.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradeTarget {
    pub id: String,
    pub name: String,
    pub station: Station,
    /// The tool it is, which has to be good enough for each level.
    #[serde(default)]
    pub tool: Option<ToolKind>,
    /// A skill without which the thing cannot be used.
    #[serde(default)]
    pub skill: Option<Unlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradeDef {
    pub id: String,
    pub target: String,
    pub name: String,
    pub text: String,
    pub effect: Effect,
    pub values: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpgradeFile {
    pub prices: Vec<UpgradePrice>,
    /// Tool tier asked for by levels I, II and III.
    pub tool_tiers: [u8; 3],
    pub targets: Vec<UpgradeTarget>,
    pub upgrades: Vec<UpgradeDef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Unlock {
    Dash,
    ChargedStrike,
    Torches,
    WideMagnet,
    DoubleInscription,
    RangedPages,
    ExtraPage,
    PhraseInsight,
    Compass,
    DeepHarvest,
    SecondWind,
    FourthSlash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub branch: Branch,
    /// Row inside the branch column, for layout.
    pub row: u8,
    pub col: u8,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub stats: Vec<(Stat, f32)>,
    #[serde(default)]
    pub unlock: Option<Unlock>,
}

/// The branches of the pet's tree: fighting, gathering and company.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PetBranch {
    Fang,
    Nose,
    Bond,
}

impl PetBranch {
    pub const ALL: [PetBranch; 3] = [PetBranch::Fang, PetBranch::Nose, PetBranch::Bond];

    pub fn name(self) -> &'static str {
        match self {
            PetBranch::Fang => "Colmillo",
            PetBranch::Nose => "Olfato",
            PetBranch::Bond => "Vínculo",
        }
    }
}

/// Something only the pet can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PetPerk {
    Bite,
    BiteHaste,
    Pounce,
    Fetch,
    Dig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PetSkillDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub branch: PetBranch,
    /// Row inside the branch column, for layout.
    pub row: u8,
    #[serde(default)]
    pub requires: Vec<String>,
    /// Bonuses for the owner.
    #[serde(default)]
    pub stats: Vec<(Stat, f32)>,
    #[serde(default)]
    pub perks: Vec<(PetPerk, f32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CanonDef {
    pub text: String,
    pub name: String,
    pub slot: Slot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hazard {
    None,
    Embers,
    Water,
    Ice,
    Vines,
    Brine,
    Dunes,
    Spikes,
    Mirrors,
    Gloom,
    Bones,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiomeDef {
    pub id: String,
    pub name: String,
    pub floor: [String; 3],
    pub wall_top: String,
    pub wall_front: String,
    pub trim: String,
    pub spines: Vec<String>,
    pub hazard: Hazard,
    /// Fraction of floor tiles covered by the hazard patches.
    pub hazard_density: f32,
    pub light_tint: [f32; 3],
    /// Weighted extra resource nodes.
    pub nodes: Vec<(NodeKind, u32)>,
    pub creatures: Vec<String>,
    pub decor: Vec<String>,
    /// Root note (semitones from A2) and scale for the ambient music.
    pub music_root: i32,
    pub music_minor: bool,
    /// Lexicon word of the guaranteed item for first discovering this biome.
    pub milestone_word: String,
    pub milestone_slot: Slot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Behavior {
    Erratic,
    Charger,
    Thrower,
    Burner,
    Ambusher,
    Tank,
    Phantom,
    Swarm,
    Patroller,
    Beast,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchetypeDef {
    pub id: String,
    pub name: String,
    pub behavior: Behavior,
    pub sprite: String,
    /// First floor where biome defaults may spawn it (words ignore this).
    #[serde(default)]
    pub min_floor: u32,
    pub hp: f32,
    pub damage: f32,
    /// Pixels per second.
    pub speed: f32,
    pub xp: u32,
    pub radius: f32,
    pub material: Resource,
    pub material_chance: f32,
    pub word_drop_chance: f32,
    pub windup: f32,
    #[serde(default)]
    pub glows: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModFlag {
    BurnTrail,
    BurnOnHit,
    DarkOnly,
    PoisonOnHit,
    SlowOnHit,
    SplitOnDeath,
    Armored,
    Glows,
    Lifesteal,
    Regenerates,
    Thorns,
    Explodes,
    PhaseWalls,
    Shock,
    Enrages,
    ExtraLoot,
    Dodges,
    HeavyKnockback,
    DrainsEnergy,
    FirstHitShield,
    Calm,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModifierDef {
    /// Matches the `mod:<id>` lexicon tag.
    pub id: String,
    pub name: String,
    pub hp: f32,
    pub damage: f32,
    pub speed: f32,
    pub size: f32,
    pub xp: f32,
    pub tint: [u8; 3],
    pub flags: Vec<ModFlag>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BossAttack {
    Charge,
    Radial(u8),
    Aimed(u8),
    Slam(u8),
    Summon(u8),
    FireWave,
    Blink,
    Spiral(u8),
    Darkness,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaseDef {
    /// Phase starts when remaining health fraction drops to this value.
    pub below: f32,
    pub attacks: Vec<BossAttack>,
    pub cooldown: f32,
    #[serde(default)]
    pub taunt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuardianDef {
    pub floor: u32,
    pub name: String,
    pub intro: String,
    pub sprite: String,
    pub hp: f32,
    pub damage: f32,
    pub speed: f32,
    pub radius: f32,
    pub minion: String,
    pub relic: String,
    pub phases: Vec<PhaseDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveDef {
    /// Per-floor linear growth of enemy health, damage and experience.
    pub hp_per_floor: f32,
    pub damage_per_floor: f32,
    pub xp_per_floor: f32,
    /// Enemies in a normal site: `base + floor / divisor`, capped.
    pub base_count: u32,
    pub count_divisor: u32,
    pub max_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemiesFile {
    pub curve: CurveDef,
    pub archetypes: Vec<ArchetypeDef>,
    pub modifiers: Vec<ModifierDef>,
    pub guardians: Vec<GuardianDef>,
}
