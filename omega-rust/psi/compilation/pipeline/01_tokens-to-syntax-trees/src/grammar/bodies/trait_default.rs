use crate::diagnostics::parse_error::ParseError;
use crate::grammar::bodies::statements::parse_statement::parse_statement_handles;
use crate::tokens::token_cursor::Input;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::statement::StatementHandle;
use tokens::PunctuationKind;

pub(crate) fn parse_trait_default_machine_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<(HandleSpan<StatementHandle>, Input<'tokens, 'source>), ParseError> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut body = crate::storage::handle_spans::HandleSpanBuilder::new();
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (statements, rest) = parse_statement_handles(syntax_trees, input)?;
        body.push_span(statements);
        input = rest;
    }
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((body.finish(), input))
}
