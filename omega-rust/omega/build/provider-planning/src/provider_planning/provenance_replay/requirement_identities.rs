//! Identities of satisfied requirements, external bindings, hosted catalog
//! leaves and provider boundary arguments.

use crate::provider_planning::provenance_replay::SelectedTargetMachineOrigin;
use crate::provider_planning::{ProviderBinding, TypedTrees};

pub fn satisfied_requirement_identity(
    typed: &TypedTrees,
    machine_name: &str,
    trait_name: &str,
    requirement_name: &str,
) -> String {
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
    else {
        return String::new();
    };
    if let Some(identity) = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            conformance.name.as_str() == trait_name
                && conformance.requirement.as_ref().map(|name| name.as_str())
                    == Some(requirement_name)
        })
        .find_map(|conformance| {
            let typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(requirement) =
                typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)?
            else {
                return None;
            };
            typed
                .normalized_machine_overload_identity(requirement)
                .map(|identity| identity.identity())
        })
    {
        return identity;
    }
    let Some(definition) = typed.traits().iter().find(|definition| {
        definition.name.as_str() == trait_name
            || definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|leaf| leaf == trait_name)
    }) else {
        return String::new();
    };
    let named = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement_name)
        .collect::<Vec<_>>();
    let selected = match named.as_slice() {
        [single] => Some(*single),
        many => {
            let implementation_dispatch = typed
                .machine_states(machine)
                .first()
                .map(|entry| typed.normalized_result_dispatch_set(entry.return_type));
            let mut matching = many.iter().copied().filter(|signature| {
                implementation_dispatch.as_ref().is_some_and(|dispatch| {
                    typed.normalized_result_dispatch_set(signature.return_type) == *dispatch
                })
            });
            let selected = matching.next();
            selected.filter(|_| matching.next().is_none())
        }
    };
    selected
        .map(|signature| {
            typed
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
        })
        .unwrap_or_default()
}

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
            .normalized_machine_overload_identity(requirement)
            .map(|identity| identity.identity())
            .unwrap_or_default();
    }
    let Some(definition) = typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
    else {
        return String::new();
    };
    typed
        .trait_machine_signatures(definition)
        .iter()
        .find(|signature| signature.symbol == requirement_symbol)
        .map(|signature| {
            typed
                .normalized_trait_requirement_overload_identity(definition, signature)
                .identity()
        })
        .unwrap_or_default()
}

fn exact_external_binding_identity<'typed>(
    typed: &'typed TypedTrees,
    machine: &typed_trees::machine::Machine,
    trait_name: &str,
    requirement_name: &str,
) -> Option<&'typed language_semantics::ExternalBindingIdentity> {
    let mut matching = typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| same_semantic_name(conformance.name.as_str(), trait_name))
        .filter(|conformance| {
            conformance.requirement.as_ref().map(|name| name.as_str()) == Some(requirement_name)
        })
        .filter_map(|conformance| conformance.external_binding);
    let binding = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    typed.external_bindings.identity(binding)
}

pub(crate) fn exact_installed_external_binding_identity<'typed>(
    typed: &'typed TypedTrees,
    machine: &typed_trees::machine::Machine,
    conformance_binding: language_semantics::ExternalBindingId,
    trait_name: &str,
    requirement_name: &str,
) -> Option<&'typed language_semantics::ExternalBindingIdentity> {
    let language_semantics::MachineSupplyMode::ExternalRealization {
        binding: Some(supply_binding),
        mechanism: Some(supply_mechanism),
    } = machine.supply_mode
    else {
        return None;
    };
    if supply_binding != conformance_binding {
        return None;
    }
    let binding = exact_external_binding_identity(typed, machine, trait_name, requirement_name)?;
    (binding.mechanism() == supply_mechanism).then_some(binding)
}

/// Exact hosted catalog leaf identity: (provider nominal, boundary trait,
/// requirement) for one source-inferred compiler intrinsic. Every listed
/// leaf is realized on each hosted target (`linux_x86_64`, `linux_arm64`,
/// `macos_arm64`); a leaf that cannot be realized on one of those targets
/// needs its own target gate here rather than a widened catalog row.
fn inferred_hosted_catalog_leaf(
    machine_name: &str,
) -> Option<(&'static str, &'static str, &'static str)> {
    match machine_name {
        "ConsoleNativeProvider::exit_process" => {
            Some(("ConsoleNativeProvider", "Console", "exit_process"))
        }
        "ConsoleNativeProvider::write_byte" => {
            Some(("ConsoleNativeProvider", "Console", "write_byte"))
        }
        "ProcessExitNativeProvider::exit_process" => {
            Some(("ProcessExitNativeProvider", "ProcessExit", "exit_process"))
        }
        _ => None,
    }
}

/// Payload-free source-inferred hosted catalog leaves. This is only
/// candidate derivation: selected-dispatch independently rejoins package
/// custody and the canonical target before granting a closed execution.
pub(crate) fn inferred_hosted_console_compiler_intrinsic(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
    let (provider_name, trait_name, requirement_name) =
        inferred_hosted_catalog_leaf(machine.name.as_str())?;
    let supports_target =
        |target: &str| matches!(target, "linux_x86_64" | "linux_arm64" | "macos_arm64");
    if selected_target.is_some_and(|target| !supports_target(target))
        || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
        || machine.body_is_present
        || machine.attached_data.as_ref().map(|name| name.as_str()) != Some(provider_name)
        || !machine.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(machine).is_empty()
        || conformance.name.as_str() != trait_name
        || conformance.requirement.as_ref().map(|name| name.as_str()) != Some(requirement_name)
        || conformance.external_binding.is_some()
        || conformance.via_expression.is_valid()
        || conformance.external_binding_source_span.is_some()
    {
        return None;
    }
    let origins = target_machine_origins
        .iter()
        .filter(|origin| {
            origin.machine == machine.symbol
                && match selected_target {
                    Some(target) => origin.target == target,
                    None => supports_target(&origin.target),
                }
        })
        .collect::<Vec<_>>();
    let [origin] = origins.as_slice() else {
        return None;
    };
    let typed_trees::machine::SatisfiedDeclaration::Trait {
        definition,
        requirement,
    } = typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)?
    else {
        return None;
    };
    if !definition.is_boundary
        || definition.symbol != conformance.symbol
        || definition.name.as_str() != trait_name
        || !definition.lifetime_parameters.is_empty()
        || !typed.trait_type_parameters(definition).is_empty()
        || requirement.symbol != conformance.requirement_symbol
        || requirement.name.as_str() != requirement_name
        || machine
            .name
            .as_str()
            .strip_prefix(provider_name)
            .and_then(|suffix| suffix.strip_prefix("::"))
            != Some(requirement.name.as_str())
        || !exact_catalog_i32_to_unit_signature(
            typed,
            typed.state_signature_parameters(requirement),
            requirement.return_type,
        )
    {
        return None;
    }
    let [entry] = typed.machine_states(machine) else {
        return None;
    };
    if !exact_catalog_i32_to_unit_signature(typed, typed.state_parameters(entry), entry.return_type)
    {
        return None;
    }
    let machine = typed
        .normalized_machine_overload_identity(machine)?
        .identity();
    (!machine.is_empty()).then_some((
        ProviderBinding::CompilerIntrinsic { machine },
        (*origin).clone(),
    ))
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
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
    let hosted = selected_target
        .is_none_or(|target| matches!(target, "linux_x86_64" | "linux_arm64" | "macos_arm64"));
    if hosted
        || inferred_hosted_catalog_leaf(machine.name.as_str()).is_some()
        || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
        || machine.body_is_present
        || machine.attached_data.is_none()
        || !machine.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(machine).is_empty()
        || conformance.external_binding.is_some()
        || conformance.via_expression.is_valid()
        || conformance.external_binding_source_span.is_some()
    {
        return None;
    }
    let origins = target_machine_origins
        .iter()
        .filter(|origin| {
            origin.machine == machine.symbol && selected_target == Some(origin.target.as_str())
        })
        .collect::<Vec<_>>();
    let [origin] = origins.as_slice() else {
        return None;
    };
    let typed_trees::machine::SatisfiedDeclaration::Trait {
        definition,
        requirement,
    } = typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)?
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
    let machine = typed
        .normalized_machine_overload_identity(machine)?
        .identity();
    (!machine.is_empty()).then_some((
        ProviderBinding::CompilerIntrinsic { machine },
        (*origin).clone(),
    ))
}

/// Source-inferred compiler-leaf authority: the hosted catalog's stricter
/// admission first, then any other target-owned boundary leaf the selected
/// target-machine origins claim.
pub(crate) fn inferred_compiler_leaf_binding(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    selected_target: Option<&str>,
    target_machine_origins: &[SelectedTargetMachineOrigin],
) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
    inferred_hosted_console_compiler_intrinsic(
        typed,
        machine,
        conformance,
        selected_target,
        target_machine_origins,
    )
    .or_else(|| {
        inferred_selected_target_compiler_leaf(
            typed,
            machine,
            conformance,
            selected_target,
            target_machine_origins,
        )
    })
}

fn exact_catalog_i32_to_unit_signature(
    typed: &TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    return_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    !parameter.is_self
        && !parameter.is_const
        && !parameter.is_mutable
        && typed.primitive_type_reference(parameter.type_reference)
            == Some(typed_trees::types::PrimitiveType::I32)
        && matches!(
            typed.type_reference_table.type_reference(return_type),
            typed_trees::types::TypeReferenceNode::Unit
        )
}

/// Map one retained typed external-binding identity to its provider-plan
/// binding. Every closed identity has exactly one, so this mapping is total:
/// the string-backed import spelling that used to be rejected here no longer
/// exists to construct.
pub(crate) fn external_provider_binding(
    binding: &language_semantics::ExternalBindingIdentity,
    provider_type: &str,
    intrinsic_machine_identity: &str,
) -> ProviderBinding {
    use language_semantics::ExternalBindingIdentity;

    match binding {
        ExternalBindingIdentity::Syscall { number } => ProviderBinding::Syscall { number: *number },
        ExternalBindingIdentity::CompilerIntrinsic => ProviderBinding::CompilerIntrinsic {
            machine: intrinsic_machine_identity.to_owned(),
        },
        ExternalBindingIdentity::VtableSlot { index } => {
            ProviderBinding::VtableSlot { index: *index }
        }
        ExternalBindingIdentity::VtableField { field } => ProviderBinding::VtableField {
            table: provider_type.to_owned(),
            field: field.clone(),
        },
        ExternalBindingIdentity::TableFunction { field } => ProviderBinding::TableFunction {
            table: provider_type.to_owned(),
            field: field.clone(),
        },
    }
}

pub(crate) fn realization_machine_identity(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> String {
    typed
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
        .unwrap_or_default()
}

pub(crate) fn provider_boundary_arguments(
    typed: &TypedTrees,
    boundary: &typed_trees::trait_definition::TraitDefinition,
    provider_symbol: Option<symbols::SymbolHandle>,
) -> Vec<typed_trees::types::TypeReferenceHandle> {
    typed
        .conformances()
        .iter()
        .find(|conformance| {
            Some(conformance.carrier_symbol) == provider_symbol
                && conformance.trait_symbol == boundary.symbol
        })
        .map(|conformance| {
            typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .to_vec()
        })
        .unwrap_or_default()
}

pub(crate) fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

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
