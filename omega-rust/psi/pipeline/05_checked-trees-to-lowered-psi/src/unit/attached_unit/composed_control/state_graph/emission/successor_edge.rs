//! One successor edge of a composed-control state: the owners that die on
//! it, the staged evaluation of its scalar and structural arguments, its
//! erased actuals, and the rank it arrives with.

use super::super::super::super::super::{
    StructuralAccess, StructuralArgument, StructuralFieldId, StructuralParameterDeclaration,
    SuccessorEdge, ValueId, block_id,
};
use super::super::super::super::{
    Block, CheckedScalarExpression, CheckedScalarExpressionRole, Operation, OperationKind,
    OperationResult, PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, Terminator, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, edge_id, emit_direct_expression, place_id,
    terminal_scalar_type, unsupported, validate_direct_parameter_types,
};
use super::super::super::LoweringError;
use super::super::{
    CheckedStructuralControlSuccessorPlan, case_emission, case_leaf_copy, ranking, result_custody,
    scalars, subslices,
};
use super::StateGraphEmission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::expression_preparation::bindings::ScalarBindings;
use crate::expression_preparation::bindings::structural_fields::{
    EstablishedCasePayload, NestedEstablishedCasePayload,
};
use crate::terminal_identities::value_id;

impl StateGraphEmission<'_, '_> {
    /// Lower one edge leaving the state at `frame.position`: dispose of the
    /// owners its target does not receive, evaluate its arguments (in a
    /// private staging block when a case edge or established payload needs
    /// them after selection), and record the target's arrival and rank.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn successor_edge(
        &mut self,
        frame: &SuccessorFrame<'_>,
        emitted: &mut EmittedState<'_>,
        edge: &CheckedStructuralControlSuccessorPlan,
        payload_values: &[(u32, ValueDeclaration)],
        case_edge: bool,
        values: &[ValueDeclaration],
        established: &[EstablishedCasePayload],
    ) -> Result<SuccessorEdge, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let SuccessorFrame {
            position,
            state_parameters,
            bindings,
            current_rank,
            rank_ceiling,
            case_subject,
            guarded,
            inherited_lengths,
            inherited_field_lengths,
            consumed_case_subjects,
        } = *frame;
        let state = &plan.states[position];
        let EmittedState {
            operations,
            evaluation,
            next_value,
            next_block,
            next_edge,
            edge_blocks,
        } = emitted;
        // Every edge retains its local remainder until selected operands
        // finish. A case edge has already consumed only its subject.
        let mut trivial_affine_discards = if evaluation.selection_cleanups.is_empty() {
            result_custody::local_discards(
                checked,
                plan.machine,
                &self.admitted.source_states[position],
                state,
                Some(edge),
            )?
            .into_iter()
            .map(|ordinal| {
                let result = case_emission::result(state, ordinal, operations)?;
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
                operations,
                evaluation,
            )?
        };
        // Owned parameters the target does not receive die on this edge,
        // at whatever place their value occupies when control leaves, in
        // reverse declaration order.
        let mut dying = edge.trivial_affine_discard_parameter_positions.clone();
        if let Some((_, subject)) = consumed_case_subjects
            .iter()
            .find(|(ordinal, _)| *ordinal == edge.statement_ordinal)
            && !dying.contains(subject)
            && evaluation
                .structural_parameters
                .iter()
                .any(|(position, parameter)| {
                    position == subject && parameter.access == StructuralAccess::Owned
                })
        {
            dying.push(*subject);
            dying.sort_by_key(|position| std::cmp::Reverse(*position));
        }
        for position in &dying {
            let (_, parameter) = evaluation
                .structural_parameters
                .iter()
                .find(|(source_position, _)| source_position == position)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph edge discards a parameter its state does not declare",
                ))?;
            trivial_affine_discards.push(evaluation.current_structural_place(parameter.place));
        }
        if case_edge {
            let consumed = case_subject.ok_or(LoweringError::Unsupported(
                "Unit graph case edge lost its consumed subject",
            ))?;
            trivial_affine_discards.retain(|place| *place != consumed);
        }
        operations.byte_lengths = inherited_lengths.to_vec();
        operations.field_byte_lengths = inherited_field_lengths.to_vec();
        let target = plan
            .states
            .iter()
            .position(|state| state.state == edge.target_state)
            .ok_or(LoweringError::Unsupported(
                "Unit graph target disappeared during emission",
            ))?;
        // Established payloads are parameters of a guard dispatch block,
        // so the edge's arguments must be evaluated after selection.
        let stage = case_edge || !established.is_empty() || edge.scalar_arguments.iter().any(|argument| matches!(
            argument.source, checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
        )) || edge.transfers.iter().any(|transfer| matches!(
            transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. }
        )) || guarded
                && ((current_rank.is_some() && ranking::has_rank(plan, &plan.states[target])) || edge.transfers.iter().any(|transfer| matches!(
                    transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
                        | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice { .. }
                )));
        let operation_start = operations.len();
        let staged = if stage {
            block_id(allocate_dense(next_block)?)
        } else {
            evaluation.current
        };
        let mut edge_evaluation = evaluation.branch(staged, operation_start);
        let mut edge_values = values.to_vec();
        let mut nested_rows = Vec::new();
        if stage {
            // A read below a case payload's record member in this edge's
            // scalar arguments mints the member leaf copy and its scalar
            // field read in the staged block, which the selecting case
            // edge dominates, then the read binds at the value's
            // namespace position.
            for transfer in &edge.scalar_arguments {
                if !matches!(
                    transfer.source,
                    checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
                ) {
                    continue;
                }
                let argument = scalars::successor_value(checked, state, edge, transfer)?;
                let Some(expression) = argument.as_pure() else {
                    continue;
                };
                let mut reads = Vec::new();
                scalars::nested_case_payload_reads(expression, &mut reads);
                for (parameter_position, case_identity, member_identity, leaf_identity) in reads {
                    let Some(payload) = crate::expression_preparation::bindings::structural_fields::plan_case_payload_leaf(
                        &edge_evaluation.structural_fields,
                        parameter_position,
                        case_identity,
                        member_identity,
                        leaf_identity,
                    ) else {
                        continue;
                    };
                    if nested_rows
                        .iter()
                        .any(|row: &NestedEstablishedCasePayload| {
                            row.source == payload.source
                                && row.case == payload.case
                                && row.member == payload.member
                                && row.field == payload.field
                        })
                    {
                        continue;
                    }
                    let value = ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type: payload.scalar_type,
                    };
                    let position = edge_values.len();
                    edge_values.push(value);
                    let source = edge_evaluation.current_structural_place(payload.source);
                    let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                    let producer = operations.allocate();
                    operations.push(Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: producer,
                        result: OperationResult::Structural(
                            terminal_psi::StructuralOperationResult {
                                qualification_establishments: Vec::new(),
                                place: destination,
                                structural_type: payload.member_type,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                claims: Vec::new(),
                            },
                        ),
                        kind: OperationKind::StructuralCaseLeafCopy {
                            source,
                            path: vec![
                                semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                                    payload.case,
                                ),
                                semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                    payload.member,
                                ),
                            ],
                        },
                    });
                    self.structural_places.push(StructuralPlaceDeclaration {
                        id: destination,
                        kind: StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: payload.member_type,
                        },
                    });
                    let leaf_producer = operations.allocate();
                    operations.push(Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: leaf_producer,
                        result: OperationResult::Scalar(value),
                        kind: OperationKind::IntegerStructuralField {
                            source: destination,
                            path: Vec::new(),
                            field: payload.field,
                        },
                    });
                    nested_rows.push(NestedEstablishedCasePayload {
                        source: payload.source,
                        case: payload.case,
                        member: payload.member,
                        field: payload.field,
                        position,
                    });
                }
            }
        }
        crate::expression_preparation::bindings::structural_fields::establish_case_payloads(
            &mut edge_evaluation.structural_fields,
            established,
            &nested_rows,
        );
        edge_evaluation.parameters = payload_values.iter().map(|(_, value)| *value).collect();
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
                        return unsupported("Unit graph receiver lost original invocation place");
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
                        checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => edge_evaluation.current_structural_place(case_emission::result(state, binding_ordinal, operations)?.place),
                        checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                            edge_evaluation.current_structural_place(state_parameters.get(index as usize).ok_or(
                                LoweringError::Unsupported("Unit graph transfer source descriptor disappeared"),
                            )?.place)
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { .. }
                        | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice { .. } => {
                            let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            self.structural_places.push(subslices::emit(
                                checked, state, edge.statement_ordinal, target_parameter.position, &transfer.source,
                                state_parameters, &edge_evaluation.view_locals, destination, bindings, &edge_values,
                                next_value, operations,
                            )?);
                            destination
                        }
                        checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                            ref subject, ref case_identity, ref field_identity, ref path,
                        } => {
                            let (source, root_type) = match subject.source {
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                                    let parameter = state_parameters.get(parameter_index as usize).ok_or(
                                        LoweringError::Unsupported("Unit graph case-payload subject descriptor disappeared"),
                                    )?;
                                    (edge_evaluation.current_structural_place(parameter.place), parameter.structural_type)
                                }
                                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                                    let produced = case_emission::result(state, binding_ordinal, operations)?;
                                    (edge_evaluation.current_structural_place(produced.place), produced.structural_type)
                                }
                                _ => {
                                    return unsupported(
                                        "Unit graph case-payload subject source unsupported",
                                    );
                                }
                            };
                            if matches!(subject.access, checked_trees::CheckedStructuralAccess::WriteOnlyBorrow) {
                                return unsupported(
                                    "Unit graph case-payload subject is write-only",
                                );
                            }
                            let destination = place_id(allocate_dense(&mut self.catalogs.next_place)?);
                            self.structural_places.push(case_leaf_copy::emit(
                                &self.catalogs.structural_types, source, root_type, &subject.path,
                                &subject.type_identity, case_identity, field_identity, path,
                                target_parameter, destination, operations,
                            )?);
                            destination
                        }
                    };
                structural_arguments.push(StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: match target_parameter.access {
                        checked_trees::CheckedStructuralAccess::Owned => StructuralAccess::Owned,
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
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter { index } => {
                    bindings.expression(&CheckedScalarExpression::Parameter {
                        position: index as usize,
                        primitive_type: transfer.primitive_type,
                    })?
                }
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression => {
                    let value = scalars::successor_value(checked, state, edge, transfer)?;
                    let mut calls = self.catalogs.scalar_calls.emission_context();
                    let value = edge_evaluation.source_value(
                        checked,
                        plan.machine,
                        state.state,
                        edge.statement_ordinal,
                        CheckedScalarExpressionRole::TransitionArgument {
                            argument_ordinal: transfer.argument_ordinal,
                        },
                        &value,
                        edge_values.len(),
                        &mut edge_values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                        &mut calls,
                    )?;
                    self.catalogs.scalar_calls.next_call_obligation =
                        calls.next_obligation_identity;
                    if value.scalar_type != terminal_scalar_type(transfer.primitive_type)? {
                        return unsupported("Unit graph successor value has the wrong carrier");
                    }
                    arguments.push(value.id);
                    continue;
                }
            };
            if expression.scalar_type() != terminal_scalar_type(transfer.primitive_type)?
                || direct_expression_contains_short_circuit(&expression)
            {
                return unsupported("Unit graph successor needs a matching branch-free value");
            }
            validate_direct_parameter_types(
                &expression,
                &edge_values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            arguments.push(emit_direct_expression(
                &expression,
                &edge_values,
                next_value,
                operations,
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
                    &edge_values,
                    &self.state_erased[position],
                )
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        // Proof actuals keep the same rule: each checked term resolves
        // against the emitting state's roster and must land in the
        // target state's roster order.
        if edge.erased_proof_arguments.len() != self.state_erased_proof[target].len() {
            return unsupported("Unit graph successor erased proof arity drifted");
        }
        let erased_proof_arguments = edge
            .erased_proof_arguments
            .iter()
            .map(|term| {
                crate::scalar_graph::scalar_contracts::checked_proof_term(
                    checked,
                    term,
                    &self.state_erased_proof[position],
                )
                .and_then(|term| {
                    crate::scalar_graph::scalar_contracts::lowered_proof_term(
                        &term,
                        &edge_values,
                        &self.state_erased[position],
                    )
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let arriving_rank = if current_rank.is_some() {
            // The target's measure over this edge's actual arguments,
            // which the verifier substitutes for the target's own rank.
            if let Some(rank) = ranking::scalar_rank(plan, target_state) {
                Some(
                    ranking::emit_scalar_rank(
                        rank,
                        &arguments,
                        rank_ceiling,
                        next_value,
                        operations,
                    )?
                    .rank,
                )
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
                                next_value,
                                operations,
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
                            next_value,
                            operations,
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
            let backedge = edge_id(allocate_dense(next_edge)?);
            let selection_edge = edge_id(allocate_dense(next_edge)?);
            self.arrival_edges.entry(target).or_default().push(backedge);
            if let Some(rank) = current_rank {
                self.block_ranks.insert(staged, rank);
                self.block_ranks.insert(edge_evaluation.current, rank);
                self.block_ranks
                    .extend(edge_evaluation.blocks.iter().map(|block| (block.id, rank)));
                self.rank_edges
                    .extend(edge_evaluation.blocks.iter().flat_map(|block| {
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
            edge_blocks.extend(edge_evaluation.blocks);
            edge_blocks.push(Block {
                id: edge_evaluation.current,
                parameters: edge_evaluation.parameters,
                // The forwarding block redeclares the emitting state's
                // erased roster so forwarded proof terms stay in scope.
                erased_scalar_formals: self.state_erased[position].clone(),
                erased_proof_formals:
                    crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations(
                        &self.state_erased_proof[position],
                    ),
                structural_parameters: edge_evaluation.block_structural_parameters,
                operations: operations[edge_evaluation.operation_start..].to_vec(),
                terminator: Terminator::Jump {
                    edge: backedge,
                    target,
                    arguments,
                    structural_arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    trivial_affine_discards,
                    residual_affine_discards: Vec::new(),
                },
            });
            Ok(SuccessorEdge {
                edge: selection_edge,
                target: staged,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: (0..self.state_erased_proof[position].len())
                    .map(|position| semantic_vocabulary::ProofTerm::Formal {
                        position: u32::try_from(position)
                            .expect("erased-proof roster positions fit u32"),
                    })
                    .collect(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            })
        } else {
            let successor_edge = edge_id(allocate_dense(next_edge)?);
            self.arrival_edges
                .entry(target)
                .or_default()
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
                erased_proof_arguments,
                trivial_affine_discards,
            })
        }
    }
}

/// The emitting state's facts every successor edge reads, fixed once the
/// state's prefix, rank and dispatch are emitted.
#[derive(Clone, Copy)]
pub(super) struct SuccessorFrame<'f> {
    pub(super) position: usize,
    pub(super) state_parameters: &'f [StructuralParameterDeclaration],
    pub(super) bindings: &'f ScalarBindings,
    pub(super) current_rank: Option<ValueId>,
    pub(super) rank_ceiling: Option<ValueId>,
    /// The closed-sum subject a case edge has already consumed.
    pub(super) case_subject: Option<PlaceId>,
    /// Whether the state decides between successors on a scalar guard.
    pub(super) guarded: bool,
    pub(super) inherited_lengths: &'f [(PlaceId, ValueId)],
    pub(super) inherited_field_lengths: &'f [(
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        StructuralFieldId,
        ValueId,
    )],
    pub(super) consumed_case_subjects: &'f [(u32, u32)],
}

/// The state's emission in flight that each successor edge extends.
pub(super) struct EmittedState<'e> {
    pub(super) operations: &'e mut OperationBuffer,
    pub(super) evaluation: &'e mut crate::unit::attached_unit::argument_evaluation::Evaluation,
    pub(super) next_value: &'e mut u64,
    pub(super) next_block: &'e mut u64,
    pub(super) next_edge: &'e mut u64,
    pub(super) edge_blocks: &'e mut Vec<Block>,
}
