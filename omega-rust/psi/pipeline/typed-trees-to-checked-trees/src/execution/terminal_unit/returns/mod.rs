//! Structural Unit and scalar return analysis.
//!
//! This file builds the four return plan sets. `affine_returns.rs` builds
//! claim-free affine return machines, `guarded_call_returns.rs` payloadless
//! guarded call returns,
//! `structural_return_machine.rs` the structural return machine,
//! `structural_scalar_returns.rs` structural and trait-operator scalar
//! returns, `boundary_scalar_returns.rs` boundary scalar returns and
//! `scalar_return_expressions.rs` classifies scalar return expressions;
//! `primitive_effects.rs` and `selected_operator.rs` carry effects and
//! selected operators.

mod affine_returns;
mod boundary_scalar_returns;
mod guarded_call_returns;
pub(super) mod primitive_effects;
mod scalar_return_expressions;
mod selected_operator;
mod structural_return_machine;
mod structural_scalar_returns;

pub(crate) use boundary_scalar_returns::build_boundary_scalar_return_machine;
pub(crate) use primitive_effects::build_checked_primitive_store_scalar_return_plans;

/// The structural scalar returns Unit planning may call before the full return
/// roster exists. The call-free primitive-reference bodies are one cohort.
/// The other is every return that performs no effects and whose cleanup needs
/// no Unit plan (its builder succeeds without the cleanup catalog), such as a
/// selected operator's realization: it can be offered to Unit callers before
/// those plans exist.
/// Returns with nominal cleanup depend on the Unit plans and join only in the
/// later `build_checked_structural_scalar_return_plans` phase.
pub(crate) fn build_checked_scalar_callee_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralScalarReturnPlans {
    let mut plans = build_checked_primitive_store_scalar_return_plans(program, facts);
    let mut shapes = ShapeCollector::new(program);
    // A declined return reports through the full phase; this pre-pass only
    // offers the plans that succeed without a cleanup catalog.
    let mut diagnostics = Vec::new();
    let independent = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter(|machine| {
            !plans
                .machines
                .iter()
                .any(|plan| plan.machine == machine.symbol)
        })
        .filter_map(|machine| {
            build_structural_scalar_return_machine(
                program,
                facts,
                None,
                &mut shapes,
                machine,
                &mut diagnostics,
            )
        })
        .filter(|plan| plan.effects.is_empty())
        .collect::<Vec<_>>();
    let retained = independent
        .iter()
        .flat_map(|plan| {
            plan.attachment_type_identity.as_deref().into_iter().chain(
                plan.structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    for shape in shapes.types.into_values() {
        if !plans
            .structural_types
            .iter()
            .any(|existing| existing.identity == shape.identity)
        {
            plans.structural_types.push(shape);
        }
    }
    plans
        .structural_types
        .sort_by(|left, right| left.identity.cmp(&right.identity));
    plans.machines.extend(independent);
    plans
}
pub(crate) use scalar_return_expressions::checked_boolean_contains_short_circuit;
pub(crate) use structural_return_machine::build_structural_return_machine;
pub(crate) use structural_scalar_returns::build_structural_scalar_return_machine;

use super::{
    BTreeSet, CheckFacts, CheckedBoundaryScalarReturnPlans, CheckedStructuralAccess,
    CheckedStructuralCallReturnPlans, CheckedStructuralReturnPlans,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralScalarReturnPlans,
    CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans, CheckedUnitStructuralParameterPlan,
    Diagnostic, ExpressionNode, MachineSupplyMode, Multiplicity, StatementNode, TypeReferenceNode,
    TypedTrees,
};
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::control::build_boundary_machine;
use crate::execution::terminal_unit::control::build_static_boundary_requirements;
use crate::execution::terminal_unit::returns::affine_returns::build_claim_free_affine_structural_return_machine;
use crate::execution::terminal_unit::returns::guarded_call_returns::build_payloadless_guarded_call_return_machine;
use crate::execution::terminal_unit::returns::structural_scalar_returns::build_trait_operator_scalar_return_machine;
use selected_operator::build_selected_operator_structural_scalar_return_machine;

/// Build the exact checked carriers for `T in D -> T in D` whole-root
/// passthrough and claim-free affine identity returns. Every wider ownership
/// or control shape is omitted atomically.
pub(crate) fn build_checked_structural_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| build_structural_return_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    let claim_free_affine_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_claim_free_affine_structural_return_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .map(|local| local.type_identity.as_str()),
                )
                .chain(std::iter::once(plan.result.type_identity.as_str()))
        })
        .chain(claim_free_affine_machines.iter().flat_map(|plan| {
            plan.attachment_type_identity.as_deref().into_iter().chain([
                plan.structural_parameter.type_identity.as_str(),
                plan.result.type_identity.as_str(),
            ])
        }))
        .collect::<BTreeSet<_>>();
    let retained_domains = machines
        .iter()
        .flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .flat_map(|parameter| &parameter.qualifications)
                .chain(&plan.result.qualifications)
                .map(|domain| domain.0)
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    shapes
        .domains
        .retain(|domain| retained_domains.contains(&domain.domain.0));
    shapes.domains.sort_by_key(|domain| domain.domain.0);
    CheckedStructuralReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: shapes.domains,
        machines,
        claim_free_affine_machines,
    }
}

/// Build the guarded payloadless call returns: a zero-input call saved once
/// and returned unchanged through exhaustive case arms that may bind the
/// callee's selected guarded evidence. The callee is an ordinary Unit-effect
/// machine, not a plan of this roster.
pub(crate) fn build_checked_structural_call_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralCallReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let payloadless_guarded_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_payloadless_guarded_call_return_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = payloadless_guarded_machines
        .iter()
        .flat_map(|plan| {
            [
                plan.attachment_type_identity.as_str(),
                plan.result.type_identity.as_str(),
            ]
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralCallReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: Vec::new(),
        payloadless_guarded_machines,
    }
}

/// Compose the exact cleanup rows with source-independent structural
/// signatures and whole-parameter transfer maps for the first terminal
/// structural-control producer.
pub(crate) fn build_checked_structural_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedStructuralScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_scalar_return_machine(
                program,
                facts,
                Some(unit_effects),
                &mut shapes,
                machine,
                diagnostics,
            )
        })
        .collect::<Vec<_>>();
    let selected_operator_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_selected_operator_structural_scalar_return_machine(
                program,
                facts,
                &mut shapes,
                machine,
                &machines,
                selected_operator_applications,
            )
        })
        .collect::<Vec<_>>();
    let trait_operator_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_trait_operator_scalar_return_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            machine
                .attachment_type_identity
                .as_deref()
                .into_iter()
                .chain(
                    machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        })
        .chain(trait_operator_machines.iter().flat_map(|machine| {
            machine
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        }))
        .chain(selected_operator_machines.iter().flat_map(|machine| {
            machine
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.as_str())
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
        selected_operator_machines,
        trait_operator_machines,
    }
}

pub(crate) fn build_checked_boundary_scalar_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedBoundaryScalarReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let mut boundary_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode.is_boundary_declaration())
        .filter_map(|machine| build_boundary_machine(program, facts, &mut shapes, machine))
        .filter(|boundary| boundary.result.scalar().is_some())
        .collect::<Vec<_>>();
    boundary_machines.extend(
        build_static_boundary_requirements(program, facts, &mut shapes)
            .into_iter()
            .filter(|boundary| boundary.result.scalar().is_some()),
    );
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_boundary_scalar_return_machine(
                program,
                facts,
                &mut shapes,
                &boundary_machines,
                machine,
            )
        })
        .collect::<Vec<_>>();
    let retained = boundary_machines
        .iter()
        .flat_map(|boundary| {
            boundary
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    boundary
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
        })
        .chain(machines.iter().flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .structural_parameters
                    .iter()
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        }))
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedBoundaryScalarReturnPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines,
    }
}
