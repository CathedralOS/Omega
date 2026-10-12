//! Settled checking consumes evaluated array lengths and range endpoints.
//! Their authored calls may survive only in the preliminary checkpoint while
//! the semantic evaluator retains the continuation that will replace them.

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
            let TypeReferenceNode::FixedArray {
                length:
                    FixedArrayLength::ConstCall {
                        name, source_span, ..
                    },
                ..
            } = program.type_reference_table.type_reference(*reference)
            else {
                return None;
            };
            Some(
                Diagnostic::error(format!(
                    "array length `{}` has an unresolved build-time evaluation dependency",
                    name.as_str()
                ))
                .with_source_span(*source_span),
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
        .map(|expression| {
            if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some() {
                let call = program.expression_table.call_node(*expression);
                let original = program.expression_table.source_call_node(*expression);
                eprintln!(
                    "TRACE_PENDING_CONSTANT_INPUT expression={:?} span={:?} node={:?} target={:?} original_target={:?}",
                    expression,
                    program.expression_table.source_span(*expression),
                    std::mem::discriminant(program.expression_table.expression(*expression)),
                    call.map(|call| (call.target_symbol, call.target.as_str())),
                    original.map(|call| (call.target_symbol, call.target.as_str())),
                );
            }
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
