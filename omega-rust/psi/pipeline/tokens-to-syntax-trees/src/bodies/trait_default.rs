use crate::bodies::statements::parse_statement::parse_statement_handles;
use crate::input::token_cursor::Input;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use tokens::PunctuationKind;

pub(crate) fn parse_trait_default_machine_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    (
        HandleSpan<syntax_trees::statement::StatementHandle>,
        Input<'tokens, 'source>,
    ),
    crate::diagnostics::parse_error::ParseError,
> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut start = Handle::invalid();
    let mut count = 0u32;
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (statements, rest) = parse_statement_handles(syntax_trees, input)?;
        if count == 0 {
            start = statements.start();
        }
        count = count
            .checked_add(statements.count())
            .expect("trait default statement span count overflow");
        input = rest;
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let body = if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    };
    Ok((body, input))
}
