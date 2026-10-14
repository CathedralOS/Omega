//! One authored state of the composed-control graph: its scalar prefix and
//! call operations, the guard or case dispatch, the successor edges with
//! their staged transfers and ranks, and the blocks it leaves behind. Each
//! successor edge lowers through `successor_edge`, which reads the state's
//! fixed facts from a `SuccessorFrame`. Before those edges exist, a
//! conditional return's value arm lowers through `conditional_return` and an
//! ordered multi-arm tail stages its later guards through `guarded_chain`.
//! `seal` then disposes of exit custody and publishes the state's blocks,
//! rank and counters.

use super::super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralCaseSuccessorEdge, SuccessorEdge, block_id,
};
use super::super::super::super::{
    Block, Terminator, ValueDeclaration, allocate_dense, edge_id, unsupported,
};
use super::super::super::LoweringError;
use super::super::{CheckedStructuralControlSuccessorPlan, case_emission, ranking};
use super::StateGraphEmission;
use crate::emission::boolean_control::LoweredBooleanDecision;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::{CollectionLengthObservations, OperationBuffer};
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
use crate::proofs::crash_routes::{lower_checked_crash_exit, lower_structural_crash_predicates};

impl StateGraphEmission<'_, '_> {
    /// Standalone and selected crash exits share the exact authored crash site.
    /// No normal cleanup or result establishment is performed on this edge.
    pub(super) fn crash_completion(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        statement_ordinal: u32,
        next_edge: &mut u64,
    ) -> Result<Terminator, LoweringError> {
        let state = &self.plan.states[position];
        let crash = lower_checked_crash_exit(
            self.checked,
            self.admitted.authored[position].0.symbol,
            state.state,
            statement_ordinal,
            &self.claims.source_claims,
        )?;
        Ok(Terminator::Crash {
            edge: edge_id(allocate_dense(next_edge)?),
            cause: crash.cause,
            site_guard: self.crash_site_guards(identities, position, &crash.site_guard)?,
            frontier_lower_bound: crash.frontier_lower_bound,
        })
    }

    fn crash_site_guards(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
        predicates: &[checked_trees::CheckedBooleanExpression],
    ) -> Result<Vec<terminal_psi::CrashPredicateTerm>, LoweringError> {
        if predicates.is_empty() {
            return Ok(Vec::new());
        }
        let owner = self.admitted.authored[position].0;
        if owner.symbol == self.plan.machine {
            let signature = crate::unit::attached_unit::signatures::find(
                &self.catalogs.signatures,
                owner.symbol,
            )?;
            return lower_structural_crash_predicates(
                identities,
                predicates,
                &signature.scalar_parameters,
                &signature.predicate_parameters,
                &self.catalogs.structural_types,
                &signature.runtime_requirements,
            );
        }

        // A fused owner has no separate machine formals. Its exact entry block
        // supplies the invocation actuals; ordinary Terminal dominance and
        // guard checking must still prove their availability at this exit.
        // Keep the immutable contract roster: sealing a state drains its views.
        let entry =
            self.checked
                .machine_states(owner)
                .first()
                .ok_or(LoweringError::Unsupported(
                    "fused crash owner has no invocation entry",
                ))?;
        let entry_position = self
            .admitted
            .authored
            .iter()
            .position(|(candidate, state)| {
                candidate.symbol == owner.symbol && state.symbol == entry.symbol
            })
            .filter(|position| self.admitted.live[*position])
            .ok_or(LoweringError::Unsupported(
                "fused crash predicate has no live invocation entry namespace",
            ))?;
        let actual_values = &self.state_values[entry_position];
        let actual_places = self.state_contract_rosters[entry_position]
            .iter()
            .map(|(position, declaration)| {
                let mut parameter = declaration.clone();
                parameter.position = *position;
                parameter
            })
            .collect::<Vec<_>>();
        // Fused edges need not allocate a standalone callee signature. Lower
        // premises and guards directly into the retained entry-block roster,
        // so no declaration-local value or place leaks into the graph.
        let requirements = crate::unit::attached_unit::entry_requirements::lower(
            identities,
            self.checked,
            owner.symbol,
            actual_values,
            &self.state_erased[entry_position],
            &actual_places,
            &self.catalogs.structural_types,
        )?;
        lower_structural_crash_predicates(
            identities,
            predicates,
            actual_values,
            &actual_places,
            &self.catalogs.structural_types,
            &requirements.runtime_requirements,
        )
    }

    /// Emit the state at `position` in authored order.
    pub(super) fn emit_state(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        position: usize,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let state = &plan.states[position];
        // A fused roster mixes owners: fact lookups for this state resolve
        // its own machine, while scalar computation tables merge every
        // owner the roster admits.
        let owner = self.admitted.authored[position].0.symbol;
        let owners = self
            .admitted
            .authored
            .iter()
            .map(|(owner, _)| owner.symbol)
            .collect::<Vec<_>>();
        let state_parameters = self.state_views[position]
            .iter()
            .zip(&state.structural_parameters)
            .map(|(parameter, source)| {
                // Source readers use authored positions in mixed signatures.
                let mut parameter = parameter.clone();
                parameter.position = source.position;
                parameter
            })
            .collect::<Vec<_>>();
        let mut operations = OperationBuffer::new(
            self.catalogs.next_operation - 1,
            std::sync::Arc::new(crate::unit::dynamic_dispatch::DynamicDispatchIndex::new(
                checked,
            )),
        );
        let mut evaluation = crate::unit::attached_unit::argument_evaluation::Evaluation {
            shared_qualifications: self
                .catalogs
                .prepared_scalar_qualifications
                .cloned()
                .unwrap_or_else(
                crate::expression_preparation::qualifications::PreparedScalarQualifications::empty,
            ),
            machine: Some(self.terminal_machine),
            qualification_coercions: Vec::new(),
            structural_types: std::sync::Arc::from([]),
            structural_value_owners: Vec::new(),
            selection_cleanups: Vec::new(),
            computation_address_roots: Vec::new(),
            structural_locals: Vec::new(),
            view_locals: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
            place_observations:
                crate::scalar_lowering::computations::fields::prepare_place_observations(
                    identities,
                    checked,
                    plan.machine,
                    state.state,
                    &self.catalogs.structural_types,
                ),
            record_fields: crate::scalar_lowering::computations::fields::prepare(
                identities,
                checked,
                plan.machine,
                &owners,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            literals: crate::scalar_lowering::computations::literals::prepare(
                identities,
                checked,
                plan.machine,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            copies: crate::scalar_lowering::computations::copies::prepare(
                identities,
                checked,
                plan.machine,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            produced_values: crate::scalar_lowering::computations::produced_values::prepare(
                identities,
                checked,
                &owners,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            subslices: crate::scalar_lowering::computations::subslices::prepare(
                identities,
                checked,
                &owners,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            cases: crate::scalar_lowering::computations::cases::prepare(
                identities,
                checked,
                &owners,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            primitive_storage: Vec::new(),
            parameter_storage: Vec::new(),
            scalar_bindings: crate::expression_preparation::bindings::ScalarBindings::new(
                std::iter::empty(),
            ),
            structural_fields: Default::default(),
            structural_cases: Vec::new(),
            previous_parameter_homes: Vec::new(),
            structural_parameters: state
                .structural_parameters
                .iter()
                .zip(&state_parameters)
                .map(|(source, parameter)| (source.position, parameter.clone()))
                .collect(),
            erased_scalar_formals: self.state_erased[position].clone(),
            erased_proof_formals: state.erased_proof_parameters.clone(),
            entry: self.state_ids[position],
            block_structural_parameters: Vec::new(),
            current: self.state_ids[position],
            parameters: if position == 0 && !self.entry_reentered {
                Vec::new()
            } else {
                self.state_values[position].clone()
            },
            operation_start: 0,
            blocks: Vec::new(),
        };
        // A `&`-primitive state parameter arrives by edge argument: its
        // referent storage joins the state's primitive-storage table exactly
        // as a machine-level `&` parameter's does, so a bare referent read
        // resolves the loaned scalar. `state_parameters` carry authored
        // positions, not the dense signature index.
        evaluation.primitive_storage =
            crate::unit::attached_unit::parameters::primitive_reference_storage(
                checked,
                state.state,
                &state.structural_parameters,
                &evaluation.structural_parameters,
            )?;
        let mut values = self.state_values[position].clone();
        // A `mut` scalar formal owns storage from arrival: the state
        // establishes it from the arriving value before its first statement,
        // as an ordinary body does, so the formal's stores, loans and later
        // reads all name one place. Its operation sequences store into it by
        // symbol (`Evaluation::parameter_storage`).
        let parameter_storage =
            super::super::super::super::primitive_locals::establish_parameter_storage(
                &state.parameter_storage,
                &state.scalar_parameters,
                &values,
                &self.catalogs.type_ids,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
                &mut self.catalogs.scalar_calls.next_call_obligation,
                &mut operations,
            )?;
        for storage in &parameter_storage {
            self.catalogs
                .temporary_places
                .push(storage.local.declaration);
        }
        evaluation.parameter_storage = parameter_storage
            .iter()
            .map(|storage| storage.local)
            .collect();
        let mut next_value = self.catalogs.next_value;
        let mut next_block = self.catalogs.next_block;
        let mut next_edge = self.catalogs.next_edge;
        // The state's namespace starts from its scalar formals, its arriving
        // referents and its formal storage; its statements, immutable locals
        // included, lower through the one operation sequence.
        evaluation.scalar_bindings = crate::expression_preparation::bindings::ScalarBindings::entry(
            values.iter().map(|value| value.scalar_type),
            &evaluation.primitive_storage,
            &evaluation.structural_parameters,
            &self.catalogs.structural_types,
            &evaluation.place_observations,
        );
        super::super::super::super::primitive_locals::register_mut_formal_storage_homes(
            &mut evaluation,
            &parameter_storage,
        )?;
        evaluation.structural_types = self.catalogs.structural_types.as_ref().into();
        evaluation.structural_fields =
            crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
                &evaluation.structural_parameters,
                &self.catalogs.structural_types,
            );
        evaluation.structural_cases =
            crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
                &evaluation.structural_parameters,
                &self.catalogs.structural_types,
            );
        evaluation.element_views = crate::expression_preparation::bindings::element_views(
            &evaluation.structural_parameters,
            &self.catalogs.structural_types,
        );
        // Observe the immutable arrival values before the first authored
        // statement. Mutable formal homes have been established above, but
        // the retained root's Parameters bind the captured scalar snapshots.
        let current_rank = if let Some(root) = ranking::observation(checked, plan, state)? {
            let scalar_slots = values
                .iter()
                .enumerate()
                .map(|(position, value)| (position, value.scalar_type))
                .collect::<Vec<_>>();
            let structural_actuals = state_parameters
                .iter()
                .map(|parameter| {
                    (
                        terminal_psi::StructuralArgument {
                            place: parameter.place,
                            path: Vec::new(),
                            access: parameter.access,
                        },
                        parameter.structural_type,
                    )
                })
                .collect::<Vec<_>>();
            let bindings = ranking::formal_bindings(
                checked,
                state,
                &evaluation.scalar_bindings,
                &scalar_slots,
                &structural_actuals,
            )?;
            let mut calls = self.catalogs.scalar_calls.emission_context();
            let observation_entry = evaluation.current;
            let observed = evaluation.retained_observation(
                identities,
                checked,
                root,
                &bindings,
                &mut values,
                &mut next_value,
                &mut next_block,
                &mut next_edge,
                &mut operations,
                &mut calls,
            )?;
            self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
            // Admitted source measures are pure scalar observations. Their
            // prepared chains complete inline before the body, so this value
            // dominates every cyclic block carrying the state's rank. Never
            // attach a later completion's SSA value to pre-observation blocks
            // if a retained root unexpectedly contains selective control.
            if evaluation.current != observation_entry {
                return unsupported("Unit graph entry rank has pre-observation control");
            }
            if observed.scalar_type.as_integer().is_none() {
                return unsupported("Unit graph rank observation lost its integer carrier");
            }
            Some(observed.id)
        } else {
            None
        };
        super::super::super::emission::emit_call_operations(
            identities,
            checked,
            owner,
            state,
            &state.operations,
            self.catalogs,
            &state_parameters,
            self.claims.published(),
            &mut evaluation,
            &mut values,
            &self.state_erased[position],
            &mut next_value,
            &mut next_block,
            &mut next_edge,
            &mut operations,
        )?;
        let current_parameters = evaluation.current_parameters(&state_parameters);
        let state_parameters = current_parameters.as_ref();
        if operations.external_completion.is_some() {
            evaluation.remap_transported_call_operands(&mut operations);
            let terminator = operations.take_external_completion(&mut next_edge)?.ok_or(
                LoweringError::Unsupported("external completion lost its evaluated invocation"),
            )?;
            let body_end = operations.len();
            return self.seal_state(
                identities,
                position,
                terminator,
                Vec::new(),
                evaluation,
                operations,
                super::seal::SealFrame {
                    state_parameters,
                    current_rank,
                    is_guarded_return: false,
                    dispatches: false,
                    body_end,
                    next_value,
                    next_block,
                    next_edge,
                },
            );
        }
        // Successors observe every local the state body established, through
        // the same custody-preserving context ordinary arguments consume.
        let bindings = evaluation.contextual_bindings();
        let mut branch_guard = None;
        // The shared scalar decision is recorded under the true arm's
        // transition ordinal — the named successor's for `Conditional`, or the
        // authored `(expression)` arm's when the return sits first.
        let conditional_true_ordinal = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } => {
                Some(when_true.statement_ordinal)
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                jump,
                return_arm,
                return_when_true,
                ..
            } => {
                if *return_when_true {
                    return_arm.statement_ordinal()
                } else {
                    Some(jump.statement_ordinal)
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                when_true, ..
            } => when_true.statement_ordinal(),
            _ => None,
        };
        let capture_boundary = match conditional_true_ordinal {
            Some(statement) => statement,
            None => state.statement_count,
        };
        evaluation.prepare_storage_before(
            identities,
            self.checked,
            state.state,
            capture_boundary,
            &self.catalogs.structural_types,
            &values,
            &mut self.catalogs.next_place,
            &mut next_value,
            &mut operations,
            &mut self.catalogs.temporary_places,
        )?;
        // The selected outcome may evaluate payload reads in either successor
        // arguments or completion operands. Both inherit the guard's dispatch.
        let first_successor_reads_payloads = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } => {
                successor_may_read_payloads(when_true)
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                jump,
                return_when_true: false,
                ..
            } => successor_may_read_payloads(jump),
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { return_arm, .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                when_true: return_arm,
                ..
            } => return_may_read_payloads(return_arm),
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } => {
                arms.first().is_some_and(guarded_arm_may_read_payloads)
            }
            _ => false,
        };
        let condition = if let Some(true_ordinal) = conditional_true_ordinal {
            branch_guard = evaluation.branch_guard(
                identities,
                checked,
                state.state,
                true_ordinal,
                &values,
                first_successor_reads_payloads,
            )?;
            if branch_guard.is_some() {
                None
            } else {
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let condition = evaluation.guard_value(
                    identities,
                    checked,
                    owner,
                    state.state,
                    true_ordinal,
                    &mut values,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut calls,
                )?;
                self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                Some(condition.id)
            }
        } else if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } =
            &state.terminator
        {
            // Only the first guard is observed on the entry path. Every
            // later guard is staged below into its own private block. A
            // short-circuit guard is planned below exactly like a two-arm
            // conditional's, so it evaluates no value here.
            let Some(first) = arms.first() else {
                return unsupported("guarded jump chain lost its first arm");
            };
            if arms.len() < 2 {
                return unsupported("guarded jump chain lost its ordered arms");
            }
            branch_guard = evaluation.branch_guard(
                identities,
                checked,
                state.state,
                first.statement_ordinal(),
                &values,
                first_successor_reads_payloads,
            )?;
            if branch_guard.is_some() {
                None
            } else {
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let condition = evaluation.guard_value(
                    identities,
                    checked,
                    owner,
                    state.state,
                    first.statement_ordinal(),
                    &mut values,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut calls,
                )?;
                self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                Some(condition.id)
            }
        } else {
            None
        };
        let body_end = operations.len();
        let prepared_cases = case_emission::prepare(
            identities,
            state,
            self.catalogs,
            state_parameters,
            &operations,
            &mut next_value,
        )?;
        let guarded_return = super::super::guarded::emit(
            identities,
            checked,
            plan,
            state,
            self.catalogs,
            state_parameters,
            self.claims.published(),
            &mut evaluation,
            &mut values,
            &self.state_erased[position],
            &mut next_value,
            &mut next_block,
            &mut next_edge,
            &mut operations,
        )?;
        let is_guarded_return = guarded_return.is_some();
        // A scalar-result state evaluates the value its selected exit returns
        // through the same exit evaluator an ordinary scalar body uses; its
        // private arm and join blocks stay inside this state.
        let mut scalar_return = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: checked_trees::CheckedScalarReturnPlan::Exits(exits),
            } => {
                // Allocate each exit's obligations before the call context takes
                // its counter snapshot. Proofs are produced from the completed
                // graph, where the selected arm and result effects are known.
                let cleanups = super::super::cleanup::scalar_returns(
                    identities,
                    self.claims.published(),
                    self.checked,
                    state,
                    exits,
                    self.catalogs,
                    state_parameters,
                    &evaluation,
                    &operations,
                )?;
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let value = evaluation.scalar_control_result(
                    identities,
                    checked,
                    owner,
                    state.state,
                    exits,
                    &cleanups,
                    &mut values,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                    &mut calls,
                )?;
                self.catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
                Some(value)
            }
            // The operation sequence above already bound the returned value;
            // read it from the state's scalar namespace at its ordinal.
            CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
            } => {
                let slot = usize::try_from(binding.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(state.scalar_parameters.len()))
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph scalar return binding ordinal overflows",
                    ))?;
                let position = evaluation.scalar_bindings.immutable_position(slot)?;
                let value = values
                    .get(position)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph scalar return binding is absent",
                    ))?;
                Some(value)
            }
            _ => None,
        };
        // A short-circuit guard is planned before the successor closure so
        // its case dispatches can allocate payload values; the true edge's
        // arguments then read the payloads its dispatches established. It is
        // planned before any later chain guard evaluates, over the namespace
        // the first guard observes.
        let planned_guard = match &branch_guard {
            Some(expression) => Some(plan_short_circuit_guard(
                expression,
                &values,
                &self.catalogs.structural_types,
                &evaluation.structural_parameters,
                &mut next_value,
                first_successor_reads_payloads,
            )?),
            None => None,
        };
        let inherited_lengths = operations.length_observations.clone();
        // Ordered multi-arm guards: every later guard is observed inside its
        // own private block reached only along the previous decision's false
        // edge. Their evaluation drafts are staged before the successor-edge
        // closure exists; the terminator match below assembles the chain.
        let (guarded_decisions, guarded_drafts, guarded_first_namespace) =
            if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } =
                &state.terminator
            {
                self.stage_guarded_chain(
                    identities,
                    position,
                    arms,
                    &mut values,
                    &mut evaluation,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
        // The authored `(expression)` arm lowers its value producer into its
        // own block closed by ReturnStructural now, before the successor-edge
        // closure borrows this state's emission slots. The staged edge joins
        // the conditional arms below as the return block's jump target.
        let mut conditional_return_edge =
            if let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                return_arm,
                return_when_true,
                ..
            } = &state.terminator
            {
                let arm_namespace = match &planned_guard {
                    Some((_, namespace)) if *return_when_true => namespace.clone(),
                    _ => values.clone(),
                };
                Some(self.conditional_return_arm(
                    identities,
                    position,
                    state_parameters,
                    arm_namespace,
                    match &planned_guard {
                        Some((planned, _)) if *return_when_true => planned.established.as_slice(),
                        _ => &[],
                    },
                    &[],
                    &[],
                    return_arm,
                    &mut evaluation,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?)
            } else {
                None
            };
        // Both arms of a conditional completion lower into private
        // completion blocks, each seeded with the namespace its guard outcome
        // observes, before the successor-edge closure borrows this state.
        let mut completion_edges =
            if let CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                when_true,
                when_false,
                ..
            } = &state.terminator
            {
                let true_namespace = match &planned_guard {
                    Some((_, namespace)) => namespace.clone(),
                    None => values.clone(),
                };
                let true_edge = self.conditional_return_arm(
                    identities,
                    position,
                    state_parameters,
                    true_namespace,
                    planned_guard
                        .as_ref()
                        .map_or(&[], |(guard, _)| guard.established.as_slice()),
                    &[],
                    &[],
                    when_true,
                    &mut evaluation,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?;
                let false_edge = self.conditional_return_arm(
                    identities,
                    position,
                    state_parameters,
                    values.clone(),
                    &[],
                    &[],
                    &[],
                    when_false,
                    &mut evaluation,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?;
                Some((true_edge, false_edge))
            } else {
                None
            };
        // A closed-sum case whose arm is an authored `(expression)` returns
        // it: mint each one's private return block exactly like a conditional
        // return arm's, before the successor-edge closure borrows this
        // state's emission slots. Ordinary selected-exit cleanup owns the
        // subject's residual partition after the arm's operand transfers.
        // Cases sharing one payload-free returning arm complete through one
        // block: the arm's operands and call run once, so its source
        // coordinates stay unique. Each later case enters through its own
        // staged block that jumps there, as a successor edge does, so no
        // block follows two dispatch decisions.
        let mut shared_return_blocks = Vec::new();
        let closed_return_edges = if let Some(prepared) = &prepared_cases {
            prepared
                .cases
                .iter()
                .map(|case| match &case.outcome {
                    checked_trees::CheckedClosedSumCaseOutcomePlan::Return(return_arm)
                        if case.values.is_empty()
                            && shared_return_blocks
                                .iter()
                                .any(|(arm, _)| *arm == return_arm) =>
                    {
                        let (_, target) = shared_return_blocks
                            .iter()
                            .find(|(arm, _)| *arm == return_arm)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph shared return arm lost its block",
                            ))?;
                        let staged = block_id(allocate_dense(&mut next_block)?);
                        evaluation.blocks.push(Block::plain(
                            staged,
                            Vec::new(),
                            Terminator::Jump {
                                edge: edge_id(allocate_dense(&mut next_edge)?),
                                target: *target,
                                arguments: Vec::new(),
                                erased_arguments: Vec::new(),
                                erased_proof_arguments: Vec::new(),
                                structural_arguments: Vec::new(),
                                cleanup_actions: Vec::new(),
                            },
                        ));
                        Ok(Some(SuccessorEdge {
                            edge: edge_id(allocate_dense(&mut next_edge)?),
                            target: staged,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            cleanup_actions: Vec::new(),
                        }))
                    }
                    checked_trees::CheckedClosedSumCaseOutcomePlan::Return(return_arm) => {
                        let payload_parameters = case
                            .values
                            .iter()
                            .map(|(_, value)| *value)
                            .collect::<Vec<_>>();
                        let mut namespace = values.clone();
                        namespace.extend(&payload_parameters);
                        let established = case
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(ordinal, field)| EstablishedCasePayload {
                                source: prepared.source,
                                path: prepared.path.clone(),
                                case: case.identity,
                                field: *field,
                                position: values.len() + ordinal,
                            })
                            .collect::<Vec<_>>();
                        let edge = self.conditional_return_arm(
                            identities,
                            position,
                            state_parameters,
                            namespace,
                            &established,
                            &payload_parameters,
                            &[],
                            return_arm,
                            &mut evaluation,
                            &mut next_value,
                            &mut next_block,
                            &mut next_edge,
                            &mut operations,
                        )?;
                        if case.values.is_empty() {
                            shared_return_blocks.push((return_arm, edge.target));
                        }
                        Ok(Some(edge))
                    }
                    checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(_) => Ok(None),
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
        } else {
            Vec::new()
        };
        // A mid-chain `(expression)` arm of a guarded-jump tail returns under
        // its own guard the same way the fallback does: mint each such arm's
        // private return block now, before the successor-edge closure
        // borrows this state's emission slots — seeded with the namespace
        // that arm's guard evaluation left.
        let mut guarded_arm_return_edges: Vec<Option<SuccessorEdge>> =
            if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } =
                &state.terminator
            {
                let mut staged = Vec::with_capacity(arms.len());
                for (index, arm) in arms.iter().enumerate() {
                    match arm {
                        checked_trees::CheckedGuardedJumpArmPlan::Jump(_) => staged.push(None),
                        checked_trees::CheckedGuardedJumpArmPlan::Return(plan) => {
                            let (namespace, established) = if index == 0 {
                                match &planned_guard {
                                    Some((guard, namespace)) => {
                                        (namespace.clone(), guard.established.as_slice())
                                    }
                                    None => (guarded_first_namespace.clone(), &[][..]),
                                }
                            } else {
                                let draft = &guarded_drafts[index - 1];
                                match &draft.4 {
                                    ChainGuard::Decision((guard, namespace)) => {
                                        (namespace.clone(), guard.established.as_slice())
                                    }
                                    ChainGuard::Value(_) => (draft.5.clone(), &[][..]),
                                }
                            };
                            staged.push(Some(self.conditional_return_arm(
                                identities,
                                position,
                                state_parameters,
                                namespace,
                                established,
                                &[],
                                &[],
                                &plan.return_arm,
                                &mut evaluation,
                                &mut next_value,
                                &mut next_block,
                                &mut next_edge,
                                &mut operations,
                            )?));
                        }
                    }
                }
                staged
            } else {
                Vec::new()
            };
        // A guarded-jump tail whose `_` arm is an authored `(identities, expression)`
        // returns it: mint its private return block exactly like a
        // conditional return arm's, before the successor-edge closure
        // borrows this state's emission slots.
        let mut guarded_return_edge =
            if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps {
                fallback: checked_trees::CheckedGuardedJumpFallbackPlan::Return(return_arm),
                ..
            } = &state.terminator
            {
                Some(self.conditional_return_arm(
                    identities,
                    position,
                    state_parameters,
                    values.clone(),
                    &[],
                    &[],
                    &[],
                    return_arm,
                    &mut evaluation,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?)
            } else {
                None
            };
        // Ordinary completion has one exact authored exit. Lower its checked
        // action stream after result evaluation, before successor emission
        // borrows the current value namespace. Scalar exits already completed
        // their exact cleanup on private edges before result reconvergence.
        let mut ordinary_return_cleanup = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: checked_trees::CheckedScalarReturnPlan::Exits(_),
            } => Some(Vec::new()),
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                let [cleanup] = state.normal_exit_cleanups.as_slice() else {
                    return unsupported("ordinary return has no unique source cleanup coordinate");
                };
                let mut regions = super::super::cleanup::for_exit_regions(
                    identities,
                    self.claims.published(),
                    self.checked,
                    state,
                    cleanup.exit,
                    self.catalogs,
                    state_parameters,
                    &evaluation,
                    &operations,
                )?;
                super::super::cleanup::selection(&evaluation, &operations, &mut regions)?;
                // Capture the authored result before hooks run. Private cleanup
                // edges transport that value, never evaluate the result again.
                let return_position = scalar_return.map(|value| {
                    values
                        .iter()
                        .position(|candidate| candidate.id == value.id)
                        .unwrap_or_else(|| {
                            values.push(value);
                            values.len() - 1
                        })
                });
                let actions = evaluation.emit_cleanup_regions(
                    identities,
                    self.checked,
                    regions,
                    &mut values,
                    &mut next_value,
                    &mut next_block,
                    &mut next_edge,
                    &mut operations,
                )?;
                if let Some(position) = return_position {
                    scalar_return = Some(values[position]);
                }
                Some(actions)
            }
            _ => None,
        };
        // A case test on a whole owned parameter consumes it, as the checked
        // cleanup records by leaving it out of the arm's discards: each tested
        // arm's edge and the dispatch's closing `_` edge dispose of it where
        // control leaves the state. A closed-sum dispatch disposes of it on a
        // returning arm's case edge, and through a successor arm's checked exit
        // cleanup on its staged jump, after operands transfer payload subtrees.
        let mut edge_blocks = Vec::new();
        let frame = super::successor_edge::SuccessorFrame {
            position,
            state_parameters,
            bindings: &bindings,
            current_rank,
            inherited_lengths: &inherited_lengths,
        };
        let mut successor = |edge: &CheckedStructuralControlSuccessorPlan,
                             payload_values: &[(u32, ValueDeclaration)],
                             case_edge: bool,
                             values: &[ValueDeclaration],
                             established: &[EstablishedCasePayload]|
         -> Result<SuccessorEdge, LoweringError> {
            self.successor_edge(
                identities,
                &frame,
                &mut super::successor_edge::EmittedState {
                    operations: &mut operations,
                    evaluation: &mut evaluation,
                    next_value: &mut next_value,
                    next_block: &mut next_block,
                    next_edge: &mut next_edge,
                    edge_blocks: &mut edge_blocks,
                },
                edge,
                payload_values,
                case_edge,
                values,
                established,
            )
        };
        let terminator = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { .. } => guarded_return.ok_or(
                LoweringError::Unsupported("guarded structural emission absent"),
            )?,
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } => {
                let returned_claims = crate::unit::attached_unit::claims::lower_return_claims(
                    identities,
                    checked,
                    owner,
                    state.state,
                    result,
                    &operations,
                    &self.claims.source_claims,
                    crate::terminal_identities::lookup_type_id(
                        &self.catalogs.type_ids,
                        &result.type_identity,
                    )?,
                    &self.catalogs.structural_types,
                )?;
                if self.content_entry_claims.iter().any(|entry| {
                    returned_claims
                        .iter()
                        .any(|binding| binding.claim == entry.claim)
                        && !self.content_identity_reshuffles.iter().any(|identity| {
                            identity.claim == entry.claim
                                && identity.input == entry.input
                                && identity.projections == entry.projections
                        })
                }) {
                    return unsupported(
                        "structural return lost its checked content identity guarantee",
                    );
                }
                let source = match result.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?.place(),
                    // A forwarded formal returns its own place: completion
                    // forwards only an owned or shared-view formal whose
                    // custody already carries the result's contract.
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => {
                        state_parameters
                            .get(parameter_index as usize)
                            .ok_or(LoweringError::Unsupported(
                                "structural graph return parameter missing",
                            ))?
                            .place
                    }
                    _ => return unsupported("structural graph return needs a whole owned value"),
                };
                Terminator::ReturnStructural {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    source: evaluation.current_structural_place(source),
                    returned_claims,
                    cleanup_actions: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                let cleanup_actions =
                    ordinary_return_cleanup
                        .take()
                        .ok_or(LoweringError::Unsupported(
                            "ordinary return lost its selected cleanup actions",
                        ))?;
                let edge = edge_id(allocate_dense(&mut next_edge)?);
                // Result shape does not change the selected cleanup schedule.
                match (&state.terminator, scalar_return) {
                    (
                        CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. },
                        Some(value),
                    ) => Terminator::Return {
                        edge,
                        value: value.id,
                        cleanup_actions,
                    },
                    (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, None) => {
                        Terminator::ReturnUnit {
                            edge,
                            cleanup_actions,
                        }
                    }
                    _ => return unsupported("Unit graph return lost its scalar value"),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Crash { statement_ordinal } => {
                self.crash_completion(identities, position, *statement_ordinal, &mut next_edge)?
            }
            CheckedComposedUnitControlTerminatorPlan::Jump { successor: edge } => {
                let edge = successor(edge, &[], false, &values, &[])?;
                Terminator::Jump {
                    edge: edge.edge,
                    target: edge.target,
                    arguments: edge.arguments,
                    erased_arguments: edge.erased_arguments,
                    erased_proof_arguments: edge.erased_proof_arguments,
                    structural_arguments: edge.structural_arguments,
                    cleanup_actions: edge.cleanup_actions,
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. } => {
                let (when_true, when_false) = match &state.terminator {
                    CheckedComposedUnitControlTerminatorPlan::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => (
                        match &planned_guard {
                            Some((planned, namespace)) => {
                                successor(when_true, &[], false, namespace, &planned.established)?
                            }
                            None => successor(when_true, &[], false, &values, &[])?,
                        },
                        successor(when_false, &[], false, &values, &[])?,
                    ),
                    CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                        jump,
                        return_when_true,
                        ..
                    } => {
                        let jump_edge = match &planned_guard {
                            Some((planned, namespace)) if !*return_when_true => {
                                successor(jump, &[], false, namespace, &planned.established)?
                            }
                            _ => successor(jump, &[], false, &values, &[])?,
                        };
                        let return_edge =
                            conditional_return_edge
                                .take()
                                .ok_or(LoweringError::Unsupported(
                                    "Unit graph conditional return lost its staged edge",
                                ))?;
                        if *return_when_true {
                            (return_edge, jump_edge)
                        } else {
                            (jump_edge, return_edge)
                        }
                    }
                    CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion { .. } => {
                        completion_edges.take().ok_or(LoweringError::Unsupported(
                            "Unit graph conditional completion lost its staged edges",
                        ))?
                    }
                    _ => unreachable!(),
                };
                if let Some((planned, guard_values)) = &planned_guard {
                    emit_short_circuit_decision(
                        identities,
                        self.checked,
                        planned,
                        guard_values,
                        [when_true, when_false],
                        ShortCircuitFrame {
                            erased_scalar_formals: &self.state_erased[position],
                            erased_proof_formals: &self.state_erased_proof[position],
                            current_rank,
                            inherited_lengths: &inherited_lengths,
                        },
                        &mut self.block_ranks,
                        &mut self.rank_edges,
                        &mut edge_blocks,
                        &mut evaluation.blocks,
                        &mut next_block,
                        &mut next_value,
                        &mut next_edge,
                        &mut operations,
                    )?
                } else {
                    Terminator::Conditional {
                        condition: condition.ok_or(LoweringError::Unsupported(
                            "Unit graph conditional lost its guard",
                        ))?,
                        when_true,
                        when_false,
                    }
                }
            }
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
                // Every arm's true edge is staged exactly like a conditional
                // edge — a jump arm's successor custody edge, or the private
                // return block a `(expression)` arm minted above; each
                // decision namespace is the one left by that arm's guard
                // evaluation, or the one its planned short-circuit decision
                // established.
                let mut selected = Vec::with_capacity(arms.len());
                for (index, arm) in arms.iter().enumerate() {
                    let plan = match arm {
                        checked_trees::CheckedGuardedJumpArmPlan::Jump(plan) => plan,
                        checked_trees::CheckedGuardedJumpArmPlan::Return(_) => {
                            selected.push(guarded_arm_return_edges[index].take().ok_or(
                                LoweringError::Unsupported(
                                    "guarded jump arm lost its staged return edge",
                                ),
                            )?);
                            continue;
                        }
                    };
                    let planned = if index == 0 {
                        planned_guard.as_ref()
                    } else {
                        match &guarded_drafts[index - 1].4 {
                            ChainGuard::Decision(planned) => Some(planned),
                            ChainGuard::Value(_) => None,
                        }
                    };
                    selected.push(match planned {
                        Some((planned, namespace)) => {
                            successor(&plan.successor, &[], false, namespace, &planned.established)?
                        }
                        None => {
                            let namespace = if index == 0 {
                                guarded_first_namespace.as_slice()
                            } else {
                                guarded_drafts[index - 1].5.as_slice()
                            };
                            successor(&plan.successor, &[], false, namespace, &[])?
                        }
                    });
                }
                let mut selected = selected.into_iter();
                let first = selected.next().ok_or(LoweringError::Unsupported(
                    "guarded jump chain lost its first successor",
                ))?;
                let mut fallback_edge = Some(match fallback {
                    checked_trees::CheckedGuardedJumpFallbackPlan::Successor(fallback) => {
                        successor(fallback, &[], false, &values, &[])?
                    }
                    checked_trees::CheckedGuardedJumpFallbackPlan::Return(_) => guarded_return_edge
                        .take()
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph guarded return lost its staged edge",
                        ))?,
                });
                let draft_count = guarded_drafts.len();
                for (index, (id, parameters, structural_parameters, operations_draft, guard, _)) in
                    guarded_drafts.into_iter().enumerate()
                {
                    let when_false = if index + 1 == draft_count {
                        fallback_edge.take().ok_or(LoweringError::Unsupported(
                            "guarded jump chain lost its fallback edge",
                        ))?
                    } else {
                        decision_edge(
                            guarded_decisions[index + 1],
                            self.state_erased_proof[position].len(),
                            current_rank,
                            &mut self.rank_edges,
                            &mut next_edge,
                        )?
                    };
                    let when_true = selected.next().ok_or(LoweringError::Unsupported(
                        "guarded jump chain lost an ordered successor",
                    ))?;
                    let terminator = match guard {
                        ChainGuard::Value(condition) => Terminator::Conditional {
                            condition,
                            when_true,
                            when_false,
                        },
                        ChainGuard::Decision((planned, namespace)) => emit_short_circuit_decision(
                            identities,
                            checked,
                            &planned,
                            &namespace,
                            [when_true, when_false],
                            ShortCircuitFrame {
                                erased_scalar_formals: &self.state_erased[position],
                                erased_proof_formals: &self.state_erased_proof[position],
                                current_rank,
                                inherited_lengths: &inherited_lengths,
                            },
                            &mut self.block_ranks,
                            &mut self.rank_edges,
                            &mut edge_blocks,
                            &mut evaluation.blocks,
                            &mut next_block,
                            &mut next_value,
                            &mut next_edge,
                            &mut operations,
                        )?,
                    };
                    if let Some(rank) = current_rank {
                        self.block_ranks.insert(id, rank);
                    }
                    edge_blocks.push(Block {
                        id,
                        parameters,
                        erased_scalar_formals: self.state_erased[position].clone(),
                        erased_proof_formals:
                            crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                                identities,
                                self.checked,
                                &self.state_erased_proof[position],
                            ),
                        structural_parameters,
                        operations: operations_draft,
                        terminator,
                    });
                }
                let first_false = decision_edge(
                    guarded_decisions[0],
                    self.state_erased_proof[position].len(),
                    current_rank,
                    &mut self.rank_edges,
                    &mut next_edge,
                )?;
                match &planned_guard {
                    Some((planned, namespace)) => emit_short_circuit_decision(
                        identities,
                        self.checked,
                        planned,
                        namespace,
                        [first, first_false],
                        ShortCircuitFrame {
                            erased_scalar_formals: &self.state_erased[position],
                            erased_proof_formals: &self.state_erased_proof[position],
                            current_rank,
                            inherited_lengths: &inherited_lengths,
                        },
                        &mut self.block_ranks,
                        &mut self.rank_edges,
                        &mut edge_blocks,
                        &mut evaluation.blocks,
                        &mut next_block,
                        &mut next_value,
                        &mut next_edge,
                        &mut operations,
                    )?,
                    None => Terminator::Conditional {
                        condition: condition.ok_or(LoweringError::Unsupported(
                            "guarded jump chain lost its first guard",
                        ))?,
                        when_true: first,
                        when_false: first_false,
                    },
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. } => {
                let prepared = prepared_cases.as_ref().ok_or(LoweringError::Unsupported(
                    "Unit graph case terminator lost its prepared payloads",
                ))?;
                let cases = prepared
                    .cases
                    .iter()
                    .zip(closed_return_edges)
                    .map(|(case, return_edge)| {
                        let edge = match &case.outcome {
                            checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(
                                successor_plan,
                            ) => {
                                // The edge binds this case's payloads as its
                                // staged block's formals; an argument
                                // expression reads one back through its tail
                                // position in the edge's namespace, which
                                // `successor_edge` appends in this same
                                // payload order.
                                let established = case
                                    .values
                                    .iter()
                                    .zip(&case.fields)
                                    .enumerate()
                                    .map(|(ordinal, ((_, _), field))| {
                                        crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload {
                                            source: prepared.source,
                                            path: prepared.path.clone(),
                                            case: case.identity,
                                            field: *field,
                                            position: values.len() + ordinal,
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                successor(
                                    successor_plan,
                                    &case.values,
                                    true,
                                    &values,
                                    &established,
                                )?
                            }
                            checked_trees::CheckedClosedSumCaseOutcomePlan::Return(_) => {
                                return_edge.ok_or(LoweringError::Unsupported(
                                    "Unit graph returning case lost its edge",
                                ))?
                            }
                        };
                        Ok(StructuralCaseSuccessorEdge {
                            edge: edge.edge,
                            target: edge.target,
                            case: case.identity,
                            payload_fields: case.fields.clone(),
                            // Selection binds scalar payload snapshots, but
                            // arm operands may still observe nested storage or
                            // transfer its custody. Both returns and successors
                            // evaluate those operands before their selected
                            // completion applies the checked cleanup schedule.
                            cleanup_actions: Vec::new(),
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                Terminator::StructuralCase {
                    source: prepared.source,
                    path: prepared.path.clone(),
                    cases,
                }
            }
        };
        let dispatches = condition.is_some() || branch_guard.is_some() || prepared_cases.is_some();
        self.seal_state(
            identities,
            position,
            terminator,
            edge_blocks,
            evaluation,
            operations,
            super::seal::SealFrame {
                state_parameters: &state_parameters,
                current_rank,
                is_guarded_return,
                dispatches,
                body_end,
                next_value,
                next_block,
                next_edge,
            },
        )
    }
}

/// Whether a successor evaluates any argument from an expression, which may
/// read a case payload its guard selected. Parameter forwards read none.
pub(super) fn successor_may_read_payloads(
    successor: &checked_trees::CheckedStructuralControlSuccessorPlan,
) -> bool {
    successor.scalar_arguments.iter().any(|argument| {
        argument.source == checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
    })
}

pub(super) fn guarded_arm_may_read_payloads(
    arm: &checked_trees::CheckedGuardedJumpArmPlan,
) -> bool {
    match arm {
        checked_trees::CheckedGuardedJumpArmPlan::Jump(plan) => {
            successor_may_read_payloads(&plan.successor)
        }
        checked_trees::CheckedGuardedJumpArmPlan::Return(plan) => {
            return_may_read_payloads(&plan.return_arm)
        }
    }
}

fn return_may_read_payloads(arm: &checked_trees::CheckedConditionalReturnArm) -> bool {
    !matches!(
        arm,
        checked_trees::CheckedConditionalReturnArm::Unit { .. }
            | checked_trees::CheckedConditionalReturnArm::Crash { .. }
    )
}

pub(super) fn plan_short_circuit_guard(
    expression: &LoweredBooleanReturnExpression,
    values: &[ValueDeclaration],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    structural_parameters: &[(u32, terminal_psi::StructuralParameterDeclaration)],
    next_value: &mut u64,
    successor_reads_payloads: bool,
) -> Result<
    (
        crate::emission::case_payload_dispatch::PlannedGuard,
        Vec<ValueDeclaration>,
    ),
    LoweringError,
> {
    let decision = crate::emission::boolean_control::lower_boolean_control_decision(
        expression,
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: true }),
        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant { value: false }),
    );
    let mut namespace = values.to_vec();
    let declared_cases = |place| {
        let (_, parameter) = structural_parameters
            .iter()
            .find(|(_, parameter)| parameter.place == place)?;
        match &terminal_psi::find_structural_type(structural_types, parameter.structural_type)?
            .shape
        {
            terminal_psi::StructuralTypeShape::Sum { cases }
            | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => Some(cases.as_slice()),
            _ => None,
        }
    };
    let planned = crate::emission::case_payload_dispatch::plan(
        decision,
        &declared_cases,
        &mut namespace,
        next_value,
        successor_reads_payloads,
    )?;
    Ok((planned, namespace))
}

/// How one chain arm observes its guard: an evaluated Boolean value, or a
/// short-circuit decision planned with the namespace its true edge reads.
pub(super) enum ChainGuard {
    Value(semantic_vocabulary::ValueId),
    Decision(
        (
            crate::emission::case_payload_dispatch::PlannedGuard,
            Vec<ValueDeclaration>,
        ),
    ),
}

/// The state-level facts a short-circuit decision's outcome blocks repeat.
struct ShortCircuitFrame<'a> {
    erased_scalar_formals: &'a [ValueDeclaration],
    erased_proof_formals: &'a [checked_trees::CheckedErasedProofParameterPlan],
    current_rank: Option<semantic_vocabulary::ValueId>,
    /// The length observations the state held before any successor
    /// staging; the decision cannot reuse observations staged after it.
    inherited_lengths: &'a CollectionLengthObservations,
}

/// Lower one planned short-circuit guard whose outcomes take `when_true` and
/// `when_false`, and return the jump into its decision. The shared decision
/// emitter carries scalar arguments only, so each outcome is a private block
/// that keeps the original successor edge, including structural transfers,
/// cleanup and ranking identity. A two-arm conditional and every arm of an
/// ordered guard chain share this one lowering.
#[allow(clippy::too_many_arguments)]
fn emit_short_circuit_decision(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    planned: &crate::emission::case_payload_dispatch::PlannedGuard,
    guard_values: &[ValueDeclaration],
    [when_true, when_false]: [SuccessorEdge; 2],
    frame: ShortCircuitFrame<'_>,
    block_ranks: &mut std::collections::BTreeMap<
        semantic_vocabulary::BlockId,
        semantic_vocabulary::ValueId,
    >,
    rank_edges: &mut std::collections::BTreeMap<
        semantic_vocabulary::EdgeId,
        (
            semantic_vocabulary::ValueId,
            terminal_psi::TerminalNaturalRankComparison,
        ),
    >,
    edge_blocks: &mut Vec<Block>,
    decision_blocks: &mut Vec<Block>,
    next_block: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Terminator, LoweringError> {
    // Successor staging runs only after selection. Its length observations
    // cannot be reused while evaluating the guard.
    operations
        .length_observations
        .clone_from(frame.inherited_lengths);
    let true_block = block_id(allocate_dense(next_block)?);
    let false_block = block_id(allocate_dense(next_block)?);
    for (id, successor) in [(true_block, when_true), (false_block, when_false)] {
        if let Some(rank) = frame.current_rank {
            block_ranks.insert(id, rank);
        }
        edge_blocks.push(Block {
            id,
            parameters: Vec::new(),
            erased_scalar_formals: frame.erased_scalar_formals.to_vec(),
            erased_proof_formals:
                crate::scalar_lowering::contracts::erased_proof_formal_declarations(
                    identities,
                    checked,
                    frame.erased_proof_formals,
                ),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: successor.edge,
                target: successor.target,
                arguments: successor.arguments,
                erased_arguments: successor.erased_arguments,
                erased_proof_arguments: successor.erased_proof_arguments.clone(),
                structural_arguments: successor.structural_arguments,
                cleanup_actions: successor.cleanup_actions,
            },
        });
    }
    let decision = &planned.decision;
    let tests = crate::emission::boolean_control::boolean_guard_decision_block_count(decision);
    let decision_block = block_id(*next_block);
    *next_block =
        next_block
            .checked_add(u64::try_from(tests).map_err(|_| {
                LoweringError::Unsupported("guard decision count exceeds identities")
            })?)
            .ok_or(LoweringError::Unsupported(
                "guard decision identities overflow",
            ))?;
    let (root, nested) = crate::emission::boolean_control::emit_inlined_boolean_guard_blocks(
        decision,
        guard_values,
        Vec::new(),
        &crate::emission::boolean_control::LoweredBooleanDecisionTarget {
            block: true_block,
            arguments: Vec::new(),
        },
        &crate::emission::boolean_control::LoweredBooleanDecisionTarget {
            block: false_block,
            arguments: Vec::new(),
        },
        decision_block,
        block_id(
            decision_block
                .get()
                .checked_add(1)
                .ok_or(LoweringError::Unsupported(
                    "guard decision identities overflow",
                ))?,
        ),
        next_value,
        next_edge,
        operations,
    );
    decision_blocks.push(root);
    decision_blocks.extend(nested);
    let edge = edge_id(allocate_dense(next_edge)?);
    if let Some(rank) = frame.current_rank {
        rank_edges.insert(
            edge,
            (
                rank,
                terminal_psi::TerminalNaturalRankComparison::Preserving,
            ),
        );
    }
    Ok(Terminator::Jump {
        edge,
        target: decision_block,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    })
}

/// The edge a failed chain guard takes to the next staged decision. It carries
/// no scalar operands and forwards the erased-proof roster; the next decision
/// stays inside this authored state, so it preserves the incoming rank.
fn decision_edge(
    target: semantic_vocabulary::BlockId,
    erased_proof_count: usize,
    current_rank: Option<semantic_vocabulary::ValueId>,
    rank_edges: &mut std::collections::BTreeMap<
        semantic_vocabulary::EdgeId,
        (
            semantic_vocabulary::ValueId,
            terminal_psi::TerminalNaturalRankComparison,
        ),
    >,
    next_edge: &mut u64,
) -> Result<SuccessorEdge, LoweringError> {
    let edge = edge_id(allocate_dense(next_edge)?);
    if let Some(rank) = current_rank {
        rank_edges.insert(
            edge,
            (
                rank,
                terminal_psi::TerminalNaturalRankComparison::Preserving,
            ),
        );
    }
    Ok(SuccessorEdge {
        edge,
        target,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: (0..erased_proof_count)
            .map(|position| semantic_vocabulary::ProofTerm::Formal {
                position: u32::try_from(position).expect("erased-proof roster positions fit u32"),
            })
            .collect(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    })
}
