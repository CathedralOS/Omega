//! Construction performs physical stores just as assignment does. Consume the
//! normalized field geometry and the constructor's source value bounds; the
//! stored width is never a bound on the semantic value being constructed.

use super::scalar_values::ConstructionScalarValue;
use crate::proof_contracts::arithmetic_domains::{
    stored_integer_field_type, stored_integer_value_fits,
};
use checked_trees::{CheckedStoredIntegerWrite, StoredIntegerWriteSite};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, TableStructLiteral, TableStructLiteralField};
use typed_trees::machine::Machine;
use typed_trees::state::State;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_construction(
    program: &TypedTrees,
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
