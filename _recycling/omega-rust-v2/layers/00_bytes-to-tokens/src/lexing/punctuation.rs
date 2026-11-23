//! Longest-prefix punctuation lookup used by the scanner.
use omega_tokens::Punctuation;

/// Finds the longest punctuation spelling that `remaining_bytes` starts with.
///
/// This is `longest_match_by_search` written out by hand as a decision on the first three
/// bytes, so the compiler can turn it into jumps. `ORDERED_SPELLINGS` remains the definition
/// of what the answer is; a test checks this function against a search of that list for every
/// input that could tell them apart. A new spelling must be added in both places, and that
/// test fails if it is added to only one.
///
/// No spelling contains a zero byte, so zero stands in for "the source ended here".
pub fn longest_match(remaining_bytes: &[u8]) -> Option<Punctuation> {
    let first_byte = *remaining_bytes.first()?;
    let second_byte = remaining_bytes.get(1).copied().unwrap_or(0);
    let third_byte = remaining_bytes.get(2).copied().unwrap_or(0);

    match_leading_bytes(first_byte, second_byte, third_byte)
}

/// `longest_match` for a caller that already holds the next three bytes, with zero for any
/// byte past the end of the source. The scanner calls this directly so that choosing a
/// punctuation token costs no slice and no bounds check.
pub(crate) fn match_leading_bytes(
    first_byte: u8,
    second_byte: u8,
    third_byte: u8,
) -> Option<Punctuation> {
    Some(match (first_byte, second_byte) {
        (b'+', b'%') => Punctuation::PlusWrapping,
        (b'+', b'|') => Punctuation::PlusSaturating,
        (b'+', b'!') => Punctuation::PlusTrapping,
        (b'+', b'=') => Punctuation::PlusEqual,
        (b'+', _) => Punctuation::Plus,
        (b'-', b'%') => Punctuation::MinusWrapping,
        (b'-', b'|') => Punctuation::MinusSaturating,
        (b'-', b'!') => Punctuation::MinusTrapping,
        (b'-', b'>') => Punctuation::Arrow,
        (b'-', b'=') => Punctuation::MinusEqual,
        (b'-', _) => Punctuation::Minus,
        (b'*', b'%') => Punctuation::AsteriskWrapping,
        (b'*', b'|') => Punctuation::AsteriskSaturating,
        (b'*', b'!') => Punctuation::AsteriskTrapping,
        (b'*', b'=') => Punctuation::AsteriskEqual,
        (b'*', _) => Punctuation::Asterisk,
        (b'/', b'%') => Punctuation::SlashWrapping,
        (b'/', b'|') => Punctuation::SlashSaturating,
        (b'/', b'!') => Punctuation::SlashTrapping,
        (b'/', b'=') => Punctuation::SlashEqual,
        (b'/', _) => Punctuation::Slash,
        (b'\\', _) => Punctuation::Backslash,
        (b'%', b'%') => Punctuation::PercentWrapping,
        (b'%', b'|') => Punctuation::PercentSaturating,
        (b'%', b'!') => Punctuation::PercentTrapping,
        (b'%', b'=') => Punctuation::PercentEqual,
        (b'%', _) => Punctuation::Percent,
        (b'<', b'<') => match third_byte {
            b'%' => Punctuation::LessLessWrapping,
            b'|' => Punctuation::LessLessSaturating,
            b'!' => Punctuation::LessLessTrapping,
            _ => Punctuation::LessLess,
        },
        (b'<', b'=') => Punctuation::LessEqual,
        (b'<', _) => Punctuation::Less,
        (b'>', b'=') => Punctuation::GreaterEqual,
        (b'>', _) => Punctuation::Greater,
        (b'=', b'=') => Punctuation::EqualEqual,
        (b'=', _) => Punctuation::Equal,
        (b'!', b'=') => Punctuation::ExclamationEqual,
        (b'!', _) => Punctuation::Exclamation,
        (b'&', b'&') => Punctuation::AmpersandAmpersand,
        (b'&', _) => Punctuation::Ampersand,
        (b'|', b'|') => Punctuation::PipePipe,
        (b'|', _) => Punctuation::Pipe,
        (b':', b':') => Punctuation::ColonColon,
        (b':', _) => Punctuation::Colon,
        (b'.', b'.') => match third_byte {
            b'=' => Punctuation::DotDotEqual,
            _ => Punctuation::DotDot,
        },
        (b'.', _) => Punctuation::Dot,
        (b'\'', _) => Punctuation::Apostrophe,
        (b'^', _) => Punctuation::Caret,
        (b'~', _) => Punctuation::Tilde,
        (b'#', _) => Punctuation::Hash,
        (b'$', _) => Punctuation::Dollar,
        (b'@', _) => Punctuation::At,
        (b',', _) => Punctuation::Comma,
        (b';', _) => Punctuation::Semicolon,
        (b'(', _) => Punctuation::LeftParenthesis,
        (b')', _) => Punctuation::RightParenthesis,
        (b'[', _) => Punctuation::LeftBracket,
        (b']', _) => Punctuation::RightBracket,
        (b'{', _) => Punctuation::LeftBrace,
        (b'}', _) => Punctuation::RightBrace,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // :LongestMatchOrder
    #[test]
    fn no_spelling_follows_one_of_its_own_prefixes() {
        let ordered_spellings = Punctuation::ORDERED_SPELLINGS;

        for (later_spelling_index, (later_spelling, _)) in ordered_spellings.iter().enumerate() {
            for (earlier_spelling, _) in &ordered_spellings[..later_spelling_index] {
                assert!(
                    !later_spelling.starts_with(earlier_spelling),
                    "`{later_spelling}` is listed after its prefix `{earlier_spelling}` and could never match"
                );
            }
        }
    }

    #[test]
    fn every_spelling_matches_itself_and_round_trips() {
        for &(spelling, punctuation) in Punctuation::ORDERED_SPELLINGS {
            assert_eq!(longest_match(spelling.as_bytes()), Some(punctuation));
            assert_eq!(punctuation.get_spelling(), spelling);
        }
    }

    #[test]
    fn every_lookahead_window_matches_the_longest_declared_spelling() {
        // A three-byte lookahead can distinguish every spelling in the vocabulary.
        let vocabulary = Punctuation::ORDERED_SPELLINGS;
        assert!(vocabulary.iter().all(|(text, _)| (1..=3).contains(&text.len())));
        for first in u8::MIN..=u8::MAX {
            let row: Vec<_> = vocabulary
                .iter()
                .filter(|(text, _)| text.as_bytes()[0] == first)
                .collect();
            let single = row
                .iter()
                .find(|(text, _)| text.len() == 1)
                .map(|(_, kind)| *kind);
            assert_eq!(longest_match(&[first]), single);
            for second in u8::MIN..=u8::MAX {
                let pair = [first, second];
                let double = row
                    .iter()
                    .find(|(text, _)| text.as_bytes() == pair)
                    .map(|(_, kind)| *kind);
                let prefix = double.or(single);
                assert_eq!(longest_match(&pair), prefix);
                let mut expected = [prefix; 256];
                for (text, kind) in &row {
                    if let Some([third]) = text.as_bytes().strip_prefix(pair.as_slice()) {
                        expected[usize::from(*third)] = Some(*kind);
                    }
                }
                for third in u8::MIN..=u8::MAX {
                    assert_eq!(
                        match_leading_bytes(first, second, third),
                        expected[usize::from(third)],
                        "incorrect punctuation at {first:02x} {second:02x} {third:02x}"
                    );
                }
            }
        }
    }

    #[test]
    fn bytes_that_start_no_punctuation_match_nothing() {
        assert_eq!(longest_match(b""), None);
        assert_eq!(longest_match(b"a"), None);
        assert_eq!(longest_match(b"?"), None);
        assert_eq!(longest_match(&[0xC3, 0xA9]), None);
    }

    #[test]
    fn right_shift_is_not_a_token() {
        assert_eq!(longest_match(b">>"), Some(Punctuation::Greater));
    }
}
