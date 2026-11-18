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
    `<`. `longest_match` returns the first entry that matches, so the order of
    this list *is* the longest-match rule. The test at the bottom of this file
    fails if an entry is ever placed after one of its own prefixes.

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

/// For each ASCII byte, the half-open range of `ORDERED_SPELLINGS` indices whose spellings begin
/// with that byte. A byte that begins no punctuation has the empty range `(0, 0)`.
const FIRST_BYTE_RANGES: [(u8, u8); 128] = build_first_byte_ranges();

const fn build_first_byte_ranges() -> [(u8, u8); 128] {
    let mut ranges = [(0u8, 0u8); 128];
    let ordered_spellings = Punctuation::ORDERED_SPELLINGS;
    let mut spelling_index = 0;
    while spelling_index < ordered_spellings.len() {
        let first_byte = ordered_spellings[spelling_index].0.as_bytes()[0] as usize;
        if ranges[first_byte].1 == 0 {
            ranges[first_byte].0 = spelling_index as u8;
        }
        // Spellings that share a first byte sit together in the list, so the range is tight. If
        // one were ever misplaced the range would only widen; `longest_match` would stay correct.
        ranges[first_byte].1 = spelling_index as u8 + 1;
        spelling_index += 1;
    }
    ranges
}

impl Punctuation {
    /// Finds the longest punctuation spelling that `remaining_bytes` starts with.
    ///
    /// We look only at the spellings that share the first byte, at most six of them. Walking the
    /// whole list instead cost most of the lexer's time: a third of all tokens are punctuation, and
    /// the commonest ones (`;`, `,`, brackets) are at the end of the list. See EXPERIMENTS.md.
    pub fn longest_match(remaining_bytes: &[u8]) -> Option<Punctuation> {
        let &first_byte = remaining_bytes.first()?;
        let &(range_start, range_end) = FIRST_BYTE_RANGES.get(usize::from(first_byte))?;
        Self::ORDERED_SPELLINGS
            .get(usize::from(range_start)..usize::from(range_end))?
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

    #[test]
    fn first_byte_ranges_hold_exactly_the_spellings_that_start_with_that_byte() {
        for first_byte in 0u8..128 {
            let (range_start, range_end) = super::FIRST_BYTE_RANGES[usize::from(first_byte)];
            for (spelling_index, (spelling, _)) in Punctuation::ORDERED_SPELLINGS.iter().enumerate()
            {
                let starts_with_this_byte = spelling.as_bytes()[0] == first_byte;
                let is_in_range =
                    (usize::from(range_start)..usize::from(range_end)).contains(&spelling_index);
                assert_eq!(
                    starts_with_this_byte, is_in_range,
                    "`{spelling}` and the range for byte {first_byte:#04x} disagree"
                );
            }
        }
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
