//! Collection metadata belongs to the completed descriptor, not its referent.
//!
//! Public normal-return equations and caller observations use this same term.
//! In particular, a returned slice can publish its length without publishing
//! the bytes behind its reference. Ordinary record fields keep their own
//! projection/observation checks, even when named `len` or `capacity`.

use super::StructuralTerm;
use language_semantics::declaration_selection::CollectionMeasure;
use typed_trees::expression::TableMemberExpression;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::proof_contracts::contract_entailment) fn collection_measure_term(
    program: &dyn crate::CheckingInput,
    member: &TableMemberExpression,
    receiver: TypeReferenceHandle,
    collection: impl FnOnce() -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    let measure = crate::value_custody::places::collection_operations::collection_measure_member(
        program,
        member,
        crate::value_custody::places::collection_operations::MeasureReceiver::Declared(receiver),
    )?;
    observed_collection_measure(program, receiver, measure, collection)
}

/// Form a typed metadata observation also when its consumer is a compiler
/// query rather than an authored Member. The measure enum is already selected;
/// receiver geometry and descriptor identity still need the same authority.
pub(in crate::proof_contracts::contract_entailment) fn observed_collection_measure(
    program: &dyn crate::CheckingInput,
    receiver: TypeReferenceHandle,
    measure: CollectionMeasure,
    collection: impl FnOnce() -> Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    if measure == CollectionMeasure::Length
        && let Some(extent) =
            crate::value_custody::places::fixed_array_type_length(program, receiver)
    {
        return Some(StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
            u64::try_from(extent).ok()?,
        )));
    }
    // A direct view carries its own extent. Another reference around a slice
    // instead points at a replaceable descriptor home; its current extent
    // needs a separately captured inner observation, not the outer pointer.
    crate::slice_view_shell(program, receiver, &[])?;
    // Display-only unknowns cannot identify a descriptor. This is only an
    // identity check: the authenticated measure above grants metadata access,
    // while referent-content authority stays with its existing owner.
    let collection = collection()?.into_collection_descriptor();
    collection.observation_identity_key()?;
    Some(StructuralTerm::CollectionMeasure {
        measure,
        collection: Box::new(collection),
    })
}

/// The input descriptor and its frame-authorized referent are different
/// observations. Metadata keeps the actual completed descriptor even when the
/// current contents are unknown or were changed through another view.
pub(in crate::proof_contracts::contract_entailment) fn reference_argument_observation(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
    descriptor: &StructuralTerm,
    referent: Option<StructuralTerm>,
) -> Option<StructuralTerm> {
    if crate::slice_view_shell(program, declared, &[]).is_some() {
        Some(match referent {
            Some(value) => StructuralTerm::ObservedValue {
                observed_type: declared,
                value: Box::new(value),
                descriptor: Some(Box::new(descriptor.clone().into_collection_descriptor())),
            },
            None => descriptor_only_observation(declared, descriptor.clone()),
        })
    } else {
        referent
    }
}

/// Retaining a descriptor supplies metadata authority only. An absent current
/// referent must not be represented by the descriptor's ordinary value identity:
/// an input binder can otherwise identify element reads across a backing write.
pub(in crate::proof_contracts::contract_entailment) fn descriptor_only_observation(
    declared: TypeReferenceHandle,
    descriptor: StructuralTerm,
) -> StructuralTerm {
    StructuralTerm::ObservedValue {
        observed_type: declared,
        value: Box::new(StructuralTerm::Opaque(
            "unobserved slice contents".to_owned(),
        )),
        descriptor: Some(Box::new(descriptor.into_collection_descriptor())),
    }
}

/// Immutable geometry belongs to the completed typed value, even when a caller
/// observes it under a slice formal or its backing contents were invalidated.
/// Do not recover a source expression or use capacity as live length. Untyped
/// binders and unresolved projections supply no new geometry here.
pub(in crate::proof_contracts::contract_entailment) fn completed_collection_measure(
    program: &dyn crate::CheckingInput,
    measure: CollectionMeasure,
    collection: &StructuralTerm,
) -> Option<StructuralTerm> {
    if measure != CollectionMeasure::Length {
        return None;
    }
    let reference = match collection {
        StructuralTerm::CapturedValue { result_type, .. }
        | StructuralTerm::Application { result_type, .. } => *result_type,
        StructuralTerm::IndexedValue { element_type, .. } => *element_type,
        StructuralTerm::CallProjection {
            result_type,
            field,
            field_name,
            ..
        } => {
            program
                .closed_data_member_field(*result_type, *field, field_name)?
                .type_reference
        }
        _ => return None,
    };
    let extent = crate::value_custody::places::fixed_array_type_length(program, reference)?;
    Some(StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
        u64::try_from(extent).ok()?,
    )))
}
