//! Authentication of the bundled float map's inferred intrinsic members.
//!
//! Numeric meaning comes from the exact public requirement. A bare signature
//! or a same-shaped member elsewhere grants no implementation or proof facts.

use crate::TypedTrees;
use crate::machine::Machine;
use crate::signature::{SignatureContract, SignatureContractKind, StateParameter};
use crate::trait_definition::ConformanceRowSource;
use language_semantics::numeric_intrinsics::NumericIntrinsic;
use language_semantics::{ExternalBindingIdentity, ExternalBindingMechanism, MachineSupplyMode};
use symbols::SymbolHandle;

pub const SOURCE: &str = "float_provider.omg";

fn sealed(typed: &TypedTrees, symbol: SymbolHandle, path: &str, source: &str) -> bool {
    typed
        .symbol_toolchain_relative_path(symbol)
        .is_some_and(|relative| relative == std::path::Path::new(source))
        && typed
            .normalized_hermetic_symbol_identity(symbol)
            .is_ok_and(|identity| identity.strip_prefix("toolchain::") == Some(path))
}

/// Whether a declaration claims the one core map's implicit catalog supply.
/// Validation must refuse a claim that cannot rejoin `intrinsic_member`.
pub fn is_catalog_declaration(typed: &TypedTrees, machine: &Machine) -> bool {
    typed
        .symbol_toolchain_relative_path(machine.symbol)
        .is_some_and(|relative| relative == std::path::Path::new(SOURCE))
        && machine.name.as_str().starts_with("NativeFloats::")
}

/// Rejoin the exact owned member, its authored public satisfaction, complete
/// value signature and fixed numeric meaning. No provider is selected here.
pub fn intrinsic_member(
    typed: &TypedTrees,
    machine: &Machine,
) -> Option<(SymbolHandle, NumericIntrinsic)> {
    let (implementation, row) = typed.owned_conformance_realization(machine.symbol)?;
    let primary = typed.trait_definition_by_symbol(implementation.trait_symbol)?;
    if !sealed(typed, implementation.symbol, "NativeFloats", SOURCE)
        || !sealed(typed, primary.symbol, "FloatNativeOperations", SOURCE)
        || primary.is_boundary
        || !typed.trait_type_parameters(primary).is_empty()
        || !primary.lifetime_parameters.is_empty()
        // These are inherited trait obligations, separate from machine slots.
        || !typed.trait_requirements(primary).is_empty()
        || !primary.conformance_bounds.is_empty()
        || row.source != ConformanceRowSource::Inline
        || !implementation.type_parameters.is_empty()
        || !implementation.lifetime_parameters.is_empty()
        || !implementation.arguments.is_empty()
        || !implementation.trait_lifetime_arguments.is_empty()
        || machine.body_is_present
        || machine.attached_data.is_some()
        || !typed.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || machine.suspends
        || machine.blocks
        || !machine.invokes.is_empty()
        || !machine.owned_data.is_empty()
        || typed.machine_contracts(machine).iter().any(|contract| {
            matches!(
                contract.kind,
                SignatureContractKind::Ensures | SignatureContractKind::EnsuresForResultCase { .. }
            )
        })
    {
        return None;
    }
    let MachineSupplyMode::ExternalRealization {
        binding: Some(binding),
        mechanism: Some(ExternalBindingMechanism::CompilerIntrinsic),
    } = machine.supply_mode
    else {
        return None;
    };
    if typed.external_bindings.identity(binding)
        != Some(&ExternalBindingIdentity::CompilerIntrinsic)
    {
        return None;
    }
    let [edge] = typed.machine_trait_conformances(machine) else {
        return None;
    };
    if edge.external_binding != Some(binding)
        || edge.via_expression.is_valid()
        || edge.external_binding_source_span.is_some()
    {
        return None;
    }
    let (requirement_symbol, _) =
        crate::provider_declarations::boundary_satisfaction_identity(typed, machine, edge)?;
    let requirement =
        super::intrinsic_requirements::IntrinsicRequirement::by_symbol(typed, requirement_symbol)?;
    if !sealed(
        typed,
        requirement_symbol,
        &requirement.display(),
        numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
    ) {
        return None;
    }
    let meaning = typed.numeric_intrinsic_execution(requirement_symbol)?;
    let [entry] = typed.machine_states(machine) else {
        return None;
    };
    if entry.symbol != row.realization_state {
        return None;
    }
    let parameters = typed.state_parameters(entry);
    if parameters.len() != requirement.parameters.len()
        || parameters
            .iter()
            .zip(requirement.parameters)
            .any(|(actual, required)| {
                actual.is_const != required.is_const
                    || actual.is_mutable != required.is_mutable
                    || actual.relevance != required.relevance
                    || typed.normalized_type_identity(actual.type_reference)
                        != typed.normalized_type_identity(required.type_reference)
            })
        || typed.normalized_type_identity(entry.return_type)
            != typed.normalized_type_identity(requirement.return_type)
    {
        return None;
    }
    let actual_formation = formation_clauses(typed, parameters, typed.machine_contracts(machine))?;
    let expected_formation =
        formation_clauses(typed, requirement.parameters, requirement.contracts(typed)?)?;
    if actual_formation != expected_formation || !typed.state_contracts(entry).is_empty() {
        return None;
    }
    Some((requirement_symbol, meaning))
}

/// Preserve the complete formation and crash obligations using the shared
/// expression normalizer and positional subjects. Equality here compares
/// declared structure; it does not invent a theorem about different formulas.
fn formation_clauses(
    typed: &TypedTrees,
    parameters: &[StateParameter],
    contracts: &[SignatureContract],
) -> Option<Vec<(SignatureContractKind, Vec<String>)>> {
    let binders = parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| (parameter.symbol, format!("$V{position}")))
        .collect::<Vec<_>>();
    let mut clauses = Vec::new();
    for contract in contracts {
        if matches!(
            contract.kind,
            SignatureContractKind::Ensures | SignatureContractKind::EnsuresForResultCase { .. }
        ) {
            continue;
        }
        if contract.binding.is_some() {
            return None;
        }
        let facts = typed
            .proof_facts
            .span_or_empty(contract.facts)
            .iter()
            .map(|fact| {
                let crate::domain::ProofFact::Expression(expression) = fact else {
                    return None;
                };
                crate::type_identity::normalized_expression_identity(typed, *expression, &binders)
            })
            .collect::<Option<Vec<_>>>()?;
        clauses.push((contract.kind.clone(), facts));
    }
    Some(clauses)
}
