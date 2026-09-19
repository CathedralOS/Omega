//! State-local calls and explicit successor bindings, independent of graph shape.
use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedScalarBinding, CheckedScalarBindingValue,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan, ExpressionNode,
    Multiplicity, PermissionEventKind, PermissionEventSource, PrimitiveType, StatementNode,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode, TypedTrees,
    calls,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::{
    ShapeCollector, checked_composed_provider_attachment_requirements, composed_control, control,
    entry_claims, free_structural_scalar_signature_traced, machine_binders,
    return_unit_affine_discards, state_flow, structural_scalar_signature_traced,
};

mod closed_sum;
mod local_results;
pub(super) mod returns;

#[cfg(test)]
mod tests;

/// Test convenience: the traced builder without a trace.
#[cfg(test)]
pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    build_traced(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        call_frames,
        &control::LocalConstructionTrace::default(),
    )
}

/// `build` with a trace of the phase, state, and statement where the general
/// state-graph route stopped when it declines a body.
pub(super) fn build_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    trace.phase("state graph: shape");
    trace.state(None);
    let states = program.machine_states(machine);
    if states.is_empty() || !machine_binders(program, machine).is_empty() {
        return None;
    }
    trace.phase("state graph: result signature");
    let result = returns::signature(program, shapes, states[0].return_type)?;
    if matches!(&result, checked_trees::CheckedControlResultPlan::Structural(result)
        if result.multiplicity == Multiplicity::Linear)
        && !program.machine_contracts(machine).is_empty()
    {
        // Parameter qualifications are transported exactly below. Additional
        // authored contracts still need their own retained proof obligations.
        return None;
    }
    if states.len() < 2
        && result == checked_trees::CheckedControlResultPlan::Unit
        && !facts
            .values
            .structural_values
            .roots
            .iter()
            .any(|(_, root)| root.machine == machine.symbol)
    {
        return None;
    }
    trace.phase("state graph: natural ranks");
    let natural_ranks = if machine.termination_plan.implementation_witness.is_some() {
        // Other retained witnesses belong to their existing producer until this
        // path can preserve them. Never publish an unranked replacement.
        let ranks = crate::checks::termination::proven_state_natural_ranks_with_call_frames(
            program,
            machine,
            call_frames,
        )?;
        if ranks.is_empty() {
            return None;
        }
        ranks
    } else {
        Vec::new()
    };
    let mut attachment = None;
    let mut signatures = Vec::new();
    let mut state_entry_claims = Vec::new();
    let mut state_requires = Vec::new();
    for (state_index, state) in states.iter().enumerate() {
        trace.phase("state graph: state signature: result and contracts");
        trace.state(u32::try_from(state_index).ok());
        if returns::signature(program, shapes, state.return_type)? != result {
            trace.phase("state graph: state signature: result differs");
            return None;
        }
        // Membership contracts still restate the exact declared parameter
        // qualifications. Authored scalar `requires` expressions lower into
        // the state's proof-only contract roster; any other shape stays
        // outside the state graph's admitted custody.
        let Some(contract_predicates) =
            validation::structural_state_contract_scalar_predicates(program, state)
        else {
            trace.phase("state graph: state signature: contract shape unadmitted");
            return None;
        };
        let mut requires = Vec::with_capacity(contract_predicates.len());
        for expression in contract_predicates {
            let Some(predicate) = crate::values::lower_state_scalar_contract_predicate(
                program,
                &facts.operators,
                machine,
                state,
                expression,
                &mut 4096,
            ) else {
                trace.phase("state graph: state signature: predicate lowering failed");
                return None;
            };
            requires.push(Some(checked_trees::ClosedScalarContractValue::Predicate(
                predicate,
            )));
        }
        state_requires.push(requires);
        trace.phase("state graph: state signature: parameter signature");
        let (structural, scalar) = if machine.attached_data.is_some() {
            let (identity, structural, scalar) = structural_scalar_signature_traced(
                program,
                shapes,
                machine,
                state,
                &[],
                true,
                trace,
            )?;
            attachment = Some(identity);
            (structural, scalar)
        } else {
            free_structural_scalar_signature_traced(program, shapes, state, &[], trace)?
        };
        // Persistent receivers keep their invocation place. Other structural
        // parameters retain explicit owned-value or borrowed-view edge custody.
        // Each guard marks its own phase so the omission roster names the
        // custody shape the route lacks, not just the phase.
        trace.phase("state graph: state signature: parameter custody shape");
        for parameter in &structural {
            let reference =
                || program.state_parameters(state)[parameter.position as usize].type_reference;
            if parameter.multiplicity != Multiplicity::Linear
                && !parameter.qualifications.is_empty()
            {
                trace.phase(
                    "state graph: state signature: parameter custody shape: qualified non-linear parameter",
                );
                return None;
            }
            match (parameter.is_self, &parameter.access) {
                (true, CheckedStructuralAccess::MutableBorrow) => {}
                (true, _) => {
                    trace.phase(
                        "state graph: state signature: parameter custody shape: persistent receiver access",
                    );
                    return None;
                }
                (false, CheckedStructuralAccess::Owned) => {
                    if parameter.multiplicity != Multiplicity::Linear
                        && !matches!(
                            program.type_reference_table.type_reference(reference()),
                            TypeReferenceNode::Named { .. }
                        )
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: owned non-linear unnamed type",
                        );
                        return None;
                    }
                    if parameter.multiplicity != Multiplicity::Linear
                        && !validation::has_plain_owned_contents_with_numeric_constraints(
                            program,
                            reference(),
                        )
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: owned non-linear record contents",
                        );
                        return None;
                    }
                }
                (false, access) => {
                    if parameter.multiplicity != Multiplicity::Unrestricted {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed restricted parameter",
                        );
                        return None;
                    }
                    if !matches!(
                        access,
                        CheckedStructuralAccess::SharedBorrow
                            | CheckedStructuralAccess::MutableBorrow
                    ) {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: write-only borrow",
                        );
                        return None;
                    }
                    let reference = reference();
                    let primitive_reference = matches!(program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::Reference { referee, .. }
                            if matches!(program.type_reference_table.type_reference(*referee), TypeReferenceNode::Named { .. })
                                && program.primitive_type_reference(*referee).is_some());
                    if !primitive_reference
                        && byte_sequence_carrier(program, reference, &[])
                            != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed non-view carrier",
                        );
                        return None;
                    }
                }
            }
        }
        trace.phase("state graph: state signature: entry claims");
        let claims = entry_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural,
            program.state_parameters(state),
        )?;
        state_entry_claims.push(claims);
        signatures.push((structural, scalar));
    }
    let mut planned = Vec::new();
    for (state_index, state) in states.iter().enumerate() {
        trace.phase("state graph: prefix initializers");
        trace.state(u32::try_from(state_index).ok());
        let (structural, scalar) = &signatures[state_index];
        let statements = program.statement_table.statements(state.statement_nodes);
        // A body containing structural bindings is not a scalar-only prefix.
        // The shared sequence must account for every statement in that case.
        let bindings = crate::execution::terminal_scalar::checked_binding_prefix(
            program,
            state,
            &facts.values.scalar_computations,
        )
        .unwrap_or_default()
        .into_iter()
        // Only the initial pure bindings belong to prefix initialization.
        // The first computation/call and its suffix use ordinary sequencing,
        // just as when an earlier structural local already requires that path.
        .take_while(|binding| binding.value == CheckedScalarBindingValue::Expression)
        .collect::<Vec<_>>();
        let binding_initializers = prefix_initializers(program, facts, state, &bindings, trace)?;
        let binding_count = bindings.len();
        let terminator_index = statements
            .iter()
            .position(|statement| {
                matches!(statement, StatementNode::Transition(_))
                    || (result != checked_trees::CheckedControlResultPlan::Unit
                        && matches!(statement, StatementNode::Expression(_)))
            })
            .unwrap_or(statements.len());
        trace.phase("state graph: state flow");
        let flow = state_flow(facts, machine.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span_or_empty(flow.calls);
        if source_calls
            .windows(2)
            .any(|pair| pair[0].statement_index > pair[1].statement_index)
        {
            return None;
        }
        let first_call = source_calls.partition_point(|call| call.statement_index < binding_count);
        // A returned structural value is established by the ordinary operation
        // sequence before its return edge. Its operand calls belong to that
        // sequence too; excluding the tail statement loses their exact flow
        // occurrences when outer_calls reconstructs the constructor roots.
        let operation_end = if matches!(
            statements.get(terminator_index),
            Some(StatementNode::Expression(_))
        ) && facts
            .values
            .structural_values
            .root_at(state.symbol, u32::try_from(terminator_index).ok()?)
            .is_some()
            || matches!(statements.get(terminator_index), Some(StatementNode::Expression(expression))
                if matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_)))
        {
            terminator_index.checked_add(1)?
        } else {
            terminator_index
        };
        let after_calls = source_calls.partition_point(|call| call.statement_index < operation_end);
        // Computation roots retain handles into this arena. Borrow the original
        // occurrences so their exact identity survives nested-call validation.
        trace.phase("state graph: outer calls");
        let calls = control::outer_calls_before_traced(
            program,
            facts,
            machine.symbol,
            state,
            &source_calls[first_call..after_calls],
            operation_end,
            trace,
        )?;
        let sequence = control::statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            scalar,
            &state_entry_claims[state_index],
            &calls,
            &[],
            binding_count,
            call_frames,
            trace,
        )?;
        trace.phase("state graph: operation custody");
        let mut operations = sequence.operations;
        // Named results remain live through successor operand evaluation. The
        // selected edge owns their exact transfer/disposal partition below;
        // one global return-discard flag cannot express asymmetric successors.
        for operation in &mut operations {
            match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return,
                    ..
                } if matches!(
                    statements.get(result.statement_index as usize),
                    Some(StatementNode::LocalData(_))
                ) =>
                {
                    *discard_result_on_return = false
                }
                _ => {}
            }
        }
        for (operation_index, operation) in operations.iter().enumerate() {
            if matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                if result.multiplicity == Multiplicity::Linear)
            {
                // Call forwarding preserves checked authority; construction
                // needs a separate issuance witness, not result classification.
                trace.phase("state graph: operation custody: linear established value");
                return None;
            }
            match operation {
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    structural_arguments,
                    completion_receipts,
                    ..
                } if completion_receipts.is_empty()
                    && structural_arguments.iter().all(|argument| {
                        whole_shared_argument(argument)
                            // A borrowed view may project from a parameter,
                            // including the persistent receiver, without
                            // changing custody across the selected edges.
                            || (argument.source_parameter_index().is_some()
                                && matches!(
                                    argument.access,
                                    CheckedStructuralAccess::SharedBorrow
                                        | CheckedStructuralAccess::MutableBorrow
                                ))
                    }) => {}
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    ..
                } if claim_transfers.is_empty()
                    && structural_arguments.iter().all(|argument| {
                        whole_shared_argument(argument)
                            || ((argument.source_parameter_index().is_some()
                                || argument
                                    .source_structural_result_binding_ordinal()
                                    .is_some())
                                && matches!(argument.access,
                                    CheckedStructuralAccess::SharedBorrow
                                        | CheckedStructuralAccess::MutableBorrow))
                            // The statement sequencer already rejoins a local
                            // receiver to its completed producer and exact loan.
                            // State ownership, not parameter spelling, governs
                            // keeping that same home until the selected exit.
                            || (argument.source_structural_result_binding_ordinal().is_some()
                                && matches!(argument.access,
                                    CheckedStructuralAccess::SharedBorrow
                                        | CheckedStructuralAccess::MutableBorrow))
                    }) => {}
                CheckedUnitEffectOperationPlan::StructuralCall {
                    discard_result_on_return: false,
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    discard_result_on_return: false,
                    ..
                }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    discard_result_on_return: false,
                    ..
                }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. } => {}
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                // The ordinary sequencer has already rejoined the primitive
                // destination, RHS, and complete write frame. Crossing a state
                // edge does not turn that non-observing write into a new family.
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_) => {}
                // A call result may die immediately after its producing call.
                // The cleanup shares the call coordinate rather than consuming
                // a new authored statement.
                CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                    coordinate,
                    affine_discards,
                }
                    if affine_discards.iter().all(|discard| {
                        matches!(
                        discard.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            ..
                        }
                    )
                    }) && operation_index.checked_sub(1).is_some_and(|producer| {
                        matches!(
                            &operations[producer],
                            CheckedUnitEffectOperationPlan::StructuralCall {
                                coordinate: call,
                                ..
                            } | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                                coordinate: call,
                                ..
                            } if call == coordinate
                        )
                    }) => {}
                _ => {
                    // Name the operation family whose custody the selected
                    // edges cannot yet carry; admitted families never reach
                    // this arm.
                    trace.phase(match operation {
                        CheckedUnitEffectOperationPlan::BoundaryCall { .. } => {
                            "state graph: operation custody: boundary call arguments"
                        }
                        CheckedUnitEffectOperationPlan::CallUnit { .. } => {
                            "state graph: operation custody: unit call arguments"
                        }
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                            "state graph: operation custody: discarded structural result"
                        }
                        CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. } => {
                            "state graph: operation custody: continuation cleanup owner"
                        }
                        CheckedUnitEffectOperationPlan::ScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. } => {
                            "state graph: operation custody: scalar call"
                        }
                        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                        | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                            ..
                        } => "state graph: operation custody: selected operator call",
                        CheckedUnitEffectOperationPlan::EstablishReference { .. }
                        | CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {
                            "state graph: operation custody: reference custody"
                        }
                        CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                        | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                        | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. } => {
                            "state graph: operation custody: scalar array or primitive local"
                        }
                        CheckedUnitEffectOperationPlan::PortWrite { .. } => {
                            "state graph: operation custody: port write"
                        }
                        CheckedUnitEffectOperationPlan::Complete { .. } => {
                            "state graph: operation custody: completion"
                        }
                        _ => "state graph: operation custody: other operation",
                    });
                    return None;
                }
            }
        }
        trace.phase("state graph: terminator");
        let ordinal = u32::try_from(terminator_index).ok()?;
        let edge = |transition, edge_ordinal, kind| {
            successor(
                program,
                facts,
                machine,
                state_index,
                &signatures,
                &operations,
                transition,
                edge_ordinal,
                kind,
                trace,
            )
        };
        let terminator = if let Some(terminator) = closed_sum::build(
            program,
            facts,
            machine,
            state_index,
            &signatures,
            &operations,
            terminator_index,
            trace,
        ) {
            terminator
        } else if let Some(terminator) = returns::guarded(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            &state_entry_claims[state_index],
            &operations,
            terminator_index,
        ) {
            terminator
        } else {
            match &statements[terminator_index..] {
                [] if result == checked_trees::CheckedControlResultPlan::Unit => {
                    trace.phase("state graph: terminator: unit tail cleanup");
                    if facts.flow.ownership.permissions.iter().any(|(_, event)| {
                        event.machine_symbol == machine.symbol
                            && event.state_symbol == state.symbol
                            && event.kind == PermissionEventKind::AffineDrop
                            && !event.segments.is_empty()
                    }) {
                        return None;
                    }
                    return_unit_affine_discards(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        structural,
                        program.state_parameters(state),
                        &operations,
                        &[],
                    )?;
                    CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                }
                [StatementNode::Expression(expression)]
                    if result != checked_trees::CheckedControlResultPlan::Unit =>
                {
                    trace.phase("state graph: terminator: return expression");
                    if let Some(result) = sequence.structural_result.clone() {
                        CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result }
                    } else {
                        CheckedComposedUnitControlTerminatorPlan::ReturnCase {
                            result: returns::constructor(
                                program,
                                facts,
                                state,
                                ordinal,
                                *expression,
                            )?,
                        }
                    }
                }
                [StatementNode::Transition(transition)]
                    if transition.guard == TransitionGuardNode::Always =>
                {
                    trace.phase("state graph: terminator: jump successor");
                    CheckedComposedUnitControlTerminatorPlan::Jump {
                        successor: edge(transition, ordinal, SuccessorEdge::Jump)?,
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
                    trace
                        .phase("state graph: terminator: conditional successors: guard expression");
                    let guard = facts
                        .values
                        .scalar_expressions
                        .expression_at(state.symbol, ordinal, CheckedScalarExpressionRole::Guard)?
                        .clone();
                    if !matches!(guard, CheckedScalarExpression::Boolean(_)) {
                        trace.phase("state graph: terminator: conditional successors: guard type");
                        return None;
                    }
                    CheckedComposedUnitControlTerminatorPlan::Conditional {
                        guard,
                        when_true: edge(when_true, ordinal, SuccessorEdge::Conditional)?,
                        when_false: edge(
                            when_false,
                            ordinal.checked_add(1)?,
                            SuccessorEdge::Conditional,
                        )?,
                    }
                }
                _ => {
                    // Name the tail shape the general route lacks: the arms
                    // above admit an empty unit tail, one return expression,
                    // one unconditional jump, and an exact when/else pair.
                    trace.phase(match &statements[terminator_index..] {
                        [] => "state graph: terminator: unsupported tail: missing return value",
                        [StatementNode::Expression(_)] => {
                            "state graph: terminator: unsupported tail: expression statement with unit result"
                        }
                        [StatementNode::Transition(transition)] => match transition.exit {
                            TransitionExit::Ordinary => {
                                "state graph: terminator: unsupported tail: single guarded transition"
                            }
                            TransitionExit::Crash(_) => {
                                "state graph: terminator: unsupported tail: single guarded crash exit"
                            }
                        },
                        [StatementNode::Transition(first), StatementNode::Transition(_)]
                            if first.guard == TransitionGuardNode::Always =>
                        {
                            "state graph: terminator: unsupported tail: jump followed by a transition"
                        }
                        [StatementNode::Transition(_), StatementNode::Transition(_)] => {
                            "state graph: terminator: unsupported tail: guarded pair without exact false fallback"
                        }
                        tail @ [
                            StatementNode::Transition(_),
                            StatementNode::Transition(_),
                            StatementNode::Transition(_),
                            ..,
                        ] if tail
                            .iter()
                            .all(|statement| matches!(statement, StatementNode::Transition(_))) =>
                        {
                            "state graph: terminator: unsupported tail: transition chain"
                        }
                        [StatementNode::Transition(_), ..] => {
                            "state graph: terminator: unsupported tail: transition followed by statements"
                        }
                        [StatementNode::Expression(_), ..] => {
                            "state graph: terminator: unsupported tail: expression followed by statements"
                        }
                        _ => "state graph: terminator: unsupported tail: other statement tail",
                    });
                    return None;
                }
            }
        };
        trace.phase("state graph: exit result locals");
        let disposable_locals = if matches!(
            terminator,
            CheckedComposedUnitControlTerminatorPlan::Jump { .. }
                | CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
                | CheckedComposedUnitControlTerminatorPlan::ReturnUnit
        ) {
            crate::execution::terminal_cleanup::state_exit_result_locals(
                program, facts, machine, state,
            )?
        } else {
            Vec::new()
        };
        trace.phase("state graph: result custody accounting");
        for (producer_index, operation) in operations.iter().enumerate() {
            let result = match operation {
                CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => result,
                _ => continue,
            };
            let transferred = |edge: &CheckedStructuralControlSuccessorPlan| {
                edge.transfers.iter().filter(|transfer| matches!(transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)).count() == 1
            };
            // An owned selection moved each candidate source's custody into
            // the join's residual parameters, which die at the same authored
            // exit. The receipt, not a transfer or disposal row on the dead
            // source place, accounts for that consumption on an ordinary
            // successor; other exits still need their own residual evidence.
            let selection_residual_source = matches!(
                terminator,
                CheckedComposedUnitControlTerminatorPlan::Jump { .. }
                    | CheckedComposedUnitControlTerminatorPlan::Conditional { .. }
            ) && result.multiplicity == Multiplicity::Affine
                && facts
                    .flow
                    .ownership
                    .owned_selections
                    .iter()
                    .any(|(_, receipt)| {
                        receipt.machine == machine.symbol
                            && receipt.state == state.symbol
                            && receipt.death == PermissionEventSource::StateExit
                            && facts
                                .flow
                                .ownership
                                .selection_sources
                                .span_or_empty(receipt.sources)
                                .iter()
                                .any(|source| {
                                    source.statement_ordinal == result.statement_index
                                        && matches!(
                                            statements.get(result.statement_index as usize),
                                            Some(StatementNode::LocalData(local))
                                                if local.symbol == source.symbol
                                        )
                                })
                    });
            let consumed = match &terminator {
                CheckedComposedUnitControlTerminatorPlan::Guarded { .. } =>
                    result.multiplicity == Multiplicity::Unrestricted,
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit =>
                    local_results::permits_disposal(program, state, result, &[], &disposable_locals),
                CheckedComposedUnitControlTerminatorPlan::Jump { successor } => transferred(successor)
                    || local_results::permits_disposal(program, state, result, &[successor], &disposable_locals),
                CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, when_false, .. } => (transferred(when_true) && transferred(when_false))
                    || local_results::permits_disposal(program, state, result, &[when_true, when_false], &disposable_locals),
                CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } => matches!(subject.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal) && cases.iter().all(|case| !case.successor.transfers.iter().any(|transfer| matches!(transfer.source, checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal))),
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } => matches!(returned.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal),
                _ => false,
            } || selection_residual_source;
            // A linear result moved into an ordinary call is no longer owed by the
            // state's terminator. Count its exact whole owned uses in the
            // completed sequence; returning it as well would duplicate custody.
            // Affine call-result consumers retain their ordinary statement
            // owner until graph admission can replay their cleanup partition.
            let call_transfers = operations[producer_index + 1..]
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::CallUnit {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::ScalarCall {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryCall {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                        structural_arguments,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        structural_arguments,
                        ..
                    } => Some(structural_arguments),
                    _ => None,
                })
                .flatten()
                .filter(|argument| {
                    result.multiplicity == Multiplicity::Linear
                        && argument.source_structural_result_binding_ordinal()
                            == Some(result.binding_ordinal)
                        && argument.access == CheckedStructuralAccess::Owned
                        && argument.path.is_empty()
                })
                .count();
            // An in-sequence continuation cleanup owns an affine result whose
            // authored statement discarded it. Count that exact owner as the
            // consumption alongside terminator disposal and call transfer.
            let cleanup_discards = operations[producer_index + 1..]
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::CallContinuationCleanup {
                        affine_discards,
                        ..
                    } => Some(affine_discards),
                    _ => None,
                })
                .flatten()
                .filter(|discard| {
                    matches!(
                        discard.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal
                        } if binding_ordinal == result.binding_ordinal
                    )
                })
                .count();
            if usize::from(consumed) + call_transfers + cleanup_discards != 1 {
                return None;
            }
        }
        planned.push(CheckedComposedUnitControlStatePlan {
            state: state.symbol,
            structural_parameters: structural.clone(),
            scalar_parameters: scalar.clone(),
            erased_scalar_parameters:
                crate::execution::terminal_unit::types::erased_scalar_parameter_plans(
                    program, state,
                )?,
            requires: state_requires[state_index].clone(),
            entry_claims: state_entry_claims[state_index].clone(),
            bindings,
            binding_initializers,
            operations,
            terminator,
        });
    }
    // Retain only this machine's direct provider-field calls. Each ordinary
    // callee owns its own attachment requirements, even through a receiver loan.
    trace.phase("state graph: provider attachment requirements");
    trace.state(None);
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
    trace.phase("state graph: finish");
    let mut plan = composed_control::finish_state_graph(
        facts,
        machine,
        attachment,
        provider_attachment_requirements,
        planned,
    )?;
    plan.natural_ranks = natural_ranks;
    plan.result = result;
    Some(plan)
}

/// The pure scalar initializers ahead of the first computation, tracing the
/// binding and the guard (binding value, destination, statement, bound
/// expression, custody agreement, initializer type, short-circuit boolean)
/// that declined it.
fn prefix_initializers(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    bindings: &[CheckedScalarBinding],
    trace: &control::LocalConstructionTrace,
) -> Option<Vec<CheckedScalarExpression>> {
    use checked_trees::CheckedScalarBindingDestination;

    let statements = program.statement_table.statements(state.statement_nodes);
    let mut immutable_ordinal = 0u32;
    bindings
        .iter()
        .map(|binding| {
            let mark = |phase| {
                trace.phase(phase);
                trace.statement(Some(binding.statement_ordinal));
            };
            mark("state graph: prefix initializers: binding value");
            if binding.value != CheckedScalarBindingValue::Expression {
                return None;
            }
            mark("state graph: prefix initializers: binding destination");
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
            mark("state graph: prefix initializers: binding statement");
            let expression = match statements.get(binding.statement_ordinal as usize)? {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Assignment(assignment) => assignment.value,
                _ => return None,
            };
            mark("state graph: prefix initializers: bound expression");
            let (custody, initializer) = facts.values.scalar_expressions.bound_expression_at(
                state.symbol,
                binding.statement_ordinal,
                role,
            )?;
            mark("state graph: prefix initializers: custody agreement");
            if custody.expression != expression || custody.destination != destination {
                return None;
            }
            mark("state graph: prefix initializers: initializer type");
            if crate::values::scalar_expression_type(initializer) != Some(binding.primitive_type) {
                return None;
            }
            mark("state graph: prefix initializers: short-circuit boolean");
            if matches!(initializer, CheckedScalarExpression::Boolean(boolean)
                if checked_boolean_contains_short_circuit(boolean))
            {
                return None;
            }
            Some(initializer.clone())
        })
        .collect()
}

fn whole_shared_argument(argument: &CheckedUnitStructuralArgumentPlan) -> bool {
    argument.path.is_empty()
        && argument.access == CheckedStructuralAccess::SharedBorrow
        && (argument.source_parameter_index().is_some()
            || matches!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
            ))
}

type Signature = (
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
);

/// The terminator family an edge is built for. The trace names the guard
/// that declined the edge beneath that family's phase; the edge's statement
/// ordinal separates the true and false successors of one conditional.
#[derive(Clone, Copy)]
pub(super) enum SuccessorEdge {
    Jump,
    Conditional,
    ClosedCase,
}

#[derive(Clone, Copy)]
enum SuccessorGuard {
    TransitionForm,
    TargetState,
    ArgumentCount,
    StructuralArgument,
    ReceiverTransfer,
    SubsliceTransfer,
    ResultTransfer,
    ParameterTransfer,
    ScalarArguments,
    ErasedArguments,
    EdgeCleanup,
}

impl SuccessorEdge {
    fn phase(self, guard: SuccessorGuard) -> &'static str {
        match (self, guard) {
            (Self::Jump, SuccessorGuard::TransitionForm) => {
                "state graph: terminator: jump successor: transition form"
            }
            (Self::Jump, SuccessorGuard::TargetState) => {
                "state graph: terminator: jump successor: target state"
            }
            (Self::Jump, SuccessorGuard::ArgumentCount) => {
                "state graph: terminator: jump successor: argument count"
            }
            (Self::Jump, SuccessorGuard::StructuralArgument) => {
                "state graph: terminator: jump successor: structural argument"
            }
            (Self::Jump, SuccessorGuard::ReceiverTransfer) => {
                "state graph: terminator: jump successor: receiver transfer"
            }
            (Self::Jump, SuccessorGuard::SubsliceTransfer) => {
                "state graph: terminator: jump successor: byte-subslice transfer"
            }
            (Self::Jump, SuccessorGuard::ResultTransfer) => {
                "state graph: terminator: jump successor: result-local transfer"
            }
            (Self::Jump, SuccessorGuard::ParameterTransfer) => {
                "state graph: terminator: jump successor: parameter transfer"
            }
            (Self::Jump, SuccessorGuard::ScalarArguments) => {
                "state graph: terminator: jump successor: scalar arguments"
            }
            (Self::Jump, SuccessorGuard::ErasedArguments) => {
                "state graph: terminator: jump successor: erased arguments"
            }
            (Self::Jump, SuccessorGuard::EdgeCleanup) => {
                "state graph: terminator: jump successor: edge cleanup"
            }
            (Self::Conditional, SuccessorGuard::TransitionForm) => {
                "state graph: terminator: conditional successors: transition form"
            }
            (Self::Conditional, SuccessorGuard::TargetState) => {
                "state graph: terminator: conditional successors: target state"
            }
            (Self::Conditional, SuccessorGuard::ArgumentCount) => {
                "state graph: terminator: conditional successors: argument count"
            }
            (Self::Conditional, SuccessorGuard::StructuralArgument) => {
                "state graph: terminator: conditional successors: structural argument"
            }
            (Self::Conditional, SuccessorGuard::ReceiverTransfer) => {
                "state graph: terminator: conditional successors: receiver transfer"
            }
            (Self::Conditional, SuccessorGuard::SubsliceTransfer) => {
                "state graph: terminator: conditional successors: byte-subslice transfer"
            }
            (Self::Conditional, SuccessorGuard::ResultTransfer) => {
                "state graph: terminator: conditional successors: result-local transfer"
            }
            (Self::Conditional, SuccessorGuard::ParameterTransfer) => {
                "state graph: terminator: conditional successors: parameter transfer"
            }
            (Self::Conditional, SuccessorGuard::ScalarArguments) => {
                "state graph: terminator: conditional successors: scalar arguments"
            }
            (Self::Conditional, SuccessorGuard::ErasedArguments) => {
                "state graph: terminator: conditional successors: erased arguments"
            }
            (Self::Conditional, SuccessorGuard::EdgeCleanup) => {
                "state graph: terminator: conditional successors: edge cleanup"
            }
            (Self::ClosedCase, SuccessorGuard::TransitionForm) => {
                "state graph: terminator: closed-sum case successor: transition form"
            }
            (Self::ClosedCase, SuccessorGuard::TargetState) => {
                "state graph: terminator: closed-sum case successor: target state"
            }
            (Self::ClosedCase, SuccessorGuard::ArgumentCount) => {
                "state graph: terminator: closed-sum case successor: argument count"
            }
            (Self::ClosedCase, SuccessorGuard::StructuralArgument) => {
                "state graph: terminator: closed-sum case successor: structural argument"
            }
            (Self::ClosedCase, SuccessorGuard::ReceiverTransfer) => {
                "state graph: terminator: closed-sum case successor: receiver transfer"
            }
            (Self::ClosedCase, SuccessorGuard::SubsliceTransfer) => {
                "state graph: terminator: closed-sum case successor: byte-subslice transfer"
            }
            (Self::ClosedCase, SuccessorGuard::ResultTransfer) => {
                "state graph: terminator: closed-sum case successor: result-local transfer"
            }
            (Self::ClosedCase, SuccessorGuard::ParameterTransfer) => {
                "state graph: terminator: closed-sum case successor: parameter transfer"
            }
            (Self::ClosedCase, SuccessorGuard::ScalarArguments) => {
                "state graph: terminator: closed-sum case successor: scalar arguments"
            }
            (Self::ClosedCase, SuccessorGuard::ErasedArguments) => {
                "state graph: terminator: closed-sum case successor: erased arguments"
            }
            (Self::ClosedCase, SuccessorGuard::EdgeCleanup) => {
                "state graph: terminator: closed-sum case successor: edge cleanup"
            }
        }
    }
}

fn successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    source_index: usize,
    signatures: &[Signature],
    operations: &[CheckedUnitEffectOperationPlan],
    transition: &typed_trees::statement::TableTransition,
    ordinal: u32,
    edge: SuccessorEdge,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let successor = successor_bindings(
        program,
        facts,
        machine,
        source_index,
        signatures,
        operations,
        transition,
        ordinal,
        &[],
        edge,
        trace,
    )?;
    trace.phase(edge.phase(SuccessorGuard::EdgeCleanup));
    trace.statement(Some(ordinal));
    let source = &program.machine_states(machine)[source_index];
    let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
        machine.symbol,
        source.symbol,
        ordinal,
    )?;
    if cleanup.target_state != successor.target_state
        || !cleanup
            .trivial_affine_discard_parameter_positions
            .is_empty()
    {
        return None;
    }
    Some(successor)
}

// Operand identity is shared by ordinary and closed-case edges. The caller
// separately admits either whole-parameter cleanup or exact result-local
// cleanup; constructing bindings establishes neither cleanup contract.
fn successor_bindings(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    source_index: usize,
    signatures: &[Signature],
    operations: &[CheckedUnitEffectOperationPlan],
    transition: &typed_trees::statement::TableTransition,
    ordinal: u32,
    payload_parameters: &[u32],
    edge: SuccessorEdge,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let mark = |guard| {
        trace.phase(edge.phase(guard));
        trace.statement(Some(ordinal));
    };
    mark(SuccessorGuard::TransitionForm);
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    mark(SuccessorGuard::TargetState);
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
    mark(SuccessorGuard::ArgumentCount);
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
        let mut argument = argument_at(position)?;
        // `value as T` on a structural type ascribes the same value, so the
        // transferred place is still the source parameter root. Scalar
        // (primitive-target) casts stay opaque: they compute a new value.
        while let ExpressionNode::Cast(cast) = program.expression_table.expression(argument)
            && program.primitive_type_reference(cast.target_type).is_none()
        {
            argument = cast.value;
        }
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
                mark(SuccessorGuard::ReceiverTransfer);
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
            mark(SuccessorGuard::StructuralArgument);
            let expression = argument_at(target.position)?;
            if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
                && let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
            {
                mark(SuccessorGuard::SubsliceTransfer);
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
            if target.access == CheckedStructuralAccess::Owned {
                mark(SuccessorGuard::ResultTransfer);
                let place = crate::flow::canonical_place_from_expression_in_state(program, source.symbol, ordinal as usize, expression)?;
                if place.segments.is_empty() {
                    let mut matches = operations.iter().filter_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralCall { result, discard_result_on_return: false, .. }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, discard_result_on_return: false, .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, discard_result_on_return: false, .. } => Some(result),
                        _ => None,
                    }).filter(|result| result.statement_index < ordinal && matches!(program.statement_table.statements(source.statement_nodes).get(result.statement_index as usize), Some(StatementNode::LocalData(local)) if place.root == facts::PlaceRoot::Symbol(local.symbol)));
                    if let Some(result) = matches.next() {
                        if matches.next().is_some() { return None; }
                        if result.type_identity != target.type_identity || result.multiplicity != target.multiplicity { return None; }
                        return Some(CheckedStructuralControlTransferPlan {
                            source: checked_trees::CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal: result.binding_ordinal },
                            target_parameter_index: u32::try_from(target_index).ok()?,
                        });
                    }
                }
            }
            mark(SuccessorGuard::ParameterTransfer);
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
    mark(SuccessorGuard::ScalarArguments);
    let scalar_arguments = target_scalar
        .iter()
        .enumerate()
        .filter(|(index, _)| !payload_parameters.contains(&(*index as u32)))
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
    mark(SuccessorGuard::ErasedArguments);
    // Erased formals read the same retained transition expressions but index
    // the proof-only roster densely; their lowered terms are emitted against
    // the source state's erased namespace.
    let erased_arguments =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, target)?
            .iter()
            .enumerate()
            .map(|(erased_index, target)| {
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
                    || crate::values::scalar_expression_type(expression)
                        != Some(target.primitive_type)
                {
                    return None;
                }
                Some(CheckedStructuralScalarArgumentPlan {
                    argument_ordinal: target.source_position,
                    source: checked_trees::CheckedStructuralScalarArgumentSourcePlan::Expression,
                    target_scalar_parameter_index: u32::try_from(erased_index).ok()?,
                    primitive_type: target.primitive_type,
                })
            })
            .collect::<Option<Vec<_>>>()?;
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: target.symbol,
        transfers,
        scalar_arguments,
        erased_arguments,
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
