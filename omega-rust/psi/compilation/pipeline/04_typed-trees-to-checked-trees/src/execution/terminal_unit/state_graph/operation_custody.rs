//! Admitting one composed Unit state's planned operations.
//!
//! Named structural results stay live through successor operand evaluation:
//! the selected edge, not one global return flag, owns their transfer or
//! disposal. Every remaining operation must then carry custody a composed
//! state can replay; one that does not names its kind in the trace.
use super::result_custody::{call_owns_result, retained_structural_result};
use super::{
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
    Multiplicity, StatementNode, control,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

pub(super) fn admit(
    program: &dyn ::validation::CheckingInput,
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
            // Capturing coordinates neither moves the destination nor opens
            // restoration debt. The same checked storage namespace must be
            // available before evaluating the assignment's value producer.
            CheckedUnitEffectOperationPlan::CaptureAssignmentDestination { destination, .. }
                if call_argument_admitted(destination, &operations[..operation_index]) => {}
            // The common source replay authenticates construction and each
            // issuance or transferred claim. A state graph changes only the
            // selected-edge disposition, not establishment authority.
            CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                discard_result_on_return: false,
                ..
            })
            // A view-subslice local owns nothing: its shared view ends
            // with the loan, on every selected edge alike.
            | checked_trees::byte_sequence_establishment_pattern!()
            | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. })
            // Fixed-array, mutable-primitive, and trivial-affine locals mint
            // storage at the authored statement the same way: the binding's
            // own loans and disposal carry its custody, not the op.
            | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. })
            | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. })
            | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. })
            // A declared structural local's establishment mints storage at
            // its authored statement the same way: each member operation's
            // loans and the binding's disposal carry the custody, not the op.
            | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Structural { .. })
            // A reference pair's custody is the loan's: the establishment is
            // the authored production and the release the authored weakening
            // boundary. Selected edges carry the loan itself; neither op
            // claims a new root.
            | checked_trees::reference_hold_pattern!() => {}
            // The ordinary sequencer has already rejoined every store's
            // destination, value and complete write frame; crossing a state
            // edge does not turn that write into a new family. A store
            // through a view or recast loan writes through the loan the
            // edge carries; a sum field's window closes within the store,
            // and literal construction or unrestricted leaf copy carries no
            // new ownership across the selected edge. Receiving source
            // replay and the move/restore ledger still check the exact
            // destination and replacement independently. A window repair or
            // local rebind installs a whole owned binding that rejoined at
            // mint: it consumes that binding like a call argument does.
            CheckedUnitEffectOperationPlan::Store(store)
                if store.installed_value().is_none_or(|value| {
                    value.access == CheckedStructuralAccess::Owned
                        && call_argument_admitted(value, &operations[..operation_index])
                }) => {}
            // An atomic event's root authority and operands rejoined at
            // mint; its observed prior is an ordinary dense scalar local.
            // A port write carries exact constant operands and its service
            // reach: no structural custody crosses the selected edge.
            CheckedUnitEffectOperationPlan::AtomicFence { .. }
            | CheckedUnitEffectOperationPlan::AtomicAccess(_)
            | CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::RegisterAccess { .. } => {}
            // The window pair the sequencer itself joined — a move-out
            // binding the displaced field value and the exact restoration
            // into the opened hole — carries no new custody across the
            // selected edges: its owned source, borrowed destination
            // window, and consumed result binding all rejoined at mint.
            CheckedUnitEffectOperationPlan::MoveStructuralField { source, .. }
                if source.access == CheckedStructuralAccess::Owned
                    && call_argument_admitted(source, &operations[..operation_index]) => {}
            // Every cleanup boundary uses the same live storage namespace as
            // calls and stores. The ownership producer selected death; this
            // join asks only whether the graph can address that storage.
            // Result-only and nearest-producer recognizers lose owned child
            // replacement and do not establish any additional authority.
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary,
                cleanup_actions, ..
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
                            .filter_map(CheckedUnitEffectOperationPlan::invocation_coordinate)
                            .any(|invocation| invocation == *coordinate),
                        checked_trees::CheckedUnitCleanupBoundary::StatementEntry { statement_index }
                        | checked_trees::CheckedUnitCleanupBoundary::Replacement { statement_index, .. } => statements.get(*statement_index as usize).is_some(),
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
                    CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. }) => {
                        "state graph: operation custody: discarded structural result"
                    }
                    CheckedUnitEffectOperationPlan::ContinuationCleanup { .. } => {
                        "state graph: operation custody: continuation cleanup owner"
                    }
                    checked_trees::reference_hold_pattern!() => {
                        "state graph: operation custody: reference custody"
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
