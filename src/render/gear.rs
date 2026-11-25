//! What the player's equipment looks like. The word an item was born from picks
//! its shape (an axe, a hood, sandals, a ring…); its rarity, or the tier of a
//! tool, picks its colours.

use super::canvas::{Rgba, mix, rgb};
use babel_core::babel::rarity::Rarity;
use babel_core::data::{GameData, Slot, ToolKind};
use babel_core::progression::items::Item;
use babel_core::progression::player::PlayerData;

/// Declares a family of shapes and the item words that wear each one.
macro_rules! shapes {
    ($(#[$meta:meta])* $name:ident, $by_word:ident { $($shape:ident => [$($word:literal),*]),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($shape),*
        }

        impl $name {
            #[cfg(test)]
            pub const ALL: &[$name] = &[$($name::$shape),*];
        }

        /// The shape a word calls for, if it names one.
        pub fn $by_word(word: &str) -> Option<$name> {
            match word {
                $($($word => Some($name::$shape),)*)*
                _ => None,
            }
        }
    };
}

shapes!(
    /// Things held in the hand: weapons, and the tools themselves.
    Held, weapon_for {
        Spear => ["lanza", "jabalina", "pica", "aguja", "dardo", "flecha"],
        Trident => ["tridente"],
        Sword => ["espada"],
        Dagger => ["daga", "cuchillo", "puñal", "navaja", "estilete", "bisturi", "cincel", "punzon", "tijera", "compas"],
        Rapier => ["estoque", "florete"],
        Curved => ["sable", "cimitarra", "alfanje", "machete"],
        Axe => ["hacha"],
        Scythe => ["hoz", "guadaña"],
        Mace => ["martillo", "maza", "mazo", "garrote", "porra", "ariete", "yunque", "clava"],
        Staff => ["baston", "cetro", "vara", "pala", "remo"],
        Bow => ["arco", "ballesta", "honda"],
        Whip => ["latigo", "cadena"],
        Quill => ["pluma"],
        // Tool shapes: no item word asks for them.
        Hammer => [],
        Lantern => [],
        Lens => [],
    }
);

shapes!(Armor, armor_for {
    Plate => ["coraza", "armadura", "peto", "cota", "guantelete", "brazal", "hombrera"],
    Cloak => ["capa", "manto"],
    Robe => ["tunica", "sotana"],
    Vest => ["chaleco", "delantal", "faja"],
    Shield => ["escudo"],
    Helmet => ["yelmo", "casco"],
    Hood => ["capucha"],
    Hat => ["gorro", "turbante", "mitra"],
    Crown => ["corona", "diadema"],
    Mask => ["mascara", "antifaz"],
});

shapes!(Footwear, footwear_for {
    Boots => ["bota", "media", "paso", "sendero"],
    Sandals => ["sandalia", "zapato", "zueco", "alpargata", "escarpin", "pantufla"],
    Greaves => ["greba", "coturno", "polaina"],
    Spurs => ["espuela", "estribo", "galope"],
});

shapes!(LampGear, lamp_for {
    Lantern => ["linterna", "lucerna", "reflector", "foco"],
    Candle => ["candil", "candela", "veladora", "velon", "bujia"],
    Torch => ["tea", "mechero"],
    Brazier => ["pebetero"],
});

shapes!(
    /// Anything not listed here hangs from the neck.
    Charm, charm_for {
        Pendant => ["amuleto", "talisman", "collar", "dije", "medalla", "pendiente", "broche"],
        Ring => ["anillo", "brazalete", "eslabon"],
        Flask => ["caliz", "copa", "anfora", "frasco", "vasija", "redoma", "elixir", "grial", "cantaro", "jarra", "botella"],
        Pouch => ["bolsa", "cofre", "arcon", "tesoro", "cesto", "canasto"],
        Book => ["indice", "catalogo", "mapa", "carta", "diario", "agenda", "glosa", "abecedario", "diccionario", "prologo", "pagina", "biblioteca"],
    }
);

/// Main colour, shade and accent of one piece.
pub type Colors = [Rgba; 3];

/// One equipped piece: its shape and the colours it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece<S> {
    pub shape: S,
    pub colors: Colors,
    /// Light it gives off, for the finest pieces.
    pub glow: Option<Rgba>,
}

/// Everything visible that the player carries. Cheap to compare, so the hero
/// is only redrawn when it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gear {
    /// In the hand: the weapon with the knife, or the tool in use.
    pub held: Piece<Held>,
    /// The weapon, slung on the back while another tool is out.
    pub back: Option<Piece<Held>>,
    pub armor: Option<Piece<Armor>>,
    pub boots: Option<Piece<Footwear>>,
    pub lamp: Option<Piece<LampGear>>,
    pub charm: Option<Piece<Charm>>,
}

/// Metal of each tool tier: base and shade, from rustic to golden.
const METALS: [(Rgba, Rgba); 6] = [
    (rgb(0xa8, 0x92, 0x78), rgb(0x74, 0x60, 0x50)),
    (rgb(0xc0, 0xc8, 0xd8), rgb(0x78, 0x80, 0x98)),
    (rgb(0xe8, 0xb4, 0x54), rgb(0xa4, 0x72, 0x2c)),
    (rgb(0xec, 0xf6, 0xff), rgb(0xa2, 0xb8, 0xd4)),
    (rgb(0xa0, 0xf6, 0xfa), rgb(0x3c, 0xac, 0xcc)),
    (rgb(0xff, 0xe0, 0x50), rgb(0xd0, 0x88, 0x14)),
];

const NIGHT: Rgba = rgb(0x24, 0x1c, 0x3c);
const STEEL: Rgba = rgb(0xaa, 0xb2, 0xc6);
const GOLD: Rgba = rgb(0xe6, 0xb0, 0x3c);
const BRASS: Rgba = rgb(0xd0, 0xa0, 0x50);

fn rarity_rgb(r: Rarity) -> Rgba {
    let c = r.color();
    rgb(c[0], c[1], c[2])
}

fn shade(c: Rgba) -> Rgba {
    mix(c, NIGHT, 0.42)
}

fn glow_for(r: Rarity) -> Option<Rgba> {
    (r >= Rarity::Epic).then(|| rarity_rgb(r))
}

/// A tool or weapon: the metal comes from the knife or tool tier, the accent
/// from the item's rarity.
fn held_piece(shape: Held, tier: u8, rarity: Option<Rarity>) -> Piece<Held> {
    let (base, dark) = METALS[usize::from(tier).min(METALS.len() - 1)];
    let accent = rarity.map_or(BRASS, rarity_rgb);
    Piece {
        shape,
        colors: [base, dark, accent],
        glow: rarity
            .and_then(glow_for)
            .or_else(|| (tier >= 4).then_some(base)),
    }
}

fn cloth(r: Rarity) -> Colors {
    let main = mix(rarity_rgb(r), NIGHT, 0.34);
    [
        main,
        shade(main),
        mix(rarity_rgb(r), rgb(255, 255, 255), 0.35),
    ]
}

fn plated(r: Rarity) -> Colors {
    let main = mix(STEEL, rarity_rgb(r), 0.34);
    [main, shade(main), rarity_rgb(r)]
}

fn armor_colors(shape: Armor, r: Rarity) -> Colors {
    match shape {
        Armor::Plate | Armor::Helmet | Armor::Shield => plated(r),
        Armor::Crown => {
            let main = mix(GOLD, rarity_rgb(r), 0.2);
            [main, shade(main), rarity_rgb(r)]
        }
        Armor::Cloak | Armor::Robe | Armor::Vest | Armor::Hood | Armor::Hat | Armor::Mask => {
            cloth(r)
        }
    }
}

/// The word that gives an item its look: its first word, if it is the name of
/// an object made for that slot.
fn shape_word<'a>(data: &GameData, item: &'a Item) -> &'a str {
    item.words
        .first()
        .filter(|word| {
            data.lexicon.find(word).is_some_and(|entry| {
                entry.tag_value("slot").and_then(Slot::from_tag) == Some(item.slot)
            })
        })
        .map_or("", String::as_str)
}

impl Gear {
    /// What a new hero carries: a spear and nothing else.
    pub fn starting() -> Gear {
        Gear {
            held: held_piece(Held::Spear, 1, None),
            back: None,
            armor: None,
            boots: None,
            lamp: None,
            charm: None,
        }
    }

    pub fn of(data: &GameData, player: &PlayerData) -> Gear {
        let worn = |slot: Slot| {
            player
                .equipped(slot)
                .map(|item| (shape_word(data, item), item.rarity))
        };
        // The knife tier is the metal of whatever the knife hand wields. With
        // no weapon, the knife is mounted on a spear.
        let weapon = worn(Slot::Weapon);
        let blade = player.tool_tier(ToolKind::Knife);
        let wielded = held_piece(
            weapon
                .and_then(|(word, _)| weapon_for(word))
                .unwrap_or(match weapon {
                    Some(_) => Held::Dagger,
                    None => Held::Spear,
                }),
            blade,
            weapon.map(|(_, rarity)| rarity),
        );
        let tool = |shape| held_piece(shape, player.tool_tier(player.active_tool), None);
        let (held, back) = match player.active_tool {
            ToolKind::Knife => (wielded, None),
            ToolKind::Hammer => (tool(Held::Hammer), Some(wielded)),
            ToolKind::Lamp => (tool(Held::Lantern), Some(wielded)),
            ToolKind::Lens => (tool(Held::Lens), Some(wielded)),
        };
        Gear {
            held,
            back,
            armor: worn(Slot::Armor).map(|(word, r)| {
                // A plain item of this slot is named «Capa».
                let shape = armor_for(word).unwrap_or(Armor::Cloak);
                Piece {
                    shape,
                    colors: armor_colors(shape, r),
                    glow: glow_for(r),
                }
            }),
            boots: worn(Slot::Boots).map(|(word, r)| {
                let shape = footwear_for(word).unwrap_or(Footwear::Boots);
                Piece {
                    shape,
                    colors: if shape == Footwear::Greaves {
                        plated(r)
                    } else {
                        cloth(r)
                    },
                    glow: glow_for(r),
                }
            }),
            lamp: worn(Slot::Lamp).map(|(word, r)| {
                let main = mix(BRASS, rarity_rgb(r), 0.4);
                Piece {
                    shape: lamp_for(word).unwrap_or(LampGear::Lantern),
                    colors: [main, shade(main), rarity_rgb(r)],
                    glow: glow_for(r),
                }
            }),
            charm: worn(Slot::Charm).map(|(word, r)| {
                let main = mix(GOLD, rarity_rgb(r), 0.25);
                Piece {
                    shape: charm_for(word).unwrap_or(Charm::Pendant),
                    colors: [main, shade(main), rarity_rgb(r)],
                    glow: glow_for(r),
                }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use babel_core::progression::items::from_words;

    fn data() -> GameData {
        GameData::built_in().expect("data")
    }

    #[test]
    fn every_equipment_word_has_a_shape_of_its_own_family() {
        let data = data();
        let mut unmapped = Vec::new();
        for entry in data.lexicon.entries() {
            let known = match entry.tag_value("slot").and_then(Slot::from_tag) {
                Some(Slot::Weapon) => weapon_for(&entry.word).is_some(),
                Some(Slot::Armor) => armor_for(&entry.word).is_some(),
                Some(Slot::Boots) => footwear_for(&entry.word).is_some(),
                Some(Slot::Lamp) => lamp_for(&entry.word).is_some(),
                // Charms without a special shape hang from the neck.
                Some(Slot::Charm) | None => true,
            };
            if !known {
                unmapped.push(entry.word.clone());
            }
        }
        assert!(unmapped.is_empty(), "no shape for {unmapped:?}");
    }

    #[test]
    fn the_hand_shows_the_weapon_or_the_tool_in_use() {
        let data = data();
        let mut player = PlayerData::default();
        assert_eq!(Gear::of(&data, &player).held.shape, Held::Spear);
        player.receive_item(from_words(&data, 1, "hacha", None, Rarity::Rare));
        let armed = Gear::of(&data, &player);
        assert_eq!(armed.held.shape, Held::Axe);
        assert_eq!(armed.back, None);

        player.tools[ToolKind::Knife.index()] = 5;
        let golden = Gear::of(&data, &player);
        assert_ne!(golden.held.colors, armed.held.colors, "tier sets the metal");

        player.active_tool = ToolKind::Hammer;
        let mining = Gear::of(&data, &player);
        assert_eq!(mining.held.shape, Held::Hammer);
        assert_eq!(mining.back.map(|b| b.shape), Some(Held::Axe));
    }

    #[test]
    fn every_slot_and_rarity_shows() {
        let data = data();
        let mut player = PlayerData::default();
        for (uid, word) in ["capucha", "sandalia", "pebetero", "anillo"]
            .into_iter()
            .enumerate()
        {
            player.receive_item(from_words(
                &data,
                uid as u64 + 1,
                word,
                None,
                Rarity::Common,
            ));
        }
        let plain = Gear::of(&data, &player);
        assert_eq!(plain.armor.map(|p| p.shape), Some(Armor::Hood));
        assert_eq!(plain.boots.map(|p| p.shape), Some(Footwear::Sandals));
        assert_eq!(plain.lamp.map(|p| p.shape), Some(LampGear::Brazier));
        assert_eq!(plain.charm.map(|p| p.shape), Some(Charm::Ring));
        assert_eq!(plain.armor.and_then(|p| p.glow), None);

        player.equipment[Slot::Armor as usize] =
            Some(from_words(&data, 9, "capucha", None, Rarity::Legendary));
        let fine = Gear::of(&data, &player);
        assert_ne!(fine.armor.map(|p| p.colors), plain.armor.map(|p| p.colors));
        assert!(fine.armor.and_then(|p| p.glow).is_some());
    }
}
