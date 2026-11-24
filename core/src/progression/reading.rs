//! Reading a site's book with the Lens and filling the Glossary.

use super::player::{GlossEntry, PlayerData};
use super::stats::Derived;
use crate::babel::body_gen;
use crate::babel::detect::{Analysis, Place};
use crate::babel::lexicon::WordId;
use crate::babel::rarity::Rarity;
use crate::data::{GameData, Stat, ToolKind, Unlock};

/// Body pages the current lens can resolve (the inscription is always legible).
pub fn readable_pages(player: &PlayerData, derived: &Derived) -> usize {
    if player.has_relic("lente_profunda") {
        return body_gen::PAGES;
    }
    let from_lens = match player.tool_tier(ToolKind::Lens) {
        0 => 1,
        1 | 2 => 2,
        _ => 3,
    };
    (from_lens + usize::from(derived.has(Unlock::ExtraPage))).min(body_gen::PAGES)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadResult {
    pub new_words: Vec<(WordId, Rarity)>,
    pub improved: Vec<(WordId, Rarity)>,
    pub xp: u32,
}

/// Registers every word legible with `pages` body pages.
pub fn read(
    data: &GameData,
    analysis: &Analysis,
    player: &mut PlayerData,
    derived: &Derived,
    pages: usize,
) -> ReadResult {
    let mut best: Vec<(WordId, Rarity, bool)> = Vec::new();
    for h in &analysis.hits {
        let visible = match h.place {
            Place::Inscription => true,
            Place::Body { page, .. } => (page as usize) < pages,
        };
        if !visible {
            continue;
        }
        match best.iter_mut().find(|b| b.0 == h.word) {
            Some(b) => {
                b.1 = b.1.max(h.rarity);
                b.2 |= h.in_phrase;
            }
            None => best.push((h.word, h.rarity, h.in_phrase)),
        }
    }
    let bonus = 1.0 + derived.stat(Stat::ReadBonus);
    let mut result = ReadResult::default();
    let mut xp = 0.0;
    for (word, rarity, in_phrase) in best {
        let text = data.lexicon.get(word).word.clone();
        let insight = if in_phrase && derived.has(Unlock::PhraseInsight) {
            2.0
        } else {
            1.0
        };
        match player.glossary.get_mut(&text) {
            Some(entry) => {
                entry.times += 1;
                if rarity > entry.rarity {
                    entry.rarity = rarity;
                    xp += 4.0 * (1 + rarity.tier()) as f32 * insight;
                    result.improved.push((word, rarity));
                }
            }
            None => {
                player
                    .glossary
                    .insert(text, GlossEntry { rarity, times: 1 });
                xp += (4 + 5 * rarity.tier()) as f32 * insight;
                result.new_words.push((word, rarity));
            }
        }
    }
    result.xp = (xp * bonus).round() as u32;
    result
}
