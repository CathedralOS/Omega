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
    StructuralDomainRequirement, StructuralTypeId, UnitPlans, boundary_machine_id,
    checked_unit_boundary_identity, dense_identity, lookup_type_id,
    lower_boundary_content_guarantees, lower_boundary_crash_routes, lower_boundary_parameter_order,
    lower_fixed_boundary_service_reach, lower_published_service_ceiling, lower_unit_parameters,
    terminal_scalar_type, unique_unit_boundary, unsupported,
};
use crate::emission::borrowed_window::terminal_multiplicity;
use crate::terminal_identities::value_id;
use crate::terminal_identities::{StructuralDomainBinding, lookup_domain_id_for_carrier};
use crate::unit::attached_unit::call_closure::BoundaryIdentity;
use crate::unit::attached_unit::catalog::lower_program_local_root_introductions;
use crate::unit::{Proposition, ScalarType, ValueDeclaration};

/// Declare each retained boundary, in roster order, with the parameter roster
/// its callers bind against. Structural parameters take places from
/// `next_place`.
pub(crate) fn lower_declarations(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    boundaries: &[(&CheckedBoundaryMachinePlan, BoundaryIdentity)],
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    domain_ids: &[StructuralDomainBinding],
    service_ids: &[(ServiceReachId, ServiceId)],
    next_place: &mut u64,
) -> Result<(Vec<BoundaryMachineDeclaration>, Vec<BoundaryParameters>), LoweringError> {
    let mut declarations = Vec::with_capacity(boundaries.len());
    let mut parameter_rosters = Vec::with_capacity(boundaries.len());
    for (index, (plan, identity)) in boundaries.iter().enumerate() {
        validate_requirement_kind(checked, plan)?;
        // Stage 04 retained the exact boundary's scalar consequences. Lower
        // that roster; deriving and comparing it again would replay checking
        // against the same source in the same compilation.
        let parameters = lower_unit_parameters(
            identities,
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
        // Terminal call operands use packed structural positions; checked
        // predicates retain authored positions across interleaved scalar lanes.
        // Reuse the allocated places and types, changing only predicate lookup.
        let predicate_parameters = plan
            .structural_parameters
            .iter()
            .zip(&parameters)
            .map(
                |(source, terminal)| terminal_psi::StructuralParameterDeclaration {
                    position: source.position,
                    ..terminal.clone()
                },
            )
            .collect::<Vec<_>>();
        // A scalar predicate may observe a structural formal's extent or
        // member. Boundary contracts use the same declared-input namespace
        // as ordinary bodies, not an empty roster or a provider's body locals.
        let view_roster = predicate_parameters
            .iter()
            .map(|parameter| (parameter.position, parameter.clone()))
            .collect::<Vec<_>>();
        let element_views =
            crate::expression_preparation::bindings::element_views(&view_roster, structural_types);
        let views = crate::scalar_lowering::contracts::ContractViewNamespace {
            parameters: &view_roster,
            result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
            structural_types,
            element_views: &element_views,
        };
        let scalar_requires = lower_scalar_requires(identities, plan, &scalar_parameters, &views)?;
        // The boundary format publishes scalar results, not a structural
        // result place. Retain that exact authored coordinate as unavailable;
        // it is not an extra formal or permission to erase malformed clauses.
        let result = if plan.result.structural_identity().is_some() {
            let signature =
                crate::declarations::target_signature(checked, plan.machine, plan.machine)?;
            let position = u32::try_from(signature.parameters.len()).map_err(|_| {
                LoweringError::Unsupported(
                    "boundary result coordinate exceeds the contract namespace",
                )
            })?;
            crate::scalar_lowering::contracts::ContractStructuralResult::Unavailable { position }
        } else {
            crate::scalar_lowering::contracts::ContractStructuralResult::Absent
        };
        let scalar_ensures = lower_scalar_ensures(
            identities,
            plan,
            &scalar_parameters,
            &crate::scalar_lowering::contracts::ContractViewNamespace { result, ..views },
        )?;
        let crash_routes = lower_boundary_crash_routes(
            identities,
            checked,
            plan,
            &scalar_parameters,
            &predicate_parameters,
            structural_types,
            &scalar_requires,
        )?;
        let requirement_count = scalar_requires.len();
        let id = boundary_machine_id(dense_identity(index)?);
        declarations.push(BoundaryMachineDeclaration {
            declaration: checked
                .machine(plan.machine)
                .map(|_| {
                    Ok::<_, LoweringError>(terminal_psi::CheckedBoundaryCallable {
                        source: crate::machine_lowering::declarations::identity(
                            identities,
                            checked,
                            exact_toolchain_sources,
                            plan.machine,
                        )
                        .map_err(LoweringError::Unsupported)?,
                        method_arguments:
                            crate::retention::conformance_applications::checked_method_arguments(
                                identities,
                                checked,
                                plan.machine,
                            )?,
                    })
                })
                .transpose()?,
            role: terminal_psi::BoundaryDeclarationRole::Callable,
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
            identity: identities.intern(&identity.render(checked)?),
            owner: crate::machine_lowering::declarations::nominal(
                identities,
                checked,
                exact_toolchain_sources,
                plan.contract_owner,
            )
            .map_err(LoweringError::Unsupported)?,
            attachment: plan
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(type_ids, identity))
                .transpose()?,
            scalar_parameters: scalar_parameters.clone(),
            crash_routes,
            structural_parameters: parameters.clone(),
            result: lower_boundary_result(
                identities,
                &plan.result,
                type_ids,
                structural_types,
                domain_ids,
            )?,
            requires,
            program_local_root_introductions: lower_program_local_root_introductions(
                checked,
                plan,
                *identity,
                &parameters,
                domain_ids,
            )?,
            content_guarantees: lower_boundary_content_guarantees(
                &checked.facts.qualifications.content.conservation_plans,
                plan.state,
            )?,
            fixed_service_reach: lower_fixed_boundary_service_reach(checked, plan, service_ids)?,
            owning_service: if plan.kind == semantic_vocabulary::BoundaryRequirementKind::Trait {
                let owner = checked
                    .typed
                    .service_reaches
                    .id_for_symbol(plan.contract_owner)
                    .ok_or(LoweringError::Unsupported(
                        "boundary trait has no service identity",
                    ))?;
                Some(crate::terminal_identities::lookup_service_id(
                    service_ids,
                    owner,
                )?)
            } else {
                None
            },
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
                        .trait_machine_instances(definition)
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    plan: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
    views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
) -> Result<Vec<Proposition>, LoweringError> {
    lower_scalar_contract(identities, &plan.scalar_requires, scalar_types, views)
}

fn lower_scalar_ensures(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    plan: &CheckedBoundaryMachinePlan,
    scalar_types: &[ScalarType],
    views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
) -> Result<Vec<Proposition>, LoweringError> {
    let mut namespace = scalar_types.to_vec();
    if let Some(result) = plan.result.scalar() {
        namespace.push(terminal_scalar_type(result)?);
    }
    let formals = scalar_contract_formals(&namespace)?;
    let mut predicate = |value| {
        crate::scalar_lowering::contracts::proposition(identities, value, &formals, &[], views)
    };
    let mut rows = Vec::new();
    for guarantee in &plan.scalar_ensures {
        use checked_trees::CheckedBoundaryScalarGuarantee;
        let row = (|| match guarantee {
            CheckedBoundaryScalarGuarantee::Predicate(value) => predicate(value),
            CheckedBoundaryScalarGuarantee::Implication {
                premise,
                conclusion,
            } => {
                let premise = predicate(premise);
                let conclusion = predicate(conclusion);
                // An unavailable antecedent cannot hide a lowering failure in
                // its conclusion. Both sides are inspected before deciding
                // whether this whole source-checked consequence can publish.
                match (premise, conclusion) {
                    (Err(error), _) if error != LoweringError::UnavailableContractResult => {
                        Err(error)
                    }
                    (_, Err(error)) if error != LoweringError::UnavailableContractResult => {
                        Err(error)
                    }
                    (Ok(premise), Ok(conclusion)) => Ok(Proposition::Implication {
                        premise: Box::new(premise),
                        conclusion: Box::new(conclusion),
                    }),
                    _ => Err(LoweringError::UnavailableContractResult),
                }
            }
        })();
        match row {
            Ok(row) => rows.push(row),
            // A complete source-checked guarantee without a boundary reading
            // stays in the checked contract. Never strip just its antecedent,
            // select a disjunct or suppress another lowering failure.
            Err(LoweringError::UnavailableContractResult) => {}
            Err(error) => return Err(error),
        }
    }
    canonical_scalar_contract_rows(rows)
}

fn lower_scalar_contract(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    clauses: &[checked_trees::ClosedScalarContractValue],
    scalar_types: &[ScalarType],
    views: &crate::scalar_lowering::contracts::ContractViewNamespace<'_>,
) -> Result<Vec<Proposition>, LoweringError> {
    if clauses.is_empty() {
        return Ok(Vec::new());
    }
    let formals = scalar_contract_formals(scalar_types)?;
    let rows = clauses
        .iter()
        .map(|row| {
            crate::scalar_lowering::contracts::clauses(
                identities,
                std::slice::from_ref(&Some(row.clone())),
                &formals,
                &[],
                views,
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
    crate::scalar_lowering::contracts::canonicalize_requires(&mut rows)?;
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

pub(crate) fn lower_boundary_result(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    result: &CheckedBoundaryMachineResultPlan,
    type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
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
            projected_qualifications,
        } => BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type: lookup_type_id(type_ids, type_identity)?,
            multiplicity: terminal_multiplicity(*multiplicity),
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
            projected_qualifications: super::parameters::lower_projected_qualifications(
                identities,
                projected_qualifications,
                domain_ids,
                lookup_type_id(type_ids, type_identity)?,
                structural_types,
            )?,
        }),
    })
}

/// Add the boundary `machine` names to the closure roster once, under its
/// canonical identity. Stage 04 planned each call against this exact target.
pub(crate) fn retain_unit_boundary<'plans>(
    checked: &CheckedTrees,
    plans: UnitPlans<'plans>,
    boundaries: &mut Vec<(&'plans CheckedBoundaryMachinePlan, BoundaryIdentity)>,
    machine: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let target = unique_unit_boundary(plans, machine)?;
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
            .machine(plan.machine)
            .and_then(|machine| machine.termination_plan.interface.published()),
        BoundaryRequirementKind::Trait => checked
            .typed
            .trait_definition_by_symbol(plan.contract_owner)
            .and_then(|definition| {
                checked
                    .typed
                    .trait_machine_signature_by_symbol(definition, plan.machine)
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
