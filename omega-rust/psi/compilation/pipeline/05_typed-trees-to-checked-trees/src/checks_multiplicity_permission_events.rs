/*
    Recording permission events: crash frontiers, proven case membership
    and the statically inactive call results that shape which events count.
*/

use super::claim_outcomes::ClaimOutcomePreparation;
use super::claim_outcomes::receiving_value;
use super::cleanup_dispositions::RemainingCustody;
use super::cleanup_dispositions::StorageEstablishment;
use super::instruction_dispositions::InstructionDispositions;
use super::linear_obligations::prepare_storage_deaths;
use super::linear_validation::completed_statement_values;
use super::linear_validation::event_statement_index;
use super::linear_validation::loan_completions::regions_for_calls;
use super::permission_schedule::PermissionStep;
use super::permission_schedule::SelectedCases;
use super::permission_schedule::exclude_case;
use super::selected_cleanup::SelectedCleanup;
use crate::checks::multiplicity::claim_outcomes::{
    apply_claim_origin_rewrites, call_result_origin_rewrites, derive_checked_claim_outcome_maps,
    publish_call_claim_joins, publish_claim_outcome_maps,
};
use crate::checks::multiplicity::linear_obligations::{
    CheckedClaimOutcomeMap, ClaimIdentityAllocator, LinearPlace,
};
use crate::checks::multiplicity::linear_validation::{
    append_borrow_permission_events, append_owned_cleanup_permission_events,
    apply_recorded_state_entry_events, apply_recorded_statement_events,
    complete_statement_permissions, evaluate_statement_permissions, initial_linear_places,
    reserve_owner_places,
};
use crate::checks::multiplicity::owned_selection;
use crate::checks::multiplicity::temporary_results;
use crate::published::places::place_at;
use checked_trees::{
    CheckFacts, CheckFactsWalk, ContextualCleanupFacts, FlowPermissionEventFact as EventFact,
};
use diagnostics::Diagnostic;
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity as ClaimIdentity, PermissionEventKind,
    PermissionEventSource,
};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

/*
    The `Owned` permission events recorded for one `(machine, state)` flow, in
    emission order — the roster both the linear replay and the entry-claim
    walk replay against the same published rules.
*/
pub(crate) fn owned_permission_events(
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Vec<&EventFact> {
    let handles = facts
        .flow
        .ownership
        .with_state_permissions(state_symbol, |handles| handles.to_vec());
    handles
        .into_iter()
        .filter_map(|handle| {
            let event = facts.flow.ownership.permissions.get(handle);
            (event.machine_symbol == machine_symbol
                && event.state_symbol == state_symbol
                && event.access == PermissionAccess::Owned)
                .then_some(event)
        })
        .collect()
}

/*
    Prepare source custody without inspecting derived callable contracts. Crash
    frontier projection belongs after those contracts have been constructed.
*/
pub(crate) fn prepare_permission_events(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    claim_outcomes: &mut ClaimOutcomePreparation,
    mutation_summaries: &MutationSummaries,
    shared_call_frames: Option<&validation::CallFrameResolver<'_>>,
    memory: &mut PermissionWalkMemory,
    inputs: WalkInputs,
) -> Result<bool, Vec<Diagnostic>> {
    claim_outcomes.begin();
    facts.qualifications.content.body_conservations = Default::default();
    facts.qualifications.content.body_conservation_plans.clear();
    facts.qualifications.content.body_claim_bindings = Default::default();
    facts.qualifications.content.body_available_inputs = Default::default();
    facts.qualifications.content.body_transformations = Default::default();
    facts.qualifications.content.body_derivations = Default::default();
    facts
        .qualifications
        .content
        .boundary_requirement_plans
        .clear();
    facts.qualifications.content.boundary_body_requirements = Default::default();
    facts.qualifications.content.backing_addresses = Default::default();
    facts.qualifications.content.claim_transformations = Default::default();
    facts.flow.ownership.instruction_dispositions = Default::default();
    facts.flow.ownership.instruction_effect_receipts = Default::default();
    facts.flow.ownership.storage_transfers = Default::default();
    facts.flow.ownership.selected_cleanup_permissions = Default::default();
    facts.flow.ownership.owned_selections = Default::default();
    facts.flow.ownership.selection_sources = Default::default();
    facts.flow.ownership.selection_transfers = Default::default();
    facts.flow.ownership.selection_transfer_claims = Default::default();
    facts.flow.ownership.selection_place_bindings = Default::default();
    facts.flow.ownership.loan_completions = Default::default();
    let mut owned_frames = None;
    let call_frames =
        crate::flow::shared_call_frames_or(shared_call_frames, program, &mut owned_frames);

    let state_flows = facts
        .flow
        .control
        .states
        .iter()
        .map(|(handle, state)| (handle, state.clone()))
        .collect::<Vec<_>>();
    let scratch = match walk_states_apart(
        program,
        facts,
        claim_outcomes,
        mutation_summaries,
        call_frames,
        &state_flows,
        memory,
        inputs,
    ) {
        Some(scratch) => scratch,
        None => walk_states_serially(
            program,
            facts,
            claim_outcomes,
            mutation_summaries,
            call_frames,
            &state_flows,
        ),
    };
    let PermissionWalkScratch {
        mut permission_events,
        conditional_carrier_transfers,
        selection_diagnostics,
        mut claim_identities,
        mut cleanup_dispositions,
        selector_lookup: _,
    } = scratch;

    claim_identities.finish_walk();
    {
        append_borrow_permission_events(
            program,
            &mut facts.walk(),
            &mut permission_events,
            &mut claim_identities,
        );
    };
    let claim_outcome_maps = reconcile_state_call_result_origins(
        program,
        &facts.borrow,
        &mut facts.flow.ownership,
        &mut permission_events,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;

    let mut event_index = 0;
    permission_events.retain(|event| {
        let absent_carrier_transfer = conditional_carrier_transfers
            .binary_search(&event_index)
            .is_ok()
            && !event.obligation_live
            && event.claim_identity == ClaimIdentity::Unknown;
        event_index += 1;
        !absent_carrier_transfer
    });

    facts.flow.ownership.permissions_by_state.clear();
    facts.flow.ownership.permissions = Default::default();
    facts
        .flow
        .ownership
        .permissions
        .insert_many(permission_events);
    publish_claim_outcome_maps(&mut facts.walk(), claim_outcome_maps);
    let joined_receivers = publish_call_claim_joins(program, &mut facts.walk())
        .map_err(|message| vec![Diagnostic::error(message)])?;
    let joined_events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event.clone())
        .collect::<Vec<_>>();
    let joined_maps = derive_checked_claim_outcome_maps(
        program,
        &facts.borrow,
        &facts.flow.ownership,
        &joined_events,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    let correspondence_changed = claim_outcomes
        .update(joined_maps.clone(), joined_receivers)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    publish_claim_outcome_maps(&mut facts.walk(), joined_maps);
    if correspondence_changed {
        return Ok(true);
    }
    if selection_diagnostics.is_empty() {
        cleanup_dispositions.entry_requirements =
            std::mem::take(&mut facts.contextual_cleanup.entry_requirements);
        facts.contextual_cleanup = cleanup_dispositions;
        prepare_storage_deaths(program, facts)?;
        let mut cleanup_dispositions = std::mem::take(&mut facts.contextual_cleanup);
        {
            crate::published::facts::contextual_cleanup::bind_applications(
                program,
                &facts.flow,
                &mut cleanup_dispositions,
            );
        };
        facts.contextual_cleanup = cleanup_dispositions;
        Ok(false)
    } else {
        Err(selection_diagnostics)
    }
}

/*
    What the per-state walk accumulates beside the facts, in roster order:
    permission events (and the positions of conditional carrier transfers
    among them), minted claim identities, cleanup dispositions and selection
    diagnostics. The selector lookup is a lazily built read-only index.
*/
#[derive(Default)]
struct PermissionWalkScratch<'program> {
    permission_events: Vec<EventFact>,
    conditional_carrier_transfers: Vec<usize>,
    selection_diagnostics: Vec<Diagnostic>,
    claim_identities: ClaimIdentityAllocator,
    cleanup_dispositions: ContextualCleanupFacts,
    selector_lookup: Option<validation::ImmutableBoundLookup<'program>>,
}

/*
    The flow pass's settled per-state mutation summaries, which every
    state's walk reads and none writes.
*/
type MutationSummaries = crate::flow::StateMutationSummaryCache;

/* One flow state row, by handle, in roster order. */
type StateFlow = (
    arena::Handle<checked_trees::FlowStateFact>,
    checked_trees::FlowStateFact,
);

/*
    Prepare every state's walk on the checking pool (`prepare_state_walks`),
    with call writes read from the facts as they stand.
*/
fn prepare_walks<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flows: &[StateFlow],
    mutation_summaries: &MutationSummaries,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> Vec<PreparedStateWalk> {
    let mutated_places = |machine: &typed_trees::machine::Machine,
                          state: &typed_trees::state::State,
                          call: &checked_trees::BorrowCallFact| {
        crate::flow::call_mutated_places(
            program,
            machine.symbol,
            state.symbol,
            &facts.borrow,
            call,
            mutation_summaries,
            call_frames,
        )
    };
    prepare_state_walks(program, facts, state_flows, &mutated_places)
}

/*
    Walk every state in roster order on `facts` itself. Each state's control
    row takes its storage-transfer span right after that state's walk.

    Each state's walk is prepared just before it is walked, not all at once:
    every prepared walk holds its case exclusions and conservation answers, and
    holding all of them for a large program (the psi product package's 2,135
    states, about 2.7 GB) pushed `omega update` past a 15 GB host. Preparing
    late reads the same facts. A walk writes only `flow.ownership`, the
    `qualifications.content` body tables (`CheckFactsWalk` hands out nothing
    else mutably) and, after it, its own state row's `storage_transfers`.
    Preparation reads none of those: it reads the control rows' calls,
    statements, operands and retirements, the flow contexts, `borrow`,
    `operators` and the content plans and conservation plans, which no walk
    changes. Each prepared walk depends only on its own state, so preparing a
    one-state slice gives the same answer as preparing the whole roster.
*/
fn walk_states_serially<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    claim_outcomes: &mut ClaimOutcomePreparation,
    mutation_summaries: &MutationSummaries,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    state_flows: &[StateFlow],
) -> PermissionWalkScratch<'program> {
    let mut scratch = PermissionWalkScratch::default();
    for (position, state_entry) in state_flows.iter().enumerate() {
        let (state_handle, state_flow) = state_entry;
        let prepared = prepare_walks(
            program,
            facts,
            std::slice::from_ref(state_entry),
            mutation_summaries,
            call_frames,
        )
        .pop()
        .expect("one prepared walk per state");
        scratch.claim_identities.begin_state(position);
        let Some(storage_transfers) = walk_state(
            program,
            &mut facts.walk(),
            &mut scratch,
            claim_outcomes,
            mutation_summaries,
            call_frames,
            state_flow,
            &prepared,
        ) else {
            continue;
        };
        facts.flow.control.states.update_row(*state_handle, |row| {
            row.storage_transfers = storage_transfers
        });
    }
    scratch
}

/*
    Below this many states the walk stays serial: splitting it costs one
    ownership fork per worker and a merge.
*/
const MIN_STATES_APART: usize = 64;

/* One state's walk, done apart from every other state. */
#[derive(Clone)]
struct StateWalkedApart {
    ownership_pages: checked_trees::FlowOwnershipFacts,
    content: checked_trees::ContentWalkRows,
    storage_transfers: Option<arena::HandleSpan<checked_trees::FlowStorageTransferFact>>,
    permission_events: Vec<EventFact>,
    conditional_carrier_transfers: Vec<usize>,
    selection_diagnostics: Vec<Diagnostic>,
    claim_identities: ClaimIdentityAllocator,
    cleanup_dispositions: ContextualCleanupFacts,
    consulted: ClaimOutcomePreparation,
    crossed: bool,
}

/*
    Each state's walk from the previous call of `prepare_permission_events`
    within one claim-outcome fixed point, so that the next call can skip the
    states whose walk would come out the same.

    The product program walks every state twice: the first call learns the
    claim-outcome maps, and the second walks again with them. Usually only a
    few states consulted a summary that changed (2 of 555 on the one-line
    program). Every other state's second walk reads exactly what its first
    read, and a walk is a pure function of what it reads, so its result is the
    same.

    That is GUARDED, not assumed. A state reuses its previous walk only when
    every input of that walk is shown unchanged:
    - the flow and semantic facts, and the borrow, proof, domain,
      scalar-expression and operator facts flow reads, by
      `WalkInputs::restored`: the caller restored the flow construction the
      previous call walked, after checking those inputs equal, and nothing but
      that call's own permission preparation ran in between. That preparation
      writes only the ownership tables and content rows the walk resets and
      rebuilds (it holds a `CheckFactsWalk`), and the restore replaces the flow;
    - the roster, page range, contextual cleanup, termination facts and content
      plans, by equality with the previous call's;
    - every claim-outcome map and join binding the state looked up, by
      equality (`ClaimOutcomePreparation::reads_unchanged`).

    A state that fails the last check is prepared and walked again from
    scratch: preparing reads only the facts the first two checks cover, never
    a claim-outcome summary, so the new preparation equals the old one. No
    preparation is kept between calls; holding one per state for the psi gate
    batch cost gigabytes. When the first two checks fail, every state is
    prepared and walked afresh. Nothing is spliced into a fresh walk.
*/
#[derive(Default)]
pub(crate) struct PermissionWalkMemory {
    previous: Option<PreviousWalk>,
}

struct PreviousWalk {
    state_flows: Vec<StateFlow>,
    first_page: u32,
    contextual_cleanup: ContextualCleanupFacts,
    termination: checked_trees::TerminationFacts,
    plans: Vec<language_semantics::content::ContentProjectionPlan>,
    conservation_plans: Vec<language_semantics::content::ContentConservationPlan>,
    /* The claim-outcome maps and join bindings the walks read. */
    summaries: ClaimOutcomePreparation,
    states: Vec<StateWalkedApart>,
}

/*
    Whether the facts a walk reads are the ones the previous call walked; see
    `PermissionWalkMemory`.
*/
#[derive(Clone, Copy)]
pub(crate) struct WalkInputs {
    pub(crate) restored: bool,
}

impl PermissionWalkMemory {
    /*
        Each state's previous walk, by roster position, where it is still
        exactly what walking the state now would produce; `None` elsewhere.
    */
    fn take_reusable(
        &mut self,
        inputs: WalkInputs,
        facts: &CheckFacts,
        state_flows: &[StateFlow],
        first_page: u32,
        claim_outcomes: &ClaimOutcomePreparation,
    ) -> Vec<Option<StateWalkedApart>> {
        let previous = self.previous.take();
        let Some(previous) = previous.filter(|previous| {
            inputs.restored
                && previous.first_page == first_page
                && previous.state_flows == state_flows
                && previous.contextual_cleanup == facts.contextual_cleanup
                && previous.termination == facts.termination
                && previous.plans == facts.qualifications.content.plans
                && previous.conservation_plans == facts.qualifications.content.conservation_plans
        }) else {
            return vec![None; state_flows.len()];
        };
        let PreviousWalk {
            summaries, states, ..
        } = previous;
        states
            .into_iter()
            .map(|state| {
                claim_outcomes
                    .reads_unchanged(&summaries, &state.consulted)
                    .then_some(state)
            })
            .collect()
    }
}

/*
    What one checking-pool worker keeps across the states it walks: its own
    copy of the ownership tables (sharing every page the facts already hold),
    a copy of the claim-outcome summaries, and its selector lookup.
*/
struct WalkWorker<'program> {
    ownership: checked_trees::FlowOwnershipFacts,
    claim_outcomes: ClaimOutcomePreparation,
    selector_lookup: Option<validation::ImmutableBoundLookup<'program>>,
}

/*
    Walk every state apart from the others on the checking pool, then merge
    the results in roster order into exactly what `walk_states_serially`
    would have left, without copying `CheckFacts`.

    One state's walk reads only the facts as they stand before the walk, the
    previous iteration's claim-outcome summaries, and the rows that state
    itself writes. So each state can run on its own worker:
    - its ownership rows go on a fresh page whose generation is fixed by the
      state's roster position, so adopting the pages keeps every handle and,
      because pages iterate in generation order, the serial row order;
    - its content rows and cleanup dispositions are built from empty and
      appended in roster order with their handles shifted;
    - its claim ordinals depend only on its roster position
      (`ClaimIdentityAllocator::begin_state`);
    - its storage-transfer span reaches its control row at the merge, which
      is equivalent because no state reads another state's row.

    That last premise, and every other per-state read, is GUARDED rather than
    assumed: each worker's ownership tables carry a `WalkGuard` naming the one
    state being walked, and every per-state lookup the walk can reach notes the
    state it reads. If any read asked for another state, a shifted append is
    refused, or the facts gained a page in the workers' range, this returns
    `None` with `facts` as it was, and the caller walks serially. It also
    returns `None`, before walking anything, for a call too small to split.

    States whose previous walk is still exact (`PermissionWalkMemory`) take
    that walk instead of walking again; the rest are prepared and walked from
    scratch, each on its worker just before its walk.
*/
#[allow(clippy::too_many_arguments)]
fn walk_states_apart<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    claim_outcomes: &mut ClaimOutcomePreparation,
    mutation_summaries: &MutationSummaries,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    state_flows: &[StateFlow],
    memory: &mut PermissionWalkMemory,
    inputs: WalkInputs,
) -> Option<PermissionWalkScratch<'program>> {
    use rayon::prelude::*;
    let pool = validation::checking_thread_pool();
    if state_flows.len() < MIN_STATES_APART || pool.current_num_threads() < 2 {
        memory.previous = None;
        return None;
    }
    let first_page = facts
        .flow
        .ownership
        .latest_page_generation()
        .checked_add(1)
        .filter(|first| {
            u32::try_from(state_flows.len())
                .ok()
                .and_then(|states| first.checked_add(states))
                .is_some()
        });
    let Some(first_page) = first_page else {
        memory.previous = None;
        return None;
    };
    let page_of = |position: usize| {
        first_page + u32::try_from(position).expect("state position fits the page range")
    };
    let mut walked = memory.take_reusable(inputs, facts, state_flows, first_page, claim_outcomes);
    let positions = walked
        .iter()
        .enumerate()
        .filter_map(|(position, state)| state.is_none().then_some(position))
        .collect::<Vec<_>>();
    facts.flow.ownership.share_pages();
    let shared: &CheckFacts = facts;
    let summaries: &ClaimOutcomePreparation = claim_outcomes;
    let rewalked = pool.install(|| {
        positions
            .par_iter()
            .map_init(
                || WalkWorker {
                    ownership: shared.flow.ownership.clone(),
                    claim_outcomes: summaries.fork_for_walk(),
                    selector_lookup: None,
                },
                |worker, &position| {
                    // Prepared here, not for the whole roster up front, so at
                    // most one preparation per worker is alive; see
                    // `walk_states_serially` for why late preparation reads
                    // the same facts.
                    let prepared = prepare_walks(
                        program,
                        shared,
                        std::slice::from_ref(&state_flows[position]),
                        mutation_summaries,
                        call_frames,
                    )
                    .pop()
                    .expect("one prepared walk per state");
                    walk_state_apart(
                        program,
                        shared,
                        worker,
                        mutation_summaries,
                        call_frames,
                        position,
                        page_of(position),
                        &state_flows[position].1,
                        &prepared,
                    )
                },
            )
            .collect::<Vec<_>>()
    });
    if rewalked.iter().any(|state| state.crossed) {
        memory.previous = None;
        return None;
    }
    for (position, state) in positions.into_iter().zip(rewalked) {
        walked[position] = Some(state);
    }
    let mut walked = walked
        .into_iter()
        .map(|state| state.expect("every state is walked or reused"))
        .collect::<Vec<_>>();

    let mut content = checked_trees::ContentProjectionFacts::default();
    let mut cleanup_dispositions = ContextualCleanupFacts::default();
    for state in &walked {
        if !content.append_walk_rows(state.content.clone())
            || !cleanup_dispositions.append_shifted(state.cleanup_dispositions.clone())
        {
            memory.previous = None;
            return None;
        }
    }
    if facts.flow.ownership.latest_page_generation() >= first_page
        || !facts
            .qualifications
            .content
            .append_walk_rows(content.take_walk_rows())
    {
        memory.previous = None;
        return None;
    }

    for state in &mut walked {
        state.ownership_pages.share_pages();
    }
    memory.previous = Some(PreviousWalk {
        state_flows: state_flows.to_vec(),
        first_page,
        contextual_cleanup: facts.contextual_cleanup.clone(),
        termination: facts.termination.clone(),
        plans: facts.qualifications.content.plans.clone(),
        conservation_plans: facts.qualifications.content.conservation_plans.clone(),
        summaries: claim_outcomes.fork_for_walk(),
        states: walked.clone(),
    });

    let mut scratch = PermissionWalkScratch {
        cleanup_dispositions,
        ..PermissionWalkScratch::default()
    };
    for ((state_handle, _), state) in state_flows.iter().zip(walked) {
        let StateWalkedApart {
            ownership_pages,
            content: _,
            storage_transfers,
            permission_events,
            conditional_carrier_transfers,
            selection_diagnostics,
            claim_identities,
            cleanup_dispositions: _,
            consulted,
            crossed: _,
        } = state;
        facts.flow.ownership.adopt_additional_pages(ownership_pages);
        if let Some(transfers) = storage_transfers {
            facts
                .flow
                .control
                .states
                .update_row(*state_handle, |row| row.storage_transfers = transfers);
        }
        let event_offset = scratch.permission_events.len();
        scratch.permission_events.extend(permission_events);
        scratch.conditional_carrier_transfers.extend(
            conditional_carrier_transfers
                .into_iter()
                .map(|position| position + event_offset),
        );
        scratch.selection_diagnostics.extend(selection_diagnostics);
        scratch.claim_identities.absorb(claim_identities);
        claim_outcomes.absorb_walk(consulted);
    }
    Some(scratch)
}

/*
    Walk the state at roster `position` on `worker`'s ownership tables,
    writing its ownership rows on page `page` and everything else into its own
    output. The worker's tables equal the facts' again afterwards.
*/
#[allow(clippy::too_many_arguments)]
fn walk_state_apart<'program>(
    program: &'program dyn ::validation::CheckingInput,
    shared: &CheckFacts,
    worker: &mut WalkWorker<'program>,
    mutation_summaries: &MutationSummaries,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    position: usize,
    page: u32,
    state_flow: &checked_trees::FlowStateFact,
    prepared: &PreparedStateWalk,
) -> StateWalkedApart {
    worker.ownership.begin_page(page);
    worker.ownership.walk_guard =
        checked_trees::WalkGuard::for_states(vec![state_flow.state_symbol]);
    let mut content = checked_trees::ContentWalkRows::default();
    let mut scratch = PermissionWalkScratch {
        selector_lookup: worker.selector_lookup.take(),
        ..PermissionWalkScratch::default()
    };
    scratch.claim_identities.begin_state(position);
    let storage_transfers = {
        let shared_flow = &shared.flow;
        let shared_content = &shared.qualifications.content;
        let mut facts = CheckFactsWalk {
            semantic: &shared.semantic,
            borrow: &shared.borrow,
            contextual_cleanup: &shared.contextual_cleanup,
            proof: &shared.proof,
            operators: &shared.operators,
            flow: checked_trees::FlowFactsWalk {
                contexts: &shared_flow.contexts,
                invalidations: &shared_flow.invalidations,
                borrow_lifetimes: &shared_flow.borrow_lifetimes,
                ownership: &mut worker.ownership,
                control: &shared_flow.control,
            },
            termination: &shared.termination,
            qualifications: checked_trees::QualificationFactsWalk {
                content: checked_trees::ContentProjectionFactsWalk {
                    plans: &shared_content.plans,
                    conservation_plans: &shared_content.conservation_plans,
                    body_conservations: &mut content.body_conservations,
                    body_conservation_plans: &mut content.body_conservation_plans,
                    body_claim_bindings: &mut content.body_claim_bindings,
                    body_available_inputs: &mut content.body_available_inputs,
                    body_transformations: &mut content.body_transformations,
                    body_derivations: &mut content.body_derivations,
                    boundary_requirement_plans: &mut content.boundary_requirement_plans,
                    boundary_body_requirements: &mut content.boundary_body_requirements,
                    backing_addresses: &mut content.backing_addresses,
                    claim_transformations: &mut content.claim_transformations,
                },
            },
        };
        walk_state(
            program,
            &mut facts,
            &mut scratch,
            &mut worker.claim_outcomes,
            mutation_summaries,
            call_frames,
            state_flow,
            prepared,
        )
    };
    let crossed = worker.ownership.walk_guard.crossed();
    worker.ownership.walk_guard = checked_trees::WalkGuard::default();
    let ownership_pages = worker
        .ownership
        .take_pages_from(page, &shared.flow.ownership);
    let PermissionWalkScratch {
        permission_events,
        conditional_carrier_transfers,
        selection_diagnostics,
        claim_identities,
        cleanup_dispositions,
        selector_lookup,
    } = scratch;
    worker.selector_lookup = selector_lookup;
    StateWalkedApart {
        ownership_pages,
        content,
        storage_transfers,
        permission_events,
        conditional_carrier_transfers,
        selection_diagnostics,
        claim_identities,
        cleanup_dispositions,
        consulted: worker.claim_outcomes.take_consulted(),
        crossed,
    }
}

/*
    Walk one state's permission schedule. Its ownership and content rows go
    to `facts`, everything else to `scratch`; the result is the state's
    storage-transfer span, which the caller applies to the state's control
    row, or `None` for a state the walk skips.
*/
#[allow(clippy::too_many_arguments)]
fn walk_state<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &mut CheckFactsWalk<'_>,
    scratch: &mut PermissionWalkScratch<'program>,
    claim_outcomes: &mut ClaimOutcomePreparation,
    mutation_summaries: &MutationSummaries,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    state_flow: &checked_trees::FlowStateFact,
    prepared: &PreparedStateWalk,
) -> Option<arena::HandleSpan<checked_trees::FlowStorageTransferFact>> {
    let PermissionWalkScratch {
        permission_events,
        conditional_carrier_transfers,
        selection_diagnostics,
        claim_identities,
        cleanup_dispositions,
        selector_lookup,
    } = scratch;
    let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
    let state = program.state_by_symbol(state_flow.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let remaining = RemainingCustody::for_state(program, state);
    let mut places = initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);

    for place in places.iter_mut().filter(|place| {
        place.ever_established && (place.multiplicity == Multiplicity::Linear || place.conditional)
    }) {
        let claim_identity = claim_identities.mint(
            state_flow.machine_symbol,
            state.symbol,
            PermissionEventSource::StateEntry,
        );
        place.claim_identity = Some(claim_identity);
        let segments = facts
            .flow
            .ownership
            .segments
            .insert_many(place.path.iter().copied());
        permission_events.push(EventFact {
            expression: Default::default(),
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol: state_flow.machine_symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::StateEntry,
            kind: PermissionEventKind::Establish,
            multiplicity: place.multiplicity,
            access: PermissionAccess::Owned,
            claim_identity,
            provenance: place.provenance.expect("entry place has provenance"),
            root: place.root,
            segments,
            obligation_live: true,
        });
    }

    if machine.has_bodyless_signature() {
        return None;
    }

    let mut moves = crate::flow::discover_state_move_events(
        program,
        facts.borrow,
        facts.operators,
        machine,
        state,
        &mut facts.flow.ownership.segments,
    );
    for event in &mut moves {
        let original = facts
            .flow
            .ownership
            .segments
            .span_or_empty(event.segments)
            .to_vec();
        let statement_index = event_statement_index(event.source)
            .expect("every discovered move belongs to a statement");
        if !original
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
            || !super::transfers_owned_storage(
                program,
                state.symbol,
                statement_index,
                event,
                &original,
            )
        {
            continue;
        }
        let lookup =
            selector_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
        let mut selected = original.clone();
        for segment in &mut selected {
            let facts::PlaceSegment::Index { expression } = *segment else {
                continue;
            };
            if let Some(index) = crate::checks::captured_storage_index(
                program,
                crate::checks::StorageSelectorFacts {
                    flow: facts.flow.view(),
                    semantic: facts.semantic,
                },
                lookup,
                expression,
            ) {
                *segment = facts::PlaceSegment::FixedIndex { index };
            }
        }
        if selected != original {
            event.segments = facts.flow.ownership.segments.insert_many(selected);
        }
    }
    let calls = crate::flow::call_flows(facts.flow.control, state_flow.calls).to_vec();
    let completion_regions = if !places
        .iter()
        .any(|place| place.multiplicity == Multiplicity::Linear)
    {
        Vec::new()
    } else {
        match regions_for_calls(
            program,
            facts.view(),
            machine.symbol,
            state.symbol,
            &calls,
            call_frames,
            mutation_summaries,
        ) {
            Ok(regions) => regions,
            Err(diagnostic) => {
                selection_diagnostics.push(diagnostic);
                return None;
            }
        }
    };
    let call_writes = &prepared.call_writes;
    let temporary_owners = match &prepared.temporary_owners {
        Ok(owners) => owners,
        Err(diagnostic) => {
            selection_diagnostics.push(diagnostic.clone());
            return None;
        }
    };
    let mut completed_temporaries_by_statement = Vec::with_capacity(statements.len());
    let mut observed_receivers_by_statement = Vec::with_capacity(statements.len());
    for ((statement_index, statement), temporary_occurrences) in statements
        .iter()
        .enumerate()
        .zip(&prepared.temporary_occurrences)
    {
        for &(expression, reference) in &temporary_occurrences.completed {
            reserve_owner_places(
                program,
                facts::PlaceRoot::Expression(expression),
                reference,
                &mut places,
            );
        }
        completed_temporaries_by_statement.push(temporary_occurrences.completed.as_slice());
        observed_receivers_by_statement.push(temporary_occurrences.observed.clone());
        for (expression, _) in completed_statement_values(program, statement) {
            let Some(place) = place_at(program, state.symbol, statement_index, expression) else {
                continue;
            };
            if place.root != facts::PlaceRoot::Expression(expression) {
                continue;
            }
            if let Some(reference) = validation::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &place,
            ) {
                reserve_owner_places(program, place.root, reference, &mut places);
            }
        }
    }
    for owner in temporary_owners {
        reserve_owner_places(
            program,
            facts::PlaceRoot::Expression(owner.expression),
            owner.type_reference,
            &mut places,
        );
    }
    if moves
        .iter()
        .any(|event| event.evaluation == checked_trees::FlowPermissionEvaluation::Unknown)
    {
        selection_diagnostics.push(Diagnostic::error(
            "ownership occurrence has no exact source evaluation region",
        ));
        return None;
    }
    let state_needs_case_exclusions = !places.is_empty()
        || !moves.is_empty()
        || !temporary_owners.is_empty()
        || statements.iter().any(|statement| {
            statement.as_local_data().is_some_and(|local| {
                !local.relevance.is_erased()
                    && validation::reference_result_custody::result_multiplicity(
                        program,
                        local.type_reference,
                    ) != Multiplicity::Unrestricted
            })
        })
        || program.state_parameters(state).iter().any(|parameter| {
            !parameter.relevance.is_erased()
                && validation::reference_result_custody::result_multiplicity(
                    program,
                    parameter.type_reference,
                ) != Multiplicity::Unrestricted
        });
    let mut visit = 0;
    super::permission_schedule::walk(
        program,
        state,
        &calls,
        (
            places,
            remaining,
            owned_selection::SelectionOccurrences::default(),
            SelectedCleanup::default(),
            InstructionDispositions::default(),
        ),
        |step, custody| {
            let (places, remaining, selections, selected_cleanup, instruction_dispositions) =
                custody;
            let excluded = &prepared.case_exclusions[visit];
            visit += 1;
            if state_needs_case_exclusions && let Some(excluded) = excluded {
                for case in excluded.iter() {
                    exclude_case(case, places, Some(&mut *remaining));
                }
            }
            match step {
                PermissionStep::Evaluate {
                    statement: statement_index,
                    evaluation,
                } => {
                    if !matches!(
                        evaluation,
                        checked_trees::FlowPermissionEvaluation::TransitionTarget(_)
                    ) && let Err(diagnostic) = selections.check_statement(
                        program,
                        facts.view(),
                        machine,
                        state,
                        statement_index,
                        &statements[statement_index],
                    ) {
                        selection_diagnostics.push(diagnostic);
                        return;
                    }
                    let capture = match owned_selection::SelectionProduction::for_statement(
                        program,
                        facts.view(),
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        evaluation,
                        places,
                    ) {
                        Ok(capture) => capture,
                        Err(diagnostic) => {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    let occurrences =
                        crate::checks::ownership_occurrences::statement_steps_at_evaluation(
                            program,
                            machine,
                            state,
                            statement_index,
                            &statements[statement_index],
                            &moves,
                            facts.operators,
                            Some(evaluation),
                        );
                    let selected = occurrences
                        .iter()
                        .filter_map(|occurrence| match occurrence.step {
                            crate::checks::ownership_occurrences::StatementStep::Move {
                                event,
                                conditional: true,
                                ordered: true,
                            } if moves[event].evaluation == evaluation => {
                                Some(moves[event].expression)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let event_start = permission_events.len();
                    remaining.reserve(program, &statements[statement_index]);
                    let (evaluated, routes) = match evaluate_statement_permissions(
                        program,
                        facts,
                        state_flow.machine_symbol,
                        state.symbol,
                        &moves,
                        &occurrences,
                        completed_temporaries_by_statement[statement_index],
                        &calls,
                        &completion_regions,
                        call_writes,
                        temporary_owners,
                        statement_index,
                        &statements[statement_index],
                        evaluation,
                        capture.as_ref(),
                        places,
                        remaining,
                        &mut *permission_events,
                        &mut *claim_identities,
                        &mut *conditional_carrier_transfers,
                        selections,
                        &mut *cleanup_dispositions,
                        claim_outcomes,
                        &prepared.conservation_candidates,
                    ) {
                        Ok(Some(evaluated)) => evaluated,
                        Ok(None) => return,
                        Err(diagnostic) => {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    if let Some(capture) = &capture {
                        if let Err(diagnostic) = capture.record(
                            program,
                            facts,
                            machine.symbol,
                            state.symbol,
                            statement_index,
                            &evaluated.transfers,
                            &evaluated.observations,
                            &routes,
                            &calls,
                            &permission_events[event_start..],
                            call_frames,
                        ) {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                        if let Some((handle, _receipt)) =
                            facts.flow.ownership.owned_selection_at_evaluation(
                                state.symbol,
                                statement_index as u32,
                                evaluation,
                            )
                            && let Err(diagnostic) = selected_cleanup.observe_selection(
                                program,
                                handle,
                                facts.flow.ownership,
                                &evaluated.transfers,
                                &evaluated.observations,
                                &routes,
                                &calls,
                                &permission_events[event_start..],
                                &occurrences,
                            )
                        {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                    }
                    let produced_storage = match complete_statement_permissions(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        &statements[statement_index],
                        evaluation,
                        evaluated,
                        places,
                        remaining,
                        &mut *permission_events,
                        &mut *claim_identities,
                        &mut *conditional_carrier_transfers,
                        selections,
                        &mut *cleanup_dispositions,
                        claim_outcomes,
                        &prepared.conservation_candidates,
                    ) {
                        Ok(produced) => produced,
                        Err(diagnostic) => {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                    };
                    if let Some((_, receipt)) = facts.flow.ownership.owned_selection_at_evaluation(
                        state.symbol,
                        statement_index as u32,
                        evaluation,
                    ) {
                        owned_selection::apply_availability(
                            program,
                            facts.flow.ownership,
                            receipt,
                            places,
                        );
                        owned_selection::retire_source_storage(
                            facts.flow.ownership,
                            receipt,
                            places,
                            remaining,
                        );
                        selections.consume(program, facts.flow.ownership, receipt);
                    }
                    instruction_dispositions.statement(
                        program,
                        facts,
                        machine,
                        state,
                        statement_index,
                        evaluation,
                        places,
                        &mut *permission_events,
                    );
                    if !selected_cleanup.is_empty()
                        || !selected.is_empty()
                        || permission_events.len() > event_start
                    {
                        if let Err(diagnostic) = selected_cleanup.observe(
                            program,
                            &selected,
                            &produced_storage.transfers,
                            &routes,
                            &calls,
                            &permission_events[event_start..],
                            &occurrences,
                            &produced_storage.establishments,
                            &produced_storage.binding_changes,
                            facts.flow.ownership,
                            &mut *cleanup_dispositions,
                        ) {
                            selection_diagnostics.push(diagnostic);
                            return;
                        }
                        let receiving = produced_storage
                            .establishments
                            .iter()
                            .filter(|establishment| {
                                establishment.event_ordinal >= produced_storage.evaluated_events
                            })
                            .map(|establishment| StorageEstablishment {
                                event_ordinal: establishment.event_ordinal
                                    - produced_storage.evaluated_events,
                                destination: establishment.destination.clone(),
                                prior_value_transferred: establishment.prior_value_transferred,
                            })
                            .collect::<Vec<_>>();
                        if let Err(diagnostic) = remaining.observe_evaluation(
                            program,
                            &routes,
                            &calls,
                            &permission_events[event_start + produced_storage.evaluated_events..],
                            &facts.flow.ownership.segments,
                            &mut *cleanup_dispositions,
                            &receiving,
                        ) {
                            selection_diagnostics.push(diagnostic);
                        }
                    }
                }
                PermissionStep::Exit {
                    coordinate,
                    crash: false,
                } => {
                    let event_start = permission_events.len();
                    instruction_dispositions.exit(
                        program,
                        facts,
                        machine,
                        state,
                        coordinate,
                        places,
                        &mut *permission_events,
                    );
                    append_owned_cleanup_permission_events(
                        program,
                        &observed_receivers_by_statement,
                        state,
                        state_flow.machine_symbol,
                        coordinate,
                        places,
                        remaining,
                        &mut facts.flow.ownership.segments,
                        &mut *permission_events,
                    );
                    if let Err(diagnostic) = selected_cleanup.finish(
                        program,
                        state,
                        coordinate,
                        &permission_events[event_start..],
                        remaining,
                        facts.flow.ownership,
                        &mut *cleanup_dispositions,
                    ) {
                        selection_diagnostics.push(diagnostic);
                    }
                }
                PermissionStep::Exit { crash: true, .. } | PermissionStep::MissedGuard => {}
            }
        },
    );
    Some(facts.flow.ownership.storage_transfers.insert_many(moves))
}

/*
    The parts of one state's permission walk that read only the program and
    facts the walk never writes: flow control rows, semantic facts and content
    plans. Each state's preparation is built just before that state is
    walked, on the serial and the parallel path alike, and dropped after it,
    so a large roster never holds every preparation at once.
*/
#[derive(Clone)]
struct PreparedStateWalk {
    /* What each call may write, by call ordinal in the state's call roster. */
    call_writes: Vec<(
        PermissionEventSource,
        Option<Vec<validation::CanonicalPlace>>,
    )>,
    temporary_owners: Result<Vec<temporary_results::TemporaryOwner>, Diagnostic>,
    /* Owned temporaries completed and observed in each statement. */
    temporary_occurrences: Vec<crate::checks::ownership_occurrences::OwnedTemporaryOccurrences>,
    /*
        Inactive owned case namespaces at each step `walk` visits, by visit
        ordinal; `None` where the step selects no case evidence. Consecutive
        steps with the same exclusions share one list.
    */
    case_exclusions: Vec<Option<std::sync::Arc<[validation::CanonicalPlace]>>>,
    conservation_candidates: Vec<crate::checks::content::ConservationCandidate>,
}

impl PreparedStateWalk {
    /* The preparation of a state the walk skips before reading any of it. */
    fn skipped() -> Self {
        Self {
            call_writes: Vec::new(),
            temporary_owners: Ok(Vec::new()),
            temporary_occurrences: Vec::new(),
            case_exclusions: Vec::new(),
            conservation_candidates: Vec::new(),
        }
    }
}

/*
    The queries one state's walk will ask, enumerated before answering any,
    with the state's cheaper preparation already done.
*/
struct StateQueries<'program> {
    state_flow: &'program checked_trees::FlowStateFact,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    steps: Vec<PermissionStep>,
    conservation: crate::checks::content::ConservationSites,
    call_writes: Vec<(
        PermissionEventSource,
        Option<Vec<validation::CanonicalPlace>>,
    )>,
    temporary_owners: Result<Vec<temporary_results::TemporaryOwner>, Diagnostic>,
    temporary_occurrences: Vec<crate::checks::ownership_occurrences::OwnedTemporaryOccurrences>,
}

/*
    Steps per parallel run of case-exclusion preparation. A run computes its
    steps in order and shares an unchanged exclusion list with the previous
    step; shorter runs keep a large state's steps spread across threads.
*/
const CASE_EXCLUSION_RUN: usize = 8;

/*
    One prepared walk per state, in roster order. A bodyless or unknown state
    has no walk and gets an empty preparation.
*/
fn prepare_state_walks(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flows: &[(
        arena::Handle<checked_trees::FlowStateFact>,
        checked_trees::FlowStateFact,
    )],
    mutated_places: &MutatedPlaces<'_>,
) -> Vec<PreparedStateWalk> {
    use rayon::prelude::*;
    validation::checking_thread_pool().install(|| {
        let queries = state_flows
            .par_iter()
            .map(|(_, state_flow)| {
                let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
                let state = program.state_by_symbol(state_flow.state_symbol)?;
                if machine.has_bodyless_signature() {
                    return None;
                }
                let calls = crate::flow::call_flows(&facts.flow.control, state_flow.calls);
                let mut steps = Vec::new();
                super::permission_schedule::walk(program, state, calls, (), |step, _| {
                    steps.push(step)
                });
                let conservation = if facts.qualifications.content.conservation_plans.is_empty() {
                    crate::checks::content::ConservationSites::default()
                } else {
                    crate::checks::content::conservation_sites(program, facts, machine, state)
                };
                let call_writes = state_call_writes(program, machine, state, calls, mutated_places);
                let temporary_owners =
                    temporary_results::temporary_owners(program, facts, machine, state, calls);
                let temporary_occurrences = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .enumerate()
                    .map(|(statement_index, statement)| {
                        crate::checks::ownership_occurrences::owned_temporary_occurrences(
                            program,
                            machine,
                            state,
                            statement_index,
                            statement,
                            &facts.operators,
                            &facts.flow.control,
                        )
                    })
                    .collect();
                Some(StateQueries {
                    state_flow,
                    machine,
                    state,
                    steps,
                    conservation,
                    call_writes,
                    temporary_owners,
                    temporary_occurrences,
                })
            })
            .collect::<Vec<_>>();
        let step_runs = queries
            .iter()
            .flatten()
            .flat_map(|state| {
                state
                    .steps
                    .chunks(CASE_EXCLUSION_RUN)
                    .map(move |steps| (state, steps))
            })
            .collect::<Vec<_>>();
        let groups = queries
            .iter()
            .flatten()
            .flat_map(|state| (0..state.conservation.groups()).map(move |group| (state, group)))
            .collect::<Vec<_>>();
        let (case_exclusions, conservation_candidates) = rayon::join(
            || {
                step_runs
                    .par_iter()
                    .flat_map_iter(|&(query, steps)| {
                        let mut carried = None;
                        steps
                            .iter()
                            .map(|&step| {
                                let excluded = SelectedCases::at_step(
                                    program,
                                    facts,
                                    query.state_flow,
                                    query.machine,
                                    query.state,
                                    step,
                                )?
                                .inactive_owned_cases_after(carried.as_ref());
                                carried = Some(std::sync::Arc::clone(&excluded));
                                Some(excluded)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            },
            || {
                groups
                    .par_iter()
                    .map(|&(query, group)| {
                        query.conservation.answer_group(
                            program,
                            facts,
                            query.machine,
                            query.state,
                            group,
                        )
                    })
                    .collect::<Vec<_>>()
            },
        );
        let mut case_exclusions = case_exclusions.into_iter();
        let mut conservation_candidates = conservation_candidates.into_iter();
        queries
            .into_iter()
            .map(|query| match query {
                None => PreparedStateWalk::skipped(),
                Some(query) => PreparedStateWalk {
                    case_exclusions: case_exclusions.by_ref().take(query.steps.len()).collect(),
                    conservation_candidates: {
                        let mut answers = conservation_candidates
                            .by_ref()
                            .take(query.conservation.groups())
                            .flatten()
                            .collect::<Vec<_>>();
                        answers.sort_unstable_by_key(|(site, _)| *site);
                        answers.into_iter().map(|(_, answer)| answer).collect()
                    },
                    call_writes: query.call_writes,
                    temporary_owners: query.temporary_owners,
                    temporary_occurrences: query.temporary_occurrences,
                },
            })
            .collect()
    })
}

/*
    The places one call may write, from the borrow facts and the published
    mutation summaries; built once by the caller and shared by every state.
*/
type MutatedPlaces<'a> = dyn Fn(
        &typed_trees::machine::Machine,
        &typed_trees::state::State,
        &checked_trees::BorrowCallFact,
    ) -> Option<Vec<validation::CanonicalPlace>>
    + Sync
    + 'a;

/*
    What each call of the state may write. Mutation invalidates observations
    even when no linear claim needs restoration. The shared write owner may be
    unknown: forgetting then requires no invented storage authority or
    additional admission.
*/
fn state_call_writes(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &[checked_trees::FlowCallFact],
    mutated_places: &MutatedPlaces<'_>,
) -> Vec<(
    PermissionEventSource,
    Option<Vec<validation::CanonicalPlace>>,
)> {
    calls
        .iter()
        .map(|call| {
            let borrowed = checked_trees::BorrowCallFact {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                authored_expression: call.authored_expression,
                input_reference_loans: None,
                reference_slot_effects: Default::default(),
                completed_reference_loans: Default::default(),
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                has_receiver: call.has_receiver,
                accesses: call.accesses,
            };
            let source = PermissionEventSource::Call {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: call.target_symbol,
            };
            let internal_transfer = validation::find_call_site_in_state(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            )
            .is_some_and(|site| site.is_internal_state_transfer(program, machine.symbol));
            (
                source,
                if internal_transfer {
                    Some(Vec::new())
                } else {
                    mutated_places(machine, state, &borrowed)
                },
            )
        })
        .collect()
}

#[derive(Debug)]
pub(crate) struct DerivedCrashFrontier {
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_ordinal: u32,
    claims: Vec<ClaimIdentity>,
}

/*
    Retain the machine-local claims that are definitely live at each explicit
    crash. The result is an underapproximation by design: a conditional sum
    payload is omitted until active-case proof can make its liveness definite.
    Crash abandons these claims; it does not synthesize cleanup or consumption.

    This derives them from the facts alone; `record_crash_frontier_lower_bounds`
    writes them into the contract plans.
*/
pub(crate) fn derive_crash_frontier_lower_bounds(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
) -> Vec<DerivedCrashFrontier> {
    let mut derived = Vec::new();
    for (_, state_flow) in facts.flow.control.states.iter() {
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
            continue;
        };
        if !program
            .statement_table
            .admitted_execution_statements(state.statement_nodes)
            .any(|(_, statement)| {
                statement.as_transition().is_some_and(|transition| {
                    matches!(
                        transition.exit,
                        typed_trees::statement::TransitionExit::Crash(_)
                    )
                })
            })
        {
            continue;
        }
        let events = owned_permission_events(facts, state_flow.machine_symbol, state.symbol);
        let mut places =
            initial_linear_places(program, state, state_flow.machine_symbol, state.symbol);
        apply_recorded_state_entry_events(&events, &facts.flow.ownership.segments, &mut places);
        let incoming = incoming_guards.for_machine(machine.symbol);
        let proven_conditional_claims =
            proven_conditional_entry_claims(program, state, incoming, &places);
        let has_case_places = places.iter().any(|place| {
            place
                .path
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        });

        let mut ignored_diagnostics = Vec::new();
        super::permission_schedule::walk(
            program,
            state,
            facts.flow.control.calls.span_or_empty(state_flow.calls),
            places,
            |step, places| {
                if has_case_places
                    && let Some(selected) =
                        SelectedCases::at_step(program, facts, state_flow, machine, state, step)
                {
                    selected.apply_claims(places);
                }
                match step {
                    PermissionStep::Evaluate {
                        statement,
                        evaluation,
                    } => {
                        if evaluation == checked_trees::FlowPermissionEvaluation::Statement
                            && let Some((_, receipt)) = facts
                                .flow
                                .ownership
                                .owned_selection_at(state.symbol, statement as u32)
                        {
                            owned_selection::apply_availability(
                                program,
                                &facts.flow.ownership,
                                receipt,
                                places,
                            );
                            return;
                        }
                        apply_recorded_statement_events(
                            program,
                            facts,
                            machine,
                            state,
                            statement,
                            evaluation,
                            &events,
                            places,
                            &mut ignored_diagnostics,
                        );
                    }
                    PermissionStep::Exit {
                        coordinate,
                        crash: true,
                    } => {
                        let claims = places
                            .iter()
                            .filter_map(|place| {
                                (place.live
                                    && (!place.conditional
                                        || place.claim_identity.is_some_and(|identity| {
                                            proven_conditional_claims.contains(&identity)
                                        })))
                                .then_some(place.claim_identity?)
                                .filter(|identity| *identity != ClaimIdentity::Unknown)
                            })
                            .collect();
                        derived.push(DerivedCrashFrontier {
                            machine_symbol: state_flow.machine_symbol,
                            state_symbol: state.symbol,
                            statement_ordinal: u32::try_from(coordinate.statement_index).expect(
                                "state-local statement ordinal exceeds checked identity range",
                            ),
                            claims,
                        });
                    }
                    _ => {}
                }
            },
        );
    }

    derived
}

/*
    Retain each checked crash site's derived frontier in its contract's crash
    plan; a site without one keeps its current lower bound.
*/
pub(crate) fn record_crash_frontier_lower_bounds(
    contract_plans: &mut checked_trees::MachineContractPlans,
    derived: &[DerivedCrashFrontier],
) {
    contract_plans.update_plans(|contract| {
        let checked_sites = contract
            .crash
            .checked_sites()
            .iter()
            .map(|site| {
                let location = site.location();
                let frontier = derived
                    .iter()
                    .find(|frontier| {
                        frontier.machine_symbol == contract.machine
                            && frontier.state_symbol == location.state()
                            && frontier.statement_ordinal == location.statement_ordinal()
                    })
                    .map(|frontier| frontier.claims.clone())
                    .unwrap_or_else(|| site.frontier_lower_bound().to_vec());
                site.clone().with_frontier_lower_bound(frontier)
            })
            .collect();
        contract.crash = contract
            .crash
            .clone()
            .with_checked_sites(checked_sites)
            .expect("derived crash frontiers retain valid checked-site identity");
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProvenCaseMembership {
    parameter: SymbolHandle,
    /*
        Source-independent path below the final-state parameter at which this
        case tag was tested. The selected variant is stored separately.
    */
    subject_path: Vec<facts::PlaceSegment>,
    variant: SymbolHandle,
}

/*
    Conditional sum payloads are only a crash-frontier lower bound when the
    path into this state proves every case on the payload path active.
    Case-pattern dispatch is retained in typed trees as a symbol-stamped
    `value == Type::Case` guard. Incoming-edge argument composition rebinds the
    tested subject to a final-state parameter plus a canonical symbol path.

    The proof is attached to the claim identity rather than the parameter
    spelling. A whole-value transfer in the target state therefore preserves
    the proof, while overwriting the parameter mints a different identity and
    cannot accidentally inherit it. A nested claim enters the lower bound only
    when membership evidence covers every case segment.
*/
fn proven_conditional_entry_claims(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    places: &[LinearPlace],
) -> Vec<ClaimIdentity> {
    let parameters = program.state_parameters(state).iter().collect::<Vec<_>>();
    let mut memberships = Vec::new();

    for entry in incoming.iter().filter(|entry| entry.holds_at(state.symbol)) {
        let mut case_tests = Vec::new();
        collect_positive_case_tests(
            program,
            entry.evaluation_state(),
            entry.guard(),
            &mut case_tests,
        );
        for (subject, variant) in case_tests {
            let Some(subject) = crate::flow::source_independent_place(program, subject) else {
                continue;
            };
            for parameter in &parameters {
                let Some(argument) = entry.argument_place_for_parameter(parameter.symbol) else {
                    continue;
                };
                if subject.root != argument.root
                    || !subject.segments.starts_with(&argument.segments)
                {
                    continue;
                }
                let membership = ProvenCaseMembership {
                    parameter: parameter.symbol,
                    subject_path: subject.segments[argument.segments.len()..].to_vec(),
                    variant,
                };
                crate::shared::collections::push_unique(&mut memberships, membership);
            }
        }
    }

    let mut proven = Vec::new();
    for place in places.iter().filter(|place| place.conditional) {
        let mut subject_path = Vec::new();
        let all_cases_proven = place.path.iter().all(|segment| {
            let proven = match segment {
                facts::PlaceSegment::Case { variant } => memberships.iter().any(|evidence| {
                    facts::PlaceRoot::Symbol(evidence.parameter) == place.root
                        && evidence.subject_path == subject_path
                        && evidence.variant == *variant
                }),
                _ => true,
            };
            subject_path.push(*segment);
            proven
        });
        if all_cases_proven
            && let Some(identity) = place.claim_identity
            && identity != ClaimIdentity::Unknown
            && !proven.contains(&identity)
        {
            proven.push(identity);
        }
    }
    proven
}

/*
    Extract positive case-membership conjuncts. Boolean-arm lowering may wrap
    a predicate as `predicate == true`; unwrap that shell but deliberately do
    not infer through negation or disjunction here.
*/
pub(crate) fn collect_positive_case_tests(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    tests: &mut Vec<(typed_trees::expression::ExpressionHandle, SymbolHandle)>,
) {
    visit_positive_case_tests(program, state_symbol, expression, &mut |_, test| {
        if let Some(test) = test {
            tests.push(test);
        }
    });
}

/*
    Report each leaf through the same traversal used by custody selection.
    A declined leaf is diagnostic information, never an authenticated test.
*/
pub(crate) fn visit_positive_case_tests(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    visit: &mut impl FnMut(
        typed_trees::expression::ExpressionHandle,
        Option<(typed_trees::expression::ExpressionHandle, SymbolHandle)>,
    ),
) {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let Some(binary) = program.expression_table.binary_node(expression) else {
        visit(expression, None);
        return;
    };
    if binary.operator == BinaryOperator::And {
        visit_positive_case_tests(program, state_symbol, binary.left, visit);
        visit_positive_case_tests(program, state_symbol, binary.right, visit);
        return;
    }
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        visit(expression, None);
        return;
    }

    let is_true = |candidate| {
        matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(binary.left) {
        visit_positive_case_tests(program, state_symbol, binary.right, visit);
        return;
    }
    if is_true(binary.right) {
        visit_positive_case_tests(program, state_symbol, binary.left, visit);
        return;
    }

    let Some(machine) = program.machine_holding_state(state_symbol) else {
        visit(expression, None);
        return;
    };
    let variant = validation::applied_case_membership_variant(
        program,
        machine,
        program.state_by_symbol(state_symbol),
        expression,
        binary,
    );
    visit(expression, variant.map(|variant| (binary.left, variant)));
}

pub(crate) fn exact_positive_case_test(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(typed_trees::expression::ExpressionHandle, SymbolHandle)> {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};

    let binary = program.expression_table.binary_node(expression)?;
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return None;
    }
    let is_true = |candidate| {
        matches!(
            program.expression_table.expression(candidate),
            ExpressionNode::Boolean(true)
        )
    };
    if is_true(binary.left) {
        return exact_positive_case_test(program, state_symbol, binary.right);
    }
    if is_true(binary.right) {
        return exact_positive_case_test(program, state_symbol, binary.left);
    }
    let mut tests = Vec::new();
    collect_positive_case_tests(program, state_symbol, expression, &mut tests);
    let [test] = tests.as_slice() else {
        return None;
    };
    Some(*test)
}

pub(crate) fn case_transition_run_is_exhaustive(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statements: &[StatementNode],
    arm_indices: &[usize],
) -> bool {
    let mut subject_label: Option<String> = None;
    let mut covered = Vec::new();
    for &statement_index in arm_indices {
        let Some(StatementNode::Transition(transition)) = statements.get(statement_index) else {
            return false;
        };
        let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
            return false;
        };
        let Some((subject, variant)) = exact_positive_case_test(program, state_symbol, guard)
        else {
            let mut tests = Vec::new();
            collect_positive_case_tests(program, state_symbol, guard, &mut tests);
            if tests.is_empty() {
                return false;
            }
            for (subject, _) in tests {
                let label = program.expression_table.display_name(subject);
                if subject_label
                    .as_ref()
                    .is_some_and(|expected| *expected != label)
                {
                    return false;
                }
                subject_label = Some(label);
            }
            continue;
        };
        let label = program.expression_table.display_name(subject);
        if subject_label
            .as_ref()
            .is_some_and(|expected| *expected != label)
        {
            return false;
        }
        subject_label = Some(label);
        crate::shared::collections::push_unique(&mut covered, variant);
    }
    let Some(first_variant) = covered.first() else {
        return false;
    };
    program.data_definitions().iter().any(|definition| {
        let variants = program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant)
                    if !::validation::impossible_variant(program, variant) =>
                {
                    Some(variant.symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        variants.contains(first_variant)
            && variants.len() == covered.len()
            && variants.iter().all(|variant| covered.contains(variant))
    })
}

/*
    Forward an invariant input/result correspondence to the caller's held
    claim. A callee-local establishment instead retains the receiving
    occurrence's identity; the later call join records its exact return
    correspondence without copying a template identity across invocations.
*/
fn reconcile_state_call_result_origins(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    ownership: &mut checked_trees::FlowOwnershipFacts,
    permission_events: &mut [EventFact],
) -> Result<Vec<CheckedClaimOutcomeMap>, &'static str> {
    let iteration_limit = permission_events.len().saturating_add(1);
    for _ in 0..iteration_limit {
        let maps =
            derive_checked_claim_outcome_maps(program, borrow, ownership, permission_events)?;
        let rewrites =
            call_result_origin_rewrites(program, &ownership.segments, permission_events, &maps);
        let origins_changed = apply_claim_origin_rewrites(permission_events, ownership, &rewrites);
        let liveness_changed = apply_statically_inactive_call_results(
            program,
            &ownership.segments,
            permission_events,
            &maps,
        );
        if !origins_changed && !liveness_changed {
            return Ok(maps);
        }
    }
    Err("call-result claim origins did not converge")
}

fn apply_statically_inactive_call_results(
    program: &dyn ::validation::CheckingInput,
    segments: &arena::PagedArenaSet<facts::PlaceSegment>,
    permission_events: &mut [EventFact],
    maps: &[CheckedClaimOutcomeMap],
) -> bool {
    if maps.is_empty() {
        return false;
    }
    let inactive_identities = permission_events
        .iter()
        .filter_map(|event| {
            if event.kind != PermissionEventKind::Establish
                || event.access != PermissionAccess::Owned
                || !event.obligation_live
            {
                return None;
            }
            let (_, expression, receiving_path) = receiving_value(program, segments, event)?;
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            let target = program.state_by_symbol(call.target_symbol)?;
            let map = maps.iter().find(|map| map.state_symbol == target.symbol)?;
            (!map
                .entries
                .iter()
                .any(|entry| entry.output_path == receiving_path))
            .then_some(event.claim_identity)
        })
        .filter(|identity| *identity != ClaimIdentity::Unknown)
        .collect::<Vec<_>>();
    if inactive_identities.is_empty() {
        return false;
    }
    let mut changed = false;
    for event in permission_events.iter_mut().filter(|event| {
        event.access == PermissionAccess::Owned
            && inactive_identities.contains(&event.claim_identity)
    }) {
        changed |= event.obligation_live || event.claim_identity != ClaimIdentity::Unknown;
        event.obligation_live = false;
        event.claim_identity = ClaimIdentity::Unknown;
    }
    changed
}
