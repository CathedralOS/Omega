//! The closed punctuation vocabulary.

// One list generates the enum, the lookup table and the spelling of each kind, so they cannot disagree.
macro_rules! define_punctuation {
    ($(($spelling:literal, $kind:ident)),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Punctuation {
            $($kind,)*
        }

        impl Punctuation {
            /// Every punctuation spelling, in the order `longest_match` tries them.
            pub const ORDERED_SPELLINGS: &'static [(&'static str, Punctuation)] = &[
                $(($spelling, Punctuation::$kind),)*
            ];

            // @Speed: `self` by value on purpose. With `&self` the whole lexer measured about 2
            // percent slower with function alignment held equal; the scanner asks every
            // punctuation token for the length of its spelling. See experiment 8 in EXPERIMENTS.md.
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
    a faster hand-written form of that search and is tested against it. A lexer test also fails if an entry is ever placed after one
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
    ("\\", Backslash),
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
    ("$", Dollar),
    ("@", At),
    (",", Comma),
    (";", Semicolon),
    ("(", LeftParenthesis),
    (")", RightParenthesis),
    ("[", LeftBracket),
    ("]", RightBracket),
    ("{", LeftBrace),
    ("}", RightBrace),
}
