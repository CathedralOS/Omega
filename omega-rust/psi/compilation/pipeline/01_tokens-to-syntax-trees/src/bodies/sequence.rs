use crate::bodies::statements::parse_statement::parse_statement_handles;
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::statement::StatementHandle;
use tokens::PunctuationKind;

#[derive(Clone, Copy)]
pub(crate) enum BodyKind {
    MachineEntry { requires_result: bool },
    ExplicitState,
}

impl BodyKind {
    fn at_end(self, input: Input<'_, '_>) -> bool {
        match self {
            Self::MachineEntry { requires_result } => {
                // Before a value completion, `value machine helper...` is a
                // result followed by a declaration, not a target prefix. Once
                // a terminal statement completes the body, declaration parsing
                // owns any target prefix that follows it.
                let ambiguous_value =
                    requires_result && input.at_identifier_then_contextual("machine");
                !ambiguous_value && super::parse_body::starts_machine_member(input)
            }
            Self::ExplicitState => input.at_punctuation(PunctuationKind::RightBrace),
        }
    }

    fn bare_arrow_error(self) -> &'static str {
        match self {
            Self::MachineEntry { .. } => {
                "machine entry bodies must use the `transition` keyword; bare `->` transitions are not supported"
            }
            Self::ExplicitState => {
                "explicit state bodies must use the `transition` keyword; bare `->` transitions are only allowed in implicit entry"
            }
        }
    }
}

pub(crate) fn parse_statements<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    body: BodyKind,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let mut statements = crate::handle_spans::HandleSpanBuilder::new();
    while !body.at_end(input) {
        if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(body.bare_arrow_error()));
        }
        let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
        let completes_body = matches!(body, BodyKind::MachineEntry { .. })
            && syntax_trees
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
