/*
    `FlowBuildContext`, the mutable state of one flow build: the output fact arenas
    that `take_output_pages` publishes as `FlowFacts` and `discard_output` clears
    between sweeps, the incoming state values and dirty set that decide re-sweeps,
    and memo tables for lookups that depend only on the program, which survive
    between sweeps.
*/

use crate::flow::StateMutationSummaryCache;
use crate::flow::entry_origins;
use crate::flow::place_cases_are_selected;
use crate::published::facts::boolean_predicates;
use crate::published::facts::build_dynamic_conformance_facts;
use crate::published::facts::contract_occurrences::append_expression_occurrences;
use crate::published::facts::contract_occurrences::fact_referenced_occurrences;
use crate::published::facts::entry_parameter_place_before_statement;
use crate::published::places::place_at;
use checked_trees::{
    BorrowFacts, FlowBorrowLifetimeFacts, FlowBoundaryFacts, FlowContextFacts, FlowControlFacts,
    FlowFacts, FlowInvalidationFacts, FlowOwnershipFacts, ProofFacts,
};
use facts::FactPlan;
/* Handle-keyed maps take the integer hasher; none is iterated for output order. */
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
use std::rc::Rc;
use symbols::SymbolHandle;

/*
    Typed callee-relative exit relations contain no caller observations.
    An explicit row with no answer is a prepared unknown, not an empty effect.
*/
struct StoredReferencePreparation {
    target: SymbolHandle,
    effects: Option<Rc<Vec<validation::StoredReferenceEffect>>>,
}

/*
    Cached guard algebra contains only source identities. Storage binding and
    read-epoch agreement consume fresh Flow observations at every evaluation;
    no sweep-local address or constraint handle may survive in this cache.
*/
struct GuardPreparation {
    occurrence: (
        SymbolHandle,
        usize,
        typed_trees::expression::ExpressionHandle,
    ),
    expression: Option<checked_trees::CheckedBooleanExpression>,
}

/*
    Immutable preparation for one authored invocation. Neither result contains
    live flow contexts: the source, borrow inventory, mutation summaries and
    exact-prefix alias resolver stay fixed throughout this flow build. Keep
    unknown writes (`Some(None)`) distinct from not yet prepared (`None`).
*/
#[derive(Default)]
pub(super) struct CallPreparation<'program> {
    pub(super) site: Option<Option<validation::CallSite<'program>>>,
    pub(super) storage_writes: Option<Option<Rc<super::call_phases::CallStorageWrites>>>,
}

/*
    Where each machine and state sits in the program's tables, indexed by the
    symbol's arena position. A row keeps its symbol so another generation at
    the same position finds nothing; a later duplicate replaces an earlier one.
*/
#[derive(Default)]
struct SymbolPositions {
    machines: Vec<(SymbolHandle, usize)>,
    states: Vec<(SymbolHandle, (usize, usize))>,
}

impl SymbolPositions {
    fn build(program: &dyn ::validation::CheckingInput) -> Self {
        fn place<T: Copy + Default>(
            rows: &mut Vec<(SymbolHandle, T)>,
            symbol: SymbolHandle,
            value: T,
        ) {
            let index = symbol.arena_index() as usize;
            if rows.len() <= index {
                rows.resize(index + 1, (SymbolHandle::invalid(), T::default()));
            }
            rows[index] = (symbol, value);
        }
        let mut positions = Self::default();
        for (machine_index, machine) in program.machines().iter().enumerate() {
            place(&mut positions.machines, machine.symbol, machine_index);
            for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                place(
                    &mut positions.states,
                    state.symbol,
                    (machine_index, state_index),
                );
            }
        }
        positions
    }

    fn find<T: Copy>(rows: &[(SymbolHandle, T)], symbol: SymbolHandle) -> Option<T> {
        let (stored, value) = rows.get(symbol.arena_index() as usize)?;
        (symbol.is_valid() && *stored == symbol).then_some(*value)
    }
}

/*
    Every machine's parameter-origin fixed point, solved once before the flow
    groups fan out and indexed by machine position. Storage origins also
    follow writes through calls, so they exist only with a call-frame resolver.
*/
struct MachineOrigins {
    parameters: Vec<std::sync::Arc<entry_origins::MachineParameterOrigins>>,
    storage: Vec<std::sync::Arc<entry_origins::MachineParameterOrigins>>,
}

impl MachineOrigins {
    fn solve(
        program: &dyn ::validation::CheckingInput,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Self {
        use rayon::prelude::*;
        let machines = program.machines();
        let (parameters, storage) = validation::checking_thread_pool().install(|| {
            let parameters = machines
                .par_iter()
                .map(|machine| {
                    std::sync::Arc::new(entry_origins::machine_parameter_origins(program, machine))
                })
                .collect();
            let storage = call_frames.map_or_else(Vec::new, |frames| {
                machines
                    .par_iter()
                    .map(|machine| {
                        std::sync::Arc::new(entry_origins::machine_storage_origins(
                            program, frames, machine,
                        ))
                    })
                    .collect()
            });
            (parameters, storage)
        });
        Self {
            parameters,
            storage,
        }
    }
}

pub(super) struct SharedFlowTables {
    dynamic_conformances: Option<std::sync::Arc<checked_trees::DynamicConformanceFacts>>,
    symbol_positions: SymbolPositions,
    machine_origins: MachineOrigins,
    return_dependents: Vec<Vec<SymbolHandle>>,
    scalar_bindings_by_site: HashMap<
        (SymbolHandle, u32),
        Vec<arena::Handle<checked_trees::CheckedScalarExpressionBindings>>,
    >,
    scalar_expressions_by_site: HashMap<(SymbolHandle, u32), Vec<usize>>,
    integer_literal_thresholds: std::sync::OnceLock<std::sync::Arc<Vec<numerics::bignum::BigInt>>>,
}

impl SharedFlowTables {
    pub(super) fn new(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        scalar_expressions: &checked_trees::CheckedScalarExpressionPlans,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
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
            dynamic_conformances: build_dynamic_conformance_facts(program)
                .ok()
                .map(std::sync::Arc::new),
            symbol_positions: SymbolPositions::build(program),
            machine_origins: MachineOrigins::solve(program, call_frames),
            return_dependents: super::normal_returns::dependents(program, borrow),
            scalar_bindings_by_site,
            scalar_expressions_by_site,
            integer_literal_thresholds: std::sync::OnceLock::new(),
        }
    }
}

pub(super) struct FlowBuildContext<'plans> {
    /*
        Chronological normal-call descriptor values for the state being built.
        Completed by-value payloads of the target currently being evaluated.
        The target owner restores this scope before another arm is considered.
    */
    pub(super) active_completed_copies: arena::HandleSpan<checked_trees::FlowOperandFact>,
    program: &'plans dyn ::validation::CheckingInput,
    pub(super) scalar_expressions: &'plans checked_trees::CheckedScalarExpressionPlans,
    pub(super) operators: &'plans checked_trees::CheckedOperatorFacts,
    pub(super) call_frames: Option<&'plans validation::CallFrameResolver<'plans>>,
    shared_tables: std::sync::Arc<SharedFlowTables>,
    /*
        Immutable constructor/row receipts, never a closed-world roster of
        possible incoming selectors. Only a completed value can select a row.
    */
    pub(super) dynamic_conformances: Option<std::sync::Arc<checked_trees::DynamicConformanceFacts>>,
    pub(super) state_value_inputs: Vec<super::state_values::StateValues>,
    /*
        Symbol to `state_value_inputs` position, indexed by the state symbol's
        arena position. Rows are only appended or wholesale replaced, never
        removed or reordered, so the table stays exact between sweeps and
        turns per-call reachability and join lookups from roster scans into
        single probes.
    */
    reached_state_positions: Vec<(SymbolHandle, usize)>,
    pub(super) dirty_state_value_inputs: Vec<SymbolHandle>,
    pub(super) normal_return_predicates: super::normal_returns::NormalReturnPredicates,
    pub(super) new_state_field_input_height: usize,
    /* Set only while one part of a split group converges alone. */
    pub(super) input_recording: Option<super::state_values::InputRecording>,
    /* Set only while a split group replays its independent parts. */
    pub(super) replay_guard: Option<super::state_values::ReplayGuard>,
    pub(super) state_mutation_summary_cache: &'plans StateMutationSummaryCache,
    /*
        Program-global lookups repeated by every call site on every pass. The
        typed program is immutable for the whole fixpoint, so each answer
        memoizes once and survives `discard_output` like the immutable plans
        above -- they hold typed-tree handles only, never sweep-local arena
        handles.
    */
    pub(super) call_sites:
        HashMap<(SymbolHandle, SymbolHandle, usize, usize), CallPreparation<'plans>>,
    /*
        Body-derived formal descriptor relations contain only typed program
        coordinates. Prepare each target once; caller capture substitution is
        invocation-local and never enters this memo.
    */
    call_stored_reference_effects: Vec<StoredReferencePreparation>,
    pub(super) call_target_parameters:
        HashMap<SymbolHandle, Option<&'plans [typed_trees::signature::StateParameter]>>,
    pub(super) call_target_returns:
        HashMap<SymbolHandle, Option<typed_trees::types::TypeReferenceHandle>>,
    /*
        Result contracts are invocation-owned: a shared requirement target
        does not imply the same applied local index tuple.
    */
    pub(super) call_result_identities: HashMap<
        (SymbolHandle, usize, typed_trees::types::TypeReferenceHandle),
        Rc<Vec<validation::field_domains::FieldDomainContract>>,
    >,
    /*
        The type-driven half of `call_result_qualification_identities`:
        owned-field and constraint domain rows keyed by return type, shared
        across every target returning that type so each distinct result type
        pays the scan once rather than each distinct target.
    */
    pub(super) result_identity_domain_rows: HashMap<
        typed_trees::types::TypeReferenceHandle,
        Rc<Vec<validation::field_domains::FieldDomainContract>>,
    >,
    /*
        Out-parameter `ensures <parameter> in D` rows per invocation: the same
        program-pure contract collection as `call_result_identities`.
    */
    pub(super) call_parameter_identities: HashMap<
        (SymbolHandle, usize),
        Rc<Vec<(usize, SymbolHandle, language_semantics::SemanticDomainId)>>,
    >,
    /*
        Provable `(field path, domain)` reseed rows a readable `&mut`
        parameter's declared referent type hands back at return; `None` marks
        parameters that are not readable mutable references or have no
        provable paths. Keyed by the declared type reference -- the rows are
        program-pure on it.
    */
    pub(super) referent_type_paths: HashMap<
        typed_trees::types::TypeReferenceHandle,
        Option<Rc<Vec<validation::field_domains::FieldDomainIdentity>>>,
    >,
    /*
        Sorted by exact source occurrence, populated only for guards reached by
        flow. Retain unsuccessful source binding as well as successful binding.
    */
    guard_predicates: Vec<GuardPreparation>,
    pub(super) assignment_observations: super::assignment_observations::AssignmentObservations,
    pub(super) contexts: FlowContextFacts,
    pub(super) invalidations: FlowInvalidationFacts,
    pub(super) borrow_lifetimes: FlowBorrowLifetimeFacts,
    pub(super) boundaries: FlowBoundaryFacts,
    pub(super) control: FlowControlFacts,
    /* Current state's contiguous statement frontier while its flow is built. */
    pub(super) state_statements_start: arena::PagedArenaFrontier<checked_trees::FlowStatementFact>,
    /* Current state's contiguous outgoing frontier while its flow is built. */
    pub(super) state_outgoing_start:
        arena::PagedArenaFrontier<checked_trees::FlowStateOutgoingFact>,
}

impl<'plans> FlowBuildContext<'plans> {
    /* Each active semantic context, resolved against the plan's arena. */
    pub(super) fn semantic_context_views<'a>(
        &'a self,
        semantic: &'a facts::FactPlan,
        active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
    ) -> impl Iterator<Item = &'a facts::FactContext> + 'a {
        self.contexts
            .semantic_context_refs
            .span_or_empty(active)
            .iter()
            .map(|reference| semantic.contexts.get(reference.context))
    }

    pub(super) fn stored_reference_effects(
        &mut self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        indices: &[Option<usize>],
    ) -> Option<Rc<Vec<validation::StoredReferenceEffect>>> {
        if indices.iter().any(Option::is_some) {
            return self
                .call_frames
                .and_then(|frames| {
                    frames.state_stored_reference_effects_for_indices(machine, state, indices)
                })
                .map(Rc::new);
        }
        if let Some(prepared) = self
            .call_stored_reference_effects
            .iter()
            .find(|prepared| prepared.target == state.symbol)
        {
            return prepared.effects.clone();
        }
        let effects = self
            .call_frames
            .and_then(|frames| frames.state_stored_reference_effects(machine, state))
            .map(Rc::new);
        self.call_stored_reference_effects
            .push(StoredReferencePreparation {
                target: state.symbol,
                effects: effects.clone(),
            });
        effects
    }

    /*
        Call-site selector proofs do not belong in the target-only memo. The
        unspecialized summary still answers every caller that has not narrowed
        the same paths.
    */
    pub(super) fn stored_reference_effects_for_selectors(
        &self,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        indices: &[Option<usize>],
        selectors: &[(
            symbols::SymbolHandle,
            Vec<facts::PlaceSegment>,
            symbols::SymbolHandle,
        )],
    ) -> Option<Rc<Vec<validation::StoredReferenceEffect>>> {
        self.call_frames
            .and_then(|frames| {
                frames.state_stored_reference_effects_for_selectors(
                    machine, state, indices, selectors,
                )
            })
            .map(Rc::new)
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        program: &'plans dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        proof: &ProofFacts,
        semantic: &FactPlan,
        scalar_expressions: &'plans checked_trees::CheckedScalarExpressionPlans,
        operators: &'plans checked_trees::CheckedOperatorFacts,
        call_frames: Option<&'plans validation::CallFrameResolver<'plans>>,
        state_mutation_summary_cache: &'plans StateMutationSummaryCache,
    ) -> Self {
        let shared_tables = std::sync::Arc::new(SharedFlowTables::new(
            program,
            borrow,
            scalar_expressions,
            call_frames,
        ));
        Self::with_shared_tables(
            program,
            borrow,
            proof,
            semantic,
            scalar_expressions,
            operators,
            call_frames,
            state_mutation_summary_cache,
            shared_tables,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn with_shared_tables(
        program: &'plans dyn ::validation::CheckingInput,
        _borrow: &BorrowFacts,
        _proof: &ProofFacts,
        _semantic: &FactPlan,
        scalar_expressions: &'plans checked_trees::CheckedScalarExpressionPlans,
        operators: &'plans checked_trees::CheckedOperatorFacts,
        call_frames: Option<&'plans validation::CallFrameResolver<'plans>>,
        state_mutation_summary_cache: &'plans StateMutationSummaryCache,
        shared_tables: std::sync::Arc<SharedFlowTables>,
    ) -> Self {
        let dynamic_conformances = shared_tables.dynamic_conformances.clone();
        Self {
            active_completed_copies: arena::HandleSpan::empty(),
            program,
            state_statements_start: Default::default(),
            state_outgoing_start: Default::default(),
            scalar_expressions,
            operators,
            call_frames,
            shared_tables,
            dynamic_conformances,
            state_value_inputs: Vec::new(),
            reached_state_positions: Vec::new(),
            dirty_state_value_inputs: Vec::new(),
            normal_return_predicates: Default::default(),
            new_state_field_input_height: 0,
            input_recording: None,
            replay_guard: None,
            call_sites: HashMap::default(),
            call_stored_reference_effects: Vec::new(),
            call_target_parameters: HashMap::default(),
            call_target_returns: HashMap::default(),
            call_result_identities: HashMap::default(),
            result_identity_domain_rows: HashMap::default(),
            call_parameter_identities: HashMap::default(),
            referent_type_paths: HashMap::default(),
            guard_predicates: Vec::new(),
            state_mutation_summary_cache,
            assignment_observations: Default::default(),
            contexts: FlowContextFacts::default(),
            invalidations: FlowInvalidationFacts::default(),
            borrow_lifetimes: FlowBorrowLifetimeFacts::default(),
            boundaries: FlowBoundaryFacts::default(),
            control: FlowControlFacts::default(),
        }
    }

    /*
        Publish one already-appended semantic context to both evidence lists a
        flow site extends together: the context ref and its constraint ref.
    */
    pub(super) fn append_semantic_context(
        &mut self,
        contexts: &mut arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
        constraints: &mut arena::HandleSpan<checked_trees::FlowConstraintRef>,
        context: facts::FactContextHandle,
    ) {
        super::reference_spans::append_flow_reference(
            &mut self.contexts.semantic_context_refs,
            contexts,
            checked_trees::FlowSemanticContextRef { context },
        );
        super::reference_spans::append_constraint_ref(
            &mut self.contexts.constraint_refs,
            constraints,
            checked_trees::FlowConstraintKind::SemanticContext { context },
        );
    }

    /*
        Retain `place` as an origin write of `span`. Origin writes carry no
        formation; the segments are copied into the referent arena and the root
        kept verbatim.
    */
    pub(super) fn append_origin_write(
        &mut self,
        span: &mut arena::HandleSpan<checked_trees::FlowOperandReferent>,
        place: &validation::CanonicalPlace,
    ) {
        let segments = self
            .control
            .operand_referent_segments
            .insert_many(place.segments.iter().copied());
        self.control.operand_origin_writes.append_to_span(
            span,
            checked_trees::FlowOperandReferent {
                formation: arena::Handle::invalid(),
                root: place.root,
                segments,
            },
        );
    }

    /* Position of `machine_symbol` in `program.machines()`. */
    pub(super) fn machine_index(
        &self,
        _program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
    ) -> Option<usize> {
        SymbolPositions::find(
            &self.shared_tables.symbol_positions.machines,
            machine_symbol,
        )
    }

    /* Position of `state_symbol` inside `machine_symbol`'s state list. */
    pub(super) fn state_index_in_machine(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<usize> {
        let (machine, state) = self.state_location(program, state_symbol)?;
        (program.machines()[machine].symbol == machine_symbol).then_some(state)
    }

    /* `state_value_inputs` position of a reached state, if it has a row. */
    pub(super) fn reached_state_index(&self, state: SymbolHandle) -> Option<usize> {
        let (stored, index) = self
            .reached_state_positions
            .get(state.arena_index() as usize)?;
        (state.is_valid() && *stored == state).then_some(*index)
    }

    /* Append one reached-state input row and index it. */
    pub(super) fn push_state_value_input(
        &mut self,
        input: super::state_values::StateValues,
    ) -> usize {
        let slot = self.state_value_inputs.len();
        let idx = input.state.arena_index() as usize;
        if self.reached_state_positions.len() <= idx {
            self.reached_state_positions
                .resize(idx + 1, (SymbolHandle::invalid(), 0));
        }
        self.reached_state_positions[idx] = (input.state, slot);
        self.state_value_inputs.push(input);
        slot
    }

    /*
        Wholesale replacement for the nonconvergent fallback: rebuild the
        position index for the fresh roster.
    */
    pub(super) fn replace_state_value_inputs(
        &mut self,
        inputs: Vec<super::state_values::StateValues>,
    ) {
        self.reached_state_positions.clear();
        for (index, input) in inputs.iter().enumerate() {
            let idx = input.state.arena_index() as usize;
            if self.reached_state_positions.len() <= idx {
                self.reached_state_positions
                    .resize(idx + 1, (SymbolHandle::invalid(), 0));
            }
            self.reached_state_positions[idx] = (input.state, index);
        }
        self.state_value_inputs = inputs;
    }

    /*
        `(machines index, in-machine state index)` of a state symbol anywhere
        in the program -- the shape `TypedTrees::state_by_symbol` resolves.
    */
    pub(super) fn state_location(
        &self,
        _program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
    ) -> Option<(usize, usize)> {
        SymbolPositions::find(&self.shared_tables.symbol_positions.states, state_symbol)
    }

    pub(super) fn guard_expression_at(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&checked_trees::CheckedBooleanExpression> {
        let occurrence = (state, statement, expression);
        let order_key = |(state, statement, expression): (
            SymbolHandle,
            usize,
            typed_trees::expression::ExpressionHandle,
        )| {
            (
                state.arena_index(),
                state.generation(),
                statement,
                expression.arena_index(),
                expression.generation(),
            )
        };
        let position = match self
            .guard_predicates
            .binary_search_by_key(&order_key(occurrence), |prepared| {
                order_key(prepared.occurrence)
            }) {
            Ok(position) => position,
            Err(position) => {
                let expression = boolean_predicates::lower_current_guard(
                    program,
                    self.operators,
                    state,
                    statement,
                    expression,
                );
                self.guard_predicates.insert(
                    position,
                    GuardPreparation {
                        occurrence,
                        expression,
                    },
                );
                position
            }
        };
        self.guard_predicates[position].expression.as_ref()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn completed_guard_predicate_at(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        value: bool,
        writes: arena::HandleSpan<checked_trees::FlowOperandReferent>,
        unknown_write: bool,
    ) -> Option<boolean_predicates::BoundPredicate> {
        let expression = self
            .guard_expression_at(program, state, statement, expression)?
            .clone();
        boolean_predicates::bind_current_expression(program, state, expression, value, |operand| {
            super::observations::current_scalar_observation(
                program,
                self,
                state,
                statement,
                writes,
                unknown_write,
                operand.scalar()?,
            )
        })
    }

    pub(super) fn complete_predicate_bindings(
        &self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        mut predicate: boolean_predicates::BoundPredicate,
    ) -> Option<boolean_predicates::BoundPredicate> {
        for binding in &mut predicate.bindings {
            let observed = super::observations::scalar_observation_at(
                program,
                self,
                state,
                statement,
                binding.operand.scalar()?,
            )?;
            if program.primitive_type_reference(observed.reference)
                != binding.operand.primitive_type()
            {
                return None;
            }
            binding.place = observed.place;
        }
        Some(predicate)
    }

    /*
        Rebase the actual completed bindings at this operand-write cut. A saved
        copy keeps its own place; an unresolved copy never falls back to newer
        storage at the source spelling. Every original-entry observation must
        independently survive the evaluated prefix before it can be published.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn entry_predicate_at(
        &self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        predicate: &boolean_predicates::BoundPredicate,
        writes: arena::HandleSpan<checked_trees::FlowOperandReferent>,
        unknown_write: bool,
    ) -> Option<checked_trees::CheckedBooleanExpression> {
        let (machine, _) = validation::find_state(program, state)?;
        boolean_predicates::entry_expression_with(program, machine.symbol, predicate, |place| {
            if !super::entry_origin_is_stable(program, &self.control, writes, unknown_write, place)
            {
                return None;
            }
            entry_parameter_place_before_statement(program, machine.symbol, state, statement, place)
        })
    }

    /*
        Current value identity differs from address syntax when an arm has
        already copied its payload. Reference descendants keep their saved
        descriptor's actual referent, whose contents remain subject to writes.
    */
    pub(super) fn completed_read_place_at(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<validation::CanonicalPlace> {
        let place = self.canonical_place_at(program, state, statement, expression)?;
        if self.active_completed_copies.is_empty() {
            return Some(place);
        }
        self.completed_read_place(program, state, statement, expression, &place)
    }

    pub(super) fn completed_read_place(
        &self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        place: &validation::CanonicalPlace,
    ) -> Option<validation::CanonicalPlace> {
        super::observations::completed_copy_read_place(
            program,
            &self.control,
            self.active_completed_copies,
            state,
            statement,
            expression,
            place,
        )
    }

    pub(super) fn canonical_place_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<validation::CanonicalPlace> {
        place_at(program, state_symbol, statement_index, expression)
    }

    /*
        Reuse the source provenance, never a previous prefix's admission. The
        predicate reader may query a leaf speculatively while distinguishing
        integer from float meaning, so keep each callback and its live check.
        Its own path validation and work budget still run after this returns.
    */
    pub(super) fn entry_subject_at(
        &self,
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
        let place = place_at(program, state, statement, expression)?;
        if !super::entry_origin_is_stable(
            program,
            &self.control,
            entry_writes,
            entry_unknown_write,
            &place,
        ) {
            return None;
        }
        let (machine, _) = validation::find_state(program, state)?;
        entry_parameter_place_before_statement(program, machine.symbol, state, statement, &place)
    }

    pub(super) fn expression_result_type_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        validation::expression_result_type_reference(program, machine, state, expression)
    }

    pub(super) fn transition_call_target_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<typed_trees::statement::TransitionTargetHandle> {
        validation::transition_call_target(program, machine, state, statement_index, call_ordinal)
    }

    /*
        The solved origins of `machine` in `solved`, the table the flow build
        prepared by machine position. A machine outside that roster, or one
        whose states the table does not cover, is solved on the spot.
    */
    fn machine_origins(
        &self,
        solved: &[std::sync::Arc<entry_origins::MachineParameterOrigins>],
        machine: &typed_trees::machine::Machine,
    ) -> Option<std::sync::Arc<entry_origins::MachineParameterOrigins>> {
        let program = self.program;
        let position = self.machine_index(program, machine.symbol)?;
        let origins = solved.get(position)?;
        origins.covers(program, machine).then(|| origins.clone())
    }

    /*
        Entry-parameter origin chain for one state, read from its machine's
        prepared parameter-origin fixed point.
    */
    pub(super) fn entry_origin_chain_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Vec<(SymbolHandle, SymbolHandle)> {
        let solved = self
            .machine_origins(&self.shared_tables.machine_origins.parameters, machine)
            .unwrap_or_else(|| {
                std::sync::Arc::new(entry_origins::machine_parameter_origins(program, machine))
            });
        entry_origins::state_origins(program, machine, state, &solved)
    }

    pub(super) fn storage_origins_at(
        &self,
        machine: SymbolHandle,
    ) -> Option<std::sync::Arc<entry_origins::MachineParameterOrigins>> {
        let program = self.program;
        let machine = program.machine_by_symbol(machine)?;
        let frames = self.call_frames?;
        Some(
            self.machine_origins(&self.shared_tables.machine_origins.storage, machine)
                .unwrap_or_else(|| {
                    std::sync::Arc::new(entry_origins::machine_storage_origins(
                        program, frames, machine,
                    ))
                }),
        )
    }

    pub(super) fn dirty_return_dependents(&mut self, changed: &[usize]) {
        for &machine in changed {
            for &state in &self.shared_tables.return_dependents[machine] {
                crate::shared::collections::push_unique(&mut self.dirty_state_value_inputs, state);
            }
        }
    }

    pub(super) fn expression_occurrences_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Vec<typed_trees::expression::ExpressionHandle> {
        let mut occurrences = Vec::new();
        append_expression_occurrences(program, expression, &mut occurrences);
        occurrences
    }

    pub(super) fn proof_fact_occurrences_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> Vec<typed_trees::expression::ExpressionHandle> {
        fact_referenced_occurrences(program, fact)
    }

    /*
        The program's sorted integer-literal widening thresholds. The authored
        set cannot change mid-build, so the collection and its per-literal
        `BigInt` parsing memoize on first widening demand.
    */
    pub(super) fn integer_literal_thresholds_at(
        &self,
        program: &dyn ::validation::CheckingInput,
    ) -> std::sync::Arc<Vec<numerics::bignum::BigInt>> {
        self.shared_tables
            .integer_literal_thresholds
            .get_or_init(|| {
                std::sync::Arc::new(super::state_values::fields::integer_literal_thresholds(
                    program,
                ))
            })
            .clone()
    }

    /*
        The checked scalar row at `(state, statement ordinal)` whose binding
        `accepts`: `Some(Some(row))` is the one selected row, `Some(None)` is
        the coordinate's conflict -- more than one binding matched, or the
        selected binding's role owns no single expression row -- and `None` is
        no matching binding at all. The plans retain one custody row per
        selected scalar, so a conflict resolves to nothing rather than to the
        first row. An expected expression, destination or symbol namespace is
        checked after this lookup, so an expected value can never filter a
        conflict into agreement.
    */
    pub(super) fn selected_scalar_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        accepts: impl Fn(&checked_trees::CheckedScalarExpressionBindings) -> bool,
    ) -> Option<
        Option<(
            &'plans checked_trees::CheckedScalarExpressionBindings,
            &'plans checked_trees::CheckedScalarExpression,
        )>,
    > {
        let plans = self.scalar_expressions;
        let mut bindings = self
            .shared_tables
            .scalar_bindings_by_site
            .get(&(state, statement_ordinal))
            .into_iter()
            .flatten()
            .map(|handle| plans.source_bindings.get(*handle))
            .filter(|binding| accepts(binding));
        let binding = bindings.next()?;
        if bindings.next().is_some() {
            return Some(None);
        }
        let mut expressions = self
            .shared_tables
            .scalar_expressions_by_site
            .get(&(state, statement_ordinal))
            .into_iter()
            .flatten()
            .map(|row| &plans.expressions[*row])
            .filter(|expression| expression.role == binding.role);
        let Some(expression) = expressions.next() else {
            return Some(None);
        };
        if expressions.next().is_some() {
            return Some(None);
        }
        Some(Some((binding, &expression.expression)))
    }

    /*
        Storage-write places for one statement site. The enumeration walks
        local origins, origin paths, and alias closures per statement, all
        program-pure on (state, index): the statement sits at that index in
        that state, its machine is implied, and the call-frame resolver is
        the shared one the whole build carries.
    */
    pub(super) fn statement_storage_writes_at(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &checked_trees::statement::StatementNode,
    ) -> Option<Vec<validation::CanonicalPlace>> {
        crate::flow::statement_storage_writes(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            self.call_frames,
        )
    }

    /*
        Discard a complete unpublished sweep, retaining only reusable storage.
        StateValues contains owned evidence and typed-tree handles, never handles
        into these output arenas or the changing semantic plan. The immutable
        plans and mutation cache likewise retain no sweep-local handles.
        Completed return predicates likewise retain only source identities;
        keep them for the next sweep, clearing them on fallback or group exit.

        This is the former whole-context replacement boundary, not a partial
        rollback: all output rows must disappear together, before semantic
        restoration and before another transfer. No copied output handle may
        survive reuse. Published output has already left this context, because
        `take_output_pages` takes the finished pages before this reset, so a
        reset can only empty unpublished roots. Preserve these constraints when
        adding a context field.
    */
    pub(super) fn discard_output(&mut self) {
        self.active_completed_copies = arena::HandleSpan::empty();
        self.assignment_observations.clear();
        self.contexts.reset_pages();
        self.invalidations.reset_pages();
        self.borrow_lifetimes.reset_pages();
        self.boundaries.reset_pages();
        self.control.reset_pages();
        self.new_state_field_input_height = 0;
    }

    /* Write the next machine's rows on page `generation` of every output. */
    pub(super) fn begin_output_page(&mut self, generation: u32) {
        self.contexts.begin_page(generation);
        self.invalidations.begin_page(generation);
        self.borrow_lifetimes.begin_page(generation);
        self.boundaries.begin_page(generation);
        self.control.begin_page(generation);
    }

    /*
        Hand back the pages one machine group produced, leaving empty outputs
        and no carried value inputs, so the context can build another group.
        Program-pure memos survive: they hold no output handles.
    */
    pub(super) fn take_output_pages(&mut self) -> FlowFacts {
        let flow = FlowFacts::with_roots(
            self.contexts.take_additional_pages(),
            self.invalidations.take_additional_pages(),
            self.borrow_lifetimes.take_additional_pages(),
            FlowOwnershipFacts::default(),
            self.boundaries.take_additional_pages(),
            self.control.take_additional_pages(),
        );
        self.discard_output();
        self.state_value_inputs.clear();
        self.reached_state_positions.clear();
        self.dirty_state_value_inputs.clear();
        self.normal_return_predicates.clear();
        flow
    }
}

/* The semantic context handles active at one program point, in span order. */
pub(crate) fn active_context_handles(
    contexts: &checked_trees::FlowContextFacts,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
) -> Vec<facts::FactContextHandle> {
    contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>()
}

/* The call flow facts a state's call span names, in span order. */
pub(crate) fn call_flows(
    control: &checked_trees::FlowControlFacts,
    calls: arena::HandleSpan<checked_trees::FlowCallFact>,
) -> &[checked_trees::FlowCallFact] {
    control.calls.span_or_empty(calls)
}

/* The statements a state's statement span names, in authored order. */
pub(crate) fn state_statements(
    program: &dyn ::validation::CheckingInput,
    statements: arena::HandleSpan<typed_trees::statement::StatementNode>,
) -> &[typed_trees::statement::StatementNode] {
    program.statement_table.statements(statements)
}

/*
    The one value a state returns: a trailing expression, or one transition
    whose only target is a value. Locals may precede it. Any other statement
    is a different control shape and names no single returned value.
*/
pub(crate) fn state_returned_value(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<typed_trees::expression::ExpressionHandle> {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    let mut returned = None;
    for statement in state_statements(program, state.statement_nodes) {
        match statement {
            StatementNode::LocalData(_) => {}
            StatementNode::Expression(value) if returned.is_none() => returned = Some(*value),
            StatementNode::Transition(transition) if returned.is_none() => {
                let mut value = None;
                for target in transition.targets() {
                    match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Value(expression) if value.is_none() => {
                            value = Some(*expression);
                        }
                        _ => return None,
                    }
                }
                returned = Some(value?);
            }
            _ => return None,
        }
    }
    returned
}

/*
    The value a machine's published result contract names: the first unnamed
    `ensures` conjunct `result == value` (either side) whose value does not
    itself read the result. The callee proves that equation on every normal
    exit, so a caller may use it whatever control shape the body has.
*/
pub(crate) fn contract_returned_value(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Option<typed_trees::expression::ExpressionHandle> {
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::signature::SignatureContractKind;
    let entry = program.machine_states(machine).first()?;
    if program
        .state_parameters(entry)
        .iter()
        .any(|parameter| parameter.name.as_str() == "result")
    {
        return None;
    }
    let names_result = |expression: ExpressionHandle| {
        matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
            if matches!(program.expression_table.name_path_members(path.members), [member]
                if member.as_str() == "result"))
    };
    let reads_result = |expression: ExpressionHandle| {
        let mut found = false;
        ::validation::expression_traversal::visit_post_order(
            program,
            expression,
            &mut ::validation::expression_traversal::EnteredExpressions::default(),
            |handle, _| found |= names_result(handle),
        );
        found
    };
    let mut pending = program
        .machine_contracts(machine)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .filter(|contract| contract.binding.is_none())
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| fact.as_expression().copied())
        .collect::<Vec<_>>();
    pending.reverse();
    while let Some(expression) = pending.pop() {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            continue;
        };
        match binary.operator {
            BinaryOperator::And => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            BinaryOperator::Equal => {
                for (result, value) in [(binary.left, binary.right), (binary.right, binary.left)] {
                    if names_result(result) && !reads_result(value) {
                        return Some(value);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/* The statements a state's statement span names, paired with their index. */
pub(crate) fn indexed_statements(
    program: &dyn ::validation::CheckingInput,
    statements: arena::HandleSpan<typed_trees::statement::StatementNode>,
) -> impl Iterator<Item = (usize, &typed_trees::statement::StatementNode)> {
    program.statement_table.indexed_statements(statements)
}

/* The expression table of the program under check. */
pub(crate) fn expressions(
    program: &dyn ::validation::CheckingInput,
) -> &typed_trees::expression::ExpressionTable {
    &program.expression_table
}

/* Every fact payload the active contexts carry, in span order. */
pub(super) fn active_context_facts(
    contexts: &FlowBuildContext<'_>,
    semantic: &facts::FactPlan,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
) -> Vec<facts::Fact> {
    active_context_facts_matching(contexts, semantic, active, |_| true)
}

/*
    Snapshot only the facts a consumer will inspect, at the same active cut.
    Filtering precedes allocation; source order and repeated facts survive.
*/
pub(super) fn active_context_facts_matching(
    contexts: &FlowBuildContext<'_>,
    semantic: &facts::FactPlan,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
    mut selected: impl FnMut(&facts::Fact) -> bool,
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
        .filter(|fact| selected(fact))
        .copied()
        .collect::<Vec<_>>()
}

/*
    The case-selection gate every source transfer applies before writing:
    the statement's active contexts must select every case the source path's
    segments name.
*/
pub(super) fn source_transfer_cases_selected(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &FlowBuildContext<'_>,
    active: arena::HandleSpan<checked_trees::FlowSemanticContextRef>,
    point: facts::ProgramPoint,
    root: facts::PlaceRoot,
    segments: Vec<facts::PlaceSegment>,
) -> bool {
    let context_handles = active_context_handles(&contexts.contexts, active);
    let facts::ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = point
    else {
        return false;
    };
    place_cases_are_selected(
        program,
        semantic,
        &context_handles,
        machine_symbol,
        state_symbol,
        statement_index,
        &validation::CanonicalPlace { root, segments },
    )
}

#[cfg(test)]
mod tests {
    use super::FlowBuildContext;
    use checked_trees::{
        CheckedLocatedScalarExpression, CheckedScalarExpression, CheckedScalarExpressionBindings,
        CheckedScalarExpressionPlans, CheckedScalarExpressionRole,
    };

    #[test]
    fn selected_scalar_keeps_missing_or_ambiguous_rows_authoritative() {
        let program = typed_trees::TypedTrees::default();
        let borrow = checked_trees::BorrowFacts::default();
        let proof = checked_trees::ProofFacts::default();
        let semantic = facts::FactPlan::default();
        let operators = checked_trees::CheckedOperatorFacts::default();
        let mutation = crate::flow::StateMutationSummaryCache::default();
        let state = symbols::SymbolHandle::invalid();
        let role = CheckedScalarExpressionRole::UnitCallArgument {
            call_ordinal: 0,
            argument_ordinal: 0,
        };
        for (binding_count, expression_count, expected) in [
            (0, 0, None),
            (1, 0, Some(false)),
            (1, 1, Some(true)),
            (1, 2, Some(false)),
            (2, 1, Some(false)),
        ] {
            let mut plans = CheckedScalarExpressionPlans::default();
            for _ in 0..binding_count {
                plans
                    .source_bindings
                    .append(CheckedScalarExpressionBindings {
                        state,
                        role,
                        ..Default::default()
                    });
            }
            for expression_role in std::iter::once(CheckedScalarExpressionRole::Return)
                .chain(std::iter::repeat_n(role, expression_count))
            {
                plans.expressions.push(CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal: 0,
                    role: expression_role,
                    expression: CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: typed_trees::types::PrimitiveType::U64,
                    },
                });
            }
            let context = FlowBuildContext::new(
                &program, &borrow, &proof, &semantic, &plans, &operators, None, &mutation,
            );
            let actual = context
                .selected_scalar_at(state, 0, |binding| binding.role == role)
                .map(|selected| selected.is_some());
            assert_eq!(
                actual, expected,
                "bindings={binding_count}, expressions={expression_count}"
            );
        }
    }
}
