//! Admitting one composed Unit state's planned operations.
//!
//! Named structural results stay live through successor operand evaluation:
//! the selected edge, not one global return flag, owns their transfer or
//! disposal. Every remaining operation must then carry custody a composed
//! state can replay; one that does not names its kind in the trace.
use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, StatementNode, TypedTrees,
    call_argument_admitted, call_custody_refusal, control,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

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
        if let Some((result, discard_result_on_return)) =
            operation.discardable_structural_result_mut()
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::LocalData(_))
            )
        {
            *discard_result_on_return = false
        }
    }
    for (operation_index, operation) in operations.iter().enumerate() {
        match operation {
            CheckedUnitEffectOperationPlan::Call(call)
                if call_custody_admitted(call, &operations[..operation_index]) => {}
            // The common source replay authenticates construction and each
            // issuance or transferred claim. A state graph changes only the
            // selected-edge disposition, not establishment authority.
            CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                discard_result_on_return: false,
                ..
            }
            // A view-subslice local owns nothing: its shared view ends
            // with the loan, on every selected edge alike.
            | CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            // Fixed-array, mutable-primitive, and trivial-affine locals mint
            // storage at the authored statement the same way: the binding's
            // own loans and disposal carry its custody, not the op.
            | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
            | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            // A declared structural local's establishment mints storage at
            // its authored statement the same way: each member operation's
            // loans and the binding's disposal carry the custody, not the op.
            | CheckedUnitEffectOperationPlan::EstablishStructuralLocal { .. }
            // A reference pair's custody is the loan's: the establishment is
            // the authored production and the release the authored weakening
            // boundary. Selected edges carry the loan itself; neither op
            // claims a new root.
            | CheckedUnitEffectOperationPlan::EstablishReference { .. }
            | CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {}
            CheckedUnitEffectOperationPlan::ByteSequenceStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            // The ordinary sequencer has already rejoined the primitive
            // destination, RHS, and complete write frame. Crossing a state
            // edge does not turn that non-observing write into a new family.
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            // An atomic event's root authority and operands rejoined at
            // mint; its observed prior is an ordinary dense scalar local.
            | CheckedUnitEffectOperationPlan::AtomicAccess(_)
            // A view write observes through the view's own loan: the
            // destination borrows the slice rather than owning it, and the
            // index and element scalars rejoined dense at mint. The edge
            // carries the loan; the write claims no root of its own.
            | CheckedUnitEffectOperationPlan::ElementViewWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            // Case replacement closes its destination window within the
            // operation. Literal construction and unrestricted leaf copy
            // carry no new ownership across the selected edge; receiving
            // source replay and the ordinary move/restore ledger still
            // check the exact destination and replacement independently.
            | CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_)
            | CheckedUnitEffectOperationPlan::StructuralCaseLiteralFieldStore(_)
            // A representation view store writes through the view's own
            // loaned destination; each footprint byte's index obligation,
            // operand, and destination rejoined at mint. No edge carries a
            // root the write did not already claim.
            | CheckedUnitEffectOperationPlan::RepresentationViewStore(_) => {}
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
                    } | checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        ..
                    }
                )
                && value.access == CheckedStructuralAccess::Owned => {}
            // A result may die on its producing or borrowing call's continuation.
            // The cleanup shares that call's coordinate rather than
            // consuming a new authored statement; sequenced stores may sit
            // between the producer -- the statement's call, or the
            // establishment of a construction replacing a field -- and the
            // discard that retires a displaced binding.
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                cleanup_actions,
            }
                if cleanup_actions.iter().all(|discard| {
                    matches!(
                    *discard.source(),
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        ..
                    }
                )
                }) && operations[..operation_index]
                    .iter()
                    .rev()
                    .find(|operation| match operation {
                        // A scalar/Unit consumer can retire its argument owners
                        // just as a structural producer can retire its result.
                        CheckedUnitEffectOperationPlan::Call(_) => true,
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => true,
                        // A bound-value replacement's move-out produces the
                        // displaced binding the cleanup retires.
                        CheckedUnitEffectOperationPlan::MoveStructuralField { .. } => true,
                        _ => false,
                    })
                    .is_some_and(|producer| match producer {
                        CheckedUnitEffectOperationPlan::Call(call) => call.coordinate == *coordinate,
                        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                            result,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::MoveStructuralField {
                            result, ..
                        } => {
                            result.statement_index == coordinate.statement_index
                                && coordinate.call_ordinal == 0
                        }
                        _ => false,
                    }) => {}
            // Statement entry retires an earlier whole carrier without
            // consuming the following statement. Statement planning has joined
            // the complete loan roster and whole-local liveness at this boundary.
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary:
                    checked_trees::CheckedUnitCleanupBoundary::StatementEntry { statement_index },
                cleanup_actions,
            } if statements.get(*statement_index as usize).is_some()
                && !cleanup_actions.is_empty()
                && cleanup_actions.iter().all(|discard| {
                    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } = *discard.source()
                    else {
                        return false;
                    };
                    let mut producers = operations[..operation_index]
                        .iter()
                        .filter_map(|operation| {
                            let result = operation.statement_structural_result_binding()?;
                            (result.binding_ordinal == binding_ordinal)
                                .then_some((operation, result))
                        });
                    let Some((producer, result)) = producers.next() else {
                        return false;
                    };
                    producers.next().is_none()
                        && discard.path().is_empty()
                        && discard.type_identity() == result.type_identity
                        && result.multiplicity == language_semantics::Multiplicity::Affine
                        && result.statement_index < *statement_index
                        && matches!(
                            statements.get(result.statement_index as usize),
                            Some(StatementNode::LocalData(_))
                        )
                        && producer
                            .discardable_structural_result()
                            .is_some_and(|(_, discard_on_return)| !discard_on_return)
                }) => {}
            _ => {
                // Name the operation family whose custody the selected
                // edges cannot yet carry; admitted families never reach
                // this arm.
                trace.phase(match operation {
                    CheckedUnitEffectOperationPlan::Call(call) => match (call.route(), call.result()) {
                        (_, CheckedUnitCallResult::Structural { .. }) => {
                            "state graph: operation custody: discarded structural result"
                        }
                        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
                            "state graph: operation custody: boundary call arguments"
                        }
                        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Scalar(_)) => {
                            "state graph: operation custody: scalar call"
                        }
                        (CheckedUnitCallRoute::Module { .. }, _) => call_custody_refusal(
                            &call.structural_arguments,
                            &operations[..operation_index],
                        ),
                    },
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. } => {
                        "state graph: operation custody: discarded structural result"
                    }
                    CheckedUnitEffectOperationPlan::ContinuationCleanup { .. } => {
                        "state graph: operation custody: continuation cleanup owner"
                    }
                    CheckedUnitEffectOperationPlan::EstablishReference { .. }
                    | CheckedUnitEffectOperationPlan::ReleaseReference { .. } => {
                        "state graph: operation custody: reference custody"
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

/// Whether the selected edges of a composed state can carry one call's
/// custody. A structural result must stay live for its edge to own; a
/// boundary call completes its receipts at the call; an ordinary call consumes
/// its exact argument claims through the shared call custody replay. Every
/// structural operand except a structural result's must already be admitted.
pub(super) fn call_custody_admitted(
    call: &CheckedUnitCallPlan,
    earlier: &[CheckedUnitEffectOperationPlan],
) -> bool {
    let arguments_admitted = || {
        call.structural_arguments
            .iter()
            .all(|argument| call_argument_admitted(argument, earlier))
    };
    match (call.route(), call.result()) {
        (
            _,
            CheckedUnitCallResult::Structural {
                discard_result_on_return,
                ..
            },
        ) => !discard_result_on_return,
        (_, _) => arguments_admitted(),
    }
}
