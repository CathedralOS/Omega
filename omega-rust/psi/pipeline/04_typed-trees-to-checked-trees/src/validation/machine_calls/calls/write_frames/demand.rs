//! Public and crate-private demand facade for caller-visible write frames.
//!
//! The facade gathers expression and statement call demand and delegates exact
//! complete-or-opaque summaries to its parent frame engine. It does not own
//! call validation, alias-origin inference, or transition fixed points.

use super::boundary_calls::receiver_requires_boundary_frame;
use super::caller_aliases::{
    AssignmentWriteTarget, LocalWriteOrigin, assignment_write_target,
    local_write_origins_before_statement,
};
use super::caller_aliases::{
    CallOriginContext, CallerWriteSite, expression_has_calls, with_caller_origins,
};
use super::{
    coarse_place_path, known_boundary_call_written_paths_for_parts,
    known_call_written_paths_for_parts_with_origins, known_call_written_paths_with_summaries,
    normalize_state_relative_path, receiver_member_chain,
};
use crate::fact_plan::NormalizedWriteFrame;
use crate::validation::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::validation::machine_calls::calls::write_frames::FrameInference;
use crate::validation::machine_calls::calls::write_frames::permuted_cycle_frames::summarize_state_written_paths_with_permuted_cycles;
use crate::validation::machine_calls::calls::write_frames::state_write_walk::{
    CollectedStatementPrefix, summarize_state_written_paths,
};
use std::sync::Mutex;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    ExpressionHandle, ExpressionNode,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use symbols::SymbolHandle;
use symbols::SymbolKeyMap as HashMap;

/// Shared conservative call-frame resolver. A complete result is the set of
/// caller-visible places the call may write; `None` is deliberately opaque and
/// requires consumers to invalidate every fact they cannot otherwise prove.
///
/// The resolver owns the top-level symbol cache so validation, proof, recast,
/// and invariant consumers share one resolution law instead of reimplementing
/// call identity. Per-machine caches are built at the query boundary and fail
/// closed if the program's symbols are already invalid.
pub struct CallFrameResolver<'program> {
    program: &'program TypedTrees,
    symbols: TopLevelSymbols<'program>,
    /// The query memos. Inside a frozen-program scope every resolver built
    /// for the scope's program shares one set; see
    /// `enter_frozen_program_scope`.
    caches: std::sync::Arc<CallFrameCaches>,
    /// Per-machine member/state symbol tables, built once with the resolver
    /// so every write-frame query shares them. `None` retains the build's
    /// non-empty-diagnostics verdict so callers keep failing closed.
    machine_symbols: HashMap<SymbolHandle, Option<std::sync::Arc<MachineSymbols<'program>>>>,
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
    local_reference_origins: Mutex<
        HashMap<
            (SymbolHandle, usize, SymbolHandle),
            Option<(SymbolHandle, Vec<crate::fact_plan::PlaceSegment>)>,
        >,
    >,
    stable_expression_bindings: Mutex<HashMap<(SymbolHandle, ExpressionHandle), bool>>,
    stable_call_bindings: Mutex<HashMap<(SymbolHandle, usize), bool>>,
    caller_isolated_proof_values: Mutex<
        HashMap<
            symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
            bool,
        >,
    >,
    write_origin_requirements: Mutex<
        HashMap<
            symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
            bool,
        >,
    >,
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
    /// Successful acyclic state summaries are context-independent relative
    /// frames. Retain them across resolver queries; opaque and cycle fallback
    /// results remain one-shot so the conservative frontier is unchanged.
    complete_state_summaries: Mutex<HashMap<SymbolHandle, Vec<String>>>,
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
    /// consumes to test call-candidate totality. Both are pure over this
    /// resolver's immutable program, so one lazy pair serves every operand
    /// the fixpoint evaluates instead of a fresh whole-program derivation per
    /// call expression.
    call_plans: Mutex<
        Option<
            std::sync::Arc<(
                crate::flow_effects::OperationalPlan,
                crate::flow_effects::ServiceReachInferencePlan,
            )>,
        >,
    >,
}

/// Run `compute` once per key for the resolver's immutable program. The lock
/// is not held across `compute`, so nested resolver queries through it cannot
/// deadlock; concurrent misses may compute twice and publish the same value.
fn memoized<K, V>(cache: &Mutex<HashMap<K, V>>, key: K, compute: impl FnOnce() -> V) -> V
where
    K: Eq + std::hash::Hash,
    V: Clone,
{
    if let Ok(cache) = cache.lock()
        && let Some(hit) = cache.get(&key)
    {
        return hit.clone();
    }
    let value = compute();
    if let Ok(mut cache) = cache.lock() {
        cache.insert(key, value.clone());
    }
    value
}

fn compute_call_plans(
    program: &TypedTrees,
) -> (
    crate::flow_effects::OperationalPlan,
    crate::flow_effects::ServiceReachInferencePlan,
) {
    let operational = crate::validation::infer_operational_may(program);
    let service_reaches = crate::validation::infer_service_reaches(program, &operational);
    (operational, service_reaches)
}

/// Whole-program call plans for operand call-candidate checks. A live
/// resolver shares its once-computed pair; resolver-free sites compute a
/// one-shot pair under the same law.
pub fn operand_call_plans(
    program: &TypedTrees,
    frames: Option<&CallFrameResolver<'_>>,
) -> std::sync::Arc<(
    crate::flow_effects::OperationalPlan,
    crate::flow_effects::ServiceReachInferencePlan,
)> {
    match frames {
        Some(frames) => frames.call_plans(),
        None => std::sync::Arc::new(compute_call_plans(program)),
    }
}

impl<'program> CallFrameResolver<'program> {
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

    /// Resolve a bare local reference to exact live storage at this prefix.
    /// This query includes shared references, without granting them writes.
    /// Unknown origins, untracked reference slots, and coarse selectors reject.
    pub fn local_reference_origin_before_statement(
        &self,
        machine: &Machine,
        statement: &StatementNode,
        local: SymbolHandle,
    ) -> Option<(SymbolHandle, Vec<crate::fact_plan::PlaceSegment>)> {
        memoized(
            &self.caches.local_reference_origins,
            (machine.symbol, std::ptr::from_ref(statement).addr(), local),
            || {
                super::reference_subjects::local_origin(
                    self.program,
                    machine,
                    &self.symbols,
                    statement,
                    local,
                    &self.caches.state_write_shared_collections,
                )
                .map(|source| (source.root, source.segments))
            },
        )
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
                    &self.symbols,
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
                    &self.symbols,
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
        reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        memoized(&self.caches.caller_isolated_proof_values, reference, || {
            super::isolation::type_is_caller_isolated_proof_value(self.program, reference)
        })
    }

    /// Shared classification only; storage origins still require prefix evidence.
    pub fn local_requires_write_origin(
        &self,
        reference: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        memoized(&self.caches.write_origin_requirements, reference, || {
            super::type_may_carry_write(self.program, reference)
                && !super::type_is_caller_isolated_local(self.program, reference)
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
                    &self.symbols,
                    statement,
                    &|state| self.state_assignments(current_machine, state),
                )
            },
        )
    }

    fn state_assignments(
        &self,
        machine: &Machine,
        state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    ) -> std::sync::Arc<Vec<(usize, super::state_write_walk::AssignmentPrefix)>> {
        memoized(
            &self.caches.state_assignments,
            (machine.symbol, state.symbol),
            || {
                std::sync::Arc::new(super::state_write_walk::walk_state_assignments(
                    self.program,
                    machine,
                    state,
                    &self.symbols,
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
                super::caller_aliases::assignment_write_paths(
                    self.program,
                    current_machine,
                    &self.symbols,
                    statement,
                    &|state| self.state_assignments(current_machine, state),
                )
                .map_or_else(NormalizedWriteFrame::opaque, NormalizedWriteFrame::complete)
            },
        )
    }

    /// Recover the exact prefix origins shared with inferred state frames.
    /// `None` means the prefix cannot establish every write-capable local's
    /// origin; a consumer must not treat such locals as private storage.
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
                    &self.symbols,
                    statement,
                    &self.caches.state_write_collections,
                )
            },
        )
    }

    pub fn new(program: &'program TypedTrees) -> Option<Self> {
        let mut diagnostics = Vec::new();
        let symbols = TopLevelSymbols::build(program, &mut diagnostics);
        diagnostics.is_empty().then_some(Self {
            program,
            symbols,
            caches: crate::validation::frozen_program::frozen_program_memos(program)
                .map(|memos| memos.call_frames.clone())
                .unwrap_or_default(),
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

    /// The machine's member/state symbol table, built once per machine for
    /// this resolver. `None` is the build's diagnostics verdict — the exact
    /// contract the inline `diagnostics.is_empty()` checks encoded.
    fn machine_symbols(
        &self,
        machine: &'program Machine,
    ) -> Option<std::sync::Arc<MachineSymbols<'program>>> {
        self.machine_symbols.get(&machine.symbol).cloned().flatten()
    }

    /// Whole-program call plans, derived once per resolver and shared by
    /// every operand call-candidate check this resolver serves.
    pub fn call_plans(
        &self,
    ) -> std::sync::Arc<(
        crate::flow_effects::OperationalPlan,
        crate::flow_effects::ServiceReachInferencePlan,
    )> {
        if let Ok(cache) = self.caches.call_plans.lock()
            && let Some(plans) = cache.as_ref()
        {
            return plans.clone();
        }
        let plans = std::sync::Arc::new(compute_call_plans(self.program));
        if let Ok(mut cache) = self.caches.call_plans.lock() {
            *cache = Some(plans.clone());
        }
        plans
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

        let frame =
            if let Some(machine_symbols) = self.machine_symbols(current_machine) {
                with_caller_origins(
                    self.program,
                    current_machine,
                    &self.symbols,
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
                                &self.symbols,
                                inference,
                            );
                        }
                        let receiver = self
                            .program
                            .statement_table
                            .name_path_members(call.receiver)
                            .iter()
                            .map(|member| member.as_str().to_owned())
                            .collect::<Vec<_>>();
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
                            &self.symbols,
                        );
                        let argument_origins = prefix.argument_origins(
                            self.program,
                            current_machine,
                            &machine_symbols,
                            &self.symbols,
                            inference,
                            arguments,
                            &argument_types,
                        )?;
                        let has_divergent_actual = arguments
                            .iter()
                            .any(|argument| prefix.mentions_divergent(self.program, *argument));
                        let known =
                            self.with_complete_state_summaries(|complete_state_summaries| {
                                known_call_written_paths_with_summaries(
                                    self.program,
                                    call,
                                    current_machine,
                                    &machine_symbols,
                                    &self.symbols,
                                    complete_state_summaries,
                                    inference,
                                    Some(&argument_origins),
                                )
                            });
                        known
                        .or_else(|| {
                            known_boundary_call_written_paths_for_parts(
                                self.program,
                                current_machine,
                                &machine_symbols,
                                &self.symbols,
                                &receiver,
                                call.target.as_str(),
                                CallerWriteSite::Call(call),
                                arguments,
                                inference,
                            )
                        })
                        .or_else(|| {
                            super::boundary_calls::known_requirement_call_written_paths_for_parts(
                                self.program,
                                current_machine,
                                &machine_symbols,
                                &self.symbols,
                                &receiver,
                                call.target.as_str(),
                                None,
                                CallerWriteSite::Call(call),
                                arguments,
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
                                &self.symbols,
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

    fn expression_write_frame_uncached(
        &self,
        current_machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> NormalizedWriteFrame {
        let Some(machine_symbols) = self.machine_symbols(current_machine) else {
            return NormalizedWriteFrame::opaque();
        };
        if !expression_has_calls(self.program, expression) {
            return NormalizedWriteFrame::complete(Vec::new());
        }
        with_caller_origins(
            self.program,
            current_machine,
            &self.symbols,
            CallerWriteSite::Expression(expression),
            &self.caches.state_write_collections,
            |inference, prefix| {
                let mut written = Vec::new();
                self.with_complete_state_summaries(|complete_state_summaries| {
                    collect_expression_call_written_paths(
                        self.program,
                        expression,
                        current_machine,
                        &machine_symbols,
                        &self.symbols,
                        inference,
                        &mut written,
                        complete_state_summaries,
                        prefix,
                    )
                })?;
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

    pub(crate) fn statement_value_may_write_paths_with_symbols(
        &self,
        current_machine: &'program Machine,
        machine_symbols: &MachineSymbols<'program>,
        statement: &StatementNode,
    ) -> Option<Vec<String>> {
        self.statement_value_write_frame_with_symbols(current_machine, machine_symbols, statement)
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
        self.statement_value_write_frame_with_symbols(current_machine, &machine_symbols, statement)
    }

    fn statement_value_write_frame_with_symbols(
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
        if !expressions
            .iter()
            .any(|expression| expression_has_calls(self.program, *expression))
        {
            return NormalizedWriteFrame::complete(Vec::new());
        }
        with_caller_origins(
            self.program,
            current_machine,
            &self.symbols,
            CallerWriteSite::Statement(statement),
            &self.caches.state_write_collections,
            |inference, prefix| {
                let mut written = Vec::new();
                self.with_complete_state_summaries(|complete_state_summaries| {
                    for expression in expressions {
                        collect_expression_call_written_paths(
                            self.program,
                            expression,
                            current_machine,
                            machine_symbols,
                            &self.symbols,
                            inference,
                            &mut written,
                            complete_state_summaries,
                            prefix,
                        )?;
                    }
                    Some(written)
                })
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
            self.with_complete_state_summaries(|complete_state_summaries| {
                self.inferred_state_write_frame_with_summaries(
                    machine,
                    state,
                    complete_state_summaries,
                )
            })
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
                self.with_complete_state_summaries(|complete_state_summaries| {
                    self.program
                        .machine_states(machine)
                        .iter()
                        .map(|state| {
                            self.inferred_state_write_frame_with_summaries(
                                machine,
                                state,
                                complete_state_summaries,
                            )
                        })
                        .collect()
                }),
            )
        });
        (*frames).clone()
    }

    fn inferred_state_write_frame_with_summaries(
        &self,
        machine: &'program Machine,
        state: &'program State,
        complete_state_summaries: &mut HashMap<SymbolHandle, Vec<String>>,
    ) -> NormalizedWriteFrame {
        let mut inference = FrameInference::for_state(state.symbol);
        let relative_paths = summarize_state_written_paths(
            self.program,
            machine,
            state,
            &self.symbols,
            &mut inference,
            complete_state_summaries,
        )
        .or_else(|| {
            summarize_state_written_paths_with_permuted_cycles(
                self.program,
                machine,
                state,
                &self.symbols,
                &inference,
                complete_state_summaries,
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

    fn with_complete_state_summaries<T>(
        &self,
        resolve: impl FnOnce(&mut HashMap<SymbolHandle, Vec<String>>) -> T,
    ) -> T {
        match self.caches.complete_state_summaries.lock() {
            Ok(mut summaries) => resolve(&mut summaries),
            Err(_) => resolve(&mut HashMap::default()),
        }
    }
}

/// Parent/child places overlap in both directions: writing `self.item` kills a
/// fact about `self.item.len`, and writing the child kills a whole-value fact.
pub fn frame_paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
}

pub(crate) fn statement_value_expression_roots(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    let mut roots = Vec::new();
    match statement {
        StatementNode::RootBinding(binding) => {
            roots.extend(
                [binding.receiver, binding.implementation_operand]
                    .into_iter()
                    .filter(|expression| expression.is_valid()),
            );
        }
        StatementNode::AssemblyFact(fact) => roots.push(fact.expression),
        StatementNode::Assignment(assignment) => {
            roots.push(assignment.target);
            roots.push(assignment.value);
        }
        StatementNode::Call(call) => roots.extend(
            program
                .statement_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        ),
        StatementNode::Expression(expression) => roots.push(*expression),
        StatementNode::LocalData(local) => roots.push(local.initial_value),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => roots.extend(
                        program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .copied(),
                    ),
                    TransitionTargetNode::Value(value) => roots.push(*value),
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
    roots
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_expression_call_written_paths(
    program: &TypedTrees,
    expression: ExpressionHandle,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    written: &mut Vec<String>,
    complete_state_summaries: &mut HashMap<SymbolHandle, Vec<String>>,
    origins: &CallOriginContext<'_>,
) -> Option<()> {
    if !expression.is_valid() {
        return Some(());
    }
    let mut visit = |child| {
        collect_expression_call_written_paths(
            program,
            child,
            current_machine,
            machine_symbols,
            symbols,
            inference,
            written,
            complete_state_summaries,
            origins,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            visit(dispatch.subject)?;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let symbol_resolved_trees_to_typed_trees::typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    visit(pattern)?;
                }
                visit(arm.value)?;
            }
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value)?,
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                visit(call.receiver)?;
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                visit(*argument)?;
            }
            let receiver_members = if call.receiver.is_valid() {
                receiver_member_chain(program, call.receiver)
            } else {
                Some(Vec::new())
            };
            // Reserved value/view builtins are operand operations, not machine
            // calls. They may read their operands or create a view, but they do
            // not write caller storage. Keep this list aligned with the value
            // call validation exemptions below so frame consumers do not turn
            // `min`/`max` reductions into opaque whole-receiver clobbers.
            // A boundary member with the same spelling still has its declared
            // receiver and argument reach; spelling cannot bypass resolution.
            if value_builtin_has_empty_write_frame(program, call)
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
            let (receiver_members, receiver_origins, receiver_data_name) =
                super::receiver_frame_origins(
                    program,
                    current_machine,
                    call.receiver,
                    symbols,
                    inference,
                )?;
            if !exact_receiver
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
                call.target_symbol,
                call.target.as_str(),
                &receiver_members,
                CallerWriteSite::Expression(expression),
                symbols,
            );
            let argument_origins = origins.argument_origins(
                program,
                current_machine,
                machine_symbols,
                symbols,
                inference,
                arguments,
                &argument_types,
            )?;
            let has_divergent_actual = arguments
                .iter()
                .any(|argument| origins.mentions_divergent(program, *argument));
            let paths = known_call_written_paths_for_parts_with_origins(
                program,
                call.target_symbol,
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
                complete_state_summaries,
            )
            .or_else(|| {
                if !exact_receiver {
                    return None;
                }
                known_boundary_call_written_paths_for_parts(
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
            })
            .or_else(|| {
                // Requirement receivers resolve by their single chain or
                // origin; a divergent candidate set names neither.
                super::boundary_calls::known_requirement_call_written_paths_for_parts(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    &receiver_members,
                    call.target.as_str(),
                    (receiver_origins.len() == 1)
                        .then(|| receiver_origins.first())
                        .flatten(),
                    CallerWriteSite::Expression(expression),
                    arguments,
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
            for path in origins.close_paths(paths) {
                if !written.contains(&path) {
                    written.push(path);
                }
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left)?;
            visit(binary.right)?;
        }
        ExpressionNode::Unary(unary) => visit(unary.operand)?,
        ExpressionNode::Cast(cast) => visit(cast.value)?,
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection)?;
            visit(indexed.index)?;
        }
        ExpressionNode::Member(member) => visit(member.receiver)?,
        ExpressionNode::Borrow(inner) => visit(inner.target)?,
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                visit(*element)?;
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit(field.value)?;
            }
        }
        ExpressionNode::Range(range) => {
            visit(range.start)?;
            visit(range.end)?;
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    Some(())
}

fn value_builtin_has_empty_write_frame(
    program: &TypedTrees,
    call: &symbol_resolved_trees_to_typed_trees::typed_trees::expression::TableCallExpression,
) -> bool {
    if !call.receiver.is_valid() {
        // One roster for every receiver-free value builtin, including the
        // float intrinsics that selected execution settles an authored call
        // into after checking: the settled body must derive the same frame
        // the authored call did.
        return program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .is_some_and(symbols::BuiltinFunction::has_empty_write_frame);
    }
    // View operations remain receiver-bearing builtins. Numeric builtins are
    // free functions: an unresolved method cannot acquire their empty frame.
    language_semantics::declaration_selection::CollectionViewOperation::from_authored_spelling(
        call.target.as_str(),
    )
    .is_some()
        && program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
}

pub(super) fn syntactic_call_written_paths(
    program: &TypedTrees,
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
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCall,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    syntactic_call_written_paths(
        program,
        current_machine,
        &receiver_members,
        program.statement_table.expression_handles(call.arguments),
        machine_symbols,
        symbols,
    )
}

#[cfg(test)]
mod tests {
    use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
    use symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode;
    use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;

    /// Settle the `min` initializer call into a receiver-free float builtin
    /// the way selected execution does after checking: same node, new target.
    fn retarget_initializer_call(
        program: &mut TypedTrees,
        function: symbols::BuiltinFunction,
        receiver: Option<
            symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
        >,
    ) {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let initializer = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) => Some(local.initial_value),
                _ => None,
            })
            .expect("local initializer");
        let symbol = program
            .symbols
            .builtin_function_symbol(function)
            .expect("builtin symbol");
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(initializer)
        else {
            panic!("the initializer is a call");
        };
        call.target =
            symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated(
                function.name(),
            );
        call.target_symbol = symbol;
        if let Some(receiver) = receiver {
            call.receiver = receiver;
        }
    }

    const SOURCE: &str = "machine observe(output: &mut f32, value: f32) { let settled: f32 = min(value, value); output = settled; }";

    #[test]
    fn receiver_free_float_builtin_calls_infer_an_empty_write_frame() {
        for function in [
            symbols::BuiltinFunction::FloatFusedMultiplyAddF32,
            symbols::BuiltinFunction::FloatIsNan,
            symbols::BuiltinFunction::FloatAddTowardZeroF32,
            symbols::BuiltinFunction::Min,
        ] {
            let mut program = crate::validation::front_end::typed_program(SOURCE);
            retarget_initializer_call(&mut program, function, None);
            let machine = &program.machines()[0];
            let state = &program.machine_states(machine)[0];
            let resolver =
                crate::validation::CallFrameResolver::new(&program).expect("frame resolver");
            assert_eq!(
                resolver
                    .inferred_state_write_frame(machine, state)
                    .complete_paths(),
                Some(["$P0".to_owned()].as_slice()),
                "{function:?}: only the assignment to `output` writes caller storage",
            );
        }
    }

    #[test]
    fn receiver_bearing_builtin_calls_still_write_their_receiver() {
        let mut program = crate::validation::front_end::typed_program(SOURCE);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let output = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::Assignment(assignment) => Some(assignment.target),
                _ => None,
            })
            .expect("assignment target names `output`");
        retarget_initializer_call(
            &mut program,
            symbols::BuiltinFunction::FloatIsNan,
            Some(output),
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let resolver = crate::validation::CallFrameResolver::new(&program).expect("frame resolver");
        let frame = resolver.inferred_state_write_frame(machine, state);
        assert!(
            frame.complete_paths().is_none() || frame.paths().iter().any(|path| path == "$P0"),
            "a receiver-bearing call keeps its receiver reach: {frame:?}",
        );
        assert!(
            !symbols::BuiltinFunction::AsmPortOut.has_empty_write_frame()
                && !symbols::BuiltinFunction::ContentSeparate.has_empty_write_frame(),
            "machine-control and content builtins keep their own custody",
        );
    }

    #[test]
    fn boundary_machine_calls_frame_their_exclusive_arguments() {
        // A signature-only callee has no body to summarize: the declared
        // signature's exclusive reach is the complete write contract, so the
        // `&mut slot` actual -- not an empty body summary -- names the frame.
        let program = crate::validation::front_end::typed_program(
            "machine caller(slot: u64) { poke(&mut slot); }
             boundary machine poke(value: &mut u64) -> u64;",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let call_statement = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::Call(call) => Some(call),
                _ => None,
            })
            .expect("statement call");
        let resolver = crate::validation::CallFrameResolver::new(&program).expect("frame resolver");
        assert_eq!(
            resolver
                .may_write_frame(machine, call_statement)
                .complete_paths(),
            Some(["slot".to_owned()].as_slice()),
            "the exclusive parameter's actual origin is the whole frame",
        );
    }

    #[test]
    fn boundary_machine_calls_with_value_parameters_write_nothing() {
        // A boundary declaration whose signature carries no exclusive reach
        // frames to the empty set: the call cannot write caller storage.
        let program = crate::validation::front_end::typed_program(
            "machine caller(slot: u64) { note(slot); }
             boundary machine note(value: u64) -> u64;",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let call_statement = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::Call(call) => Some(call),
                _ => None,
            })
            .expect("statement call");
        let resolver = crate::validation::CallFrameResolver::new(&program).expect("frame resolver");
        assert_eq!(
            resolver
                .may_write_frame(machine, call_statement)
                .complete_paths(),
            Some([].as_slice()),
            "a value-only boundary signature admits no caller write",
        );
    }
}
