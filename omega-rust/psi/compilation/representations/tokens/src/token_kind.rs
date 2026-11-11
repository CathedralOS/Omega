/*
We classify lexical tokens into identifiers, numeric literals, string literals,
keywords (`KeywordKind`), punctuation (`PunctuationKind`), whitespace, and
comments (`CommentKind`), ordering multi-character punctuation lexemes before
their prefixes for deterministic longest-match lexing.
*/

pub(crate) mod comment_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum CommentKind {
        Line,
        #[default]
        Block,
    }
}
pub(crate) mod keyword_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum KeywordKind {
        As,
        CallingConvention,
        Capability,
        Contains,
        Data,
        Else,
        False,
        Foreign,
        Host,
        If,
        Let,
        Library,
        Loop,
        Machine,
        Match,
        Owns,
        Platform,
        Public,
        Return,
        SelfValue,
        State,
        Struct,
        Target,
        Transition,
        True,
        Use,
        When,
        While,
        #[default]
        Unknown,
    }

    const KEYWORDS: &[(&str, KeywordKind)] = &[
        ("as", KeywordKind::As),
        ("calling_convention", KeywordKind::CallingConvention),
        ("capability", KeywordKind::Capability),
        ("contains", KeywordKind::Contains),
        ("data", KeywordKind::Data),
        ("else", KeywordKind::Else),
        ("false", KeywordKind::False),
        ("foreign", KeywordKind::Foreign),
        ("host", KeywordKind::Host),
        ("if", KeywordKind::If),
        ("let", KeywordKind::Let),
        ("library", KeywordKind::Library),
        ("loop", KeywordKind::Loop),
        ("machine", KeywordKind::Machine),
        ("match", KeywordKind::Match),
        ("owns", KeywordKind::Owns),
        ("platform", KeywordKind::Platform),
        ("public", KeywordKind::Public),
        ("return", KeywordKind::Return),
        ("self", KeywordKind::SelfValue),
        ("state", KeywordKind::State),
        ("struct", KeywordKind::Struct),
        ("target", KeywordKind::Target),
        ("transition", KeywordKind::Transition),
        ("true", KeywordKind::True),
        ("use", KeywordKind::Use),
        ("when", KeywordKind::When),
        ("while", KeywordKind::While),
    ];

    impl KeywordKind {
        pub fn from_lexeme(lexeme: &str) -> Option<Self> {
            KEYWORDS
                .binary_search_by(|(spelling, _)| (*spelling).cmp(lexeme))
                .ok()
                .map(|index| KEYWORDS[index].1)
        }

        pub fn lexeme(self) -> &'static str {
            match KEYWORDS.iter().find(|(_, kind)| *kind == self) {
                Some((spelling, _)) => spelling,
                None => "<unknown>",
            }
        }

        pub fn is_strict_identifier_keyword(self) -> bool {
            matches!(self, Self::SelfValue)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn keyword_spellings_round_trip_through_one_sorted_table() {
            assert!(
                KEYWORDS.windows(2).all(|pair| pair[0].0 < pair[1].0),
                "the keyword table must stay sorted for `from_lexeme`'s search"
            );
            for (spelling, keyword) in KEYWORDS {
                assert_eq!(KeywordKind::from_lexeme(spelling), Some(*keyword));
                assert_eq!(keyword.lexeme(), *spelling);
            }
            assert_eq!(KeywordKind::from_lexeme("not_a_keyword"), None);
            assert_eq!(KeywordKind::lexeme(KeywordKind::Unknown), "<unknown>");
            assert_eq!(KeywordKind::from_lexeme("<unknown>"), None);
        }
    }
}
pub(crate) mod numeric_literal_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NumericLiteralKind {
        Integer(IntegerLiteralKind),
        Float(FloatLiteralKind),
    }

    impl Default for NumericLiteralKind {
        fn default() -> Self {
            Self::Integer(IntegerLiteralKind::default())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct IntegerLiteralKind {
        pub base: NumericBase,
        pub empty_digits: bool,
        pub has_suffix: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct FloatLiteralKind {
        pub has_exponent: bool,
        pub empty_exponent: bool,
        pub has_suffix: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum NumericBase {
        Binary,
        Octal,
        #[default]
        Decimal,
        Hexadecimal,
    }
}
pub(crate) mod punctuation_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum PunctuationKind {
        Ampersand,
        AndAnd,
        Apostrophe,
        Arrow,
        Asterisk,
        Caret,
        Colon,
        ColonColon,
        Comma,
        Dot,
        DotDot,
        DotDotEqual,
        Equal,
        EqualEqual,
        Exclamation,
        ExclamationEqual,
        Greater,
        GreaterEqual,
        GreaterGreater,
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
            token. Right shift keeps its `>>` token; the parser reads its
            suffix from the adjacent punctuation so nested static argument
            lists still close one `>` at a time.
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

    impl PunctuationKind {
        pub fn ordered_lexemes() -> &'static [(&'static str, Self)] {
            &[
                ("+%", Self::PlusWrapping),
                ("+|", Self::PlusSaturating),
                ("+!", Self::PlusTrapping),
                ("-%", Self::MinusWrapping),
                ("-|", Self::MinusSaturating),
                ("-!", Self::MinusTrapping),
                ("*%", Self::AsteriskWrapping),
                ("*|", Self::AsteriskSaturating),
                ("*!", Self::AsteriskTrapping),
                ("/%", Self::SlashWrapping),
                ("/|", Self::SlashSaturating),
                ("/!", Self::SlashTrapping),
                ("%%", Self::PercentWrapping),
                ("%|", Self::PercentSaturating),
                ("%!", Self::PercentTrapping),
                ("<<%", Self::LessLessWrapping),
                ("<<|", Self::LessLessSaturating),
                ("<<!", Self::LessLessTrapping),
                ("::", Self::ColonColon),
                ("->", Self::Arrow),
                ("..=", Self::DotDotEqual),
                ("..", Self::DotDot),
                ("==", Self::EqualEqual),
                ("!=", Self::ExclamationEqual),
                ("<<", Self::LessLess),
                ("<=", Self::LessEqual),
                (">>", Self::GreaterGreater),
                (">=", Self::GreaterEqual),
                ("&&", Self::AndAnd),
                ("||", Self::PipePipe),
                ("+=", Self::PlusEqual),
                ("-=", Self::MinusEqual),
                ("*=", Self::AsteriskEqual),
                ("/=", Self::SlashEqual),
                ("%=", Self::PercentEqual),
                ("&", Self::Ampersand),
                ("'", Self::Apostrophe),
                ("*", Self::Asterisk),
                ("^", Self::Caret),
                (":", Self::Colon),
                (",", Self::Comma),
                (".", Self::Dot),
                ("=", Self::Equal),
                ("!", Self::Exclamation),
                (">", Self::Greater),
                ("#", Self::Hash),
                ("{", Self::LeftBrace),
                ("[", Self::LeftBracket),
                ("(", Self::LeftParen),
                ("<", Self::Less),
                ("-", Self::Minus),
                ("%", Self::Percent),
                ("|", Self::Pipe),
                ("+", Self::Plus),
                ("}", Self::RightBrace),
                ("]", Self::RightBracket),
                (")", Self::RightParen),
                (";", Self::Semicolon),
                ("/", Self::Slash),
                ("~", Self::Tilde),
            ]
        }

        pub fn lexeme(self) -> &'static str {
            match Self::ordered_lexemes()
                .iter()
                .find(|(_, kind)| *kind == self)
            {
                Some((lexeme, _)) => lexeme,
                None => "<unknown>",
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn punctuation_spellings_round_trip_through_the_ordered_table() {
            for (lexeme, punctuation) in PunctuationKind::ordered_lexemes() {
                assert_eq!(punctuation.lexeme(), *lexeme);
            }
            assert_eq!(
                PunctuationKind::lexeme(PunctuationKind::Unknown),
                "<unknown>"
            );
        }
    }
}

pub use comment_kind::CommentKind;
pub use keyword_kind::KeywordKind;
pub use numeric_literal_kind::{
    FloatLiteralKind, IntegerLiteralKind, NumericBase, NumericLiteralKind,
};
pub use punctuation_kind::PunctuationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenKind {
    #[default]
    Identifier,
    NumericLiteral(NumericLiteralKind),
    StringLiteral,
    Keyword(KeywordKind),
    Punctuation(PunctuationKind),
    Whitespace,
    Comment(CommentKind),
}
