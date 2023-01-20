use crate::emoji::Emoji;

/// Does not remove emoji.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// A re-implementation of [euphoria's nick hue hashing algorithm][0].
///
/// [0]: https://github.com/euphoria-io/heim/blob/master/client/lib/hueHash.js
fn hue_hash(text: &str, offset: i64) -> u8 {
    let mut val = 0_i32;
    for bibyte in text.encode_utf16() {
        let char_val = (bibyte as i32).wrapping_mul(439) % 256;
        val = val.wrapping_mul(33).wrapping_add(char_val);
    }

    let val: i64 = val as i64 + 2_i64.pow(31);
    ((val + offset) % 255) as u8
}

const GREENIE_OFFSET: i64 = 148 - 192; // 148 - hue_hash("greenie", 0)

/// Calculate the nick hue without removing colon-delimited emoji as part of
/// normalization.
///
/// This should be slightly faster than [`nick_hue`] but produces incorrect
/// results if any colon-delimited emoji are present.
pub fn nick_hue_without_removing_emoji(nick: &str) -> u8 {
    let normalized = normalize(nick);
    if normalized.is_empty() {
        hue_hash(nick, GREENIE_OFFSET)
    } else {
        hue_hash(&normalized, GREENIE_OFFSET)
    }
}

pub fn nick_hue(emoji: &Emoji, nick: &str) -> u8 {
    nick_hue_without_removing_emoji(&emoji.remove(nick))
}
