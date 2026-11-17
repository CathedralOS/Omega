use tokens::PunctuationKind;

#[test]
fn every_punctuation_spelling_reaches_its_longest_match() {
    for (lexeme, punctuation) in PunctuationKind::ordered_lexemes() {
        assert_eq!(
            PunctuationKind::matching_prefix(lexeme.as_bytes()),
            Some((*lexeme, *punctuation)),
        );
    }
    assert_eq!(
        PunctuationKind::lexeme(PunctuationKind::Unknown),
        "<unknown>"
    );
}
