//! Little drawings: emblems for the title plaques. Each letter stands for a
//! colour (see [`color_of`]); a dot is nothing.

use super::{Place, Theme};
use crate::render::canvas::{Canvas, Rgba, rgb};

const ANVIL: &[&str] = &[
    ".....aaafffffaaa.....",
    "....ooooooooooooooo..",
    "...osssssssssssssssoo",
    "oooogggggggggggggggoo",
    "ogggggggggggggggggooo",
    ".ookkkgggggggggkko...",
    "...oookkkgggkkkooo...",
    "......okgggggko......",
    "......okgggggko......",
    ".....okkgggggkko.....",
    "...ooossssssssssooo..",
    "...okkkkkkkkkkkkko...",
    "...ooooooooooooooo...",
];

const BOOK: &[&str] = &[
    ".oooooooo.....oooooooo.",
    "ollllllllo...ollllllllo",
    "olbbbbbbllo.ollbbbbbclo",
    "olllllllllloollllllllco",
    "olbbbbblllrolllbbbbbclo",
    "olllllllllloorllllllllo",
    "olbbbblllllrolllbbbbclo",
    "olllllllllloorllllllllo",
    "ommmmmmmmmmrommmmmmmmmo",
    ".oooooooooorooooooooooo",
    "..........oro..........",
    "...........o...........",
];

const FLASK: &[&str] = &[
    "......ooooo......",
    ".......olo.......",
    ".......olo.......",
    "......olmmo......",
    ".....olmmmmo.....",
    "....olmmmmmmo....",
    "...olaaaaaaaao...",
    "..olaahaaaaaaao..",
    "..oaaaaaaahaaao..",
    "..oaaahaaaaaaao..",
    "...oaaaaaaaaao...",
    "....ooooooooo....",
];

const QUILL: &[&str] = &[
    "...............ooo.",
    ".............oollo.",
    "...........oollllo.",
    ".........oollllo...",
    "........ollllo.....",
    ".......ollloo......",
    "......oooo.........",
    "..ooooomooooo......",
    "...ohhhmhhho.......",
    "..oaaaaaaaaao......",
    "..oaaaaaaaaao......",
    "...ooooooooo.......",
];

const COMPASS: &[&str] = &[
    "......ooo......",
    ".....olmlo.....",
    "......ooo......",
    ".....om.mo.....",
    ".....om.mo.....",
    "....om...mo....",
    "....omaaamo....",
    "...om.....mo...",
    "...om.....mo...",
    "..om.......mo..",
    "..oo.......oo..",
    ".oo.........aa.",
    ".o...........a.",
];

const COFFER: &[&str] = &[
    "...ooooooooooooo...",
    "..ommmmmmmmmmmmmo..",
    ".omwwwwwwwwwwwwwmo.",
    ".omwwwwwwwwwwwwwmo.",
    ".ooooooooaoooooooo.",
    ".omwwwwoaaaowwwwmo.",
    ".omwwwwoalaowwwwmo.",
    ".omwwwwooooowwwwmo.",
    ".omwwwwwwwwwwwwwmo.",
    ".ommmmmmmmmmmmmmmo.",
    ".ooooooooooooooooo.",
];

const LANTERN: &[&str] = &[
    ".....ooo.....",
    "....o...o....",
    "...ooooooo...",
    "..ommmmmmmo..",
    "...oaaaaao...",
    "..oaafffaao..",
    "..oaafffaao..",
    "..oaafffaao..",
    "..oaaafaaao..",
    "...oaaaaao...",
    "..ommmmmmmo..",
    "...ooooooo...",
];

const SATCHEL: &[&str] = &[
    "......ooooo......",
    ".....o.....o.....",
    "..ooooooooooooo..",
    ".ommmmmmmmmmmmmo.",
    ".omwwwwwwwwwwwmo.",
    ".omwwwwoaowwwwmo.",
    ".ommmmmoaommmmmo.",
    ".odddddoaodddddo.",
    ".odddddooodddddo.",
    ".odddddddddddddo.",
    "..ooooooooooooo..",
];

const STAR: &[&str] = &[
    "......o......",
    ".....oao.....",
    ".....oao.....",
    "....oalao....",
    "ooooaalaaoooo",
    ".oaaalllaaao.",
    "..oaaalaaao..",
    "...oaaaaao...",
    "...oaaoaao...",
    "..oaao.oaao..",
    "..ooo...ooo..",
];

const CARDS: &[&str] = &[
    "..ooooooooooooo..",
    "..ollllllllllco..",
    ".oooooooooooooo..",
    ".ollllllllllcoo..",
    "ooooooooooooooo..",
    "oaaaaaaaaaaaaao..",
    "oaaoooooooooaao..",
    "oaaolllllllloao..",
    "oaaoooooooooaao..",
    "oaaaaaaaaaaaaao..",
    "ooooooooooooooo..",
];

const WIND_ROSE: &[&str] = &[
    "......o......",
    ".....oao.....",
    "..o..oao..o..",
    "...o.oao.o...",
    "....ooaoo....",
    "oooooalaooooo",
    "ommmmlllmmmmo",
    "oooooalaooooo",
    "....oomoo....",
    "...o.omo.o...",
    "..o..omo..o..",
    ".....omo.....",
    "......o......",
];

const HEXAGON: &[&str] = &[
    "....ooooooo....",
    "...oaaaaaaao...",
    "..oaoooooooao..",
    ".oaoolllllooao.",
    "oaoollhhhlloaao",
    "oaolhhhhhhhloao",
    "oaoollhhhlloaao",
    ".oaoolllllooao.",
    "..oaoooooooao..",
    "...oaaaaaaao...",
    "....ooooooo....",
];

const BOOK_STACK: &[&str] = &[
    "....ooooooooo....",
    "....oaaalaaao....",
    "....ooooooooo....",
    "..ooooooooooooo..",
    "..ohhhhlhhhhhho..",
    "..ooooooooooooo..",
    ".ooooooooooooooo.",
    ".orrrrrlrrrrrrro.",
    ".ooooooooooooooo.",
];

const LINKED: &[&str] = &[
    "...ooooo...ooooo...",
    "..ohhhhho.oaaaaao..",
    ".ohooooohoaoooooao.",
    "oho.....oaho.....ao",
    "oho....oahoo.....ao",
    "oho....oaoho.....ao",
    "oho.....oaho....oao",
    ".ohooooohoaoooooao.",
    "..ohhhhho.oaaaaao..",
    "...ooooo...ooooo...",
];

const COG: &[&str] = &[
    ".....ooo.....",
    "..oo.oao.oo..",
    ".oaaooaooaao.",
    ".oaaaaaaaaao.",
    "..oaaoooaao..",
    "ooaao...oaaoo",
    "oaaao...oaaao",
    "ooaao...oaaoo",
    "..oaaoooaao..",
    ".oaaaaaaaaao.",
    ".oaaooaooaao.",
    "..oo.oao.oo..",
    ".....ooo.....",
];

const BOOKMARK: &[&str] = &[
    "..ooooooooo..",
    "..orrrrrrro..",
    "..orrlrrrro..",
    "..orrlrrrro..",
    "..orrrarrro..",
    "..orraaarro..",
    "..orrrarrro..",
    "..orrrrrrro..",
    "..orrroorro..",
    "..orro..oro..",
    "..ooo....oo..",
];

const FALLEN_LEAF: &[&str] = &[
    "........ooo...",
    "......oommo...",
    "....oommdmo...",
    "...ommmdmmo...",
    "..ommmdmmmo...",
    "..ommdmmmo....",
    ".ommdmmmo.....",
    ".omdmmoo......",
    ".odooo........",
    "oo............",
];

const SUNRISE: &[&str] = &[
    "........o........",
    "..o.....a.....o..",
    "...a....a....a...",
    "....a.ooooo.a....",
    ".....oaaaaao.....",
    "....oaafffaao....",
    "...oaafffffaao...",
    "oooaafffffffaaooo",
    "ooooooooooooooooo",
    ".ommmmmmmmmmmmmo.",
    "..ooooooooooooo..",
];

const LEDGER: &[&str] = &[
    "..ooooooooooo..",
    ".ollllolllllbo.",
    ".olbbbolbbblbo.",
    ".ollllolllllbo.",
    ".olbbbolbbblbo.",
    ".ollllolllllbo.",
    ".olbbbolbbblbo.",
    ".ollllolllllbo.",
    ".oaaaaaoaaaaao.",
    ".ooooooooooooo.",
    "..o.........o..",
];

const DIAMOND: &[&str] = &[
    "....o....",
    "...oao...",
    "..oalao..",
    ".oalllao.",
    "oalllllao",
    ".oalllao.",
    "..oalao..",
    "...oao...",
    "....o....",
];

pub fn emblem(place: Place) -> &'static [&'static str] {
    match place {
        Place::Forge => ANVIL,
        Place::Binder => BOOK,
        Place::Alembic => FLASK,
        Place::Scriptorium => QUILL,
        Place::Build => COMPASS,
        Place::Chest => COFFER,
        Place::Lamps => LANTERN,
        Place::Inventory => SATCHEL,
        Place::Skills => STAR,
        Place::Glossary => CARDS,
        Place::Map => WIND_ROSE,
        Place::Catalog => HEXAGON,
        Place::Library => BOOK_STACK,
        Place::Rooms => LINKED,
        Place::Clockwork => COG,
        Place::Reader => BOOKMARK,
        Place::Ashes => FALLEN_LEAF,
        Place::Dawn => SUNRISE,
        Place::Records => LEDGER,
        _ => DIAMOND,
    }
}

/// The colour a letter of a drawing stands for in a place: o outline, d and
/// m the frame, l light, a accent, h heading; the rest are materials.
pub fn color_of(letter: char, theme: &Theme) -> Option<Rgba> {
    Some(match letter {
        'o' => theme.frame_dark,
        'd' => theme.frame,
        'm' => theme.frame_light,
        'l' => rgb(244, 232, 214),
        'a' => theme.accent,
        'h' => theme.heading,
        // Iron: dark, middle and shine.
        'k' => rgb(70, 62, 78),
        'g' => rgb(118, 108, 126),
        's' => rgb(214, 204, 214),
        // Hot metal and flame.
        'f' => rgb(255, 232, 150),
        // Paper in shade, ink on it, a ribbon and wood.
        'c' => rgb(214, 190, 150),
        'b' => rgb(170, 130, 96),
        'r' => rgb(214, 62, 70),
        'w' => rgb(150, 94, 52),
        _ => return None,
    })
}

/// Paints a drawing with its top-left corner at `(x, y)`.
pub fn stamp(canvas: &mut Canvas, art: &[&str], x: i32, y: i32, theme: &Theme) {
    for (dy, row) in art.iter().enumerate() {
        for (dx, letter) in row.chars().enumerate() {
            if let Some(color) = color_of(letter, theme) {
                canvas.put(x + dx as i32, y + dy as i32, color);
            }
        }
    }
}
