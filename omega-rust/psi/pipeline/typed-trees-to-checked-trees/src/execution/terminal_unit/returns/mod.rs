//! Structural Unit and scalar return analysis.
//!
//! This file builds the four return plan sets. `affine_and_case_returns.rs`
//! builds claim-free affine and payloadless case return machines,
//! `guarded_call_returns.rs` payloadless guarded call returns,
//! `structural_return_machine.rs` the structural return machine,
//! `structural_scalar_returns.rs` structural and trait-operator scalar
//! returns, `boundary_scalar_returns.rs` boundary scalar returns and
//! `scalar_return_expressions.rs` classifies scalar return expressions;
//! `primitive_effects.rs` and `selected_operator.rs` carry effects and
//! selected operators.

mod affine_and_case_returns;
mod boundary_scalar_returns;
mod guarded_call_returns;
pub(super) mod primitive_effects;
mod scalar_return_expressions;
mod selected_operator;
mod structural_return_machine;
mod structural_scalar_returns;

pub(crate) use boundary_scalar_returns::build_boundary_scalar_return_machine;
pub(crate) use primitive_effects::build_checked_primitive_store_scalar_return_plans;
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
use crate::execution::terminal_unit::ShapeCollector;

use crate::execution::terminal_unit::control::build_boundary_machine;
use crate::execution::terminal_unit::control::build_static_boundary_requirements;
use crate::execution::terminal_unit::returns::affine_and_case_returns::{
    build_claim_free_affine_structural_return_machine, build_payloadless_case_return_machine,
};
use crate::execution::terminal_unit::returns::guarded_call_returns::build_payloadless_guarded_call_return_machine;
use crate::execution::terminal_unit::returns::structural_scalar_returns::build_trait_operator_scalar_return_machine;
use selected_operator::build_selected_operator_structural_scalar_return_machine;

/// Build the exact checked carriers for `T in D -> T in D` whole-root
/// passthrough and the separate zero-input payload-less sum-case constructor.
/// Every wider ownership or control shape is omitted atomically.
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
    let payloadless_case_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_payloadless_case_return_machine(program, facts, &mut shapes, machine)
        })
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
        .chain(payloadless_case_machines.iter().flat_map(|plan| {
            [
                plan.attachment_type_identity.as_str(),
                plan.result.type_identity.as_str(),
            ]
        }))
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
        payloadless_case_machines,
    }
}

/// Build the bounded internal structural-result call slice. The caller has
/// one linear whole-root input, performs one final direct call to an already
/// admitted structural-return machine, and returns that result immediately.
/// Bodyless calls, projections, staged locals, and wider result maps remain
/// deliberately outside this carrier.
pub(crate) fn build_checked_structural_call_return_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    structural_returns: &CheckedStructuralReturnPlans,
) -> CheckedStructuralCallReturnPlans {
    let mut shapes = ShapeCollector::new(program);
    let payloadless_guarded_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_payloadless_guarded_call_return_machine(
                program,
                facts,
                structural_returns,
                &mut shapes,
                machine,
            )
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
