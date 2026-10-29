//! Identities of satisfied requirements, external bindings, hosted catalog
//! leaves and provider boundary arguments.

use crate::provider_planning::provenance_replay::SelectedTargetMachineOrigin;
use crate::provider_planning::{ProviderBinding, TypedTrees};
use target::HostedIntrinsicBundle;

pub(crate) fn exact_satisfied_requirement_identity(
    typed: &TypedTrees,
    trait_symbol: symbols::SymbolHandle,
    requirement_symbol: symbols::SymbolHandle,
) -> String {
    if trait_symbol == requirement_symbol
        && let Some(requirement) = typed.machines().iter().find(|requirement| {
            requirement.symbol == requirement_symbol
                && requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
    {
        return typed
            .machine_callable_identity_text(requirement)
            .unwrap_or_default();
    }
    let Some(definition) = typed.trait_definition_by_symbol(trait_symbol) else {
        return String::new();
    };
    typed
        .trait_machine_signature_by_symbol(definition, requirement_symbol)
        .map(|signature| typed.trait_requirement_callable_identity_text(definition, signature))
        .unwrap_or_default()
}

/// Payload-free selected-target compiler leaf: a bodyless `boundary machine`
/// satisfying a boundary-trait requirement, claimed by exactly one selected
/// target-machine origin. Hosted targets admit only the catalog above —
/// their realization machinery is name-keyed and every listed leaf is gated
/// per target. Every other target package owns its compiler leaves outright:
/// a `boundary machine` satisfies on the selected non-hosted target binds by
/// its exact normalized realization identity. Candidate derivation only:
/// realization authority is re-derived downstream from that same identity
/// and custody.
pub(crate) fn inferred_selected_target_compiler_leaf(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
    conformance: &checked_trees::typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
    let hosted = selected_target
        .is_none_or(|target| HostedIntrinsicBundle::from_target_name(target).is_some());
    if hosted
        || typed.hosted_intrinsic_declaration(machine.symbol).is_some()
        || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
        || machine.body_is_present
        || machine.attached_data.is_none()
        || !machine.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(machine).is_empty()
    {
        return None;
    }
    let origin = sole_machine_origin(target_machine_origins, machine.symbol, |origin| {
        selected_target == Some(origin.target.as_str())
    })?;
    let checked_trees::typed_trees::machine::SatisfiedDeclaration::Trait {
        definition,
        requirement,
    } = checked_trees::typed_trees::machine::resolve_satisfied_declaration(
        typed,
        machine,
        conformance,
    )?
    else {
        return None;
    };
    if !definition.is_boundary
        || definition.symbol != conformance.symbol
        || requirement.symbol != conformance.requirement_symbol
    {
        return None;
    }
    let [_entry] = typed.machine_states(machine) else {
        return None;
    };
    let machine = typed.machine_callable_identity_text(machine)?;
    (!machine.is_empty()).then_some((
        ProviderBinding::CompilerIntrinsic { machine },
        (*origin).clone(),
    ))
}

/// Exact immutable numeric catalogue membership establishes primitive meaning;
/// the receiving target independently checks its ISA availability. Other hosted
/// catalogues retain their exact target-origin requirement. Neither route grants
/// package authority or substitutes for receiving realization admission.
pub(crate) fn inferred_compiler_leaf_binding(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
    conformance: &checked_trees::typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(ProviderBinding, Option<SelectedTargetMachineOrigin>)> {
    let numeric =
        typed.sealed_numeric_realization_descriptor(machine.symbol, conformance.requirement_symbol);
    if let Some(catalogue) = numeric {
        let target = language_semantics::declaration_selection::ProductTarget::from_canonical_name(
            selected_target?,
        )?;
        if !catalogue.instruction_set.supports(target) {
            return None;
        }
        return Some((
            ProviderBinding::CompilerIntrinsic {
                machine: typed.machine_callable_identity_text(machine)?,
            },
            None,
        ));
    }
    if machine.supply_mode == language_semantics::MachineSupplyMode::CompilerPrimitive {
        return None;
    }
    if let Some(catalogue) = typed.hosted_intrinsic_declaration(machine.symbol) {
        if catalogue.service.symbol != conformance.symbol
            || catalogue.requirement.symbol != conformance.requirement_symbol
            || selected_target
                .is_some_and(|target| HostedIntrinsicBundle::from_target_name(target).is_none())
        {
            return None;
        }
        return Some((
            ProviderBinding::CompilerIntrinsic {
                machine: typed.machine_callable_identity_text(machine)?,
            },
            None,
        ));
    }
    inferred_selected_target_compiler_leaf(
        typed,
        machine,
        conformance,
        selected_target,
        target_machine_origins,
    )
    .map(|(binding, origin)| (binding, Some(origin)))
}

pub(crate) fn realization_machine_identity(
    typed: &TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
) -> String {
    typed
        .machine_callable_identity_text(machine)
        .unwrap_or_default()
}

/// The exact requirement a provider's retained satisfies edge installs on
/// its plan row. An edge naming a generic token-machine template is
/// satisfied by the provider instance's closed applications: the clause
/// keeps the template's symbols while the retained specialization receipt
/// and validated operator application select the closed requirement.
/// Checking resolved that join once into `CheckedProviderClosedRequirement`
/// facts; realization reads the recorded decision rather than replaying
/// source-specialization evidence, and a template never aliases its closed
/// requirement.
pub(crate) fn satisfied_row_requirement<'typed>(
    typed: &'typed TypedTrees,
    check_facts: &'typed checked_trees::CheckFacts,
    provider: &'typed checked_trees::typed_trees::machine::Machine,
    requirement: &'typed checked_trees::typed_trees::machine::Machine,
) -> Option<&'typed checked_trees::typed_trees::machine::Machine> {
    if typed.machine_type_parameters(requirement).is_empty() {
        return Some(requirement);
    }
    let mut applications = check_facts
        .provider_closed_requirements
        .iter()
        .filter(|row| {
            row.provider == provider.symbol
                && row.selecting_declaration == provider.symbol
                && row.template == requirement.symbol
        });
    let application = applications.next()?;
    if applications.next().is_some() {
        return None;
    }
    typed.machine(application.closed_requirement)
}

/// Whether two boundary names denote the same slot: equal, or one is the
/// unqualified leaf of the other. External-root slot resolution matches
/// retained selected plans with this same rule.
pub use symbols::same_semantic_name;

/// The stable name shared by derivation, reports, selection, and backend row
/// extraction. External leaves may use the anonymous form; a real provider
/// type is deliberately visible in artifact identity.
pub fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
    match (target.is_empty(), provider_type.is_empty()) {
        (true, true) => format!("satisfies::{trait_name}"),
        (false, true) => format!("{target}::satisfies::{trait_name}"),
        (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
        (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
    }
}

/// The single target-machine origin claiming `machine` under `admits`, when
/// exactly one does.
fn sole_machine_origin<'origins>(
    target_machine_origins: &'origins [SelectedTargetMachineOrigin],
    machine: symbols::SymbolHandle,
    admits: impl Fn(&SelectedTargetMachineOrigin) -> bool,
) -> Option<&'origins SelectedTargetMachineOrigin> {
    let origins = target_machine_origins
        .iter()
        .filter(|origin| origin.machine == machine && admits(origin))
        .collect::<Vec<_>>();
    let [origin] = origins.as_slice() else {
        return None;
    };
    Some(origin)
}
