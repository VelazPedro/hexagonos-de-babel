use super::floors;
use super::site::{self, SiteKey};
use crate::babel::body_gen;
use crate::babel::feistel::Feistel;
use crate::data::test_data;
use std::time::Instant;

fn check(world_seed: u64, key: &SiteKey) -> site::Site {
    let s = site::generate(test_data(), world_seed, key);
    if let Err(e) = s.validate() {
        panic!("seed {world_seed} {key:?} ({:?}): {e}", s.layout);
    }
    s
}

#[test]
fn same_site_is_byte_identical() {
    for (seed, key) in [
        (1, SiteKey::walk(1, 0, 0)),
        (42, SiteKey::walk(7, -3, 9)),
        (9_999, SiteKey::walk(30, 12, -4)),
    ] {
        let a = ron::to_string(&check(seed, &key)).expect("serialize");
        let b = ron::to_string(&check(seed, &key)).expect("serialize");
        assert_eq!(a, b);
    }
}

#[test]
fn book_has_the_canonical_shape() {
    let s = check(5, &SiteKey::walk(3, 1, 1));
    assert_eq!(s.book.inscription.chars().count(), 80);
    assert_eq!(s.book.pages.len(), body_gen::PAGES);
    for page in &s.book.pages {
        assert_eq!(page.len(), body_gen::LINES_PER_PAGE);
        for line in page {
            assert_eq!(line.chars().count(), body_gen::LINE_WIDTH);
            assert!(line.chars().all(|c| crate::babel::alphabet::index_of(c).is_some()));
        }
    }
}

#[test]
fn home_is_safe_and_open() {
    let s = check(11, &SiteKey::walk(1, 0, 0));
    assert!(s.enemies.is_empty());
    assert!(s.exits.iter().all(|&e| e));
    assert!(s.stairs.is_none());
}

#[test]
fn catalog_phrase_shapes_the_remote_site() {
    let data = test_data();
    let feistel = Feistel::new(77);
    let phrase = "lobo sombrio hielo laberinto";
    let key = site::key_for_phrase(&feistel, phrase, 12).expect("valid phrase");
    assert!(key.is_remote());
    let s = check(77, &key);
    assert!(s.book.inscription.starts_with(phrase));
    assert!(s.analysis.hits.iter().all(|h| h.rarity <= crate::babel::rarity::Rarity::Epic));
    let hielo = data.biome_index("hielo");
    let found = s
        .analysis
        .hits
        .iter()
        .any(|h| data.lexicon.get(h.word).word == "hielo");
    assert!(found);
    // The inscription weighs triple, so a lone body word cannot outvote it easily.
    assert!(s.recipe.biome == hielo || s.analysis.hits.len() > 4);
}

#[test]
fn guardian_floors_hold_a_guardian_at_the_stairs() {
    for floor in [5, 10, 15, 20, 25, 30, 35] {
        let seed = 123;
        let (a, b) = floors::stair_offset(seed, floor);
        let s = check(seed, &SiteKey::walk(floor, a, b));
        assert!(s.stairs.is_some(), "floor {floor}");
        let g = s.guardian.expect("guardian");
        assert!(s.enemies.is_empty());
        if floor <= 30 {
            assert_eq!(test_data().enemies.guardians[g.index].floor, floor);
        }
    }
}

#[test]
fn many_sites_pass_validation() {
    for i in 0..150u64 {
        let floor = 1 + (i % 33) as u32;
        let q = (i % 11) as i32 - 5;
        let r = (i % 7) as i32 - 3;
        check(i.wrapping_mul(31), &SiteKey::walk(floor, q, r));
    }
}

#[test]
fn no_cluster_of_sites_is_ever_sealed() {
    use super::hex::{self, Dir};
    let data = test_data();
    for seed in [1u64, 31679, 77] {
        let feistel = Feistel::new(seed);
        for start in [(0, 0), (50, -100), (-7, 13)] {
            // Following only each site's own doors must keep finding new sites.
            let mut seen = std::collections::HashSet::from([start]);
            let mut stack = vec![start];
            while let Some((q, r)) = stack.pop() {
                if seen.len() > 80 {
                    break;
                }
                let s = site::generate_with(data, &feistel, seed, &SiteKey::walk(22, q, r));
                for d in Dir::ALL {
                    let next = hex::neighbor(q, r, d);
                    if s.exits[d.index()] && seen.insert(next) {
                        stack.push(next);
                    }
                }
            }
            assert!(seen.len() > 80, "seed {seed} {start:?}: sealed at {}", seen.len());
        }
    }
}

/// Headless stress test; run in release: `cargo test -p babel_core --release -- --ignored`.
#[test]
#[ignore = "stress test, run in release"]
fn stress_ten_thousand_sites() {
    let data = test_data();
    const TOTAL: usize = 10_000;
    let mut words_by_floor = vec![(0usize, 0usize); 61];
    let mut biomes = vec![0usize; data.biomes.len()];
    let mut layouts = std::collections::BTreeMap::new();
    let (mut elites, mut canon, mut calm) = (0usize, 0usize, 0usize);
    let mut slowest = 0u128;
    let start = Instant::now();
    for i in 0..TOTAL {
        let floor = 1 + (i % 60) as u32;
        let q = ((i / 60) % 41) as i32 - 20;
        let r = ((i / 2460) % 17) as i32 - 8;
        let world_seed = 1_000 + (i / 97) as u64;
        let t = Instant::now();
        let s = check(world_seed, &SiteKey::walk(floor, q, r));
        slowest = slowest.max(t.elapsed().as_micros());
        let slot = &mut words_by_floor[floor as usize];
        slot.0 += s.analysis.hits.len();
        slot.1 += 1;
        biomes[s.recipe.biome] += 1;
        *layouts.entry(format!("{:?}", s.layout)).or_insert(0) += 1;
        elites += s.enemies.iter().filter(|e| e.elite.is_some()).count();
        canon += s.analysis.canons.len();
        calm += usize::from(s.recipe.calm);
    }
    let mean_ms = start.elapsed().as_secs_f64() * 1000.0 / TOTAL as f64;
    let mean = |f: usize| words_by_floor[f].0 as f32 / words_by_floor[f].1.max(1) as f32;
    println!("mean {mean_ms:.3} ms per site, slowest {} ms", slowest / 1000);
    for f in [1, 5, 10, 15, 20, 25, 30, 45, 60] {
        println!("floor {f:2}: {:.1} words", mean(f));
    }
    println!("biomes {biomes:?}");
    println!("layouts {layouts:?}");
    println!("elites {elites}, canon {canon}, calm {calm}");
    assert!(mean_ms < 30.0, "mean generation time {mean_ms} ms");
    assert!((4.0..=8.0).contains(&mean(1)), "floor 1: {}", mean(1));
    assert!((20.0..=35.0).contains(&mean(30)), "floor 30: {}", mean(30));
    for f in 2..=30 {
        assert!((3.5..=35.0).contains(&mean(f)), "floor {f}: {}", mean(f));
    }
    for f in 31..=60 {
        assert!((20.0..=40.0).contains(&mean(f)), "floor {f}: {}", mean(f));
    }
    assert!(biomes.iter().all(|&n| n > 0), "every biome appears: {biomes:?}");
    assert!(elites > 0 && canon > 0);
}

#[test]
fn word_counts_follow_depth() {
    let data = test_data();
    let feistel = Feistel::new(3);
    let average = |floor: u32| {
        let total: usize = (0..60)
            .map(|i| {
                site::generate_with(data, &feistel, 3, &SiteKey::walk(floor, i, -i))
                    .analysis
                    .hits
                    .len()
            })
            .sum();
        total as f32 / 60.0
    };
    let (shallow, deep) = (average(1), average(30));
    assert!((4.0..=8.0).contains(&shallow), "floor 1: {shallow}");
    assert!((20.0..=35.0).contains(&deep), "floor 30: {deep}");
}
