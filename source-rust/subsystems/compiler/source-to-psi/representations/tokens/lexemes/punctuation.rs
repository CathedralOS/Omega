//! Punctuation spellings stay ordered before their prefixes for longest-match scanning.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PunctuationKind {
    Ampersand,
    AndAnd,
    Apostrophe,
    At,
    Arrow,
    Asterisk,
    Backslash,
    Caret,
    Colon,
    ColonColon,
    Comma,
    Dollar,
    Dot,
    DotDot,
    DotDotEqual,
    Equal,
    EqualEqual,
    Exclamation,
    ExclamationEqual,
    Greater,
    GreaterEqual,
    Hash,
    LeftBrace,
    LeftBracket,
    LeftParen,
    Less,
    LessEqual,
    LessLess,
    Minus,
    Percent,
    Pipe,
    PipePipe,
    Plus,
    PlusEqual,
    MinusEqual,
    AsteriskEqual,
    SlashEqual,
    PercentEqual,
    RightBrace,
    RightBracket,
    RightParen,
    Semicolon,
    Slash,
    Tilde,
    /*
        Policy operators: an arithmetic or left-shift operator with a
        `%` (Wrapping), `|` (Saturating) or `!` (Trapping) suffix is one
        token. Right shift has no token of its own: the lexer emits two
        `>`, and the parser joins two that touch and reads the suffix that
        touches the second, so a nested static argument list closes one
        `>` at a time (wiki/spec/language/expressions.md).
    */
    PlusWrapping,
    PlusSaturating,
    PlusTrapping,
    MinusWrapping,
    MinusSaturating,
    MinusTrapping,
    AsteriskWrapping,
    AsteriskSaturating,
    AsteriskTrapping,
    SlashWrapping,
    SlashSaturating,
    SlashTrapping,
    PercentWrapping,
    PercentSaturating,
    PercentTrapping,
    LessLessWrapping,
    LessLessSaturating,
    LessLessTrapping,
    #[default]
    Unknown,
}

// First-byte dispatch and the ordered vocabulary come from the same spellings.
macro_rules! define_punctuation {
    ($($first:literal => [$(($spelling:literal, $kind:ident)),* $(,)?]),* $(,)?) => {
        impl PunctuationKind {
            pub fn ordered_lexemes() -> &'static [(&'static str, Self)] {
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
                    Self::Unknown => "<unknown>",
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
    b'\'' => [
        ("'", Apostrophe),
    ],
    b'^' => [
        ("^", Caret),
    ],
    b',' => [
        (",", Comma),
    ],
    b'#' => [
        ("#", Hash),
    ],
    b'{' => [
        ("{", LeftBrace),
    ],
    b'[' => [
        ("[", LeftBracket),
    ],
    b'(' => [
        ("(", LeftParen),
    ],
    b'}' => [
        ("}", RightBrace),
    ],
    b']' => [
        ("]", RightBracket),
    ],
    b')' => [
        (")", RightParen),
    ],
    b';' => [
        (";", Semicolon),
    ],
    b'~' => [
        ("~", Tilde),
    ],
}
