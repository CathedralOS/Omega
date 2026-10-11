//! Standard calling vocabulary recognition over the typed program.
//!
//! Calling vocabulary recognition enables policy evaluation, not ABI admission.
//! Recognize its resolved interface rather than the bundled file's bytes: a
//! dependency may retain the same contract across source refactors.

use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::trait_definition::TraitDefinition;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

/// Every standard calling vocabulary the program loads, paired per declaring
/// source file. The toolchain copy and a dependency-supplied public copy are
/// each one exact vocabulary; another copy under a different
/// file is still a distinct declaration pair, so consumers that must select
/// the vocabulary a boundary references join on the relationship symbol
/// rather than the first match.
pub fn standard_calling_vocabularies(
    program: &TypedTrees,
) -> Vec<(&TraitDefinition, &TraitDefinition)> {
    let mut per_file: std::collections::BTreeMap<
        usize,
        (Option<&TraitDefinition>, Option<&TraitDefinition>),
    > = std::collections::BTreeMap::new();
    for definition in program.traits().iter() {
        if definition.is_boundary || definition.refines.is_some() {
            continue;
        }
        let Some(file) = program
            .symbols
            .symbol_source_span(definition.symbol)
            .and_then(|span| program.symbols.source_file(span))
        else {
            continue;
        };
        let entry = per_file.entry(file.source_id.0).or_default();
        match definition.name.as_str() {
            "Calling" => entry.0 = Some(definition),
            "CallingPolicy" => entry.1 = Some(definition),
            _ => {}
        }
    }
    per_file
        .into_values()
        .filter_map(|vocabulary| match vocabulary {
            (Some(calling), Some(policy)) => {
                has_calling_interface(program, calling, policy).then_some((calling, policy))
            }
            _ => None,
        })
        .collect()
}

fn has_calling_interface(
    program: &TypedTrees,
    calling: &TraitDefinition,
    policy: &TraitDefinition,
) -> bool {
    let [parameter] = program.trait_type_parameters(calling) else {
        return false;
    };
    let [bound] = calling.conformance_bounds.as_slice() else {
        return false;
    };
    // Calling selects an explicit implementation map, not a data namespace
    // whose similarly named machine happens to satisfy the plan requirement.
    if parameter.kind != TypeParameterKind::Implementation
        || bound.binder != parameter.symbol
        || bound.trait_symbol != policy.symbol
        || !bound.arguments.is_empty()
        || !program.trait_machine_signatures(calling).is_empty()
        || !program.trait_type_parameters(policy).is_empty()
    {
        return false;
    }
    let mut plans = program
        .trait_machine_signatures(policy)
        .iter()
        .filter(|signature| signature.name.as_str() == "plan");
    let Some(plan) = plans.next() else {
        return false;
    };
    if plans.next().is_some() {
        return false;
    }
    let [signature] = program.state_signature_parameters(plan) else {
        return false;
    };
    program.state_signature_type_parameters(plan).is_empty()
        && plan.native_callback_parameters.is_empty()
        && !signature.is_const
        && signature.relevance == language_core::BindingRelevance::Relevant
        && is_calling_interface_data(
            program,
            policy.symbol,
            signature.type_reference,
            "BoundarySignature",
        )
        && is_calling_interface_data(
            program,
            policy.symbol,
            plan.return_type,
            "BoundaryPlanResult",
        )
}

fn is_calling_interface_data(
    program: &TypedTrees,
    owner: SymbolHandle,
    type_reference: TypeReferenceHandle,
    name: &str,
) -> bool {
    // The evaluator passes and receives owned nominal values. Unwrapping a
    // reference, array, or generic application to its base symbol would erase
    // that callable contract. Match the declaration, not the reference spelling.
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    let Some(definition) = program.unique_data_definition_by_symbol(*symbol) else {
        return false;
    };
    let declaring_source = |symbol| {
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .map(|source| source.source_id)
    };
    definition.name.as_str() == name
        && declaring_source(owner).is_some_and(|owner| declaring_source(*symbol) == Some(owner))
}
