//! Punctuation spellings stay ordered before their prefixes for longest-match scanning.

// First-byte dispatch, the enum variants, and diagnostic spellings come from one table.
// Policy operators (`+%`, `+|`, `+!`, etc.) are single tokens. Right shift has no token
// of its own: the lexer emits two `>`, and the parser joins two that touch and reads any
// suffix touching the second so nested static argument lists close one `>` at a time
// (wiki/spec/language/expressions.md).
macro_rules! define_punctuation {
    ($($first:literal => [$(($spelling:literal, $kind:ident)),* $(,)?]),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PunctuationKind {
            $($($kind,)*)*
        }

        impl PunctuationKind {
            #[cfg(test)]
            fn ordered_lexemes() -> &'static [(&'static str, Self)] {
                &[$($(($spelling, Self::$kind),)*)*]
            }

            pub fn matching_prefix(source: &[u8]) -> Option<(&'static str, Self)> {
                match *source.first()? {
                    $($first => {
                        $(if source.starts_with($spelling.as_bytes()) {
                            return Some(($spelling, Self::$kind));
                        })*
                        None
                    },)*
                    _ => None,
                }
            }

            pub fn lexeme(self) -> &'static str {
                match self {
                    $($(Self::$kind => $spelling,)*)*
                }
            }
        }
    };
}

define_punctuation! {
    b'+' => [
        ("+%", PlusWrapping),
        ("+|", PlusSaturating),
        ("+!", PlusTrapping),
        ("+=", PlusEqual),
        ("+", Plus),
    ],
    b'-' => [
        ("-%", MinusWrapping),
        ("-|", MinusSaturating),
        ("-!", MinusTrapping),
        ("->", Arrow),
        ("-=", MinusEqual),
        ("-", Minus),
    ],
    b'*' => [
        ("*%", AsteriskWrapping),
        ("*|", AsteriskSaturating),
        ("*!", AsteriskTrapping),
        ("*=", AsteriskEqual),
        ("*", Asterisk),
    ],
    b'/' => [
        ("/%", SlashWrapping),
        ("/|", SlashSaturating),
        ("/!", SlashTrapping),
        ("/=", SlashEqual),
        ("/", Slash),
    ],
    b'%' => [
        ("%%", PercentWrapping),
        ("%|", PercentSaturating),
        ("%!", PercentTrapping),
        ("%=", PercentEqual),
        ("%", Percent),
    ],
    b'<' => [
        ("<<%", LessLessWrapping),
        ("<<|", LessLessSaturating),
        ("<<!", LessLessTrapping),
        ("<<", LessLess),
        ("<=", LessEqual),
        ("<", Less),
    ],
    b':' => [
        ("::", ColonColon),
        (":", Colon),
    ],
    b'.' => [
        ("..=", DotDotEqual),
        ("..", DotDot),
        (".", Dot),
    ],
    b'=' => [
        ("==", EqualEqual),
        ("=", Equal),
    ],
    b'!' => [
        ("!=", ExclamationEqual),
        ("!", Exclamation),
    ],
    b'>' => [
        (">=", GreaterEqual),
        (">", Greater),
    ],
    b'\\' => [("\\", Backslash)],
    b'$' => [("$", Dollar)],
    b'@' => [("@", At)],
    b'&' => [
        ("&&", AndAnd),
        ("&", Ampersand),
    ],
    b'|' => [
        ("||", PipePipe),
        ("|", Pipe),
    ],
    b'\'' => [("'", Apostrophe)],
    b'^' => [("^", Caret)],
    b',' => [(",", Comma)],
    b'#' => [("#", Hash)],
    b'{' => [("{", LeftBrace)],
    b'[' => [("[", LeftBracket)],
    b'(' => [("(", LeftParen)],
    b'}' => [("}", RightBrace)],
    b']' => [("]", RightBracket)],
    b')' => [(")", RightParen)],
    b';' => [(";", Semicolon)],
    b'~' => [("~", Tilde)],
}

#[cfg(test)]
mod tests {
    use super::PunctuationKind;

    #[test]
    fn every_punctuation_spelling_reaches_its_longest_match() {
        for (lexeme, punctuation) in PunctuationKind::ordered_lexemes() {
            assert_eq!(
                PunctuationKind::matching_prefix(lexeme.as_bytes()),
                Some((*lexeme, *punctuation)),
            );
        }
    }
}
