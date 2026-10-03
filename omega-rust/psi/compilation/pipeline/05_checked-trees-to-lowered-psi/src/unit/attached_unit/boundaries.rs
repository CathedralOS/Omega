//! Boundary machines a Unit closure calls: retaining each exact checked
//! target once, and declaring the retained roster in the closure's shared
//! type, domain, and service namespaces.
//!
//! Both the shared closure and a standalone composed root declare their
//! boundaries here, so a boundary's parameters, requirements, crash routes,
//! and service ceilings have one lowering.

use super::super::{ServiceId, ServiceReachId};
use super::operation_frame::BoundaryParameters;
use super::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedTrees, LoweringError,
    Multiplicity, ServiceReachSummary, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralTypeId, UnitPlans, boundary_machine_id, checked_unit_boundary_identity,
    checked_unit_target_reach_matches, dense_identity, lookup_type_id,
    lower_boundary_content_guarantees, lower_boundary_crash_routes, lower_boundary_parameter_order,
    lower_fixed_boundary_service_reach, lower_published_service_ceiling, lower_unit_parameters,
    terminal_scalar_type, unique_unit_boundary, unsupported,
};
use crate::terminal_identities::value_id;
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use crate::unit::attached_unit::catalog::lower_program_local_root_introductions;
use crate::unit::{Proposition, ScalarType, ValueDeclaration};

/// Declare each retained boundary, in roster order, with the parameter roster
/// its callers bind against. Structural parameters take places from
/// `next_place`.
pub(crate) fn lower_declarations(
    checked: &CheckedTrees,
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    domain_ids: &[StructuralDomainBinding],
    service_ids: &[(ServiceReachId, ServiceId)],
    next_place: &mut u64,
) -> Result<(Vec<BoundaryMachineDeclaration>, Vec<BoundaryParameters>), LoweringError> {
    let mut declarations = Vec::with_capacity(boundaries.len());
    let mut parameter_rosters = Vec::with_capacity(boundaries.len());
    for (index, (plan, identity)) in boundaries.iter().enumerate() {
        validate_requirement_kind(checked, plan)?;
        if let Some(machine) = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.machine)
        {
            crate::scalar_graph::scalar_contracts::validate_boundary_guarantees(checked, machine)?;
        }
        let expected_guarantees = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.machine)
            .and_then(|machine| {
                checked
                    .facts
                    .contract_plans
                    .for_machine(machine.symbol)
                    .map(|contract| {
                        validation::derive_scalar_boundary_guarantees(
                            &checked.typed,
                            &checked.facts,
                            machine,
                            contract,
                        )
                    })
            })
            .unwrap_or_default();
        if plan.scalar_ensures != expected_guarantees {
            return unsupported("boundary scalar guarantees disagree with the authored contract");
        }
        let parameters = lower_unit_parameters(
            &plan.structural_parameters,
            type_ids,
            structural_types,
            domain_ids,
            next_place,
        )?;
        let scalar_parameters = plan
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let requires = lower_requirements(plan, &parameters, domain_ids)?;
        let scalar_requires = lower_scalar_requires(plan, &scalar_parameters)?;
        let scalar_ensures = lower_scalar_ensures(plan, &scalar_parameters)?;
        let requirement_count = scalar_requires.len();
        let id = boundary_machine_id(dense_identity(index)?);
        declarations.push(BoundaryMachineDeclaration {
            unconditional_termination: unconditional_termination(checked, plan)?,
            completion: validation::boundary_requirement_completion(&checked.typed, plan.machine),
            kind: plan.kind,
            scalar_requires,
            scalar_ensures,
            parameter_order: lower_boundary_parameter_order(
                &plan.scalar_parameters,
                &plan.structural_parameters,
            )?,
            id,
            identity: identity.clone(),
            attachment: plan
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(type_ids, identity))
                .transpose()?,
            scalar_parameters: scalar_parameters.clone(),
            crash_routes: lower_boundary_crash_routes(checked, plan, &scalar_parameters)?,
            structural_parameters: parameters.clone(),
            result: lower_boundary_result(&plan.result, type_ids, domain_ids)?,
            requires,
            program_local_root_introductions: lower_program_local_root_introductions(
                checked,
                plan,
                identity,
                &parameters,
                domain_ids,
            )?,
            content_guarantees: lower_boundary_content_guarantees(
                &checked.facts.qualifications.content.conservation_plans,
                plan.state,
            )?,
            fixed_service_reach: lower_fixed_boundary_service_reach(checked, plan, service_ids)?,
            published_service_ceiling: lower_published_service_ceiling(
                &checked.facts.service_reaches.rows,
                plan.contract_service_reach,
                plan.service_reach,
                service_ids,
            )?,
        });
        parameter_rosters.push(BoundaryParameters {
            source: plan.machine,
            id,
            structural: parameters,
            scalar: scalar_parameters,
            requirement_count,
        });
    }
    Ok((declarations, parameter_rosters))
}

/// Rejoin the published role to its authored owner. Handle equality is not
/// a declaration kind, and a selected implementation cannot supply this fact.
pub(crate) fn validate_requirement_kind(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryMachinePlan,
) -> Result<(), LoweringError> {
    use semantic_vocabulary::BoundaryRequirementKind;
    let matches_source =
        match plan.kind {
            BoundaryRequirementKind::Machine => checked.typed.machines().iter().any(|machine| {
                machine.symbol == plan.machine && plan.contract_owner == machine.symbol
            }),
            BoundaryRequirementKind::Trait => checked.typed.traits().iter().any(|definition| {
                definition.is_boundary
                    && definition.symbol == plan.contract_owner
                    && checked
                        .typed
                        .trait_machine_signatures(definition)
                        .iter()
                        .any(|signature| {
                            signature.symbol == plan.machine && plan.state == signature.symbol
                        })
            }),
            BoundaryRequirementKind::Operator => checked.typed.operators().iter().any(|operator| {
                operator.is_boundary
                    && operator.symbol == plan.machine
                    && plan.state == operator.symbol
                    && plan.contract_owner == operator.symbol
            }),
        };
    if !matches_source
        || (plan.kind != BoundaryRequirementKind::Machine
            && plan.attachment_type_identity.is_some())
    {
        return unsupported("boundary requirement kind disagrees with its authored owner");
    }
    Ok(())
}

/// A boundary's scalar `requires` rows in its declaration-local formal
/// telescope (scalar lane position `k` is formal `ValueId` `k + 1`, the crash
/// routes' namespace), normalized as a callee's published requires are: no
/// `true` rows, conjunctions split into rows, canonical order, no repeats.
fn lower_scalar_requires(
    plan: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
) -> Result<Vec<Proposition>, LoweringError> {
    lower_scalar_contract(&plan.scalar_requires, scalar_types)
}

fn lower_scalar_ensures(
    plan: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
) -> Result<Vec<Proposition>, LoweringError> {
    let mut namespace = scalar_types.to_vec();
    if let Some(result) = plan.result.scalar() {
        namespace.push(terminal_scalar_type(result)?);
    }
    let formals = scalar_contract_formals(&namespace)?;
    let predicate = |value| {
        crate::scalar_graph::scalar_contracts::proposition(
            value,
            &formals,
            &[],
            &crate::scalar_graph::scalar_contracts::ContractViewNamespace::EMPTY,
        )
    };
    let rows = plan
        .scalar_ensures
        .iter()
        .map(|guarantee| {
            use checked_trees::CheckedBoundaryScalarGuarantee;
            Ok(match guarantee {
                CheckedBoundaryScalarGuarantee::Predicate(value) => predicate(value)?,
                CheckedBoundaryScalarGuarantee::Implication {
                    premise,
                    conclusion,
                } => Proposition::Implication {
                    premise: Box::new(predicate(premise)?),
                    conclusion: Box::new(predicate(conclusion)?),
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    canonical_scalar_contract_rows(rows)
}

fn lower_scalar_contract(
    clauses: &[checked_trees::ClosedScalarContractValue],
    scalar_types: &[ScalarType],
) -> Result<Vec<Proposition>, LoweringError> {
    if clauses.is_empty() {
        return Ok(Vec::new());
    }
    let formals = scalar_contract_formals(scalar_types)?;
    let rows = clauses
        .iter()
        .map(|row| {
            crate::scalar_graph::scalar_contracts::clauses(
                std::slice::from_ref(&Some(row.clone())),
                &formals,
                &[],
                &crate::scalar_graph::scalar_contracts::ContractViewNamespace::EMPTY,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    canonical_scalar_contract_rows(rows)
}

fn scalar_contract_formals(
    scalar_types: &[ScalarType],
) -> Result<Vec<ValueDeclaration>, LoweringError> {
    scalar_types
        .iter()
        .enumerate()
        .map(|(position, scalar_type)| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(position)?),
                scalar_type: *scalar_type,
                qualifications: Default::default(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()
}

fn canonical_scalar_contract_rows(
    propositions: Vec<Proposition>,
) -> Result<Vec<Proposition>, LoweringError> {
    let mut pending = propositions.into_iter().rev().collect::<Vec<_>>();
    let mut rows = Vec::new();
    while let Some(row) = pending.pop() {
        match row {
            Proposition::Truth => {}
            Proposition::Conjunction(conjuncts) => pending.extend(conjuncts.into_iter().rev()),
            other => rows.push(other),
        }
    }
    crate::scalar_graph::scalar_contracts::canonicalize_requires(&mut rows)?;
    Ok(rows)
}

/// A boundary's structural domain requirements, each naming one of its
/// structural parameters, in canonical order and without repetition.
fn lower_requirements(
    plan: &CheckedBoundaryMachinePlan,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    domain_ids: &[StructuralDomainBinding],
) -> Result<Vec<StructuralDomainRequirement>, LoweringError> {
    let mut requires = plan
        .domain_requirements
        .iter()
        .map(|requirement| {
            let parameter = parameters.get(requirement.argument_index as usize).ok_or(
                LoweringError::Unsupported(
                    "boundary structural requirement has an invalid argument index",
                ),
            )?;
            Ok(StructuralDomainRequirement {
                argument_index: requirement.argument_index,
                domain: lookup_domain_id_for_carrier(
                    domain_ids,
                    requirement.domain,
                    parameter.structural_type,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    requires.sort();
    let original_requirement_count = requires.len();
    requires.dedup();
    if requires.len() != original_requirement_count {
        return unsupported("boundary structural requirements contain duplicates");
    }
    Ok(requires)
}

pub(super) fn lower_boundary_result(
    result: &CheckedBoundaryMachineResultPlan,
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[StructuralDomainBinding],
) -> Result<BoundaryMachineResult, LoweringError> {
    Ok(match result {
        CheckedBoundaryMachineResultPlan::Unit => BoundaryMachineResult::Unit,
        CheckedBoundaryMachineResultPlan::Scalar(scalar) => {
            BoundaryMachineResult::Scalar(terminal_scalar_type(*scalar)?)
        }
        CheckedBoundaryMachineResultPlan::Structural {
            type_identity,
            multiplicity,
            qualifications,
        } => BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type: lookup_type_id(type_ids, type_identity)?,
            multiplicity: match multiplicity {
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: qualifications
                .iter()
                .map(|domain| {
                    lookup_domain_id_for_carrier(
                        domain_ids,
                        *domain,
                        lookup_type_id(type_ids, type_identity)?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
    })
}

/// Retain the boundary one call names after matching that call's exact
/// target state, contract fingerprint, result and service reach.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retain_exact_unit_boundary<'plans>(
    checked: &CheckedTrees,
    plans: UnitPlans<'plans>,
    boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, String)>,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    target_contract_report_fingerprint: u64,
    service_reach: ServiceReachSummary,
    expected_result: CheckedBoundaryMachineResultPlan,
) -> Result<(), LoweringError> {
    let target = unique_unit_boundary(plans, target_machine)?;
    if target.state != target_state
        || target.contract_report_fingerprint != target_contract_report_fingerprint
        || target.result != expected_result
        || !checked_unit_target_reach_matches(service_reach, target.contract_service_reach)
    {
        return unsupported(
            "Unit boundary call does not match the exact checked target state, result, contract, and reach",
        );
    }
    retain_unit_boundary_identity(checked, boundaries, target)
}

/// Retain a boundary that carries no call of its own to match: one a
/// composed callee's calls already admitted through
/// `retain_exact_unit_boundary`, or one an external owner names as a root.
///
/// The plan's own `service_reach` is the declaration's checked-inferred
/// reach, which `lower_contract_service_ceiling` requires only to lie within
/// its published ceiling. It is not a call's reach: a bodyless top-level
/// `boundary requirement` infers the empty row while publishing its authored
/// ceiling, so comparing the plan against itself as if it were a call rejects
/// every such requirement with a nonempty ceiling.
pub(crate) fn retain_admitted_unit_boundary<'plans>(
    checked: &CheckedTrees,
    plans: UnitPlans<'plans>,
    boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, String)>,
    admitted: &CheckedBoundaryMachinePlan,
) -> Result<(), LoweringError> {
    let target = unique_unit_boundary(plans, admitted.machine)?;
    if target != admitted {
        return unsupported("admitted Unit boundary is not its unique checked target");
    }
    retain_unit_boundary_identity(checked, boundaries, target)
}

/// Rejoin a retained target's contract fingerprint and commitment to its
/// canonical owner, then add it to the closure roster once.
fn retain_unit_boundary_identity<'plans>(
    checked: &CheckedTrees,
    boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, String)>,
    target: &'plans CheckedBoundaryMachinePlan,
) -> Result<(), LoweringError> {
    if target.contract_report_fingerprint == 0 {
        return unsupported("Unit boundary target has a null checked contract fingerprint");
    }
    let exact_identity = checked
        .facts
        .contract_plans
        .for_machine(target.contract_owner)
        .map(|contract| (contract.report_fingerprint, contract.commitment))
        .or_else(|| {
            checked
                .facts
                .contract_plans
                .crash_capsule(target.contract_owner, target.state)
                .map(|capsule| {
                    (
                        capsule.target_contract_report_fingerprint(),
                        capsule.target_contract_commitment(),
                    )
                })
        })
        .ok_or(LoweringError::Unsupported(
            "Unit boundary target is missing its canonical checked contract identity",
        ))?;
    if (
        target.contract_report_fingerprint,
        target.contract_commitment,
    ) != exact_identity
    {
        return unsupported(
            "Unit boundary target contract compatibility coordinate or strong commitment drifted",
        );
    }
    if !boundaries
        .iter()
        .any(|(candidate, _)| candidate.machine == target.machine)
    {
        boundaries.push((
            target,
            checked_unit_boundary_identity(checked, target.machine)?,
        ));
    }
    Ok(())
}

/// Transport only the unconditional projection of the exact published
/// requirement. Conditional progress subjects remain unproved by this bit.
fn unconditional_termination(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryMachinePlan,
) -> Result<bool, LoweringError> {
    use language_semantics::TerminationGuarantee;
    use semantic_vocabulary::BoundaryRequirementKind;
    let guarantee = match plan.kind {
        BoundaryRequirementKind::Machine => checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == plan.machine)
            .and_then(|machine| machine.termination_plan.interface.published()),
        BoundaryRequirementKind::Trait => checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.symbol == plan.contract_owner)
            .and_then(|definition| {
                checked
                    .typed
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == plan.machine)
            })
            .map(|signature| &signature.termination_guarantee),
        // The current operator requirement vocabulary publishes no progress
        // contract. Its result type or chosen implementation cannot supply one.
        BoundaryRequirementKind::Operator => None,
    };
    Ok(
        matches!(guarantee, Some(TerminationGuarantee::Terminates { premises }) if premises.is_empty()),
    )
}
