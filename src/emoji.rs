//! All emoji the euphoria.leet.nu client knows.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::RangeInclusive;

/// Euphoria.leet.nu emoji list, obtainable via shell command:
///
/// ```bash
/// curl 'https://euphoria.leet.nu/static/emoji.json' \
///   | jq 'to_entries | sort_by(.key) | from_entries' \
///   > emoji.json
/// ```
const EMOJI_JSON: &str = include_str!("emoji.json");

/// A map from emoji names to their unicode representation. Not all emojis have
/// such a representation.
pub struct Emoji(pub HashMap<String, Option<String>>);

fn parse_hex_to_char(hex: &str) -> Option<char> {
    u32::from_str_radix(hex, 16).ok()?.try_into().ok()
}

fn parse_code_points(code_points: &str) -> Option<String> {
    code_points
        .split('-')
        .map(parse_hex_to_char)
        .collect::<Option<String>>()
}

impl Emoji {
    /// Load a list of emoji compiled into the library.
    pub fn load() -> Self {
        Self::load_from_json(EMOJI_JSON).unwrap()
    }

    /// Load a list of emoji from a string containing a JSON object.
    ///
    /// The object keys are the emoji names (without colons `:`). The object
    /// values are the emoji code points encoded as hexadecimal numbers and
    /// separated by a dash `-` (e.g. `"34-fe0f-20e3"`). Emojis whose values
    /// don't match this schema are interpreted as emojis without unicode
    /// representation.
    pub fn load_from_json(json: &str) -> Option<Self> {
        let map = serde_json::from_str::<HashMap<String, String>>(json)
            .ok()?
            .into_iter()
            .map(|(k, v)| (k, parse_code_points(&v)))
            .collect::<HashMap<_, _>>();

        Some(Self(map))
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

    pub fn replace<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let emoji = self.find(text);
        if emoji.is_empty() {
            return Cow::Borrowed(text);
        }

        let mut result = String::new();

        let mut after_last_emoji = 0;
        for (range, replace) in emoji {
            // Only replace emoji with a replacement
            if let Some(replace) = replace {
                if *range.start() > after_last_emoji {
                    // There were non-emoji characters between the last and the
                    // current emoji.
                    result.push_str(&text[after_last_emoji..*range.start()]);
                }
                result.push_str(replace);
                after_last_emoji = range.end() + 1;
            }
        }

        if after_last_emoji < text.len() {
            result.push_str(&text[after_last_emoji..]);
        }

        Cow::Owned(result)
    }

    pub fn remove<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let emoji = self.find(text);
        if emoji.is_empty() {
            return Cow::Borrowed(text);
        }

        let mut result = String::new();

        let mut after_last_emoji = 0;
        for (range, _) in emoji {
            if *range.start() > after_last_emoji {
                // There were non-emoji characters between the last and the
                // current emoji.
                result.push_str(&text[after_last_emoji..*range.start()]);
            }
            after_last_emoji = range.end() + 1;
        }

        if after_last_emoji < text.len() {
            result.push_str(&text[after_last_emoji..]);
        }

        Cow::Owned(result)
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

    #[test]
    fn replace() {
        let emoji = Emoji::load();
        assert_eq!(emoji.replace("no:emo:ji:here"), "no:emo:ji:here");
        assert_eq!(emoji.replace(":bad:x:o:"), ":bad❌o:");
        assert_eq!(emoji.replace(":x:bad:o:"), "❌bad⭕");
        assert_eq!(emoji.replace("ab:bad:x:o:cd"), "ab:bad❌o:cd");
        assert_eq!(emoji.replace("ab:x:bad:o:cd"), "ab❌bad⭕cd");
        assert_eq!(emoji.replace("chᴜm:crown::ant:"), "chᴜm👑🐜");
        assert_eq!(
            emoji.replace(":waning_crescent_moon: (2% full)"),
            "🌘 (2% full)"
        );
        assert_eq!(emoji.replace("Jan-20 17:58 Z"), "Jan-20 17:58 Z");
    }

    #[test]
    fn remove() {
        let emoji = Emoji::load();
        assert_eq!(emoji.remove("no:emo:ji:here"), "no:emo:ji:here");
        assert_eq!(emoji.remove(":bad:x:o:"), ":bado:");
        assert_eq!(emoji.remove(":x:bad:o:"), "bad");
        assert_eq!(emoji.remove("ab:bad:x:o:cd"), "ab:bado:cd");
        assert_eq!(emoji.remove("ab:x:bad:o:cd"), "abbadcd");
        assert_eq!(emoji.remove("chᴜm:crown::ant:"), "chᴜm");
        assert_eq!(
            emoji.remove(":waning_crescent_moon: (2% full)"),
            " (2% full)"
        );
        assert_eq!(emoji.remove("Jan-20 17:58 Z"), "Jan-20 17:58 Z");
    }
}
