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
        // These views have invocation lifetime and no owned frontier. Other
        // structural values need their state-edge ownership transfer retained.
        if structural.iter().any(|parameter| {
            parameter.is_self
                || parameter.multiplicity != Multiplicity::Unrestricted
                || parameter.access != CheckedStructuralAccess::SharedBorrow
                || !parameter.qualifications.is_empty()
                || byte_sequence_carrier(
                    program,
                    program.state_parameters(state)[parameter.position as usize].type_reference,
                    &[],
                ) != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
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
        let call_count = statements
            .iter()
            .enumerate()
            .take_while(|(ordinal, statement)| {
                matches!(statement, StatementNode::Call(_))
                    || control::tail_call(program, state, *ordinal).is_some()
            })
            .count();
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let body_calls = facts
            .flow
            .control
            .calls
            .span_or_empty(flow.calls)
            .iter()
            .filter(|call| call.statement_index < call_count)
            .cloned()
            .collect::<Vec<_>>();
        let calls = control::outer_calls(program, facts, machine.symbol, state, &body_calls)?;
        if calls.len() != call_count {
            return None;
        }
        let mut operations = Vec::new();
        for (ordinal, call) in calls.iter().enumerate() {
            if call.statement_index != ordinal || call.call_ordinal != 0 {
                return None;
            }
            let operation = build_call_operation(
                program,
                facts,
                machine,
                state,
                structural,
                &[],
                &[],
                &[],
                call,
                false,
                None,
                &[],
            )?;
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
                } if structural_arguments.is_empty() && claim_transfers.is_empty() => {}
                _ => return None,
            }
            operations.push(operation);
        }
        let ordinal = u32::try_from(call_count).ok()?;
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
        let terminator = match &statements[call_count..] {
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
            bindings: Vec::new(),
            binding_initializers: Vec::new(),
            operations,
            terminator,
        });
    }
    // Receiver-backed provider slots retain their specialized owner path. This
    // graph path uses direct calls, not implicit attachment field authority.
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
    if !provider_attachment_requirements.is_empty() {
        return None;
    }
    composed_control::finish_state_graph(
        facts,
        machine,
        attachment,
        provider_attachment_requirements,
        planned,
    )
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
    let target_index = states
        .iter()
        .position(|state| state.symbol == path.symbol)?;
    let source = &states[source_index];
    let target = &states[target_index];
    let arguments = program.statement_table.expression_handles(*arguments);
    if arguments.len() != program.state_parameters(target).len() {
        return None;
    }
    let (source_structural, source_scalar) = &signatures[source_index];
    let (target_structural, target_scalar) = &signatures[target_index];
    let source_position = |position: u32| {
        let argument = *arguments.get(position as usize)?;
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
            let source_position = source_position(target.position)?;
            let source_index = source_structural
                .iter()
                .position(|parameter| parameter.position as usize == source_position)?;
            let source = &source_structural[source_index];
            if source.type_identity != target.type_identity || source.access != target.access {
                return None;
            }
            Some(CheckedStructuralControlTransferPlan {
                source_parameter_index: u32::try_from(source_index).ok()?,
                target_parameter_index: u32::try_from(target_index).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let scalar_arguments = target_scalar
        .iter()
        .enumerate()
        .map(|(target_index, target)| {
            let source_position = source_position(target.source_position)?;
            let source_index = source_scalar
                .iter()
                .position(|parameter| parameter.source_position as usize == source_position)?;
            if source_scalar[source_index].primitive_type != target.primitive_type {
                return None;
            }
            Some(CheckedStructuralScalarArgumentPlan {
                argument_ordinal: target.source_position,
                source_scalar_parameter_index: u32::try_from(source_index).ok()?,
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
