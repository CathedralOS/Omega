//! Scalar contents require an observed value, not merely a readable type.
//!
//! Raw declaration types still select the builtin operation. This owner checks
//! its substituted operand before interpreting scalar contents. Completed
//! reference outputs retain descriptors until an independent live-origin owner
//! supplies a referent observation; input subject bindings already carry that
//! observation. The same check runs after transport and at equality boundaries.

use super::StructuralTerm;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub(in crate::proof_contracts::contract_entailment) fn scalar_operand_meaning(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
    term: &StructuralTerm,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    let meaning =
        crate::value_custody::expression_types::scalar_observation_meaning(program, declared)?;
    scalar_operand_is_observed(program, declared, term).then_some(meaning)
}

pub(in crate::proof_contracts::contract_entailment) fn scalar_operand_is_observed(
    program: &dyn crate::CheckingInput,
    declared: TypeReferenceHandle,
    term: &StructuralTerm,
) -> bool {
    if let Some(observed) = super::shared_inline_observation_type(program, term) {
        return crate::declared_carriers_match(program, declared, observed);
    }
    if let Some(observed) = super::owned_observation_type(program, term) {
        // Owned observation permits reading inline carrier cells only. The
        // projection owner masks each reference child before another crossing;
        // this cannot turn a borrowed referent into an owned snapshot.
        return crate::reference_shell(program, declared).is_none()
            && crate::declared_carriers_match(program, declared, observed);
    }
    if term_is_unobserved_descriptor(program, term) {
        return false;
    }
    if let StructuralTerm::ObservedValue { observed_type, .. } = term {
        return crate::value_custody::expression_types::scalar_observation_meaning(
            program, declared,
        ) == crate::value_custody::expression_types::scalar_observation_meaning(
            program,
            *observed_type,
        );
    }
    if crate::reference_shell(program, declared).is_none() {
        return true;
    }
    match term {
        StructuralTerm::ObservedValue { .. } => unreachable!("handled above"),
        // These are the existing declaration/prefix subject observations.
        // An authenticated input reference may bind a scalar completed value.
        StructuralTerm::CollectionMeasure { .. }
        | StructuralTerm::Variable(_)
        | StructuralTerm::BoundValue(_)
        | StructuralTerm::BoundProjection { .. }
        | StructuralTerm::Integer(_)
        | StructuralTerm::Float(_)
        | StructuralTerm::Constructor { .. }
        | StructuralTerm::ScalarBinary { .. } => true,
        StructuralTerm::Application { result_type, .. }
        | StructuralTerm::CapturedValue { result_type, .. } => {
            crate::value_custody::expression_types::scalar_observation_meaning(
                program,
                *result_type,
            )
            .is_some()
                // A completed owned aggregate can be observed through a
                // borrow of that exact carrier. A produced reference remains
                // a descriptor and was rejected above; this does not follow
                // reference fields inside the owned snapshot.
                || (crate::reference_shell(program, *result_type).is_none()
                    && crate::reference_shell(program, declared).is_some_and(|shell| {
                        shell.access.is_readable()
                            && crate::declared_carriers_match(program, shell.referee, *result_type)
                    }))
        }
        StructuralTerm::IndexedValue { element_type, .. } => {
            crate::value_custody::expression_types::scalar_observation_meaning(program, declared)
                == crate::value_custody::expression_types::scalar_observation_meaning(
                    program,
                    *element_type,
                )
        }
        StructuralTerm::CallProjection { .. } => true,
        // A produced owned field can be borrowed as its exact carrier too.
        // Check every projection crossing: a reference field is a descriptor,
        // not a snapshot of the storage it points at.
        StructuralTerm::Projection { subject, .. } => {
            !has_produced_subject(subject)
                || completed_owned_carrier(program, term).is_some_and(|reference| {
                    crate::reference_shell(program, declared).is_some_and(|shell| {
                        shell.access.is_readable()
                            && crate::declared_carriers_match(program, shell.referee, reference)
                    })
                })
        }
        StructuralTerm::Opaque(_)
        | StructuralTerm::IntegerEmbedding { .. }
        | StructuralTerm::MathematicalBinary { .. } => false,
    }
}

pub(in crate::proof_contracts::contract_entailment) fn term_is_unobserved_descriptor(
    program: &dyn crate::CheckingInput,
    term: &StructuralTerm,
) -> bool {
    match term {
        StructuralTerm::ObservedValue {
            observed_type,
            value,
            ..
        } => {
            program.primitive_type_reference(*observed_type).is_none()
                || term_is_unobserved_descriptor(program, value)
        }
        StructuralTerm::Application {
            result_type,
            arguments,
            ..
        } => {
            crate::reference_shell(program, *result_type).is_some()
                || arguments
                    .iter()
                    .any(|argument| term_is_unobserved_descriptor(program, argument))
        }
        StructuralTerm::CapturedValue { result_type, .. } => {
            crate::reference_shell(program, *result_type).is_some()
        }
        StructuralTerm::CallProjection {
            result_type,
            field,
            field_name,
            arguments,
            ..
        } => {
            crate::reference_shell(program, *result_type).is_some()
                || program
                    .closed_data_member_field(*result_type, *field, field_name)
                    .is_none_or(|field| {
                        crate::reference_shell(program, field.type_reference).is_some()
                    })
                || arguments
                    .iter()
                    .any(|argument| term_is_unobserved_descriptor(program, argument))
        }
        StructuralTerm::IndexedValue {
            element_type,
            collection,
            index,
            ..
        } => {
            crate::reference_shell(program, *element_type).is_some()
                || term_is_unobserved_descriptor(program, collection)
                || term_is_unobserved_descriptor(program, index)
        }
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_is_unobserved_descriptor(program, value)),
        StructuralTerm::Projection { subject, .. } => {
            term_is_unobserved_descriptor(program, subject)
        }
        StructuralTerm::ScalarBinary {
            references,
            left,
            right,
            ..
        } => {
            !scalar_operand_is_observed(program, references[0], left)
                || !scalar_operand_is_observed(program, references[1], right)
        }
        StructuralTerm::IntegerEmbedding { reference, value } => {
            !scalar_operand_is_observed(program, *reference, value)
        }
        StructuralTerm::MathematicalBinary { left, right, .. } => {
            term_is_unobserved_descriptor(program, left)
                || term_is_unobserved_descriptor(program, right)
        }
        _ => false,
    }
}

/// Recover a produced value's owned carrier through ordinary record fields.
/// The term's declaration-bound result type selects each field; neither a
/// destination type nor a reference's referent can supply a missing crossing.
/// Case payloads need independent active-case evidence and are not record fields.
pub(in crate::proof_contracts::contract_entailment) fn completed_owned_carrier(
    program: &dyn crate::CheckingInput,
    term: &StructuralTerm,
) -> Option<TypeReferenceHandle> {
    let reference = match term {
        StructuralTerm::Application { result_type, .. }
        | StructuralTerm::CapturedValue { result_type, .. } => *result_type,
        StructuralTerm::IndexedValue { element_type, .. } => *element_type,
        StructuralTerm::CallProjection {
            result_type,
            field,
            field_name,
            ..
        } => {
            if crate::reference_shell(program, *result_type).is_some() {
                return None;
            }
            program
                .closed_data_member_field(*result_type, *field, field_name)?
                .type_reference
        }
        StructuralTerm::Projection { subject, path } => {
            let mut reference = completed_owned_carrier(program, subject)?;
            for name in path.split('.') {
                let definition = program.closed_data_definition(reference)?;
                let mut fields = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| {
                        let typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.name.as_str() == name).then_some(field)
                    });
                let field = fields.next()?;
                if fields.next().is_some() {
                    return None;
                }
                reference = program
                    .closed_data_member_field(reference, field.symbol, name)?
                    .type_reference;
                if crate::reference_shell(program, reference).is_some() {
                    return None;
                }
            }
            reference
        }
        _ => return None,
    };
    (reference.is_valid() && crate::reference_shell(program, reference).is_none())
        .then_some(reference)
}

fn has_produced_subject(term: &StructuralTerm) -> bool {
    match term {
        StructuralTerm::Application { .. }
        | StructuralTerm::CapturedValue { .. }
        | StructuralTerm::CallProjection { .. } => true,
        StructuralTerm::Projection { subject, .. } => has_produced_subject(subject),
        StructuralTerm::IndexedValue { collection, .. } => has_produced_subject(collection),
        _ => false,
    }
}
