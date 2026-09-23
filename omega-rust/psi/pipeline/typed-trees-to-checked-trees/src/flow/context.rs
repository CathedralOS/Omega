use crate::flow::StateMutationSummaryCache;
use checked_trees::{
    BorrowFacts, FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts,
    FlowFacts, FlowInvalidationFacts, FlowOwnershipFacts, ProofFacts,
};
use facts::FactPlan;
use std::collections::HashMap;
use std::rc::Rc;
use symbols::SymbolHandle;

/// Program shape lookups indexed once per context: every machine, state, and
/// their in-machine positions. All values are table positions, so the map is
/// lifetime-free and rebuilds lazily on the first query of a fresh context.
struct SymbolIndex {
    machine_index: HashMap<SymbolHandle, usize>,
    state_index_in_machine: HashMap<(SymbolHandle, SymbolHandle), usize>,
    state_location: HashMap<SymbolHandle, (usize, usize)>,
}

impl SymbolIndex {
    fn build(program: &typed_trees::TypedTrees) -> Self {
        let mut index = Self {
            machine_index: HashMap::new(),
            state_index_in_machine: HashMap::new(),
            state_location: HashMap::new(),
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
    #[cfg(test)]
    pub(super) built_state_value_inputs: Vec<SymbolHandle>,
    #[cfg(test)]
    pub(super) state_value_inputs_changed_after_build: bool,
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
        Vec<crate::facts::field_domain::ByteSequencePredicate>,
    )>,
    pub(super) state_mutation_summary_cache: &'plans StateMutationSummaryCache,
    /// Program-global lookups repeated by every call site on every pass. The
    /// typed program is immutable for the whole fixpoint, so each answer
    /// memoizes once and survives `discard_output` like the immutable plans
    /// above -- they hold typed-tree handles only, never sweep-local arena
    /// handles.
    pub(super) call_sites: HashMap<
        (SymbolHandle, SymbolHandle, usize, usize),
        Option<crate::semantic::calls::CallSite<'plans>>,
    >,
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
        Option<crate::flow::CanonicalPlace>,
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
    reference_candidate_places:
        HashMap<(SymbolHandle, usize, SymbolHandle), Option<Rc<Vec<crate::flow::CanonicalPlace>>>>,
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
    pub(super) contexts: FlowContextFacts,
    pub(super) invalidations: FlowInvalidationFacts,
    pub(super) borrow_lifetimes: FlowBorrowLifetimeFacts,
    pub(super) ownership: FlowOwnershipFacts,
    pub(super) boundaries: FlowBoundaryFacts,
    pub(super) control: FlowControlFacts,
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
        let mut scalar_bindings_by_site = HashMap::new();
        for (handle, binding) in scalar_expressions.source_bindings.iter() {
            scalar_bindings_by_site
                .entry((binding.state, binding.statement_ordinal))
                .or_insert_with(Vec::new)
                .push(handle);
        }
        let mut scalar_expressions_by_site = HashMap::new();
        for (row, expression) in scalar_expressions.expressions.iter().enumerate() {
            scalar_expressions_by_site
                .entry((expression.state, expression.statement_ordinal))
                .or_insert_with(Vec::new)
                .push(row);
        }
        Self {
            scalar_expressions,
            operators,
            exact_integer_casts,
            call_frames,
            state_value_inputs: Vec::new(),
            dirty_state_value_inputs: Vec::new(),
            #[cfg(test)]
            built_state_value_inputs: Vec::new(),
            #[cfg(test)]
            state_value_inputs_changed_after_build: false,
            new_state_field_input_height: 0,
            element_store_potentials: Vec::new(),
            call_sites: HashMap::new(),
            call_target_parameters: HashMap::new(),
            call_target_returns: HashMap::new(),
            call_result_identities: HashMap::new(),
            symbol_index: None,
            scalar_bindings_by_site,
            scalar_expressions_by_site,
            canonical_places: HashMap::new(),
            expression_result_types: HashMap::new(),
            transition_call_targets: HashMap::new(),
            entry_origin_chains: HashMap::new(),
            reference_candidate_places: HashMap::new(),
            correspondence_root_types: HashMap::new(),
            expression_occurrences: HashMap::new(),
            proof_fact_occurrences: HashMap::new(),
            integer_literal_thresholds: None,
            state_mutation_summary_cache,
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

    fn symbol_index(&mut self, program: &typed_trees::TypedTrees) -> &SymbolIndex {
        self.symbol_index
            .get_or_insert_with(|| SymbolIndex::build(program))
    }

    /// Position of `machine_symbol` in `program.machines()`.
    pub(super) fn machine_index(
        &mut self,
        program: &typed_trees::TypedTrees,
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
        program: &typed_trees::TypedTrees,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<usize> {
        self.symbol_index(program)
            .state_index_in_machine
            .get(&(machine_symbol, state_symbol))
            .copied()
    }

    /// `(machines index, in-machine state index)` of a state symbol anywhere
    /// in the program -- the shape `semantic::calls::find_state` scans for.
    pub(super) fn state_location(
        &mut self,
        program: &typed_trees::TypedTrees,
        state_symbol: SymbolHandle,
    ) -> Option<(usize, usize)> {
        self.symbol_index(program)
            .state_location
            .get(&state_symbol)
            .copied()
    }

    pub(super) fn canonical_place_at(
        &mut self,
        program: &typed_trees::TypedTrees,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<crate::flow::CanonicalPlace> {
        self.canonical_places
            .entry((state_symbol, statement_index, expression))
            .or_insert_with(|| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    expression,
                )
            })
            .clone()
    }

    pub(super) fn expression_result_type_at(
        &mut self,
        program: &typed_trees::TypedTrees,
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
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<typed_trees::statement::TransitionTargetHandle> {
        *self
            .transition_call_targets
            .entry((machine.symbol, state.symbol, statement_index, call_ordinal))
            .or_insert_with(|| {
                crate::semantic::calls::transition_call_target(
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
        program: &typed_trees::TypedTrees,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Rc<Vec<(SymbolHandle, SymbolHandle)>> {
        self.entry_origin_chains
            .entry((machine.symbol, state.symbol))
            .or_insert_with(|| {
                Rc::new(crate::flow::entry_origins::state_origins(
                    program, machine, state,
                ))
            })
            .clone()
    }

    pub(super) fn expression_occurrences_at(
        &mut self,
        program: &typed_trees::TypedTrees,
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
        program: &typed_trees::TypedTrees,
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
        program: &typed_trees::TypedTrees,
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

    pub(super) fn reference_candidate_places_at(
        &mut self,
        program: &typed_trees::TypedTrees,
        state_symbol: SymbolHandle,
        statement_index: usize,
        root: SymbolHandle,
    ) -> Option<Rc<Vec<crate::flow::CanonicalPlace>>> {
        self.reference_candidate_places
            .entry((state_symbol, statement_index, root))
            .or_insert_with(|| {
                crate::flow::reference_result_candidates_before_statement(
                    program,
                    state_symbol,
                    statement_index,
                    root,
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
        discard_output_arena(&mut self.control.operator_operands);
        discard_output_arena(&mut self.control.operand_referents);
        discard_output_arena(&mut self.control.operand_referent_segments);
        discard_output_arena(&mut self.control.calls);
        discard_output_arena(&mut self.control.exits);
        discard_output_arena(&mut self.control.exit_parameter_origins);
        discard_output_arena(&mut self.control.states);
        self.new_state_field_input_height = 0;
        #[cfg(test)]
        {
            self.built_state_value_inputs.clear();
            self.state_value_inputs_changed_after_build = false;
        }
    }
}

fn discard_output_arena<T: Default>(arena: &mut arena::Arena<T>) {
    arena.reset_retain_capacity();
    // Reproduce fresh-context semantics even if an invalid mutable lookup had
    // changed the arena's dummy; retaining capacity must not retain that value.
    *arena.get_mut(arena::Handle::invalid()) = T::default();
}

#[cfg(test)]
mod tests {
    use super::{StateMutationSummaryCache, SymbolHandle};
    use crate::flow::FlowBuildContext;
    use crate::flow::context::discard_output_arena;

    #[test]
    fn discard_output_replaces_all_roots_and_preserves_convergence_inputs() {
        let scalar_expressions = Default::default();
        let operators = Default::default();
        let mutation_cache = StateMutationSummaryCache::default();
        let fresh = || {
            FlowBuildContext::new(
                &Default::default(),
                &Default::default(),
                &Default::default(),
                &scalar_expressions,
                &operators,
                &[],
                None,
                &mutation_cache,
            )
        };
        let mut context = fresh();
        context
            .contexts
            .semantic_context_refs
            .append(Default::default());
        context.contexts.constraint_refs.append(Default::default());
        context.invalidations.segments.append(Default::default());
        context.invalidations.events.append(Default::default());
        context
            .borrow_lifetimes
            .activations
            .append(Default::default());
        context
            .borrow_lifetimes
            .weakenings
            .append(Default::default());
        context.ownership.segments.append(Default::default());
        context.ownership.permissions.append(Default::default());
        context
            .ownership
            .claim_outcome_entries
            .append(Default::default());
        context
            .ownership
            .claim_outcome_maps
            .append(Default::default());
        context.boundaries.edges.append(Default::default());
        context.control.statements.append(Default::default());
        context.control.calls.append(Default::default());
        context.control.exits.append(Default::default());
        context
            .control
            .exit_parameter_origins
            .append(Default::default());
        context.control.states.append(Default::default());
        context
            .dirty_state_value_inputs
            .push(SymbolHandle::invalid());
        context
            .built_state_value_inputs
            .push(SymbolHandle::invalid());
        context.state_value_inputs_changed_after_build = true;
        context.new_state_field_input_height = 9;

        context.discard_output();

        assert_eq!(context.dirty_state_value_inputs, [SymbolHandle::invalid()]);
        assert!(context.built_state_value_inputs.is_empty());
        assert!(!context.state_value_inputs_changed_after_build);
        assert_eq!(context.new_state_field_input_height, 0);
        assert_eq!(context.finish(), fresh().finish());
    }

    #[test]
    fn discard_output_replaces_a_modified_dummy_and_reuses_storage() {
        let mut arena = arena::Arena::<usize>::with_capacity(8);
        arena.append(7);
        *arena.get_mut(arena::Handle::invalid()) = 99;
        let pointer = arena.storage_slice().as_ptr();
        discard_output_arena(&mut arena);
        assert!(arena.is_empty());
        assert_eq!(*arena.get(arena::Handle::invalid()), 0);
        assert_eq!(arena.storage_slice().as_ptr(), pointer);
        let replacement = arena.append(11);
        assert_eq!(*arena.get(replacement), 11);
    }
}
