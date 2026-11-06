//! Cast representability and target predicates are obligations, never source
//! interval premises.
//! Policy qualification cannot repair failed initial membership. Inspect every
//! authored range shell; unsupported bounds must not disappear through an
//! optional interval query that also represents absence of a range.

use super::ValueEnvironment;
use crate::proof_contracts::arithmetic_domains::integer_ranges::primitive_range;
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// The expression scanner owns selected reachability and calls this only for
/// an executing cast. Arithmetic analysis owns the proof from the current live
/// environment; a target annotation never substitutes for that analysis.
pub(crate) fn validate_range_cast_at_use(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(cast) = program.expression_table.cast_node(expression) else {
        return;
    };
    if cast.form.is_recast() {
        return;
    }
    // Every integer target demands an exact fit, independently of result policy.
    // In call arguments the
    // expression scanner is the validation owner; collecting successful cast
    // facts afterward cannot diagnose missing evidence. Reuse the same live
    // arithmetic judgment as ranged casts rather than deferring rejection to
    // the Unit planner. Lowering retains the distinct address interpretation.
    let integer_target = program
        .primitive_type_reference(cast.target_type)
        .is_some_and(|primitive| primitive_range(primitive).is_some());
    if !integer_target {
        return;
    }
    let owner = format!(
        "machine `{}` state `{}` cast expression",
        machine.name, state.name
    );
    let mut found = Vec::new();
    super::validate_arithmetic_domains(
        program,
        machine,
        Some(state),
        expression,
        environment,
        None,
        numerics::arithmetic::ArithmeticDomain::Exact,
        &owner,
        &mut found,
    );
    for diagnostic in found {
        let duplicate = diagnostics.iter().any(|existing| {
            existing.source_span == diagnostic.source_span
                && (existing.message == diagnostic.message
                    // Root and nested scans describe different enclosing
                    // owners, but this is the same source membership failure.
                    || (existing.message.starts_with("cast target range in ")
                        && diagnostic.message.starts_with("cast target range in ")))
        });
        if !duplicate {
            diagnostics.push(diagnostic);
        }
    }
}

/// Whether `cast` is a plain Exact widening whose result value is its
/// operand's: not a borrow recast, not a semantic-domain conversion, carrying
/// no annotation payload, and landing on a nominal primitive target. Only
/// such a cast lets the source's own evidence carry the result carrier.
pub fn plain_exact_widening_cast(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
) -> bool {
    unannotated_exact_cast(cast)
        && matches!(
            program
                .type_reference_table
                .type_reference(cast.target_type),
            typed_trees::types::TypeReferenceNode::Named { .. }
        )
}

/// Whether `cast` is an Exact cast carrying no annotation payload: not a
/// recast, no semantic-domain attachment, and no distinct result type.
/// Target-shape requirements stay with each caller.
pub fn unannotated_exact_cast(cast: &typed_trees::expression::TableCastExpression) -> bool {
    !cast.form.is_recast()
        && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
        && cast.semantic_domain.is_empty()
        && cast.semantic_domain_arguments.is_empty()
        && !cast.semantic_domain_symbol.is_valid()
        && !cast.semantic_domain_id.is_valid()
        && !(cast.result_type.is_valid() && cast.result_type != cast.target_type)
}
