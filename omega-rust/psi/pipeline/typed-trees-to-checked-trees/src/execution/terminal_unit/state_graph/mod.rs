//! State-local calls and explicit successor bindings, independent of graph shape.
use super::{
    CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedGuardedJumpPlan, CheckedScalarBinding,
    CheckedScalarBindingValue, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralScalarArgumentPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
    Multiplicity, PermissionEventKind, PermissionEventSource, PrimitiveType, StatementNode,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, TypeReferenceNode, TypedTrees,
    calls,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_signature_traced, structural_scalar_signature_traced,
};
use crate::execution::terminal_unit::providers::checked_composed_provider_attachment_requirements;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::types::{
    ShapeCollector, machine_binders, return_unit_affine_discards, state_flow,
};
use crate::execution::terminal_unit::types::{borrowed_slice_view_element, byte_sequence_carrier};
use crate::execution::terminal_unit::{composed_control, control};

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
    if let checked_trees::CheckedControlResultPlan::Scalar { .. } = result {
        // A single-state body completes its scalar through the ordinary
        // sequence, and a machine another scalar producer already owns keeps
        // that owner: this route takes only the multi-state bodies no other
        // producer describes, so no callee gains two competing bodies.
        trace.phase("state graph: result signature: scalar owner precedence");
        if states.len() < 2
            || facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine.symbol)
                .is_some()
            || scalar_callees
                .structural_returns
                .for_machine(machine.symbol)
                .is_some()
            || scalar_callees
                .boundary_returns
                .machines
                .iter()
                .any(|plan| plan.machine == machine.symbol)
        {
            return None;
        }
        // Emission publishes no result guarantee, so a body whose contract
        // promises one about its result stays unadmitted rather than losing it.
        trace.phase("state graph: result signature: scalar result guarantee");
        if program
            .machine_contracts(machine)
            .iter()
            .chain(
                states
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .any(|contract| {
                !matches!(
                    contract.kind,
                    super::SignatureContractKind::Requires
                        | super::SignatureContractKind::Crashes { .. }
                )
            })
        {
            return None;
        }
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
        // Selected operator and FMA applications are not yet routed to the
        // composed states, so the sequence sees none and rewrites no
        // parameter identity; the signature stays the one recorded above.
        let mut sequence_structural = structural.clone();
        let sequence = control::statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            &mut sequence_structural,
            scalar,
            &state_entry_claims[state_index],
            &calls,
            &[],
            binding_count,
            control::statement_sequence::SelectedApplications {
                operators: &[],
                ieee_float_fma: &[],
            },
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
                    ..
                }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                    structural_arguments,
                    ..
                } if structural_arguments.iter().all(|argument| {
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
                            // The ordinary call sequencer has already joined
                            // owned arguments to their exact Consume events
                            // and completion receipts. Graph topology does not
                            // change that operation's custody.
                            || (argument.source_parameter_index().is_some()
                                && argument.path.is_empty()
                                && argument.access == CheckedStructuralAccess::Owned)
                            || call_consumes_result_argument(
                                argument,
                                &operations[..operation_index],
                            )
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
                            // The ordinary call sequencer has already joined
                            // owned arguments to their exact Consume events and
                            // completion receipts, exactly as it does for a
                            // boundary call. Graph topology does not change that
                            // operation's custody, so the same whole-parameter
                            // owned argument is admitted here.
                            || (argument.source_parameter_index().is_some()
                                && argument.path.is_empty()
                                && argument.access == CheckedStructuralAccess::Owned)
                            || call_consumes_result_argument(
                                argument,
                                &operations[..operation_index],
                            )
                    }) => {}
                CheckedUnitEffectOperationPlan::ScalarCall {
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
                            // The ordinary call sequencer has already joined
                            // owned arguments to their exact Consume events and
                            // completion receipts, exactly as it does for a
                            // boundary call. Graph topology does not change that
                            // operation's custody, so the same whole-parameter
                            // owned argument is admitted here.
                            || (argument.source_parameter_index().is_some()
                                && argument.path.is_empty()
                                && argument.access == CheckedStructuralAccess::Owned)
                            || call_consumes_result_argument(
                                argument,
                                &operations[..operation_index],
                            )
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
                // A view-subslice local owns nothing: its shared view ends
                // with the loan, on every selected edge alike.
                | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. } => {}
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                // The ordinary sequencer has already rejoined the primitive
                // destination, RHS, and complete write frame. Crossing a state
                // edge does not turn that non-observing write into a new family.
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_) => {}
                // The window pair the sequencer itself joined — a move-out
                // binding the displaced field value and the exact restoration
                // into the opened hole — carries no new custody across the
                // selected edges: its owned source, borrowed destination
                // window, and consumed result binding all rejoined at mint.
                CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. }
                    if source.source_parameter_index().is_some()
                        && source.access == CheckedStructuralAccess::Owned => {}
                CheckedUnitEffectOperationPlan::StoreStructuralField {
                    destination,
                    value,
                    ..
                } if destination.source_parameter_index().is_some()
                    && destination.access == CheckedStructuralAccess::Owned
                    && matches!(
                        value.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            ..
                        }
                    )
                    && value.access == CheckedStructuralAccess::Owned => {}
                // A result may die on its producing statement's continuation.
                // The cleanup shares the producer's coordinate rather than
                // consuming a new authored statement; sequenced stores may sit
                // between the producer -- the statement's call, or the
                // establishment of a construction replacing a field -- and the
                // discard that retires a displaced binding.
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
                    }) && operations[..operation_index]
                        .iter()
                        .rev()
                        .find(|operation| {
                            matches!(
                                operation,
                                CheckedUnitEffectOperationPlan::StructuralCall { .. }
                                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                                        ..
                                    }
                                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                        ..
                                    }
                            )
                        })
                        .is_some_and(|producer| match producer {
                            CheckedUnitEffectOperationPlan::StructuralCall {
                                coordinate: call,
                                ..
                            }
                            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                                coordinate: call,
                                ..
                            } => call == coordinate,
                            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                result,
                                ..
                            } => {
                                result.statement_index == coordinate.statement_index
                                    && coordinate.call_ordinal == 0
                            }
                            _ => false,
                        }) => {}
                _ => {
                    // Name the operation family whose custody the selected
                    // edges cannot yet carry; admitted families never reach
                    // this arm.
                    trace.phase(match operation {
                        CheckedUnitEffectOperationPlan::BoundaryCall { .. } => {
                            "state graph: operation custody: boundary call arguments"
                        }
                        CheckedUnitEffectOperationPlan::CallUnit {
                            structural_arguments,
                            claim_transfers,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::ScalarCall {
                            structural_arguments,
                            claim_transfers,
                            ..
                        } => call_custody_refusal(
                            structural_arguments,
                            claim_transfers,
                            &operations[..operation_index],
                        ),
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                            "state graph: operation custody: discarded structural result"
                        }
                        CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. } => {
                            "state graph: operation custody: continuation cleanup owner"
                        }
                        CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. } => {
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
        let edge =
            |operations: &[CheckedUnitEffectOperationPlan], transition, edge_ordinal, kind| {
                successor(
                    program,
                    facts,
                    machine,
                    state_index,
                    &signatures,
                    operations,
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
        } else if let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = result
            && let Some(completion) = returns::scalar_completion(
                program,
                facts,
                machine,
                state,
                terminator_index,
                primitive_type,
                sequence.scalar_result.as_ref(),
                trace,
            )
        {
            trace.phase("state graph: terminator: scalar return cleanup");
            return_cleanup_is_whole(
                program,
                facts,
                machine,
                state,
                structural,
                &operations,
                shapes,
            )?;
            CheckedComposedUnitControlTerminatorPlan::ReturnScalar { completion }
        } else if let Some((terminator, operand_calls)) = returns::guarded(
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
            trace,
        ) {
            operations.extend(operand_calls);
            terminator
        } else {
            match &statements[terminator_index..] {
                [] if result == checked_trees::CheckedControlResultPlan::Unit => {
                    trace.phase("state graph: terminator: unit tail cleanup");
                    return_cleanup_is_whole(
                        program,
                        facts,
                        machine,
                        state,
                        structural,
                        &operations,
                        shapes,
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
                    if matches!(transition.exit, TransitionExit::Crash(_)) =>
                {
                    trace.phase("state graph: terminator: crash exit");
                    CheckedComposedUnitControlTerminatorPlan::Crash {
                        statement_ordinal: ordinal,
                    }
                }
                // Typing already lowered a run-closing constant-true arm
                // (`transition true { true -> done(v) }`) to `Always`.
                [StatementNode::Transition(transition)]
                    if transition.guard == TransitionGuardNode::Always =>
                {
                    trace.phase("state graph: terminator: jump successor");
                    CheckedComposedUnitControlTerminatorPlan::Jump {
                        successor: edge(&operations, transition, ordinal, SuccessorEdge::Jump)?,
                    }
                }
                [
                    StatementNode::Transition(when_true),
                    StatementNode::Transition(when_false),
                ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                    && (when_false.guard == TransitionGuardNode::Always
                        || crate::execution::guard_complement::complementary(
                            program,
                            &facts.values.scalar_expressions,
                            state,
                            ordinal,
                        )) =>
                {
                    trace
                        .phase("state graph: terminator: conditional successors: guard expression");
                    let guard = retained_guard(facts, machine.symbol, state.symbol, ordinal)?;
                    if !guard_is_boolean(facts, &guard) {
                        trace.phase("state graph: terminator: conditional successors: guard type");
                        return None;
                    }
                    // An authored `(expression)` arm returns the established
                    // value instead of transferring to a named state; a named
                    // arm keeps the ordinary successor custody plan.
                    let mut return_count = returns::next_result_ordinal(&operations)?;
                    let mut branch = |transition: &typed_trees::statement::TableTransition,
                                      edge_ordinal: u32|
                     -> Option<
                        Result<
                            CheckedStructuralControlSuccessorPlan,
                            (
                                checked_trees::CheckedConditionalReturnArm,
                                Vec<CheckedUnitEffectOperationPlan>,
                            ),
                        >,
                    > {
                        if let TransitionTargetNode::Value(expression) =
                            program.statement_table.transition_target(transition.target)
                        {
                            if transition.exit != TransitionExit::Ordinary
                                || transition.continuation.is_valid()
                            {
                                trace.phase(
                                    SuccessorEdge::Conditional.phase(SuccessorGuard::TargetState),
                                );
                                return None;
                            }
                            trace.phase(
                                "state graph: terminator: conditional successors: return arm value",
                            );
                            trace.statement(Some(edge_ordinal));
                            // A scalar result is the value checking retained
                            // under the arm's `Return` role; the arm alone
                            // evaluates it.
                            if let checked_trees::CheckedControlResultPlan::Scalar {
                                primitive_type,
                            } = result
                            {
                                let role = CheckedScalarExpressionRole::Return;
                                let retained = facts
                                    .values
                                    .scalar_expressions
                                    .expression_at(state.symbol, edge_ordinal, role)
                                    .is_some()
                                    || facts
                                        .values
                                        .scalar_computations
                                        .root_at(state.symbol, edge_ordinal, role)
                                        .is_some_and(|root| root.machine == machine.symbol);
                                return retained.then_some(Err((
                                    checked_trees::CheckedConditionalReturnArm::Scalar {
                                        statement_ordinal: edge_ordinal,
                                        primitive_type,
                                    },
                                    Vec::new(),
                                )));
                            }
                            return returns::return_value_operation(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                &state_entry_claims[state_index],
                                &mut return_count,
                                edge_ordinal,
                                *expression,
                                trace,
                            )
                            .map(|(operation, operand_calls)| {
                                Err((
                                    checked_trees::CheckedConditionalReturnArm::Structural(
                                        operation,
                                    ),
                                    operand_calls,
                                ))
                            });
                        }
                        successor(
                            program,
                            facts,
                            machine,
                            state_index,
                            &signatures,
                            &operations,
                            transition,
                            edge_ordinal,
                            SuccessorEdge::Conditional,
                            trace,
                        )
                        .map(Ok)
                    };
                    match (
                        branch(when_true, ordinal)?,
                        branch(when_false, ordinal.checked_add(1)?)?,
                    ) {
                        (Ok(when_true), Ok(when_false)) => {
                            CheckedComposedUnitControlTerminatorPlan::Conditional {
                                guard,
                                when_true,
                                when_false,
                            }
                        }
                        (Ok(jump), Err((return_arm, operand_calls))) => {
                            operations.extend(operand_calls);
                            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                                guard,
                                jump,
                                return_arm,
                                return_when_true: false,
                            }
                        }
                        (Err((return_arm, operand_calls)), Ok(jump)) => {
                            operations.extend(operand_calls);
                            CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                                guard,
                                jump,
                                return_arm,
                                return_when_true: true,
                            }
                        }
                        // Both arms `(expression)` targets check as Guarded.
                        (Err(_), Err(_)) => return None,
                    }
                }
                tail @ [
                    StatementNode::Transition(_),
                    StatementNode::Transition(_),
                    StatementNode::Transition(_),
                    ..,
                ] if tail
                    .iter()
                    .all(|statement| matches!(statement, StatementNode::Transition(_)))
                    && tail[..tail.len() - 1].iter().all(|statement| {
                        matches!(statement, StatementNode::Transition(transition)
                            if matches!(transition.guard, TransitionGuardNode::When(_)))
                    })
                    && matches!(tail.last(), Some(StatementNode::Transition(transition))
                        if transition.guard == TransitionGuardNode::Always) =>
                {
                    trace.phase("state graph: terminator: guarded jump successors: roster");
                    let retained = facts
                        .flow
                        .terminal_scalar_graphs
                        .guarded_tails
                        .iter()
                        .filter(|tail| tail.state == state.symbol)
                        .collect::<Vec<_>>();
                    let [retained] = retained.as_slice() else {
                        return None;
                    };
                    let exits = facts
                        .flow
                        .terminal_scalar_graphs
                        .guarded_exits
                        .span(retained.arms)?;
                    // The shared scalar roster owns this tail's guard order,
                    // coverage and selected destinations; the composed edges
                    // must agree with it exactly.
                    if exits.len() != tail.len() - 1 {
                        return None;
                    }
                    let mut arms = Vec::with_capacity(exits.len());
                    for (index, (statement, exit)) in
                        tail[..tail.len() - 1].iter().zip(exits.iter()).enumerate()
                    {
                        let StatementNode::Transition(transition) = statement else {
                            return None;
                        };
                        let arm_ordinal = ordinal.checked_add(u32::try_from(index).ok()?)?;
                        let checked_trees::CheckedScalarBranchDestination::Jump(selected) =
                            &exit.destination
                        else {
                            return None;
                        };
                        if exit.guard_statement_ordinal != arm_ordinal
                            || selected.statement_ordinal != arm_ordinal
                        {
                            return None;
                        }
                        trace.phase(
                            "state graph: terminator: guarded jump successors: guard expression",
                        );
                        trace.statement(Some(arm_ordinal));
                        let guard =
                            retained_guard(facts, machine.symbol, state.symbol, arm_ordinal)?;
                        if !guard_is_boolean(facts, &guard) {
                            return None;
                        }
                        let successor = edge(
                            &operations,
                            transition,
                            arm_ordinal,
                            SuccessorEdge::GuardedJump,
                        )?;
                        if successor.target_state != selected.target {
                            return None;
                        }
                        arms.push(CheckedGuardedJumpPlan { guard, successor });
                    }
                    let Some(StatementNode::Transition(fallback_transition)) = tail.last() else {
                        return None;
                    };
                    let fallback_ordinal = ordinal.checked_add(u32::try_from(exits.len()).ok()?)?;
                    let Some(checked_trees::CheckedScalarBranchDestination::Jump(selected)) =
                        &retained.fallback
                    else {
                        return None;
                    };
                    if selected.statement_ordinal != fallback_ordinal {
                        return None;
                    }
                    let fallback = edge(
                        &operations,
                        fallback_transition,
                        fallback_ordinal,
                        SuccessorEdge::GuardedJump,
                    )?;
                    if fallback.target_state != selected.target {
                        return None;
                    }
                    CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback }
                }
                _ => {
                    // Name the tail shape the general route lacks: the arms
                    // above admit an empty unit tail, one return expression,
                    // one unconditional jump, an exact when/else pair, and an
                    // ordered guarded chain ending in its authored `_` arm.
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
                | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
                | CheckedComposedUnitControlTerminatorPlan::GuardedJumps { .. }
                | CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                | CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
        ) {
            crate::execution::terminal_cleanup::state_exit_result_locals(
                program, facts, machine, state,
            )?
        } else {
            Vec::new()
        };
        trace.phase("state graph: result custody accounting");
        let case_successors = match &terminator {
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } => {
                cases.iter().map(|case| &case.successor).collect::<Vec<_>>()
            }
            _ => Vec::new(),
        };
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
                    | CheckedComposedUnitControlTerminatorPlan::ConditionalReturn { .. }
                    | CheckedComposedUnitControlTerminatorPlan::GuardedJumps { .. }
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
                CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } => {
                    result.multiplicity == Multiplicity::Unrestricted
                        || return_values.iter().any(|value| {
                            value.with_value_calls().skip(1).any(|operation| {
                                terminator_call_consumes_result(operation, result.binding_ordinal)
                            })
                        })
                }
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                | CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. } => {
                    local_results::permits_disposal(program, state, result, &[], &disposable_locals)
                }
                CheckedComposedUnitControlTerminatorPlan::Jump { successor } => {
                    transferred(successor)
                        || local_results::permits_disposal(
                            program,
                            state,
                            result,
                            &[successor],
                            &disposable_locals,
                        )
                }
                CheckedComposedUnitControlTerminatorPlan::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    (transferred(when_true) && transferred(when_false))
                        || local_results::permits_disposal(
                            program,
                            state,
                            result,
                            &[when_true, when_false],
                            &disposable_locals,
                        )
                }
                CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                    jump,
                    return_arm,
                    ..
                } => {
                    // The return arm consumes its producers through the
                    // `EstablishStructuralValue` operands like Guarded; only
                    // the named edge participates in transfer custody.
                    transferred(jump)
                        || local_results::permits_disposal(
                            program,
                            state,
                            result,
                            &[jump],
                            &disposable_locals,
                        )
                        || matches!(return_arm,
                        checked_trees::CheckedConditionalReturnArm::Structural(operation)
                            if operation.with_value_calls().skip(1).any(|operation| {
                                terminator_call_consumes_result(operation, result.binding_ordinal)
                            }))
                }
                CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback } => {
                    let successors = arms
                        .iter()
                        .map(|arm| &arm.successor)
                        .chain(std::iter::once(fallback))
                        .collect::<Vec<_>>();
                    successors.iter().all(|edge| transferred(edge))
                        || local_results::permits_disposal(
                            program,
                            state,
                            result,
                            &successors,
                            &disposable_locals,
                        )
                }
                CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } => {
                    if matches!(subject.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)
                    {
                        case_successors.iter().all(|edge| !transferred(edge))
                    } else {
                        case_successors.iter().all(|edge| transferred(edge))
                            || local_results::permits_disposal(
                                program,
                                state,
                                result,
                                &case_successors,
                                &disposable_locals,
                            )
                    }
                }
                CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result: returned } => {
                    matches!(returned.source, CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } if binding_ordinal == result.binding_ordinal)
                }
                _ => false,
            } || selection_residual_source;
            // A result the call owns (`call_owns_result`) is no longer owed by
            // the state's terminator. Count its exact whole owned uses in the
            // completed sequence; returning it as well would duplicate custody.
            // Affine and unrestricted locals keep their ordinary statement
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
                    call_owns_result(operation)
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
            // A hole-restore stores a whole owned result into borrowed
            // storage; custody moved into the field, which owns it after
            // this state. Count it with the terminator exits: a disposable
            // local stored into a hole is not a second consumption. A copy
            // of an `Unrestricted` value moves into its field the same way.
            let stored = operations[producer_index + 1..].iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StoreStructuralField {
                        value, ..
                    } if value.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                        && value.access == CheckedStructuralAccess::Owned
                        && value.path.is_empty()
                )
            });
            if usize::from(consumed || stored) + call_transfers + cleanup_discards != 1 {
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
            erased_proof_parameters:
                crate::execution::terminal_unit::types::erased_proof_parameter_plans(
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
    // Persistent receivers keep their invocation place. Other structural
    // parameters retain explicit owned-value or borrowed-view edge custody.
    // Each guard marks its own phase so the omission roster names the
    // custody shape the route lacks, not just the phase. States outside the
    // entry successor closure never execute, so their custody rows cannot
    // disqualify the route — the shape check applies to live positions only.
    trace.phase("state graph: state signature: parameter custody shape");
    let Some(live_states) = CheckedComposedUnitControlStatePlan::live_mask(&planned) else {
        trace.phase("state graph: state signature: parameter custody shape: edge target missing");
        return None;
    };
    for (state_index, state) in states.iter().enumerate() {
        if !live_states[state_index] {
            continue;
        }
        trace.state(u32::try_from(state_index).ok());
        for parameter in &planned[state_index].structural_parameters {
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
                (
                    true,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow,
                ) => {}
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
                    let borrowed_named_referent = matches!(
                        program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::Reference { referee, .. }
                            if matches!(
                                program.type_reference_table.type_reference(*referee),
                                TypeReferenceNode::Named { .. }
                            )
                    );
                    if !borrowed_named_referent
                        && byte_sequence_carrier(program, reference, &[])
                            != Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
                        && borrowed_slice_view_element(program, reference, &[]).is_none()
                    {
                        trace.phase(
                            "state graph: state signature: parameter custody shape: borrowed non-view carrier",
                        );
                        return None;
                    }
                }
            }
        }
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

/// A returning exit disposes whole roots only: a partial affine drop or a
/// residual discard at the return has no Terminal return-cleanup row here.
#[allow(clippy::too_many_arguments)]
fn return_cleanup_is_whole(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
    shapes: &ShapeCollector<'_>,
) -> Option<()> {
    if facts.flow.ownership.permissions.iter().any(|(_, event)| {
        event.machine_symbol == machine.symbol
            && event.state_symbol == state.symbol
            && event.kind == PermissionEventKind::AffineDrop
            && !event.segments.is_empty()
    }) {
        return None;
    }
    let (_, residual_affine_discards, _) = return_unit_affine_discards(
        program,
        facts,
        machine.symbol,
        state.symbol,
        structural,
        program.state_parameters(state),
        operations,
        &[],
        &shapes.types,
    )?;
    residual_affine_discards.is_empty().then_some(())
}

/// The exact Boolean value one authored guard retains at its `Guard`
/// coordinate: the pure expression, or the unique computation root this
/// machine owns there (a selected comparison, a call, or any other checked
/// computation). The computation producer records a root only where no pure
/// expression exists, so the coordinate names at most one of the two forms.
fn retained_guard(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
) -> Option<checked_trees::CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::Guard;
    if let Some(expression) =
        facts
            .values
            .scalar_expressions
            .expression_at(state, statement_ordinal, role)
    {
        return Some(checked_trees::CheckedCallScalarArgument::Pure(
            expression.clone(),
        ));
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state, statement_ordinal, role)?;
    (root.machine == machine && computations.nodes.is_valid(root.root)).then_some(
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    )
}

fn guard_is_boolean(facts: &CheckFacts, guard: &checked_trees::CheckedCallScalarArgument) -> bool {
    match guard {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            matches!(expression, CheckedScalarExpression::Boolean(_))
        }
        checked_trees::CheckedCallScalarArgument::Computation(root) => {
            facts
                .values
                .scalar_computations
                .nodes
                .get(*root)
                .primitive_type
                == PrimitiveType::Bool
        }
    }
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

/// Whether an owned whole call argument moves its structural result's single
/// custody into the call. A linear result has exactly one owner, so the call
/// is it. A construction authored as the call's own argument —
/// `f(Event::Trigger)`, recorded with a `CallArgument` source — has no owner
/// but the call that reads it, so the call owns it whatever its multiplicity;
/// the Psi lowering admits exactly that join and no wider one. Every other
/// result keeps its statement owner: a local is disposed through the state's
/// exit, and counting a call as well would owe it twice. The result custody
/// accounting then requires exactly one consumer per result, so a temporary
/// the sequence also discarded or returned still refuses.
/// A call retained inside a value-returning terminator (`Guarded` arm
/// payloads, `ConditionalReturn`'s value arm) consumes its producers through
/// owned whole arguments the same way an in-sequence call does: the argument
/// names the exact result binding it moves into the call's custody.
fn terminator_call_consumes_result(
    operation: &CheckedUnitEffectOperationPlan,
    binding_ordinal: u32,
) -> bool {
    let structural_arguments = match operation {
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
        } => structural_arguments,
        _ => return false,
    };
    structural_arguments.iter().any(|argument| {
        argument.source_structural_result_binding_ordinal() == Some(binding_ordinal)
            && argument.access == CheckedStructuralAccess::Owned
            && argument.path.is_empty()
    })
}

fn call_owns_result(producer: &CheckedUnitEffectOperationPlan) -> bool {
    match producer {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            operand_source,
            ..
        } => {
            result.multiplicity == Multiplicity::Linear
                || matches!(
                    operand_source,
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
                )
        }
        CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
            result.multiplicity == Multiplicity::Linear
        }
        _ => false,
    }
}

/// Whether an owned whole call argument draws on a result this sequence
/// produced earlier and the call owns. The result custody accounting counts
/// that call as the result's one consumption with the same rule, so admitting
/// the argument here and counting it there cannot disagree.
fn call_consumes_result_argument(
    argument: &CheckedUnitStructuralArgumentPlan,
    earlier: &[CheckedUnitEffectOperationPlan],
) -> bool {
    let Some(ordinal) = argument.source_structural_result_binding_ordinal() else {
        return false;
    };
    argument.path.is_empty()
        && argument.access == CheckedStructuralAccess::Owned
        && earlier
            .iter()
            .rev()
            .find(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                        if result.binding_ordinal == ordinal
                )
            })
            .is_some_and(call_owns_result)
}

/// Name the first requirement a unit or scalar call's custody does not meet,
/// so a body the selected edges decline reports the argument shape rather
/// than the whole operation family. Diagnostic only: the admitting guards
/// above decide what is carried, and this reproduces their conditions in the
/// order they read.
fn call_custody_refusal(
    structural_arguments: &[CheckedUnitStructuralArgumentPlan],
    claim_transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
    earlier: &[CheckedUnitEffectOperationPlan],
) -> &'static str {
    if !claim_transfers.is_empty() {
        return "state graph: operation custody: call claim transfers";
    }
    for argument in structural_arguments {
        if whole_shared_argument(argument) {
            continue;
        }
        let borrowed = matches!(
            argument.access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        );
        if borrowed
            && (argument.source_parameter_index().is_some()
                || argument
                    .source_structural_result_binding_ordinal()
                    .is_some())
        {
            continue;
        }
        if argument.source_parameter_index().is_some()
            && argument.path.is_empty()
            && argument.access == CheckedStructuralAccess::Owned
        {
            continue;
        }
        if call_consumes_result_argument(argument, earlier) {
            continue;
        }
        if argument.access == CheckedStructuralAccess::Owned {
            if let Some(ordinal) = argument.source_structural_result_binding_ordinal() {
                let producer = earlier.iter().rev().find_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                        if result.binding_ordinal == ordinal =>
                    {
                        Some(result)
                    }
                    _ => None,
                });
                return match producer.map(|result| result.multiplicity) {
                    None => "state graph: operation custody: call owned result without a producer",
                    Some(_) => "state graph: operation custody: call owned local result",
                };
            }
            match argument.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. } => {
                    return "state graph: operation custody: call owned structural local";
                }
                CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. } => {
                    return "state graph: operation custody: call owned trivial affine local";
                }
                _ => {}
            }
        }
        return match (
            argument.access,
            argument.source_parameter_index().is_some(),
            argument.path.is_empty(),
        ) {
            (CheckedStructuralAccess::Owned, true, false) => {
                "state graph: operation custody: call owned parameter field argument"
            }
            (CheckedStructuralAccess::Owned, false, _) => {
                "state graph: operation custody: call owned non-parameter argument"
            }
            (_, false, _) => "state graph: operation custody: call non-parameter argument",
            _ => "state graph: operation custody: unit call arguments",
        };
    }
    "state graph: operation custody: unit call arguments"
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
    GuardedJump,
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
    CasePayloadTransfer,
    ParameterTransfer,
    ScalarArguments,
    ErasedArguments,
    ErasedProofArguments,
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
            (Self::Jump, SuccessorGuard::CasePayloadTransfer) => {
                "state graph: terminator: jump successor: case-payload transfer"
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
            (Self::Jump, SuccessorGuard::ErasedProofArguments) => {
                "state graph: terminator: jump successor: erased proof arguments"
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
            (Self::Conditional, SuccessorGuard::CasePayloadTransfer) => {
                "state graph: terminator: conditional successors: case-payload transfer"
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
            (Self::Conditional, SuccessorGuard::ErasedProofArguments) => {
                "state graph: terminator: conditional successors: erased proof arguments"
            }
            (Self::Conditional, SuccessorGuard::EdgeCleanup) => {
                "state graph: terminator: conditional successors: edge cleanup"
            }
            (Self::GuardedJump, SuccessorGuard::TransitionForm) => {
                "state graph: terminator: guarded jump successors: transition form"
            }
            (Self::GuardedJump, SuccessorGuard::TargetState) => {
                "state graph: terminator: guarded jump successors: target state"
            }
            (Self::GuardedJump, SuccessorGuard::ArgumentCount) => {
                "state graph: terminator: guarded jump successors: argument count"
            }
            (Self::GuardedJump, SuccessorGuard::StructuralArgument) => {
                "state graph: terminator: guarded jump successors: structural argument"
            }
            (Self::GuardedJump, SuccessorGuard::ReceiverTransfer) => {
                "state graph: terminator: guarded jump successors: receiver transfer"
            }
            (Self::GuardedJump, SuccessorGuard::SubsliceTransfer) => {
                "state graph: terminator: guarded jump successors: byte-subslice transfer"
            }
            (Self::GuardedJump, SuccessorGuard::ResultTransfer) => {
                "state graph: terminator: guarded jump successors: result-local transfer"
            }
            (Self::GuardedJump, SuccessorGuard::CasePayloadTransfer) => {
                "state graph: terminator: guarded jump successors: case-payload transfer"
            }
            (Self::GuardedJump, SuccessorGuard::ParameterTransfer) => {
                "state graph: terminator: guarded jump successors: parameter transfer"
            }
            (Self::GuardedJump, SuccessorGuard::ScalarArguments) => {
                "state graph: terminator: guarded jump successors: scalar arguments"
            }
            (Self::GuardedJump, SuccessorGuard::ErasedArguments) => {
                "state graph: terminator: guarded jump successors: erased arguments"
            }
            (Self::GuardedJump, SuccessorGuard::ErasedProofArguments) => {
                "state graph: terminator: guarded jump successors: erased proof arguments"
            }
            (Self::GuardedJump, SuccessorGuard::EdgeCleanup) => {
                "state graph: terminator: guarded jump successors: edge cleanup"
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
            (Self::ClosedCase, SuccessorGuard::CasePayloadTransfer) => {
                "state graph: terminator: closed-sum case successor: case-payload transfer"
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
            (Self::ClosedCase, SuccessorGuard::ErasedProofArguments) => {
                "state graph: terminator: closed-sum case successor: erased proof arguments"
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
    if cleanup.target_state != successor.target_state {
        return None;
    }
    // An owned parameter the target does not receive dies on this edge. The
    // cleanup evidence names exactly those positions; the edge carries them
    // so lowering disposes each where control leaves the state.
    let mut successor = successor;
    successor.trivial_affine_discard_parameter_positions =
        cleanup.trivial_affine_discard_parameter_positions.clone();
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
    // A guard that is an exact case test selects one sum case for this edge,
    // so the destructure-bound payload subtree `subject.Case::field` carries
    // the edge's proven membership into the target's custody. Record the
    // tested subject's canonical place and the selected case once; the
    // transfer walk reuses them for every structural argument.
    let case_test = match transition.guard {
        TransitionGuardNode::When(guard) => crate::proof::exact_outcome_case_test(program, guard)
            .and_then(|(subject, case)| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    source.symbol,
                    ordinal as usize,
                    subject,
                )
                .map(|place| (place, case))
            }),
        _ => None,
    };
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
                && matches!(program.expression_table.expression(indexed.index), ExpressionNode::Range(_))
            {
                mark(SuccessorGuard::SubsliceTransfer);
                let target_parameter = target_parameters.get(target.position as usize)?;
                let subslice = calls::view_subslice::admit(
                    program, facts, machine, source, source_structural,
                    target_parameter.type_reference, expression, ordinal as usize,
                    checked_trees::CheckedSubsliceSite::TransitionArgument {
                        argument_ordinal: target.position,
                    },
                )?;
                if subslice.range.type_identity != target.type_identity {
                    return None;
                }
                return Some(CheckedStructuralControlTransferPlan {
                    source: subslice.transfer(),
                    target_parameter_index: u32::try_from(target_index).ok()?,
                });
            }
            // A whole view local forwards the view its `let` published: the
            // target re-borrows that same shared place exactly as a forwarded
            // view parameter does, so the edge names the local's result
            // binding just as an owned local's move does.
            let shared_view = target.access == CheckedStructuralAccess::SharedBorrow
                && target_parameters.get(target.position as usize).is_some_and(|parameter| {
                    calls::view_subslice::view_kind(program, parameter.type_reference).is_some()
                });
            if target.access == CheckedStructuralAccess::Owned || shared_view {
                mark(SuccessorGuard::ResultTransfer);
                let place = crate::flow::canonical_place_from_expression_in_state(program, source.symbol, ordinal as usize, expression)?;
                if place.segments.is_empty() {
                    let mut matches = operations.iter().filter_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralCall { result, discard_result_on_return: false, .. }
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, discard_result_on_return: false, .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, discard_result_on_return: false, .. }
                        | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => Some(result),
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
            if let Some((subject_place, selected_case)) = &case_test {
                mark(SuccessorGuard::CasePayloadTransfer);
                let place = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    source.symbol,
                    ordinal as usize,
                    expression,
                )?;
                if let Some(source) = case_payload_transfer(
                    program,
                    place,
                    subject_place,
                    *selected_case,
                    source,
                    source_structural,
                    target,
                ) {
                    return Some(CheckedStructuralControlTransferPlan {
                        source,
                        target_parameter_index: u32::try_from(target_index).ok()?,
                    });
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
            let role = CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: target.source_position,
            };
            let pure = &facts.values.scalar_expressions;
            let computations = &facts.values.scalar_computations;
            let mut roots = computations
                .roots
                .iter()
                .map(|(_, root)| root)
                .filter(|root| {
                    root.state == source.symbol
                        && root.statement_ordinal == ordinal
                        && root.role == role
                });
            let root = roots.next();
            if roots.next().is_some() {
                return None;
            }
            if let Some(root) = root {
                let has_pure = pure.source_bindings.iter().any(|(_, binding)| {
                    binding.state == source.symbol
                        && binding.statement_ordinal == ordinal
                        && binding.role == role
                }) || pure.expressions.iter().any(|expression| {
                    expression.state == source.symbol
                        && expression.statement_ordinal == ordinal
                        && expression.role == role
                });
                if has_pure
                    || root.machine != machine.symbol
                    || !computations.nodes.is_valid(root.root)
                    || computations.nodes.get(root.root).authored_root != argument
                    || computations.nodes.get(root.root).primitive_type != target.primitive_type
                {
                    return None;
                }
            } else {
                let (custody, expression) =
                    pure.bound_expression_at(source.symbol, ordinal, role)?;
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
    mark(SuccessorGuard::ErasedProofArguments);
    // Erased contract-term formals carry proof terms, not scalar
    // expressions: each actual is the term recorded under this edge's exact
    // coordinate by the scalar expression pass.
    let erased_proof_arguments =
        crate::execution::terminal_unit::types::erased_proof_parameter_plans(program, target)?
            .iter()
            .map(|target| {
                facts
                    .values
                    .proof_terms
                    .term_at(
                        source.symbol,
                        ordinal,
                        checked_trees::CheckedProofTermRole::TransitionArgument {
                            argument_ordinal: target.source_position,
                        },
                    )
                    .cloned()
            })
            .collect::<Option<Vec<_>>>()?;
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: target.symbol,
        transfers,
        scalar_arguments,
        erased_arguments,
        erased_proof_arguments,
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}

/// Admit a `[copy]` case-payload subtree transfer on a case-tested edge.
///
/// The edge's guard selected `selected_case` on `subject_place`, so an
/// argument whose place extends that subject through exactly that case
/// into one declared payload field carries proven membership into the
/// target's custody — the same authorization an authored case arm gives
/// its destructure bindings. The checked source keeps the resolved
/// subject argument plus the selected case and field identities, so a
/// downstream channel can rejoin the projected subtree without
/// re-deriving the destructure local.
fn case_payload_transfer(
    program: &TypedTrees,
    place: crate::flow::CanonicalPlace,
    subject_place: &crate::flow::CanonicalPlace,
    selected_case: symbols::SymbolHandle,
    source: &typed_trees::state::State,
    source_structural: &[CheckedUnitStructuralParameterPlan],
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<checked_trees::CheckedStructuralControlTransferSourcePlan> {
    // The argument must name exactly one payload field beneath the edge's
    // proven case: `subject-place ++ [Case{selected}, Field{payload}]`.
    // Longer tails reach inside the payload field's own subtree and stay
    // unadmitted.
    if place.root != subject_place.root
        || place.segments.len() != subject_place.segments.len() + 2
        || place.segments[..subject_place.segments.len()] != subject_place.segments[..]
    {
        return None;
    }
    let [
        facts::PlaceSegment::Case { variant },
        facts::PlaceSegment::Field {
            symbol: field_symbol,
        },
    ] = &place.segments[subject_place.segments.len()..]
    else {
        return None;
    };
    if *variant != selected_case {
        return None;
    }
    // The selected case's declared payload field owns the transferred
    // subtree's identity and type.
    let field = program.data_definitions().iter().find_map(|data| {
        program.data_members(data).iter().find_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.symbol == selected_case)
                .then(|| {
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == *field_symbol)
                })
                .flatten()
        })
    })?;
    // Copying the payload out of the borrowed subject must not disturb
    // the subject's custody: the field's own contents copy, its declared
    // type and multiplicity match the target exactly, and the target
    // takes owned custody of the copied subtree.
    if !validation::has_plain_owned_contents_with_numeric_constraints(program, field.type_reference)
        || program
            .normalized_type_identity(field.type_reference)
            .into_string()
            != target.type_identity
        || program.type_multiplicity(field.type_reference) != target.multiplicity
        || target.access != CheckedStructuralAccess::Owned
    {
        return None;
    }
    // The tested subject resolves to a retained structural parameter.
    let facts::PlaceRoot::Symbol(root) = subject_place.root else {
        return None;
    };
    let position = program
        .state_parameters(source)
        .iter()
        .position(|parameter| parameter.symbol == root)?;
    let (parameter_index, _) = source_structural
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.position as usize == position)?;
    // The projection reaching the tested sum keeps the shared
    // structural-path vocabulary; the subject's own type is the field
    // type at the end of that projection.
    let mut subject_type = program
        .state_parameters(source)
        .get(position)?
        .type_reference;
    let path = subject_place
        .segments
        .iter()
        .map(|segment| {
            let facts::PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            subject_type = program.data_definitions().iter().find_map(|data| {
                program.data_members(data).iter().find_map(|member| {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (field.symbol == *symbol).then_some(field.type_reference)
                })
            })?;
            super::types::terminal_field_identity(program, *symbol)
                .map(CheckedUnitStructuralPathSegment::Field)
        })
        .collect::<Option<Vec<_>>>()?;
    let variant = program.data_definitions().iter().find_map(|data| {
        program.data_members(data).iter().find_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.symbol == selected_case).then_some(variant)
        })
    })?;
    Some(
        checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
            subject: CheckedUnitStructuralArgumentPlan {
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(parameter_index).ok()?,
                },
                path,
                type_identity: program.normalized_type_identity(subject_type).into_string(),
                access: CheckedStructuralAccess::SharedBorrow,
            },
            case_identity: variant
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| variant.name.as_str().to_owned()),
            field_identity: field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned()),
            path: Vec::new(),
        },
    )
}
