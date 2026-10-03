//! Construction performs physical stores just as assignment does. Consume the
//! normalized field geometry and the constructor's source value bounds; the
//! stored width is never a bound on the semantic value being constructed.

use crate::proof_contracts::arithmetic_domains::{
    Interval, ValueEnvironment, immutable_integer_expression_interval,
    occurrence_integer_expression_bounds, stored_integer_field_type, stored_integer_value_fits,
    validate_value_range,
};
use checked_trees::{CheckedStoredIntegerWrite, StoredIntegerWriteSite};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, TableStructLiteral, TableStructLiteralField};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_construction(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    field: &TableStructLiteralField,
    field_type: TypeReferenceHandle,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    statement_index: usize,
) -> Option<CheckedStoredIntegerWrite> {
    let plan = program
        .plan_laid_layouts
        .iter()
        .find(|plan| plan.data_symbol == literal.type_symbol)?;
    let field_index = plan
        .field_symbols
        .iter()
        .position(|symbol| *symbol == field.field_symbol)?;
    if !plan
        .integer_fields
        .iter()
        .any(|field| field.field_index == field_index)
    {
        return None;
    }
    let stored = program
        .type_reference_table
        .find_named_type_reference(literal.type_symbol)
        .ok_or(())
        .and_then(|carrier| stored_integer_field_type(program, carrier, field.field_symbol));
    let Ok(Some(stored)) = stored else {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt store in {owner} has no exact stored field geometry"
        )));
        return None;
    };
    let mut arithmetic_diagnostics = Vec::new();
    let (interval, primitive, _) = validate_value_range(
        program,
        machine,
        Some(state),
        field.value,
        environment,
        program.primitive_type_reference(field_type),
        program.arithmetic_domain_for_type_reference(field_type),
        owner,
        &mut arithmetic_diagnostics,
    );
    // Read the value at its own evaluation point. A constructor can be nested
    // between effectful siblings; a statement-final environment would confuse
    // a completed value with the storage that a later operand changes.
    let occurrence = occurrence_integer_expression_bounds(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        field.value,
    )
    .map(|(low, high)| Interval {
        low: Some(low),
        high: Some(high),
    });
    // The occurrence reader independently checks arithmetic under its exact
    // prefix facts. Without those facts the standalone scan can only use the
    // conservative declaration/immutable environment and must retain failures.
    if !arithmetic_diagnostics.is_empty() && occurrence.is_none() {
        diagnostics.extend(arithmetic_diagnostics);
        return None;
    }
    let interval = occurrence.unwrap_or(interval);
    let interval = immutable_integer_expression_interval(program, machine, state, field.value)
        .map_or(interval, |standing| interval.intersect(standing));
    if !stored_integer_value_fits(
        program,
        field.value,
        interval,
        primitive,
        environment,
        stored,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt store in {owner} requires a value proven to fit {}-bit {} storage",
            stored.bits(),
            match stored.sign() {
                semantic_vocabulary::IntegerSign::Signed => "signed",
                semantic_vocabulary::IntegerSign::Unsigned => "unsigned",
            },
        )));
        return None;
    }
    Some(CheckedStoredIntegerWrite {
        machine: machine.symbol,
        state: state.symbol,
        site: StoredIntegerWriteSite::Construction {
            expression,
            statement_index,
        },
        value: field.value,
        field: field.field_symbol,
        stored_type: stored,
    })
}
