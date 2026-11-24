//! Canonical 30-symbol alphabet: a–z, ñ, space, comma and period.

pub const SIZE: u32 = 30;

pub const SYMBOLS: [char; 30] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'ñ', 'o', 'p', 'q', 'r',
    's', 't', 'u', 'v', 'w', 'x', 'y', 'z', ' ', ',', '.',
];

pub const SPACE: u8 = 27;

pub fn index_of(c: char) -> Option<u8> {
    match c {
        'a'..='n' => Some(c as u8 - b'a'),
        'ñ' => Some(14),
        'o'..='z' => Some(c as u8 - b'o' + 15),
        ' ' => Some(27),
        ',' => Some(28),
        '.' => Some(29),
        _ => None,
    }
}

pub fn symbol(i: u8) -> char {
    SYMBOLS[(i as usize) % SYMBOLS.len()]
}

pub fn is_letter(c: char) -> bool {
    matches!(c, 'a'..='z' | 'ñ')
}

/// Maps arbitrary user input to the canonical alphabet (lowercase, no accents).
/// Unsupported characters are dropped.
pub fn normalize_char(c: char) -> Option<char> {
    let lower = c.to_lowercase().next().unwrap_or(c);
    let mapped = match lower {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        other => other,
    };
    index_of(mapped).map(|_| mapped)
}

pub fn normalize(text: &str) -> String {
    text.chars().filter_map(normalize_char).collect()
}

pub fn encode(text: &str) -> Option<Vec<u8>> {
    text.chars().map(index_of).collect()
}

pub fn decode(symbols: &[u8]) -> String {
    symbols.iter().map(|&s| symbol(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_symbol_roundtrip() {
        for (i, &c) in SYMBOLS.iter().enumerate() {
            assert_eq!(index_of(c), Some(i as u8), "{c}");
            assert_eq!(symbol(i as u8), c);
        }
    }

    #[test]
    fn normalizes_accents_and_drops_unknown() {
        assert_eq!(normalize("¡Árbol Ñandú 42!"), "arbol ñandu ");
    }
}
