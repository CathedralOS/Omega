//! Derive target-neutral effects from typed bodies and declared contracts.
//! Stored summaries live in `flow-effects`; fixed-point work stays here.
//! Checked service rows union authored contributions with all callees, including
//! through exported wrappers. Direct boundary uses still need declarations;
//! opaque requirements and operational may-ceilings retain their fixed contracts.

mod invocations;
pub(crate) use invocations::full_parameter_invocation;
pub(crate) mod operational;
pub(crate) mod service_reach;

pub use invocations::{
    declared_machine_invocations, declared_signature_invocations,
    has_self_forwarded_boundary_parameter, infer_synchronous_invocations, invocation_target_label,
};
pub use operational::{infer_operational_may, infer_operational_may_with_cleanup_dependencies};
pub(crate) use service_reach::required_boundary_services;
pub use service_reach::{fixed_installation_boundary_service_reach, infer_service_reaches};

/// The service a call-target roster entry names: a parameter's exact bound
/// service requirement, falling back to its type's head symbol; a service
/// target names itself.
pub(super) fn invocation_service_symbol(
    program: &typed_trees::TypedTrees,
    parameters: &[&typed_trees::signature::StateParameter],
    target: &flow_effects::InvocationTarget,
) -> Option<symbols::SymbolHandle> {
    match target {
        flow_effects::InvocationTarget::Parameter(index) => parameters
            .get(*index as usize)
            .map(|parameter| {
                typed_trees::service::exact_bound_service_requirement(
                    program,
                    parameter.type_reference,
                )
                .unwrap_or_else(|| {
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                        .type_symbol(&program.type_reference_table)
                })
            }),
        flow_effects::InvocationTarget::Service(symbol) => Some(*symbol),
    }
}
