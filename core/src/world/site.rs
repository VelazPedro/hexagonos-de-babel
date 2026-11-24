//! Real-time deterministic generation of one playable site.

use super::floors;
use super::grid::{self, Grid, Pos, Tile, entry_pos};
use super::hex::{self, Dir};
use super::layout::{self, LayoutKind};
use super::site_recipe::{self, SiteRecipe};
use super::validation::{Reach, flood};
use crate::babel::body_gen;
use crate::babel::detect::{Analysis, Book, Hit, Place};
use crate::babel::feistel::Feistel;
use crate::babel::inscription;
use crate::babel::lexicon::{Category, WordId};
use crate::babel::rarity::Rarity;
use crate::babel::seeds::{self, Dice, Seed};
use crate::data::{GameData, Hazard, NodeKind, Resource};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SiteKey {
    Walk {
        floor: u32,
        q: i32,
        r: i32,
    },
    /// A Catalog destination outside the walkable library.
    Remote {
        address: Vec<u8>,
        difficulty: u32,
    },
}

impl SiteKey {
    pub fn walk(floor: u32, q: i32, r: i32) -> SiteKey {
        SiteKey::Walk { floor, q, r }
    }

    /// Stable text id used as the key of saved deltas.
    pub fn id(&self) -> String {
        match self {
            SiteKey::Walk { floor, q, r } => format!("w:{floor}:{q}:{r}"),
            SiteKey::Remote { address, .. } => {
                let hash = blake3::hash(address);
                format!("r:{}", &hash.to_hex()[..24])
            }
        }
    }

    /// Floor used for difficulty and word legibility.
    pub fn depth(&self) -> u32 {
        match self {
            SiteKey::Walk { floor, .. } => (*floor).max(1),
            SiteKey::Remote { difficulty, .. } => (*difficulty).max(1),
        }
    }

    pub fn is_home(&self) -> bool {
        matches!(self, SiteKey::Walk { floor, q, r } if floors::is_home(*floor, *q, *r))
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, SiteKey::Remote { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeSpawn {
    pub id: u32,
    pub kind: NodeKind,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Elite {
    pub name: String,
    pub noun: WordId,
    pub quality: WordId,
    pub rarity: Rarity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemySpawn {
    pub id: u32,
    pub archetype: usize,
    /// Creature word that summoned it, if any.
    pub word: Option<(WordId, Rarity)>,
    pub modifiers: Vec<usize>,
    pub elite: Option<Elite>,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ChestReward {
    Canon {
        canon: usize,
        rarity: Rarity,
    },
    Compound {
        noun: WordId,
        quality: WordId,
        rarity: Rarity,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChestSpawn {
    pub id: u32,
    pub pos: Pos,
    pub reward: ChestReward,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decor {
    pub pos: Pos,
    pub kind: u8,
    pub variant: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuardianSpawn {
    pub index: usize,
    pub words: Vec<(WordId, Rarity)>,
    pub modifiers: Vec<usize>,
    pub pos: Pos,
}

/// Building plots of the home hexagon.
pub const HOME_PLOTS: [Pos; 5] = [
    Pos::new(14, 15),
    Pos::new(17, 12),
    Pos::new(23, 12),
    Pos::new(26, 15),
    Pos::new(25, 21),
];
pub const HOME_LAMP: Pos = Pos::new(20, 15);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub key: SiteKey,
    pub depth: u32,
    pub seed: Seed,
    pub book: Book,
    pub analysis: Analysis,
    pub recipe: SiteRecipe,
    pub layout: LayoutKind,
    pub grid: Grid,
    pub hub: Pos,
    /// Doorways the site opens on its own.
    pub exits: [bool; 6],
    pub nodes: Vec<NodeSpawn>,
    pub enemies: Vec<EnemySpawn>,
    pub lamps: Vec<Pos>,
    pub chests: Vec<ChestSpawn>,
    pub decor: Vec<Decor>,
    pub stairs: Option<Pos>,
    pub guardian: Option<GuardianSpawn>,
}

fn address_bytes(a: &BigUint) -> Vec<u8> {
    a.to_bytes_le()
}

/// Key of the site whose inscription is `phrase`: walkable if the address happens
/// to be one, otherwise a remote site of the given difficulty.
pub fn key_for_phrase(feistel: &Feistel, phrase: &str, difficulty: u32) -> Option<SiteKey> {
    let address = inscription::address_of(feistel, phrase)?;
    Some(match inscription::unpack(&address) {
        Some((floor, q, r)) => SiteKey::Walk { floor, q, r },
        None => SiteKey::Remote {
            address: address_bytes(&address),
            difficulty,
        },
    })
}

/// Generates the book of a site: bijective inscription plus procedural body.
pub fn book_for(data: &GameData, feistel: &Feistel, seed: &Seed, key: &SiteKey) -> Book {
    let address = match key {
        SiteKey::Walk { floor, q, r } => inscription::pack(*floor, *q, *r),
        SiteKey::Remote { address, .. } => BigUint::from_bytes_le(address),
    };
    Book {
        inscription: inscription::inscription_for(feistel, &address),
        pages: body_gen::generate(
            seed,
            key.depth(),
            &data.lexicon,
            &data.sampler,
            &data.canon_texts,
        ),
    }
}

pub fn seed_for(world_seed: u64, key: &SiteKey) -> Seed {
    match key {
        SiteKey::Walk { floor, q, r } => seeds::site_seed(world_seed, *floor, *q, *r),
        SiteKey::Remote { address, .. } => seeds::remote_seed(world_seed, address),
    }
}

/// Catalog destinations never yield more than Epic.
fn cap_rarity(analysis: &mut Analysis) {
    for h in &mut analysis.hits {
        h.rarity = h.rarity.min(Rarity::Epic);
    }
    for p in &mut analysis.phrases {
        p.rarity = p.rarity.min(Rarity::Epic);
    }
}

fn step_toward(from: (i32, i32), to: (i32, i32)) -> Option<Dir> {
    let current = hex::distance(from, to);
    Dir::ALL
        .into_iter()
        .find(|&d| hex::distance(hex::neighbor(from.0, from.1, d), to) < current)
}

/// Directions that strictly advance through the lattice.
const DRIFT: [Dir; 3] = [Dir::NE, Dir::SE, Dir::S];

/// Doorways owned by the site. One always leads toward the nearest lattice
/// staircase, so stairs can never be walled off.
fn own_exits(world_seed: u64, key: &SiteKey, recipe: &SiteRecipe, seed: &Seed) -> [bool; 6] {
    let SiteKey::Walk { floor, q, r } = *key else {
        return [false; 6];
    };
    if floors::is_home(floor, q, r) {
        return [true; 6];
    }
    let mut dice = Dice::labelled(seed, "exits");
    let mut exits = recipe.forced_exits;
    let stairs = floors::nearest_stairs(world_seed, floor, q, r);
    if let Some(d) = step_toward((q, r), stairs) {
        exits[d.index()] = true;
    }
    // The drift directions all increase `2q + r`, so following them never loops:
    // every site reaches infinitely many others and no cluster can be sealed.
    let drift = DRIFT[dice.index(DRIFT.len())];
    exits[drift.index()] = true;
    let target = if recipe.has_orientation {
        dice.range(2, 4) as usize
    } else {
        2
    };
    let mut rest: Vec<Dir> = Dir::ALL.into_iter().filter(|d| !exits[d.index()]).collect();
    dice.shuffle(&mut rest);
    while exits.iter().filter(|&&e| e).count() < target {
        match rest.pop() {
            Some(d) => exits[d.index()] = true,
            None => break,
        }
    }
    exits
}

fn paint_hazards(
    dice: &mut Dice,
    grid: &mut Grid,
    density: f32,
    reach: &Reach,
    keep_clear: &[Pos],
) {
    let candidates: Vec<Pos> = grid
        .positions()
        .filter(|&p| grid.at(p) == Tile::Floor && reach.contains(p))
        .filter(|&p| keep_clear.iter().all(|&k| p.dist2(k) > 12))
        .collect();
    if candidates.is_empty() {
        return;
    }
    let wanted = (candidates.len() as f32 * density) as usize;
    let mut painted = 0;
    let mut attempts = 0;
    while painted < wanted && attempts < wanted * 3 + 8 {
        attempts += 1;
        let Some(&start) = dice.pick(&candidates) else {
            break;
        };
        let mut p = start;
        for _ in 0..dice.range(4, 14) {
            if grid.at(p) == Tile::Floor && keep_clear.iter().all(|&k| p.dist2(k) > 12) {
                grid.set(p.x, p.y, Tile::Hazard);
                painted += 1;
            }
            match dice.below(4) {
                0 => p.x += 1,
                1 => p.x -= 1,
                2 => p.y += 1,
                _ => p.y -= 1,
            }
        }
    }
}

struct Placer {
    /// Shuffled reachable tiles.
    pool: Vec<Pos>,
    taken: Vec<Pos>,
}

impl Placer {
    fn free(&self, p: Pos, spacing2: i32) -> bool {
        self.taken.iter().all(|&t| p.dist2(t) > spacing2)
    }

    /// First pooled tile satisfying `accept` and spaced from everything placed.
    fn take(&mut self, spacing2: i32, accept: impl Fn(Pos) -> bool) -> Option<Pos> {
        let found = self
            .pool
            .iter()
            .position(|&p| self.free(p, spacing2) && accept(p))?;
        let p = self.pool.swap_remove(found);
        self.taken.push(p);
        Some(p)
    }
}

fn mandatory_nodes(depth: u32) -> Vec<NodeKind> {
    let mut nodes = vec![
        NodeKind::Shelf,
        NodeKind::Shelf,
        NodeKind::Rock,
        NodeKind::Railing,
        NodeKind::BrokenLamp,
        NodeKind::Vine,
    ];
    // Depth guarantees: each tier's material appears in every site from its floor on.
    if depth >= 2 {
        nodes.push(NodeKind::InkWell);
    }
    if depth >= 3 {
        nodes.push(NodeKind::BrassVein);
    }
    if depth >= 6 {
        nodes.push(NodeKind::AshPile);
    }
    if depth >= 8 {
        nodes.push(NodeKind::SilverVein);
    }
    if depth >= 10 {
        nodes.push(NodeKind::SaltCrust);
    }
    if depth >= 13 {
        nodes.push(NodeKind::Mirror);
    }
    if depth >= 15 {
        nodes.push(NodeKind::BonePile);
    }
    if depth >= 18 {
        nodes.push(NodeKind::VitralShard);
    }
    nodes
}

/// Node that provides a material mentioned in the text.
fn node_for_material(r: Resource) -> NodeKind {
    match r {
        Resource::Paper | Resource::Leather => NodeKind::Shelf,
        Resource::Fiber => NodeKind::Vine,
        Resource::Ink => NodeKind::InkWell,
        Resource::Oil => NodeKind::BrokenLamp,
        Resource::Stone => NodeKind::Rock,
        Resource::Iron => NodeKind::Railing,
        Resource::Brass => NodeKind::BrassVein,
        Resource::Silver => NodeKind::SilverVein,
        Resource::Crystal => NodeKind::Mirror,
        Resource::Ash => NodeKind::AshPile,
        Resource::Salt => NodeKind::SaltCrust,
        Resource::Bone => NodeKind::BonePile,
        Resource::Vitral => NodeKind::VitralShard,
    }
}

/// Modifier granted by a quality word: its `mod:` tag, or a stable fallback.
pub fn modifier_of_word(data: &GameData, word: WordId) -> usize {
    let entry = data.lexicon.get(word);
    entry
        .tag_value("mod")
        .and_then(|m| data.modifier_index(m))
        .unwrap_or_else(|| (entry.variant_hash() % data.enemies.modifiers.len() as u64) as usize)
}

fn near(a: &Hit, b: &Hit) -> bool {
    let col = (i32::from(a.start) - i32::from(b.start)).abs();
    match (a.place, b.place) {
        (Place::Inscription, Place::Inscription) => true,
        (Place::Body { page: p1, line: l1 }, Place::Body { page: p2, line: l2 }) => {
            p1 == p2 && ((l1 == l2 && col <= 40) || (l1.abs_diff(l2) == 1 && col <= 20))
        }
        _ => false,
    }
}

/// Modifiers from quality words close to a creature word.
fn nearby_modifiers(data: &GameData, analysis: &Analysis, creature: &Hit) -> Vec<usize> {
    let mut out = Vec::new();
    for h in &analysis.hits {
        let entry = data.lexicon.get(h.word);
        if entry.category != Category::Cualidad || !near(creature, h) {
            continue;
        }
        if let Some(m) = entry.tag_value("mod").and_then(|m| data.modifier_index(m))
            && !out.contains(&m)
            && out.len() < 2
        {
            out.push(m);
        }
    }
    out
}

struct PlannedEnemy {
    archetype: usize,
    word: Option<(WordId, Rarity)>,
    modifiers: Vec<usize>,
    elite: Option<Elite>,
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn plan_enemies(
    data: &GameData,
    analysis: &Analysis,
    recipe: &SiteRecipe,
    depth: u32,
    dice: &mut Dice,
) -> Vec<PlannedEnemy> {
    let curve = &data.enemies.curve;
    let mut count = (curve.base_count + depth / curve.count_divisor.max(1)).min(curve.max_count);
    count = (count as i32 + dice.range(-1, 1)).max(1) as u32;
    if depth <= 1 {
        count = count.min(3);
    }
    if recipe.calm {
        count = dice.range(0, 2) as u32;
    }
    let mut planned = Vec::new();

    // Phrases naming a creature become elites with their own name.
    for phrase in &analysis.phrases {
        let entry = data.lexicon.get(phrase.noun);
        if entry.category != Category::Criatura || planned.len() >= 2 {
            continue;
        }
        let archetype = data.archetype_index(entry.tag_value("arq").unwrap_or(""));
        planned.push(PlannedEnemy {
            archetype,
            word: Some((phrase.noun, phrase.rarity)),
            modifiers: vec![modifier_of_word(data, phrase.quality)],
            elite: Some(Elite {
                name: capitalize(&phrase.text),
                noun: phrase.noun,
                quality: phrase.quality,
                rarity: phrase.rarity,
            }),
        });
    }

    let cap = if recipe.calm {
        count as usize
    } else {
        (count as usize + 3).min(curve.max_count as usize + 2)
    };
    for hit in &analysis.hits {
        if planned.len() >= cap {
            break;
        }
        let entry = data.lexicon.get(hit.word);
        if entry.category != Category::Criatura || hit.in_phrase {
            continue;
        }
        let archetype = data.archetype_index(entry.tag_value("arq").unwrap_or(""));
        let def = &data.enemies.archetypes[archetype];
        let group = match def.behavior {
            crate::data::Behavior::Swarm | crate::data::Behavior::Erratic => 2,
            _ => 1,
        };
        let modifiers = nearby_modifiers(data, analysis, hit);
        for _ in 0..group {
            if planned.len() < cap {
                planned.push(PlannedEnemy {
                    archetype,
                    word: Some((hit.word, hit.rarity)),
                    modifiers: modifiers.clone(),
                    elite: None,
                });
            }
        }
    }

    let biome = &data.biomes[recipe.biome];
    let mut defaults: Vec<usize> = biome
        .creatures
        .iter()
        .map(|c| data.archetype_index(c))
        .filter(|&a| data.enemies.archetypes[a].min_floor <= depth)
        .collect();
    if defaults.is_empty() {
        defaults = vec![
            data.archetype_index("libro"),
            data.archetype_index("polilla"),
        ];
    }
    while planned.len() < count as usize {
        let archetype = defaults[dice.index(defaults.len())];
        planned.push(PlannedEnemy {
            archetype,
            word: None,
            modifiers: Vec::new(),
            elite: None,
        });
    }
    planned
}

fn chest_rewards(data: &GameData, analysis: &Analysis, remote: bool) -> Vec<ChestReward> {
    let mut rewards = Vec::new();
    let mut seen = Vec::new();
    for c in &analysis.canons {
        if !seen.contains(&c.canon) && seen.len() < 2 {
            seen.push(c.canon);
            rewards.push(ChestReward::Canon {
                canon: c.canon,
                rarity: if remote {
                    Rarity::Epic
                } else {
                    Rarity::Canonical
                },
            });
        }
    }
    if let Some(p) = analysis
        .phrases
        .iter()
        .find(|p| data.lexicon.get(p.noun).category == Category::Objeto)
    {
        rewards.push(ChestReward::Compound {
            noun: p.noun,
            quality: p.quality,
            rarity: p.rarity,
        });
    }
    rewards
}

/// Generates the site at `key`. Pure function of `(world_seed, key)`.
pub fn generate(data: &GameData, world_seed: u64, key: &SiteKey) -> Site {
    let feistel = Feistel::new(world_seed);
    generate_with(data, &feistel, world_seed, key)
}

pub fn generate_with(data: &GameData, feistel: &Feistel, world_seed: u64, key: &SiteKey) -> Site {
    let seed = seed_for(world_seed, key);
    let depth = key.depth();
    let book = book_for(data, feistel, &seed, key);
    let mut analysis = data.detector.analyze(&data.lexicon, &book, depth);
    if key.is_remote() {
        cap_rarity(&mut analysis);
    }
    let home = key.is_home();
    let mut recipe = site_recipe::build(data, &analysis, depth);
    if home {
        recipe.biome = 0;
        recipe.ambient = 0.62;
        recipe.tint = [1.0, 0.92, 0.80];
        recipe.layout = LayoutKind::Home;
        recipe.calm = true;
        recipe.stairs_word = false;
        recipe.lamp_count = 4;
    }

    let stairs_here = match *key {
        SiteKey::Walk { floor, q, r } => {
            !home && (recipe.stairs_word || floors::is_stair_site(world_seed, floor, q, r))
        }
        SiteKey::Remote { .. } => false,
    };
    let guardian_here = stairs_here && floors::is_guardian_floor(depth) && !key.is_remote();
    let kind = if guardian_here {
        LayoutKind::Arena
    } else {
        recipe.layout
    };
    let exits = own_exits(world_seed, key, &recipe, &seed);
    let built = layout::build(&seed, kind, &exits);
    let mut grid = built.grid;
    let hub = built.hub;

    let mut keep_clear: Vec<Pos> = Dir::ALL.map(entry_pos).to_vec();
    keep_clear.push(hub);
    let biome = &data.biomes[recipe.biome];
    if !home && biome.hazard != Hazard::None {
        let reach = flood(&grid, hub, &[]);
        let mut dice = Dice::labelled(&seed, "hazards");
        paint_hazards(
            &mut dice,
            &mut grid,
            biome.hazard_density,
            &reach,
            &keep_clear,
        );
    }

    let reach = flood(&grid, hub, &[]);
    let mut decor_dice = Dice::labelled(&seed, "decor");
    let mut loot_dice = Dice::labelled(&seed, "loot");
    let mut pool: Vec<Pos> = grid
        .positions()
        .filter(|&p| reach.contains(p) && grid.at(p) != Tile::Door)
        .collect();
    loot_dice.shuffle(&mut pool);
    let mut placer = Placer {
        pool,
        taken: vec![hub],
    };
    if home {
        placer.taken.extend(HOME_PLOTS);
        placer.taken.push(HOME_LAMP);
    }
    let far_from_landings = |p: Pos, d2: i32| keep_clear.iter().all(|&k| p.dist2(k) >= d2);
    let solid_ok = |grid: &Grid, p: Pos| {
        grid.open_area(p) && grid.at(p) == Tile::Floor && far_from_landings(p, 10)
    };

    let stairs = if stairs_here {
        placer
            .take(8, |p| {
                solid_ok(&grid, p) && p.dist2(Pos::new(20, 17)) <= 100
            })
            .or_else(|| placer.take(2, |p| solid_ok(&grid, p)))
            .or_else(|| placer.take(2, |p| grid.at(p) == Tile::Floor))
    } else {
        None
    };

    let guardian = match stairs {
        Some(_) if guardian_here => {
            let words = site_recipe::rarest_words(data, &analysis, 4);
            let mut modifiers = Vec::new();
            for &(w, _) in &words {
                let entry = data.lexicon.get(w);
                if let Some(m) = entry.tag_value("mod").and_then(|m| data.modifier_index(m))
                    && !modifiers.contains(&m)
                    && modifiers.len() < 2
                {
                    modifiers.push(m);
                }
            }
            let index =
                ((depth / floors::GUARDIAN_EVERY) as usize - 1) % data.enemies.guardians.len();
            let pos = placer
                .take(6, |p| solid_ok(&grid, p) && p.dist2(hub) <= 40)
                .unwrap_or(hub);
            Some(GuardianSpawn {
                index,
                words,
                modifiers,
                pos,
            })
        }
        _ => None,
    };

    let mut chests = Vec::new();
    for (i, reward) in chest_rewards(data, &analysis, key.is_remote())
        .into_iter()
        .enumerate()
    {
        if let Some(pos) = placer.take(6, |p| solid_ok(&grid, p)) {
            chests.push(ChestSpawn {
                id: i as u32,
                pos,
                reward,
            });
        }
    }

    // Lamp fixtures prefer to hug a wall.
    let mut lamps = Vec::new();
    if home {
        lamps.extend([
            Pos::new(12, 9),
            Pos::new(28, 9),
            Pos::new(31, 17),
            Pos::new(17, 26),
            Pos::new(9, 17),
        ]);
    } else {
        for _ in 0..recipe.lamp_count {
            let hugging = |p: Pos| {
                grid.at(p) == Tile::Floor
                    && far_from_landings(p, 5)
                    && grid.get(p.x, p.y - 1).opaque()
            };
            let pos = placer
                .take(48, hugging)
                .or_else(|| placer.take(30, |p| solid_ok(&grid, p)));
            if let Some(p) = pos {
                lamps.push(p);
            }
        }
    }

    let mut kinds = if home {
        vec![
            NodeKind::Shelf,
            NodeKind::Shelf,
            NodeKind::Shelf,
            NodeKind::Shelf,
            NodeKind::Rock,
            NodeKind::Rock,
            NodeKind::Rock,
            NodeKind::Railing,
            NodeKind::Railing,
            NodeKind::BrokenLamp,
            NodeKind::Vine,
            NodeKind::Vine,
        ]
    } else {
        let mut kinds = mandatory_nodes(depth);
        let weights: Vec<u32> = biome.nodes.iter().map(|n| n.1).collect();
        for _ in 0..loot_dice.range(3, 7) {
            if let Some(i) = loot_dice.weighted(&weights) {
                kinds.push(biome.nodes[i].0);
            }
        }
        for &m in &recipe.materials {
            kinds.push(node_for_material(m));
        }
        kinds
    };
    if guardian.is_some() {
        kinds.truncate(5);
    }
    let mut nodes = Vec::new();
    for (i, kind) in kinds.into_iter().enumerate() {
        // Nodes are solid, so they only go where they cannot cut a path.
        let pos = placer
            .take(5, |p| solid_ok(&grid, p))
            .or_else(|| placer.take(2, |p| grid.open_area(p) && far_from_landings(p, 5)));
        if let Some(pos) = pos {
            nodes.push(NodeSpawn {
                id: i as u32,
                kind,
                pos,
            });
        }
    }

    let mut enemies = Vec::new();
    if !home && guardian.is_none() {
        let mut dice = Dice::labelled(&seed, "enemies");
        for (i, e) in plan_enemies(data, &analysis, &recipe, depth, &mut dice)
            .into_iter()
            .enumerate()
        {
            let pos = placer
                .take(3, |p| far_from_landings(p, 36) && p.dist2(hub) >= 30)
                .or_else(|| placer.take(1, |p| far_from_landings(p, 20) && p.dist2(hub) >= 16))
                .or_else(|| placer.take(0, |p| far_from_landings(p, 10) && p.dist2(hub) >= 9));
            if let Some(pos) = pos {
                enemies.push(EnemySpawn {
                    id: i as u32,
                    archetype: e.archetype,
                    word: e.word,
                    modifiers: e.modifiers,
                    elite: e.elite,
                    pos,
                });
            }
        }
    }

    let mut decor = Vec::new();
    if !biome.decor.is_empty() {
        let floor_tiles: Vec<Pos> = grid
            .positions()
            .filter(|&p| grid.at(p) == Tile::Floor)
            .collect();
        for _ in 0..decor_dice.range(26, 48) {
            if let Some(&pos) = decor_dice.pick(&floor_tiles) {
                decor.push(Decor {
                    pos,
                    kind: decor_dice.index(biome.decor.len()) as u8,
                    variant: decor_dice.below(4) as u8,
                });
            }
        }
    }

    Site {
        key: key.clone(),
        depth,
        seed,
        book,
        analysis,
        recipe,
        layout: kind,
        grid,
        hub,
        exits,
        nodes,
        enemies,
        lamps,
        chests,
        decor,
        stairs,
        guardian,
    }
}

impl Site {
    /// Tiles occupied by solid objects.
    pub fn solids(&self) -> Vec<Pos> {
        self.nodes
            .iter()
            .map(|n| n.pos)
            .chain(self.chests.iter().map(|c| c.pos))
            .collect()
    }

    /// Flood-fill guarantees: doorways, stairs, resources and enemies are all
    /// reachable, nothing sits inside a wall and nothing spawns on a landing.
    pub fn validate(&self) -> Result<(), String> {
        let solids = self.solids();
        let reach = flood(&self.grid, self.hub, &solids);
        for d in Dir::ALL {
            if !reach.contains(entry_pos(d)) {
                return Err(format!("doorway {d:?} unreachable"));
            }
            if self.exits[d.index()] {
                let mid = grid::door_mid(d);
                let has_door = self.grid.positions().any(|p| {
                    self.grid.at(p) == Tile::Door
                        && (p.x as f32 + 0.5 - mid.0).abs() < 3.0
                        && (p.y as f32 + 0.5 - mid.1).abs() < 3.0
                        && reach.contains(p)
                });
                if !has_door {
                    return Err(format!("exit {d:?} is not open and reachable"));
                }
            }
        }
        if let Some(s) = self.stairs
            && !reach.contains(s)
        {
            return Err("stairs unreachable".into());
        }
        let adjacent = |p: Pos| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|d| reach.contains(Pos::new(p.x + d.0, p.y + d.1)))
        };
        for p in &solids {
            if !self.grid.walkable(p.x, p.y) || !adjacent(*p) {
                return Err(format!("solid object at {p:?} is unreachable"));
            }
        }
        let landing = |p: Pos| Dir::ALL.iter().any(|&d| p.dist2(entry_pos(d)) < 9);
        for e in &self.enemies {
            if !reach.contains(e.pos) {
                return Err(format!("enemy at {:?} unreachable", e.pos));
            }
            if landing(e.pos) || e.pos.dist2(self.hub) < 9 {
                return Err(format!("enemy at {:?} spawns on the player", e.pos));
            }
        }
        if let Some(g) = &self.guardian
            && !reach.contains(g.pos)
        {
            return Err("guardian unreachable".into());
        }
        Ok(())
    }
}
