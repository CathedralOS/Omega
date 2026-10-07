//!
//! `bounds.rs` checks conformance bounds, `machine_conformance.rs` validates
//! each machine's conformances, `trait_applications.rs` their application
//! obligations, `signature_matching.rs` the signature match and
//! `external_shapes.rs` the external native shapes; `generic_operator.rs`
//! stays as it was.

/// The provider's effectively-reached services outside `allowed_services`,
/// named for diagnostics; `None` (with the missing-summary diagnostic
/// already pushed) when the provider has no inferred summary.
fn disallowed_reached_service_names<'program>(
    program: &'program typed_trees::TypedTrees,
    service_reaches: &'program flow_effects::ServiceReachInferencePlan,
    machine: &'program typed_trees::machine::Machine,
    allowed_services: &[language_semantics::ServiceReachId],
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Option<Vec<&'program str>> {
    let Some(summary) = service_reaches.for_machine(machine.symbol) else {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "machine `{}` has no inferred service-reach summary for requirement checking",
            machine.name,
        )));
        return None;
    };
    Some(
        service_reaches
            .services(summary.effective)
            .iter()
            .filter(|service| !allowed_services.contains(*service))
            .map(|service| {
                program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("<unknown canonical service>")
            })
            .collect(),
    )
}

mod bounds;
mod external_shapes;
mod generic_operator;
mod machine_conformance;
mod signature_matching;
mod trait_applications;

pub(crate) use bounds::{
    GenericBoundRequirement, named_conformance_argument_matches,
    validate_generic_conformance_bounds, validate_trait_conformance_bounds,
};
pub(crate) use external_shapes::{
    validate_external_leaf_native_shapes, validate_external_via_expression,
};
pub use generic_operator::generic_bound_operator_requirement;
pub(crate) use machine_conformance::validate_machine_trait_conformances;
pub use machine_conformance::{
    compose_forwarded_trait_arguments, resolve_closed_requirement_application,
    revalidate_top_level_requirement_realization,
};
pub(crate) use signature_matching::validate_machine_state_satisfies_trait_signature_with_arguments;
pub(crate) use trait_applications::validate_trait_application_obligations;
