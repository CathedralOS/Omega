//! Public and crate-private demand facade for caller-visible write frames.
//!
//! The facade gathers expression and statement call demand and delegates exact
//! complete-or-opaque summaries to its parent frame engine. It does not own
//! call validation, alias-origin inference, or transition fixed points.

mod call_origins;
mod normal_subjects;
mod result_transport;
mod transfer_operands;

pub(crate) use normal_subjects::InvocationReferenceSubject;

use super::boundary_calls::receiver_requires_boundary_frame;
use super::caller_aliases::{
    AssignmentWriteTarget, LocalWriteOrigin, LocalWriteSource, ReferenceResultLeafOrigin,
    assignment_write_target, local_write_origins_before_statement,
};
use super::caller_aliases::{
    CallOriginContext, CallerWriteSite, expression_has_calls, with_caller_origins,
};
use super::{
    coarse_place_path, known_boundary_call_written_paths_for_parts, known_call_written_paths,
    known_call_written_paths_for_parts, memoized, normalize_state_relative_path,
    receiver_member_chain,
};
use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::cycle_frames::summarize_state_written_paths_for_cycles;
use crate::machine_calls::calls::write_frames::state_write_walk::{
    CollectedStatementPrefix, summarize_state_written_paths,
};
use facts::NormalizedWriteFrame;
use std::sync::{Mutex, OnceLock};
use symbols::SymbolHandle;
use symbols::SymbolKeyMap as HashMap;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};

/// Shared conservative call-frame resolver. A complete result is the set of
/// caller-visible places the call may write; `None` is deliberately opaque and
/// requires consumers to invalidate every fact they cannot otherwise prove.
///
/// The resolver owns the top-level symbol cache so validation, proof, recast,
/// and invariant consumers share one resolution law instead of reimplementing
/// call identity. Per-machine caches are built at the query boundary and fail
/// closed if the program's symbols are already invalid.
pub struct CallFrameResolver<'program> {
    program: &'program dyn crate::CheckingInput,
    tables: std::sync::Arc<CallFrameTables<'program>>,
    /// The query memos. Inside a frozen-program scope every resolver built
    /// for the scope's program shares one set; see
    /// `CheckingContext`.
    caches: std::sync::Arc<CallFrameCaches>,
}

/// The program-fixed symbol tables a resolver reads. They depend only on the
/// typed program, so a frozen checking context builds them once and every
/// resolver over it shares them; building them per resolver cost about 2 ms of
/// whole-program work across thousands of resolvers in one std check.
pub struct CallFrameTables<'program> {
    pub(crate) symbols: TopLevelSymbols<'program>,
    /// Per-machine member/state symbol tables. `None` retains the build's
    /// non-empty-diagnostics verdict so callers keep failing closed.
    machine_symbols: HashMap<SymbolHandle, Option<std::sync::Arc<MachineSymbols<'program>>>>,
}

impl<'program> CallFrameTables<'program> {
    /// `None` when the program's top-level symbols carry diagnostics.
    pub(crate) fn build(program: &'program dyn crate::CheckingInput) -> Option<Self> {
        let mut diagnostics = Vec::new();
        let symbols = TopLevelSymbols::build(program, &mut diagnostics);
        diagnostics.is_empty().then(|| Self {
            symbols,
            machine_symbols: program
                .machines()
                .iter()
                .map(|machine| {
                    let mut diagnostics = Vec::new();
                    let built = MachineSymbols::build(program, machine, &mut diagnostics);
                    (
                        machine.symbol,
                        diagnostics.is_empty().then(|| std::sync::Arc::new(built)),
                    )
                })
                .collect(),
        })
    }
}

pub(super) fn machine_symbols<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
) -> Option<std::sync::Arc<MachineSymbols<'program>>> {
    if let Some(tables) = program.call_frame_tables() {
        return tables
            .machine_symbols
            .get(&machine.symbol)
            .cloned()
            .flatten();
    }
    let mut diagnostics = Vec::new();
    let built = MachineSymbols::build(program, machine, &mut diagnostics);
    diagnostics.is_empty().then(|| std::sync::Arc::new(built))
}

/// Every memo a resolver fills. Each is pure over the resolver's immutable
/// program, so resolvers over the same frozen program may share them.
#[derive(Default)]
pub(crate) struct CallFrameCaches {
    /// Exact statement calls are queried repeatedly by monotone validation
    /// fixpoints. The program is immutable for this resolver's lifetime, so a
    /// call-node address plus its owning machine is a stable cache key.
    statement_calls: Mutex<HashMap<(u32, u32, usize), NormalizedWriteFrame>>,
    /// Every public query below is pure over that same immutable program, so
    /// the fixpoint's per-pass repetition memoizes under the same law: AST
    /// nodes key by address plus owning machine, handles and symbols by their
    /// own durable identities.
    binding_replacements: Mutex<HashMap<(SymbolHandle, usize), Option<bool>>>,
    reference_slot_replacements: Mutex<HashMap<(SymbolHandle, usize), Option<bool>>>,
    local_reference_origins: Mutex<
        HashMap<
            (SymbolHandle, usize, SymbolHandle),
            Option<(SymbolHandle, Vec<facts::PlaceSegment>)>,
        >,
    >,
    reference_result_origins: Mutex<HashMap<SymbolHandle, Option<Vec<LocalWriteSource>>>>,
    reference_result_leaves: Mutex<HashMap<SymbolHandle, Option<Vec<ReferenceResultLeafOrigin>>>>,
    reference_result_transports:
        Mutex<HashMap<SymbolHandle, Option<Vec<super::ReferenceResultTransport>>>>,
    stable_expression_bindings: Mutex<HashMap<(SymbolHandle, ExpressionHandle), bool>>,
    stable_call_bindings: Mutex<HashMap<(SymbolHandle, usize), bool>>,
    caller_isolated_proof_values: Mutex<HashMap<typed_trees::types::TypeReferenceHandle, bool>>,
    write_origin_requirements: Mutex<HashMap<typed_trees::types::TypeReferenceHandle, bool>>,
    assignment_targets: Mutex<HashMap<(SymbolHandle, usize), Option<AssignmentWriteTarget>>>,
    assignment_frames: Mutex<HashMap<(SymbolHandle, usize), NormalizedWriteFrame>>,
    local_write_origins: Mutex<HashMap<(SymbolHandle, usize), Option<Vec<LocalWriteOrigin>>>>,
    /// One prefix-walk snapshot per statement index, built once per
    /// (machine, state): replaces a fresh O(index) prefix walk per demand
    /// site with a single O(statements) walk per state.
    state_write_collections:
        Mutex<HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>>,
    /// The shared-accumulator flavor of the same collection, serving
    /// `ReferenceBefore` boundaries.
    state_write_shared_collections:
        Mutex<HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>>,
    expression_frames: Mutex<HashMap<(SymbolHandle, ExpressionHandle), NormalizedWriteFrame>>,
    statement_value_frames: Mutex<HashMap<(SymbolHandle, usize), NormalizedWriteFrame>>,
    inferred_state_frames: Mutex<HashMap<SymbolHandle, NormalizedWriteFrame>>,
    inferred_machine_frames:
        Mutex<HashMap<SymbolHandle, std::sync::Arc<Vec<NormalizedWriteFrame>>>>,
    /// Every assignment's target and prefix origins per (machine, state),
    /// from one recorded walk of the state instead of one prefix walk per
    /// assignment statement and per query kind.
    state_assignments: Mutex<
        HashMap<
            (SymbolHandle, SymbolHandle),
            std::sync::Arc<Vec<(usize, super::state_write_walk::AssignmentPrefix)>>,
        >,
    >,
    /// Whole-program operational and service-reach plans an operand Call node
    /// consumes to test call-candidate totality. The immutable checking
    /// context owns this pair; changing selected cleanup or other prepared
    /// inputs replaces its memos, even when TypedTrees stays unchanged.
    call_plans: OnceLock<
        std::sync::Arc<(
            flow_effects::OperationalPlan,
            flow_effects::ServiceReachInferencePlan,
        )>,
    >,
}

impl CallFrameCaches {
    fn call_plans(
        &self,
        program: &dyn crate::CheckingInput,
    ) -> std::sync::Arc<(
        flow_effects::OperationalPlan,
        flow_effects::ServiceReachInferencePlan,
    )> {
        self.call_plans
            .get_or_init(|| std::sync::Arc::new(compute_call_plans(program)))
            .clone()
    }
}

/// Both known-callee written-path lanes (boundary call and requirement call)
/// take the same parts; a probe closure picks the lane.
type KnownWrittenPathsForParts = for<'site> fn(
    &'site dyn crate::CheckingInput,
    &Machine,
    &MachineSymbols<'_>,
    &TopLevelSymbols<'_>,
    &[String],
    &str,
    CallerWriteSite<'site>,
    &[ExpressionHandle],
    &mut FrameInference,
) -> Option<Vec<String>>;

fn compute_call_plans(
    program: &dyn crate::CheckingInput,
) -> (
    flow_effects::OperationalPlan,
    flow_effects::ServiceReachInferencePlan,
) {
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    (operational, service_reaches)
}

/// Whole-program call plans for operand call-candidate checks. A live
/// resolver shares its once-computed pair. Resolver-free inference shares the
/// same pair inside a frozen-program scope, or computes a one-shot pair outside
/// it. Recursive frame queries must not deep-clone both plans per callee.
pub fn operand_call_plans(
    program: &dyn crate::CheckingInput,
    frames: Option<&CallFrameResolver<'_>>,
) -> std::sync::Arc<(
    flow_effects::OperationalPlan,
    flow_effects::ServiceReachInferencePlan,
)> {
    match frames {
        Some(frames) => frames.call_plans(),
        None => crate::frozen_program::frozen_program_memos(program).map_or_else(
            || std::sync::Arc::new(compute_call_plans(program)),
            |memos| memos.call_frames.call_plans(program),
        ),
    }
}

impl<'program> CallFrameResolver<'program> {
    /// Reconstruct every carried slot along this local home's successor path.
    /// This proves intermediate retention and prevents a repeated establishment
    /// from overwriting a prior live generation; it is not a return summary.
    pub fn local_backing_survives_state_jump(
        &self,
        machine: &Machine,
        source: &State,
        statement_index: usize,
        target: &State,
        arguments: &[ExpressionHandle],
        backing: SymbolHandle,
    ) -> bool {
        super::stored_calls::retention::local_backing_survives_jump(
            self.program,
            machine,
            source,
            statement_index,
            target,
            arguments,
            backing,
            &self.tables.symbols,
        )
        .is_some()
    }

    /// Exact logical contract. This does not authenticate normal completion or
    /// lend authority; the consumer must join its performed-call resource row.
    pub fn reference_slot_contract<'query>(
        &'query self,
        machine: &'query Machine,
        state: &'query State,
        statement_index: usize,
        call_ordinal: usize,
        target: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<super::ReferenceSlotContract<'query>> {
        super::stored_calls::invocation_contract(
            self.program,
            machine,
            state,
            statement_index,
            call_ordinal,
            target,
            expression,
        )
    }

    /// Body-derived normal-exit slot relation in the selected state's formal
    /// namespace. This is also required for a stored-output body with no callers.
    pub fn state_stored_reference_effects(
        &self,
        machine: &Machine,
        state: &State,
    ) -> Option<Vec<super::StoredReferenceEffect>> {
        super::stored_calls::state_effects(
            self.program,
            machine,
            state,
            &self.tables.symbols,
            &mut FrameInference::default(),
        )
        .and_then(super::stored_calls::public_effects)
    }

    /// The same formal relation under fixed scalar values proved by completed
    /// caller operands. These invocation rows never enter the state-only memo.
    /// Unknown values retain the ordinary full possible-slot relation.
    pub fn state_stored_reference_effects_for_indices(
        &self,
        machine: &Machine,
        state: &State,
        indices: &[Option<usize>],
    ) -> Option<Vec<super::StoredReferenceEffect>> {
        if self.program.state_parameters(state).len() != indices.len() {
            return None;
        }
        let mut inference = FrameInference::default();
        inference.with_invocation(self.program, state, &[], indices, |inference| {
            super::stored_calls::state_effects(
                self.program,
                machine,
                state,
                &self.tables.symbols,
                inference,
            )
            .and_then(super::stored_calls::public_effects)
        })
    }

    /// Closed dispatch joins the checked slot-output relation of every possible
    /// realization. These are custody suppliers, never private predicate facts.
    pub fn dynamic_stored_reference_effects(
        &self,
        target: SymbolHandle,
    ) -> Option<Vec<super::StoredReferenceEffect>> {
        let slot = self.program.dynamic_slot_by_symbol(target)?;
        super::stored_calls::dynamic_effects(
            self.program,
            slot,
            &self.tables.symbols,
            &mut FrameInference::default(),
        )
        .and_then(super::stored_calls::public_effects)
    }

    /// Complete normal-exit reference-slot relation, reconstructed from the
    /// selected body and instantiated in this exact caller prefix. Unknown
    /// evidence is not an empty effect set.
    pub fn stored_reference_effects(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<Vec<super::StoredReferenceEffect>> {
        if statement.as_call().is_none()
            && !statement_value_expression_roots(self.program, statement)
                .iter()
                .any(|expression| expression_has_calls(self.program, *expression))
        {
            return Some(Vec::new());
        }
        let (_, _, _, transfers) = self.statement_reference_transfer(machine, statement)?;
        let transfer = transfers.all_routes();
        if !transfer.is_complete() {
            return None;
        }
        super::stored_calls::public_effects(transfer.effects)
    }

    /// Reference generations retained by the lexical successor. Calls in a
    /// departing transition arm keep their own invocation/edge custody; they
    /// cannot replace the descriptor roster on the guard's fallthrough route.
    pub fn stored_reference_fallthrough_effects(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<Vec<super::StoredReferenceEffect>> {
        if statement.as_call().is_none()
            && !statement_value_expression_roots(self.program, statement)
                .iter()
                .any(|expression| expression_has_calls(self.program, *expression))
        {
            return Some(Vec::new());
        }
        let (_, _, _, transfers) = self.statement_reference_transfer(machine, statement)?;
        let transfer = transfers.fallthrough();
        if !transfer.is_complete() {
            return None;
        }
        super::stored_calls::public_effects(transfer.effects)
    }

    /// Each normal invocation establishes its own slot contents before the
    /// next operand. Keep those cuts from the same ordered evaluator used by
    /// the joined statement query; no exit summary can locate an earlier call.
    pub fn stored_reference_call_contexts(
        &self,
        machine: &Machine,
        state: &State,
        statement_index: usize,
    ) -> Option<Vec<super::stored_calls::StoredReferenceCallContext>> {
        let statement = self
            .program
            .statement_table
            .statement_at(state, statement_index)?;
        if statement.as_call().is_none()
            && !statement_value_expression_roots(self.program, statement)
                .iter()
                .any(|expression| expression_has_calls(self.program, *expression))
        {
            return Some(vec![super::stored_calls::StoredReferenceCallContext::Entry]);
        }
        let prefix = super::state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            statement_index,
            true,
        )?;
        super::stored_calls::call_contexts(
            self.program,
            machine,
            state,
            statement,
            &prefix,
            &self.tables.symbols,
            &mut FrameInference::for_state(state.symbol),
        )
    }

    /// Completed calls at this exact operand, including its evaluated children.
    /// The ordinary evaluator retains the selected target and match-arm cut;
    /// the statement's joined exit relation is a different occurrence.
    pub fn stored_reference_effects_at_expression(
        &self,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<Vec<super::StoredReferenceEffect>> {
        let statement = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)?;
        // The caller's current loan roster already owns prior statements.
        // A call-free statement adds no invocation effects and needs no second
        // reconstruction of that entry through source-level write inference.
        if statement.as_call().is_none()
            && !statement_value_expression_roots(self.program, statement)
                .iter()
                .any(|expression| expression_has_calls(self.program, *expression))
        {
            return Some(Vec::new());
        }
        // Prove empty reference-slot demand from the same ordered evaluator.
        // A completed invocation with a known empty writable frontier adds no
        // caller-entry substitution. Unknown or symbolic calls remain incomplete
        // even if this empty starting context contains no slots to invalidate.
        let empty = CollectedStatementPrefix {
            isolated_locals: Vec::new(),
            aliases: Vec::new(),
            divergent: Vec::new(),
            stored: Vec::new(),
        };
        let unbound = super::stored_calls::effects_at_expression(
            self.program,
            machine,
            state,
            statement,
            &empty,
            &self.tables.symbols,
            &mut FrameInference::for_state(state.symbol),
            expression,
        );
        if unbound.is_some_and(|snapshot| snapshot.is_complete() && snapshot.effects.is_empty()) {
            return Some(Vec::new());
        }
        let prefix = super::state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            statement_index,
            true,
        )?;
        let transfer = super::stored_calls::effects_at_expression(
            self.program,
            machine,
            state,
            statement,
            &prefix,
            &self.tables.symbols,
            &mut FrameInference::for_state(state.symbol),
            expression,
        )?;
        if !transfer.is_complete() {
            return None;
        }
        super::stored_calls::public_effects(transfer.effects)
    }

    /// Failures belong to actual invocations, not a target-wide classification.
    /// A symbolic relation remains unavailable to concrete effect consumers,
    /// while later concrete calls still retain their own failed obligations.
    pub fn stored_reference_call_failures(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<Vec<super::stored_calls::StoredReferenceCallFailure>> {
        self.statement_reference_transfer(machine, statement)
            .map(|(_, _, _, transfers)| transfers.all_routes().incomplete_calls)
    }

    /// A reachable opaque output slot needs a complete normal-exit relation,
    /// even when failed attribution installed no loans. Nested borrowed slots
    /// use the same inventory as effect construction; unknown is not empty.
    pub fn provider_output_relation_required(
        &self,
        machine: SymbolHandle,
        target: SymbolHandle,
        expression: ExpressionHandle,
    ) -> bool {
        super::stored_calls::provider_output_relation_required(
            self.program,
            machine,
            target,
            expression,
        )
    }

    /// Reference inventory after destination and RHS evaluation, before the
    /// assignment's own store. Operand calls can introduce descriptor leaves
    /// absent at entry, so an entry inventory is not a complete store frontier.
    /// Reuse the state walker's ordered slot transfer, without installing the
    /// enclosing assignment or reconstructing its captured destination.
    pub fn local_write_origins_before_store(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        binding_is_stable: impl Fn(SymbolHandle, &[facts::PlaceSegment]) -> bool,
    ) -> Option<Vec<LocalWriteOrigin>> {
        let (state, index, mut prefix, transfers) =
            self.statement_reference_transfer(machine, statement)?;
        let transfer = transfers.all_routes();
        if !transfer.is_complete() {
            return None;
        }
        super::stored_calls::apply(self.program, &mut prefix.stored, &transfer.effects);
        let mut origins = super::caller_aliases::prefix_reference_origins(
            self.program,
            Some((state, index)),
            &prefix.stored,
            &[],
            &[],
        )?;
        // Stored-slot transfer is current. Standalone alias bindings are an
        // entry inventory: a replaced slot's produced row supersedes them,
        // while an unaccounted binding change supplies an unknown origin.
        let bindings = super::caller_aliases::prefix_reference_origins(
            self.program,
            Some((state, index)),
            &[],
            &prefix.aliases,
            &prefix.divergent,
        )?;
        let stored_count = origins.len();
        for mut binding in bindings {
            if origins[..stored_count].iter().any(|origin| {
                origin.local_symbol == binding.local_symbol
                    && origin.local_segments == binding.local_segments
            }) {
                continue;
            }
            if !binding_is_stable(binding.local_symbol, &binding.local_segments) {
                binding.source = None;
            }
            origins.push(binding);
        }
        Some(origins)
    }

    fn statement_reference_transfer(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<(
        &State,
        usize,
        CollectedStatementPrefix,
        super::stored_calls::StatementSlotTransfers,
    )> {
        let trace_refusal = |phase: &str, state: SymbolHandle, index: Option<usize>| {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE stored-reference-statement-transfer-refusal machine={:?} state={:?} statement={:?} phase={}",
                    machine.symbol, state, index, phase,
                );
            }
        };
        let Some((state, _, index)) = super::caller_aliases::caller_statement_at_site(
            self.program,
            machine,
            &self.tables.symbols,
            CallerWriteSite::Statement(statement),
        ) else {
            trace_refusal("statement-site", SymbolHandle::invalid(), None);
            return None;
        };
        let Some(prefix) = super::state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            index,
            true,
        ) else {
            trace_refusal("entry-prefix", state.symbol, Some(index));
            return None;
        };
        let Some(effects) = super::stored_calls::statement_effects(
            self.program,
            machine,
            state,
            statement,
            &prefix,
            &self.tables.symbols,
            &mut FrameInference::for_state(state.symbol),
        ) else {
            trace_refusal("statement-effects", state.symbol, Some(index));
            return None;
        };
        Some((state, index, prefix, effects))
    }

    /// A bare reference-local replacement changes its binding, not the storage
    /// reached through that binding. Unknown reference-shaped results are not
    /// evidence of a store to the previous referent.
    pub fn assignment_replaces_local_reference_binding(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<bool> {
        memoized(
            &self.caches.binding_replacements,
            (machine.symbol, std::ptr::from_ref(statement).addr()),
            || super::reference_subjects::replaces_binding(self.program, machine, statement),
        )
    }

    /// Classify the selected reference slot, including projections, separately
    /// from the bare-local rebinding query used by alias and value transport.
    pub fn assignment_replaces_reference_binding(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<bool> {
        memoized(
            &self.caches.reference_slot_replacements,
            (machine.symbol, std::ptr::from_ref(statement).addr()),
            || {
                super::reference_subjects::replaces_reference_binding(
                    self.program,
                    machine,
                    statement,
                )
            },
        )
    }

    /// Whether this expression denotes a parameter's immutable incoming
    /// borrow address. This grants no read or write to its referent.
    pub fn expression_is_fixed_incoming_reference(
        &self,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        super::reference_subjects::expression_is_fixed_incoming_reference(
            self.program,
            state,
            expression,
        )
    }

    /// Resolve a bare local reference to exact live storage at this prefix.
    /// This query includes shared references, without granting them writes.
    /// Unknown origins, untracked reference slots, and coarse selectors reject.
    pub fn local_reference_origin_before_statement(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        local: SymbolHandle,
    ) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
        memoized(
            &self.caches.local_reference_origins,
            (machine.symbol, std::ptr::from_ref(statement).addr(), local),
            || {
                super::reference_subjects::local_origin(
                    self.program,
                    machine,
                    &self.tables.symbols,
                    statement,
                    local,
                    &self.caches.state_write_shared_collections,
                )
                .map(|source| (source.root, source.segments))
            },
        )
    }

    /// An incoming exclusive parameter supplies its address rather than a
    /// replaceable local descriptor home. Consumers still inspect the referee:
    /// a reference-valued referent is itself a replaceable stored descriptor.
    pub fn expression_borrows_fixed_parameter_address(
        &self,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        super::reference_subjects::expression_is_fixed_incoming_reference(
            self.program,
            state,
            expression,
        )
    }

    /// Complete exact referents captured by a reference-valued operand at this
    /// statement prefix. This source relation grants neither a loan nor a write.
    pub fn reference_expression_origins_before_statement(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        expression: ExpressionHandle,
    ) -> Option<Vec<(SymbolHandle, Vec<facts::PlaceSegment>)>> {
        super::reference_subjects::expression_origins_before_statement(
            self.program,
            machine,
            &self.tables.symbols,
            statement,
            expression,
            &self.caches.state_write_shared_collections,
        )
        .map(|origins| {
            origins
                .into_iter()
                .map(|source| (source.root, source.segments))
                .collect()
        })
    }

    /// Complete conservative backing reads for invalidation. Unlike exact
    /// origins, these may name a whole collection for a window. Callers may
    /// compare overlap only, never derive predicate or storage identity.
    pub fn reference_expression_read_paths_before_statement(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        super::reference_subjects::expression_read_paths_before_statement(
            self.program,
            machine,
            &self.tables.symbols,
            statement,
            expression,
            &self.caches.state_write_shared_collections,
        )
    }

    /// Persistent field/element declarations of the original lender and every
    /// retained view, without observing write-only contents or granting access.
    pub(crate) fn reference_lender_declarations_before_statement(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        expression: ExpressionHandle,
    ) -> Result<Vec<typed_trees::types::TypeReferenceHandle>, &'static str> {
        super::reference_subjects::expression_lender_declarations_before_statement(
            self.program,
            machine,
            &self.tables.symbols,
            statement,
            expression,
            &self.caches.state_write_shared_collections,
        )
    }

    /// Preserve all exact captured alternatives instead of requiring a
    /// singleton identity when the consumer accepts an origin family.
    pub fn local_reference_origins_before_statement(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        local: SymbolHandle,
    ) -> Option<Vec<(SymbolHandle, Vec<facts::PlaceSegment>)>> {
        super::reference_subjects::local_origins(
            self.program,
            machine,
            &self.tables.symbols,
            statement,
            local,
            &self.caches.state_write_shared_collections,
        )
        .map(|origins| {
            origins
                .into_iter()
                .map(|source| (source.root, source.segments))
                .collect()
        })
    }

    /// Complete storage alternatives for a whole reference result, relative
    /// to the selected state's exact declarations. This uses the result
    /// relation, including internal jumps and chronological alias capture;
    /// a may-write footprint cannot establish reference provenance. Consumers
    /// still authenticate projected types, access and live loan custody.
    pub(crate) fn reference_result_origins(
        &self,
        state: SymbolHandle,
    ) -> Option<Vec<LocalWriteSource>> {
        memoized(&self.caches.reference_result_origins, state, || {
            let state = self.program.state_by_symbol(state)?;
            let machine = self.program.machine_holding_state(state.symbol)?;
            super::transparent_results::transparent_callee_result_origins(
                self.program,
                machine,
                state,
                &self.tables.symbols,
                &mut FrameInference::default(),
            )
            .map(|origins| {
                origins
                    .into_iter()
                    .map(|origin| LocalWriteSource::from(origin.place))
                    .collect()
            })
        })
    }

    /// Complete returned leaf sources retain the frame owner's declaration
    /// coordinates and admitted-formation candidates, including re-views and
    /// state/call substitution. Publication independently joins each candidate
    /// to its captured returned loan and actual checked value formation.
    pub fn reference_result_leaf_origins(
        &self,
        state: SymbolHandle,
    ) -> Option<Vec<ReferenceResultLeafOrigin>> {
        memoized(&self.caches.reference_result_leaves, state, || {
            self.reference_result_transports(state)?
                .into_iter()
                .map(|transport| match transport {
                    super::ReferenceResultTransport::Reference(leaf) => Some(leaf),
                    // Flat consumers cannot choose the active payload of an
                    // opaque moved value. Capture-aware consumers use the
                    // transport relation and retain that input identity.
                    super::ReferenceResultTransport::Value { .. } => None,
                })
                .collect()
        })
    }

    /// A complete may-write frame does not exempt reference-binding exposure.
    /// Check only this operand and its preceding statement prefix, not later
    /// operands or statements. This query never adds write permissions.
    pub fn expression_reference_bindings_are_stable(
        &self,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> bool {
        memoized(
            &self.caches.stable_expression_bindings,
            (machine.symbol, expression),
            || {
                super::reference_subjects::bindings::are_stable_at_site(
                    self.program,
                    machine,
                    &self.tables.symbols,
                    CallerWriteSite::Expression(expression),
                    &self.caches.state_write_shared_collections,
                )
                .is_some()
            },
        )
    }

    pub fn call_reference_bindings_are_stable(&self, machine: &Machine, call: &TableCall) -> bool {
        memoized(
            &self.caches.stable_call_bindings,
            (machine.symbol, std::ptr::from_ref(call).addr()),
            || {
                super::reference_subjects::bindings::are_stable_at_site(
                    self.program,
                    machine,
                    &self.tables.symbols,
                    CallerWriteSite::Call(call),
                    &self.caches.state_write_shared_collections,
                )
                .is_some()
            },
        )
    }

    /// Reference-free erased value shape; unlike runtime layout, inline proof
    /// recursion does not make a value capable of aliasing caller storage.
    pub fn proof_value_is_caller_isolated(
        &self,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        memoized(&self.caches.caller_isolated_proof_values, reference, || {
            super::isolation::type_is_caller_isolated_proof_value(self.program, reference)
        })
    }

    /// A fully known owned value cannot observe a write through an alias.
    /// Opaque data and reference-bearing storage need independent origins.
    pub fn value_is_reference_free(
        &self,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        super::type_capabilities::type_is_reference_free_value(self.program, reference, &[])
    }

    /// Checked storage with no exclusive reference leaves writes only its own
    /// local place. Other reference-bearing storage still requires prefix origins.
    pub fn local_requires_write_origin(
        &self,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        memoized(&self.caches.write_origin_requirements, reference, || {
            super::type_may_carry_write(self.program, reference)
                && !super::type_is_caller_isolated_local(self.program, reference)
                && super::stored_origins::declared_origins(
                    self.program,
                    SymbolHandle::invalid(),
                    "",
                    reference,
                )
                .is_none_or(|origins| !origins.references.is_empty())
        })
    }

    pub fn assignment_write_target(
        &self,
        current_machine: &Machine,
        statement: &StatementNode,
    ) -> Option<AssignmentWriteTarget> {
        memoized(
            &self.caches.assignment_targets,
            (current_machine.symbol, std::ptr::from_ref(statement).addr()),
            || {
                assignment_write_target(
                    self.program,
                    current_machine,
                    &self.tables.symbols,
                    statement,
                    &|state| self.state_assignments(current_machine, state),
                )
            },
        )
    }

    /// Whether RHS execution preserves the address inputs needed to name the
    /// completed store by its current destination expression. A complete write
    /// footprint alone supplies no such correspondence.
    pub fn assignment_destination_remains_current(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> bool {
        let Some(assignment) = statement.as_assignment() else {
            return false;
        };
        let writes = self.expression_may_write_paths(machine, assignment.value);
        super::assignment_addresses::destination_remains_current(
            self.program,
            machine,
            statement,
            writes.as_deref(),
            None,
        )
        .is_some()
    }

    /// Establishment under a completed captured coordinate, rather than the
    /// authored alias which RHS execution may already have redirected. The
    /// caller supplies a destination from this statement's store capture;
    /// this query verifies stability, not its access or provenance.
    pub fn captured_assignment_destination_remains_current(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        destination: &crate::CanonicalPlace,
    ) -> bool {
        let Some(assignment) = statement.as_assignment() else {
            return false;
        };
        let writes = self.expression_may_write_paths(machine, assignment.value);
        super::assignment_addresses::destination_remains_current(
            self.program,
            machine,
            statement,
            writes.as_deref(),
            Some(destination),
        )
        .is_some()
    }

    /// Performed destinations and reference origins share the captured store
    /// prefix and exposure checks. An empty destination set denotes binding
    /// replacement; unavailable provenance stays `None`.
    pub fn assignment_storage_origins(
        &self,
        machine: &Machine,
        statement: &StatementNode,
    ) -> Option<super::caller_aliases::AssignmentStorageOrigins> {
        super::caller_aliases::assignment_storage_origins(
            self.program,
            machine,
            &self.tables.symbols,
            self.machine_symbols(machine)?.as_ref(),
            statement,
            &|state| self.state_assignments(machine, state),
        )
    }

    /// Finite address substitutions use the existing storage-root SCC law:
    /// a selector-growing dependency cannot lie on a recursive root cycle.
    pub fn storage_origin_substitutions_are_finite(
        &self,
        bindings: &[(SymbolHandle, super::caller_aliases::LocalWriteSource)],
    ) -> bool {
        let mut substitutions = super::cycle_substitutions::Substitutions::default();
        for (target, source) in bindings {
            if !target.is_valid() || !source.root.is_valid() {
                return false;
            }
            let from = substitutions.parameter(self.program.symbols.get(*target).parent, *target);
            let to =
                substitutions.parameter(self.program.symbols.get(source.root).parent, source.root);
            substitutions.insert(
                from,
                to,
                !source.segments.is_empty() || !source.recasts.is_empty(),
            );
        }
        substitutions.is_finite()
    }

    fn state_assignments(
        &self,
        machine: &Machine,
        state: &typed_trees::state::State,
    ) -> std::sync::Arc<Vec<(usize, super::state_write_walk::AssignmentPrefix)>> {
        memoized(
            &self.caches.state_assignments,
            (machine.symbol, state.symbol),
            || {
                std::sync::Arc::new(super::state_write_walk::walk_state_assignments(
                    self.program,
                    machine,
                    state,
                    &self.tables.symbols,
                ))
            },
        )
    }

    /// Direct store only; operand calls have their own value-expression frame.
    pub fn assignment_write_frame(
        &self,
        current_machine: &Machine,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        memoized(
            &self.caches.assignment_frames,
            (current_machine.symbol, std::ptr::from_ref(statement).addr()),
            || {
                self.machine_symbols(current_machine)
                    .and_then(|machine_symbols| {
                        super::caller_aliases::assignment_write_paths(
                            self.program,
                            current_machine,
                            &self.tables.symbols,
                            &machine_symbols,
                            statement,
                            &|state| self.state_assignments(current_machine, state),
                        )
                    })
                    .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
            },
        )
    }

    /// Recover the prefix reference inventory shared with inferred state frames.
    /// Unknown sources stay attached to their leaf selectors. Consumers may
    /// select disjoint storage, but an overlapping unknown must remain opaque;
    /// exact-origin queries also retain unknown candidates when checking uniqueness.
    /// `None` means the prefix cannot establish even this complete inventory.
    pub fn local_write_origins_before_statement(
        &self,
        current_machine: &Machine,
        statement: &StatementNode,
    ) -> Option<Vec<LocalWriteOrigin>> {
        memoized(
            &self.caches.local_write_origins,
            (current_machine.symbol, std::ptr::from_ref(statement).addr()),
            || {
                local_write_origins_before_statement(
                    self.program,
                    current_machine,
                    &self.tables.symbols,
                    statement,
                    &self.caches.state_write_collections,
                )
            },
        )
    }

    pub fn new(program: &'program dyn crate::CheckingInput) -> Option<Self> {
        // A frozen context shares one set of tables; a bare program builds its own.
        let tables = match program.call_frame_tables() {
            Some(tables) => tables,
            None => std::sync::Arc::new(CallFrameTables::build(program)?),
        };
        Some(Self {
            program,
            tables,
            caches: crate::frozen_program::frozen_program_memos(program)
                .map(|memos| memos.call_frames.clone())
                .unwrap_or_default(),
        })
    }

    /// The machine's member/state symbol table, built once per machine for
    /// this resolver. `None` is the build's diagnostics verdict — the exact
    /// contract the inline `diagnostics.is_empty()` checks encoded.
    fn machine_symbols(
        &self,
        machine: &'program Machine,
    ) -> Option<std::sync::Arc<MachineSymbols<'program>>> {
        self.tables
            .machine_symbols
            .get(&machine.symbol)
            .cloned()
            .flatten()
    }

    /// Whole-program call plans, derived once per resolver and shared by
    /// every operand call-candidate check this resolver serves.
    pub fn call_plans(
        &self,
    ) -> std::sync::Arc<(
        flow_effects::OperationalPlan,
        flow_effects::ServiceReachInferencePlan,
    )> {
        self.caches.call_plans(self.program)
    }

    pub fn may_write_paths(
        &self,
        current_machine: &'program Machine,
        call: &'program TableCall,
    ) -> Option<Vec<String>> {
        self.may_write_frame(current_machine, call)
            .into_complete_paths()
    }

    pub fn may_write_frame(
        &self,
        current_machine: &'program Machine,
        call: &'program TableCall,
    ) -> NormalizedWriteFrame {
        let cache_key = (
            current_machine.symbol.arena_index(),
            current_machine.symbol.generation(),
            std::ptr::from_ref(call).addr(),
        );
        if let Ok(cache) = self.caches.statement_calls.lock()
            && let Some(frame) = cache.get(&cache_key)
        {
            return frame.clone();
        }

        let frame = if let Some(machine_symbols) = self.machine_symbols(current_machine) {
            with_caller_origins(
                    self.program,
                    current_machine,
                    &self.tables.symbols,
                    CallerWriteSite::Call(call),
                    &self.caches.state_write_collections,
                    |inference, prefix| {
                        let arguments = self
                            .program
                            .statement_table
                            .expression_handles(call.arguments);
                        // A synthesized wire codec frames from its borrowed
                        // arguments; its type-name receiver is not a place the
                        // ownership floor may poison. A divergent argument is
                        // resolved rather than refused: the resolver takes the
                        // candidate set and unions every proven referent, and
                        // its own name arm keeps an UNPROVEN binding opaque, so
                        // refusing here only discarded sets it could resolve.
                        if super::wire_codecs::is_wire_codec_call(self.program, call) {
                            return super::wire_codecs::known_wire_codec_call_written_paths(
                                self.program,
                                current_machine,
                                call,
                                prefix.parameters,
                                prefix.isolated_locals,
                                prefix.aliases,
                                prefix.divergent,
                                &self.tables.symbols,
                                inference,
                            );
                        }
                        let receiver = crate::machine_calls::calls::signature_queries::call_receiver_member_names(
                            self.program,
                            call,
                        );
                        if receiver.first().is_some_and(|root| {
                            prefix.divergent.iter().any(|(name, _)| name == root)
                        }) {
                            return None;
                        }
                        let argument_types = super::call_targets::call_argument_types(
                            self.program,
                            current_machine,
                            call.target_symbol,
                            call.target.as_str(),
                            &receiver,
                            CallerWriteSite::Call(call),
                            &self.tables.symbols,
                        );
                        let argument_origins = prefix.argument_origins(
                            self.program,
                            current_machine,
                            &machine_symbols,
                            &self.tables.symbols,
                            inference,
                            arguments,
                            &argument_types,
                        )?;
                        let has_divergent_actual = arguments
                            .iter()
                            .any(|argument| prefix.mentions_divergent(self.program, *argument));
                        let known = known_call_written_paths(
                            self.program,
                            call,
                            current_machine,
                            &machine_symbols,
                            &self.tables.symbols,
                            inference,
                            Some(&argument_origins),
                        );
                        let probe_paths = |paths: KnownWrittenPathsForParts,
                                         inference: &mut FrameInference| {
                            paths(
                                self.program,
                                current_machine,
                                &machine_symbols,
                                &self.tables.symbols,
                                &receiver,
                                call.target.as_str(),
                                CallerWriteSite::Call(call),
                                arguments,
                                inference,
                            )
                        };
                        known
                        .or_else(|| {
                            probe_paths(known_boundary_call_written_paths_for_parts, inference)
                        })
                        .or_else(|| {
                            probe_paths(
                                super::boundary_calls::known_requirement_call_written_paths_for_parts,
                                inference,
                            )
                        })
                        .or_else(|| {
                            if has_divergent_actual { return None; }
                            conservative_call_written_paths(
                                self.program,
                                current_machine,
                                call,
                                &machine_symbols,
                                &self.tables.symbols,
                            )
                        })
                    },
                )
                .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
        } else {
            NormalizedWriteFrame::opaque()
        };
        if let Ok(mut cache) = self.caches.statement_calls.lock() {
            cache.insert(cache_key, frame.clone());
        }
        frame
    }

    /// Conservative aggregate frame of every value-position call nested in
    /// `expression`. `Some([])` means the expression is call-free; `None`
    /// means at least one call is opaque, so consumers must fail closed.
    pub fn expression_may_write_paths(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> Option<Vec<String>> {
        self.expression_write_frame(current_machine, expression)
            .into_complete_paths()
    }

    /// A declaration without an executable caller can retain observations
    /// only when every selected invocation has a complete empty frame in its
    /// own namespace. No alias translation is needed for an empty frame;
    /// opaque or nonempty frames establish nothing about caller storage.
    pub fn declaration_expression_has_empty_write_frame(
        &self,
        expression: ExpressionHandle,
        builtin_operation: &impl Fn(ExpressionHandle) -> bool,
    ) -> bool {
        let mut expressions = Vec::new();
        crate::expression_traversal::collect_expression_tree(
            self.program,
            expression,
            &mut expressions,
        );
        expressions.into_iter().all(|expression| {
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Atomic(_) => false,
                ExpressionNode::Binary(_) => builtin_operation(expression),
                ExpressionNode::Call(call) => {
                    // An integer projection is a proof operation, not one of
                    // the executable value builtins. Its argument is still
                    // visited independently; placement, custody and checked
                    // denotation do not follow from this empty local frame.
                    if crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                        self.program,
                        expression,
                    )
                    .is_some()
                        || value_builtin_has_empty_write_frame(self.program, call)
                        || crate::mathematical_applications::selected_definition(self.program, call)
                            .is_some()
                        || crate::machine_calls::denotational_calls::abstract_value_call_signature(
                            self.program,
                            call.target_symbol,
                        )
                        .is_some()
                    {
                        return true;
                    }
                    super::machine_state_by_symbol(self.program, call.target_symbol).is_some_and(
                        |(machine, state)| {
                            self.inferred_state_write_frame(machine, state)
                                .into_complete_paths()
                                .is_some_and(|paths| paths.is_empty())
                        },
                    )
                }
                _ => true,
            }
        })
    }

    pub fn expression_write_frame(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> NormalizedWriteFrame {
        memoized(
            &self.caches.expression_frames,
            (current_machine.symbol, expression),
            || self.expression_write_frame_uncached(current_machine, expression),
        )
    }

    /// The effect at operation completion. Operands have already executed in
    /// the proof observer; callers that have not walked them need the complete
    /// expression frame instead. Unknown chronological origins remain opaque.
    pub(crate) fn operation_write_frame(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> NormalizedWriteFrame {
        let Some(machine_symbols) = self.machine_symbols(current_machine) else {
            return NormalizedWriteFrame::opaque();
        };
        // Operands have completed: substitute their captured origins, not the
        // statement prefix's old descriptor slots. Unknown origins remain in
        // their exact positions and matter only if a relative write uses them.
        let completed_arguments = if self
            .program
            .expression_table
            .call_node(expression)
            .is_some()
        {
            let Some(origins) = self.expression_call_write_origins(current_machine, expression)
            else {
                return NormalizedWriteFrame::opaque();
            };
            if origins.receiver.is_none() {
                return NormalizedWriteFrame::opaque();
            }
            Some(
                origins
                    .arguments
                    .into_iter()
                    .map(|origins| {
                        origins.map(|sources| {
                            sources
                                .iter()
                                .map(LocalWriteSource::frame_origin)
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let written = super::caller_aliases::with_caller_origin_context(
            self.program,
            current_machine,
            &self.tables.symbols,
            CallerWriteSite::Expression(expression),
            &self.caches.state_write_collections,
            |inference, prefix| {
                let mut written = Vec::new();
                collect_operation_written_paths(
                    self.program,
                    expression,
                    current_machine,
                    &machine_symbols,
                    &self.tables.symbols,
                    inference,
                    &mut written,
                    prefix,
                    completed_arguments.as_deref(),
                )?;
                prefix.close_observed_paths(self.program, written)
            },
        );
        written.map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    fn expression_write_frame_uncached(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> NormalizedWriteFrame {
        let Some(machine_symbols) = self.machine_symbols(current_machine) else {
            return NormalizedWriteFrame::opaque();
        };
        if !expression_has_calls(self.program, expression)
            && !super::caller_aliases::expression_any(self.program, expression, |child| {
                crate::value_custody::atomic_operations::atomic_once_carrier(self.program, child)
                    .is_some()
            })
        {
            return NormalizedWriteFrame::complete(Vec::new());
        }
        with_caller_origins(
            self.program,
            current_machine,
            &self.tables.symbols,
            CallerWriteSite::Expression(expression),
            &self.caches.state_write_collections,
            |inference, prefix| {
                let mut written = Vec::new();
                collect_expression_call_written_paths(
                    self.program,
                    expression,
                    current_machine,
                    &machine_symbols,
                    &self.tables.symbols,
                    inference,
                    &mut written,
                    prefix,
                )?;
                Some(written)
            },
        )
        .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    /// Aggregate only the value-position calls embedded in a statement. The
    /// statement-position call itself is handled separately by
    /// `may_write_paths`; its receiver is a path, not an evaluated expression.
    pub fn statement_value_may_write_paths(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> Option<Vec<String>> {
        self.statement_value_write_frame(current_machine, statement)
            .into_complete_paths()
    }

    pub(crate) fn statement_value_may_write_paths_for_symbols(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
    ) -> Option<Vec<String>> {
        self.statement_value_write_frame_for_symbols(current_machine, machine_symbols, statement)
            .into_complete_paths()
    }

    pub fn statement_value_write_frame(
        &self,
        current_machine: &'program Machine,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let Some(machine_symbols) = self.machine_symbols(current_machine) else {
            return NormalizedWriteFrame::opaque();
        };
        self.statement_value_write_frame_for_symbols(current_machine, &machine_symbols, statement)
    }

    fn statement_value_write_frame_for_symbols(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        memoized(
            &self.caches.statement_value_frames,
            (current_machine.symbol, std::ptr::from_ref(statement).addr()),
            || {
                self.statement_value_write_frame_uncached(
                    current_machine,
                    machine_symbols,
                    statement,
                )
            },
        )
    }

    fn statement_value_write_frame_uncached(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
    ) -> NormalizedWriteFrame {
        let expressions = statement_value_expression_roots(self.program, statement);
        self.statement_expression_roots_write_frame(
            current_machine,
            machine_symbols,
            statement,
            &expressions,
        )
    }

    fn statement_expression_roots_write_frame(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
        expressions: &[ExpressionHandle],
    ) -> NormalizedWriteFrame {
        if !expressions
            .iter()
            .any(|expression| expression_has_calls(self.program, *expression))
        {
            return NormalizedWriteFrame::complete(Vec::new());
        }
        with_caller_origins(
            self.program,
            current_machine,
            &self.tables.symbols,
            CallerWriteSite::Statement(statement),
            &self.caches.state_write_collections,
            |inference, prefix| {
                let mut written = Vec::new();
                for &expression in expressions {
                    collect_expression_call_written_paths(
                        self.program,
                        expression,
                        current_machine,
                        machine_symbols,
                        &self.tables.symbols,
                        inference,
                        &mut written,
                        prefix,
                    )?;
                }
                Some(written)
            },
        )
        .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
    }

    /// Body-derived frame in the target state's own namespace. `self` remains
    /// `self`; non-self state parameters normalize positionally as `$P<N>`, so
    /// source renames and discovery order do not perturb implementation identity.
    pub fn inferred_state_write_frame(
        &self,
        machine: &'program Machine,
        state: &'program State,
    ) -> NormalizedWriteFrame {
        memoized(&self.caches.inferred_state_frames, state.symbol, || {
            if !self
                .program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
            {
                return NormalizedWriteFrame::opaque();
            }
            self.infer_state_write_frame(machine, state)
        })
    }

    /// Body-derived frames for every state in one machine, in source order.
    /// Complete acyclic and solved cycle summaries are independent of the
    /// requesting root, so sibling queries share their memo. Opaque results
    /// and contextual depth-first prefixes never enter that memo.
    pub fn inferred_machine_state_write_frames(
        &self,
        machine: &'program Machine,
    ) -> Vec<NormalizedWriteFrame> {
        let frames = memoized(&self.caches.inferred_machine_frames, machine.symbol, || {
            std::sync::Arc::new(
                self.program
                    .machine_states(machine)
                    .iter()
                    .map(|state| self.infer_state_write_frame(machine, state))
                    .collect(),
            )
        });
        (*frames).clone()
    }

    fn infer_state_write_frame(
        &self,
        machine: &'program Machine,
        state: &'program State,
    ) -> NormalizedWriteFrame {
        let mut inference = FrameInference::for_state(state.symbol);
        let relative_paths = summarize_state_written_paths(
            self.program,
            machine,
            state,
            &self.tables.symbols,
            &mut inference,
        )
        .or_else(|| {
            summarize_state_written_paths_for_cycles(
                self.program,
                machine,
                state,
                &self.tables.symbols,
                &inference,
            )
        });
        let Some(relative_paths) = relative_paths else {
            return NormalizedWriteFrame::opaque();
        };
        let mut normalized = Vec::new();
        for relative in relative_paths {
            match normalize_state_relative_path(self.program, state, &relative) {
                Some(Some(path)) => normalized.push(path),
                Some(None) => {}
                None => return NormalizedWriteFrame::opaque(),
            }
        }
        NormalizedWriteFrame::complete(normalized)
    }
}

/// Parent/child places overlap in both directions. A collection-content scope
/// overlaps all its elements, but not the collection descriptor's `.len`.
pub fn frame_paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left.strip_suffix("[*]").is_some_and(|collection| {
            right
                .strip_prefix(collection)
                .is_some_and(|suffix| suffix.starts_with('['))
        })
        || right.strip_suffix("[*]").is_some_and(|collection| {
            left.strip_prefix(collection)
                .is_some_and(|suffix| suffix.starts_with('['))
        })
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
}

pub(crate) fn statement_value_expression_roots(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    crate::value_custody::placed_views::statement_expression_roots(program, statement, true)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_expression_call_written_paths(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    written: &mut Vec<String>,
    origins: &CallOriginContext<'_>,
) -> Option<()> {
    if !expression.is_valid() {
        return Some(());
    }
    let mut complete = Some(());
    crate::expression_traversal::executable_children(
        program,
        program.expression_table.expression(expression),
        |child| {
            if complete.is_some() {
                complete = collect_expression_call_written_paths(
                    program,
                    child,
                    current_machine,
                    machine_symbols,
                    symbols,
                    inference,
                    written,
                    origins,
                );
            }
        },
    );
    complete?;
    collect_operation_written_paths(
        program,
        expression,
        current_machine,
        machine_symbols,
        symbols,
        inference,
        written,
        origins,
        None,
    )
}

/// One operation's own effects, excluding already evaluated operands. Aggregate
/// footprint queries and chronological proof execution share this resolver;
/// applying a subtree footprint at each completion would erase child guarantees
/// a second time after an outer call that never writes their subjects.
#[allow(clippy::too_many_arguments)]
fn collect_operation_written_paths(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    written: &mut Vec<String>,
    origins: &CallOriginContext<'_>,
    completed_arguments: Option<&[Option<Vec<super::FramePlaceOrigin>>]>,
) -> Option<()> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            if crate::value_custody::atomic_operations::atomic_once_operands(program, atomic)
                .is_some()
            {
                // Operand calls complete before the attempt. This is a may-write
                // footprint only; even a publishing success ordering proves no
                // successful exchange on the mismatch/uncommitted paths.
                let (state, _, _) = super::caller_aliases::caller_statement_owner(
                    program,
                    current_machine,
                    CallerWriteSite::Expression(expression),
                )?;
                let place = crate::value_custody::atomic_operations::atomic_once_write_place(
                    program,
                    current_machine,
                    state,
                    expression,
                )?;
                let path = coarse_place_path(program, place)?;
                let paths = super::stored_origins::expand_write_paths(
                    program,
                    &[path],
                    origins.aliases,
                    origins.stored,
                )?;
                for path in origins.close_paths(program, paths) {
                    if !written.contains(&path) {
                        written.push(path);
                    }
                }
            }
        }
        ExpressionNode::Call(call) => {
            // Queries observe their already evaluated operands. Use the
            // same resolved intrinsic identity as stored-call effects;
            // an ordinary same-spelled callable keeps its inferred frame.
            if crate::compiler_operations::expression_call_only_observes_operands(program, call) {
                return Some(());
            }
            // Named operators may acquire their exact identity after initial
            // symbol resolution. Reuse ordinary call selection before asking
            // the signature owner for its write ceiling; a namespace is not
            // mutable caller storage, and an unresolved call stays opaque.
            let target_symbol = typed_trees::operator::resolve_named_expression_call(program, call)
                .map_or(call.target_symbol, |operator| operator.symbol);
            let receiver_members = if call.receiver.is_valid() {
                receiver_member_chain(program, call.receiver)
            } else {
                Some(Vec::new())
            };
            // Reserved value builtins are operand operations, not machine
            // calls. They may read their operands, but they do not write
            // caller storage. Keep this list aligned with the value
            // call validation exemptions below so frame consumers do not turn
            // `min`/`max` reductions into opaque whole-receiver clobbers.
            // A boundary member with the same spelling still has its declared
            // receiver and argument reach; spelling cannot bypass resolution.
            if (typed_trees::byte_predicates::byte_predicate_call(
                program.typed_program(),
                expression,
            )
            .is_some()
                || value_builtin_has_empty_write_frame(program, call))
                && !super::boundary_calls::selected_boundary_signature(program, call.target_symbol)
                && !super::boundary_calls::expression_receiver_requires_boundary_frame(
                    program,
                    current_machine,
                    call.receiver,
                )
                && super::machine_state_by_symbol(program, call.target_symbol).is_none()
                && super::boundary_calls::requirement_signature_for_site(
                    program,
                    current_machine,
                    receiver_members.as_deref().unwrap_or(&[]),
                    call.target.as_str(),
                    super::caller_aliases::CallerWriteSite::Expression(expression),
                )
                .is_none()
                && !receiver_members.as_deref().is_some_and(|receiver| {
                    receiver_requires_boundary_frame(
                        program,
                        current_machine,
                        machine_symbols,
                        symbols,
                        receiver,
                    )
                })
            {
                return Some(());
            }
            let exact_receiver = receiver_members.is_some();
            // A computed call or match receiver has no member spelling, but a
            // proven divergent candidate set plus its declared referent data
            // may still resolve the callee by type. Everything else without a
            // resolvable target keeps failing closed.
            if !exact_receiver
                && super::machine_state_by_symbol(program, call.target_symbol).is_none()
                && super::boundary_calls::requirement_signature_by_target(
                    program,
                    call.target_symbol,
                )
                .is_none()
                && !matches!(
                    program.expression_table.expression(call.receiver),
                    ExpressionNode::Call(_) | ExpressionNode::Match(_)
                )
            {
                return None;
            }
            let (receiver_members, receiver_origins, receiver_data_name, authority_receiver) =
                super::receiver_frame_origins(
                    program,
                    current_machine,
                    expression,
                    symbols,
                    inference,
                )?;
            if !exact_receiver
                && !authority_receiver
                && !super::call_trees::receiver_expression_preserves_origin(
                    program,
                    current_machine,
                    call.receiver,
                    machine_symbols,
                    symbols,
                    inference,
                )
            {
                return None;
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            let argument_types = super::call_targets::call_argument_types(
                program,
                current_machine,
                target_symbol,
                call.target.as_str(),
                &receiver_members,
                CallerWriteSite::Expression(expression),
                symbols,
            );
            let argument_origins = match completed_arguments {
                Some(captured) if captured.len() == arguments.len() => captured.to_vec(),
                Some(_) => return None,
                None => origins.argument_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    inference,
                    arguments,
                    &argument_types,
                )?,
            };
            let has_divergent_actual = arguments
                .iter()
                .any(|argument| origins.mentions_divergent(program, *argument));
            let probe_paths = |paths: KnownWrittenPathsForParts, inference: &mut FrameInference| {
                paths(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    &receiver_members,
                    call.target.as_str(),
                    CallerWriteSite::Expression(expression),
                    arguments,
                    inference,
                )
            };
            let paths = known_call_written_paths_for_parts(
                program,
                target_symbol,
                call.target.as_str(),
                &receiver_members,
                &receiver_origins,
                receiver_data_name.as_deref(),
                arguments,
                current_machine,
                machine_symbols,
                symbols,
                inference,
                Some(&argument_origins),
            )
            .or_else(|| {
                if !exact_receiver && !authority_receiver {
                    return None;
                }
                probe_paths(known_boundary_call_written_paths_for_parts, inference)
            })
            .or_else(|| {
                // Requirement receivers resolve by their single chain or
                // origin; a divergent candidate set names neither.
                probe_paths(
                    super::boundary_calls::known_requirement_call_written_paths_for_parts,
                    inference,
                )
            })
            // Even when the callee body is opaque (transitioning, cyclic,
            // static-machine, or unresolved), ownership still gives a sound
            // caller-visible floor: it cannot mutate an unpassed caller local.
            // Conservatively poison the whole receiver (`self` for an implicit
            // receiver) plus every explicit mutable argument.
            .or_else(|| {
                if !exact_receiver || has_divergent_actual {
                    return None;
                }
                syntactic_call_written_paths(
                    program,
                    current_machine,
                    &receiver_members,
                    arguments,
                    machine_symbols,
                    symbols,
                )
            })?;
            for path in origins.close_paths(program, paths) {
                if !written.contains(&path) {
                    written.push(path);
                }
            }
        }
        _ => {}
    }
    Some(())
}

fn value_builtin_has_empty_write_frame(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    // One roster for every value builtin, including the float intrinsics that
    // selected execution settles an authored call into after checking: the
    // settled body must derive the same frame the authored call did. Value
    // builtins are free functions: an unresolved method cannot acquire their
    // empty frame.
    !call.receiver.is_valid()
        && program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .is_some_and(|builtin| {
                builtin.has_empty_write_frame()
                    // Flag snapshot and restore use separately admitted
                    // temporary stack storage. They write no caller Omega
                    // place; the place keeps the flag requirements a following
                    // label reads.
                    || language_core::inline_assembly::asm_intrinsic_retains_flag_place(
                        builtin.name(),
                    )
            })
}

pub(super) fn syntactic_call_written_paths(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    if receiver_requires_boundary_frame(
        program,
        current_machine,
        machine_symbols,
        symbols,
        receiver_members,
    ) {
        return None;
    }
    let mut written = vec![if receiver_members.is_empty() {
        "self".to_owned()
    } else {
        receiver_members.join(".")
    }];
    for argument in arguments {
        let place = match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(place) if place.access.is_exclusive() => place.target,
            ExpressionNode::Borrow(_) => continue,
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                *argument
            }
            ExpressionNode::StructLiteral(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Call(_) => return None,
            _ => continue,
        };
        let path = coarse_place_path(program, place)?;
        if !written.contains(&path) {
            written.push(path);
        }
    }
    Some(written)
}

/// Ownership-derived caller-visible ceiling used when body inference or a
/// non-boundary call cannot provide a narrower complete frame. The receiver
/// and every place-shaped argument conservatively cover the caller places the
/// call could mutate; without a resolved signature, even a by-value place is
/// retained rather than guessed immutable. Rejected trait calls cannot use
/// this fallback. Aggregate literals and unproven call results also remain
/// opaque: their reachable references are not described by producer writes.
pub(crate) fn conservative_call_written_paths(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    call: &TableCall,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    let receiver_members =
        crate::machine_calls::calls::signature_queries::call_receiver_member_names(program, call);
    syntactic_call_written_paths(
        program,
        current_machine,
        &receiver_members,
        program.statement_table.expression_handles(call.arguments),
        machine_symbols,
        symbols,
    )
}
