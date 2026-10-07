//! Admitting one composed Unit state's planned operations.
//!
//! Named structural results stay live through successor operand evaluation:
//! the selected edge, not one global return flag, owns their transfer or
//! disposal. Every remaining operation must then carry custody a composed
//! state can replay; one that does not names its kind in the trace.
use super::result_custody::{call_owns_result, retained_structural_result};
use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    Multiplicity, StatementNode, TypedTrees, control,
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
                if source.access == CheckedStructuralAccess::Owned
                    && call_argument_admitted(source, &operations[..operation_index]) => {}
            CheckedUnitEffectOperationPlan::StoreStructuralField {
                destination,
                value,
                ..
            }
            | CheckedUnitEffectOperationPlan::RebindStructuralLocal {
                destination,
                value,
                ..
            } if destination.access == CheckedStructuralAccess::Owned
                && value.access == CheckedStructuralAccess::Owned
                && call_argument_admitted(destination, &operations[..operation_index])
                && call_argument_admitted(value, &operations[..operation_index]) => {}
            // Every cleanup boundary uses the same live storage namespace as
            // calls and stores. The ownership producer selected death; this
            // join asks only whether the graph can address that storage.
            // Result-only and nearest-producer recognizers lose owned child
            // replacement and do not establish any additional authority.
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary,
                cleanup_actions,
            }
                if !cleanup_actions.is_empty()
                    && cleanup_actions.iter().all(|action| call_argument_admitted(
                        &checked_trees::CheckedUnitStructuralArgumentPlan {
                            source: action.source().clone(),
                            path: action.path().to_vec(),
                            type_identity: action.type_identity().to_owned(),
                            access: CheckedStructuralAccess::Owned,
                        },
                        &operations[..operation_index],
                    ))
                    && match boundary {
                        checked_trees::CheckedUnitCleanupBoundary::Call(coordinate) => operations[..operation_index].iter()
                            .filter_map(CheckedUnitEffectOperationPlan::call)
                            .any(|call| call.coordinate == *coordinate),
                        checked_trees::CheckedUnitCleanupBoundary::StatementEntry { statement_index }
                        | checked_trees::CheckedUnitCleanupBoundary::Replacement { statement_index } => statements.get(*statement_index as usize).is_some(),
                    } => {}
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
                        (
                            CheckedUnitCallRoute::Module { .. } | CheckedUnitCallRoute::Dynamic { .. },
                            _,
                        ) => call_custody_refusal(
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

/// The call sequencer already joined each operand to its loan or consumption.
/// A state edge does not change that call's argument custody. Boundary and
/// ordinary calls share this rule; their receipts and claim transfers remain
/// separate obligations at their respective consumers.
fn call_argument_admitted(
    argument: &CheckedUnitStructuralArgumentPlan,
    earlier: &[CheckedUnitEffectOperationPlan],
) -> bool {
    if argument.access != CheckedStructuralAccess::Owned {
        // Loan lifetime and restoration remain the sequencer's obligations;
        // borrowing does not consume a graph-owned structural result.
        return true;
    }
    let Some(ordinal) = argument.source_structural_result_binding_ordinal() else {
        // Parameters, checked local storage and inline operands retain their
        // sequencer custody. In particular an inline value is established at
        // the call, not by a fictitious earlier result. Only an explicit result
        // binding introduces the additional graph disposition checked below.
        return true;
    };
    let mut producers = earlier.iter().filter_map(|operation| {
        let result = operation.structural_result_binding()?;
        (result.binding_ordinal == ordinal).then_some((operation, result))
    });
    let Some((producer, result)) = producers.next() else {
        return false;
    };
    producers.next().is_none()
        && retained_structural_result(producer).is_some()
        // The argument planner matched the selected subtree's declaration
        // and exact transfer. Graph custody retains its owner binding; a
        // projection does not turn that binding into a whole-value argument.
        // Copies leave the original for its existing state-exit accounting;
        // affine/linear transfers use the same consumption rule as that
        // accounting, which independently requires exactly one disposition.
        && (result.multiplicity == Multiplicity::Unrestricted || call_owns_result(producer))
}

/// Name the first unmet call requirement using the same argument admission
/// as the operation guard, then distinguish its unsupported custody.
fn call_custody_refusal(
    structural_arguments: &[CheckedUnitStructuralArgumentPlan],
    earlier: &[CheckedUnitEffectOperationPlan],
) -> &'static str {
    for argument in structural_arguments {
        if call_argument_admitted(argument, earlier) {
            continue;
        }
        if argument.access == CheckedStructuralAccess::Owned
            && let Some(ordinal) = argument.source_structural_result_binding_ordinal()
        {
            let producer = earlier.iter().rev().find_map(|operation| {
                operation
                    .structural_result_binding()
                    .filter(|result| result.binding_ordinal == ordinal)
            });
            return match producer.map(|result| result.multiplicity) {
                None => "state graph: operation custody: call owned result without a producer",
                Some(_) => "state graph: operation custody: call owned local result",
            };
        }
        return "state graph: operation custody: unit call arguments";
    }
    "state graph: operation custody: unit call arguments"
}
