//! Trait operator applications and selected trait operator meanings.

use crate::TypedTrees;
use crate::type_identity::TypeIdentityRequest;
use crate::typed_trees::declarations::operator::operand_signatures::{
    TypeParameterNormalizer, collect_type_parameter_occurrences, normalized_operand_parameters,
};
use crate::typed_trees::declarations::operator::type_matching::type_reference_matches;
use crate::types::TypeReferenceHandle;
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

/// Whether one exact selected conformance application supplies this
/// trait-owned fixed-token requirement for the complete operand tuple.
///
/// The application is already the proof-static binder selected by generic
/// specialization. Matching therefore consults no visible conformance set: it
/// binds the trait's `Self` owner and declared type parameters from the
/// requirement telescope, then checks those bindings against the closed
/// application's retained subject and trait arguments.
pub fn trait_operator_matches_application(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
    application: &crate::typed_trees::ClosedConformanceApplication,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let parameters = program.state_signature_parameters(requirement);
    if parameters.len() != operand_types.len() || operand_types.iter().all(Option::is_none) {
        return false;
    }

    let type_parameters = program
        .trait_type_parameters(trait_definition)
        .iter()
        .chain(program.state_signature_type_parameters(requirement))
        .cloned()
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    if !operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .all(|(actual, expected)| {
            actual.is_none_or(|actual| {
                type_reference_matches(
                    program,
                    actual,
                    expected.type_reference,
                    Some(trait_definition.symbol),
                    &type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                )
            })
        })
    {
        return false;
    }

    let binding_identity = |symbol| {
        bindings.iter().find_map(|(bound, actual)| {
            (*bound == symbol).then(|| program.display_type_reference(*actual))
        })
    };
    if let Some(subject) = binding_identity(trait_definition.symbol)
        && application.subject_identity.as_deref() != Some(subject.as_str())
    {
        return false;
    }
    application.trait_definition != trait_definition.symbol
        || program
            .trait_type_parameters(trait_definition)
            .iter()
            .zip(&application.trait_arguments)
            .all(|(parameter, expected)| {
                binding_identity(parameter.symbol).is_none_or(|actual| actual == *expected)
            })
}

#[derive(Debug, Clone, Copy)]
pub struct SelectedTraitOperatorMeaning<'program> {
    pub trait_definition: &'program crate::trait_definition::TraitDefinition,
    pub requirement: &'program crate::signature::StateSignature,
    pub application: &'program crate::typed_trees::ClosedConformanceApplication,
    pub row: &'program crate::typed_trees::ClosedConformanceRowIdentity,
}

/// Fixed-token meanings supplied by the proof-static conformance applications
/// already selected on one specialized machine. This is the sole lookup: it
/// walks no package-visible conformance declarations and cannot manufacture an
/// application from operand types.
pub fn selected_trait_operator_meanings<'program>(
    program: &'program TypedTrees,
    machine_symbol: SymbolHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<SelectedTraitOperatorMeaning<'program>> {
    let Some(specialization) = program
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == machine_symbol)
    else {
        return Vec::new();
    };

    specialization
        .conformance_applications
        .iter()
        .flat_map(|application| {
            application.rows.iter().filter_map(move |row| {
                let trait_definition = program
                    .traits()
                    .iter()
                    .find(|candidate| candidate.symbol == row.declaring_trait)?;
                let requirement = program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|candidate| candidate.symbol == row.requirement)?;
                (requirement.spelling == Some(spelling)
                    && trait_operator_matches_application(
                        program,
                        trait_definition,
                        requirement,
                        application,
                        operand_types,
                    ))
                .then_some(SelectedTraitOperatorMeaning {
                    trait_definition,
                    requirement,
                    application,
                    row,
                })
            })
        })
        .collect()
}

/// Canonical operand-type signature for a trait-owned operator requirement.
/// Trait and requirement binders share one alpha-normalized telescope. An
/// attached receiver is always position zero; otherwise the first explicit
/// parameter is position zero, matching expression operand order.
pub fn trait_operator_operand_signature(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
) -> String {
    let mut normalizer = TypeParameterNormalizer::new(
        program
            .trait_type_parameters(trait_definition)
            .iter()
            .chain(program.state_signature_type_parameters(requirement))
            .map(|parameter| parameter.symbol)
            .collect(),
    );
    let parameters = program.state_signature_parameters(requirement);
    for parameter in normalized_operand_parameters(parameters) {
        collect_type_parameter_occurrences(program, parameter.type_reference, &mut normalizer);
    }
    let binders = normalizer.bindings();
    normalized_operand_parameters(parameters)
        .map(|parameter| {
            program
                .type_identity(TypeIdentityRequest {
                    binders: &binders,
                    ..TypeIdentityRequest::ordinary(parameter.type_reference)
                })
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}
