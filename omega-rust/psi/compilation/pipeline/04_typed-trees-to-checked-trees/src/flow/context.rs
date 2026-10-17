//! `FlowBuildContext`, the mutable state of one flow build: the output fact arenas
//! that `finish` publishes as `FlowFacts` and `discard_output` clears between
//! sweeps, the incoming state values and dirty set that decide re-sweeps, and memo
//! tables for lookups that depend only on the program, which survive between sweeps.
use crate::flow::StateMutationSummaryCache;
use checked_trees::{
    BorrowFacts, FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts,
    FlowFacts, FlowInvalidationFacts, FlowOwnershipFacts, ProofFacts,
};
use facts::FactPlan;
/// Handle-keyed maps take the integer hasher; none is iterated for output order.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
use std::rc::Rc;
use symbols::SymbolHandle;

/// A source-bound guard contains typed-tree identities, never sweep-local fact
/// handles. Its source occurrence and polarity completely determine binding;
/// the live guard checks and fresh evidence allocation remain at the caller.
struct GuardPreparation {
    occurrence: (
        SymbolHandle,
        usize,
        typed_trees::expression::ExpressionHandle,
        bool,
    ),
    predicate: Option<crate::facts::boolean_predicates::BoundPredicate>,
}

/// Immutable preparation for one authored invocation. Neither result contains
/// live flow contexts: the source, borrow inventory, mutation summaries and
/// exact-prefix alias resolver stay fixed throughout this flow build. Keep
/// unknown writes (`Some(None)`) distinct from not yet prepared (`None`).
#[derive(Default)]
pub(super) struct CallPreparation<'program> {
    pub(super) site: Option<Option<validation::CallSite<'program>>>,
    pub(super) storage_writes: Option<Option<Rc<super::call_phases::CallStorageWrites>>>,
}

/// Source-only answers for one place occurrence. The outer option records
/// whether entry provenance was queried; its inner absence is an unproved
/// origin. Neither result says whether the current evaluated prefix wrote it.
struct PlacePreparation {
    place: Option<validation::CanonicalPlace>,
    entry_subject: Option<
        Option<(
            u32,
            Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        )>,
    >,
}

impl PlacePreparation {
    fn new(
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Self {
        Self {
            place: validation::canonical_place_from_expression_in_state(
                program, state, statement, expression,
            ),
            entry_subject: None,
        }
    }
}

/// Program shape lookups indexed once per context: every machine, state, and
/// their in-machine positions. All values are table positions, so the map is
/// lifetime-free and rebuilds lazily on the first query of a fresh context.
struct SymbolIndex {
    machine_index: HashMap<SymbolHandle, usize>,
    state_index_in_machine: HashMap<(SymbolHandle, SymbolHandle), usize>,
    state_location: HashMap<SymbolHandle, (usize, usize)>,
}

impl SymbolIndex {
    fn build(program: &dyn ::validation::CheckingInput) -> Self {
        let mut index = Self {
            machine_index: HashMap::default(),
            state_index_in_machine: HashMap::default(),
            state_location: HashMap::default(),
        };
        for (machine_index, machine) in program.machines().iter().enumerate() {
            index.machine_index.insert(machine.symbol, machine_index);
            for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                index
                    .state_index_in_machine
                    .insert((machine.symbol, state.symbol), state_index);
                index
                    .state_location
                    .insert(state.symbol, (machine_index, state_index));
            }
        }
        index
    }
}

pub(super) struct FlowBuildContext<'plans> {
    pub(super) scalar_expressions: &'plans checked_trees::CheckedScalarExpressionPlans,
    pub(super) operators: &'plans checked_trees::CheckedOperatorFacts,
    pub(super) exact_integer_casts: &'plans [validation::ExactIntegerCastFact],
    pub(super) call_frames: Option<&'plans validation::CallFrameResolver<'plans>>,
    pub(super) state_value_inputs: Vec<super::state_values::StateValues>,
    pub(super) dirty_state_value_inputs: Vec<SymbolHandle>,
    pub(super) new_state_field_input_height: usize,
    /// Per (state, carrier field) byte-class evidence of this pass's element
    /// stores, intersected across the state's stores: the greatest claim the
    /// state's outgoing edges could carry for the carrier once the carrier's
    /// own premise is satisfied. Captured at store time; read by field
    /// capture as each edge's delivery potential and reset when a state
    /// rebuilds.
    pub(super) element_store_potentials: Vec<(
        SymbolHandle,
        Vec<facts::PlaceSegment>,
        Vec<validation::field_domains::ByteSequencePredicate>,
    )>,
    pub(super) state_mutation_summary_cache: &'plans StateMutationSummaryCache,
    /// Program-global lookups repeated by every call site on every pass. The
    /// typed program is immutable for the whole fixpoint, so each answer
    /// memoizes once and survives `discard_output` like the immutable plans
    /// above -- they hold typed-tree handles only, never sweep-local arena
    /// handles.
    pub(super) call_sites:
        HashMap<(SymbolHandle, SymbolHandle, usize, usize), CallPreparation<'plans>>,
    pub(super) call_target_parameters:
        HashMap<SymbolHandle, Option<&'plans [typed_trees::signature::StateParameter]>>,
    pub(super) call_target_returns:
        HashMap<SymbolHandle, Option<typed_trees::types::TypeReferenceHandle>>,
    pub(super) call_result_identities: HashMap<
        SymbolHandle,
        Rc<
            Vec<(
                Vec<facts::PlaceSegment>,
                SymbolHandle,
                language_semantics::SemanticDomainId,
            )>,
        >,
    >,
    /// The type-driven half of `call_result_qualification_identities`:
    /// owned-field and constraint domain rows keyed by return type, shared
    /// across every target returning that type so each distinct result type
    /// pays the scan once rather than each distinct target.
    pub(super) result_identity_domain_rows: HashMap<
        typed_trees::types::TypeReferenceHandle,
        Rc<
            Vec<(
                Vec<facts::PlaceSegment>,
                SymbolHandle,
                language_semantics::SemanticDomainId,
            )>,
        >,
    >,
    /// Out-parameter `ensures <parameter> in D` rows per callable: the same
    /// program-pure contract collection as `call_result_identities`.
    pub(super) call_parameter_identities: HashMap<
        SymbolHandle,
        Rc<
            Vec<(
                usize,
                SymbolHandle,
                language_semantics::SemanticDomainId,
                bool,
            )>,
        >,
    >,
    /// Provable `(field path, domain)` reseed rows a readable `&mut`
    /// parameter's declared referent type hands back at return; `None` marks
    /// parameters that are not readable mutable references or have no
    /// provable paths. Keyed by the declared type reference -- the rows are
    /// program-pure on it.
    pub(super) referent_type_paths: HashMap<
        typed_trees::types::TypeReferenceHandle,
        Option<Rc<Vec<validation::field_domains::FieldDomainIdentity>>>,
    >,
    /// Lazily built on the first symbol lookup; positions only, so it holds
    /// no typed-tree borrow and survives `discard_output`.
    symbol_index: Option<SymbolIndex>,
    /// Checked scalar plan rows grouped by their binding site, keyed
    /// (state, statement_ordinal). Both plans tables are program-immutable;
    /// role filtering stays at the call site, so the row lists keep the
    /// table's iteration order.
    pub(super) scalar_bindings_by_site: HashMap<
        (SymbolHandle, u32),
        Vec<arena::Handle<checked_trees::CheckedScalarExpressionBindings>>,
    >,
    pub(super) scalar_expressions_by_site: HashMap<(SymbolHandle, u32), Vec<usize>>,
    /// Program-pure per-site answers repeated on every pass: canonical
    /// places, expression result types, transition call targets, entry
    /// parameter origin chains, and reference-local candidate places. The
    /// program cannot change mid-build, so each memoized value is final.
    canonical_places: HashMap<
        (
            SymbolHandle,
            usize,
            typed_trees::expression::ExpressionHandle,
        ),
        PlacePreparation,
    >,
    expression_result_types: HashMap<
        (
            SymbolHandle,
            SymbolHandle,
            typed_trees::expression::ExpressionHandle,
        ),
        Option<typed_trees::types::TypeReferenceHandle>,
    >,
    transition_call_targets: HashMap<
        (SymbolHandle, SymbolHandle, usize, usize),
        Option<typed_trees::statement::TransitionTargetHandle>,
    >,
    entry_origin_chains:
        HashMap<(SymbolHandle, SymbolHandle), Rc<Vec<(SymbolHandle, SymbolHandle)>>>,
    /// One solved parameter-origin fixed point per machine, shared by every
    /// state's `entry_origin_chains` row.
    machine_parameter_origins:
        HashMap<SymbolHandle, Rc<crate::flow::entry_origins::MachineParameterOrigins>>,
    /// Statement storage-write places per site: the answer is program-pure on
    /// (state, statement index) -- the machine and statement are implied by
    /// the key and the shared call-frame resolver is the same `call_frames`
    /// the whole build shares -- so the enumeration runs at most once per
    /// site rather than once per pass.
    statement_storage_writes:
        HashMap<(SymbolHandle, usize), Option<Rc<Vec<validation::CanonicalPlace>>>>,
    /// Declared root type for a transfer-correspondence place: another
    /// program-immutable lookup over machines, states, and statement tables.
    pub(super) correspondence_root_types: HashMap<
        (SymbolHandle, SymbolHandle, usize, SymbolHandle),
        Option<typed_trees::types::TypeReferenceHandle>,
    >,
    /// Expression sub-occurrences extracted per operand on every pass --
    /// program-pure walks over the expression table.
    expression_occurrences: HashMap<
        typed_trees::expression::ExpressionHandle,
        Rc<Vec<typed_trees::expression::ExpressionHandle>>,
    >,
    proof_fact_occurrences: HashMap<
        arena::Handle<typed_trees::domain::ProofFact>,
        Rc<Vec<typed_trees::expression::ExpressionHandle>>,
    >,
    /// Sorted unique integer literals authored anywhere in the program -- the
    /// widening thresholds `state_values::fields` consults when a joined bound
    /// keeps extending. Program-pure like the memo tables above, so the
    /// whole-arena scan and its literal parsing run at most once per build
    /// rather than once per widening rejoin.
    integer_literal_thresholds: Option<Rc<Vec<numerics::bignum::BigInt>>>,
    /// Sorted by exact source occurrence, populated only for guards reached by
    /// flow. Retain unsuccessful source binding as well as successful binding.
    guard_predicates: Vec<GuardPreparation>,
    pub(super) assignment_observations: super::assignment_observations::AssignmentObservations,
    pub(super) contexts: FlowContextFacts,
    pub(super) invalidations: FlowInvalidationFacts,
    pub(super) borrow_lifetimes: FlowBorrowLifetimeFacts,
    pub(super) ownership: FlowOwnershipFacts,
    pub(super) boundaries: FlowBoundaryFacts,
    pub(super) control: FlowControlFacts,
    /// Current state's contiguous statement frontier while its flow is built.
    pub(super) state_statements_start: usize,
}

impl<'plans> FlowBuildContext<'plans> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        borrow: &BorrowFacts,
        proof: &ProofFacts,
        semantic: &FactPlan,
        scalar_expressions: &'plans checked_trees::CheckedScalarExpressionPlans,
        operators: &'plans checked_trees::CheckedOperatorFacts,
        exact_integer_casts: &'plans [validation::ExactIntegerCastFact],
        call_frames: Option<&'plans validation::CallFrameResolver<'plans>>,
        state_mutation_summary_cache: &'plans StateMutationSummaryCache,
    ) -> Self {
        let mut scalar_bindings_by_site = HashMap::default();
        for (handle, binding) in scalar_expressions.source_bindings.iter() {
            scalar_bindings_by_site
                .entry((binding.state, binding.statement_ordinal))
                .or_insert_with(Vec::new)
                .push(handle);
        }
        let mut scalar_expressions_by_site = HashMap::default();
        for (row, expression) in scalar_expressions.expressions.iter().enumerate() {
            scalar_expressions_by_site
                .entry((expression.state, expression.statement_ordinal))
                .or_insert_with(Vec::new)
                .push(row);
        }
        Self {
            state_statements_start: 0,
            scalar_expressions,
            operators,
            exact_integer_casts,
            call_frames,
            state_value_inputs: Vec::new(),
            dirty_state_value_inputs: Vec::new(),
            new_state_field_input_height: 0,
            element_store_potentials: Vec::new(),
            call_sites: HashMap::default(),
            call_target_parameters: HashMap::default(),
            call_target_returns: HashMap::default(),
            call_result_identities: HashMap::default(),
            result_identity_domain_rows: HashMap::default(),
            call_parameter_identities: HashMap::default(),
            referent_type_paths: HashMap::default(),
            symbol_index: None,
            scalar_bindings_by_site,
            scalar_expressions_by_site,
            canonical_places: HashMap::default(),
            expression_result_types: HashMap::default(),
            transition_call_targets: HashMap::default(),
            entry_origin_chains: HashMap::default(),
            machine_parameter_origins: HashMap::default(),
            statement_storage_writes: HashMap::default(),
            correspondence_root_types: HashMap::default(),
            expression_occurrences: HashMap::default(),
            proof_fact_occurrences: HashMap::default(),
            integer_literal_thresholds: None,
            guard_predicates: Vec::new(),
            state_mutation_summary_cache,
            assignment_observations: Default::default(),
            contexts: FlowContextFacts::with_roots(
                arena::Arena::with_capacity(semantic.contexts.len().saturating_mul(2)),
                arena::Arena::with_capacity(
                    semantic
                        .contexts
                        .len()
                        .saturating_mul(3)
                        .saturating_add(borrow.states.len())
                        .saturating_add(borrow.calls.len())
                        .saturating_add(borrow.loans.len()),
                ),
            ),
            invalidations: FlowInvalidationFacts::with_roots(
                arena::Arena::default(),
                arena::Arena::default(),
            ),
            borrow_lifetimes: FlowBorrowLifetimeFacts::with_roots(
                arena::Arena::default(),
                arena::Arena::default(),
            ),
            ownership: FlowOwnershipFacts::with_roots(
                arena::Arena::default(),
                arena::Arena::default(),
                arena::Arena::default(),
                arena::Arena::default(),
            ),
            boundaries: FlowBoundaryFacts::with_roots(arena::Arena::with_capacity(
                borrow.calls.len(),
            )),
            control: FlowControlFacts::with_roots(
                arena::Arena::with_capacity(borrow.calls.len()),
                arena::Arena::with_capacity(borrow.calls.len()),
                arena::Arena::with_capacity(proof.contract_exits.len()),
                arena::Arena::with_capacity(borrow.states.len()),
            ),
        }
    }

    pub(super) fn finish(self) -> FlowFacts {
        FlowFacts::with_roots(
            self.contexts,
            self.invalidations,
            self.borrow_lifetimes,
            self.ownership,
            self.boundaries,
            self.control,
        )
    }

    fn symbol_index(&mut self, program: &dyn ::validation::CheckingInput) -> &SymbolIndex {
        self.symbol_index
            .get_or_insert_with(|| SymbolIndex::build(program))
    }

    /// Position of `machine_symbol` in `program.machines()`.
    pub(super) fn machine_index(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<usize> {
        self.symbol_index(program)
            .machine_index
            .get(&machine_symbol)
            .copied()
    }

    /// Position of `state_symbol` inside `machine_symbol`'s state list.
    pub(super) fn state_index_in_machine(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<usize> {
        self.symbol_index(program)
            .state_index_in_machine
            .get(&(machine_symbol, state_symbol))
            .copied()
    }

    /// `(machines index, in-machine state index)` of a state symbol anywhere
    /// in the program -- the shape `TypedTrees::state_by_symbol` resolves.
    pub(super) fn state_location(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
    ) -> Option<(usize, usize)> {
        self.symbol_index(program)
            .state_location
            .get(&state_symbol)
            .copied()
    }

    pub(super) fn guard_predicate_at(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        value: bool,
    ) -> Option<&crate::facts::boolean_predicates::BoundPredicate> {
        let occurrence = (state, statement, expression, value);
        let order_key = |(state, statement, expression, value): (
            SymbolHandle,
            usize,
            typed_trees::expression::ExpressionHandle,
            bool,
        )| {
            (
                state.arena_index(),
                state.generation(),
                statement,
                expression.arena_index(),
                expression.generation(),
                value,
            )
        };
        let position = match self
            .guard_predicates
            .binary_search_by_key(&order_key(occurrence), |prepared| {
                order_key(prepared.occurrence)
            }) {
            Ok(position) => position,
            Err(position) => {
                let predicate = crate::facts::boolean_predicates::bind_current_guard(
                    program,
                    self.operators,
                    state,
                    statement,
                    expression,
                    value,
                );
                self.guard_predicates.insert(
                    position,
                    GuardPreparation {
                        occurrence,
                        predicate,
                    },
                );
                position
            }
        };
        self.guard_predicates[position].predicate.as_ref()
    }

    pub(super) fn canonical_place_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<validation::CanonicalPlace> {
        self.canonical_places
            .entry((state_symbol, statement_index, expression))
            .or_insert_with(|| {
                PlacePreparation::new(program, state_symbol, statement_index, expression)
            })
            .place
            .clone()
    }

    /// Reuse the source provenance, never a previous prefix's admission. The
    /// predicate reader may query a leaf speculatively while distinguishing
    /// integer from float meaning, so keep each callback and its live check.
    /// Its own path validation and work budget still run after this returns.
    pub(super) fn entry_subject_at(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        entry_writes: arena::HandleSpan<checked_trees::FlowOperandReferent>,
        entry_unknown_write: bool,
    ) -> Option<(
        u32,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    )> {
        let prepared = self
            .canonical_places
            .entry((state, statement, expression))
            .or_insert_with(|| PlacePreparation::new(program, state, statement, expression));
        let place = prepared.place.as_ref()?;
        if !super::entry_origin_is_stable(
            program,
            &self.control,
            entry_writes,
            entry_unknown_write,
            place,
        ) {
            return None;
        }
        prepared
            .entry_subject
            .get_or_insert_with(|| {
                let (machine, _) = validation::find_state(program, state)?;
                crate::facts::entry_parameter_place_before_statement(
                    program,
                    machine.symbol,
                    state,
                    statement,
                    place,
                )
            })
            .clone()
    }

    pub(super) fn expression_result_type_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        *self
            .expression_result_types
            .entry((machine.symbol, state.symbol, expression))
            .or_insert_with(|| {
                validation::expression_result_type_reference(program, machine, state, expression)
            })
    }

    pub(super) fn transition_call_target_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<typed_trees::statement::TransitionTargetHandle> {
        *self
            .transition_call_targets
            .entry((machine.symbol, state.symbol, statement_index, call_ordinal))
            .or_insert_with(|| {
                validation::transition_call_target(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                )
            })
    }

    /// Entry-parameter origin chain for one state -- the whole-machine
    /// reachability fixpoint in `entry_origins`, memoized because its inputs
    /// are all program-immutable.
    pub(super) fn entry_origin_chain_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Rc<Vec<(SymbolHandle, SymbolHandle)>> {
        if let Some(chain) = self
            .entry_origin_chains
            .get(&(machine.symbol, state.symbol))
        {
            return chain.clone();
        }
        let solved = match self.machine_parameter_origins.get(&machine.symbol) {
            Some(solved) if solved.covers(program, machine) => solved.clone(),
            _ => {
                let solved = Rc::new(crate::flow::entry_origins::machine_parameter_origins(
                    program, machine,
                ));
                self.machine_parameter_origins
                    .insert(machine.symbol, solved.clone());
                solved
            }
        };
        let chain = Rc::new(crate::flow::entry_origins::state_origins(
            program, machine, state, &solved,
        ));
        self.entry_origin_chains
            .insert((machine.symbol, state.symbol), chain.clone());
        chain
    }

    pub(super) fn expression_occurrences_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Rc<Vec<typed_trees::expression::ExpressionHandle>> {
        self.expression_occurrences
            .entry(expression)
            .or_insert_with(|| {
                let mut occurrences = Vec::new();
                crate::facts::contract_occurrences::append_expression_occurrences(
                    program,
                    expression,
                    &mut occurrences,
                );
                Rc::new(occurrences)
            })
            .clone()
    }

    pub(super) fn proof_fact_occurrences_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> Rc<Vec<typed_trees::expression::ExpressionHandle>> {
        self.proof_fact_occurrences
            .entry(fact)
            .or_insert_with(|| {
                Rc::new(
                    crate::facts::contract_occurrences::fact_referenced_occurrences(program, fact),
                )
            })
            .clone()
    }

    /// The program's sorted integer-literal widening thresholds. The authored
    /// set cannot change mid-build, so the collection and its per-literal
    /// `BigInt` parsing memoize on first widening demand.
    pub(super) fn integer_literal_thresholds_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
    ) -> Rc<Vec<numerics::bignum::BigInt>> {
        self.integer_literal_thresholds
            .get_or_insert_with(|| {
                Rc::new(super::state_values::fields::integer_literal_thresholds(
                    program,
                ))
            })
            .clone()
    }

    pub(super) fn scalar_binding_handles_at(
        &self,
        state_symbol: SymbolHandle,
        statement_ordinal: u32,
    ) -> &[arena::Handle<checked_trees::CheckedScalarExpressionBindings>] {
        self.scalar_bindings_by_site
            .get(&(state_symbol, statement_ordinal))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn scalar_expression_rows_at(
        &self,
        state_symbol: SymbolHandle,
        statement_ordinal: u32,
    ) -> &[usize] {
        self.scalar_expressions_by_site
            .get(&(state_symbol, statement_ordinal))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Storage-write places for one statement site. The enumeration walks
    /// local origins, origin paths, and alias closures per statement, all
    /// program-pure on (state, index): the statement sits at that index in
    /// that state, its machine is implied, and the call-frame resolver is
    /// the shared one the whole build carries.
    pub(super) fn statement_storage_writes_at(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &checked_trees::statement::StatementNode,
    ) -> Option<Rc<Vec<validation::CanonicalPlace>>> {
        self.statement_storage_writes
            .entry((state_symbol, statement_index))
            .or_insert_with(|| {
                crate::flow::statement_storage_writes(
                    program,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    statement,
                    self.call_frames,
                )
                .map(Rc::new)
            })
            .clone()
    }

    /// Discard a complete unpublished sweep, retaining only reusable storage.
    /// StateValues contains owned evidence and typed-tree handles, never handles
    /// into these output arenas or the changing semantic plan. The immutable
    /// plans and mutation cache likewise retain no sweep-local handles.
    ///
    /// This is the former whole-context replacement boundary, not a partial
    /// rollback: all output rows must disappear together, before semantic
    /// restoration and before another transfer. No copied output handle may
    /// survive reuse. Published output cannot be reset because finish consumes
    /// the context. Preserve these constraints when adding a context field.
    pub(super) fn discard_output(&mut self) {
        self.assignment_observations.clear();
        discard_output_arena(&mut self.contexts.semantic_context_refs);
        discard_output_arena(&mut self.contexts.constraint_refs);
        discard_output_arena(&mut self.invalidations.segments);
        discard_output_arena(&mut self.invalidations.events);
        discard_output_arena(&mut self.borrow_lifetimes.activations);
        discard_output_arena(&mut self.borrow_lifetimes.weakenings);
        discard_output_arena(&mut self.ownership.segments);
        discard_output_arena(&mut self.ownership.permissions);
        discard_output_arena(&mut self.ownership.claim_outcome_entries);
        discard_output_arena(&mut self.ownership.claim_outcome_maps);
        discard_output_arena(&mut self.boundaries.edges);
        discard_output_arena(&mut self.control.statements);
        discard_output_arena(&mut self.control.operator_invocations);
        discard_output_arena(&mut self.control.atomic_executions);
        discard_output_arena(&mut self.control.operands);
        discard_output_arena(&mut self.control.case_accesses);
        discard_output_arena(&mut self.control.operand_children);
        discard_output_arena(&mut self.control.operand_child_paths);
        discard_output_arena(&mut self.control.operand_origin_writes);
        discard_output_arena(&mut self.control.operand_referents);
        discard_output_arena(&mut self.control.operand_referent_segments);
        discard_output_arena(&mut self.control.calls);
        discard_output_arena(&mut self.control.exits);
        discard_output_arena(&mut self.control.outgoing);
        discard_output_arena(&mut self.control.exit_parameter_origins);
        discard_output_arena(&mut self.control.states);
        self.new_state_field_input_height = 0;
    }
}

fn discard_output_arena<T: Default>(arena: &mut arena::Arena<T>) {
    arena.reset_retain_capacity();
    // Reproduce fresh-context semantics even if an invalid mutable lookup had
    // changed the arena's dummy; retaining capacity must not retain that value.
    *arena.get_mut(arena::Handle::invalid()) = T::default();
}

/// The semantic context handles active at one program point, in span order.
pub(super) fn active_context_handles(
    contexts: &FlowBuildContext<'_>,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
) -> Vec<facts::FactContextHandle> {
    contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>()
}

/// Every fact payload the active contexts carry, in span order.
pub(super) fn active_context_facts(
    contexts: &FlowBuildContext<'_>,
    semantic: &facts::FactPlan,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
) -> Vec<facts::Fact> {
    contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .flat_map(|reference| {
            semantic
                .context_view(semantic.contexts.get(reference.context))
                .facts()
        })
        .copied()
        .collect::<Vec<_>>()
}

/// The case-selection gate every source transfer applies before writing:
/// the statement's active contexts must select every case the source path's
/// segments name.
pub(super) fn source_transfer_cases_selected(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &FlowBuildContext<'_>,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
    point: facts::ProgramPoint,
    root: facts::PlaceRoot,
    segments: Vec<facts::PlaceSegment>,
) -> bool {
    let context_handles = active_context_handles(contexts, active);
    let facts::ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = point
    else {
        return false;
    };
    crate::flow::place_cases_are_selected(
        program,
        semantic,
        &context_handles,
        machine_symbol,
        state_symbol,
        statement_index,
        &validation::CanonicalPlace { root, segments },
    )
}
