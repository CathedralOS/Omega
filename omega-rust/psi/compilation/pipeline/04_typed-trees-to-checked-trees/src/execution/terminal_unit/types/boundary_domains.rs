//! Boundary membership admission has two value lanes, not two proof rules.
//! Classify the exact subject before choosing its representation. Scalar
//! predicates and nominal identity remain independent; neither provider choice
//! nor projection of the numeric component can establish routed membership.

use super::signature_contracts::{
    normal_mutable_parameter_position, signature_progress_requirement,
};
use super::{
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralParameterPlan, ShapeCollector,
};
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryScalarDomainRequirementPlan,
    CheckedStructuralScalarParameterPlan,
};
use symbols::SymbolHandle;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};

pub(in crate::execution::terminal_unit) struct BoundaryDomainRequirements {
    pub structural: Vec<CheckedUnitStructuralDomainRequirementPlan>,
    pub normal: Vec<CheckedUnitStructuralDomainRequirementPlan>,
    pub scalar: Vec<CheckedBoundaryScalarDomainRequirementPlan>,
}

pub(in crate::execution::terminal_unit) fn boundary_domain_requirements<'a>(
    program: &dyn validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    source_parameters: &[StateParameter],
    contracts: impl Iterator<Item = &'a SignatureContract>,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<BoundaryDomainRequirements> {
    let mut structural = Vec::new();
    let mut scalar = Vec::new();
    let mut normal = Vec::new();
    for (argument_index, parameter) in structural_parameters.iter().enumerate() {
        for domain in &parameter.qualifications {
            structural.push(CheckedUnitStructuralDomainRequirementPlan {
                argument_index: u32::try_from(argument_index).ok()?,
                domain: *domain,
            });
        }
    }
    for contract in contracts.filter(|contract| {
        matches!(
            contract.kind,
            SignatureContractKind::Requires | SignatureContractKind::Ensures
        )
    }) {
        let entry = contract.kind == SignatureContractKind::Requires;
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            if entry
                && let Some(valid) =
                    signature_progress_requirement(program, source_parameters, membership)
            {
                if !valid {
                    return None;
                }
                continue;
            }
            // The published formal owns the value lane. A structural subject
            // can expose a scalar observation (for example a borrowed u32),
            // but that observation does not replace its reference or move its
            // nominal requirement into a nonexistent scalar argument slot.
            let source_position = validation::single_segment_name_symbol(program, membership.value)
                .and_then(|root| {
                    source_parameters
                        .iter()
                        .position(|parameter| parameter.symbol == root)
                });
            // Result guarantees have their own owner. This normal roster
            // preserves existing memberships on one exact mutable formal;
            // it neither infers projected subjects nor grants new membership.
            if !entry
                && normal_mutable_parameter_position(program, source_parameters, membership.value)
                    .is_none()
            {
                continue;
            }
            let primitive_type = validation::scalar_membership_value_carrier(program, membership);
            if primitive_type.is_some()
                && !validation::field_domains::domain_requires_provenance(
                    program,
                    membership.domain_symbol,
                )
                && !validation::field_domains::domain_is_vacuous(
                    program,
                    membership.domain_symbol,
                    &mut Vec::new(),
                )
            {
                // Predicate-only scalar membership has an independent numeric
                // contract owner. Do not replace that predicate with opaque
                // identity merely because its subject occupies a structural slot.
                continue;
            }
            if let Some(argument_index) = source_position.and_then(|source_position| {
                structural_parameters
                    .iter()
                    .position(|parameter| parameter.position as usize == source_position)
            }) {
                let domain = membership.semantic_domain;
                if !domain.is_valid() {
                    return None;
                }
                let carrier = program
                    .unwrapped_type_reference(source_parameters[source_position?].type_reference)?;
                shapes.add_selected_domain(
                    domain,
                    membership.domain_symbol,
                    carrier,
                    binders,
                    false,
                )?;
                let row = CheckedUnitStructuralDomainRequirementPlan {
                    argument_index: u32::try_from(argument_index).ok()?,
                    domain,
                };
                if entry {
                    structural.push(row);
                } else {
                    normal.push(row);
                }
                continue;
            }
            if !entry {
                continue;
            }
            if let Some(primitive_type) = primitive_type {
                let source_position = source_position?;
                let argument_index = scalar_parameters
                    .iter()
                    .position(|parameter| parameter.source_position as usize == source_position)?;
                if scalar_parameters[argument_index].primitive_type != primitive_type
                    || !membership.semantic_domain.is_valid()
                {
                    return None;
                }
                let subject = checked_trees::CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                };
                let predicate =
                    validation::scalar_membership_value_predicates(program, membership, &subject)?
                        .into_iter()
                        .reduce(|left, right| CheckedBooleanExpression::And {
                            left: Box::new(left),
                            right: Box::new(right),
                        })
                        .unwrap_or(CheckedBooleanExpression::Constant(true));
                scalar.push(CheckedBoundaryScalarDomainRequirementPlan {
                    argument_index: u32::try_from(argument_index).ok()?,
                    domain_symbol: membership.domain_symbol,
                    domain: membership.semantic_domain,
                    primitive_type,
                    predicate,
                });
                continue;
            }
            return None;
        }
    }
    structural.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
    structural.dedup();
    scalar.sort_by_key(|requirement| (requirement.argument_index, requirement.domain.0));
    if scalar.windows(2).any(|pair| {
        pair[0].argument_index == pair[1].argument_index
            && pair[0].domain == pair[1].domain
            && pair[0] != pair[1]
    }) {
        return None;
    }
    scalar.dedup();
    normal.sort_by_key(|row| (row.argument_index, row.domain.0));
    normal.dedup();
    Some(BoundaryDomainRequirements {
        structural,
        scalar,
        normal,
    })
}
