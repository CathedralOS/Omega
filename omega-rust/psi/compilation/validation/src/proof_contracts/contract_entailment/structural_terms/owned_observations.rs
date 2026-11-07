//! Copies retain the inline values they actually observed. A live shared borrow
//! also freezes those cells, so its saved copy keeps the same observed contents.
//! Pointer fields do not capture the data they point to; each later crossing
//! still needs its own read.

use super::StructuralTerm;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(in crate::proof_contracts::contract_entailment) fn owned_observation_type(
    program: &dyn crate::CheckingInput,
    value: &StructuralTerm,
) -> Option<TypeReferenceHandle> {
    let StructuralTerm::ObservedValue {
        observed_type,
        descriptor: None,
        ..
    } = value
    else {
        return None;
    };
    (observed_type.is_valid()
        && crate::reference_shell(program, *observed_type).is_none()
        && program.primitive_type_reference(*observed_type).is_none())
    .then_some(*observed_type)
}

pub(in crate::proof_contracts::contract_entailment) fn owned_projection_observation(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
    value: StructuralTerm,
) -> StructuralTerm {
    if let Some(algebra) = crate::content_algebra_for_type(program, declared) {
        if matches!(&value, StructuralTerm::ContentValue { algebra: actual, .. } if actual == &algebra)
        {
            value
        } else {
            StructuralTerm::ContentValue {
                algebra,
                value: Box::new(value),
            }
        }
    } else if shared_inline_observation_type(program, &value)
        .is_some_and(|observed| crate::declared_carriers_match(program, declared, observed))
    {
        value
    } else if crate::reference_shell(program, declared).is_some() {
        super::descriptor_only_observation(declared, value.into_collection_descriptor())
    } else if matches!(&value, StructuralTerm::ObservedValue { observed_type, descriptor: None, .. } if *observed_type == declared)
    {
        value
    } else {
        StructuralTerm::ObservedValue {
            observed_type: declared,
            value: Box::new(value),
            descriptor: None,
        }
    }
}

/// A saved shared reference keeps the same inline contents while its loan is
/// live. Contained pointers still need a separate read before another crossing.
pub(in crate::proof_contracts::contract_entailment) fn shared_inline_observation_type(
    program: &dyn crate::CheckingInput,
    value: &StructuralTerm,
) -> Option<TypeReferenceHandle> {
    let StructuralTerm::ObservedValue {
        observed_type,
        value,
        descriptor: None,
    } = value
    else {
        return None;
    };
    let shell = crate::reference_shell(program, *observed_type)?;
    if shell.access != language_semantics::ReferenceAccess::Shared
        || crate::slice_view_shell(program, *observed_type, &[]).is_some()
    {
        return None;
    }
    let contents_type = owned_observation_type(program, value).or_else(|| {
        let StructuralTerm::ObservedValue {
            observed_type,
            descriptor: None,
            ..
        } = value.as_ref()
        else {
            return None;
        };
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(*observed_type);
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(carrier)
        else {
            return None;
        };
        program
            .symbols
            .builtin_type_atom(*symbol)
            .filter(|atom| !atom.is_atomic())?;
        program.primitive_type_reference(carrier)?;
        Some(*observed_type)
    })?;
    crate::declared_carriers_match(program, shell.referee, contents_type).then_some(*observed_type)
}

pub(in crate::proof_contracts::contract_entailment) fn shared_inline_observation(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
    value: StructuralTerm,
) -> Option<StructuralTerm> {
    let shell = crate::reference_shell(program, declared)?;
    // A readable pointer type alone supplies no scalar contents. This wrapper
    // carries an already completed read, never a raw descriptor or annotation.
    if program.primitive_type_reference(shell.referee).is_some()
        && !super::scalar_operand_is_observed(program, declared, &value)
    {
        return None;
    }
    let value = StructuralTerm::ObservedValue {
        observed_type: declared,
        value: Box::new(owned_projection_observation(program, shell.referee, value)),
        descriptor: None,
    };
    shared_inline_observation_type(program, &value)?;
    Some(value)
}
