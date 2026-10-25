use crate::bodies::statements::parse_statement::parse_statement_handles;
use crate::input::token_cursor::Input;
use arena::HandleSpan;
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
    let mut body = crate::handle_spans::HandleSpanBuilder::new();
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (statements, rest) = parse_statement_handles(syntax_trees, input)?;
        body.push_span(statements);
        input = rest;
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((body.finish(), input))
}
