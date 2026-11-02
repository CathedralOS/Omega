//! Derive target-neutral effects from typed bodies and declared contracts.
//! Stored summaries live in `flow-effects`; fixed-point work stays here.
//! Checked service rows derive from the complete call closure. Optional authored
//! ceilings bound that row; callers retain a written contract's slack rather
//! than narrowing it to a selected body. Opaque requirements and operational
//! may-ceilings retain their independent fixed contracts.

mod call_sites;
mod callback_requirements;
mod invocations;
pub(crate) use invocations::full_parameter_invocation;
pub(crate) mod operational;
mod prepared_effects;
pub use prepared_effects::PreparedEffectPlans;
pub(crate) mod service_reach;

pub use invocations::{
    declared_machine_invocations, declared_signature_invocations,
    has_self_forwarded_boundary_parameter, infer_synchronous_invocations, invocation_target_label,
};
pub(crate) use operational::signature_operational;
pub use operational::{infer_operational_may, infer_operational_may_for_dependencies};
pub(crate) use service_reach::signature_service_reach;
pub use service_reach::{
    boundary_signature_service_reach, fixed_installation_boundary_service_reach,
    infer_service_reaches,
};

/// The published contract of one requirement an opaque realizer satisfies.
struct GoverningCeiling {
    suspends: bool,
    blocks: bool,
    service_reach_row: language_semantics::ServiceReachRowId,
    service_reach_is_installation_bound: bool,
}

/// A bodyless external realization (`satisfies Requirement via binding`)
/// inherits each governing requirement's admitted contract, not an inferred
/// native-body summary (language/effects.md, "Opaque supplies inherit").
/// Providers call such a leaf directly when calling the trait member would add
/// an invocation edge their public requirement does not publish, so the leaf's
/// own omitted clauses must not read as an empty reach or a non-blocking call.
fn opaque_governing_ceilings(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<GoverningCeiling> {
    if machine.body_is_present
        || !matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        )
    {
        return Vec::new();
    }
    program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            match typed_trees::machine::resolve_satisfied_declaration(
                program.typed_program(),
                machine,
                conformance,
            )? {
                typed_trees::machine::SatisfiedDeclaration::Trait { requirement, .. } => {
                    Some(GoverningCeiling {
                        suspends: requirement.suspends,
                        blocks: requirement.blocks,
                        service_reach_row: requirement.service_reach_row,
                        service_reach_is_installation_bound: requirement
                            .service_reach_is_installation_bound,
                    })
                }
                typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(requirement) => {
                    Some(GoverningCeiling {
                        suspends: requirement.suspends,
                        blocks: requirement.blocks,
                        service_reach_row: requirement.service_reach_row,
                        service_reach_is_installation_bound: requirement
                            .service_reach_is_installation_bound,
                    })
                }
                typed_trees::machine::SatisfiedDeclaration::Operator(_) => None,
            }
        })
        .collect()
}

/// Family interfaces derive effects from every member without adding executable
/// call edges. Build families still have an ordinary canonical body.
fn family_interface_members<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> &'program [typed_trees::machine::TargetMachineFamilyMember] {
    if machine.body_is_present {
        return &[];
    }
    program
        .target_machine_families
        .iter()
        .find(|family| family.canonical == machine.symbol)
        .map_or(&[], |family| family.members.as_slice())
}
