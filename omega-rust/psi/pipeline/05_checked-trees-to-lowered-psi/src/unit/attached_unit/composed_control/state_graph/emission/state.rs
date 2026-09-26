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
    Block, CheckedScalarExpression, CheckedUnitEffectOperationPlan, Terminator, ValueDeclaration,
    allocate_dense, edge_id, lookup_claim_id, terminal_scalar_type, unsupported,
};
use super::super::super::LoweringError;
use super::super::{
    CheckedStructuralControlSuccessorPlan, case_emission, edges, ranking, result_custody, returns,
    scalars,
};
use super::StateGraphEmission;
use crate::emission::boolean_control::LoweredBooleanDecision;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload;
use crate::proofs::crash_routes::{lower_checked_crash_exit, lower_checked_crash_predicates};

impl StateGraphEmission<'_, '_> {
    /// Emit the state at `position` in authored order.
    pub(super) fn emit_state(&mut self, position: usize) -> Result<(), LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let state = &plan.states[position];
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
        let mut operations = OperationBuffer::new(self.catalogs.next_operation - 1);
        let mut evaluation = crate::unit::attached_unit::argument_evaluation::Evaluation {
            structural_value_owners: Vec::new(),
            selection_cleanups: Vec::new(),
            structural_locals: Vec::new(),
            view_locals: Vec::new(),
            element_views: std::collections::BTreeMap::new(),
            local_cases: Vec::new(),
            record_fields: crate::scalar_graph::scalar_computations::fields::prepare(
                checked,
                plan.machine,
                &self.catalogs.structural_types,
            )?,
            arrays: crate::scalar_graph::scalar_computations::arrays::prepare(
                checked,
                plan.machine,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            cases: crate::scalar_graph::scalar_computations::cases::prepare(
                checked,
                plan.machine,
                &self.catalogs.structural_types,
                &mut self.catalogs.next_place,
            )?,
            primitive_storage: Vec::new(),
            scalar_bindings: None,
            structural_fields: Vec::new(),
            structural_cases: Vec::new(),
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
        let mut values = self.state_values[position].clone();
        let mut next_value = self.catalogs.next_value;
        let mut next_block = self.catalogs.next_block;
        let mut next_edge = self.catalogs.next_edge;
        // The state's own rank, evaluated once on entry; `rank_ceiling` is the
        // constant its successor edges recompute a distance rank from.
        let mut rank_ceiling = None;
        let current_rank = if let Some(rank) = ranking::scalar_rank(plan, state) {
            let lane = values.iter().map(|value| value.id).collect::<Vec<_>>();
            let evaluated =
                ranking::emit_scalar_rank(rank, &lane, None, &mut next_value, &mut operations)?;
            rank_ceiling = evaluated.ceiling;
            Some(evaluated.rank)
        } else {
            ranking::parameter_position(plan, state).map(|parameter_position| {
                crate::emission::operation_emission::emit_byte_length(
                    state_parameters[parameter_position].place,
                    &mut next_value,
                    &mut operations,
                )
            })
        };
        let bindings = scalars::emit_prefix(
            checked,
            state,
            &evaluation.structural_parameters,
            &self.catalogs.structural_types,
            &mut values,
            &mut next_value,
            &mut operations,
        )?;
        evaluation.scalar_bindings = Some(bindings.clone());
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
        super::super::super::emission::emit_call_operations(
            checked,
            plan.machine,
            state,
            &state.operations,
            self.catalogs,
            &state_parameters,
            &self.claims.source_claims,
            &mut evaluation,
            &mut values,
            &self.state_erased[position],
            &mut next_value,
            &mut next_block,
            &mut next_edge,
            &mut operations,
        )?;
        let bindings = evaluation
            .scalar_bindings
            .clone()
            .ok_or(LoweringError::Unsupported(
                "graph body lost its scalar namespace",
            ))?;
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
            } => match (return_when_true, return_arm) {
                (false, _) => Some(jump.statement_ordinal),
                (
                    true,
                    typed_trees_to_checked_trees::checked_trees::CheckedConditionalReturnArm::Structural(
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. },
                    ),
                ) => Some(result.statement_index),
                (
                    true,
                    typed_trees_to_checked_trees::checked_trees::CheckedConditionalReturnArm::Scalar {
                        statement_ordinal, ..
                    },
                ) => Some(*statement_ordinal),
                (true, typed_trees_to_checked_trees::checked_trees::CheckedConditionalReturnArm::Structural(_)) => None,
            },
            _ => None,
        };
        // A named true successor with expression arguments may read the case
        // payloads the guard's own case tests select.
        let first_successor_reads_payloads = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } => {
                successor_may_read_payloads(when_true)
            }
            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                jump,
                return_when_true: false,
                ..
            } => successor_may_read_payloads(jump),
            CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } => arms
                .first()
                .is_some_and(|arm| successor_may_read_payloads(&arm.successor)),
            _ => false,
        };
        let condition = if let Some(true_ordinal) = conditional_true_ordinal {
            branch_guard = evaluation.branch_guard(
                checked,
                plan.machine,
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
                    checked,
                    plan.machine,
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
                checked,
                plan.machine,
                state.state,
                first.successor.statement_ordinal,
                &values,
                first_successor_reads_payloads,
            )?;
            if branch_guard.is_some() {
                None
            } else {
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let condition = evaluation.guard_value(
                    checked,
                    plan.machine,
                    state.state,
                    first.successor.statement_ordinal,
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
            checked,
            state,
            self.catalogs,
            &state_parameters,
            &operations,
            &mut next_value,
        )?;
        let returned_case = returns::emit(
            checked,
            state,
            &self.machine_result,
            &bindings,
            self.catalogs,
            &values,
            &mut next_value,
            &mut operations,
        )?;
        let guarded_return = super::super::guarded::emit(
            checked,
            plan,
            state,
            self.catalogs,
            &state_parameters,
            &self.claims.source_claims,
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
        let scalar_return = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion:
                    typed_trees_to_checked_trees::checked_trees::CheckedScalarReturnPlan::Exits(exits),
            } => {
                let mut calls = self.catalogs.scalar_calls.emission_context();
                let value = evaluation.scalar_control_result(
                    checked,
                    plan.machine,
                    state.state,
                    exits,
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
                completion:
                    typed_trees_to_checked_trees::checked_trees::CheckedScalarReturnPlan::Binding(
                        binding,
                    ),
            } => {
                let slot = usize::try_from(binding.binding_ordinal)
                    .ok()
                    .and_then(|ordinal| ordinal.checked_add(state.scalar_parameters.len()))
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph scalar return binding ordinal overflows",
                    ))?;
                let position = match &evaluation.scalar_bindings {
                    Some(bindings) => bindings.immutable_position(slot)?,
                    None => slot,
                };
                let value = values
                    .get(position)
                    .copied()
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph scalar return binding is absent",
                    ))?;
                if value.scalar_type != terminal_scalar_type(binding.primitive_type)? {
                    return unsupported("Unit graph scalar return binding changed its carrier");
                }
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
        let inherited_lengths = operations.byte_lengths.clone();
        let inherited_field_lengths = operations.field_byte_lengths.clone();
        // Ordered multi-arm guards: every later guard is observed inside its
        // own private block reached only along the previous decision's false
        // edge. Their evaluation drafts are staged before the successor-edge
        // closure exists; the terminator match below assembles the chain.
        let (guarded_decisions, guarded_drafts, guarded_first_namespace) =
            if let CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, .. } =
                &state.terminator
            {
                self.stage_guarded_chain(
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
                    position,
                    &state_parameters,
                    arm_namespace,
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
        // A case test on a whole owned parameter consumes it, as the checked
        // cleanup records by leaving it out of the arm's discards: each tested
        // arm's edge and the dispatch's closing `_` edge dispose of it where
        // control leaves the state. A closed-sum dispatch disposes of it on its
        // own case edges instead.
        let consumed_case_subjects = case_subject_consumptions(&state.terminator);
        let mut edge_blocks = Vec::new();
        let frame = super::successor_edge::SuccessorFrame {
            position,
            state_parameters: &state_parameters,
            bindings: &bindings,
            current_rank,
            rank_ceiling,
            case_subject: prepared_cases.as_ref().map(|prepared| prepared.source),
            guarded: condition.is_some() || branch_guard.is_some(),
            inherited_lengths: &inherited_lengths,
            inherited_field_lengths: &inherited_field_lengths,
            consumed_case_subjects: &consumed_case_subjects,
        };
        let mut successor = |edge: &CheckedStructuralControlSuccessorPlan,
                             payload_values: &[(u32, ValueDeclaration)],
                             case_edge: bool,
                             values: &[ValueDeclaration],
                             established: &[EstablishedCasePayload]|
         -> Result<SuccessorEdge, LoweringError> {
            self.successor_edge(
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
                let returned_claims = match result.source {
                    typed_trees_to_checked_trees::checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?
                        .claims
                        .iter()
                        .map(|binding| binding.claim)
                        .collect::<Vec<_>>(),
                    typed_trees_to_checked_trees::checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => state
                        .entry_claims
                        .iter()
                        .filter(|claim| claim.parameter_index == parameter_index)
                        .map(|claim| {
                            lookup_claim_id(&self.claims.source_claims, claim.claim_identity)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return unsupported("structural return has no whole claim source"),
                };
                if self.content_entry_claims.iter().any(|entry| {
                    returned_claims.contains(&entry.claim)
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
                    typed_trees_to_checked_trees::checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?.place,
                    typed_trees_to_checked_trees::checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
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
                    trivial_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. } => {
                Terminator::ReturnStructural {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    source: returned_case.ok_or(LoweringError::Unsupported(
                        "case return construction missing",
                    ))?,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                let local_discards = return_root_discards(
                    checked,
                    plan,
                    state,
                    &operations,
                    &evaluation,
                    &state_parameters,
                )?;
                let edge = edge_id(allocate_dense(&mut next_edge)?);
                // A scalar return disposes the same whole roots a Unit return
                // does, after the returned value is established.
                match (&state.terminator, scalar_return) {
                    (
                        CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. },
                        Some(value),
                    ) => Terminator::Return {
                        edge,
                        value: value.id,
                        cleanup_actions: local_discards
                            .into_iter()
                            .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
                            .collect(),
                    },
                    (CheckedComposedUnitControlTerminatorPlan::ReturnUnit, None) => {
                        Terminator::ReturnUnit {
                            edge,
                            trivial_affine_discards: local_discards,
                        }
                    }
                    _ => return unsupported("Unit graph return lost its scalar value"),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Crash { statement_ordinal } => {
                let crash = lower_checked_crash_exit(
                    checked,
                    plan.machine,
                    state.state,
                    *statement_ordinal,
                    &self.claims.source_claims,
                )?;
                Terminator::Crash {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    cause: crash.cause,
                    site_guard: lower_checked_crash_predicates(
                        &crash.site_guard,
                        &self.state_values[position],
                    )?,
                    frontier_lower_bound: crash.frontier_lower_bound,
                }
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
                    trivial_affine_discards: edge.trivial_affine_discards,
                    residual_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
            | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. } => {
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
                    _ => unreachable!(),
                };
                if let Some((planned, guard_values)) = &planned_guard {
                    emit_short_circuit_decision(
                        planned,
                        guard_values,
                        [when_true, when_false],
                        ShortCircuitFrame {
                            erased_scalar_formals: &self.state_erased[position],
                            erased_proof_formals: &self.state_erased_proof[position],
                            current_rank,
                            inherited_lengths: (&inherited_lengths, &inherited_field_lengths),
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
                // Every arm's successor edge is staged exactly like a
                // conditional edge; each decision namespace is the one left
                // by that arm's guard evaluation, or the one its planned
                // short-circuit decision established.
                let mut selected = Vec::with_capacity(arms.len());
                for (index, arm) in arms.iter().enumerate() {
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
                            successor(&arm.successor, &[], false, namespace, &planned.established)?
                        }
                        None => {
                            let namespace = if index == 0 {
                                guarded_first_namespace.as_slice()
                            } else {
                                guarded_drafts[index - 1].5.as_slice()
                            };
                            successor(&arm.successor, &[], false, namespace, &[])?
                        }
                    });
                }
                let mut selected = selected.into_iter();
                let first = selected.next().ok_or(LoweringError::Unsupported(
                    "guarded jump chain lost its first successor",
                ))?;
                let mut fallback_edge = Some(successor(fallback, &[], false, &values, &[])?);
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
                            &planned,
                            &namespace,
                            [when_true, when_false],
                            ShortCircuitFrame {
                                erased_scalar_formals: &self.state_erased[position],
                                erased_proof_formals: &self.state_erased_proof[position],
                                current_rank,
                                inherited_lengths: (&inherited_lengths, &inherited_field_lengths),
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
                            crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
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
                        planned,
                        namespace,
                        [first, first_false],
                        ShortCircuitFrame {
                            erased_scalar_formals: &self.state_erased[position],
                            erased_proof_formals: &self.state_erased_proof[position],
                            current_rank,
                            inherited_lengths: (&inherited_lengths, &inherited_field_lengths),
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
                    .map(|case| {
                        // The edge binds this case's payloads as its staged
                        // block's formals; an argument expression reads one
                        // back through its tail position in the edge's
                        // namespace, which `successor_edge` appends in this
                        // same payload order.
                        let established = case
                            .values
                            .iter()
                            .zip(&case.fields)
                            .enumerate()
                            .map(|(ordinal, ((_, _), field))| {
                                crate::expression_preparation::bindings::structural_fields::EstablishedCasePayload {
                                    source: prepared.source,
                                    case: case.identity,
                                    field: *field,
                                    position: values.len() + ordinal,
                                }
                            })
                            .collect::<Vec<_>>();
                        let edge = successor(
                            case.successor,
                            &case.values,
                            true,
                            &values,
                            &established,
                        )?;
                        Ok(StructuralCaseSuccessorEdge {
                            edge: edge.edge,
                            target: edge.target,
                            case: case.identity,
                            payload_fields: case.fields.clone(),
                            trivial_affine_discards: vec![prepared.source],
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                Terminator::StructuralCase {
                    source: prepared.source,
                    cases,
                }
            }
        };
        let dispatches = condition.is_some() || branch_guard.is_some() || prepared_cases.is_some();
        self.seal_state(
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

/// Plan one short-circuit guard as a Boolean decision whose case dispatches
/// allocate payload values. Returns the plan and the scalar namespace its true
/// outcome reads, which includes the payloads its dispatches established.
/// The whole roots a returning exit of this state disposes: its dying local
/// results, then the parameters its exit drops.
pub(super) fn return_root_discards(
    checked: &typed_trees_to_checked_trees::checked_trees::CheckedTrees,
    plan: &typed_trees_to_checked_trees::checked_trees::CheckedComposedUnitControlMachinePlan,
    state: &typed_trees_to_checked_trees::checked_trees::CheckedComposedUnitControlStatePlan,
    operations: &OperationBuffer,
    evaluation: &crate::unit::attached_unit::argument_evaluation::Evaluation,
    state_parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> Result<Vec<semantic_vocabulary::PlaceId>, LoweringError> {
    let source = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .and_then(|machine| {
            checked
                .machine_states(machine)
                .iter()
                .find(|source| source.symbol == state.state)
        })
        .ok_or(LoweringError::Unsupported(
            "Unit return source state missing",
        ))?;
    let discards = edges::return_discards(checked, plan.machine, source, state)?;
    let mut local_discards =
        result_custody::local_discards(checked, plan.machine, source, state, None)?
            .into_iter()
            .map(|ordinal| {
                let result = case_emission::result(state, ordinal, operations)?;
                Ok(evaluation.current_structural_place(result.place))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
    local_discards.extend(
        discards
            .into_iter()
            .map(|index| state_parameters[index].place),
    );
    Ok(local_discards)
}

/// The owned parameter each edge of a case-test dispatch consumes, keyed by
/// the edge's transition ordinal: a tested arm consumes its own subject, and
/// the dispatch's closing `_` arm the subject of the arm before it.
fn case_subject_consumptions(
    terminator: &typed_trees_to_checked_trees::checked_trees::CheckedComposedUnitControlTerminatorPlan,
) -> Vec<(u32, u32)> {
    fn subject(
        guard: &typed_trees_to_checked_trees::checked_trees::CheckedCallScalarArgument,
    ) -> Option<u32> {
        let CheckedScalarExpression::Boolean(guard) = guard.as_pure()? else {
            return None;
        };
        let membership = match guard.as_ref() {
            typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression::Equal {
                left,
                right,
            } => match (left.as_ref(), right.as_ref()) {
                (
                    typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression::Constant(
                        true,
                    ),
                    tested,
                )
                | (
                    tested,
                    typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression::Constant(
                        true,
                    ),
                ) => tested,
                _ => return None,
            },
            tested => tested,
        };
        match membership {
            typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression::StructuralCaseMembership {
                subject, ..
            } if subject.path.is_empty() => Some(subject.parameter_position),
            _ => None,
        }
    }
    match terminator {
        typed_trees_to_checked_trees::checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
            guard,
            when_true,
            when_false,
        } => subject(guard)
            .map(|position| {
                vec![
                    (when_true.statement_ordinal, position),
                    (when_false.statement_ordinal, position),
                ]
            })
            .unwrap_or_default(),
        typed_trees_to_checked_trees::checked_trees::CheckedComposedUnitControlTerminatorPlan::GuardedJumps {
            arms,
            fallback,
        } => {
            let mut consumed = arms
                .iter()
                .filter_map(|arm| {
                    subject(&arm.guard).map(|position| (arm.successor.statement_ordinal, position))
                })
                .collect::<Vec<_>>();
            if let Some(position) = arms.last().and_then(|arm| subject(&arm.guard)) {
                consumed.push((fallback.statement_ordinal, position));
            }
            consumed
        }
        _ => Vec::new(),
    }
}

/// Whether a successor evaluates any argument from an expression, which may
/// read a case payload its guard selected. Parameter forwards read none.
pub(super) fn successor_may_read_payloads(
    successor: &typed_trees_to_checked_trees::checked_trees::CheckedStructuralControlSuccessorPlan,
) -> bool {
    successor.scalar_arguments.iter().any(|argument| {
        argument.source == typed_trees_to_checked_trees::checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
    })
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
        match &structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)?
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
    erased_proof_formals:
        &'a [typed_trees_to_checked_trees::checked_trees::CheckedErasedProofParameterPlan],
    current_rank: Option<semantic_vocabulary::ValueId>,
    /// The byte-length observations the state held before any successor
    /// staging; the decision cannot reuse observations staged after it.
    inherited_lengths: (
        &'a [(semantic_vocabulary::PlaceId, semantic_vocabulary::ValueId)],
        &'a [(
            semantic_vocabulary::PlaceId,
            Vec<terminal_psi::StructuralPathSegment>,
            semantic_vocabulary::StructuralFieldId,
            semantic_vocabulary::ValueId,
        )],
    ),
}

/// Lower one planned short-circuit guard whose outcomes take `when_true` and
/// `when_false`, and return the jump into its decision. The shared decision
/// emitter carries scalar arguments only, so each outcome is a private block
/// that keeps the original successor edge, including structural transfers,
/// cleanup and ranking identity. A two-arm conditional and every arm of an
/// ordered guard chain share this one lowering.
#[allow(clippy::too_many_arguments)]
fn emit_short_circuit_decision(
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
    operations.byte_lengths = frame.inherited_lengths.0.to_vec();
    operations.field_byte_lengths = frame.inherited_lengths.1.to_vec();
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
                crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
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
                trivial_affine_discards: successor.trivial_affine_discards,
                residual_affine_discards: Vec::new(),
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
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
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
        trivial_affine_discards: Vec::new(),
    })
}
