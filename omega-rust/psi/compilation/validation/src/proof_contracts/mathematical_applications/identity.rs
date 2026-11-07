//! The sealed identity declaration retains its carrier and exact source subjects.
//!
//! This view forms a statement; it grants neither a witness nor a value law.
//! Source elaboration checks its kernel Id, and consumers retain the same
//! selected declaration instead of recognizing a call's displayed spelling.

use symbols::FixedCoreDeclaration;
use typed_trees::expression::{ExpressionHandle, TableCallExpression};
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone, Copy)]
pub struct IdentityApplication {
    pub carrier: TypeReferenceHandle,
    pub subjects: [ExpressionHandle; 2],
}

pub fn is_identity_call(program: &dyn crate::CheckingInput, call: &TableCallExpression) -> bool {
    call.target_symbol.is_valid()
        && program
            .symbols
            .fixed_core_declaration_symbol(FixedCoreDeclaration::Identity)
            == Some(call.target_symbol)
}

pub fn identity_application(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Result<IdentityApplication, &'static str> {
    if !is_identity_call(program, call) {
        return Err("identity requires the sealed core declaration");
    }
    if call.receiver.is_valid()
        || call.static_machine_parameter.is_valid()
        || !call.selects_only_nominal_route()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement.acknowledges_suspend
        || call.operational_acknowledgement.acknowledges_block
    {
        return Err(
            "core::Identity cannot carry a receiver, machine, evidence or operational argument",
        );
    }
    let [argument] = call.machine_arguments.as_ref() else {
        return Err("core::Identity requires exactly one type argument");
    };
    if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
        return Err("core::Identity requires a type, not a value or evidence argument");
    }
    let carrier = if argument.type_reference.is_valid() {
        argument.type_reference
    } else if argument.symbol.is_valid() {
        program
            .type_reference_table
            .find_named_type_reference(argument.symbol)
            .ok_or("core::Identity type argument has no retained type")?
    } else {
        return Err("core::Identity type argument is unresolved");
    };
    if !program
        .type_reference_table
        .contains_type_reference(carrier)
    {
        return Err("core::Identity type argument is unavailable");
    }
    let [left, right] = program.expression_table.expression_handles(call.arguments) else {
        return Err("core::Identity requires exactly two subjects");
    };
    Ok(IdentityApplication {
        carrier,
        subjects: [*left, *right],
    })
}
