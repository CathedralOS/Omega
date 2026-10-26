//! Ordinary owned arguments capture their inline cells, not the contents
//! behind contained references. Keep that typed observation through projections
//! so descriptor metadata can compose without weakening mathematical isolation.

use super::StructuralTerm;
use typed_trees::types::TypeReferenceHandle;

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
    if shared_inline_observation_type(program, &value)
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

/// A checked shared borrow freezes its referent's inline cells; references in
/// those cells still need their own observation before another crossing.
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
        || !crate::declared_carriers_match(
            program,
            shell.referee,
            owned_observation_type(program, value)?,
        )
    {
        return None;
    }
    Some(*observed_type)
}

pub(in crate::proof_contracts::contract_entailment) fn shared_inline_observation(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
    value: StructuralTerm,
) -> Option<StructuralTerm> {
    let shell = crate::reference_shell(program, declared)?;
    let value = StructuralTerm::ObservedValue {
        observed_type: declared,
        value: Box::new(owned_projection_observation(program, shell.referee, value)),
        descriptor: None,
    };
    shared_inline_observation_type(program, &value)?;
    Some(value)
}
