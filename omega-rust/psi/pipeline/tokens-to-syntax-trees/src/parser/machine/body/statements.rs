use crate::parser::input::{Input, ParseResult};
use crate::parser::statement::{
    parse_asm_block_statement_handles, parse_statement_handle, reject_retired_proof_output_binding,
    try_parse_atomic_compare_exchange_let, try_parse_atomic_fetch_let, try_parse_atomic_swap_let,
    try_parse_destructure_let, try_parse_proof_output_binding,
};
use crate::parser::transition::parse_transition_block_handles;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::statement::StatementHandle;
use tokens::{KeywordKind, PunctuationKind};

#[derive(Clone, Copy)]
pub(in crate::parser) enum BodyKind {
    MachineEntry,
    ExplicitState,
}

impl BodyKind {
    fn at_end(self, input: Input<'_, '_>) -> bool {
        match self {
            Self::MachineEntry => super::starts_machine_member(input),
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

pub(in crate::parser) fn parse_statements<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    body: BodyKind,
) -> ParseResult<'tokens, 'source, HandleSpan<StatementHandle>> {
    let mut statements = HandleSpan::empty();
    while !body.at_end(input) {
        reject_retired_proof_output_binding(input)?;
        let (parsed, rest) = if input.at_punctuation(PunctuationKind::Arrow) {
            return Err(input.error_here(body.bare_arrow_error()));
        } else if input.at_keyword(KeywordKind::Transition) {
            let next = input.take_keyword(KeywordKind::Transition, "transition")?;
            parse_transition_block_handles(syntax_trees, next)?
        } else if input.at_contextual("asm") {
            parse_asm_block_statement_handles(syntax_trees, input)?
        } else if let Some(parsed) = try_parse_proof_output_binding(syntax_trees, input) {
            parsed
        } else if let Some(parsed) = try_parse_destructure_let(syntax_trees, input) {
            parsed
        } else if let Some(parsed) = try_parse_atomic_fetch_let(syntax_trees, input) {
            parsed
        } else if let Some(parsed) = try_parse_atomic_swap_let(syntax_trees, input) {
            parsed
        } else if let Some(parsed) = try_parse_atomic_compare_exchange_let(syntax_trees, input) {
            parsed
        } else {
            let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
            let handle = syntax_trees.items.append_statement_handle(statement);
            (HandleSpan::from_parts(handle, 1), rest)
        };
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
