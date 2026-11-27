//! What each station can improve: for every thing it works on, one choice, at
//! one of three levels. Nothing is left to luck, and all of it stays on.

use super::crafting::discounted;
use super::guide::{Guide, needs_skill};
use super::player::PlayerData;
use super::stats::{self, Derived, MAX_HAMMER_EXTRA, describe, hammer_base};
use crate::data::{
    Effect, GameData, Perk, Resource, Stat, Station, ToolKind, UpgradeDef, UpgradeTarget, tier_name,
};
use crate::game::player as combat;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_LEVEL: u8 = 3;

/// The improvement a player has on one thing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fitted {
    pub id: String,
    pub level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Nothing was on this thing yet.
    New,
    Raise,
    /// Another one takes its place and keeps the level.
    Swap,
}

/// What choosing an improvement would do right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub level: u8,
    pub change: Change,
    pub cost: Vec<(Resource, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeError {
    Unknown,
    /// Already at the last level.
    Max,
    NeedStation(Station),
    NotAtHome,
    /// The tool has to be at least this tier.
    NeedTool(ToolKind, u8),
    MissingResources,
}

impl UpgradeError {
    pub fn message(self) -> String {
        match self {
            UpgradeError::Unknown => "Esa mejora no existe".into(),
            UpgradeError::Max => "Ya está en el último nivel".into(),
            UpgradeError::NeedStation(s) => format!("Falta construir: {}", s.name()),
            UpgradeError::NotAtHome => "Solo en tu hexágono".into(),
            UpgradeError::NeedTool(kind, tier) => {
                format!("Pide {} {} o mejor", kind.name(), tier_name(tier))
            }
            UpgradeError::MissingResources => "Faltan materiales".into(),
        }
    }
}

fn find<'a>(data: &'a GameData, id: &str) -> Option<(&'a UpgradeDef, &'a UpgradeTarget)> {
    let upgrade = data.upgrades.upgrades.iter().find(|u| u.id == id)?;
    let target = data
        .upgrades
        .targets
        .iter()
        .find(|t| t.id == upgrade.target)?;
    Some((upgrade, target))
}

/// Whether what is saved under a thing is an improvement made for it.
pub(crate) fn belongs(data: &GameData, target: &str, id: &str) -> bool {
    find(data, id).is_some_and(|(u, _)| u.target == target)
}

fn value_at(upgrade: &UpgradeDef, level: u8) -> f32 {
    upgrade.values[usize::from(level.clamp(1, MAX_LEVEL) - 1)]
}

/// Everything this player has fitted, as what it does and by how much.
pub(crate) fn in_effect<'a>(
    data: &'a GameData,
    player: &'a PlayerData,
) -> impl Iterator<Item = (Effect, f32)> + 'a {
    player.upgrades.iter().filter_map(|(thing, fitted)| {
        let (upgrade, _) = find(data, &fitted.id)?;
        (upgrade.target == *thing).then(|| (upgrade.effect, value_at(upgrade, fitted.level)))
    })
}

fn cost_of(
    data: &GameData,
    derived: &Derived,
    station: Station,
    level: u8,
) -> Vec<(Resource, u32)> {
    let base = data
        .upgrades
        .prices
        .iter()
        .find(|p| p.station == station)
        .and_then(|p| p.levels.get(usize::from(level - 1)));
    discounted(
        base.map_or(&[][..], Vec::as_slice),
        derived.stat(Stat::CraftDiscount),
    )
}

/// What choosing this improvement would do, whether or not it can be paid.
pub fn offer(
    data: &GameData,
    player: &PlayerData,
    derived: &Derived,
    id: &str,
) -> Result<Offer, UpgradeError> {
    let (upgrade, target) = find(data, id).ok_or(UpgradeError::Unknown)?;
    let (level, change) = match player.upgrades.get(&target.id) {
        None => (1, Change::New),
        Some(f) if f.id == upgrade.id => {
            if f.level >= MAX_LEVEL {
                return Err(UpgradeError::Max);
            }
            (f.level.max(1) + 1, Change::Raise)
        }
        Some(f) => (f.level.clamp(1, MAX_LEVEL), Change::Swap),
    };
    Ok(Offer {
        level,
        change,
        cost: cost_of(data, derived, target.station, level),
    })
}

pub fn check(
    data: &GameData,
    player: &PlayerData,
    derived: &Derived,
    stations: &BTreeSet<Station>,
    at_home: bool,
    id: &str,
) -> Result<Offer, UpgradeError> {
    let offer = offer(data, player, derived, id)?;
    let (_, target) = find(data, id).ok_or(UpgradeError::Unknown)?;
    if !at_home {
        return Err(UpgradeError::NotAtHome);
    }
    if !stations.contains(&target.station) {
        return Err(UpgradeError::NeedStation(target.station));
    }
    if let Some(kind) = target.tool {
        let tier = data.upgrades.tool_tiers[usize::from(offer.level - 1)];
        if player.tool_tier(kind) < tier {
            return Err(UpgradeError::NeedTool(kind, tier));
        }
    }
    if !player.can_pay(&offer.cost) {
        return Err(UpgradeError::MissingResources);
    }
    Ok(offer)
}

/// Pays for an improvement and puts it on.
pub fn apply(
    data: &GameData,
    player: &mut PlayerData,
    derived: &Derived,
    stations: &BTreeSet<Station>,
    at_home: bool,
    id: &str,
) -> Result<Offer, UpgradeError> {
    let offer = check(data, player, derived, stations, at_home, id)?;
    let (_, target) = find(data, id).ok_or(UpgradeError::Unknown)?;
    if !player.pay(&offer.cost) {
        return Err(UpgradeError::MissingResources);
    }
    let fitted = Fitted {
        id: id.to_string(),
        level: offer.level,
    };
    player.upgrades.insert(target.id.clone(), fitted);
    Ok(offer)
}

/// What an improvement gives at one of its values.
fn effect_text(effect: Effect, value: f32) -> String {
    let share = (value * 100.0).round() as i32;
    let amount = if (value - value.round()).abs() < 0.05 {
        format!("{}", value.round() as i32)
    } else {
        format!("{value:.1}")
    };
    let perk = match effect {
        Effect::Stat(stat) => return describe(stat, value),
        Effect::Perk(perk) => perk,
    };
    match perk {
        Perk::HammerExtra => {
            format!("+{amount} materiales por cosa rota (tope total +{MAX_HAMMER_EXTRA})")
        }
        Perk::HammerPower => format!("+{share}% de fuerza al romper con el martillo"),
        Perk::PageDamage => format!(
            "las páginas pegan el {}% de tu daño",
            ((combat::PAGE_DAMAGE + value) * 100.0).round() as i32
        ),
        Perk::PageBurn => format!("{share}% de que una página queme"),
        Perk::PageThrift => format!(
            "cada página gasta {} de energía",
            (combat::PAGE_ENERGY - value).max(1.0).round() as i32
        ),
        Perk::TorchReach => format!("tus antorchas alumbran {share}% más lejos"),
        Perk::TorchWarmth => {
            format!("cerca de una antorcha recuperás {amount} de vida por segundo")
        }
        Perk::TorchFire => {
            format!("las antorchas hacen {amount} de daño por segundo a los enemigos cercanos")
        }
        Perk::TonicHeal => format!(
            "el tónico recupera el {}% de tu vida",
            ((combat::TONIC_HEAL + value) * 100.0).round() as i32
        ),
        Perk::TonicSpirit => format!("el tónico además da {amount} de energía"),
        Perk::TonicBatch if value < 1.5 => "cada tanda trae 1 tónico más".into(),
        Perk::TonicBatch => format!("cada tanda trae {amount} tónicos más"),
        Perk::InkBatch => format!("cada destilado da {amount} de tinta más"),
    }
}

const NUMERALS: [&str; 3] = ["I", "II", "III"];

/// A level as it is written.
pub fn numeral(level: u8) -> &'static str {
    NUMERALS[usize::from(level.clamp(1, MAX_LEVEL) - 1)]
}

/// What an improvement is and what this player would get from it.
pub fn explain(data: &GameData, player: &PlayerData, id: &str) -> Option<Guide> {
    let (upgrade, target) = find(data, id)?;
    let at = |level: u8| {
        let gives = effect_text(upgrade.effect, value_at(upgrade, level));
        // What ends in an abbreviation gets its stop from the sentence.
        format!("nivel {}: {}", numeral(level), gives.trim_end_matches('.'))
    };
    let helps = match player.upgrades.get(&target.id) {
        None => format!("Con {}.", at(1)),
        Some(f) if f.id == upgrade.id && f.level >= MAX_LEVEL => {
            format!("Ahora, {}. Es el último.", at(MAX_LEVEL))
        }
        Some(f) if f.id == upgrade.id => {
            let level = f.level.max(1);
            format!("Ahora, {}. Al subir, {}.", at(level), at(level + 1))
        }
        Some(f) => {
            let other = find(data, &f.id).map_or("la actual", |(u, _)| u.name.as_str());
            format!(
                "Reemplaza a «{other}» y conserva el nivel. Con {}.",
                at(f.level.clamp(1, MAX_LEVEL))
            )
        }
    };
    let derived = stats::derive(data, player);
    let missing = target
        .skill
        .and_then(|unlock| needs_skill(data, &derived, unlock))
        .or_else(|| {
            let full = upgrade.effect == Effect::Perk(Perk::HammerExtra)
                && hammer_base(data, player.tool_tier(ToolKind::Hammer)) >= MAX_HAMMER_EXTRA;
            full.then(|| {
                format!("Tu martillo ya da el máximo de +{MAX_HAMMER_EXTRA}: ahora no suma nada.")
            })
        });
    Some(Guide {
        does: upgrade.text.clone(),
        helps,
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_data;

    const EVERY_STATION: [Station; 3] = [Station::Forge, Station::Binder, Station::Alembic];

    /// Plenty of everything, with every tool golden.
    fn rich() -> PlayerData {
        let mut p = PlayerData {
            tools: [5; 4],
            ..PlayerData::default()
        };
        for resource in Resource::ALL {
            p.add_resource(resource, 900);
        }
        p
    }

    fn at_home(p: &mut PlayerData, id: &str) -> Result<Offer, UpgradeError> {
        let d = test_data();
        let derived = stats::derive(d, p);
        apply(d, p, &derived, &BTreeSet::from(EVERY_STATION), true, id)
    }

    fn price(station: Station, level: u8) -> Vec<(Resource, u32)> {
        let prices = &test_data().upgrades.prices;
        let at = prices.iter().find(|p| p.station == station).expect("price");
        at.levels[usize::from(level - 1)].clone()
    }

    fn fitted(p: &PlayerData, target: &str) -> Option<(String, u8)> {
        p.upgrades.get(target).map(|f| (f.id.clone(), f.level))
    }

    #[test]
    fn there_are_three_choices_for_each_thing() {
        let file = &test_data().upgrades;
        assert_eq!(file.targets.len(), 9);
        assert_eq!(file.upgrades.len(), 27);
        let ids: BTreeSet<&str> = file.upgrades.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids.len(), 27);
        for target in &file.targets {
            let own: Vec<_> = file
                .upgrades
                .iter()
                .filter(|u| u.target == target.id)
                .collect();
            assert_eq!(own.len(), 3, "{}", target.id);
            for u in own {
                assert!(u.values[0] > 0.0, "{}", u.id);
                assert!(u.values.windows(2).all(|w| w[1] >= w[0]), "{}", u.id);
                assert!(u.values[2] > u.values[0], "{}", u.id);
            }
        }
        for station in EVERY_STATION {
            for level in 1..=MAX_LEVEL {
                assert!(!price(station, level).is_empty());
            }
        }
    }

    #[test]
    fn choosing_raising_and_changing_are_paid_by_level() {
        let mut p = rich();
        let iron = p.resource(Resource::Iron);
        let first = at_home(&mut p, "filo_igneo").expect("new");
        assert_eq!((first.level, first.change), (1, Change::New));
        assert_eq!(first.cost, price(Station::Forge, 1));
        assert_eq!(p.resource(Resource::Iron), iron - 4);
        let second = at_home(&mut p, "filo_igneo").expect("raise");
        assert_eq!((second.level, second.change), (2, Change::Raise));
        assert_eq!(second.cost, price(Station::Forge, 2));
        // Changing keeps the level and costs what that level costs.
        let swap = at_home(&mut p, "filo_sediento").expect("swap");
        assert_eq!((swap.level, swap.change), (2, Change::Swap));
        assert_eq!(swap.cost, price(Station::Forge, 2));
        assert_eq!(fitted(&p, "knife"), Some(("filo_sediento".into(), 2)));
        assert_eq!(at_home(&mut p, "filo_sediento").expect("raise").level, 3);
        assert_eq!(at_home(&mut p, "filo_sediento"), Err(UpgradeError::Max));
        // Each thing has its own.
        at_home(&mut p, "lomo_reforzado").expect("another thing");
        assert_eq!(p.upgrades.len(), 2);
        assert_eq!(at_home(&mut p, "inventada"), Err(UpgradeError::Unknown));
    }

    #[test]
    fn the_forge_asks_for_a_good_enough_tool() {
        let mut p = rich();
        p.tools = [0; 4];
        let need = |tier| Err(UpgradeError::NeedTool(ToolKind::Knife, tier));
        assert_eq!(at_home(&mut p, "filo_igneo"), need(1));
        p.tools[0] = 1;
        assert!(at_home(&mut p, "filo_igneo").is_ok());
        assert_eq!(at_home(&mut p, "filo_igneo"), need(3));
        p.tools[0] = 3;
        assert!(at_home(&mut p, "filo_igneo").is_ok());
        assert_eq!(at_home(&mut p, "filo_igneo"), need(4));
        // The other stations ask for materials only.
        assert!(at_home(&mut p, "tonico_espeso").is_ok());
        let said = UpgradeError::NeedTool(ToolKind::Knife, 3).message();
        assert!(
            said.contains("Cuchilla") && said.contains("Plata"),
            "{said}"
        );
    }

    #[test]
    fn what_is_refused_spends_nothing() {
        let d = test_data();
        let mut p = rich();
        let before = p.clone();
        let derived = stats::derive(d, &p);
        let forge = BTreeSet::from([Station::Forge]);
        assert_eq!(
            apply(d, &mut p, &derived, &BTreeSet::new(), true, "filo_igneo"),
            Err(UpgradeError::NeedStation(Station::Forge))
        );
        assert_eq!(
            apply(d, &mut p, &derived, &forge, false, "filo_igneo"),
            Err(UpgradeError::NotAtHome)
        );
        assert_eq!(p, before);
        let mut poor = PlayerData {
            tools: [5; 4],
            ..PlayerData::default()
        };
        poor.add_resource(Resource::Iron, 4);
        assert_eq!(
            at_home(&mut poor, "filo_igneo"),
            Err(UpgradeError::MissingResources)
        );
        assert_eq!(poor.resource(Resource::Iron), 4);
        assert!(poor.upgrades.is_empty());
    }

    #[test]
    fn the_crafting_discount_applies_to_improvements_too() {
        let mut p = rich();
        p.skills.insert("a_ahorro".into());
        p.skills.insert("a_ahorro2".into());
        let paid = at_home(&mut p, "lomo_reforzado").expect("new").cost;
        let total = |c: &[(Resource, u32)]| c.iter().map(|x| x.1).sum::<u32>();
        assert!(total(&paid) < total(&price(Station::Binder, 1)), "{paid:?}");
    }

    #[test]
    fn what_is_fitted_changes_the_player() {
        let d = test_data();
        let mut p = rich();
        let before = stats::derive(d, &p);
        assert_eq!(before.perk(Perk::HammerPower), 0.0);
        for id in [
            "lomo_reforzado",
            "lomo_reforzado",
            "filo_igneo",
            "golpe_seco",
            "mecha_clara",
        ] {
            at_home(&mut p, id).expect(id);
        }
        let after = stats::derive(d, &p);
        assert_eq!(after.max_hp, before.max_hp + 20.0);
        assert!((after.stat(Stat::BurnChance) - 0.08).abs() < 1e-5);
        assert!((after.perk(Perk::HammerPower) - 0.20).abs() < 1e-5);
        assert!(after.light_radius > before.light_radius);
    }

    #[test]
    fn edited_and_foreign_improvements_are_cleaned() {
        let d = test_data();
        let old: PlayerData = ron::from_str("(level: 4)").expect("old player data");
        assert!(old.upgrades.is_empty());
        let mut p = PlayerData::default();
        let put = |p: &mut PlayerData, target: &str, id: &str, level| {
            let fitted = Fitted {
                id: id.into(),
                level,
            };
            p.upgrades.insert(target.into(), fitted);
        };
        put(&mut p, "knife", "filo_igneo", 200);
        put(&mut p, "cover", "lomo_reforzado", 0);
        // Made for another thing, or for nothing at all.
        put(&mut p, "hammer", "mecha_clara", 2);
        put(&mut p, "lamp", "inventada", 2);
        put(&mut p, "nada", "brasa", 2);
        let tidy = p.sanitized(d);
        assert_eq!(fitted(&tidy, "knife"), Some(("filo_igneo".into(), 3)));
        assert_eq!(fitted(&tidy, "cover"), Some(("lomo_reforzado".into(), 1)));
        assert_eq!(tidy.upgrades.len(), 2);
    }

    #[test]
    fn each_improvement_explains_itself_with_this_players_levels() {
        let d = test_data();
        let mut p = rich();
        for upgrade in &d.upgrades.upgrades {
            let guide = explain(d, &p, &upgrade.id).expect(&upgrade.id);
            assert!(guide.does.len() > 15, "{}", upgrade.id);
            assert!(
                guide.helps.contains("nivel I:"),
                "{}: {}",
                upgrade.id,
                guide.helps
            );
        }
        at_home(&mut p, "filo_igneo").expect("new");
        let raise = explain(d, &p, "filo_igneo").expect("guide").helps;
        assert!(
            raise.contains("Ahora, nivel I:") && raise.contains("nivel II:"),
            "{raise}"
        );
        let swap = explain(d, &p, "filo_gemelo").expect("guide").helps;
        assert!(
            swap.contains("Filo ígneo") && swap.contains("conserva el nivel"),
            "{swap}"
        );
        at_home(&mut p, "filo_igneo").expect("II");
        at_home(&mut p, "filo_igneo").expect("III");
        let top = explain(d, &p, "filo_igneo").expect("guide").helps;
        assert!(top.contains("nivel III") && top.contains("último"), "{top}");
    }

    #[test]
    fn explanations_have_no_doubled_full_stop() {
        let d = test_data();
        for upgrade in &d.upgrades.upgrades {
            for other in d
                .upgrades
                .upgrades
                .iter()
                .filter(|u| u.target == upgrade.target)
            {
                for level in 0..=MAX_LEVEL {
                    let mut p = rich();
                    if level > 0 {
                        let fitted = Fitted {
                            id: other.id.clone(),
                            level,
                        };
                        p.upgrades.insert(other.target.clone(), fitted);
                    }
                    let guide = explain(d, &p, &upgrade.id).expect(&upgrade.id);
                    for text in [&guide.does, &guide.helps] {
                        assert!(!text.contains(".."), "{}: {text}", upgrade.id);
                    }
                }
            }
        }
    }

    #[test]
    fn improvements_that_would_do_nothing_say_why() {
        let d = test_data();
        let mut p = rich();
        let pages = explain(d, &p, "paginas_pesadas").expect("guide").missing;
        assert!(pages.expect("no skill").contains("«Páginas voladoras»"));
        let torches = explain(d, &p, "antorcha_calida").expect("guide").missing;
        assert!(torches.expect("no skill").contains("«Antorchas»"));
        // A golden hammer already gives all the extra there is.
        let quarry = explain(d, &p, "cantero").expect("guide").missing;
        assert!(quarry.expect("capped").contains("+5"));
        p.tools[1] = 1;
        assert_eq!(explain(d, &p, "cantero").expect("guide").missing, None);
        assert_eq!(
            explain(d, &p, "tonico_espeso").expect("guide").missing,
            None
        );
        assert_eq!(explain(d, &p, "no_existe"), None);
    }

    #[test]
    fn the_whetstones_are_made_at_the_forge() {
        let d = test_data();
        for level in 1..=5 {
            let stone = d.recipe(&format!("whet_{level}")).expect("whetstone");
            assert_eq!(stone.station, Some(Station::Forge));
        }
    }
}
