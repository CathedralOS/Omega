//! Construction performs physical stores just as assignment does. Consume the
//! normalized field geometry and the constructor's source value bounds; the
//! stored width is never a bound on the semantic value being constructed.

use super::scalar_values::ConstructionScalarValue;
use crate::proof_contracts::arithmetic_domains::{
    stored_integer_field_type, stored_integer_value_fits,
};
use checked_trees::{CheckedStoredIntegerWrite, StoredIntegerWriteSite};
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, TableStructLiteral, TableStructLiteralField};
use typed_trees::machine::Machine;
use typed_trees::state::State;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_construction(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    field: &TableStructLiteralField,
    scalar: &ConstructionScalarValue,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    statement_index: usize,
) -> Vec<CheckedStoredIntegerWrite> {
    let layout_context = crate::LayoutCheckingContext::for_owner(program, machine.symbol);
    let program: &dyn crate::CheckingInput = &layout_context;
    if program.plan_laid_layout(literal.type_symbol).is_none() {
        return Vec::new();
    }
    let Some(carrier) = program
        .type_reference_table
        .find_named_type_reference(literal.type_symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt construction in {owner} lost its exact type"
        )));
        return Vec::new();
    };
    let Ok(targets) = crate::layout_checking_targets(program, [carrier]) else {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt construction in {owner} has no complete layout alternative roster"
        )));
        return Vec::new();
    };
    let initial_diagnostics = diagnostics.len();
    let mut writes = Vec::new();
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        if let Some(write) = validate_alternative(
            &alternative,
            machine,
            state,
            expression,
            literal,
            field,
            scalar,
            owner,
            diagnostics,
            statement_index,
        ) && !writes.contains(&write)
        {
            writes.push(write);
        }
    }
    if diagnostics.len() != initial_diagnostics {
        return Vec::new();
    }
    writes
}

#[allow(clippy::too_many_arguments)]
fn validate_alternative(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    field: &TableStructLiteralField,
    scalar: &ConstructionScalarValue,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
    statement_index: usize,
) -> Option<CheckedStoredIntegerWrite> {
    let plan = program.plan_laid_layout(literal.type_symbol)?;
    let field_index = plan
        .field_symbols
        .iter()
        .position(|symbol| *symbol == field.field_symbol)?;
    let Some(geometry) = plan.geometry_for_target(program.typed_program(), program.layout_target())
    else {
        diagnostics.push(Diagnostic::error(format!(
            "IntegerAt construction in {owner} has no exact alternative geometry"
        )));
        return None;
    };
    if !geometry
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
    if !stored_integer_value_fits(
        program,
        field.value,
        scalar.interval,
        scalar.primitive,
        &scalar.environment,
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
