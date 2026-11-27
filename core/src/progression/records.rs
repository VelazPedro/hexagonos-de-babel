//! What each character has done along the way. It travels with them: in co-op
//! each one keeps their own on their computer.

use crate::babel::body_gen::PAGES;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The most books whose reading is remembered.
pub const MAX_OPENED: usize = 20_000;
/// Far more canonical phrases than the Library has.
const MAX_CANONS: usize = 500;
/// Counts stay far below anything that could overflow.
const MAX_COUNT: u32 = 100_000_000;
const MAX_TOTAL: u64 = 1_000_000_000_000;
const MAX_SECONDS: u64 = 1_000_000_000;

/// The counts. They are small and travel on their own to a partner, so that a
/// blow does not send everything else again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tally {
    pub play_seconds: u64,
    pub deepest_floor: u32,
    pub hexagons: u32,
    pub falls: u32,
    pub enemies: u32,
    pub elites: u32,
    pub guardians: u32,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub books: u32,
    pub pages: u32,
    /// Canonical phrases found, each counted once.
    pub canons: u32,
    pub materials: u64,
    pub broken: u32,
    pub crafted: u32,
    pub improved: u32,
}

impl Tally {
    /// The same counts within what a game can reach.
    pub fn tidy(self) -> Tally {
        let count = |n: u32| n.min(MAX_COUNT);
        Tally {
            play_seconds: self.play_seconds.min(MAX_SECONDS),
            deepest_floor: count(self.deepest_floor),
            hexagons: count(self.hexagons),
            falls: count(self.falls),
            enemies: count(self.enemies),
            elites: count(self.elites),
            guardians: count(self.guardians),
            damage_dealt: self.damage_dealt.min(MAX_TOTAL),
            damage_taken: self.damage_taken.min(MAX_TOTAL),
            books: count(self.books),
            pages: count(self.pages),
            canons: count(self.canons),
            materials: self.materials.min(MAX_TOTAL),
            broken: count(self.broken),
            crafted: count(self.crafted),
            improved: count(self.improved),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Records {
    /// False until what an older save already knew has been taken.
    pub begun: bool,
    pub tally: Tally,
    /// The books read, by a number of their own, and how many pages of each.
    pub opened: BTreeMap<u64, u8>,
    /// Which canonical phrases have been found.
    pub found_canons: BTreeSet<u16>,
}

impl Records {
    /// A book counts once; a better lens adds only the pages it shows anew.
    pub fn read_book(&mut self, book: u64, pages: u8) {
        let seen = self.opened.get(&book).copied();
        let new = pages.saturating_sub(seen.unwrap_or(0));
        if seen.is_none() {
            self.tally.books = self.tally.books.saturating_add(1);
            // Past the limit a book is no longer remembered.
            if self.opened.len() >= MAX_OPENED {
                self.tally.pages = self.tally.pages.saturating_add(u32::from(new));
                return;
            }
        }
        if new > 0 {
            self.tally.pages = self.tally.pages.saturating_add(u32::from(new));
            self.opened.insert(book, pages);
        }
    }

    /// A canonical phrase counts the first time it is read.
    pub fn find_canon(&mut self, canon: usize) {
        let Ok(canon) = u16::try_from(canon) else {
            return;
        };
        if self.found_canons.len() < MAX_CANONS && self.found_canons.insert(canon) {
            self.tally.canons = self.tally.canons.saturating_add(1);
        }
    }

    /// Brings values that came from elsewhere within limits.
    pub fn tidy(&mut self) {
        self.tally = self.tally.tidy();
        while self.opened.len() > MAX_OPENED {
            self.opened.pop_last();
        }
        while self.found_canons.len() > MAX_CANONS {
            self.found_canons.pop_last();
        }
        for pages in self.opened.values_mut() {
            *pages = (*pages).min(PAGES as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_data;
    use crate::progression::player::PlayerData;

    #[test]
    fn a_book_counts_once_and_a_better_lens_adds_only_new_pages() {
        let mut r = Records::default();
        let counted = |r: &Records| (r.tally.books, r.tally.pages);
        r.read_book(7, 1);
        assert_eq!(counted(&r), (1, 1));
        r.read_book(7, 1);
        assert_eq!(counted(&r), (1, 1), "read again");
        r.read_book(7, 3);
        assert_eq!(counted(&r), (1, 3), "a better lens");
        r.read_book(7, 2);
        assert_eq!(counted(&r), (1, 3), "a worse one");
        r.read_book(8, 2);
        assert_eq!(counted(&r), (2, 5));
    }

    #[test]
    fn old_saves_load_without_records_and_edited_ones_are_clamped() {
        let old: PlayerData = ron::from_str("(level: 4)").expect("old player data");
        assert_eq!(old.records, Records::default());
        let mut player = PlayerData::default();
        player.records.tally = Tally {
            play_seconds: u64::MAX,
            enemies: u32::MAX,
            damage_dealt: u64::MAX,
            ..Tally::default()
        };
        for book in 0..(MAX_OPENED as u64 + 50) {
            player.records.opened.insert(book, 200);
        }
        let tidy = player.sanitized(test_data()).records;
        assert!(tidy.tally.play_seconds <= 1_000_000_000);
        assert!(tidy.tally.enemies <= 100_000_000);
        assert!(tidy.tally.damage_dealt <= 1_000_000_000_000);
        assert_eq!(tidy.opened.len(), MAX_OPENED);
        assert!(tidy.opened.values().all(|&p| p <= 3));
    }
}
