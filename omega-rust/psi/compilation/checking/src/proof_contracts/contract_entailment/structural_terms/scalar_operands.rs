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
        StructuralTerm::Variable(_)
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
        }
        StructuralTerm::IndexedValue { element_type, .. } => {
            crate::value_custody::expression_types::scalar_observation_meaning(program, declared)
                == crate::value_custody::expression_types::scalar_observation_meaning(
                    program,
                    *element_type,
                )
        }
        StructuralTerm::CallProjection { .. } => true,
        // A produced aggregate's reference field is still a descriptor. An
        // exact input subject projection instead starts at its retained binder.
        StructuralTerm::Projection { subject, .. } => !has_produced_subject(subject),
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
