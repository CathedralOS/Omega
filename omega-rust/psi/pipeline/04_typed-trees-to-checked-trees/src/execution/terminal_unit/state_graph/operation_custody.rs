//! Admitting one composed Unit state's planned operations.
//!
//! Named structural results stay live through successor operand evaluation:
//! the selected edge, not one global return flag, owns their transfer or
//! disposal. Every remaining operation must then carry custody a composed
//! state can replay; one that does not names its kind in the trace.
use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, Multiplicity, StatementNode,
    TypedTrees, call_consumes_result_argument, call_custody_refusal, control,
    whole_shared_argument,
};

pub(super) fn admit(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    operations: &mut [CheckedUnitEffectOperationPlan],
    trace: &control::LocalConstructionTrace,
) -> Option<()> {
    let statements = program.statement_table.statements(state.statement_nodes);
    // Named results remain live through successor operand evaluation. The
    // selected edge owns their exact transfer/disposal partition below;
    // one global return-discard flag cannot express asymmetric successors.
    for operation in operations.iter_mut() {
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
            // An atomic event's root authority and operands rejoined at
            // mint; its observed prior is an ordinary dense scalar local.
            | CheckedUnitEffectOperationPlan::AtomicAccess(_)
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
    Some(())
}
