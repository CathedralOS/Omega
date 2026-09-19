//! One authored state of the composed-control graph: its scalar prefix and
//! call operations, the guard or case dispatch, the successor edges with
//! their staged transfers and ranks, and the blocks it leaves behind.

use super::super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralAccess, StructuralArgument,
    StructuralCaseSuccessorEdge, SuccessorEdge, block_id,
};
use super::super::super::super::{
    Block, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    Terminator, ValueDeclaration, allocate_dense, direct_expression_contains_short_circuit,
    edge_id, emit_direct_expression, lookup_claim_id, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types,
};
use super::super::super::LoweringError;
use super::super::{
    CheckedStructuralControlSuccessorPlan, case_emission, edges, ranking, result_custody, returns,
    scalars, subslices,
};
use super::StateGraphEmission;
use crate::emission::boolean_control::LoweredBooleanDecision;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;

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
        let current_rank =
            if let Some(scalar_position) = ranking::scalar_parameter_position(plan, state) {
                Some(values[scalar_position].id)
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
        let condition =
            if let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
                &state.terminator
            {
                branch_guard = evaluation.branch_guard(
                    checked,
                    plan.machine,
                    state.state,
                    when_true.statement_ordinal,
                    &values,
                )?;
                if branch_guard.is_some() {
                    None
                } else {
                    let mut calls = self.catalogs.scalar_calls.emission_context();
                    let condition = evaluation.guard_value(
                        checked,
                        plan.machine,
                        state.state,
                        when_true.statement_ordinal,
                        &mut values,
                        &mut next_value,
                        &mut next_block,
                        &mut next_edge,
                        &mut operations,
                        &mut calls,
                    )?;
                    self.catalogs.scalar_calls.next_call_obligation =
                        calls.next_obligation_identity;
                    Some(condition.id)
                }
            } else {
                None
            };
        let body_end = operations.len();
        let prepared_cases = case_emission::prepare(
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
        let inherited_lengths = operations.byte_lengths.clone();
        let mut edge_blocks = Vec::new();
        let mut successor = |edge: &CheckedStructuralControlSuccessorPlan,
                             payload_values: &[(u32, ValueDeclaration)],
                             case_edge: bool|
         -> Result<SuccessorEdge, LoweringError> {
            // Case dispatch consumes its subject separately. Ordinary edges
            // retain their exact local remainder until selected operands finish.
            let trivial_affine_discards = if case_edge {
                Vec::new()
            } else if evaluation.selection_cleanups.is_empty() {
                result_custody::local_discards(
                    checked,
                    plan.machine,
                    &self.admitted.source_states[position],
                    state,
                    Some(edge),
                )?
                .into_iter()
                .map(|ordinal| {
                    let result = case_emission::result(state, ordinal, &operations)?;
                    Ok(evaluation.current_structural_place(result.place))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
            } else {
                // An owned selection's residual and transported parameters die
                // or transfer per edge; the receipt's cleanup correspondence
                // substitutes each source's positional row exactly.
                result_custody::selection_edge_discards(
                    checked,
                    plan.machine,
                    &self.admitted.source_states[position],
                    state,
                    edge,
                    &operations,
                    &evaluation,
                )?
            };
            operations.byte_lengths = inherited_lengths.clone();
            let target = plan
                .states
                .iter()
                .position(|state| state.state == edge.target_state)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph target disappeared during emission",
                ))?;
            let stage = case_edge || (condition.is_some() || branch_guard.is_some())
                    && ((current_rank.is_some() && ranking::has_rank(plan, &plan.states[target])) || edge.transfers.iter().any(|transfer| matches!(
                        transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
                    )) || edge.scalar_arguments.iter().any(|argument| {
                        matches!(
                            argument.source,
                            checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
                        )
                    }));
            let operation_start = operations.len();
            let mut arguments = Vec::new();
            let mut structural_arguments = Vec::new();
            let target_state = &plan.states[target];
            for argument_position in
                0..target_state.structural_parameters.len() + target_state.scalar_parameters.len()
            {
                if let Some((target_index, (target_parameter, transfer))) = target_state
                    .structural_parameters
                    .iter()
                    .zip(&edge.transfers)
                    .enumerate()
                    .find(|(_, (parameter, _))| parameter.position as usize == argument_position)
                {
                    if target_parameter.is_self {
                        let checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                            index,
                        } = transfer.source
                        else {
                            return unsupported("Unit graph receiver cannot be rebound");
                        };
                        if state_parameters
                            .get(index as usize)
                            .map(|parameter| parameter.place)
                            != self
                                .parameters
                                .iter()
                                .find(|parameter| parameter.is_self)
                                .map(|parameter| parameter.place)
                        {
                            return unsupported(
                                "Unit graph receiver lost original invocation place",
                            );
                        }
                        continue;
                    }
                    if self
                        .admitted
                        .claim_transport
                        .aliased
                        .get(target)
                        .is_some_and(|aliased| aliased.contains_key(&(target_index as u32)))
                    {
                        // A claim carried across the edge keeps the entry
                        // parameter's place; the block does not rebind it, so
                        // the transfer row must still name that exact
                        // parameter and no edge argument is emitted.
                        let checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                            index,
                        } = transfer.source
                        else {
                            return unsupported(
                                "Unit graph claim successor is not a whole parameter transfer",
                            );
                        };
                        if state_parameters
                            .get(index as usize)
                            .map(|parameter| parameter.place)
                            != self
                                .state_views
                                .get(target)
                                .and_then(|views| views.get(target_index))
                                .map(|parameter| parameter.place)
                        {
                            return unsupported("Unit graph claim successor lost its entry place");
                        }
                        continue;
                    }
                    let place = match transfer.source {
                            checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => evaluation.current_structural_place(case_emission::result(state, binding_ordinal, &operations)?.place),
                            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                                evaluation.current_structural_place(state_parameters.get(index as usize).ok_or(
                                    LoweringError::Unsupported("Unit graph transfer source descriptor disappeared"),
                                )?.place)
                            }
                            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { parameter_index, expression } => {
                                let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                                let source = state_parameters.get(parameter_index as usize).ok_or(
                                    LoweringError::Unsupported("Unit graph subslice source descriptor disappeared"),
                                )?;
                                self.structural_places.push(subslices::emit(
                                    checked, state, edge.statement_ordinal, target_parameter.position, expression,
                                    source, destination, &bindings, &values, &mut next_value, &mut operations,
                                )?);
                                destination
                            }
                        };
                    structural_arguments.push(StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: match target_parameter.access {
                            checked_trees::CheckedStructuralAccess::Owned => {
                                StructuralAccess::Owned
                            }
                            checked_trees::CheckedStructuralAccess::MutableBorrow => {
                                StructuralAccess::MutableBorrow
                            }
                            _ => StructuralAccess::SharedBorrow,
                        },
                    });
                    continue;
                }
                if let Some((scalar_position, _)) = target_state
                    .scalar_parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.source_position as usize == argument_position)
                    && let Some((_, payload)) = payload_values
                        .iter()
                        .find(|(position, _)| *position as usize == scalar_position)
                {
                    arguments.push(payload.id);
                    continue;
                }
                let transfer = edge
                    .scalar_arguments
                    .iter()
                    .find(|transfer| transfer.argument_ordinal as usize == argument_position)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph successor argument position missing",
                    ))?;
                let expression = match transfer.source {
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                        index,
                    } => bindings.expression(&CheckedScalarExpression::Parameter {
                        position: index as usize,
                        primitive_type: transfer.primitive_type,
                    })?,
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                        bindings.expression_at(
                            checked,
                            state.state,
                            edge.statement_ordinal,
                            CheckedScalarExpressionRole::TransitionArgument {
                                argument_ordinal: transfer.argument_ordinal,
                            },
                        )?
                    }
                };
                if expression.scalar_type() != terminal_scalar_type(transfer.primitive_type)?
                    || direct_expression_contains_short_circuit(&expression)
                {
                    return unsupported("Unit graph successor needs a matching branch-free value");
                }
                validate_direct_parameter_types(
                    &expression,
                    &values
                        .iter()
                        .map(|value| value.scalar_type)
                        .collect::<Vec<_>>(),
                )?;
                arguments.push(emit_direct_expression(
                    &expression,
                    &values,
                    &mut next_value,
                    &mut operations,
                ));
            }
            // Proof-only actuals never evaluate: each checked erased row
            // lowers to a term over the emitting state's scalar values and
            // erased formals, in the target's erased-roster order.
            if edge.erased_arguments.len() != self.state_erased[target].len() {
                return unsupported("Unit graph successor erased arity drifted");
            }
            let erased_arguments = edge
                .erased_arguments
                .iter()
                .enumerate()
                .map(|(erased_index, argument)| {
                    if argument.target_scalar_parameter_index as usize != erased_index {
                        return unsupported("Unit graph successor erased order drifted");
                    }
                    let expression = bindings.expression_at(
                        checked,
                        state.state,
                        edge.statement_ordinal,
                        CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: argument.argument_ordinal,
                        },
                    )?;
                    crate::proofs::crash_routes::lowered_direct_scalar_term(
                        &expression,
                        &values,
                        &self.state_erased[position],
                    )
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let arriving_rank = if current_rank.is_some() {
                if let Some(position) = ranking::scalar_parameter_position(plan, target_state) {
                    arguments.get(position).copied()
                } else if let Some(dense) = ranking::parameter_position(plan, target_state) {
                    if self
                        .admitted
                        .claim_transport
                        .aliased
                        .get(target)
                        .is_some_and(|aliased| aliased.contains_key(&(dense as u32)))
                    {
                        // An aliased rank subject is not an edge argument;
                        // its byte length reads the retained entry place.
                        self.state_views
                            .get(target)
                            .and_then(|views| views.get(dense))
                            .map(|parameter| {
                                crate::emission::operation_emission::emit_byte_length(
                                    parameter.place,
                                    &mut next_value,
                                    &mut operations,
                                )
                            })
                    } else {
                        ranking::byte_argument_position(
                            plan,
                            target_state,
                            &self.admitted.claim_transport.aliased[target],
                        )
                        .map(|parameter_position| {
                            crate::emission::operation_emission::emit_byte_length(
                                structural_arguments[parameter_position].place,
                                &mut next_value,
                                &mut operations,
                            )
                        })
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let target = self.state_ids[target];
            if stage {
                let staged = block_id(allocate_dense(&mut next_block)?);
                let backedge = edge_id(allocate_dense(&mut next_edge)?);
                let selection_edge = edge_id(allocate_dense(&mut next_edge)?);
                self.arrival_edges
                    .entry(target)
                    .or_insert_with(Vec::new)
                    .push(backedge);
                if let Some(rank) = current_rank {
                    self.block_ranks.insert(staged, rank);
                    self.rank_edges.insert(
                        selection_edge,
                        (
                            rank,
                            terminal_psi::TerminalNaturalRankComparison::Preserving,
                        ),
                    );
                    if let Some(after) = arriving_rank {
                        self.rank_edges.insert(
                            backedge,
                            (after, terminal_psi::TerminalNaturalRankComparison::Strict),
                        );
                    }
                }
                edge_blocks.push(Block {
                    id: staged,
                    parameters: payload_values.iter().map(|(_, value)| *value).collect(),
                    // The forwarding block redeclares the emitting state's
                    // erased roster so forwarded proof terms stay in scope.
                    erased_scalar_formals: self.state_erased[position].clone(),
                    structural_parameters: Vec::new(),
                    operations: operations[operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge: backedge,
                        target,
                        arguments,
                        structural_arguments,
                        erased_arguments,
                        trivial_affine_discards,
                        residual_affine_discards: Vec::new(),
                    },
                });
                Ok(SuccessorEdge {
                    edge: selection_edge,
                    target: staged,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                })
            } else {
                let successor_edge = edge_id(allocate_dense(&mut next_edge)?);
                self.arrival_edges
                    .entry(target)
                    .or_insert_with(Vec::new)
                    .push(successor_edge);
                if let Some(after) = arriving_rank {
                    self.rank_edges.insert(
                        successor_edge,
                        (after, terminal_psi::TerminalNaturalRankComparison::Strict),
                    );
                }
                Ok(SuccessorEdge {
                    edge: successor_edge,
                    target,
                    arguments,
                    structural_arguments,
                    erased_arguments,
                    trivial_affine_discards,
                })
            }
        };
        let mut terminator = match &state.terminator {
            CheckedComposedUnitControlTerminatorPlan::Guarded { .. } => guarded_return.ok_or(
                LoweringError::Unsupported("guarded structural emission absent"),
            )?,
            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result } => {
                let returned_claims = match result.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?
                        .claims
                        .iter()
                        .map(|binding| binding.claim)
                        .collect::<Vec<_>>(),
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
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
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => case_emission::result(state, binding_ordinal, &operations)?.place,
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
            CheckedComposedUnitControlTerminatorPlan::ReturnUnit => {
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
                            let result = case_emission::result(state, ordinal, &operations)?;
                            Ok(evaluation.current_structural_place(result.place))
                        })
                        .collect::<Result<Vec<_>, LoweringError>>()?;
                local_discards.extend(
                    discards
                        .into_iter()
                        .map(|index| state_parameters[index].place),
                );
                Terminator::ReturnUnit {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    trivial_affine_discards: local_discards,
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Jump { successor: edge } => {
                let edge = successor(edge, &[], false)?;
                Terminator::Jump {
                    edge: edge.edge,
                    target: edge.target,
                    arguments: edge.arguments,
                    erased_arguments: edge.erased_arguments,
                    structural_arguments: edge.structural_arguments,
                    trivial_affine_discards: edge.trivial_affine_discards,
                    residual_affine_discards: Vec::new(),
                }
            }
            CheckedComposedUnitControlTerminatorPlan::Conditional {
                when_true,
                when_false,
                ..
            } => {
                let when_true = successor(when_true, &[], false)?;
                let when_false = successor(when_false, &[], false)?;
                if let Some(expression) = &branch_guard {
                    // Successor staging runs only after selection. Its length
                    // observations cannot be reused while evaluating the guard.
                    operations.byte_lengths = inherited_lengths.clone();
                    // The shared decision emitter carries scalar arguments only.
                    // These outcome blocks retain the original successor edges,
                    // including structural transfers, cleanup and ranking identity.
                    let true_block = block_id(allocate_dense(&mut next_block)?);
                    let false_block = block_id(allocate_dense(&mut next_block)?);
                    for (id, successor) in [(true_block, when_true), (false_block, when_false)] {
                        if let Some(rank) = current_rank {
                            self.block_ranks.insert(id, rank);
                        }
                        edge_blocks.push(Block {
                            id,
                            parameters: Vec::new(),
                            erased_scalar_formals: self.state_erased[position].clone(),
                            structural_parameters: Vec::new(),
                            operations: Vec::new(),
                            terminator: Terminator::Jump {
                                edge: successor.edge,
                                target: successor.target,
                                arguments: successor.arguments,
                                erased_arguments: successor.erased_arguments,
                                structural_arguments: successor.structural_arguments,
                                trivial_affine_discards: successor.trivial_affine_discards,
                                residual_affine_discards: Vec::new(),
                            },
                        });
                    }
                    let decision = crate::emission::boolean_control::lower_boolean_control_decision(
                        expression,
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                            value: true,
                        }),
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                            value: false,
                        }),
                    );
                    let tests =
                        crate::emission::boolean_control::boolean_decision_test_count(&decision);
                    let decision_block = block_id(next_block);
                    next_block = next_block
                        .checked_add(u64::try_from(tests).map_err(|_| {
                            LoweringError::Unsupported("guard decision count exceeds identities")
                        })?)
                        .ok_or(LoweringError::Unsupported(
                            "guard decision identities overflow",
                        ))?;
                    let (root, nested) =
                        crate::emission::boolean_control::emit_inlined_boolean_guard_blocks(
                            &decision,
                            &values,
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
                            block_id(decision_block.get().checked_add(1).ok_or(
                                LoweringError::Unsupported("guard decision identities overflow"),
                            )?),
                            &mut next_value,
                            &mut next_edge,
                            &mut operations,
                        );
                    evaluation.blocks.push(root);
                    evaluation.blocks.extend(nested);
                    let edge = edge_id(allocate_dense(&mut next_edge)?);
                    if let Some(rank) = current_rank {
                        self.rank_edges.insert(
                            edge,
                            (
                                rank,
                                terminal_psi::TerminalNaturalRankComparison::Preserving,
                            ),
                        );
                    }
                    Terminator::Jump {
                        edge,
                        target: decision_block,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    }
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
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. } => {
                let prepared = prepared_cases.as_ref().ok_or(LoweringError::Unsupported(
                    "Unit graph case terminator lost its prepared payloads",
                ))?;
                let cases = prepared
                    .cases
                    .iter()
                    .map(|case| {
                        let edge = successor(case.successor, &case.values, true)?;
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
        // Producing a value does not dispose of the remaining entry owners.
        // Reuse the source exit-custody join for structural and Unit returns;
        // the returned owner itself is not a discard.
        for completion in std::iter::once(&mut terminator)
            .chain(edge_blocks.iter_mut().map(|block| &mut block.terminator))
            .filter(|_| !is_guarded_return)
        {
            if let Terminator::ReturnStructural {
                trivial_affine_discards,
                ..
            } = completion
            {
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
                        "structural return source state missing",
                    ))?;
                *trivial_affine_discards =
                    edges::return_discards(checked, plan.machine, source, state)?
                        .into_iter()
                        .map(|index| {
                            evaluation.current_structural_place(state_parameters[index].place)
                        })
                        .collect();
            }
        }
        if !is_guarded_return && !evaluation.selection_cleanups.is_empty() {
            match &mut terminator {
                Terminator::ReturnStructural {
                    trivial_affine_discards,
                    ..
                }
                | Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => {
                    let discards = trivial_affine_discards;
                    let mut local_discards = Vec::new();
                    for operation in state.operations.iter().rev() {
                        if let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            result,
                            discard_result_on_return,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::StructuralCall {
                            result,
                            discard_result_on_return,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            result,
                            discard_result_on_return,
                            ..
                        } = operation
                        {
                            local_discards.push((
                                case_emission::result(state, result.binding_ordinal, &operations)?
                                    .place,
                                *discard_result_on_return,
                            ));
                        }
                    }
                    // Parameter selection sources keep the same roster shape
                    // as the guarded edge: they follow the result rows in
                    // descending authored position so the splice substitutes
                    // each residual join parameter at its own slot.
                    let mut parameter_sources = Vec::new();
                    for cleanup in &evaluation.selection_cleanups {
                        for source in &cleanup.sources {
                            if let Some((position, _)) = evaluation
                                .structural_parameters
                                .iter()
                                .find(|(_, declaration)| declaration.place == *source)
                            {
                                parameter_sources.push((*position, *source));
                            }
                        }
                    }
                    parameter_sources.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
                    parameter_sources.dedup_by_key(|(_, place)| *place);
                    local_discards.extend(
                        parameter_sources
                            .into_iter()
                            .map(|(_, place)| (place, false)),
                    );
                    local_discards.extend(discards.iter().map(|place| (*place, true)));
                    *discards = evaluation.selection_return_discards(local_discards)?;
                }
                // Ordinary successors already partitioned each edge's residual
                // custody inside `successor`; no global return splice applies.
                Terminator::Jump { .. } | Terminator::Conditional { .. } => {}
                _ => {
                    return unsupported(
                        "owned selection residuals crossing authored states require retained cleanup transfer correspondence",
                    );
                }
            }
        }
        if let Some(rank) = current_rank {
            if matches!(
                state.terminator,
                CheckedComposedUnitControlTerminatorPlan::Guarded { .. }
            ) {
                // Ordered guard/return blocks stay inside this authored state;
                // none of their edges asserts an authored decrease.
                self.block_ranks
                    .extend(edge_blocks.iter().map(|block| (block.id, rank)));
                self.rank_edges.extend(edge_blocks.iter().flat_map(|block| {
                    block.terminator.edges().map(|edge| {
                        (
                            edge,
                            (
                                rank,
                                terminal_psi::TerminalNaturalRankComparison::Preserving,
                            ),
                        )
                    })
                }));
                self.rank_edges.extend(terminator.edges().map(|edge| {
                    (
                        edge,
                        (
                            rank,
                            terminal_psi::TerminalNaturalRankComparison::Preserving,
                        ),
                    )
                }));
            }
            // Completed evaluation blocks stay inside this authored state.
            // Their private edges preserve its incoming rank; only the state
            // successor constructed above claims an authored strict decrease.
            self.rank_edges
                .extend(evaluation.blocks.iter().flat_map(|block| {
                    block.terminator.edges().map(|edge| {
                        (
                            edge,
                            (
                                rank,
                                terminal_psi::TerminalNaturalRankComparison::Preserving,
                            ),
                        )
                    })
                }));
        }
        evaluation.remap_transported_call_operands(&mut operations);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: evaluation.parameters,
            erased_scalar_formals: Vec::new(),
            structural_parameters: evaluation.block_structural_parameters,
            operations: operations[evaluation.operation_start
                ..if condition.is_some() || branch_guard.is_some() || prepared_cases.is_some() {
                    body_end
                } else {
                    operations.len()
                }]
                .to_vec(),
            terminator,
        });
        if position != 0 || self.entry_reentered {
            // Argument evaluation can split the body; bindings belong to its source root.
            // Claim-aliased parameters keep the entry parameter's place, so
            // they are not block parameters either.
            let aliased = &self.admitted.claim_transport.aliased;
            let root = evaluation
                .blocks
                .iter_mut()
                .find(|block| block.id == self.state_ids[position])
                .ok_or(LoweringError::Unsupported(
                    "Unit graph state root disappeared during evaluation",
                ))?;
            root.structural_parameters = std::mem::take(&mut self.state_views[position])
                .into_iter()
                .enumerate()
                .filter(|(dense, parameter)| {
                    !parameter.is_self
                        && !aliased
                            .get(position)
                            .is_some_and(|aliased| aliased.contains_key(&(*dense as u32)))
                })
                .map(|(_, parameter)| parameter)
                .collect();
            // The authored state's own erased roster rides on its root block;
            // a plain entry's formals already live on the machine contract.
            root.erased_scalar_formals = self.state_erased[position].clone();
        }
        if let Some(rank) = current_rank {
            self.block_ranks
                .extend(evaluation.blocks.iter().map(|block| (block.id, rank)));
        }
        self.blocks.extend(evaluation.blocks);
        self.blocks.extend(edge_blocks);
        self.occurrences.extend(operations.source_calls);
        self.catalogs.next_value = next_value;
        self.catalogs.next_block = next_block;
        self.catalogs.next_edge = next_edge;
        self.catalogs.next_operation = operations.next_identity;
        Ok(())
    }
}
