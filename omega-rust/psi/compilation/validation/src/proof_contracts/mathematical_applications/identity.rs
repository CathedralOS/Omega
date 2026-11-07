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

/// Read a direct declaration use or its emitted checked value after ordinary
/// transparent application. Only the authenticated Id row supplies meaning;
/// the source occurrences come from that application's exact capture roster.
pub fn identity_application_at(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Result<IdentityApplication, &'static str> {
    if let Some(call) = program.expression_table.call_node(expression)
        && is_identity_call(program, call)
    {
        return identity_application(program, call);
    }
    let view = program
        .prepared_mathematics()
        .ok_or("identity has no checked mathematical view")?;
    let application = view
        .application(expression)
        .ok_or("expression has no checked mathematical application")?;
    let term = match view.arena.get(application.term) {
        proof_admission::Term::Squash { ty } => ty,
        _ => application.term,
    };
    let (identity, subjects) = view
        .identity_value(application, term, 0)
        .ok_or("expression does not emit an authenticated typed Id")?;
    let captures = view
        .applications
        .captures
        .span_or_empty(application.captures);
    let capture = |term| {
        let proof_admission::Term::Variable(index) = view.arena.get(term) else {
            return Err("canonical content Id subject has no retained source capture");
        };
        let position = captures
            .len()
            .checked_sub(index as usize + 1)
            .ok_or("canonical content Id subject is outside its capture telescope")?;
        let value = &captures[position];
        if identity.type_identity != program.normalized_type_identity(value.source_type) {
            return Err("canonical content Id subject lost its exact source carrier");
        }
        Ok(value)
    };
    let left = capture(subjects[0])?;
    let right = capture(subjects[1])?;
    Ok(IdentityApplication {
        carrier: left.source_type,
        subjects: [left.expression, right.expression],
    })
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
