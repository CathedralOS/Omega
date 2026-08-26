use omega_state_graph::{StateGraph, StateParameterNode};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::name::Identifier;
use psi_checked_trees::state::State;

pub(super) fn state_parameters_for_segment(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    state: &State,
    segment_index: usize,
) -> HandleSpan<StateParameterNode> {
    if segment_index > 0 {
        return HandleSpan::empty();
    }

    let mut parameters = HandleSpan::empty();
    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        let dyn_conformance_rows =
            selected_dynamic_conformance_rows(program, parameter.type_reference);
        let dyn_conformance_candidates = if dyn_conformance_rows.is_empty() {
            eligible_dynamic_conformance_candidates(program, parameter.type_reference)
        } else {
            Vec::new()
        };
        let type_symbol = program.type_reference_symbol(parameter.type_reference);
        let type_name =
            Identifier::generated(program.display_type_reference(parameter.type_reference));

        state_graph.state_parameters.append_to_span(
            &mut parameters,
            StateParameterNode {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: parameter.type_reference,
                type_symbol,
                type_name,
                dyn_conformance_candidates,
                dyn_conformance_rows,
                is_mutable_reference: matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    psi_checked_trees::types::TypeReferenceNode::Reference { access, .. }
                        if access.is_exclusive()
                ),
            },
        );
    }

    parameters
}

fn selected_dynamic_conformance_rows(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Vec<psi_checked_trees::DynamicConformanceRowFact> {
    let Some(conformance_symbol) = dynamic_conformance_symbol(program, type_reference) else {
        return Vec::new();
    };
    let Some(conformance) = program
        .conformances()
        .iter()
        .find(|conformance| conformance.symbol == conformance_symbol)
    else {
        return Vec::new();
    };
    checked_rows_for_conformance(program, conformance)
}

fn eligible_dynamic_conformance_candidates(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Vec<psi_checked_trees::DynamicConformanceCandidateFact> {
    let Some(target_trait) = dynamic_trait_symbol(program, type_reference) else {
        return Vec::new();
    };
    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == target_trait)
    else {
        return Vec::new();
    };
    if trait_definition.is_boundary || !program.trait_type_parameters(trait_definition).is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for data in program.data_definitions() {
        for conformance in program.conformances().iter().filter(|conformance| {
            conformance.carrier_name() == Some(&data.name)
                && conformance.trait_name == trait_definition.name
                && conformance.arguments.is_empty()
                && matches!(
                    conformance.implementation,
                    psi_checked_trees::trait_definition::ConformanceImplementation::Closed { .. }
                )
        }) {
            candidates.push(psi_checked_trees::DynamicConformanceCandidateFact {
                source_data: data.symbol,
                source_name: data.name.clone(),
                conformance: conformance.symbol.is_valid().then_some(conformance.symbol),
                rows: checked_rows_for_conformance(program, conformance),
            });
        }
    }
    candidates
}

fn checked_rows_for_conformance(
    program: &CheckedTrees,
    conformance: &psi_checked_trees::trait_definition::Conformance,
) -> Vec<psi_checked_trees::DynamicConformanceRowFact> {
    program
        .closed_conformance_rows(conformance)
        .unwrap_or_default()
        .iter()
        .filter(|row| row.realization_machine.is_valid() && row.realization_state.is_valid())
        .map(|row| checked_row(program, row))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn checked_row(
    program: &CheckedTrees,
    row: &psi_checked_trees::trait_definition::ConformanceRow,
) -> Option<psi_checked_trees::DynamicConformanceRowFact> {
    let mut declaring_traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == row.declaring_trait);
    let declaring_trait = declaring_traits.next()?;
    if declaring_traits.next().is_some() {
        return None;
    }
    let mut requirements = program
        .trait_machine_signatures(declaring_trait)
        .iter()
        .filter(|requirement| requirement.symbol == row.requirement);
    let requirement = requirements.next()?;
    if requirements.next().is_some() {
        return None;
    }
    let mut realization_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == row.realization_machine);
    let realization_machine = realization_machines.next()?;
    if realization_machines.next().is_some() {
        return None;
    }

    Some(psi_checked_trees::DynamicConformanceRowFact {
        declaring_trait: row.declaring_trait,
        requirement: row.requirement,
        requirement_identity: program
            .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
            .identity(),
        realization_machine: row.realization_machine,
        realization_state: row.realization_state,
        realization_identity: program
            .normalized_machine_overload_identity(realization_machine)?
            .identity(),
        source: match row.source {
            psi_checked_trees::trait_definition::ConformanceRowSource::Inline => {
                psi_checked_trees::DynamicConformanceRowSource::Inline
            }
            psi_checked_trees::trait_definition::ConformanceRowSource::Reference => {
                psi_checked_trees::DynamicConformanceRowSource::Reference
            }
            psi_checked_trees::trait_definition::ConformanceRowSource::TraitDefault => {
                psi_checked_trees::DynamicConformanceRowSource::TraitDefault
            }
        },
    })
}

fn dynamic_conformance_symbol(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            dynamic_conformance_symbol(program, *referee)
        }
        psi_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_conformance_symbol(program, *base_type)
        }
        psi_checked_trees::types::TypeReferenceNode::DynamicTrait { conformance, .. } => {
            *conformance
        }
        _ => None,
    }
}

fn dynamic_trait_symbol(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            dynamic_trait_symbol(program, *referee)
        }
        psi_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            dynamic_trait_symbol(program, *base_type)
        }
        psi_checked_trees::types::TypeReferenceNode::DynamicTrait { symbol, .. } => Some(*symbol),
        _ => None,
    }
}
