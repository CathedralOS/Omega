//! Emit a composed-control machine: `emit` allocates the state block
//! namespaces and the invocation entry, walks every authored state through
//! `StateGraphEmission::emit_state`, then assembles the machine from the
//! blocks, ranks and places that walk accumulated.
use super::super::super::super::{
    LoweredSourceCallOccurrence, MachineId, StructuralArgument, StructuralParameterDeclaration,
    block_id,
};
use super::super::super::{
    Block, MachineContract, StructuralPlaceDeclaration, StructuralPlaceKind, TerminalMachine,
    Terminator, ValueDeclaration, allocate_dense, contract_id, edge_id, lookup_claim_id,
    lookup_type_id, lower_checked_crash_route_buckets, lower_installation_machine_service_ceiling,
    lower_unit_parameters, obligation_id, terminal_scalar_type, unsupported, value_id,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{AdmittedGraph, CheckedComposedUnitControlMachinePlan, ranking, returns, successors};
use crate::unit::TerminalMachineResult;
use crate::unit::attached_unit::claims::LoweredUnitClaims;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::{BlockId, EdgeId, ValueId};
use std::collections::BTreeMap;

mod state;

/// One composed-control machine's emission in flight: the checked plan and
/// admitted graph, the shared catalogs and their identity counters, the per
/// state block namespaces, and the blocks, ranks and occurrences accumulated
/// so far. `emit_state` (in `emission/state.rs`) emits one authored state.
pub(super) struct StateGraphEmission<'a, 'c> {
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
    admitted: AdmittedGraph<'a>,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
    machine_result: TerminalMachineResult,
    claims: LoweredUnitClaims,
    content_entry_claims: Vec<terminal_psi::ContentEntryClaim>,
    content_identity_reshuffles: Vec<terminal_psi::ContentIdentityReshuffle>,
    entry_reentered: bool,
    state_ids: Vec<BlockId>,
    state_views: Vec<Vec<StructuralParameterDeclaration>>,
    state_values: Vec<Vec<ValueDeclaration>>,
    /// Each emitted state's erased-formal roster: the machine contract
    /// formals for a plain entry, fresh block-level declarations otherwise.
    state_erased: Vec<Vec<ValueDeclaration>>,
    /// Jump and Conditional edge identities emitted so far, keyed by their
    /// target block — the exact roster a header invariant must cite.
    arrival_edges: BTreeMap<BlockId, Vec<EdgeId>>,
    structural_places: Vec<StructuralPlaceDeclaration>,
    blocks: Vec<Block>,
    occurrences: Vec<LoweredSourceCallOccurrence>,
    block_ranks: BTreeMap<BlockId, ValueId>,
    rank_edges: BTreeMap<EdgeId, (ValueId, terminal_psi::TerminalNaturalRankComparison)>,
}

pub(in crate::unit::attached_unit::composed_control) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    admitted: AdmittedGraph<'_>,
    terminal_machine: MachineId,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    entry_erased_formals: Vec<ValueDeclaration>,
    catalogs: &mut catalogs::ComposedCatalogs,
) -> Result<
    (
        TerminalMachine,
        Vec<LoweredSourceCallOccurrence>,
        Vec<terminal_psi::ScalarBlockInvariant>,
    ),
    LoweringError,
> {
    let result_places_start = catalogs.result_places.len();
    let mut structural_places = parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect::<Vec<_>>();
    let machine_result = returns::result(&plan.result, catalogs, &mut structural_places)?;
    let entry = &plan.states[0];
    let mut claims = crate::unit::attached_unit::claims::lower_unit_entry_claims(
        plan.machine,
        entry.state,
        &entry.entry_claims,
        &parameters,
    )?;
    // Successor-state claims keep the entry parameter's place; their checked
    // identities join that entry claim so call and return replays resolve.
    for (successor, entry) in &admitted.claim_transport.aliases {
        let claim = lookup_claim_id(&claims.source_claims, *entry)?;
        claims.source_claims.push((*successor, claim));
    }
    let content_entry_claims =
        crate::proofs::content_conservation::lower_whole_content_entry_claims(
            checked,
            &catalogs.structural_types,
            &entry.structural_parameters,
            &parameters,
            &entry.entry_claims,
            &claims.source_claims,
        )?;
    let content_identity_reshuffles = crate::unit::attached_unit::claims::lower_result_identity(
        checked,
        plan.machine,
        entry.state,
        &entry.structural_parameters,
        &parameters,
        &machine_result,
        &claims.source_claims,
    )?;
    let entry_reentered = plan
        .states
        .iter()
        .flat_map(successors)
        .any(|successor| successor.target_state == plan.states[0].state);
    // Invocation parameters are immutable. Reentering the authored entry
    // therefore uses an ordinary parameterized block behind a one-shot jump.
    let invocation_entry = if entry_reentered {
        Some(block_id(allocate_dense(&mut catalogs.next_block)?))
    } else {
        None
    };
    let mut state_ids = Vec::new();
    let mut state_views = Vec::new();
    let mut state_values = Vec::new();
    let mut state_erased = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        state_ids.push(block_id(allocate_dense(&mut catalogs.next_block)?));
        if position != 0 || entry_reentered {
            let mut block_parameters = lower_unit_parameters(
                &state.structural_parameters,
                &catalogs.type_ids,
                &catalogs.domain_ids,
                &mut catalogs.next_place,
            )?;
            let mut block_position = 0_u32;
            for (dense, parameter) in block_parameters.iter_mut().enumerate() {
                if parameter.is_self {
                    *parameter = parameters
                        .iter()
                        .find(|entry| entry.is_self)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph receiver invocation is missing",
                        ))?
                        .clone();
                } else if let Some(root) = admitted
                    .claim_transport
                    .aliased
                    .get(position)
                    .and_then(|aliased| aliased.get(&(dense as u32)))
                {
                    // A claim carried across an edge keeps the entry
                    // parameter's place; the block does not re-declare it.
                    parameter.place = parameters
                        .get(*root as usize)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph claim alias lost its entry parameter",
                        ))?
                        .place;
                } else {
                    // The persistent receiver and claim aliases are not block
                    // parameters. Only transferred structural values occupy
                    // its dense namespace.
                    parameter.position = block_position;
                    block_position =
                        block_position
                            .checked_add(1)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph block parameter count exceeds u32",
                            ))?;
                }
            }
            structural_places.extend(
                block_parameters
                    .iter()
                    .enumerate()
                    .filter(|(dense, parameter)| {
                        !parameter.is_self
                            && !admitted
                                .claim_transport
                                .aliased
                                .get(position)
                                .is_some_and(|aliased| aliased.contains_key(&(*dense as u32)))
                    })
                    .map(|(_, parameter)| StructuralPlaceDeclaration {
                        id: parameter.place,
                        kind: StructuralPlaceKind::BlockParameter {
                            block: state_ids[position],
                            position: parameter.position,
                        },
                    }),
            );
            state_views.push(block_parameters);
            state_values.push(
                state
                    .scalar_parameters
                    .iter()
                    .map(|parameter| {
                        Ok(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value_id(allocate_dense(&mut catalogs.next_value)?),
                            scalar_type: terminal_scalar_type(parameter.primitive_type)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            );
            state_erased.push(
                crate::scalar_graph::scalar_contracts::erased_formal_declarations(
                    &state.erased_scalar_parameters,
                    &mut catalogs.next_value,
                )?,
            );
        } else {
            state_views.push(parameters.clone());
            state_values.push(scalar_parameters.clone());
            // A plain entry shares the machine contract formals; the block
            // does not redeclare them.
            state_erased.push(entry_erased_formals.clone());
        }
    }
    let mut blocks = Vec::new();
    let mut arrival_edges = BTreeMap::new();
    if let Some(entry) = invocation_entry {
        let edge = edge_id(allocate_dense(&mut catalogs.next_edge)?);
        arrival_edges
            .entry(state_ids[0])
            .or_insert_with(Vec::new)
            .push(edge);
        blocks.push(Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge,
                target: state_ids[0],
                arguments: scalar_parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .collect(),
                // A reentered entry forwards the machine's proof-only
                // formals into the authored block's own erased declarations.
                erased_arguments: entry_erased_formals
                    .iter()
                    .map(|parameter| ScalarTerm::value(parameter.id, parameter.scalar_type))
                    .collect(),
                structural_arguments: parameters
                    .iter()
                    .enumerate()
                    .filter(|(dense, parameter)| {
                        !parameter.is_self
                            && !admitted
                                .claim_transport
                                .aliased
                                .first()
                                .is_some_and(|aliased| aliased.contains_key(&(*dense as u32)))
                    })
                    .map(|(_, parameter)| StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: parameter.access,
                    })
                    .collect(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        });
    }
    let occurrences = Vec::new();
    let block_ranks = std::collections::BTreeMap::new();
    let rank_edges = std::collections::BTreeMap::new();
    let mut emission = StateGraphEmission {
        checked,
        plan,
        admitted,
        parameters,
        scalar_parameters,
        catalogs,
        machine_result,
        claims,
        content_entry_claims,
        content_identity_reshuffles,
        entry_reentered,
        state_ids,
        state_views,
        state_values,
        state_erased,
        arrival_edges,
        structural_places,
        blocks,
        occurrences,
        block_ranks,
        rank_edges,
    };
    for position in 0..plan.states.len() {
        emission.emit_state(position)?;
    }
    let StateGraphEmission {
        parameters,
        scalar_parameters,
        catalogs,
        machine_result,
        claims,
        content_entry_claims,
        content_identity_reshuffles,
        state_ids,
        state_values,
        state_erased,
        arrival_edges,
        mut structural_places,
        mut blocks,
        occurrences,
        block_ranks,
        rank_edges,
        ..
    } = emission;
    // A retained `requires` becomes the header's scalar block invariant:
    // every incoming Jump or Conditional edge discharges it before the block
    // runs. A plain entry publishes its clauses on the machine contract
    // instead — invocation is an arrival without an edge proof.
    let mut scalar_block_invariants = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        if state.requires.is_empty() || (position == 0 && !entry_reentered) {
            continue;
        }
        let header = state_ids[position];
        let case_arrival = blocks.iter().any(|block| {
            matches!(
                &block.terminator,
                Terminator::StructuralCase { cases, .. }
                    if cases.iter().any(|case| case.target == header)
            )
        });
        if case_arrival {
            return unsupported("Unit graph requires header reached by case dispatch");
        }
        let Some(predicate) = crate::scalar_graph::scalar_contracts::clauses(
            &state.requires,
            &state_values[position],
            &state_erased[position],
        )?
        else {
            continue;
        };
        let mut arrivals = Vec::new();
        for edge in arrival_edges.get(&header).cloned().unwrap_or_default() {
            arrivals.push(terminal_psi::ScalarBlockInvariantArrival {
                edge,
                obligation: obligation_id(allocate_dense(
                    &mut catalogs.scalar_calls.next_call_obligation,
                )?),
            });
        }
        if arrivals.is_empty() {
            return unsupported("Unit graph requires header has no edge arrivals");
        }
        scalar_block_invariants.push(terminal_psi::ScalarBlockInvariant {
            machine: terminal_machine,
            header,
            predicate,
            arrivals,
        });
    }
    blocks.sort_by_key(|block| block.id);
    structural_places.extend(
        blocks
            .iter()
            .flat_map(|block| {
                crate::scalar_graph::scalar_computations::arrays::declarations(&block.operations)
                    .chain(
                        crate::scalar_graph::scalar_computations::cases::declarations(
                            &block.operations,
                        ),
                    )
            })
            .filter(|place| {
                !catalogs
                    .result_places
                    .iter()
                    .chain(&catalogs.temporary_places)
                    .any(|existing| existing.id == place.id)
            }),
    );
    structural_places.append(&mut catalogs.temporary_places);
    structural_places.extend(catalogs.result_places.drain(result_places_start..));
    structural_places.sort_by_key(|place| place.id);
    let attachment = plan
        .attachment_type_identity
        .as_ref()
        .map(|identity| lookup_type_id(&catalogs.type_ids, identity))
        .transpose()?;
    if let Some(attachment) = attachment {
        let declaration = catalogs
            .structural_types
            .iter()
            .find(|declaration| declaration.id == attachment)
            .ok_or(LoweringError::Unsupported(
                "Unit graph attachment declaration is absent",
            ))?;
        let boundaries = catalogs
            .lowered_boundaries
            .iter()
            .map(|boundary| (boundary.source, boundary.id))
            .collect::<Vec<_>>();
        structural_places.extend(
            crate::unit::attached_unit::provider_attachments::lower_provider_attachment_places(
                attachment,
                declaration,
                &plan.provider_attachment_requirements,
                &boundaries,
                &mut catalogs.next_place,
            )?,
        );
    }
    let mut machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: crate::unit::attached_unit::lower_declared_service_reach(
            checked,
            plan.machine,
            &catalogs.service_ids,
        )?,
        id: terminal_machine,
        attachment,
        parameters: scalar_parameters,
        structural_places,
        structural_parameters: parameters,
        entry_claims: claims.entry_claims,
        ranked_scc: None,
        result: machine_result,
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &catalogs.service_ids,
        )?,
        content_entry_claims,
        content_identity_reshuffles,
        content_partition_compositions: Vec::new(),
        entry: invocation_entry.unwrap_or(state_ids[0]),
        blocks,
        contract: MachineContract {
            id: contract_id(terminal_machine.get()),
            requires: Vec::new(),
            ensures: Vec::new(),
            crash_routes: Vec::new(),
            erased_scalar_formals: entry_erased_formals,
            outcome_specific_ensures: Vec::new(),
        },
    };
    machine.contract.crash_routes =
        lower_checked_crash_route_buckets(&catalogs.root_crash_routes, &machine.parameters)?;
    ranking::retain(&mut machine, &block_ranks, &rank_edges)?;
    Ok((machine, occurrences, scalar_block_invariants))
}
