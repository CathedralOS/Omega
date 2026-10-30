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

/// The one keyword spelling table, sorted by spelling. The lexer reads it
/// forward (`from_lexeme`) and diagnostics read it back (`lexeme`), so a
/// keyword's spelling is stated once: a keyword cannot lex under one spelling
/// and render under another.
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

    /// This keyword's authored spelling; `<unknown>` is the default kind that
    /// no spelling produces.
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
