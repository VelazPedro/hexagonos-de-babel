//! Procedural book body: gibberish mixed with lexicon words at rate `λ(depth)`.

use super::alphabet;
use super::lexicon::{Category, Lexicon, WordId};
use super::seeds::{Dice, Seed};

pub const PAGES: usize = 3;
pub const LINES_PER_PAGE: usize = 40;
pub const LINE_WIDTH: usize = 80;

/// Token-level probability of emitting a lexicon word, by depth.
///
/// A body holds about 1500 tokens, so these small rates are what yields the
/// design target of 4–8 detected words on floor 1 and 20–35 on floor 30
/// (see the distribution test in `world`).
pub fn word_rate(depth: u32) -> f32 {
    const LOW: f32 = 0.0031;
    const HIGH: f32 = 0.0156;
    let d = depth.max(1);
    if d <= 30 {
        LOW + (HIGH - LOW) * ((d - 1) as f32 / 29.0)
    } else {
        // Infinite mode: legibility keeps creeping up but stays bounded.
        HIGH + 0.002 * (1.0 - 1.0 / (1.0 + (d - 30) as f32 / 20.0))
    }
}

/// Chance that an emitted word becomes an adjacent noun + quality phrase.
pub const PHRASE_RATE: f32 = 0.05;

/// Chance that an emitted word is replaced by a whole canonical phrase.
pub fn canon_rate(depth: u32) -> f32 {
    0.0008 + 0.00003 * depth.min(40) as f32
}

/// Sampling tables over the lexicon, weighted inversely to base rarity.
#[derive(Clone, Debug)]
pub struct WordSampler {
    cumulative: Vec<u32>,
    nouns: Vec<(WordId, u32)>,
    qualities: Vec<(WordId, u32)>,
}

fn rarity_weight(base_rarity: u8) -> u32 {
    match base_rarity {
        0 | 1 => 20,
        2 => 12,
        3 => 6,
        4 => 3,
        _ => 1,
    }
}

fn category_weight(c: Category) -> u32 {
    match c {
        Category::Bioma => 2,
        Category::Orientacion | Category::Luz => 2,
        Category::Criatura => 2,
        _ => 1,
    }
}

fn pick_cumulative(dice: &mut Dice, table: &[(WordId, u32)]) -> Option<WordId> {
    let total = table.last()?.1;
    let roll = dice.below(total);
    let i = table.partition_point(|&(_, c)| c <= roll);
    table.get(i).map(|&(w, _)| w)
}

impl WordSampler {
    pub fn new(lexicon: &Lexicon) -> Self {
        let mut cumulative = Vec::with_capacity(lexicon.len());
        let mut nouns = Vec::new();
        let mut qualities = Vec::new();
        let (mut total, mut noun_total, mut quality_total) = (0u32, 0u32, 0u32);
        for (i, e) in lexicon.entries().iter().enumerate() {
            let w = rarity_weight(e.base_rarity);
            total += w * category_weight(e.category);
            cumulative.push(total);
            if e.category.is_noun_for_phrase() {
                noun_total += w;
                nouns.push((i as WordId, noun_total));
            }
            if e.category == Category::Cualidad {
                quality_total += w;
                qualities.push((i as WordId, quality_total));
            }
        }
        WordSampler {
            cumulative,
            nouns,
            qualities,
        }
    }

    pub fn word(&self, dice: &mut Dice) -> Option<WordId> {
        let total = *self.cumulative.last()?;
        let roll = dice.below(total);
        Some(self.cumulative.partition_point(|&c| c <= roll) as WordId)
    }

    pub fn phrase(&self, dice: &mut Dice) -> Option<(WordId, WordId)> {
        let noun = pick_cumulative(dice, &self.nouns)?;
        let quality = pick_cumulative(dice, &self.qualities)?;
        Some((noun, quality))
    }
}

const VOWELS: [u8; 5] = [0, 4, 8, 15, 21];
/// Letters that commonly follow a vowel in Spanish (s, n, r, l, d).
const COMMON_CODAS: [u8; 5] = [19, 13, 18, 11, 3];

fn gibberish(dice: &mut Dice, out: &mut String) {
    let len = dice.range(1, 9);
    let mut previous: Option<u8> = None;
    for _ in 0..len {
        let s = match previous {
            Some(p) if !VOWELS.contains(&p) && dice.chance(0.30) => VOWELS[dice.index(5)],
            Some(p) if VOWELS.contains(&p) && dice.chance(0.20) => COMMON_CODAS[dice.index(5)],
            // 27 letters: indices 0..=26 skip the three separators.
            _ => dice.below(27) as u8,
        };
        previous = Some(s);
        out.push(alphabet::symbol(s));
    }
}

struct Typesetter {
    lines: Vec<String>,
    line: String,
    width: usize,
}

impl Typesetter {
    fn full(&self) -> bool {
        self.lines.len() >= PAGES * LINES_PER_PAGE
    }

    fn break_line(&mut self) {
        let mut line = std::mem::take(&mut self.line);
        line.extend(std::iter::repeat_n(' ', LINE_WIDTH - self.width));
        self.lines.push(line);
        self.width = 0;
    }

    /// Places an unbreakable token, followed by `suffix` punctuation.
    fn place(&mut self, token: &str, suffix: &str) {
        let len = token.chars().count() + suffix.len();
        let needed = if self.width == 0 { len } else { len + 1 };
        if self.width + needed > LINE_WIDTH {
            self.break_line();
        }
        if self.width > 0 {
            self.line.push(' ');
            self.width += 1;
        }
        self.line.push_str(token);
        self.line.push_str(suffix);
        self.width += len;
    }
}

/// Generates `PAGES` pages of `LINES_PER_PAGE` lines of exactly `LINE_WIDTH` symbols.
pub fn generate(
    seed: &Seed,
    depth: u32,
    lexicon: &Lexicon,
    sampler: &WordSampler,
    canon: &[String],
) -> Vec<Vec<String>> {
    let mut dice = Dice::labelled(seed, "text");
    let rate = word_rate(depth);
    let mut setter = Typesetter {
        lines: Vec::with_capacity(PAGES * LINES_PER_PAGE),
        line: String::new(),
        width: 0,
    };
    let mut token = String::new();
    while !setter.full() {
        token.clear();
        if dice.chance(rate) {
            if dice.chance(canon_rate(depth)) {
                if let Some(phrase) = dice.pick(canon) {
                    token.push_str(phrase);
                }
            } else if dice.chance(PHRASE_RATE) {
                if let Some((noun, quality)) = sampler.phrase(&mut dice) {
                    token.push_str(&lexicon.get(noun).word);
                    token.push(' ');
                    token.push_str(&lexicon.get(quality).word);
                }
            } else if let Some(w) = sampler.word(&mut dice) {
                token.push_str(&lexicon.get(w).word);
            }
        }
        if token.is_empty() {
            gibberish(&mut dice, &mut token);
        }
        let roll = dice.below(100);
        let suffix = match roll {
            0..=7 => ",",
            8..=12 => ".",
            _ => "",
        };
        setter.place(&token, suffix);
    }
    setter.lines.truncate(PAGES * LINES_PER_PAGE);
    setter
        .lines
        .chunks(LINES_PER_PAGE)
        .map(<[String]>::to_vec)
        .collect()
}
