//! What the character has done, in four groups.

use super::state::Cmd;
use super::{DIM, INK, SKY, Ui, WIDTH, rect};
use crate::render::canvas::{Blit, rgb};
use babel_core::game::Game;

/// Lines a card has room for, and how wide each may be.
#[cfg(test)]
const MAX_LINES: usize = 5;
pub const LINE_WIDTH: i32 = 204;

/// A group of statistics: its name and each line with its value.
pub type Group = (&'static str, Vec<(&'static str, String)>);

pub fn play_time_text(seconds: u64) -> String {
    let (hours, minutes) = (seconds / 3600, seconds / 60 % 60);
    match hours {
        0 => format!("{minutes} min"),
        _ => format!("{} h {minutes:02} min", grouped(hours)),
    }
}

/// A number with a dot between each three digits.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(digit);
    }
    out
}

pub fn rows(game: &Game) -> [Group; 4] {
    let player = &game.profile.player;
    let t = player.records.tally;
    let n = |count: u32| grouped(u64::from(count));
    let words = format!("{} de {}", player.glossary.len(), game.data.lexicon.len());
    [
        (
            "Tu camino",
            vec![
                ("Tiempo jugado", play_time_text(t.play_seconds)),
                ("Piso más profundo", n(t.deepest_floor)),
                ("Hexágonos visitados", n(t.hexagons)),
                ("Veces que caíste", n(t.falls)),
            ],
        ),
        (
            "Combate",
            vec![
                ("Enemigos vencidos", n(t.enemies)),
                ("Élites", n(t.elites)),
                ("Guardianes", n(t.guardians)),
                ("Daño hecho", grouped(t.damage_dealt)),
                ("Daño recibido", grouped(t.damage_taken)),
            ],
        ),
        (
            "Lectura",
            vec![
                ("Libros leídos", n(t.books)),
                ("Páginas leídas", n(t.pages)),
                ("Palabras descubiertas", words),
                ("Frases canónicas", n(t.canons)),
            ],
        ),
        (
            "Oficio",
            vec![
                ("Materiales juntados", grouped(t.materials)),
                ("Cosas rotas", n(t.broken)),
                ("Objetos fabricados", n(t.crafted)),
                ("Mejoras", n(t.improved)),
            ],
        ),
    ]
}

/// The mark and the colour of each group, in the order they come.
const MARKS: [(&str, [u8; 4]); 4] = [
    ("ui_map", rgb(140, 222, 236)),
    ("icon_weapon", rgb(255, 150, 140)),
    ("ui_book", rgb(255, 214, 110)),
    ("icon_hammer", rgb(174, 226, 150)),
];

pub fn draw(ui: &mut Ui, game: &Game, cmds: &mut Vec<Cmd>) {
    ui.dim_background(225);
    ui.heading("Estadísticas", 8);
    let player = &game.profile.player;
    let who = match player.name.as_str() {
        "" => format!("Nivel {}", player.level),
        name => format!("{name} · Nivel {}", player.level),
    };
    ui.centered(&who, WIDTH / 2, 31, INK);
    for (i, ((title, lines), (icon, tone))) in rows(game).into_iter().zip(MARKS).enumerate() {
        let (x, y) = (8 + (i as i32 % 2) * 236, 44 + (i as i32 / 2) * 99);
        ui.card(rect(x, y, CARD_WIDTH, 93), SKY);
        ui.icon_at(icon, x + 17, y + 13, Blit::default());
        ui.section(title, x + 29, x + CARD_WIDTH - 12, y + 10, tone);
        for (row, (label, value)) in lines.into_iter().enumerate() {
            let line = y + 27 + row as i32 * 12;
            ui.plain(label, x + 12, line, DIM);
            ui.right(&value, x + 12 + LINE_WIDTH, line, INK);
        }
    }
    if ui.button(rect(WIDTH / 2 - 50, 242, 100, 13), "Cerrar") {
        cmds.push(Cmd::Pop);
    }
    let back = ui.glyph("Esc", "B");
    ui.hints(&[(back, "volver")]);
}

const CARD_WIDTH: i32 = LINE_WIDTH + 24;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::font::Font;
    use babel_core::data::GameData;
    use babel_core::game::profile::Profile;
    use babel_core::progression::records::Tally;
    use std::sync::Arc;

    fn game() -> Game {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let data = GameData::load(&assets).expect("data");
        Game::new(Arc::new(data), Profile::new(5))
    }

    #[test]
    fn numbers_are_written_for_people() {
        for (seconds, text) in [
            (0, "0 min"),
            (59, "0 min"),
            (2_700, "45 min"),
            (3_600, "1 h 00 min"),
            (11_100, "3 h 05 min"),
        ] {
            assert_eq!(play_time_text(seconds), text);
        }
        for (n, text) in [
            (0, "0"),
            (999, "999"),
            (1_000, "1.000"),
            (12_345, "12.345"),
            (1_234_567, "1.234.567"),
        ] {
            assert_eq!(grouped(n), text);
        }
    }

    #[test]
    fn the_four_groups_show_what_was_counted() {
        let mut g = game();
        g.profile.player.records.tally = Tally {
            play_seconds: 11_100,
            deepest_floor: 12,
            hexagons: 140,
            falls: 3,
            enemies: 1_250,
            elites: 14,
            guardians: 2,
            damage_dealt: 45_678,
            damage_taken: 2_001,
            books: 31,
            pages: 77,
            canons: 4,
            materials: 3_210,
            broken: 888,
            crafted: 56,
            improved: 9,
        };
        let groups = rows(&g);
        let titles: Vec<&str> = groups.iter().map(|g| g.0).collect();
        assert_eq!(titles, ["Tu camino", "Combate", "Lectura", "Oficio"]);
        let all: Vec<(&str, &str)> = groups
            .iter()
            .flat_map(|g| g.1.iter().map(|l| (l.0, l.1.as_str())))
            .collect();
        let words = format!("0 de {}", g.data.lexicon.len());
        assert_eq!(
            all,
            [
                ("Tiempo jugado", "3 h 05 min"),
                ("Piso más profundo", "12"),
                ("Hexágonos visitados", "140"),
                ("Veces que caíste", "3"),
                ("Enemigos vencidos", "1.250"),
                ("Élites", "14"),
                ("Guardianes", "2"),
                ("Daño hecho", "45.678"),
                ("Daño recibido", "2.001"),
                ("Libros leídos", "31"),
                ("Páginas leídas", "77"),
                ("Palabras descubiertas", words.as_str()),
                ("Frases canónicas", "4"),
                ("Materiales juntados", "3.210"),
                ("Cosas rotas", "888"),
                ("Objetos fabricados", "56"),
                ("Mejoras", "9"),
            ]
        );
    }

    #[test]
    fn the_longest_lines_fit_their_card() {
        let mut g = game();
        g.profile.player.records.tally = Tally {
            play_seconds: 1_000_000_000,
            deepest_floor: 100_000_000,
            hexagons: 100_000_000,
            damage_dealt: 1_000_000_000_000,
            damage_taken: 1_000_000_000_000,
            materials: 1_000_000_000_000,
            ..Tally::default()
        };
        let font = Font::default();
        for (_, lines) in rows(&g) {
            assert!(!lines.is_empty() && lines.len() <= MAX_LINES);
            for (label, value) in lines {
                let width = font.width(label) + font.width(&value) + 10;
                assert!(width <= LINE_WIDTH, "{label}: {value} takes {width}");
            }
        }
    }
}
