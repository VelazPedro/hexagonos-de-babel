//! Word, phrase and canonical-phrase detection (Aho-Corasick, whole words only).

use super::body_gen;
use super::lexicon::{Category, Lexicon, WordId};
use super::rarity::{self, Context, Rarity};
use aho_corasick::AhoCorasick;
use serde::{Deserialize, Serialize};

/// A site's book: the bijective inscription and the visible body pages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Book {
    pub inscription: String,
    pub pages: Vec<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    Inscription,
    Body { page: u8, line: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub word: WordId,
    pub place: Place,
    /// Byte offsets inside the line.
    pub start: u16,
    pub end: u16,
    pub in_phrase: bool,
    pub canonical: bool,
    pub rarity: Rarity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phrase {
    pub noun: WordId,
    pub quality: WordId,
    /// The text exactly as it appears ("lobo sombrio").
    pub text: String,
    pub rarity: Rarity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonHit {
    pub canon: usize,
    pub place: Place,
    pub start: u16,
    pub end: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    pub hits: Vec<Hit>,
    pub phrases: Vec<Phrase>,
    pub canons: Vec<CanonHit>,
}

impl Analysis {
    /// Distinct words with the best rarity each reached, in first-seen order.
    pub fn distinct(&self) -> Vec<(WordId, Rarity)> {
        let mut out: Vec<(WordId, Rarity)> = Vec::new();
        for h in &self.hits {
            match out.iter_mut().find(|(w, _)| *w == h.word) {
                Some(slot) => slot.1 = slot.1.max(h.rarity),
                None => out.push((h.word, h.rarity)),
            }
        }
        out
    }
}

pub struct Detector {
    words: Option<AhoCorasick>,
    canon: Option<AhoCorasick>,
    canon_len: usize,
}

fn is_boundary(byte: Option<&u8>) -> bool {
    matches!(byte, None | Some(b' ' | b',' | b'.'))
}

/// Whole-word matches of `automaton` in `line`: `(pattern, start, end)` ordered by start.
fn whole_matches(automaton: &AhoCorasick, line: &str) -> Vec<(usize, usize, usize)> {
    let bytes = line.as_bytes();
    let mut out: Vec<(usize, usize, usize)> = automaton
        .find_overlapping_iter(line)
        .filter(|m| {
            let before = m.start().checked_sub(1).and_then(|i| bytes.get(i));
            is_boundary(before) && is_boundary(bytes.get(m.end()))
        })
        .map(|m| (m.pattern().as_usize(), m.start(), m.end()))
        .collect();
    out.sort_by_key(|&(_, s, e)| (s, e));
    out
}

fn phrase_shape(a: Category, b: Category) -> Option<bool> {
    if a.is_noun_for_phrase() && b == Category::Cualidad {
        Some(true)
    } else if a == Category::Cualidad && b.is_noun_for_phrase() {
        Some(false)
    } else {
        None
    }
}

fn only_spaces(gap: &str) -> bool {
    !gap.is_empty() && gap.bytes().all(|b| b == b' ')
}

impl Detector {
    pub fn new(lexicon: &Lexicon, canon: &[String]) -> Self {
        Detector {
            words: AhoCorasick::new(lexicon.entries().iter().map(|e| e.word.as_str())).ok(),
            canon: AhoCorasick::new(canon).ok(),
            canon_len: canon.len(),
        }
    }

    pub fn analyze_line(
        &self,
        lexicon: &Lexicon,
        line: &str,
        place: Place,
        depth: u32,
        out: &mut Analysis,
    ) {
        let Some(words) = &self.words else { return };
        let found = whole_matches(words, line);
        if found.is_empty() {
            return;
        }
        let canons: Vec<(usize, usize, usize)> = match &self.canon {
            Some(a) if self.canon_len > 0 => whole_matches(a, line),
            _ => Vec::new(),
        };
        for &(canon, start, end) in &canons {
            out.canons.push(CanonHit {
                canon,
                place,
                start: start as u16,
                end: end as u16,
            });
        }

        // Pair up adjacent words that form a valid Spanish phrase.
        let mut in_phrase = vec![false; found.len()];
        let mut pairs = Vec::new();
        let mut i = 0;
        while i + 1 < found.len() {
            let (wa, sa, ea) = found[i];
            let (wb, sb, eb) = found[i + 1];
            let shape = phrase_shape(
                lexicon.get(wa as WordId).category,
                lexicon.get(wb as WordId).category,
            );
            match shape {
                Some(noun_first) if sb > ea && only_spaces(&line[ea..sb]) => {
                    in_phrase[i] = true;
                    in_phrase[i + 1] = true;
                    let (noun, quality) = if noun_first { (wa, wb) } else { (wb, wa) };
                    pairs.push((i, noun as WordId, quality as WordId, sa, eb));
                    i += 2;
                }
                _ => i += 1,
            }
        }

        let first_hit = out.hits.len();
        for (k, &(w, start, end)) in found.iter().enumerate() {
            let entry = lexicon.get(w as WordId);
            let canonical = canons.iter().any(|&(_, s, e)| start >= s && end <= e);
            let ctx = Context {
                in_phrase: in_phrase[k],
                in_inscription: place == Place::Inscription,
                canonical,
                depth,
            };
            out.hits.push(Hit {
                word: w as WordId,
                place,
                start: start as u16,
                end: end as u16,
                in_phrase: in_phrase[k],
                canonical,
                rarity: rarity::rarity_of(entry.base_rarity, &entry.word, ctx),
            });
        }
        for (k, noun, quality, start, end) in pairs {
            let a = out.hits[first_hit + k].rarity;
            let b = out.hits[first_hit + k + 1].rarity;
            out.phrases.push(Phrase {
                noun,
                quality,
                text: line[start..end].split_whitespace().collect::<Vec<_>>().join(" "),
                rarity: a.max(b),
            });
        }
    }

    pub fn analyze(&self, lexicon: &Lexicon, book: &Book, depth: u32) -> Analysis {
        let mut out = Analysis::default();
        self.analyze_line(lexicon, &book.inscription, Place::Inscription, depth, &mut out);
        for (p, page) in book.pages.iter().enumerate().take(body_gen::PAGES) {
            for (l, line) in page.iter().enumerate() {
                let place = Place::Body {
                    page: p as u8,
                    line: l as u8,
                };
                self.analyze_line(lexicon, line, place, depth, &mut out);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::babel::lexicon::Entry;

    fn lexicon() -> Lexicon {
        let mk = |w: &str, c, r| Entry {
            word: w.to_string(),
            category: c,
            tags: vec![],
            base_rarity: r,
            palette_hint: None,
        };
        Lexicon::new(vec![
            mk("lobo", Category::Criatura, 1),
            mk("sombrio", Category::Cualidad, 2),
            mk("sombra", Category::Luz, 1),
            mk("sal", Category::Material, 1),
            mk("salamandra", Category::Criatura, 3),
            mk("araña", Category::Criatura, 2),
        ])
    }

    fn run(line: &str) -> (Lexicon, Analysis) {
        let lex = lexicon();
        let canon = vec!["lobo sombrio sal".to_string()];
        let d = Detector::new(&lex, &canon);
        let mut a = Analysis::default();
        d.analyze_line(&lex, line, Place::Body { page: 0, line: 0 }, 1, &mut a);
        (lex, a)
    }

    fn words(lex: &Lexicon, a: &Analysis) -> Vec<String> {
        a.hits.iter().map(|h| lex.get(h.word).word.clone()).collect()
    }

    #[test]
    fn requires_word_boundaries() {
        let (lex, a) = run("salamandra xsal salx lobos");
        assert_eq!(words(&lex, &a), ["salamandra"]);
    }

    #[test]
    fn punctuation_and_edges_are_boundaries() {
        let (lex, a) = run("sal,lobo.araña");
        assert_eq!(words(&lex, &a), ["sal", "lobo", "araña"]);
    }

    #[test]
    fn detects_phrase_both_orders_but_not_across_punctuation() {
        assert_eq!(run("lobo sombrio").1.phrases.len(), 1);
        assert_eq!(run("sombrio   araña").1.phrases.len(), 1);
        assert_eq!(run("lobo, sombrio").1.phrases.len(), 0);
        assert_eq!(run("sal sombrio").1.phrases.len(), 0);
    }

    #[test]
    fn full_canonical_phrase_marks_hits() {
        let (_, a) = run("xx lobo sombrio sal xx");
        assert_eq!(a.canons.len(), 1);
        assert!(a.hits.iter().all(|h| h.rarity == Rarity::Canonical));
        let (_, partial) = run("lobo sombrio");
        assert!(partial.canons.is_empty());
    }

    #[test]
    fn empty_and_gibberish_lines_are_fine() {
        assert!(run("").1.hits.is_empty());
        assert!(run("    ,,,...  qwpoei").1.hits.is_empty());
    }
}
