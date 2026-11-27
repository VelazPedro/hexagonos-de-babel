//! What each thing that can be made or built does, and how it helps this
//! player, said before the materials are spent. Numbers come from the rules
//! themselves, so they cannot drift away from them.

use super::crafting::extra_batch;
use super::player::PlayerData;
use super::reading::readable_pages;
use super::stats::{self, Derived};
use crate::data::{
    Consumable, GameData, RecipeDef, RecipeOutput, Resource, Station, ToolKind, Unlock,
};
use crate::game::{LAMP_IN_HAND_LIGHT, player as combat};
use crate::world::floors;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Guide {
    /// What the thing is and does.
    pub does: String,
    /// What this player gets from it.
    pub helps: String,
    /// What is still missing before it is of any use.
    pub missing: Option<String>,
}

pub fn explain(data: &GameData, player: &PlayerData, recipe: &RecipeDef) -> Guide {
    let derived = stats::derive(data, player);
    let text = |does: &str, helps: String| Guide {
        does: does.to_string(),
        helps,
        missing: None,
    };
    match recipe.output {
        RecipeOutput::Tool(kind, tier) => text(
            &what_a_tool_does(kind),
            tool_help(data, player, &derived, kind, tier),
        ),
        RecipeOutput::Whetstone(level) => {
            let mut after = player.clone();
            after.whetstones = level;
            text(
                "Mejora permanente de tu arma. No ocupa lugar ni hay que equiparla.",
                format!(
                    "{} por golpe. También pegan más tus páginas lanzadas y tu mascota.",
                    damage_change(data, &derived, &after)
                ),
            )
        }
        RecipeOutput::Item(thing, _) => {
            let mut guide = consumable_guide(data, player, &derived, thing);
            guide.helps.push_str(&batch_note(recipe.output, &derived));
            guide
        }
        RecipeOutput::Material(resource, _) => text(
            "Prepara un material a partir de otros.",
            format!(
                "{} Tenés {}.{}",
                what_a_material_is_for(resource),
                player.resource(resource),
                batch_note(recipe.output, &derived)
            ),
        ),
        RecipeOutput::Build(station) => station_guide(data, station),
        RecipeOutput::ReturnLamp => text(
            "Una lámpara que queda encendida en este sitio.",
            "Sirve para volver desde tu base y para reaparecer ahí si caés.".into(),
        ),
    }
}

/// Says how much a batch brings when an improvement adds to it.
fn batch_note(output: RecipeOutput, derived: &Derived) -> String {
    let count = match output {
        RecipeOutput::Item(_, n) | RecipeOutput::Material(_, n) => n,
        _ => return String::new(),
    };
    match extra_batch(output, derived) {
        0 => String::new(),
        more => format!(" Con tu mejora salen {} por tanda.", count + more),
    }
}

fn percent(share: f32) -> i32 {
    (share * 100.0).round() as i32
}

/// "a, b y c".
fn in_words(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [first @ .., last] => format!("{} y {last}", first.join(", ")),
    }
}

fn what_a_tool_does(kind: ToolKind) -> String {
    match kind {
        ToolKind::Knife => {
            "Corta enredaderas, códices sellados y tinteros, con ella en la mano.".into()
        }
        ToolKind::Hammer => format!(
            "Con él en la mano rompe estanterías, piedras y metales. Peleando pega {} % \
             menos y empuja más.",
            percent(1.0 - combat::HAMMER_DAMAGE)
        ),
        ToolKind::Lamp => format!(
            "Tu farol. En la mano alumbra {} % más, pero pega {} % menos.",
            percent(LAMP_IN_HAND_LIGHT - 1.0),
            percent(1.0 - combat::LAMP_DAMAGE)
        ),
        ToolKind::Lens => "Con ella en la mano, atacar abre el libro del sitio. Leerlo da \
                           experiencia y llena el Glosario."
            .into(),
    }
}

fn damage_change(data: &GameData, now: &Derived, after: &PlayerData) -> String {
    format!(
        "Daño {} → {}",
        now.damage.round(),
        stats::derive(data, after).damage.round()
    )
}

/// The nodes that need exactly this tool at this tier, with what they give.
fn opened_by(data: &GameData, kind: ToolKind, tier: u8) -> String {
    let nodes: Vec<String> = data
        .items
        .nodes
        .iter()
        .filter(|n| n.tool == kind && n.min_tier == tier)
        .map(|n| {
            let gives: Vec<String> = n.yields.iter().map(|y| y.0.name().to_lowercase()).collect();
            format!("{} ({})", n.name, in_words(&gives))
        })
        .collect();
    if nodes.is_empty() {
        String::new()
    } else {
        format!(" Abre: {}.", in_words(&nodes))
    }
}

fn pages_with(data: &GameData, player: &PlayerData, tier: u8) -> usize {
    let mut with = player.clone();
    with.tools[ToolKind::Lens.index()] = tier;
    readable_pages(&with, &stats::derive(data, &with))
}

fn tool_help(
    data: &GameData,
    player: &PlayerData,
    now: &Derived,
    kind: ToolKind,
    tier: u8,
) -> String {
    let mut after = player.clone();
    after.tools[kind.index()] = tier;
    match kind {
        ToolKind::Knife => format!(
            "{}. Cosecha más rápido.{}{}",
            damage_change(data, now, &after),
            opened_by(data, kind, tier),
            way_down(kind, tier)
        ),
        ToolKind::Hammer => {
            let extra = |t: u8| stats::hammer_extra(data, now, t);
            format!(
                "Rompe más rápido. Cada cosa que rompés da +{} materiales (ahora +{}).{}{}",
                extra(tier),
                extra(player.tool_tier(kind)),
                opened_by(data, kind, tier),
                way_down(kind, tier)
            )
        }
        ToolKind::Lamp => format!(
            "Radio de luz: {:.1} → {:.1} baldosas. Ves antes a los enemigos que solo aparecen \
             en la luz.",
            now.light_radius,
            stats::derive(data, &after).light_radius
        ),
        ToolKind::Lens => {
            let before = readable_pages(player, now);
            let pages = pages_with(data, player, tier);
            if pages > before {
                format!(
                    "Leés {pages} páginas de cada libro en vez de {before}: más palabras y \
                     experiencia."
                )
            } else if player.has_relic("lente_profunda") {
                "Con la Lente Profunda ya leés el libro entero: solo cambia su aspecto.".into()
            } else if (tier + 1..=stats::MAX_TOOL_TIER)
                .any(|later| pages_with(data, player, later) > pages)
            {
                "No suma páginas por sí sola: es el paso previo a una lente que sí lo hace.".into()
            } else {
                "No suma nada nuevo: solo cambia su aspecto y da experiencia al fabricarla.".into()
            }
        }
    }
}

/// The way down this tier of knife or hammer opens, if any.
fn way_down(kind: ToolKind, tier: u8) -> String {
    let other = match kind {
        ToolKind::Knife => "el Martillo",
        ToolKind::Hammer => "la Cuchilla",
        ToolKind::Lamp | ToolKind::Lens => return String::new(),
    };
    floors::GATES
        .iter()
        .find(|g| g.1 == tier)
        .map_or_else(String::new, |(floor, _)| {
            format!(" Junto con {other} de este nivel, permite bajar al piso {floor}.")
        })
}

/// Says which skill is needed, if the player lacks it.
pub(crate) fn needs_skill(data: &GameData, now: &Derived, unlock: Unlock) -> Option<String> {
    if now.has(unlock) {
        return None;
    }
    data.skills
        .iter()
        .find(|s| s.unlock == Some(unlock))
        .map(|s| {
            format!(
                "Todavía no podés usarlas: falta la habilidad «{}» (rama {}).",
                s.name,
                s.branch.name()
            )
        })
}

fn consumable_guide(
    data: &GameData,
    player: &PlayerData,
    now: &Derived,
    thing: Consumable,
) -> Guide {
    let owned = player.consumable(thing);
    let (does, helps, missing) = match thing {
        Consumable::Torch => (
            "Un fuego que dejás en el suelo y alumbra esa zona.",
            "Para pelear a oscuras: hay enemigos que solo se ven y se golpean en la luz.".into(),
            needs_skill(data, now, Unlock::Torches),
        ),
        Consumable::PageAmmo => (
            "Munición para atacar a distancia.",
            format!(
                "Cada página pega el {} % de tu daño y gasta {} de energía.",
                percent(combat::page_share(now)),
                combat::page_energy(now)
            ),
            needs_skill(data, now, Unlock::RangedPages),
        ),
        Consumable::Tonic => (
            "Una bebida para curarte en cualquier momento.",
            format!(
                "Recupera el {} % de tu vida y quita quemaduras, veneno y lentitud.",
                percent(combat::tonic_share(now))
            ),
            None,
        ),
        Consumable::Index => (
            "Un pase del Catálogo: buscás una frase y viajás al sitio que la contiene.",
            "Elegís adónde ir. Cuanto más raras las palabras, más Índices cuesta.".into(),
            (!player.has_relic(super::catalog::SEAL_RELIC)).then(|| {
                "Todavía no sirve: el Catálogo se abre al vencer al guardián del piso 10.".into()
            }),
        ),
    };
    Guide {
        does: does.to_string(),
        helps: format!("{helps} Tenés {owned}."),
        missing,
    }
}

fn what_a_material_is_for(resource: Resource) -> &'static str {
    match resource {
        Resource::Ink => "La tinta sirve para inscribir tu equipo y para lentes e Índices.",
        _ => "Sirve para otras recetas.",
    }
}

/// What a station lets the player make, in the order the recipes are listed.
fn made_at(data: &GameData, station: Station) -> String {
    let mut kinds: Vec<String> = Vec::new();
    for recipe in data.recipes.iter().filter(|r| r.station == Some(station)) {
        let kind = match recipe.output {
            RecipeOutput::Tool(tool, _) => match tool {
                ToolKind::Knife => "cuchillas".into(),
                ToolKind::Hammer => "martillos".into(),
                ToolKind::Lamp => "lámparas".into(),
                ToolKind::Lens => "lentes".into(),
            },
            RecipeOutput::Whetstone(_) => "piedras de afilar".into(),
            RecipeOutput::Item(thing, _) => match thing {
                Consumable::Torch => "antorchas".into(),
                Consumable::Tonic => "tónicos".into(),
                Consumable::PageAmmo => "páginas afiladas".into(),
                Consumable::Index => "Índices".into(),
            },
            RecipeOutput::Material(resource, _) => resource.name().to_lowercase(),
            RecipeOutput::Build(_) | RecipeOutput::ReturnLamp => continue,
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    in_words(&kinds)
}

fn station_guide(data: &GameData, station: Station) -> Guide {
    let makes = || {
        let improves: Vec<String> = data
            .upgrades
            .targets
            .iter()
            .filter(|t| t.station == station)
            .map(|t| t.name.to_lowercase())
            .collect();
        format!(
            "Te permite fabricar {}. También podés mejorar {}.",
            made_at(data, station),
            in_words(&improves)
        )
    };
    let (does, helps) = match station {
        Station::Binder => ("Una mesa de trabajo en tu hexágono.", makes()),
        Station::Forge => ("El taller del metal, en tu hexágono.", makes()),
        Station::Alembic => ("Un destilador en tu hexágono.", makes()),
        Station::Scriptorium => (
            "El escritorio de las palabras, en tu hexágono.",
            "Te permite fabricar Índices y, con tinta, inscribir en tu equipo palabras de tu \
             Glosario para sumarle efectos."
                .into(),
        ),
        Station::Chest => (
            "Un lugar para guardar materiales en tu hexágono.",
            "Lo guardado no se pierde al caer. De lo que llevás encima, al caer queda cerca del \
             20 % en una bolsa que podés recuperar."
                .into(),
        ),
    };
    Guide {
        does: does.to_string(),
        helps,
        missing: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{RecipeOutput, ToolKind, test_data};

    fn about(player: &PlayerData, id: &str) -> Guide {
        let d = test_data();
        explain(d, player, d.recipe(id).expect(id))
    }

    fn with_tool(kind: ToolKind, tier: u8) -> PlayerData {
        let mut player = PlayerData::default();
        player.tools[kind.index()] = tier;
        player
    }

    #[test]
    fn everything_that_can_be_made_says_what_it_does_and_how_it_helps() {
        let d = test_data();
        let player = PlayerData::default();
        for recipe in &d.recipes {
            if recipe.output == RecipeOutput::ReturnLamp {
                continue;
            }
            let guide = explain(d, &player, recipe);
            assert!(guide.does.len() > 15, "{}: {:?}", recipe.id, guide.does);
            assert!(guide.helps.len() > 15, "{}: {:?}", recipe.id, guide.helps);
        }
    }

    #[test]
    fn upgrades_show_this_players_real_numbers() {
        let fresh = PlayerData::default();
        assert!(about(&fresh, "knife_1").helps.contains("Daño 7 → 11"));
        assert!(about(&fresh, "whet_1").helps.contains("Daño 7 → 10"));
        // A stronger player reads their own numbers.
        let sharper = PlayerData {
            whetstones: 2,
            ..PlayerData::default()
        };
        assert!(about(&sharper, "knife_1").helps.contains("Daño 13 → 17"));
        assert!(about(&sharper, "whet_3").helps.contains("Daño 13 → 16"));
        assert!(about(&fresh, "lamp_1").helps.contains("4.5 → 5.2"));
    }

    #[test]
    fn a_better_tool_names_what_it_opens() {
        let fresh = PlayerData::default();
        let hammer = about(&fresh, "hammer_1").helps;
        assert!(hammer.contains("Veta de latón (latón)"), "{hammer}");
        let knife = about(&with_tool(ToolKind::Knife, 1), "knife_2").helps;
        assert!(knife.contains("Códice sellado"), "{knife}");
        assert!(!about(&fresh, "knife_1").helps.contains("Abre"));
    }

    #[test]
    fn the_hammer_says_how_much_more_it_gathers() {
        let first = about(&PlayerData::default(), "hammer_1");
        assert!(
            first.helps.contains("+1 materiales (ahora +0)"),
            "{}",
            first.helps
        );
        let last = about(&with_tool(ToolKind::Hammer, 4), "hammer_5").helps;
        assert!(last.contains("+5 materiales (ahora +4)"), "{last}");
        assert!(first.does.contains("en la mano") && first.does.contains("estanterías"));
    }

    #[test]
    fn tools_that_open_a_way_down_say_which() {
        let fresh = PlayerData::default();
        for id in ["knife_1", "hammer_1"] {
            let helps = about(&fresh, id).helps;
            assert!(helps.contains("piso 4"), "{id}: {helps}");
        }
        let brass = about(&with_tool(ToolKind::Hammer, 1), "hammer_2").helps;
        assert!(brass.contains("piso 8"), "{brass}");
        let golden = about(&with_tool(ToolKind::Knife, 4), "knife_5").helps;
        assert!(!golden.contains("bajar"), "{golden}");
    }

    #[test]
    fn lenses_that_add_nothing_say_so() {
        let first = about(&PlayerData::default(), "lens_1").helps;
        assert!(
            first.contains("2 páginas") && first.contains("en vez de 1"),
            "{first}"
        );
        let second = about(&with_tool(ToolKind::Lens, 1), "lens_2").helps;
        assert!(second.contains("paso previo"), "{second}");
        let third = about(&with_tool(ToolKind::Lens, 2), "lens_3").helps;
        assert!(third.contains("3 páginas"), "{third}");
        for (tier, id) in [(3, "lens_4"), (4, "lens_5")] {
            let late = about(&with_tool(ToolKind::Lens, tier), id).helps;
            assert!(late.contains("No suma nada"), "{id}: {late}");
        }
        let mut all_seeing = PlayerData::default();
        all_seeing.relics.push("lente_profunda".into());
        let relic = about(&all_seeing, "lens_1").helps;
        assert!(relic.contains("Lente Profunda"), "{relic}");
    }

    #[test]
    fn what_cannot_be_used_yet_warns_about_it() {
        let mut player = PlayerData::default();
        let torch = about(&player, "torch").missing.expect("no skill yet");
        assert!(
            torch.contains("«Antorchas»") && torch.contains("Explorador"),
            "{torch}"
        );
        let pages = about(&player, "page_ammo").missing.expect("no skill yet");
        assert!(pages.contains("«Páginas voladoras»"), "{pages}");
        let index = about(&player, "index").missing.expect("sealed");
        assert!(index.contains("piso 10"), "{index}");
        assert_eq!(about(&player, "tonic").missing, None);

        player.skills.insert("e_antorchas".into());
        player.skills.insert("c_paginas".into());
        player.relics.push("sello_catalogo".into());
        for id in ["torch", "page_ammo", "index"] {
            assert_eq!(about(&player, id).missing, None, "{id}");
        }
    }

    #[test]
    fn improvements_show_in_the_numbers_of_what_they_improve() {
        let mut player = PlayerData::default();
        for (target, id) in [("tonic", "tonico_espeso"), ("pages", "paginas_pesadas")] {
            let fitted = crate::progression::upgrades::Fitted {
                id: id.into(),
                level: 3,
            };
            player.upgrades.insert(target.into(), fitted);
        }
        assert!(about(&player, "tonic").helps.contains("67 %"));
        assert!(about(&player, "page_ammo").helps.contains("105 %"));
        player.upgrades.clear();
        let rich = crate::progression::upgrades::Fitted {
            id: "tonico_rendidor".into(),
            level: 3,
        };
        player.upgrades.insert("tonic".into(), rich);
        let helps = about(&player, "tonic").helps;
        assert!(helps.contains("salen 4 por tanda"), "{helps}");
        let quarry = crate::progression::upgrades::Fitted {
            id: "cantero".into(),
            level: 2,
        };
        player.upgrades.insert("hammer".into(), quarry);
        let helps = about(&player, "hammer_1").helps;
        assert!(helps.contains("+3 materiales (ahora +2)"), "{helps}");
    }

    #[test]
    fn what_is_carried_is_counted() {
        let player = PlayerData::default();
        // Everyone starts with two tonics and three torches.
        assert!(about(&player, "tonic").helps.contains("Tenés 2"));
        assert!(about(&player, "torch").helps.contains("Tenés 3"));
    }

    #[test]
    fn buildings_list_what_they_allow() {
        let player = PlayerData::default();
        let binder = about(&player, "build_binder").helps;
        assert!(
            binder.contains("cuchillas, lentes, antorchas y páginas afiladas"),
            "{binder}"
        );
        let forge = about(&player, "build_forge").helps;
        assert!(
            forge.contains("martillos, lámparas y piedras de afilar"),
            "{forge}"
        );
        assert!(about(&player, "build_alembic").helps.contains("tónicos"));
        // Each of these also improves things of its own.
        for (id, things) in [
            ("build_binder", "cuaderno, páginas y antorchas"),
            ("build_forge", "cuchilla, martillo, lámpara y lente"),
            ("build_alembic", "tónico y tinta"),
        ] {
            let helps = about(&player, id).helps;
            assert!(helps.contains("mejorar"), "{id}: {helps}");
            assert!(helps.contains(things), "{id}: {helps}");
        }
        let scriptorium = about(&player, "build_scriptorium").helps;
        assert!(scriptorium.contains("Índices") && scriptorium.contains("inscribir"));
        let chest = about(&player, "build_chest").helps;
        assert!(
            chest.contains("no se pierde") && chest.contains("20 %"),
            "{chest}"
        );
    }
}
