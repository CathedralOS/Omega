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
    #[default]
    Unknown,
}

impl PunctuationKind {
    pub fn ordered_lexemes() -> &'static [(&'static str, Self)] {
        &[
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

    /// This punctuation's authored spelling; `<unknown>` is the default kind
    /// that no lexeme produces. `ordered_lexemes` stays the one table: lexing
    /// needs its longest-first order and this reads the same rows back.
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
