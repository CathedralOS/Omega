//! Boundary fields, exact traits, receiver shapes and adapter calls.

use crate::boundary_dispatch::adapter_rows::{AdapterRow, GenericBoundaryRequirement};
use crate::boundary_dispatch::signature_families::call_argument_const_identity;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundaryField {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) trait_symbol: symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryFieldDeclaration {
    pub(crate) owner: symbols::SymbolHandle,
    pub(crate) owner_name: String,
    pub(crate) field_name: String,
    pub(crate) field: BoundaryField,
}

pub(crate) fn exact_conformance_requirement_identity(
    typed: &TypedTrees,
    adapter: &typed_trees::machine::Machine,
    owner: &typed_trees::trait_definition::TraitDefinition,
    requirement: &str,
) -> Option<String> {
    let signatures = typed
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement)
        .collect::<Vec<_>>();
    let signature = match signatures.as_slice() {
        [signature] => *signature,
        many => {
            let implementation_dispatch = typed
                .machine_states(adapter)
                .first()
                .map(|entry| typed.normalized_result_dispatch_set(entry.return_type));
            let matches = many
                .iter()
                .copied()
                .filter(|signature| {
                    implementation_dispatch.as_ref().is_some_and(|dispatch| {
                        typed.normalized_result_dispatch_set(signature.return_type) == *dispatch
                    })
                })
                .collect::<Vec<_>>();
            let [signature] = matches.as_slice() else {
                return None;
            };
            *signature
        }
    };
    Some(
        typed
            .normalized_trait_requirement_overload_identity(owner, signature)
            .identity(),
    )
}

pub(crate) fn exact_boundary_trait<'typed>(
    typed: &'typed TypedTrees,
    name: &str,
    package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    role: &str,
) -> Result<&'typed typed_trees::trait_definition::TraitDefinition, Diagnostic> {
    if name.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} has no canonical identity",
        )));
    }
    // Package and checked purpose both matter: the build copy of a product
    // dependency has the same nominal name but supplies no product dispatch.
    let definitions = typed
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && provider_planning::service_schema::is_product_declaration(
                    typed,
                    definition.symbol,
                )
                && typed.trait_declaration_path(definition) == name
                && typed.symbols.symbol_package_identity(definition.symbol) == package_identity
        })
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} `{name}` resolves to {} exact boundary traits",
            definitions.len(),
        )));
    };
    if !definition.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} `{name}` has no exact typed symbol",
        )));
    }
    Ok(*definition)
}

pub(crate) fn named_type_symbol(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<symbols::SymbolHandle> {
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    symbol.is_valid().then_some(*symbol)
}

/// The exact receiver shape a selected adapter row takes: `None` forward
/// when the adapter's declared parameters equal the requirement's, `Some`
/// forward when the adapter prepends one leading parameter that carries the
/// requirement owner exactly. `receiver_access` is the requirement's
/// declared `self` receiver form: `None` splices the owned place, so the
/// leading parameter must be `Owner`; `Some(access)` splices a borrow, so
/// the leading parameter must be `&Owner`/`&mut Owner` with the same access.
pub(crate) fn exact_adapter_receiver_shape(
    typed: &TypedTrees,
    actual_parameters: &[&typed_trees::signature::StateParameter],
    requirement_parameter_count: usize,
    requirement_owner: symbols::SymbolHandle,
    receiver_access: Option<language_core::ReferenceAccess>,
) -> Option<bool> {
    match actual_parameters.len() {
        count if count == requirement_parameter_count => Some(false),
        count
            if requirement_parameter_count.checked_add(1) == Some(count)
                && actual_parameters.first().is_some_and(|parameter| {
                    receiver_parameter_matches_owner(
                        typed,
                        parameter.type_reference,
                        requirement_owner,
                        receiver_access,
                    )
                }) =>
        {
            Some(true)
        }
        _ => None,
    }
}

fn receiver_parameter_matches_owner(
    typed: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    requirement_owner: symbols::SymbolHandle,
    receiver_access: Option<language_core::ReferenceAccess>,
) -> bool {
    match receiver_access {
        None => named_type_symbol(typed, type_reference) == Some(requirement_owner),
        Some(access) => matches!(
            typed.type_reference_table.type_reference(type_reference),
            typed_trees::types::TypeReferenceNode::Reference {
                referee,
                access: parameter_access,
                ..
            } if *parameter_access == access
                && named_type_symbol(typed, *referee) == Some(requirement_owner)
        ),
    }
}

/// Whether the field's requirement family -- the boundary trait or the
/// top-level requirement machine -- owns the call's exact target state. A
/// `self` receiver place registers once per requirement on its owner type, so
/// one receiver symbol can map to several fields and only the family owning
/// the target selects the adapter.
fn field_owns_target(
    typed: &TypedTrees,
    requirement_family: symbols::SymbolHandle,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    typed.machines().iter().any(|machine| {
        machine.symbol == requirement_family
            && typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == target_symbol)
    }) || typed.traits().iter().any(|definition| {
        definition.symbol == requirement_family
            && typed
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == target_symbol)
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_adapter_call<'adapter>(
    typed: &TypedTrees,
    adapters: &'adapter [AdapterRow],
    generic_requirements: &[GenericBoundaryRequirement],
    fields: &[BoundaryField],
    receiver_symbol: symbols::SymbolHandle,
    target_symbol: symbols::SymbolHandle,
    target_name: &str,
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
) -> Result<Option<&'adapter AdapterRow>, Diagnostic> {
    let candidates = fields
        .iter()
        .filter(|field| field.symbol == receiver_symbol)
        .collect::<Vec<_>>();
    let field = if candidates.len() > 1 {
        candidates
            .iter()
            .copied()
            .find(|field| field_owns_target(typed, field.trait_symbol, target_symbol))
            .or(candidates.first().copied())
    } else {
        candidates.first().copied()
    };
    let Some(field) = field else {
        return Ok(None);
    };
    let matches = adapters
        .iter()
        .filter(|adapter| {
            adapter.receiver_trait == field.trait_symbol
                && adapter.requirement_symbol == target_symbol
        })
        .collect::<Vec<_>>();
    // A family requirement's rows are tuple-keyed: the call's static machine
    // arguments must canonically select exactly one declared tuple. An
    // ineligible generic requirement supplies no rows and rejects below.
    let matches = if matches
        .iter()
        .any(|adapter| !adapter.family_tuple.is_empty())
    {
        let call_tuple = machine_arguments
            .iter()
            .map(|argument| call_argument_const_identity(typed, argument))
            .collect::<Option<Vec<_>>>();
        let Some(call_tuple) = call_tuple else {
            return Err(Diagnostic::error(format!(
                "boundary call `{target_name}` supplies a static argument that is not a closed const value; a finite-family call must select one declared tuple",
            )));
        };
        matches
            .iter()
            .filter(|adapter| adapter.family_tuple.as_ref() == call_tuple.as_slice())
            .copied()
            .collect::<Vec<_>>()
    } else {
        matches
    };
    let adapter = match matches.as_slice() {
        [adapter] => *adapter,
        [] => {
            // A generic requirement supplies no row: a call to it would
            // silently reach the unbound template at every width.
            if let Some(requirement) = generic_requirements.iter().find(|requirement| {
                requirement.receiver_trait == field.trait_symbol
                    && requirement.requirement_symbol == target_symbol
            }) {
                return Err(Diagnostic::error(format!(
                    "boundary call `{target_name}` selects generic requirement `{}`, which supplies no executable dispatch row: {}",
                    requirement.requirement_identity, requirement.reason,
                )));
            }
            // The requirement is a declared family with settled rows, but
            // this call's tuple matched none of them.
            if let Some(family) = adapters.iter().find(|adapter| {
                adapter.receiver_trait == field.trait_symbol
                    && adapter.requirement_symbol == target_symbol
                    && !adapter.family_tuple.is_empty()
            }) {
                return Err(Diagnostic::error(format!(
                    "boundary call `{target_name}` does not select a settled tuple of requirement `{}`",
                    family.requirement_identity,
                )));
            }
            let readable = adapters
                .iter()
                .filter(|adapter| {
                    adapter.receiver_trait == field.trait_symbol
                        && adapter.requirement == target_name
                })
                .count();
            if readable == 0 {
                return Ok(None);
            }
            return Err(Diagnostic::error(format!(
                "boundary call `{}`::`{target_name}` matches {readable} selected checked-adapter rows by display name but not exact target symbol {:?}",
                adapters
                    .iter()
                    .find(|adapter| adapter.receiver_trait == field.trait_symbol)
                    .map(|adapter| adapter.receiver_trait_name.as_str())
                    .unwrap_or("<unknown>"),
                target_symbol,
            )));
        }
        many => {
            return Err(Diagnostic::error(format!(
                "boundary call target symbol {:?} matches {} selected checked-adapter rows",
                target_symbol,
                many.len(),
            )));
        }
    };
    if adapter.requirement != target_name {
        return Err(Diagnostic::error(format!(
            "boundary call target symbol {:?} names exact overload `{}`, but its readable method drifted to `{target_name}`",
            target_symbol, adapter.requirement_identity,
        )));
    }
    Ok(Some(adapter))
}
