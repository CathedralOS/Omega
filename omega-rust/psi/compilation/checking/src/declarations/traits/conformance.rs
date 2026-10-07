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

/// Two declaration-site type parameters pair on the same declared kind,
/// including static `Machine` and `Proposition` parameters.
pub(super) fn type_parameter_kinds_identical(
    required: &typed_trees::data::TypeParameterKind,
    actual: &typed_trees::data::TypeParameterKind,
) -> bool {
    matches!(
        (required, actual),
        (
            typed_trees::data::TypeParameterKind::Type,
            typed_trees::data::TypeParameterKind::Type
        ) | (
            typed_trees::data::TypeParameterKind::Const { .. },
            typed_trees::data::TypeParameterKind::Const { .. }
        ) | (
            typed_trees::data::TypeParameterKind::Value { .. },
            typed_trees::data::TypeParameterKind::Value { .. }
        ) | (
            typed_trees::data::TypeParameterKind::Machine { .. },
            typed_trees::data::TypeParameterKind::Machine { .. }
        ) | (
            typed_trees::data::TypeParameterKind::Proposition { .. },
            typed_trees::data::TypeParameterKind::Proposition { .. }
        )
    )
}

/// The kind pairs a callable's generic parameters accept: `Proposition`
/// parameters do not take part in machine-callable matching.
pub(super) fn type_parameter_kinds_callable(
    required: &typed_trees::data::TypeParameterKind,
    actual: &typed_trees::data::TypeParameterKind,
) -> bool {
    matches!(
        (required, actual),
        (
            typed_trees::data::TypeParameterKind::Type,
            typed_trees::data::TypeParameterKind::Type
        ) | (
            typed_trees::data::TypeParameterKind::Const { .. },
            typed_trees::data::TypeParameterKind::Const { .. }
        ) | (
            typed_trees::data::TypeParameterKind::Value { .. },
            typed_trees::data::TypeParameterKind::Value { .. }
        ) | (
            typed_trees::data::TypeParameterKind::Machine { .. },
            typed_trees::data::TypeParameterKind::Machine { .. }
        )
    )
}

/// The kind pairs that form a trait type binding: `Type`, `Const`, and
/// `Value` parameters only.
pub(super) fn type_parameter_kinds_bindable(
    required: &typed_trees::data::TypeParameterKind,
    actual: &typed_trees::data::TypeParameterKind,
) -> bool {
    matches!(
        (required, actual),
        (
            typed_trees::data::TypeParameterKind::Type,
            typed_trees::data::TypeParameterKind::Type
        ) | (
            typed_trees::data::TypeParameterKind::Const { .. },
            typed_trees::data::TypeParameterKind::Const { .. }
        ) | (
            typed_trees::data::TypeParameterKind::Value { .. },
            typed_trees::data::TypeParameterKind::Value { .. }
        )
    )
}

/// The (required, actual) parameter pairs that may form trait type
/// bindings: zipped positionally, then filtered to bindable kinds.
pub(super) fn type_parameter_binding_pairs<'p>(
    required: &'p [typed_trees::data::TypeParameter],
    actual: &'p [typed_trees::data::TypeParameter],
) -> impl Iterator<
    Item = (
        &'p typed_trees::data::TypeParameter,
        &'p typed_trees::data::TypeParameter,
    ),
> {
    required
        .iter()
        .zip(actual)
        .filter(|&(required, actual)| type_parameter_kinds_bindable(&required.kind, &actual.kind))
}

/// A breach of a requirement's operational ceiling, named for the caller's
/// diagnostic phrasing.
pub(super) enum OperationalCeilingBreach<'a> {
    /// The provider effectively reaches a service the requirement does not
    /// allow.
    ReachedService(&'a str),
    /// The provider declares `suspends;` the requirement does not.
    Suspends,
    /// The provider declares `blocks;` the requirement does not.
    Blocks,
}

/// The three operational-ceiling checks a machine must stay under to
/// satisfy a requirement: its effectively reached services, `suspends;`,
/// and `blocks;`. `describe` phrases each breach for the caller's own
/// diagnostics.
pub(super) fn require_operational_ceiling(
    program: &typed_trees::TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    machine: &typed_trees::machine::Machine,
    requirement_service_reach_row: language_semantics::ServiceReachRowId,
    requirement_suspends: bool,
    requirement_blocks: bool,
    describe: impl Fn(OperationalCeilingBreach<'_>) -> String,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let allowed_services = program
        .service_reach_rows
        .services(requirement_service_reach_row);
    // Checked providers contribute their whole call closure. Comparing only
    // authored clauses would let an annotation-free helper evade this bound.
    let Some(disallowed) = disallowed_reached_service_names(
        program,
        service_reaches,
        machine,
        allowed_services,
        diagnostics,
    ) else {
        return;
    };
    for service_name in disallowed {
        diagnostics.push(diagnostics::Diagnostic::error(describe(
            OperationalCeilingBreach::ReachedService(service_name),
        )));
    }
    if machine.suspends && !requirement_suspends {
        diagnostics.push(diagnostics::Diagnostic::error(describe(
            OperationalCeilingBreach::Suspends,
        )));
    }
    if machine.blocks && !requirement_blocks {
        diagnostics.push(diagnostics::Diagnostic::error(describe(
            OperationalCeilingBreach::Blocks,
        )));
    }
}
