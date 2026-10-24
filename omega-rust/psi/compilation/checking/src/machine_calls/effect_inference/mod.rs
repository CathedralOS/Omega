//! Derive target-neutral effects from typed bodies and declared contracts.
//! Stored summaries live in `flow-effects`; fixed-point work stays here.
//! Checked service rows union authored contributions with all callees, including
//! through exported wrappers. Direct boundary uses still need declarations;
//! opaque requirements and operational may-ceilings retain their fixed contracts.

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
pub use operational::{infer_operational_may, infer_operational_may_for_cleanup_dependencies};
pub(crate) use service_reach::required_boundary_services;
pub(crate) use service_reach::signature_service_reach;
pub use service_reach::{
    boundary_signature_service_reach, fixed_installation_boundary_service_reach,
    infer_service_reaches,
};

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
