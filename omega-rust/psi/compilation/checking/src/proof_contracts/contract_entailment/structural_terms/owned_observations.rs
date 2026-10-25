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
    if crate::reference_shell(program, declared).is_some() {
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
