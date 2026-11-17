use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::TableMatchField;
use syntax_trees::statement::TableOutcomeProofSelector;

pub(crate) struct DestructureFields {
    pub(crate) fields: Vec<TableMatchField>,
    pub(crate) has_rest: bool,
    pub(crate) proof_selectors: Vec<TableOutcomeProofSelector>,
}

/// A destructure pattern's `{ fields; proof selectors }`. The field
/// values are expressions, so the expression frame machine parses it.
pub(crate) fn parse_destructure_fields<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, DestructureFields> {
    use crate::expressions::parse::{Goal, Parsed, run};
    match run(syntax_trees, input, Goal::DestructureFields)? {
        (Parsed::DestructureFields(fields), rest) => Ok((fields, rest)),
        (Parsed::Expression(_), _) => {
            unreachable!("a destructure-fields goal finishes with fields")
        }
    }
}
