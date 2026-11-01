//! Proposition application label sources.

use crate::flow::expressions;

/// The label sources every proposition application label instantiation
/// prepares identically: the binder arguments' display names plus the
/// argument expression handles callers then label positionally.
pub(crate) fn proposition_label_sources<'a>(
    application: &'a typed_trees::proposition::PropositionApplication,
    program: &'a dyn ::validation::CheckingInput,
) -> (Vec<String>, &'a [typed_trees::expression::ExpressionHandle]) {
    (
        application
            .binder_arguments
            .iter()
            .map(|argument| argument.display_name())
            .collect(),
        expressions(program).expression_handles(application.arguments),
    )
}
