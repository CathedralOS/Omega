//! The closed punctuation vocabulary and longest-match lookup over it.

// One list generates the enum, the lookup table and the spelling of each kind, so they cannot disagree.
macro_rules! define_punctuation {
    ($(($spelling:literal, $kind:ident)),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Punctuation {
            $($kind,)*
        }

        impl Punctuation {
            /// Every punctuation spelling, in the order `longest_match` tries them.
            pub const ORDERED_SPELLINGS: &'static [(&'static str, Punctuation)] = &[
                $(($spelling, Punctuation::$kind),)*
            ];

            pub fn get_spelling(self) -> &'static str {
                match self {
                    $(Self::$kind => $spelling,)*
                }
            }
        }
    };
}

/*
    :LongestMatchOrder
    Within each group that shares a first character, a longer spelling must
    come before any spelling that is a prefix of it: `<<%` before `<<` before
    `<`. The right answer for any input is the first entry here that matches,
    so the order of this list *is* the longest-match rule. `longest_match` is
    a faster hand-written form of that search and is tested against it. A test
    at the bottom of this file also fails if an entry is ever placed after one
    of its own prefixes.

    The `%`, `|` and `!` suffixed forms are the policy operators (Wrapping,
    Saturating, Trapping). The spec says "lexing takes the longest operator, so
    `a +!b` is a trapping add". There is no `>>` here on purpose: the spec says
    right shift "keeps its two adjacent `>` tokens ... no `>>` token is added",
    which lets `Vec<Vec<u8>>` close one `>` at a time.
*/
define_punctuation! {
    ("+%", PlusWrapping),
    ("+|", PlusSaturating),
    ("+!", PlusTrapping),
    ("+=", PlusEqual),
    ("+", Plus),
    ("-%", MinusWrapping),
    ("-|", MinusSaturating),
    ("-!", MinusTrapping),
    ("->", Arrow),
    ("-=", MinusEqual),
    ("-", Minus),
    ("*%", AsteriskWrapping),
    ("*|", AsteriskSaturating),
    ("*!", AsteriskTrapping),
    ("*=", AsteriskEqual),
    ("*", Asterisk),
    ("/%", SlashWrapping),
    ("/|", SlashSaturating),
    ("/!", SlashTrapping),
    ("/=", SlashEqual),
    ("/", Slash),
    ("%%", PercentWrapping),
    ("%|", PercentSaturating),
    ("%!", PercentTrapping),
    ("%=", PercentEqual),
    ("%", Percent),
    ("<<%", LessLessWrapping),
    ("<<|", LessLessSaturating),
    ("<<!", LessLessTrapping),
    ("<<", LessLess),
    ("<=", LessEqual),
    ("<", Less),
    (">=", GreaterEqual),
    (">", Greater),
    ("==", EqualEqual),
    ("=", Equal),
    ("!=", ExclamationEqual),
    ("!", Exclamation),
    ("&&", AmpersandAmpersand),
    ("&", Ampersand),
    ("||", PipePipe),
    ("|", Pipe),
    ("::", ColonColon),
    (":", Colon),
    ("..=", DotDotEqual),
    ("..", DotDot),
    (".", Dot),
    ("'", Apostrophe),
    ("^", Caret),
    ("~", Tilde),
    ("#", Hash),
    (",", Comma),
    (";", Semicolon),
    ("(", LeftParenthesis),
    (")", RightParenthesis),
    ("[", LeftBracket),
    ("]", RightBracket),
    ("{", LeftBrace),
    ("}", RightBrace),
}

impl Punctuation {
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
        Self::match_leading_bytes(first_byte, second_byte, third_byte)
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
            (b'+', b'%') => Self::PlusWrapping,
            (b'+', b'|') => Self::PlusSaturating,
            (b'+', b'!') => Self::PlusTrapping,
            (b'+', b'=') => Self::PlusEqual,
            (b'+', _) => Self::Plus,
            (b'-', b'%') => Self::MinusWrapping,
            (b'-', b'|') => Self::MinusSaturating,
            (b'-', b'!') => Self::MinusTrapping,
            (b'-', b'>') => Self::Arrow,
            (b'-', b'=') => Self::MinusEqual,
            (b'-', _) => Self::Minus,
            (b'*', b'%') => Self::AsteriskWrapping,
            (b'*', b'|') => Self::AsteriskSaturating,
            (b'*', b'!') => Self::AsteriskTrapping,
            (b'*', b'=') => Self::AsteriskEqual,
            (b'*', _) => Self::Asterisk,
            (b'/', b'%') => Self::SlashWrapping,
            (b'/', b'|') => Self::SlashSaturating,
            (b'/', b'!') => Self::SlashTrapping,
            (b'/', b'=') => Self::SlashEqual,
            (b'/', _) => Self::Slash,
            (b'%', b'%') => Self::PercentWrapping,
            (b'%', b'|') => Self::PercentSaturating,
            (b'%', b'!') => Self::PercentTrapping,
            (b'%', b'=') => Self::PercentEqual,
            (b'%', _) => Self::Percent,
            (b'<', b'<') => match third_byte {
                b'%' => Self::LessLessWrapping,
                b'|' => Self::LessLessSaturating,
                b'!' => Self::LessLessTrapping,
                _ => Self::LessLess,
            },
            (b'<', b'=') => Self::LessEqual,
            (b'<', _) => Self::Less,
            (b'>', b'=') => Self::GreaterEqual,
            (b'>', _) => Self::Greater,
            (b'=', b'=') => Self::EqualEqual,
            (b'=', _) => Self::Equal,
            (b'!', b'=') => Self::ExclamationEqual,
            (b'!', _) => Self::Exclamation,
            (b'&', b'&') => Self::AmpersandAmpersand,
            (b'&', _) => Self::Ampersand,
            (b'|', b'|') => Self::PipePipe,
            (b'|', _) => Self::Pipe,
            (b':', b':') => Self::ColonColon,
            (b':', _) => Self::Colon,
            (b'.', b'.') => match third_byte {
                b'=' => Self::DotDotEqual,
                _ => Self::DotDot,
            },
            (b'.', _) => Self::Dot,
            (b'\'', _) => Self::Apostrophe,
            (b'^', _) => Self::Caret,
            (b'~', _) => Self::Tilde,
            (b'#', _) => Self::Hash,
            (b',', _) => Self::Comma,
            (b';', _) => Self::Semicolon,
            (b'(', _) => Self::LeftParenthesis,
            (b')', _) => Self::RightParenthesis,
            (b'[', _) => Self::LeftBracket,
            (b']', _) => Self::RightBracket,
            (b'{', _) => Self::LeftBrace,
            (b'}', _) => Self::RightBrace,
            _ => return None,
        })
    }

    /// The definition `longest_match` must agree with: the first entry of `ORDERED_SPELLINGS`
    /// that `remaining_bytes` starts with. Slow, obviously right, and only run by the tests.
    #[cfg(test)]
    fn longest_match_by_search(remaining_bytes: &[u8]) -> Option<Punctuation> {
        Self::ORDERED_SPELLINGS
            .iter()
            .find(|(spelling, _)| remaining_bytes.starts_with(spelling.as_bytes()))
            .map(|&(_, punctuation)| punctuation)
    }
}

#[cfg(test)]
mod tests {
    use super::Punctuation;

    // :LongestMatchOrder
    #[test]
    fn no_spelling_follows_one_of_its_own_prefixes() {
        let ordered_spellings = Punctuation::ORDERED_SPELLINGS;
        for (later_index, (later_spelling, _)) in ordered_spellings.iter().enumerate() {
            for (earlier_spelling, _) in &ordered_spellings[..later_index] {
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
            assert_eq!(
                Punctuation::longest_match(spelling.as_bytes()),
                Some(punctuation)
            );
            assert_eq!(punctuation.get_spelling(), spelling);
        }
    }

    /// Every byte that occurs in some spelling, plus bytes that occur in none: a letter, a digit,
    /// a space, an unused symbol, zero (the stand-in for end of source) and two non-ASCII bytes.
    fn get_distinguishing_bytes() -> Vec<u8> {
        let mut bytes: Vec<u8> = Punctuation::ORDERED_SPELLINGS
            .iter()
            .flat_map(|(spelling, _)| spelling.bytes())
            .collect();
        bytes.extend_from_slice(b"a0 @\x00\x80\xFF");
        bytes.sort_unstable();
        bytes.dedup();
        bytes
    }

    #[test]
    fn the_hand_written_match_agrees_with_a_search_of_the_spelling_list() {
        // No spelling is longer than three bytes, so inputs of up to four bytes built from the
        // distinguishing bytes cover every way the two functions could disagree.
        assert!(
            Punctuation::ORDERED_SPELLINGS
                .iter()
                .all(|(spelling, _)| spelling.len() <= 3)
        );

        let bytes = get_distinguishing_bytes();
        let mut checked_input_count = 0usize;
        let mut check = |input: &[u8]| {
            assert_eq!(
                Punctuation::longest_match(input),
                Punctuation::longest_match_by_search(input),
                "the match and the search disagree on {input:?}"
            );
            checked_input_count += 1;
        };

        check(&[]);
        for &first in &bytes {
            check(&[first]);
            for &second in &bytes {
                check(&[first, second]);
                for &third in &bytes {
                    check(&[first, second, third]);
                    // A fourth byte must never change the answer; one sample is enough to show it is ignored.
                    check(&[first, second, third, b'=']);
                }
            }
        }
        assert!(
            checked_input_count > 50_000,
            "only {checked_input_count} inputs were checked"
        );
    }

    #[test]
    fn bytes_that_start_no_punctuation_match_nothing() {
        assert_eq!(Punctuation::longest_match(b""), None);
        assert_eq!(Punctuation::longest_match(b"a"), None);
        assert_eq!(Punctuation::longest_match(b"@"), None);
        assert_eq!(Punctuation::longest_match(&[0xC3, 0xA9]), None);
    }

    #[test]
    fn right_shift_is_not_a_token() {
        assert_eq!(
            Punctuation::longest_match(b">>"),
            Some(Punctuation::Greater)
        );
    }
}
