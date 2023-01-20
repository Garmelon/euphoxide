//! All emoji the vanilla euphoria.io client knows.

use std::collections::HashMap;
use std::ops::RangeInclusive;

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

    pub fn get(&self, name: &str) -> Option<Option<&str>> {
        match self.0.get(name) {
            Some(Some(replace)) => Some(Some(replace)),
            Some(None) => Some(None),
            None => None,
        }
    }

    pub fn find(&self, text: &str) -> Vec<(RangeInclusive<usize>, Option<&str>)> {
        let mut result = vec![];

        let mut prev_colon_idx = None;
        for (colon_idx, _) in text.match_indices(':') {
            if let Some(prev_idx) = prev_colon_idx {
                let name = &text[prev_idx + 1..colon_idx];
                if let Some(replace) = self.get(name) {
                    let range = prev_idx..=colon_idx;
                    result.push((range, replace));
                    prev_colon_idx = None;
                    continue;
                }
            }
            prev_colon_idx = Some(colon_idx);
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::Emoji;

    #[test]
    fn load_without_panic() {
        Emoji::load();
    }

    #[test]
    fn find() {
        let emoji = Emoji::load();

        // :bad: does not exist, while :x: and :o: do.

        assert_eq!(emoji.find(":bad:x:o:"), vec![(4..=6, Some("❌"))]);
        assert_eq!(
            emoji.find(":x:bad:o:"),
            vec![(0..=2, Some("❌")), (6..=8, Some("⭕"))]
        );
        assert_eq!(emoji.find("ab:bad:x:o:cd"), vec![(6..=8, Some("❌"))]);
        assert_eq!(
            emoji.find("ab:x:bad:o:cd"),
            vec![(2..=4, Some("❌")), (8..=10, Some("⭕"))]
        );
    }
}
