//! Emit a composed-control machine: `emit` allocates the state block
//! namespaces and the invocation entry, walks every authored state through
//! `StateGraphEmission::emit_state`, then assembles the machine from the
//! blocks, ranks and places that walk accumulated.
//!
//! Source requirements are obligations of Psi checking, not a second language
//! limited to this emitter's scalar vocabulary. A checked state retains both
//! its closed projections and its unprojected authored expressions. The latter
//! can cross an internal header only when every actual incoming edge carries
//! the exact completed source-arrival decision. Closed declaration/runtime
//! requirements remain in the emitted invariant; an unprojected expression
//! never becomes Truth, an opaque certified atom, or an optimizer premise.
use super::super::super::super::{
    MachineId, StructuralArgument, StructuralParameterDeclaration, block_id,
};
use super::super::super::{
    Block, MachineContract, StructuralPlaceDeclaration, StructuralPlaceKind, TerminalMachine,
    Terminator, ValueDeclaration, allocate_dense, contract_id, edge_id, lookup_claim_id,
    lookup_type_id, lower_checked_crash_route_buckets, lower_installation_machine_service_ceiling,
    lower_unit_parameters, obligation_id, unsupported,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{AdmittedGraph, CheckedComposedUnitControlMachinePlan, ranking, returns, successors};
use crate::unit::TerminalMachineResult;
use crate::unit::attached_unit::claims::LoweredUnitClaims;
use crate::unit::claim_id;
use checked_trees::CheckedErasedProofParameterPlan;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::{BlockId, EdgeId, ValueId};
use std::collections::BTreeMap;

mod captured_loans;
mod conditional_return;
mod guarded_chain;
mod loan_chains;
mod loaned_local_homes;
mod seal;
mod state;
mod successor_edge;

/// One composed-control machine's emission in flight: the checked plan and
/// admitted graph, the shared catalogs and their identity counters, the per
/// state block namespaces, and the blocks, ranks and occurrences accumulated
/// so far. `emit_state` (in `emission/state.rs`) emits one authored state.
pub(super) struct StateGraphEmission<'a, 'c> {
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
    /// Fused source states emit into this one Terminal machine namespace.
    terminal_machine: MachineId,
    admitted: AdmittedGraph<'a>,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    catalogs: &'a mut catalogs::ComposedCatalogs<'c>,
    machine_result: TerminalMachineResult,
    claims: LoweredUnitClaims,
    content_entry_claims: Vec<terminal_psi::ContentEntryClaim>,
    content_identity_reshuffles: Vec<terminal_psi::ContentIdentityReshuffle>,
    loan_completions: Vec<terminal_psi::LoanCompletion>,
    entry_reentered: bool,
    state_ids: Vec<BlockId>,
    state_views: Vec<Vec<StructuralParameterDeclaration>>,
    state_contract_rosters: &'a [Vec<(u32, StructuralParameterDeclaration)>],
    state_values: Vec<Vec<ValueDeclaration>>,
    /// Each emitted state's erased-formal roster: the machine contract
    /// formals for a plain entry, fresh block-level declarations otherwise.
    state_erased: Vec<Vec<ValueDeclaration>>,
    /// Each emitted state's erased-proof roster in checked order — the same
    /// positions the state's own blocks declare and its edges forward.
    state_erased_proof: Vec<Vec<CheckedErasedProofParameterPlan>>,
    /// Jump and Conditional edge identities emitted so far, keyed by their
    /// target block — the exact roster a header invariant must cite.
    arrival_edges: BTreeMap<BlockId, Vec<EdgeId>>,
    /// Arrival edges whose source transition checking proved the target's
    /// whole `requires` contract for (`CheckFacts::call_requires_decided`).
    decided_arrival_edges: std::collections::BTreeSet<EdgeId>,
    structural_places: Vec<StructuralPlaceDeclaration>,
    blocks: Vec<Block>,
    scalar_block_invariants: Vec<terminal_psi::ScalarBlockInvariant>,
    qualification_coercions: Vec<terminal_psi::ScalarQualificationCoercion>,
    loaned_local_homes: BTreeMap<u32, semantic_vocabulary::PlaceId>,
    /// Exclusive loan chains a later state's local backs.
    loan_chains: loan_chains::LoanChains,
    captured_loans: captured_loans::CapturedLoans,
    occurrences: super::super::ComposedOccurrences,
    block_ranks: BTreeMap<BlockId, ValueId>,
    rank_edges: BTreeMap<EdgeId, (ValueId, terminal_psi::TerminalNaturalRankComparison)>,
}

pub(in crate::unit::attached_unit::composed_control) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        super::super::ComposedOccurrences,
        Vec<terminal_psi::ScalarBlockInvariant>,
        Vec<terminal_psi::ScalarQualificationCoercion>,
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
            },
        })
        .collect::<Vec<_>>();
    let machine_result = returns::result(
        identities,
        checked,
        plan.machine,
        plan.states[0].state,
        &plan.result,
        &plan.result_reference_sources,
        &parameters,
        catalogs,
        &mut structural_places,
    )?;
    let entry = &plan.states[0];
    let mut claims = crate::unit::attached_unit::claims::lower_unit_entry_claims(
        identities,
        plan.machine,
        entry.state,
        &entry.entry_claims,
        &parameters,
        &catalogs.structural_types,
    )?;
    // A fused graph retains states authored by sibling machines. Its namespace
    // reserves only those states' authenticated establishments and completions,
    // under each declaration's owner. Reservation creates no live custody;
    // the actual producer establishes it when the retained state executes.
    let state_owners = plan
        .states
        .iter()
        .map(|state| Ok((state.state, super::owner(checked, state.state)?)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // Every composed state borrows this fixed table. Moved-local rows retain
    // their source identity and resolve its existing binding during emission.
    let mut next_claim = u64::try_from(claims.entry_claims.len())
        .map_err(|_| LoweringError::Unsupported("Unit entry claim count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "Unit claim identities exhaust their namespace",
        ))?;
    // Reserve destination names before expression/call emission borrows the
    // namespace. Reservation is not establishment: each block arrival must
    // bind these names to the claims captured from its outgoing arguments.
    for (position, state) in plan.states.iter().enumerate() {
        if !admitted.live[position] {
            continue;
        }
        for entry in &state.entry_claims {
            if !admitted
                .claim_transport
                .aliases
                .iter()
                .any(|(alias, _)| *alias == entry.claim_identity)
                && !claims
                    .source_claims
                    .iter()
                    .any(|(identity, _)| *identity == entry.claim_identity)
            {
                let claim = claim_id(allocate_dense(&mut next_claim)?);
                claims.source_claims.push((entry.claim_identity, claim));
            }
        }
    }
    for event in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
    {
        let statement_index = match event.source {
            language_semantics::PermissionEventSource::Statement { statement_index }
            | language_semantics::PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            _ => continue,
        };
        let Some((state, machine)) = state_owners
            .iter()
            .find(|(state, _)| *state == event.state_symbol)
        else {
            continue;
        };
        if !crate::unit::attached_unit::operation_frame::claim_established_here(
            *machine,
            *state,
            statement_index,
            event,
        ) || claims
            .source_claims
            .iter()
            .any(|(identity, _)| *identity == event.claim_identity)
        {
            continue;
        }
        let claim = claim_id(allocate_dense(&mut next_claim)?);
        claims.source_claims.push((event.claim_identity, claim));
    }
    // A structural call result establishes the claims it carries when the
    // call completes. A received result keeps the receiving local's identity
    // (or an input's, returned on every exit); a result no place receives is
    // named by its call and path alone. A state's invocation-storage alias
    // handed back as a result keeps its invocation binding, joined below.
    let invocation_alias = |identity: language_semantics::PermissionClaimIdentity| {
        admitted
            .claim_transport
            .aliases
            .iter()
            .any(|(successor, _)| *successor == identity)
    };
    for (state, (_, owner)) in plan.states.iter().zip(&state_owners) {
        // Scalar operands retain structural producers in the computation graph,
        // outside the statement operation list. Reserve both dependency lanes
        // in this exact state's namespace before either lane emits.
        let computed_operations =
            crate::expression_preparation::computation_graph::value_call_operations(
                checked,
                *owner,
                Some(state.state),
            )?;
        for operation in state
            .operation_dependencies()
            .flat_map(checked_trees::CheckedUnitEffectOperationPlan::with_value_calls)
            .chain(computed_operations)
        {
            let Some((call, custody)) = operation
                .call()
                .and_then(|call| Some((call, call.structural_custody()?)))
            else {
                continue;
            };
            for received in &custody.result_claims {
                if received.claim == language_semantics::PermissionClaimIdentity::Unknown {
                    if claims.unreceived_results.iter().any(|claim| {
                        claim.state == state.state
                            && claim.coordinate == call.coordinate
                            && claim.path == received.path
                    }) {
                        continue;
                    }
                    let claim = claim_id(allocate_dense(&mut next_claim)?);
                    claims.unreceived_results.push(
                        crate::emission::claim_bindings::UnreceivedResultClaim {
                            state: state.state,
                            coordinate: call.coordinate,
                            path: received.path.clone(),
                            claim,
                        },
                    );
                } else if !invocation_alias(received.claim)
                    && !claims
                        .source_claims
                        .iter()
                        .any(|(identity, _)| *identity == received.claim)
                {
                    let claim = claim_id(allocate_dense(&mut next_claim)?);
                    claims.source_claims.push((received.claim, claim));
                }
            }
        }
    }
    // Proven invocation storage aliases have no block argument or new name.
    for (successor, root) in &admitted.claim_transport.aliases {
        let claim = lookup_claim_id(&claims.source_claims, *root)?;
        claims.source_claims.push((*successor, claim));
    }
    catalogs
        .scalar_calls
        .bind_claim_namespace(claims.published());
    let content_entry_claims =
        crate::proofs::content_conservation::lower_whole_content_entry_claims(
            identities,
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
        .zip(&admitted.live)
        .filter_map(|(state, live)| live.then_some(state))
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
    let mut state_contract_rosters = Vec::new();
    let mut state_values = Vec::new();
    let mut state_erased = Vec::new();
    let mut state_erased_proof = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        state_ids.push(block_id(allocate_dense(&mut catalogs.next_block)?));
        if !admitted.live[position] {
            // Dead positions keep roster alignment but own no emitted block.
            state_views.push(Vec::new());
            state_contract_rosters.push(Vec::new());
            state_values.push(Vec::new());
            state_erased.push(Vec::new());
            state_erased_proof.push(Vec::new());
            continue;
        }
        if position != 0 || entry_reentered {
            let mut block_parameters = lower_unit_parameters(
                identities,
                &state.structural_parameters,
                &catalogs.type_ids,
                &catalogs.structural_types,
                &catalogs.domain_ids,
                &mut catalogs.next_place,
            )?;
            let mut block_position = 0_u32;
            for (dense, parameter) in block_parameters.iter_mut().enumerate() {
                if let Some(root) = admitted
                    .entry_aliases
                    .get(position)
                    .and_then(|aliased| aliased.get(&(dense as u32)))
                {
                    // A forwarded whole entry parameter keeps the entry
                    // parameter's place; the block does not re-declare it.
                    parameter.place = parameters
                        .get(*root as usize)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph entry alias lost its entry parameter",
                        ))?
                        .place;
                } else {
                    // Exclusive plain storage crosses an edge by address.
                    // Retain write-only access on the arriving name: moving
                    // the loan cannot grant observation of its referent.
                    // Entry aliases are not block parameters. Only
                    // transferred structural values occupy its dense
                    // namespace.
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
                    .filter(|(dense, _)| {
                        !admitted
                            .entry_aliases
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
            // A retained `requires` resolves member operands through the
            // block's structural decls; `seal` drains this roster into the
            // block, so the contract namespace keeps its own copy.
            state_contract_rosters.push(
                state
                    .structural_parameters
                    .iter()
                    .zip(block_parameters.iter())
                    .map(|(source, parameter)| (source.position, parameter.clone()))
                    .collect(),
            );
            state_views.push(block_parameters);
            state_values.push(
                crate::unit::attached_unit::parameters::lower_unit_scalar_parameter_declarations(
                    checked,
                    state.state,
                    &state.scalar_parameters,
                    catalogs
                        .prepared_scalar_qualifications
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph state formals have no qualification namespace",
                        ))?,
                    &mut catalogs.next_value,
                )?,
            );
            state_erased.push(
                crate::scalar_lowering::contracts::erased_formal_declarations(
                    &state.erased_scalar_parameters,
                    &mut catalogs.next_value,
                )?,
            );
            state_erased_proof.push(state.erased_proof_parameters.clone());
        } else {
            state_contract_rosters.push(
                state
                    .structural_parameters
                    .iter()
                    .zip(parameters.iter())
                    .map(|(source, parameter)| (source.position, parameter.clone()))
                    .collect(),
            );
            state_views.push(parameters.clone());
            state_values.push(scalar_parameters.clone());
            // A plain entry shares the machine contract formals; the block
            // does not redeclare them.
            state_erased.push(entry_erased_formals.clone());
            state_erased_proof.push(state.erased_proof_parameters.clone());
        }
    }
    let mut block_entry_claims = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        if !admitted.live[position] || (position == 0 && !entry_reentered) {
            continue;
        }
        let lowered = crate::unit::attached_unit::claims::lower_unit_entry_claims(
            identities,
            state_owners[position].1,
            state.state,
            &state.entry_claims,
            &state_views[position],
            &catalogs.structural_types,
        )?;
        for (source, mut binding) in state.entry_claims.iter().zip(lowered.entry_claims) {
            if admitted.entry_aliases[position].contains_key(&source.parameter_index) {
                continue;
            }
            binding.claim = lookup_claim_id(&claims.source_claims, source.claim_identity)?;
            block_entry_claims.push(binding);
        }
    }
    block_entry_claims.sort_by_key(|binding| binding.claim);
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
            erased_proof_formals:
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    identities,
                    checked,
                    &plan.states[0].erased_proof_parameters,
                ),
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
                // The machine proof roster forwards positionally into the
                // entry block's identical declaration.
                erased_proof_arguments: (0..plan.states[0].erased_proof_parameters.len())
                    .map(|position| semantic_vocabulary::ProofTerm::Formal {
                        position: u32::try_from(position)
                            .expect("erased-proof roster positions fit u32"),
                    })
                    .collect(),
                structural_arguments: parameters
                    .iter()
                    .enumerate()
                    .filter(|(dense, _)| {
                        !admitted
                            .entry_aliases
                            .first()
                            .is_some_and(|aliased| aliased.contains_key(&(*dense as u32)))
                    })
                    .map(|(_, parameter)| StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: parameter.access,
                    })
                    .collect(),
                cleanup_actions: Vec::new(),
            },
        });
    }
    // A lender's stored view keeps its declared access ceiling, while a member
    // parameter names the canonical view of the same pointer and count. The
    // chain carries that one descriptor, so it presents the member when the
    // geometry is unchanged and the ceiling supplies the member's access.
    let presents = |stored: typed_trees::type_identity::TypeIdentityId,
                    member: &checked_trees::CheckedUnitStructuralParameterPlan| {
        if stored == member.type_identity {
            return true;
        }
        let (Ok(stored), Ok(formal)) = (
            lookup_type_id(&catalogs.type_ids, &stored),
            lookup_type_id(&catalogs.type_ids, &member.type_identity),
        ) else {
            return false;
        };
        let requested = crate::expression_preparation::bindings::structural_paths::terminal_access(
            member.access,
        );
        terminal_semantics::borrowed_referent_presentation(
            catalogs.structural_types.iter(),
            stored,
            formal,
        ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            && terminal_psi::find_structural_type(catalogs.structural_types.as_ref(), stored)
                .is_some_and(|declaration| match declaration.shape {
                    terminal_psi::StructuralTypeShape::ElementView { access, .. }
                    | terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                    ) => access.is_none_or(|access| {
                        crate::expression_preparation::bindings::structural_paths::structural_access_can_supply(
                            access, requested,
                        )
                    }),
                    _ => false,
                })
    };
    let loan_chains = loan_chains::LoanChains::plan(
        plan,
        &checked.facts.values.structural_values,
        &admitted.live,
        &admitted.entry_aliases,
        entry_reentered,
        &state_views,
        &presents,
    )?;
    let mut occurrences = super::super::ComposedOccurrences::default();
    occurrences.state_blocks = plan
        .states
        .iter()
        .zip(&state_ids)
        .zip(&admitted.live)
        .filter(|(_, live)| **live)
        .map(|((state, block), _)| lowered_psi::LoweredStateBlock {
            machine: terminal_machine,
            state: state.state,
            block: *block,
        })
        .collect();
    let captured_loans = captured_loans::CapturedLoans::plan(
        plan,
        &checked.facts.values.structural_values,
        &admitted.live,
        &state_views,
        &state_ids,
        &admitted.entry_aliases,
        catalogs,
        &mut structural_places,
    )?;
    let block_ranks = std::collections::BTreeMap::new();
    let rank_edges = std::collections::BTreeMap::new();
    let mut emission = StateGraphEmission {
        checked,
        plan,
        terminal_machine,
        admitted,
        parameters,
        scalar_parameters,
        catalogs,
        machine_result,
        claims,
        content_entry_claims,
        content_identity_reshuffles,
        loan_completions: Vec::new(),
        entry_reentered,
        state_ids,
        state_views,
        state_contract_rosters: &state_contract_rosters,
        state_values,
        state_erased,
        state_erased_proof,
        arrival_edges,
        decided_arrival_edges: std::collections::BTreeSet::new(),
        structural_places,
        blocks,
        loaned_local_homes: BTreeMap::new(),
        loan_chains,
        captured_loans,
        occurrences,
        scalar_block_invariants: Vec::new(),
        qualification_coercions: Vec::new(),
        block_ranks,
        rank_edges,
    };
    for position in 0..plan.states.len() {
        if !emission.admitted.live[position] {
            continue;
        }
        emission.emit_state(identities, position)?;
    }
    let live_states = std::mem::take(&mut emission.admitted.live);
    let StateGraphEmission {
        parameters,
        scalar_parameters,
        catalogs,
        machine_result,
        claims,
        content_entry_claims,
        content_identity_reshuffles,
        loan_completions,
        state_ids,
        state_views: _,
        state_values,
        state_erased,
        state_erased_proof: _,
        arrival_edges,
        decided_arrival_edges,
        mut structural_places,
        mut blocks,
        mut scalar_block_invariants,
        mut qualification_coercions,
        loaned_local_homes,
        loan_chains,
        captured_loans,
        mut occurrences,
        mut block_ranks,
        mut rank_edges,
        ..
    } = emission;
    // A retained `requires` becomes the header's scalar block invariant:
    // every incoming Jump or Conditional edge discharges it before the block
    // runs. A plain entry publishes its clauses on the machine contract
    // instead — invocation is an arrival without an edge proof.
    for (position, state) in plan.states.iter().enumerate() {
        if state.requires.is_empty()
            || (position == 0 && !entry_reentered)
            || !live_states[position]
        {
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
        let incoming_edges = arrival_edges.get(&header).cloned().unwrap_or_default();
        let has_source_expression = state.requires.iter().any(|requirement| {
            matches!(
                requirement,
                checked_trees::CheckedStateContractRequirement::SourceExpression(_)
            )
        });
        // Source checking owns the complete authored contract, including
        // predicates outside this scalar vocabulary. Consume its exact call
        // decision at every actual arrival; never replace an unrepresented
        // requirement with Truth or infer its discharge from its spelling.
        if has_source_expression
            && (incoming_edges.is_empty()
                || incoming_edges
                    .iter()
                    .any(|edge| !decided_arrival_edges.contains(edge)))
        {
            return unsupported("state source requirement lacks a checked arrival decision");
        }
        let projected_requires = state
            .requires
            .iter()
            .filter_map(|requirement| match requirement {
                checked_trees::CheckedStateContractRequirement::Closed(clause) => {
                    Some(Some(clause.clone()))
                }
                checked_trees::CheckedStateContractRequirement::SourceExpression(_) => None,
            })
            .collect::<Vec<_>>();
        // A state's `requires` cites its own parameter positions; the roster
        // pairs them with the terminal places each block view carried at
        // seal (the receiver entry points at the original invocation
        // storage).
        let view_roster = &state_contract_rosters[position];
        let element_views = crate::expression_preparation::bindings::element_views(
            view_roster,
            &catalogs.structural_types,
        );
        let Some(predicate) = crate::scalar_lowering::contracts::clauses(
            identities,
            &projected_requires,
            &state_values[position],
            &state_erased[position],
            &crate::scalar_lowering::contracts::ContractViewNamespace {
                result: crate::scalar_lowering::contracts::ContractStructuralResult::Absent,
                parameters: view_roster,
                structural_types: &catalogs.structural_types,
                element_views: &element_views,
            },
        )?
        else {
            continue;
        };
        // An arrival whose transition checking discharged in full needs no
        // certificate, provided every row is a plain clause the call check
        // judged; a float range rides its own catalog and keeps the search.
        // The invocation-entry arrival has no transition and never qualifies.
        let checked_rows = projected_requires.iter().all(|row| {
            matches!(
                row,
                Some(
                    checked_trees::ClosedScalarContractValue::Predicate(_)
                        | checked_trees::ClosedScalarContractValue::Boolean(_)
                        | checked_trees::ClosedScalarContractValue::Integer(_)
                )
            )
        });
        let mut arrivals = Vec::new();
        for edge in incoming_edges {
            let obligation = obligation_id(allocate_dense(
                &mut catalogs.scalar_calls.next_call_obligation,
            )?);
            if checked_rows && decided_arrival_edges.contains(&edge) {
                occurrences.source_checked_obligations.push(obligation);
            }
            arrivals.push(terminal_psi::ScalarBlockInvariantArrival { edge, obligation });
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
                crate::scalar_lowering::computations::arrays::declarations(&block.operations).chain(
                    crate::scalar_lowering::computations::cases::declarations(&block.operations),
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
    crate::emission::operation_emission::place_declarations::retain_block_parameters(
        blocks.iter(),
        &mut structural_places,
    )?;
    crate::emission::operation_emission::place_declarations::retain_operation_results(
        blocks.iter().flat_map(|block| &block.operations),
        &mut structural_places,
    )?;
    // The computation scan also encounters subslices whose ordinary call
    // or edge already owns a declaration. Reuse only that exact declaration;
    // a reused identity with different provenance remains an error.
    crate::scalar_lowering::computations::subslices::merge_declarations(
        blocks.iter(),
        &mut structural_places,
    )?;
    crate::emission::operation_emission::place_declarations::retain_block_parameters(
        blocks.iter(),
        &mut structural_places,
    )?;
    structural_places.sort_by_key(|place| place.id);
    let attachment = plan
        .attachment_type_identity
        .as_ref()
        .map(|identity| lookup_type_id(&catalogs.type_ids, identity))
        .transpose()?;
    if let Some(attachment) = attachment {
        let declaration =
            terminal_psi::find_structural_type(catalogs.structural_types.as_ref(), attachment)
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
                identities,
                checked,
                attachment,
                declaration,
                &catalogs.structural_types,
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
        declaration: None,
        nominal_cleanup_subject: None,
        parameters: scalar_parameters,
        structural_places,
        structural_parameters: parameters,
        entry_claims: claims.entry_claims,
        block_entry_claims,
        loan_completions,
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
            source_checked_requires: Vec::new(),
            ensures: Vec::new(),
            crash_routes: Vec::new(),
            erased_scalar_formals: entry_erased_formals,
            erased_proof_formals:
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    identities,
                    checked,
                    &plan.states[0].erased_proof_parameters,
                ),
            outcome_specific_ensures: Vec::new(),
        },
    };
    captured_loans.finish(&mut machine, &state_ids)?;
    loaned_local_homes::finish(
        &mut machine,
        &loaned_local_homes,
        &catalogs.structural_types,
    )?;
    loan_chains.finish(&mut machine, &state_ids, &live_states)?;
    let continuation_value_uses = machine
        .blocks
        .iter()
        .flat_map(|block| {
            block_ranks
                .get(&block.id)
                .copied()
                .into_iter()
                .chain(
                    block
                        .terminator
                        .edges()
                        .filter_map(|edge| rank_edges.get(&edge).map(|(rank, _)| *rank)),
                )
                .map(move |value| (block.id, value))
        })
        .collect::<Vec<_>>();
    let mut continuation_maps = crate::emission::continuation_specialization::complete(
        &mut machine,
        &occurrences.selected_cleanup_groups,
        &catalogs.structural_types,
        crate::emission::continuation_specialization::CompanionRows {
            continuation_value_uses: &continuation_value_uses,
            claim_introductions: &mut occurrences.claim_introductions,
            source_calls: &mut occurrences.source_calls,
            scalar_block_invariants: &mut scalar_block_invariants,
            qualification_coercions: &mut qualification_coercions,
            selected_ieee_float_comparisons: &mut occurrences.selected_ieee_float_comparisons,
            selected_integer_comparisons: &mut occurrences.selected_integer_comparisons,
            source_checked_obligations: &mut occurrences.source_checked_obligations,
        },
        crate::emission::continuation_specialization::Counters {
            next_place: &mut catalogs.next_place,
            next_value: &mut catalogs.next_value,
            next_block: &mut catalogs.next_block,
            next_edge: &mut catalogs.next_edge,
            next_operation: &mut catalogs.next_operation,
            next_obligation: &mut catalogs.scalar_calls.next_call_obligation,
        },
    )?;
    if !continuation_maps.is_empty() {
        let original_states = std::mem::take(&mut occurrences.state_blocks);
        let original_ranks = std::mem::take(&mut block_ranks);
        let original_edges = std::mem::take(&mut rank_edges);
        for namespace in &mut continuation_maps {
            let [(original, physical)] = namespace.blocks.as_slice() else {
                return unsupported("selected continuation has no unique owning block");
            };
            let (original, physical) = (*original, *physical);
            for state in original_states
                .iter()
                .filter(|state| state.block == original)
            {
                occurrences
                    .state_blocks
                    .push(lowered_psi::LoweredStateBlock {
                        block: physical,
                        ..*state
                    });
            }
            if let Some(rank) = original_ranks.get(&original) {
                block_ranks.insert(physical, namespace.value(*rank));
            }
            for (original, physical) in namespace.edges.clone() {
                if let Some((rank, comparison)) = original_edges.get(&original) {
                    rank_edges.insert(physical, (namespace.value(*rank), *comparison));
                }
            }
        }
    }
    // Authored entry/local cleanup already belongs to the composed emitter.
    // Complete checked no-code carrier frontiers after private loans and their
    // continuations have reached their final emitted graph. Plain owned type
    // capability is independent of scheduled source death rows; descriptors
    // keep their existing loan-continuation custody.
    crate::emission::affine_cleanup::complete(
        &[],
        &machine.structural_parameters,
        &crate::emission::affine_cleanup::no_code_type_ids(checked, &catalogs.type_ids),
        &machine.structural_places,
        machine.entry,
        &mut machine.blocks,
        &catalogs.structural_types,
    )?;
    // Crash ceilings live in the invocation signature, independent of the
    // state's block-local value roster. Predicate parameters retain authored
    // structural positions and scalar formals retain their dense coordinates.
    // Publishing the graph must use that complete namespace just as ordinary
    // machine emission does.
    let signature =
        crate::unit::attached_unit::signatures::find(&catalogs.signatures, plan.machine)?;
    machine.contract.crash_routes = if signature.predicate_parameters.is_empty() {
        lower_checked_crash_route_buckets(
            identities,
            &catalogs.root_crash_routes,
            &signature.scalar_parameters,
        )?
    } else {
        crate::proofs::crash_routes::lower_structural_crash_route_buckets(
            identities,
            &catalogs.root_crash_routes,
            &signature.scalar_parameters,
            &signature.predicate_parameters,
            &catalogs.structural_types,
            &signature.runtime_requirements,
        )?
    };
    ranking::retain(&mut machine, &block_ranks, &rank_edges)?;
    Ok((
        machine,
        occurrences,
        scalar_block_invariants,
        qualification_coercions,
    ))
}
