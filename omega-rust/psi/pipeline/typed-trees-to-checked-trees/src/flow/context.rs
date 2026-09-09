use super::*;

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
    pub(super) state_mutation_summary_cache: &'plans StateMutationSummaryCache,
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
    use super::*;

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
