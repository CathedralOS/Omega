//! Claim transport across composed-control state edges.
//!
//! A claim is established at every admission of the state that names it, so a
//! claim carried across a state edge cannot become a fresh block parameter:
//! it keeps the machine entry parameter's place. `resolve` replays each
//! edge's checked Transfer event and resolves every successor claim onto the
//! entry parameter whose place still stores it. Emission then trusts two
//! maps: `aliased` binds a claim-bearing dense parameter to the retained
//! entry parameter, and `aliases` joins each successor claim identity to the
//! entry claim whose lowered `ClaimId` it shares.
//!
//! The entry state's claim parameters seed the resolution with their own
//! invocation places. A successor parameter resolves once an incoming edge
//! names a source parameter whose place is already resolved; cyclic pairs
//! settle through their entry-seeded edges and the final pass rechecks every
//! edge, so two edges feeding one claim from different places still reject.
use std::collections::BTreeMap;

use super::super::super::{Multiplicity, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{CheckedComposedUnitControlMachinePlan, successors};
use checked_trees::CheckedStructuralControlTransferSourcePlan;
use language_semantics::{
    CarryPolicy, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};

/// Resolved claim custody for one composed machine. `aliased[position][dense]`
/// names the entry structural parameter whose place that successor parameter
/// keeps; `aliases` joins each successor claim identity to the entry claim it
/// transports, so the lowered `ClaimId` table resolves either identity.
pub(super) struct ClaimTransport {
    pub(super) aliases: Vec<(PermissionClaimIdentity, PermissionClaimIdentity)>,
    /// One map per plan state; a present entry binds that successor
    /// structural parameter to `states[0].structural_parameters[root]`.
    pub(super) aliased: Vec<BTreeMap<u32, u32>>,
}

/// Rejoin receipts before state-local claim identities are aliased to one
/// Terminal claim. A receipt naming an earlier state's alias is not evidence
/// that this call consumed the current state's claim.
pub(super) fn validate_boundary_consumption(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    target: symbols::SymbolHandle,
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    receipts: &[checked_trees::CheckedUnitClaimTransferPlan],
) -> Result<(), LoweringError> {
    let events = &checked.facts.flow.ownership;
    let call_source = PermissionEventSource::Call {
        statement_index: coordinate.statement_index as usize,
        call_ordinal: coordinate.call_ordinal as usize,
        target_symbol: target,
    };
    let consumption = events
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine
                && event.state_symbol == state.state
                && event.source == call_source
                && event.kind == PermissionEventKind::Consume
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .collect::<Vec<_>>();
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            state.entry_claims.iter().filter_map(move |claim| {
                (argument.access == checked_trees::CheckedStructuralAccess::Owned
                    && argument.source_parameter_index() == Some(claim.parameter_index)
                    && argument.path == claim.path)
                    .then_some((argument_index, claim))
            })
        })
        .collect::<Vec<_>>();
    if receipts.len() != expected.len() || consumption.len() != expected.len() {
        return unsupported(
            "Unit graph boundary receipt roster disagrees with checked consumption",
        );
    }
    for (argument_index, claim) in expected {
        let parameter = state
            .structural_parameters
            .get(claim.parameter_index as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph consumed claim parameter is absent",
            ))?;
        let source_parameter = checked
            .state_parameters(source)
            .get(parameter.position as usize)
            .ok_or(LoweringError::Unsupported(
                "Unit graph consumed claim source is absent",
            ))?;
        if receipts
            .iter()
            .filter(|receipt| {
                receipt.claim_identity == claim.claim_identity
                    && receipt.argument_index as usize == argument_index
            })
            .count()
            != 1
            || consumption
                .iter()
                .filter(|event| {
                    event.claim_identity == claim.claim_identity
                        && event.root == facts::PlaceRoot::Symbol(source_parameter.symbol)
                        && events.segments.span_or_empty(event.segments).len()
                            == event.segments.len()
                        && validation::structural_claim_path(
                            &checked.typed,
                            source_parameter.type_reference,
                            events.segments.span_or_empty(event.segments),
                        )
                        .is_ok_and(|path| path == claim.path)
                })
                .count()
                != 1
        {
            return unsupported("Unit graph boundary receipt lost its exact consumed claim");
        }
    }
    Ok(())
}

pub(super) fn resolve(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<ClaimTransport, LoweringError> {
    let mut aliased = vec![BTreeMap::new(); plan.states.len()];
    let mut aliases = Vec::new();
    if plan
        .states
        .iter()
        .all(|state| state.entry_claims.is_empty())
    {
        return Ok(ClaimTransport { aliases, aliased });
    }
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "Unit graph claim transport has no authored machine",
        ))?;
    let source_states = checked.machine_states(machine);
    let entry_reentered = plan
        .states
        .iter()
        .flat_map(successors)
        .any(|successor| successor.target_state == plan.states[0].state);
    // Incoming graph edges of `target` in authored order: (source position,
    // successor edge).
    let incoming = |target: symbols::SymbolHandle| {
        plan.states
            .iter()
            .enumerate()
            .flat_map(|(position, state)| {
                successors(state)
                    .into_iter()
                    .filter(move |edge| edge.target_state == target)
                    .map(move |edge| (position, edge))
            })
            .collect::<Vec<_>>()
    };
    // A parameter's retained entry root: without reentry the entry block uses
    // the invocation parameters directly, so every parameter still sits in its
    // own place; otherwise only claim-aliased parameters keep entry places.
    let resolved_root = |aliased: &[BTreeMap<u32, u32>],
                         source_position: usize,
                         source_index: u32|
     -> Option<u32> {
        if source_position == 0 && !entry_reentered {
            Some(source_index)
        } else {
            aliased
                .get(source_position)
                .and_then(|aliased| aliased.get(&source_index).copied())
        }
    };
    // The entry state's claim parameters keep their own invocation places;
    // that seed also resolves entry parameters through a reentry edge.
    for claim in &plan.states[0].entry_claims {
        aliased[0].insert(claim.parameter_index, claim.parameter_index);
    }
    // Every claim parameter must be an owned linear parameter whose incoming
    // edges all move one exact source parameter and replay that source claim's
    // checked Transfer event at the edge's authored call coordinate.
    for (position, state) in plan.states.iter().enumerate() {
        for claim in &state.entry_claims {
            let parameter = state
                .structural_parameters
                .get(claim.parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph claim parameter is missing",
                ))?;
            if claim.carry != CarryPolicy::STRICT
                || parameter.is_self
                || parameter.access != checked_trees::CheckedStructuralAccess::Owned
                || parameter.multiplicity != Multiplicity::Linear
            {
                return unsupported("Unit graph claim is not exact owned linear custody");
            }
            let incoming = incoming(state.state);
            if position != 0 && incoming.is_empty() {
                return unsupported("Unit graph claim has no transporting edge");
            }
            for (source_position, edge) in incoming {
                let Some(transfer) = edge
                    .transfers
                    .iter()
                    .find(|transfer| transfer.target_parameter_index == claim.parameter_index)
                else {
                    return unsupported("Unit graph claim transfer row is missing");
                };
                let CheckedStructuralControlTransferSourcePlan::Parameter { index } =
                    transfer.source
                else {
                    return unsupported(
                        "Unit graph claim successor is not a whole parameter transfer",
                    );
                };
                let source_state = &plan.states[source_position];
                let source_parameter = source_state
                    .structural_parameters
                    .get(index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph claim transfer source parameter is missing",
                    ))?;
                if source_parameter.is_self
                    || source_parameter.access != checked_trees::CheckedStructuralAccess::Owned
                    || source_parameter.multiplicity != Multiplicity::Linear
                {
                    return unsupported("Unit graph claim source is not owned linear custody");
                }
                // The edge moves the source claim covering this parameter at
                // this exact path; a different path transfers a different
                // claim identity.
                let source_claim = source_state
                    .entry_claims
                    .iter()
                    .find(|candidate| {
                        candidate.parameter_index == index && candidate.path == claim.path
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph claim lost its source claim",
                    ))?;
                let source = source_states
                    .iter()
                    .find(|source| source.symbol == source_state.state)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph claim source state is missing",
                    ))?;
                let source_parameter_source = checked
                    .state_parameters(source)
                    .get(source_parameter.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph claim transfer source is missing",
                    ))?;
                let root = facts::PlaceRoot::Symbol(if source_parameter_source.is_self {
                    plan.machine
                } else {
                    source_parameter_source.symbol
                });
                let mut events = checked
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .map(|(_, event)| event)
                    .filter(|event| {
                        event.machine_symbol == plan.machine
                            && event.state_symbol == source_state.state
                            && event.kind == PermissionEventKind::Transfer
                            && event.claim_identity == source_claim.claim_identity
                            && event.root == root
                            && matches!(event.source,
                                PermissionEventSource::Call {
                                    statement_index,
                                    target_symbol,
                                    ..
                                } if statement_index == edge.statement_ordinal as usize
                                    // An entry reentry can spell the machine
                                    // name; every other edge names its state.
                                    && (target_symbol == edge.target_state
                                        || (target_symbol == plan.machine
                                            && edge.target_state == plan.states[0].state)))
                    });
                let Some(event) = events.next() else {
                    return unsupported("Unit graph claim transfer event is missing");
                };
                if events.next().is_some()
                    || event.access != PermissionAccess::Owned
                    || event.multiplicity != Multiplicity::Linear
                    || !event.obligation_live
                    || event.provenance
                        != (PermissionProvenance::Established {
                            machine_symbol: plan.machine,
                            state_symbol: source_state.state,
                            source: PermissionEventSource::StateEntry,
                        })
                    || checked
                        .facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .len()
                        != event.segments.len()
                    || validation::structural_claim_path(
                        &checked.typed,
                        source_parameter_source.type_reference,
                        checked
                            .facts
                            .flow
                            .ownership
                            .segments
                            .span_or_empty(event.segments),
                    )
                    .ok()
                    .as_ref()
                        != Some(&claim.path)
                {
                    return unsupported("Unit graph claim transfer custody drifted");
                }
            }
        }
    }
    // Resolve each claimed parameter's retained entry place. An edge
    // contributes once its source parameter's place resolves, so cyclic pairs
    // settle through their entry-seeded edges.
    loop {
        let mut progress = false;
        for position in 1..plan.states.len() {
            let mut claimed = plan.states[position]
                .entry_claims
                .iter()
                .map(|claim| claim.parameter_index)
                .collect::<Vec<_>>();
            claimed.sort_unstable();
            claimed.dedup();
            for index in claimed {
                if aliased[position].contains_key(&index) {
                    continue;
                }
                let mut root = None;
                for (source_position, edge) in incoming(plan.states[position].state) {
                    let Some(transfer) = edge
                        .transfers
                        .iter()
                        .find(|transfer| transfer.target_parameter_index == index)
                    else {
                        continue;
                    };
                    let CheckedStructuralControlTransferSourcePlan::Parameter {
                        index: source_index,
                    } = transfer.source
                    else {
                        continue;
                    };
                    let Some(candidate) = resolved_root(&aliased, source_position, source_index)
                    else {
                        continue;
                    };
                    if let Some(existing) = root
                        && existing != candidate
                    {
                        return unsupported(
                            "Unit graph claim parameter carried from divergent entry places",
                        );
                    }
                    root = Some(candidate);
                }
                if let Some(root) = root {
                    aliased[position].insert(index, root);
                    progress = true;
                }
            }
        }
        if !progress {
            break;
        }
    }
    // Every incoming edge of every claimed parameter must now resolve to the
    // one entry place that parameter keeps.
    for (position, state) in plan.states.iter().enumerate() {
        let mut claimed = state
            .entry_claims
            .iter()
            .map(|claim| claim.parameter_index)
            .collect::<Vec<_>>();
        claimed.sort_unstable();
        claimed.dedup();
        for index in claimed {
            let Some(&root) = aliased[position].get(&index) else {
                return unsupported("Unit graph claim transport lost its entry place");
            };
            for (source_position, edge) in incoming(state.state) {
                let Some(transfer) = edge
                    .transfers
                    .iter()
                    .find(|transfer| transfer.target_parameter_index == index)
                else {
                    return unsupported("Unit graph claim transfer row is missing");
                };
                let CheckedStructuralControlTransferSourcePlan::Parameter {
                    index: source_index,
                } = transfer.source
                else {
                    return unsupported(
                        "Unit graph claim successor is not a whole parameter transfer",
                    );
                };
                if resolved_root(&aliased, source_position, source_index) != Some(root) {
                    return unsupported(
                        "Unit graph claim parameter carried from divergent entry places",
                    );
                }
            }
        }
    }
    // Each successor claim names the entry claim whose place it retains; the
    // lowered ClaimId resolves either spelling.
    for (position, state) in plan.states.iter().enumerate().skip(1) {
        for claim in &state.entry_claims {
            let root = aliased[position][&claim.parameter_index];
            let entry_claim = plan.states[0]
                .entry_claims
                .iter()
                .find(|entry| entry.parameter_index == root && entry.path == claim.path)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph claim lost its entry claim",
                ))?;
            aliases.push((claim.claim_identity, entry_claim.claim_identity));
        }
    }
    Ok(ClaimTransport { aliases, aliased })
}
