//! Distinct custody judgments over one exact, cycle-bounded carrier graph.
//!
//! Quotient formation cannot identify ordinary affine occurrences. Prop value
//! observation may describe checked owned affine shapes without copying them,
//! but cannot replay linear, borrowed, opaque, carry or routed authority.

use std::collections::HashSet;

use typed_trees::domain::{DomainAliasDefinition, DomainDefinition};
use typed_trees::proof_only::ProofOnlyClassification;
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CarrierFenceViolation {
    UnshareableType(String),
    RoutedQualification(String),
}

pub(crate) fn first_forbidden_quotient_content(
    program: &dyn crate::CheckingInput,
    proof_only: &ProofOnlyClassification,
    type_reference: TypeReferenceHandle,
    visited_data: &mut HashSet<u32>,
) -> Option<CarrierFenceViolation> {
    first_forbidden_content(
        program,
        proof_only,
        type_reference,
        visited_data,
        ContentUse::Quotient,
    )
}

/// Observing a semantic value in Prop is not quotient substitution or runtime
/// copying. Exact checked shapes retain their declared Type multiplicity.
pub(crate) fn type_has_forbidden_value_observation_content(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    first_forbidden_content(
        program,
        &crate::proof_only_classification(program),
        type_reference,
        &mut HashSet::new(),
        ContentUse::ValueObservation,
    )
    .is_some()
}

/// A shared input observes its checked referent for this invocation; it does
/// not return or duplicate the reference as a denotational value. Only this
/// outer parameter view is peeled. Nested references, linear contents and
/// routed qualifications retain the ordinary value-content fence.
pub(crate) fn type_has_forbidden_argument_observation_content(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            access: language_semantics::ReferenceAccess::Shared,
            ..
        } => shared_referent_has_forbidden_observation_content(program, *referee),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(domain)
                        if first_routed_qualification(program, domain, &mut HashSet::new()).is_some())
                })
                || type_has_forbidden_argument_observation_content(program, *base_type)
        }
        _ => type_has_forbidden_value_observation_content(program, type_reference),
    }
}

/// A shared slice input exposes its elements to this invocation, without
/// turning its descriptor into a proof value. Element contents still pass
/// the value fence, so nested references and resource authority stay opaque.
fn shared_referent_has_forbidden_observation_content(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Slice { element_type } => {
            type_has_forbidden_value_observation_content(program, *element_type)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(domain)
                        if first_routed_qualification(program, domain, &mut HashSet::new()).is_some())
                })
                || shared_referent_has_forbidden_observation_content(program, *base_type)
        }
        _ => type_has_forbidden_value_observation_content(program, type_reference),
    }
}

#[derive(Clone, Copy)]
enum ContentUse {
    Quotient,
    ValueObservation,
}

fn first_forbidden_content(
    program: &dyn crate::CheckingInput,
    proof_only: &ProofOnlyClassification,
    type_reference: TypeReferenceHandle,
    visited_data: &mut HashSet<u32>,
    purpose: ContentUse,
) -> Option<CarrierFenceViolation> {
    // A selected layout changes storage geometry, not the value being observed.
    // Keep qualification checks outside the representation: a routed domain
    // on the storage still carries its own authority restriction.
    if !matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Constrained { .. }
    ) && let Some(storage) = program.layout_storage_arguments(type_reference)
    {
        return first_forbidden_content(
            program,
            proof_only,
            storage.semantic_type,
            visited_data,
            purpose,
        );
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            // Primitive classification peels qualifications and maps dynamic
            // selectors, so it is safe only after matching the actual Named
            // node. Routed shells and dynamic custody must reach their fences.
            if program
                .type_reference_table
                .primitive_type(type_reference)
                .is_some()
            {
                return None;
            }
            if let Some(definition) =
                crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
            {
                return first_forbidden_data_content(
                    program,
                    proof_only,
                    definition,
                    visited_data,
                    purpose,
                );
            }
            if matches!(purpose, ContentUse::ValueObservation)
                && matches!(
                    program.symbols.builtin_type_atom(*symbol),
                    Some(symbols::BuiltinTypeAtom::Int | symbols::BuiltinTypeAtom::UInt)
                )
            {
                return None;
            }
            let parameter = match purpose {
                ContentUse::Quotient => program
                    .data_definitions()
                    .iter()
                    .flat_map(|definition| program.data_type_parameters(definition))
                    .find(|parameter| parameter.symbol == *symbol),
                ContentUse::ValueObservation => program.data_type_parameter_by_symbol(*symbol),
            };
            let Some(parameter) = parameter else {
                return matches!(purpose, ContentUse::ValueObservation)
                    .then(|| CarrierFenceViolation::UnshareableType(name.as_str().to_owned()));
            };
            (matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                && (match purpose {
                    ContentUse::Quotient => {
                        parameter.bounds.multiplicity
                            != language_semantics::Multiplicity::Unrestricted
                    }
                    ContentUse::ValueObservation => {
                        parameter.bounds.multiplicity == language_semantics::Multiplicity::Linear
                    }
                } || parameter.bounds.carry.is_some()))
            .then(|| CarrierFenceViolation::UnshareableType(name.as_str().to_owned()))
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let definition =
                crate::declarations::data_queries::data_definition_by_symbol(program, *base_symbol);
            let Some(definition) = definition else {
                return matches!(purpose, ContentUse::ValueObservation).then(|| {
                    CarrierFenceViolation::UnshareableType(
                        program.display_type_reference(type_reference),
                    )
                });
            };
            if matches!(purpose, ContentUse::ValueObservation)
                && program.data_type_parameters(definition).len() != arguments.len()
            {
                return Some(CarrierFenceViolation::UnshareableType(
                    program.display_type_reference(type_reference),
                ));
            }
            for (parameter, argument) in program.data_type_parameters(definition).iter().zip(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            ) {
                if matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    && let Some(forbidden) = first_forbidden_content(
                        program,
                        proof_only,
                        *argument,
                        visited_data,
                        purpose,
                    )
                {
                    return Some(forbidden);
                }
            }
            first_forbidden_data_content(program, proof_only, definition, visited_data, purpose)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint
                    && let Some(name) =
                        first_routed_qualification(program, domain, &mut HashSet::new())
                {
                    return Some(CarrierFenceViolation::RoutedQualification(name));
                }
            }
            first_forbidden_content(program, proof_only, *base_type, visited_data, purpose)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            first_forbidden_content(program, proof_only, *element_type, visited_data, purpose)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_) => Some(CarrierFenceViolation::UnshareableType(
            program.display_type_reference_with_constraints(type_reference),
        )),
        TypeReferenceNode::Unit => None,
    }
}

fn first_routed_qualification(
    program: &dyn crate::CheckingInput,
    constraint: &DomainConstraint,
    visited_aliases: &mut HashSet<u32>,
) -> Option<String> {
    if !constraint.establishment_routes.is_empty() {
        return Some(constraint.name.as_str().to_owned());
    }
    let definition = constraint
        .symbol
        .is_valid()
        .then(|| program.domain_definition(constraint.symbol));
    match definition {
        Some(Some(definition)) => {
            first_routed_domain_definition(program, definition, visited_aliases)
        }
        _ => None,
    }
}

fn first_routed_domain_definition(
    program: &dyn crate::CheckingInput,
    definition: &DomainDefinition,
    visited_aliases: &mut HashSet<u32>,
) -> Option<String> {
    if let Some(alias) = definition.alias.as_ref() {
        if !visited_aliases.insert(definition.symbol.arena_index()) {
            return None;
        }
        let routed = first_routed_alias_constituent(program, alias, visited_aliases);
        visited_aliases.remove(&definition.symbol.arena_index());
        return routed;
    }
    (!definition.establishment_routes.is_empty()).then(|| definition.name.as_str().to_owned())
}

fn first_routed_alias_constituent(
    program: &dyn crate::CheckingInput,
    alias: &DomainAliasDefinition,
    visited_aliases: &mut HashSet<u32>,
) -> Option<String> {
    alias.constituents.iter().find_map(|constituent| {
        program
            .domain_definition(constituent.domain_symbol)
            .and_then(|definition| {
                first_routed_domain_definition(program, definition, visited_aliases)
            })
    })
}

fn first_forbidden_data_content(
    program: &dyn crate::CheckingInput,
    proof_only: &ProofOnlyClassification,
    definition: &typed_trees::data::DataDefinition,
    visited_data: &mut HashSet<u32>,
    purpose: ContentUse,
) -> Option<CarrierFenceViolation> {
    // Guard every carrier graph, including runtime copy data. Formation does
    // not decide finite layout; representation demand owns that judgment.
    if !visited_data.insert(definition.symbol.arena_index()) {
        return None;
    }
    let forbidden = match purpose {
        ContentUse::Quotient => {
            !proof_only.is_proof_only(definition.symbol)
                && (definition.properties.multiplicity
                    != language_semantics::Multiplicity::Unrestricted
                    || definition.properties.carry.is_some())
        }
        ContentUse::ValueObservation => {
            definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                || definition.properties.multiplicity == language_semantics::Multiplicity::Linear
                || definition.properties.carry.is_some()
        }
    };
    if forbidden {
        return Some(CarrierFenceViolation::UnshareableType(
            definition.name.as_str().to_owned(),
        ));
    }

    for field in program.data_member_fields(program.data_members(definition)) {
        if let Some(forbidden) = first_forbidden_content(
            program,
            proof_only,
            field.type_reference,
            visited_data,
            purpose,
        ) {
            return Some(forbidden);
        }
    }
    None
}
