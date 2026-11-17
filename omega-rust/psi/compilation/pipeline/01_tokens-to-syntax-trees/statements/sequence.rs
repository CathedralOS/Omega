use crate::statements::parse::parse_statement_handles;
use crate::syntax_storage::tables::{items_of, stmts_of};
use crate::token_input::cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::statement::StatementHandle;
use tokens::PunctuationKind;

pub(crate) fn parse_statements<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    requires_result: bool,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let mut statements = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    while (requires_result && input.at_identifier_then_contextual("machine"))
        || !crate::machine_bodies::states::starts_machine_member(input)
    {
        if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(
                "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported",
            ));
        }
        let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
        let completes_body = items_of(syntax_trees)
            .statements(parsed)
            .last()
            .is_some_and(|statement| {
                matches!(
                    stmts_of(syntax_trees).statement(*statement),
                    syntax_trees::statement::StatementNode::Transition(_)
                        | syntax_trees::statement::StatementNode::Expression(_)
                )
            })
            && crate::declarations::items::starts_machine_declaration(rest);
        statements.push_span(parsed);
        input = rest;
        if completes_body {
            break;
        }
    }
    Ok((statements.finish(), input))
}
