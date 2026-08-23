//! Initial quotient-carrier custody fence.
//!
//! Quotient formation may traverse recursive proof data, but it cannot make
//! an ordinary affine or linear Type occurrence or routed qualification
//! substitutable. This module owns that exact, cycle-bounded carrier-graph
//! walk.

use std::collections::HashSet;

use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::{DomainAliasDefinition, DomainDefinition};
use psi_typed_trees::proof_only::ProofOnlyClassification;
use psi_typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CarrierFenceViolation {
    NonCopyType(String),
    RoutedQualification(String),
}

pub(super) fn first_forbidden_carrier_content(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    type_reference: TypeReferenceHandle,
    visited_proof_data: &mut HashSet<u32>,
) -> Option<CarrierFenceViolation> {
    if program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some()
    {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
            {
                return first_forbidden_data_content(
                    program,
                    proof_only,
                    definition,
                    visited_proof_data,
                );
            }
            let parameter = program
                .data_definitions()
                .iter()
                .flat_map(|definition| program.data_type_parameters(definition))
                .find(|parameter| parameter.symbol == *symbol)?;
            (matches!(
                parameter.kind,
                psi_typed_trees::data::TypeParameterKind::Type
            ) && (parameter.bounds.multiplicity
                != psi_language_semantics::Multiplicity::Unrestricted
                || parameter.bounds.carry.is_some()))
            .then(|| CarrierFenceViolation::NonCopyType(name.as_str().to_owned()))
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *base_symbol)?;
            for (parameter, argument) in program.data_type_parameters(definition).iter().zip(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            ) {
                if matches!(
                    parameter.kind,
                    psi_typed_trees::data::TypeParameterKind::Type
                ) && let Some(forbidden) = first_forbidden_carrier_content(
                    program,
                    proof_only,
                    *argument,
                    visited_proof_data,
                ) {
                    return Some(forbidden);
                }
            }
            first_forbidden_data_content(program, proof_only, definition, visited_proof_data)
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
            first_forbidden_carrier_content(program, proof_only, *base_type, visited_proof_data)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            first_forbidden_carrier_content(program, proof_only, *element_type, visited_proof_data)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_) => Some(CarrierFenceViolation::NonCopyType(
            program.display_type_reference_with_constraints(type_reference),
        )),
        TypeReferenceNode::Unit => None,
    }
}

fn first_routed_qualification(
    program: &TypedTrees,
    constraint: &DomainConstraint,
    visited_aliases: &mut HashSet<u32>,
) -> Option<String> {
    if !constraint.establishment_routes.is_empty() {
        return Some(constraint.name.as_str().to_owned());
    }
    let definition = constraint.symbol.is_valid().then(|| {
        program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == constraint.symbol)
    });
    match definition {
        Some(Some(definition)) => {
            first_routed_domain_definition(program, definition, visited_aliases)
        }
        _ => None,
    }
}

fn first_routed_domain_definition(
    program: &TypedTrees,
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
    program: &TypedTrees,
    alias: &DomainAliasDefinition,
    visited_aliases: &mut HashSet<u32>,
) -> Option<String> {
    alias.constituents.iter().find_map(|constituent| {
        program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == constituent.domain_symbol)
            .and_then(|definition| {
                first_routed_domain_definition(program, definition, visited_aliases)
            })
    })
}

fn first_forbidden_data_content(
    program: &TypedTrees,
    proof_only: &ProofOnlyClassification,
    definition: &psi_typed_trees::data::DataDefinition,
    visited_proof_data: &mut HashSet<u32>,
) -> Option<CarrierFenceViolation> {
    if proof_only.is_proof_only(definition.symbol) {
        if !visited_proof_data.insert(definition.symbol.arena_index()) {
            return None;
        }
    } else if definition.properties.multiplicity
        != psi_language_semantics::Multiplicity::Unrestricted
        || definition.properties.carry.is_some()
    {
        return Some(CarrierFenceViolation::NonCopyType(
            definition.name.as_str().to_owned(),
        ));
    }

    for member in program.data_members(definition) {
        let fields = match member {
            psi_typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
            psi_typed_trees::data::DataMember::Variant(variant) => {
                program.data_payload_fields(variant)
            }
        };
        for field in fields {
            if let Some(forbidden) = first_forbidden_carrier_content(
                program,
                proof_only,
                field.type_reference,
                visited_proof_data,
            ) {
                return Some(forbidden);
            }
        }
    }
    None
}
