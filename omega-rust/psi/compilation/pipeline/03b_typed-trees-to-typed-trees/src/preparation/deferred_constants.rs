//! Settled checking consumes evaluated array lengths and range endpoints.
//! Their authored calls may survive only in the preliminary checkpoint while
//! the semantic evaluator retains the continuation that will replace them,
//! or as rostered constant recipes the target closure fixes after the
//! shared judgment: those positions stay symbolic by design, and checking
//! records what it needs from each instead of refusing them.

use diagnostics::Diagnostic;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

pub(super) fn require_evaluated_constants(
    program: &dyn ::validation::CheckingInput,
) -> Result<(), Vec<Diagnostic>> {
    // A declaration projection preserves backing handles for its frozen
    // descriptors. Only the live judgment's roots own constant obligations;
    // detached Product types must not turn a settled Build into a whole-package
    // evaluation demand. Live unresolved calls still reject without exception.
    let references = typed_trees::references::collect_program_references(program);
    let pending_lengths = references
        .type_references
        .iter()
        .filter_map(|reference| {
            let TypeReferenceNode::FixedArray { length, .. } =
                program.type_reference_table.type_reference(*reference)
            else {
                return None;
            };
            let (name, source_span) = match length {
                FixedArrayLength::ConstCall {
                    name, source_span, ..
                } if !program
                    .constant_recipe_for_array_length(*reference)
                    .is_valid() =>
                {
                    (name.as_str(), *source_span)
                }
                // An unresolved declaration is preparation state, not a
                // generic binder and not geometry for a settled judgment.
                FixedArrayLength::ConstParameter { symbol, name }
                    if symbol.is_valid()
                        && program.symbols.get(*symbol).kind == symbols::SymbolKind::Const =>
                {
                    let origins = match program.type_reference_table.constant_origins(*reference) {
                        Ok(origins) => origins,
                        Err(reason) => return Some(Diagnostic::error(reason)),
                    };
                    let source_span = origins
                        .iter()
                        .find(|origin| origin.declaration == *symbol)
                        .map(|origin| origin.reference)
                        .or_else(|| program.symbols.symbol_source_span(*symbol))
                        .unwrap_or_default();
                    (name.as_str(), source_span)
                }
                _ => return None,
            };
            Some(
                Diagnostic::error(format!(
                    "array length `{name}` has an unresolved build-time evaluation dependency"
                ))
                .with_source_span(source_span),
            )
        })
        .collect::<Vec<_>>();
    // Preserve the compiler entrance's array-before-range diagnostic order.
    if !pending_lengths.is_empty() {
        return Err(pending_lengths);
    }
    // The evaluator clears a mark when its integer lands. A surviving mark
    // is a lost continuation, not an unbounded or dependent source range.
    let pending_endpoints = program
        .pending_const_range_endpoints
        .iter()
        .filter(|expression| references.expressions.contains(expression))
        .filter(|expression| {
            !program
                .constant_recipe_for_range_endpoint(**expression)
                .is_valid()
        })
        .filter(|expression| {
            !program
                .constant_recipe_for_contract_expression(**expression)
                .is_valid()
        })
        .filter(|expression| !program.is_deferred_contract_invocation(**expression))
        .filter(|expression| !program.constant_recipe_root_for_const_application(**expression))
        .map(|expression| {
            Diagnostic::error("range endpoint has an unresolved build-time evaluation dependency")
                .with_source_span(program.expression_table.source_span(*expression))
        })
        .collect::<Vec<_>>();
    if pending_endpoints.is_empty() {
        Ok(())
    } else {
        Err(pending_endpoints)
    }
}
