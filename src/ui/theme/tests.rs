use super::*;
use crate::ui::INK;
use crate::ui::state::Screen;
use babel_core::babel::rarity::Rarity;
use babel_core::data::Station;

#[test]
fn every_screen_has_the_place_the_design_gave_it() {
    for (screen, place) in [
        (Screen::Title, Place::Library),
        (Screen::NewGame, Place::Library),
        (Screen::Look, Place::Library),
        (Screen::Pause, Place::Library),
        (Screen::Coop, Place::Rooms),
        (Screen::Options, Place::Clockwork),
        (Screen::Controls, Place::Clockwork),
        (Screen::Build(0), Place::Build),
        (Screen::Chest, Place::Chest),
        (Screen::Lamps, Place::Lamps),
        (Screen::Scriptorium, Place::Scriptorium),
        (Screen::Inventory, Place::Inventory),
        (Screen::Skills, Place::Skills),
        (Screen::Glossary, Place::Glossary),
        (Screen::Map, Place::Map),
        (Screen::Catalog, Place::Catalog),
        (Screen::Reader, Place::Reader),
        (Screen::GameOver, Place::Ashes),
        (Screen::Stats, Place::Records),
        (Screen::Ending, Place::Dawn),
        (Screen::Credits, Place::Dawn),
    ] {
        assert_eq!(place_for(screen, None), place, "{screen:?}");
    }
    for (bench, place) in [
        (Some(Station::Forge), Place::Forge),
        (Some(Station::Binder), Place::Binder),
        (Some(Station::Alembic), Place::Alembic),
        (None, Place::Library),
    ] {
        assert_eq!(place_for(Screen::Crafting, bench), place, "{bench:?}");
    }
}

#[test]
fn no_two_places_look_alike() {
    for (i, a) in Place::ALL.into_iter().enumerate() {
        assert_eq!(a.theme().place, a);
        for b in Place::ALL.into_iter().skip(i + 1) {
            let (x, y) = (a.theme(), b.theme());
            assert!(
                (x.panel, x.accent) != (y.panel, y.accent),
                "{a:?} and {b:?} share their colours"
            );
            assert!(x.frame_style != y.frame_style, "{a:?} and {b:?}");
        }
    }
}

fn readable(what: &str, text: Rgba, back: Rgba, least: f32) {
    let found = contrast(text, back);
    assert!(found >= least, "{what}: {found:.2} of {least}");
}

#[test]
fn contrast_follows_the_usual_formula() {
    let (black, white) = (rgb(0, 0, 0), rgb(255, 255, 255));
    assert!((contrast(white, black) - 21.0).abs() < 0.01);
    assert!((contrast(black, white) - 21.0).abs() < 0.01);
    assert!((contrast(white, white) - 1.0).abs() < 0.01);
}

#[test]
fn text_can_be_read_on_every_place() {
    for place in Place::ALL {
        let t = place.theme();
        for (name, back) in [
            ("panel", t.panel),
            ("cell", t.cell),
            ("hot cell", t.cell_hot),
            ("well", t.well),
        ] {
            readable(&format!("{place:?} ink on {name}"), INK, back, 4.5);
        }
        for (name, back) in [("panel", t.panel), ("cell", t.cell), ("well", t.well)] {
            readable(&format!("{place:?} quiet on {name}"), t.dim, back, 4.5);
        }
        // Even what is out of reach can still be made out.
        for (name, back) in [("panel", t.panel), ("cell", t.cell)] {
            readable(&format!("{place:?} faint on {name}"), t.faint, back, 3.0);
        }
        readable(&format!("{place:?} accent"), t.accent, t.panel, 4.5);
        readable(&format!("{place:?} heading"), t.heading, t.panel, 4.5);
    }
}

#[test]
fn every_rarity_stands_out_on_every_place() {
    for place in Place::ALL.into_iter().filter(|&p| p != Place::Hud) {
        let t = place.theme();
        for rarity in [
            Rarity::Common,
            Rarity::Uncommon,
            Rarity::Rare,
            Rarity::Epic,
            Rarity::Legendary,
            Rarity::Canonical,
        ] {
            let color = crate::ui::rarity_rgba(rarity);
            for (name, back) in [("panel", t.panel), ("cell", t.cell)] {
                readable(&format!("{place:?} {rarity:?} on {name}"), color, back, 3.0);
            }
        }
    }
}

#[test]
fn only_the_quiet_places_have_no_animation() {
    for place in Place::ALL {
        let quiet = matches!(
            place,
            Place::Inventory | Place::Map | Place::Reader | Place::Records | Place::Hud
        );
        assert_eq!(place.theme().air == Air::None, quiet, "{place:?}");
    }
}

#[test]
fn only_three_screens_show_the_world_behind() {
    for screen in [Screen::Title, Screen::Pause, Screen::GameOver] {
        assert!(sees_through(screen), "{screen:?}");
    }
    for screen in [
        Screen::NewGame,
        Screen::Look,
        Screen::Options,
        Screen::Crafting,
        Screen::Inventory,
        Screen::Reader,
        Screen::Credits,
    ] {
        assert!(!sees_through(screen), "{screen:?}");
    }
}
