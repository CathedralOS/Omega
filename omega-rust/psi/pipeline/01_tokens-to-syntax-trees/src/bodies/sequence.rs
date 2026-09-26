use crate::bodies::statements::parse_statement::parse_statement_handles;
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::statement::StatementHandle;
use tokens::PunctuationKind;

#[derive(Clone, Copy)]
pub(crate) enum BodyKind {
    MachineEntry,
    ExplicitState,
}

impl BodyKind {
    fn at_end(self, input: Input<'_, '_>) -> bool {
        match self {
            Self::MachineEntry => super::parse_body::starts_machine_member(input),
            Self::ExplicitState => input.at_punctuation(PunctuationKind::RightBrace),
        }
    }

    fn bare_arrow_error(self) -> &'static str {
        match self {
            Self::MachineEntry => {
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
    let mut statements = HandleSpan::empty();
    while !body.at_end(input) {
        if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(body.bare_arrow_error()));
        }
        let (parsed, rest) = parse_statement_handles(syntax_trees, input)?;
        if !parsed.is_empty() {
            let start = if statements.is_empty() {
                parsed.start()
            } else {
                statements.start()
            };
            let count = statements
                .count()
                .checked_add(parsed.count())
                .expect("state statement span count overflow");
            statements = HandleSpan::from_parts(start, count);
        }
        input = rest;
    }
    Ok((statements, input))
}
