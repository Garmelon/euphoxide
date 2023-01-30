//! Nick-related utility functions.

use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

use crate::emoji::Emoji;

/// Does not remove emoji.
fn hue_normalize(text: &str) -> String {
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

/// Calculate a nick's hue like [`hue`] but without removing colon-delimited
/// emoji as part of normalization.
///
/// This should be slightly faster than [`hue`] but produces incorrect results
/// if any colon-delimited emoji are present.
pub fn hue_without_removing_emoji(nick: &str) -> u8 {
    let normalized = hue_normalize(nick);
    if normalized.is_empty() {
        hue_hash(nick, GREENIE_OFFSET)
    } else {
        hue_hash(&normalized, GREENIE_OFFSET)
    }
}

/// Calculate a nick's hue.
///
/// This is a reimplementation of [euphoria's nick hue hashing algorithm][0]. It
/// should always return the same value as the official client's implementation.
///
/// [0]: https://github.com/euphoria-io/heim/blob/978c921063e6b06012fc8d16d9fbf1b3a0be1191/client/lib/hueHash.js
pub fn hue(emoji: &Emoji, nick: &str) -> u8 {
    hue_without_removing_emoji(&emoji.remove(nick))
}

fn delimits_mention(c: char) -> bool {
    match c {
        ',' | '.' | '!' | '?' | ';' | '&' | '<' | '>' | '\'' | '"' => true,
        _ => c.is_whitespace(),
    }
}

/// Normalize a nick to a form that can be compared against other nicks.
///
/// This normalization is less aggressive than the nick hue normalization. It is
/// also less aggressive than the normalization used by the euphoria client to
/// determine who is pinged by a mention. This means that it will not compute
/// the same normal form for all pairs of nicks that ping each other in the
/// euphoria client.
///
/// A nick and its mention form calculated via [`mention`] will always evaluate
/// to the same normal form.
///
/// The steps performed are as follows:
///
/// 1. Remove all whitespace characters as well as the characters
///    `,`, `.`, `!`, `?`, `;`, `&`, `<`, `>`, `'`, `"`.
///    All of these are used by the [frontend to delimit mentions][0].
///    The `>` character was confirmed to delimit mentions via experimentation.
///
/// 2. Convert to NFKC
/// 3. Case fold
///
/// Steps 2 and 3 are meant to be an alternative to the NKFC_Casefold derived
/// property that's easier to implement, even though it may be incorrect in some
/// edge cases.
///
/// [0]: https://github.com/euphoria-io/heim/blob/978c921063e6b06012fc8d16d9fbf1b3a0be1191/client/lib/stores/chat.js#L14
pub fn normalize(nick: &str) -> String {
    nick.chars()
        .filter(|c| !delimits_mention(*c)) // Step 1
        .nfkc() // Step 2
        .default_case_fold() // Step 3
        .collect()
}

/// Compute a mentionable version of a nick while remaining as close to the
/// original as possible.
///
/// The return value of this function appended to an `@` character will
/// highlight as a mention in the official euphoria client. It should ping any
/// people using the original nick. It might also ping other people.
///
/// This function performs step 1 of [`normalize`].
pub fn mention(nick: &str) -> String {
    nick.replace(delimits_mention, "")
}
