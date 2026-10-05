//! Exact successor handoffs between composed Unit graph states.
use super::super::{
    CheckFacts, CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan, SymbolHandle, TransitionTargetNode, TypedTrees,
};

use crate::execution::terminal_unit::is_reference;

/// Every state parameter is either planned as a structural or scalar input,
/// erased, or the borrowed data binding the attachment supplies implicitly.
///
/// The implicit position is the one the machine's attached data declares; the
/// retired `self` flag no longer marks it, so the binding is recognized by the
/// data it names.
pub(super) fn only_implicit_data_binding_is_omitted(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    scalar: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .all(|(position, parameter)| {
            structural
                .iter()
                .any(|candidate| candidate.position as usize == position)
                || scalar
                    .iter()
                    .any(|candidate| candidate.source_position as usize == position)
                || (program.parameter_declares_attached_data(machine, parameter)
                    && is_reference(program, parameter.type_reference))
                || parameter.relevance.is_erased()
        })
}

pub(super) fn successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    source_state: &typed_trees::state::State,
    source_parameters: &[CheckedUnitStructuralParameterPlan],
    target_parameters: &[CheckedUnitStructuralParameterPlan],
    source_claims: &[CheckedUnitEntryClaimPlan],
    target_claims: &[CheckedUnitEntryClaimPlan],
    ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    expected: SymbolHandle,
    admitted_local_discards: &[SymbolHandle],
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if path.symbol != expected {
        return None;
    }
    let arguments = program.statement_table.expression_handles(*arguments);
    // One positional rule for every arity. The edge forwards each of the
    // source state's structural parameters to the target parameter in the
    // same position, as the argument that names exactly that parameter's
    // whole place. A linear parameter additionally replays its claim alias
    // events; an unrestricted borrowed parameter — which is how a machine's
    // data binding reaches every state under named implementations — carries
    // no claim and has nothing to replay. Enumerating the
    // parameter/claim/argument combinations instead admitted only the
    // claim-free empty edge and the single owned linear transfer, which left
    // every borrowed data binding without an arm.
    if source_parameters.len() != target_parameters.len()
        || source_parameters.len() != arguments.len()
        || source_claims.len() != target_claims.len()
    {
        return None;
    }
    let sole_claim_at = |claims: &[CheckedUnitEntryClaimPlan], index: usize| {
        let mut rows = claims
            .iter()
            .filter(|claim| claim.parameter_index as usize == index);
        let row = rows.next()?;
        rows.next().is_none().then_some(row.clone())
    };
    // Every claim belongs to a forwarded parameter: a claim on a position
    // this edge does not transfer would be silently dropped.
    if source_claims
        .iter()
        .chain(target_claims)
        .any(|claim| claim.parameter_index as usize >= source_parameters.len())
    {
        return None;
    }
    let mut transfers = Vec::new();
    for (index, ((source, target), argument)) in source_parameters
        .iter()
        .zip(target_parameters)
        .zip(arguments)
        .enumerate()
    {
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            source_state.symbol,
            usize::try_from(ordinal).ok()?,
            *argument,
        )?;
        let facts::PlaceRoot::Symbol(root) = place.root else {
            return None;
        };
        let source_symbol = program
            .state_parameters(source_state)
            .get(source.position as usize)?
            .symbol;
        if root != source_symbol
            || !place.segments.is_empty()
            || source.type_identity != target.type_identity
            || source.multiplicity != target.multiplicity
            || source.access != target.access
        {
            return None;
        }
        match (
            sole_claim_at(source_claims, index),
            sole_claim_at(target_claims, index),
        ) {
            (Some(source_claim), Some(target_claim)) => {
                if !super::custody::exact_claim_alias_events(
                    facts,
                    machine,
                    source_state,
                    ordinal,
                    expected,
                    source_symbol,
                    &source_claim,
                    &target_claim,
                ) {
                    return None;
                }
            }
            (None, None) => {}
            _ => return None,
        }
        let index = u32::try_from(index).ok()?;
        transfers.push(CheckedStructuralControlTransferPlan {
            source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index },
            target_parameter_index: index,
        });
    }
    if admitted_local_discards.is_empty() {
        let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
            machine.symbol,
            source_state.symbol,
            ordinal,
        )?;
        if cleanup.target_state != expected
            || !cleanup
                .trivial_affine_discard_parameter_positions
                .is_empty()
        {
            return None;
        }
    } else if admitted_local_discards.len() != 1
        || !source_parameters.is_empty()
        || !target_parameters.is_empty()
        || !source_claims.is_empty()
        || !target_claims.is_empty()
        || super::super::types::return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            source_state.symbol,
            source_parameters,
            program.state_parameters(source_state),
            &[],
            admitted_local_discards,
            // With no operations there are no moved projections to
            // reconstruct, so the residual lookup never reads the map.
            &std::collections::BTreeMap::new(),
        )
        .is_none_or(|(trivial, residuals, _)| !trivial.is_empty() || !residuals.is_empty())
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: expected,
        transfers,
        scalar_arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
