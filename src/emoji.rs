//! All emoji the vanilla euphoria.io client knows.

use std::collections::HashMap;

const EMOJI_RAW: &str = include_str!("emoji.txt");

/// A map from emoji names to their unicode representation. Not all emojis have
/// such a representation.
pub struct Emoji(pub HashMap<String, Option<String>>);

fn parse_hex_to_char(hex: &str) -> char {
    u32::from_str_radix(hex, 16).unwrap().try_into().unwrap()
}

fn parse_line(line: &str) -> (String, Option<String>) {
    let mut line = line.split_ascii_whitespace();
    let name = line.next().unwrap().to_string();
    let unicode = line.map(parse_hex_to_char).collect::<String>();
    let unicode = Some(unicode).filter(|u| !u.is_empty());
    (name, unicode)
}

impl Emoji {
    pub fn load() -> Self {
        let map = EMOJI_RAW
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(parse_line)
            .collect();
        Self(map)
    }
}

#[cfg(test)]
mod test {
    use super::Emoji;

    #[test]
    fn load_without_panic() {
        Emoji::load();
    }
}
