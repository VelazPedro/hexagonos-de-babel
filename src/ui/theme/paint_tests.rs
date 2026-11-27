//! The shared pieces, painted on a blank canvas.

use super::*;
use crate::render::atlas::Atlas;
use crate::render::canvas::{Canvas, with_alpha};
use crate::render::font::Font;
use crate::ui::{DIM, FAINT, Focus, GOLD, INK, PANEL_EDGE, Response, SKY, Ui, UiInput, rect};
use babel_core::babel::rarity::Rarity;
use std::path::Path;

struct Bench {
    canvas: Canvas,
    atlas: Atlas,
    font: Font,
    input: UiInput,
    focus: Focus,
    sounds: Vec<babel_core::game::events::Sfx>,
    art: ArtCache,
}

impl Bench {
    fn new() -> Bench {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        Bench {
            canvas: Canvas::new(480, 270),
            atlas: Atlas::load(&assets).expect("atlas"),
            font: Font::default(),
            input: UiInput::default(),
            focus: Focus::default(),
            sounds: Vec::new(),
            art: ArtCache::default(),
        }
    }

    /// Paints on a clean canvas with the look of a place.
    fn paint(&mut self, place: Place, see_through: bool, draw: impl FnOnce(&mut Ui)) {
        self.canvas.clear([0; 4]);
        let mut ui = Ui::new(
            &mut self.canvas,
            &self.atlas,
            &self.font,
            &self.input,
            &mut self.focus,
            &mut self.sounds,
            0.0,
        );
        ui.theme = place.theme();
        ui.see_through = see_through;
        ui.art = Some(&mut self.art);
        draw(&mut ui);
    }
}

fn response(focused: bool) -> Response {
    Response {
        focused,
        ..Default::default()
    }
}

/// The middle of the top border of an option drawn with an edge of its own.
fn top_border(bench: &mut Bench, place: Place, focused: bool, edge: Option<Rgba>) -> Rgba {
    bench.paint(place, false, |ui| {
        ui.cell(rect(10, 10, 101, 14), &response(focused), edge);
    });
    bench.canvas.get(60, 10)
}

#[test]
fn every_option_has_a_border() {
    let mut bench = Bench::new();
    for place in Place::ALL {
        let t = place.theme();
        assert_eq!(
            top_border(&mut bench, place, false, None),
            t.edge,
            "{place:?}"
        );
        assert_eq!(
            top_border(&mut bench, place, true, None),
            t.accent,
            "{place:?}"
        );
        // The old violet of a plain button gives way to the place's own edge.
        let plain = Some(PANEL_EDGE);
        assert_eq!(
            top_border(&mut bench, place, false, plain),
            t.edge,
            "{place:?}"
        );
    }
}

#[test]
fn borders_that_mean_something_are_kept() {
    let mut bench = Bench::new();
    for color in [
        GOLD,
        crate::ui::rarity_rgba(Rarity::Rare),
        rgb(206, 68, 100),
    ] {
        for focused in [false, true] {
            let found = top_border(&mut bench, Place::Forge, focused, Some(color));
            assert_eq!(found, color, "focused {focused}");
        }
    }
}

#[test]
fn role_colours_follow_the_place() {
    let mut bench = Bench::new();
    bench.paint(Place::Binder, false, |ui| {
        let t = ui.theme;
        assert_eq!(ui.tone(DIM), t.dim);
        assert_eq!(ui.tone(FAINT), t.faint);
        assert_eq!(ui.tone(SKY), t.heading);
        assert_eq!(ui.tone(INK), INK);
        assert_eq!(ui.tone(GOLD), GOLD);
        // A faded role stays as faded.
        assert_eq!(ui.tone(with_alpha(DIM, 90)), with_alpha(t.dim, 90));
    });
}

#[test]
fn one_widget_can_borrow_another_places_look() {
    let mut bench = Bench::new();
    bench.paint(Place::Library, false, |ui| {
        let inside = ui.with_place(Place::Map, |ui| ui.theme.place);
        assert_eq!(inside, Place::Map);
        assert_eq!(ui.theme.place, Place::Library);
    });
}

#[test]
fn backgrounds_and_textures_are_drawn_once() {
    let mut bench = Bench::new();
    let screen = |ui: &mut Ui| {
        ui.dim_background(210);
        ui.card(rect(4, 43, 222, 213), SKY);
        ui.card(rect(232, 43, 244, 213), SKY);
    };
    bench.paint(Place::Forge, false, screen);
    let first = bench.art.builds;
    assert!(first >= 3, "{first}");
    for _ in 0..5 {
        bench.paint(Place::Forge, false, screen);
    }
    assert_eq!(bench.art.builds, first);
    // Seeing through, the darkness may change every frame without new art.
    bench.paint(Place::Ashes, true, |ui| ui.dim_background(10));
    let faded = bench.art.builds;
    for alpha in [40, 90, 200] {
        bench.paint(Place::Ashes, true, |ui| ui.dim_background(alpha));
    }
    assert_eq!(bench.art.builds, faded);
}

#[test]
fn what_is_kept_looks_like_what_was_first_drawn() {
    let mut bench = Bench::new();
    let screen = |ui: &mut Ui| {
        ui.dim_background(210);
        ui.card(rect(4, 43, 222, 213), SKY);
    };
    bench.paint(Place::Binder, false, screen);
    let first = bench.canvas.px.clone();
    bench.paint(Place::Binder, false, screen);
    assert!(first == bench.canvas.px, "the kept copy differs");
}

#[test]
fn each_place_paints_its_own_background() {
    let mut bench = Bench::new();
    let mut seen: Vec<(Place, Vec<u8>)> = Vec::new();
    for place in Place::ALL.into_iter().filter(|&p| p != Place::Hud) {
        bench.paint(place, false, |ui| ui.dim_background(255));
        for (other, pixels) in &seen {
            assert!(pixels != &bench.canvas.px, "{place:?} and {other:?}");
        }
        seen.push((place, bench.canvas.px.clone()));
    }
}

#[test]
fn a_background_hides_the_world_unless_told_otherwise() {
    let mut bench = Bench::new();
    let corners = [(0, 0), (479, 0), (0, 269), (479, 269)];
    bench.paint(Place::Library, false, |ui| ui.dim_background(120));
    for (x, y) in corners {
        assert_eq!(bench.canvas.get(x, y)[3], 255, "({x}, {y})");
    }
    bench.paint(Place::Library, true, |ui| ui.dim_background(120));
    // The world shows through most of it, if not through every ornament.
    let see_through = bench
        .canvas
        .px
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] < 200)
        .count();
    assert!(see_through > 480 * 270 * 8 / 10, "{see_through}");
}

#[test]
fn long_titles_fit_on_their_plaque() {
    let mut bench = Bench::new();
    for (place, title) in [
        (Place::Binder, "Mesa de encuadernador"),
        (Place::Lamps, "Lámparas de retorno"),
        (Place::Dawn, "El Catálogo de catálogos"),
        (Place::Alembic, "Alambique de tinta"),
    ] {
        bench.paint(place, false, |ui| ui.heading(title, 6));
        let painted = bench.canvas.px.as_chunks::<4>().0.iter();
        assert!(painted.filter(|p| p[3] > 0).count() > 500, "{title}");
        for y in 0..270 {
            for x in [0, 479] {
                assert_eq!(bench.canvas.get(x, y)[3], 0, "{title} at ({x}, {y})");
            }
        }
    }
}

#[test]
fn the_plaque_leaves_room_below_the_letters() {
    let mut bench = Bench::new();
    // A title with spaces and letters that hang below the line.
    bench.paint(Place::Binder, false, |ui| ui.heading("Mesa de juego", 4));
    let panel = Place::Binder.theme().panel;
    // Under the first space nothing but the plaque, down to where the lowest
    // letters reach.
    let space = (240 - bench.font.width("Mesa de juego")) + bench.font.width("Mesa") * 2;
    for y in 12..=19 {
        assert_eq!(bench.canvas.get(space + 3, y), panel, "row {y}");
    }
}

#[test]
fn small_cards_keep_their_inside_clear() {
    let mut bench = Bench::new();
    for place in Place::ALL {
        bench.paint(place, false, |ui| ui.card(rect(100, 100, 200, 18), SKY));
        let base = bench.canvas.get(104, 105);
        for y in 105..=113 {
            for x in 104..296 {
                assert_eq!(bench.canvas.get(x, y), base, "{place:?} at ({x}, {y})");
            }
        }
    }
}

/// The places whose art is finished. Each task adds its own.
const FINISHED: &[Place] = &[
    Place::Forge,
    Place::Binder,
    Place::Alembic,
    Place::Scriptorium,
    Place::Build,
    Place::Chest,
    Place::Lamps,
    Place::Inventory,
    Place::Skills,
    Place::Glossary,
    Place::Map,
    Place::Catalog,
    Place::Library,
    Place::Rooms,
    Place::Clockwork,
    Place::Reader,
    Place::Ashes,
    Place::Dawn,
    Place::Records,
];

#[test]
fn finished_places_decorate_their_background() {
    for &place in FINISHED {
        let canvas = backdrop::opaque(&place.theme());
        // A bare gradient has rows of a single colour.
        let busy = (0..270)
            .filter(|&y| (1..480).any(|x| canvas.get(x, y) != canvas.get(0, y)))
            .count();
        assert!(busy > 100, "{place:?}: only {busy} rows are decorated");
    }
}

#[test]
fn finished_places_have_an_emblem_of_their_own() {
    for (i, &a) in FINISHED.iter().enumerate() {
        let drawing = art::emblem(a);
        assert!(drawing.len() >= 9 && drawing.len() <= 13, "{a:?}");
        let width = drawing[0].len();
        assert!((9..=25).contains(&width), "{a:?}: {width}");
        assert!(drawing.iter().all(|row| row.len() == width), "{a:?}");
        // Every letter of a drawing has a colour.
        for row in drawing {
            for c in row.chars() {
                assert!(
                    c == '.' || art::color_of(c, &a.theme()).is_some(),
                    "{a:?} '{c}'"
                );
            }
        }
        for &b in &FINISHED[i + 1..] {
            assert!(drawing != art::emblem(b), "{a:?} and {b:?}");
        }
    }
}

#[test]
fn what_floats_moves() {
    for &place in FINISHED {
        let theme = place.theme();
        let at = |time: f32| {
            let mut canvas = Canvas::new(480, 270);
            draw_air(&mut canvas, &theme, time);
            canvas.px
        };
        if theme.air == Air::None {
            assert!(at(3.0).iter().all(|&v| v == 0), "{place:?}");
            continue;
        }
        assert!(at(3.0).iter().any(|&v| v != 0), "{place:?} has nothing");
        assert!(at(3.0) != at(3.5), "{place:?} stands still");
        assert!(at(3.0) == at(3.0), "{place:?} is not repeatable");
    }
}
