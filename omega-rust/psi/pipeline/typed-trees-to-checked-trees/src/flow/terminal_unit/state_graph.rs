//! State-local calls and explicit successor bindings, independent of graph shape.

use super::*;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let states = program.machine_states(machine);
    if states.len() < 2 || !machine_binders(program, machine).is_empty() {
        return None;
    }
    let slice_length_ranks = if machine.termination_plan.implementation_witness.is_some() {
        // Other retained witnesses belong to their existing producer until this
        // path can preserve them. Never publish an unranked replacement.
        let ranks = crate::checks::termination::proven_slice_length_ranks(program, machine)?;
        if ranks.is_empty() {
            return None;
        }
        ranks
    } else {
        Vec::new()
    };
    let mut attachment = None;
    let mut signatures = Vec::new();
    for state in states {
        if !is_unit(program, state.return_type) || !program.state_contracts(state).is_empty() {
            return None;
        }
        let (structural, scalar) = if machine.attached_data.is_some() {
            let (identity, structural, scalar) =
                structural_scalar_signature(program, shapes, machine, state, &[], true)?;
            attachment = Some(identity);
            (structural, scalar)
        } else {
            free_structural_scalar_signature(program, shapes, state, &[])?
        };
        // Persistent receivers keep their invocation place. Non-receiver views
        // retain the existing explicit descriptor transfer on every edge.
        if structural.iter().any(|parameter| {
            parameter.multiplicity != Multiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || if parameter.is_self {
                    parameter.access != CheckedStructuralAccess::MutableBorrow
                } else {
                    parameter.access != CheckedStructuralAccess::SharedBorrow
                        || byte_sequence_carrier(
                            program,
                            program.state_parameters(state)[parameter.position as usize]
                                .type_reference,
                            &[],
                        ) != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                }
        }) {
            return None;
        }
        if !entry_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural,
            program.state_parameters(state),
        )?
        .is_empty()
        {
            return None;
        }
        signatures.push((structural, scalar));
    }
    let mut planned = Vec::new();
    for (state_index, state) in states.iter().enumerate() {
        let (structural, scalar) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        let bindings = match crate::flow::terminal_scalar::checked_binding_prefix(
            program,
            state,
            &facts.values.scalar_computations,
        ) {
            Some(bindings) => bindings,
            // A structural assignment is an ordered effect, not scalar storage.
            // The shared sequence must account for every statement below.
            None if matches!(statements.first(), Some(StatementNode::Assignment(_))) => Vec::new(),
            None => return None,
        };
        let binding_initializers = prefix_initializers(program, facts, state, &bindings)?;
        let binding_count = bindings.len();
        let terminator_index = statements
            .iter()
            .position(|statement| matches!(statement, StatementNode::Transition(_)))
            .unwrap_or(statements.len());
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span_or_empty(flow.calls);
        if source_calls
            .windows(2)
            .any(|pair| pair[0].statement_index > pair[1].statement_index)
        {
            return None;
        }
        let first_call = source_calls.partition_point(|call| call.statement_index < binding_count);
        let after_calls =
            source_calls.partition_point(|call| call.statement_index < terminator_index);
        // Computation roots retain handles into this arena. Borrow the original
        // occurrences so their exact identity survives nested-call validation.
        let calls = control::outer_calls(
            program,
            facts,
            machine.symbol,
            state,
            &source_calls[first_call..after_calls],
        )?;
        let sequence = control::statement_sequence::build(
            program,
            facts,
            shapes,
            machine,
            state,
            structural,
            scalar,
            &[],
            &calls,
            &[],
            &[],
            binding_count,
        )?;
        if sequence.local_count != binding_count || !sequence.structural_local_symbols.is_empty() {
            return None;
        }
        let operations = sequence.operations;
        for operation in &operations {
            match &operation {
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    structural_arguments,
                    completion_receipts,
                    ..
                } if completion_receipts.is_empty()
                    && whole_view_arguments(structural_arguments) => {}
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    ..
                } if claim_transfers.is_empty()
                    && structural_arguments.iter().all(|argument| {
                        argument.source_parameter_index().is_some()
                            && argument.path.is_empty()
                            && matches!(
                                argument.access,
                                CheckedStructuralAccess::MutableBorrow
                                    | CheckedStructuralAccess::SharedBorrow
                            )
                    }) => {}
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_) => {}
                _ => return None,
            }
        }
        let ordinal = u32::try_from(terminator_index).ok()?;
        let edge = |transition, edge_ordinal| {
            successor(
                program,
                facts,
                machine,
                state_index,
                &signatures,
                transition,
                edge_ordinal,
            )
        };
        let terminator = match &statements[terminator_index..] {
            [] => CheckedComposedUnitControlTerminatorPlan::ReturnUnit,
            [StatementNode::Transition(transition)]
                if transition.guard == TransitionGuardNode::Always =>
            {
                CheckedComposedUnitControlTerminatorPlan::Jump {
                    successor: edge(transition, ordinal)?,
                }
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                && composed_control::topology::exact_false_fallback(
                    program, when_true, when_false,
                ) =>
            {
                let guard = facts
                    .values
                    .scalar_expressions
                    .expression_at(state.symbol, ordinal, CheckedScalarExpressionRole::Guard)?
                    .clone();
                if !matches!(guard, CheckedScalarExpression::Boolean(_)) {
                    return None;
                }
                CheckedComposedUnitControlTerminatorPlan::Conditional {
                    guard,
                    when_true: edge(when_true, ordinal)?,
                    when_false: edge(when_false, ordinal.checked_add(1)?)?,
                }
            }
            _ => return None,
        };
        planned.push(CheckedComposedUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: structural.clone(),
            scalar_parameters: scalar.clone(),
            entry_claims: Vec::new(),
            bindings,
            binding_initializers,
            operations,
            terminator,
        });
    }
    // Retain only this machine's direct provider-field calls. Each ordinary
    // callee owns its own attachment requirements, even through a receiver loan.
    let provider_attachment_requirements = if let Some(identity) = &attachment {
        let flows = states
            .iter()
            .zip(&planned)
            .map(|(state, plan)| {
                let flow = state_flow(facts, machine.symbol, state.symbol)?;
                Some((
                    state,
                    facts.flow.control.calls.span_or_empty(flow.calls),
                    plan.operations.as_slice(),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        checked_composed_provider_attachment_requirements(
            program, shapes, machine, identity, &flows,
        )?
    } else {
        Vec::new()
    };
    let mut plan = composed_control::finish_state_graph(
        facts,
        machine,
        attachment,
        provider_attachment_requirements,
        planned,
    )?;
    plan.slice_length_ranks = slice_length_ranks;
    Some(plan)
}

fn prefix_initializers(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    bindings: &[CheckedScalarBinding],
) -> Option<Vec<CheckedScalarExpression>> {
    use checked_trees::CheckedScalarBindingDestination;

    let statements = program.statement_table.statements(state.statement_nodes);
    let mut immutable_ordinal = 0u32;
    bindings
        .iter()
        .map(|binding| {
            if binding.value != CheckedScalarBindingValue::Expression {
                return None;
            }
            let (role, destination) = match binding.destination {
                CheckedScalarBindingDestination::Immutable => {
                    let StatementNode::LocalData(local) =
                        statements.get(binding.statement_ordinal as usize)?
                    else {
                        return None;
                    };
                    let role = CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: immutable_ordinal,
                    };
                    immutable_ordinal = immutable_ordinal.checked_add(1)?;
                    (role, local.symbol)
                }
                CheckedScalarBindingDestination::StorageInitialize { symbol } => {
                    (CheckedScalarExpressionRole::StorageInitializer, symbol)
                }
                CheckedScalarBindingDestination::StorageAssign { symbol } => {
                    (CheckedScalarExpressionRole::AssignmentValue, symbol)
                }
            };
            let expression = match statements.get(binding.statement_ordinal as usize)? {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Assignment(assignment) => assignment.value,
                _ => return None,
            };
            let (custody, initializer) = facts.values.scalar_expressions.bound_expression_at(
                state.symbol,
                binding.statement_ordinal,
                role,
            )?;
            if custody.expression != expression
                || custody.destination != destination
                || crate::values::scalar_expression_type(initializer)
                    != Some(binding.primitive_type)
                || matches!(initializer, CheckedScalarExpression::Boolean(boolean)
                    if checked_boolean_contains_short_circuit(boolean))
            {
                return None;
            }
            Some(initializer.clone())
        })
        .collect()
}

fn whole_view_arguments(arguments: &[CheckedUnitStructuralArgumentPlan]) -> bool {
    arguments.iter().all(|argument| {
        argument.source_parameter_index().is_some()
            && argument.path.is_empty()
            && argument.access == CheckedStructuralAccess::SharedBorrow
    })
}

type Signature = (
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
);

fn successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    source_index: usize,
    signatures: &[Signature],
    transition: &typed_trees::statement::TableTransition,
    ordinal: u32,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let states = program.machine_states(machine);
    let target_index = crate::checks::termination::named_transition_target_state_index(
        program,
        machine,
        path.symbol,
    )?;
    let source = &states[source_index];
    let target = &states[target_index];
    let arguments = program.statement_table.expression_handles(*arguments);
    let target_parameters = program.state_parameters(target);
    if arguments.len()
        != target_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
    {
        return None;
    }
    let (source_structural, source_scalar) = &signatures[source_index];
    let (target_structural, target_scalar) = &signatures[target_index];
    let argument_at = |position: u32| {
        let position = target_parameters
            .iter()
            .take(position as usize)
            .filter(|parameter| !parameter.is_self)
            .count();
        arguments.get(position).copied()
    };
    let source_position = |position: u32| {
        let argument = argument_at(position)?;
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            source.symbol,
            ordinal as usize,
            argument,
        )?;
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        if !place.segments.is_empty() {
            return None;
        }
        program
            .state_parameters(source)
            .iter()
            .position(|parameter| parameter.symbol == symbol)
    };
    let transfers = target_structural
        .iter()
        .enumerate()
        .map(|(target_index, target)| {
            if target.is_self {
                let source_index = source_structural.iter().position(|parameter| parameter.is_self)?;
                if source_structural[source_index] != *target {
                    return None;
                }
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                        index: u32::try_from(source_index).ok()?,
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            let expression = argument_at(target.position)?;
            if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
                && let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
            {
                let target_parameter = target_parameters.get(target.position as usize)?;
                let (parameter_index, type_identity) = calls::byte_subslice::source(
                    program, facts, machine, source, source_structural,
                    target_parameter.type_reference, expression, ordinal as usize,
                )?;
                if type_identity != target.type_identity {
                    return None;
                }
                let source_parameter = program.state_parameters(source).get(
                    source_structural.get(parameter_index as usize)?.position as usize,
                )?;
                if !matches!(program.expression_table.expression(indexed.collection),
                    ExpressionNode::Name(path) if path.symbol == source_parameter.symbol
                        && path.head_symbol == source_parameter.symbol
                        && program.expression_table.name_path_members(path.members).len() == 1)
                {
                    return None;
                }
                for (endpoint, role) in [
                    (range.start, CheckedScalarExpressionRole::TransitionSubsliceStart {
                        argument_ordinal: target.position,
                    }),
                    (range.end, CheckedScalarExpressionRole::TransitionSubsliceEnd {
                        argument_ordinal: target.position,
                    }),
                ] {
                    if !endpoint.is_valid() {
                        continue;
                    }
                    let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                        source.symbol, ordinal, role,
                    )?;
                    if binding.expression != endpoint || binding.destination.is_valid()
                        || value.primitive_type() != Some(PrimitiveType::U64)
                    {
                        return None;
                    }
                }
                return Some(CheckedStructuralControlTransferPlan {
                    source: checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                        parameter_index,
                        expression,
                    },
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            let source_position = source_position(target.position)?;
            let source_index = source_structural
                .iter()
                .position(|parameter| parameter.position as usize == source_position)?;
            let source = &source_structural[source_index];
            if source.type_identity != target.type_identity || source.access != target.access {
                return None;
            }
            Some(CheckedStructuralControlTransferPlan {
                source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                    index: u32::try_from(source_index).ok()?,
                },
                target_parameter_index: u32::try_from(target_index).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let scalar_arguments = target_scalar
        .iter()
        .enumerate()
        .map(|(target_index, target)| {
            let argument = argument_at(target.source_position)?;
            let (custody, expression) = facts.values.scalar_expressions.bound_expression_at(
                source.symbol,
                ordinal,
                CheckedScalarExpressionRole::TransitionArgument {
                    argument_ordinal: target.source_position,
                },
            )?;
            if custody.expression != argument
                || custody.destination
                    != target_parameters
                        .get(target.source_position as usize)?
                        .symbol
                || crate::values::scalar_expression_type(expression) != Some(target.primitive_type)
            {
                return None;
            }
            let immutable_source_position = source_position(target.source_position)
                .filter(|position| !program.state_parameters(source)[*position].is_mutable);
            let source = if let Some(source_position) = immutable_source_position {
                let source_index = source_scalar
                    .iter()
                    .position(|parameter| parameter.source_position as usize == source_position)?;
                if source_scalar[source_index].primitive_type != target.primitive_type {
                    return None;
                }
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Parameter {
                    index: u32::try_from(source_index).ok()?,
                }
            } else {
                checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression
            };
            Some(CheckedStructuralScalarArgumentPlan {
                argument_ordinal: target.source_position,
                source,
                target_scalar_parameter_index: u32::try_from(target_index).ok()?,
                primitive_type: target.primitive_type,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
        machine.symbol,
        source.symbol,
        ordinal,
    )?;
    if cleanup.target_state != target.symbol
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: target.symbol,
        transfers,
        scalar_arguments,
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
