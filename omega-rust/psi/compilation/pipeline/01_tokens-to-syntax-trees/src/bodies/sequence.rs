use crate::bodies::statements::parse_statement::parse_statement_handles;
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::statement::StatementHandle;
use tokens::PunctuationKind;

pub(crate) fn parse_statements<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    requires_result: bool,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let mut statements = crate::handle_spans::HandleSpanBuilder::new();
    // Before value completion, `value machine helper...` means a result and
    // then a declaration, not a target prefix on that declaration.
    while (requires_result && input.at_identifier_then_contextual("machine"))
        || !super::parse_body::starts_machine_member(input)
    {
        if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(
                "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported",
            ));
        }
        let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
        let completes_body =
            syntax_trees
                .items
                .statements(parsed)
                .last()
                .is_some_and(|statement| {
                    matches!(
                        syntax_trees.statements.statement(*statement),
                        syntax_trees::statement::StatementNode::Transition(_)
                            | syntax_trees::statement::StatementNode::Expression(_)
                    )
                })
                && crate::declarations::parse_declaration::starts_machine_declaration(rest);
        statements.push_span(parsed);
        input = rest;
        if completes_body {
            break;
        }
    }
    Ok((statements.finish(), input))
}
