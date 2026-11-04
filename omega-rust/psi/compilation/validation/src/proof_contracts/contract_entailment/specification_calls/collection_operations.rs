//! Specification indexing forms from preceding facts, never the pending condition.
//!
//! Selection stays at the shared operator intake. Builtin geometry uses the same
//! observed-operand rule as executable indexing, with a declaration judge rather
//! than a runtime prefix that would import every entry requirement at once.

use super::{SpecificationCaller, SpecificationPremises};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

pub(super) fn validate_builtin_index(
    program: &dyn crate::CheckingInput,
    caller: SpecificationCaller<'_>,
    expression: ExpressionHandle,
    prior_facts: SpecificationPremises<'_>,
    arguments: &[ExpressionHandle],
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let proven = (|| {
        let indexed = program.expression_table.indexed_node(expression)?;
        let reference = caller.expression_type(program, indexed.collection)?;
        let array_extent = if crate::fixed_array_type_extent(program, reference).is_some() {
            let parameters = caller
                .parameters(program)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let source = crate::parameter_source_type_application(
                program,
                caller.symbol(),
                &parameters,
                indexed.collection,
            );
            match crate::resolved_projected_array_extent(program, reference, source) {
                Ok(extent) => Some(extent),
                Err(_) => return None,
            }
        } else {
            None
        };
        let collection = caller.term(program, indexed.collection)?;
        let judge = prior_facts.structural_judge(program, caller, arguments)?;
        Some(super::super::collection_bounds::proves_observed_bounds(
            program,
            &judge,
            indexed,
            reference,
            array_extent.as_ref(),
            collection,
            |source| {
                Some((
                    caller.term(program, source)?,
                    caller.expression_type(program, source),
                ))
            },
        ))
    })() == Some(true);
    if !proven {
        diagnostics.push(
            Diagnostic::error(format!(
                "cannot form specification index in {owner}: independent collection bounds are not established by preceding formed facts"
            ))
            .with_source_span(program.expression_table.source_span(expression)),
        );
    }
}
