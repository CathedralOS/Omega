use crate::EmissionPlanningInput;
use crate::blocker;
use crate::semantic_scope::state_name;
use omega_backend_report_types::EmissionBlocker;
use omega_target_operations::SelectedInstructionKind;
use psi_arena::Arena;

/// Every dispatch-loop edge that carries a `CallResultReturn` must have a
/// SELECTED return-write (integer/copy/binary/conversion) at its clone-terminal state --
/// otherwise the caller's result slot silently keeps its prior/ZII value (the
/// exact silent-wrong class the return-write matrix has been closing: field
/// bindings, binary terminals (integer AND float), transition args). A
/// terminal shape the return-write cannot serve yet (an unresolvable place)
/// refuses LOUDLY here instead of misdelivering -- which is what makes the
/// splice fences' dispatch-route exemption sound: dispatched value calls
/// either deliver correctly or fail to compile, never silently ZII.
pub(crate) fn collect_call_result_return_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, case) in input.runtime_dispatch_loop.cases.iter() {
        for edge in input
            .runtime_dispatch_loop
            .edges
            .span(case.edges)
            .into_iter()
            .flatten()
        {
            let Some(call_result) = edge.call_result else {
                continue;
            };
            // DELIVERY-PLACE granularity (2026-07-10s, closing the arithmetic
            // thread's handoff): the old check was STATE-granular -- ANY
            // write-ish instruction in the terminal case counted, so a result
            // CONSUMPTION copy (`result_slot -> let d`) satisfied it while
            // the PRODUCTION write silently dropped (the machine-array
            // fused-arg escape). Served now means a write whose TARGET is a
            // legitimate delivery place for THIS call:
            //   (a) the call's FRAME result slot -- both lookups, exactly the
            //       serve's sequence (for_dispatch with the return edge's
            //       target dispatch, then any_role; the any_role-only probe
            //       falsely blocked the transition-arg canary), matched by
            //       RANGE (enum-case serves write tag+payload at absolute
            //       offsets WITHIN the slot);
            //   (b) any MACHINE-region write, but ONLY when the caller's
            //       statement is a FIELD binding (`self.total = ...`) --
            //       a let-bound caller's delivery must hit the frame slot,
            //       so an arm's unrelated machine effect cannot false-serve
            //       a dropped result.
            let mut slots = vec![
                input
                    .runtime_storage
                    .state_call_result_slot_for_dispatch_by_ordinal(
                        edge.target_dispatch_index,
                        call_result.call_source_key,
                        call_result.statement_index,
                        call_result.call_ordinal,
                    ),
                input
                    .runtime_storage
                    .state_call_result_slot_any_role_by_ordinal(
                        call_result.call_source_key,
                        call_result.statement_index,
                        call_result.call_ordinal,
                    ),
            ];
            let mut value_calls = input
                .state_calls
                .calls_for_statement(call_result.call_source_key, call_result.statement_index)
                .filter(|call| call.role != omega_state_calls::StateCallRole::Statement);
            let only_call = value_calls.next();
            if only_call.is_some_and(|call| {
                call.call_ordinal == call_result.call_ordinal && value_calls.next().is_none()
            }) {
                // Bare single-call local initializers can deliver straight to
                // LocalStorage. Never apply this ordinal-less fallback to a
                // sibling-call statement: each result must serve its own slot.
                slots.push(input.runtime_storage.state_call_result_slot_any_role(
                    call_result.call_source_key,
                    call_result.statement_index,
                ));
            }
            let slot_ranges: Vec<(bool, usize, usize)> = slots
                .into_iter()
                .flatten()
                .map(|slot| (true, slot.byte_offset, slot.byte_offset + slot.byte_size))
                .collect();
            let caller_binds_field = caller_statement_assigns_member(
                input,
                call_result.call_source_key,
                call_result.statement_index,
            );
            let served = input
                .instructions
                .code
                .instructions
                .iter()
                .any(|(_, instruction)| {
                    if instruction.source_key.machine != case.key.machine
                        || instruction.source_key.state != case.key.state
                    {
                        return false;
                    }
                    let Some((is_frame, start, end)) = instruction_write_target(&instruction.kind)
                    else {
                        return false;
                    };
                    if !is_frame {
                        return caller_binds_field;
                    }
                    slot_ranges
                        .iter()
                        .any(|(_, lo, hi)| start < *hi && end > *lo)
                });
            if served {
                continue;
            }
            blockers.insert(blocker(
                "call result",
                &format!(
                    "{}: the dispatched value call's terminal (returning into {} \
                     statement {}) has no selected return-write -- this terminal \
                     shape is not served yet (an unresolvable value), and running \
                     it would silently leave the caller's result as ZII. Bind \
                     through a supported shape (place/literal/binary terminal, \
                     integer or float) or restructure the callee.",
                    state_name(input, case.key),
                    state_name(input, call_result.call_source_key),
                    call_result.statement_index,
                ),
            ));
        }
    }
}

/// The write TARGET of a served return-write kind: `(is_frame, start, end)`
/// byte range, or `None` for non-write kinds.
fn instruction_write_target(kind: &SelectedInstructionKind) -> Option<(bool, usize, usize)> {
    use omega_target_operations::RuntimeStorageRegion;
    let (region, offset, size) = match kind {
        SelectedInstructionKind::WritePlaceInteger {
            target, byte_size, ..
        } => match target.const_offset() {
            Some(target_offset) => (target.region, target_offset, *byte_size),
            None => return None,
        },
        // A direct (const-path) place target serves like the retired plain
        // copy; a deref/indexed place has no flat byte range to claim.
        SelectedInstructionKind::CopyPlaces {
            target, byte_count, ..
        } => match target.const_offset() {
            Some(target_offset) => (target.region, target_offset, *byte_count),
            None => return None,
        },
        SelectedInstructionKind::WritePlaceBinary {
            target, byte_size, ..
        } => match target.const_offset() {
            Some(target_offset) => (target.region, target_offset, *byte_size),
            None => return None,
        },
        SelectedInstructionKind::WritePlaceConvert {
            target,
            target_byte_size,
            ..
        } => match target.const_offset() {
            Some(target_offset) => (target.region, target_offset, *target_byte_size),
            None => return None,
        },
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_region,
            target_offset,
            target_byte_size,
            ..
        } => (*target_region, *target_offset, *target_byte_size),
        _ => return None,
    };
    Some((
        matches!(region, RuntimeStorageRegion::RuntimeFrame),
        offset,
        offset + size,
    ))
}

/// Whether the caller statement is an ASSIGNMENT to a MEMBER expression
/// (`self.total = self.count(..)` -- the field-binding shape whose delivery
/// is a machine-region write).
fn caller_statement_assigns_member(
    input: &EmissionPlanningInput<'_>,
    call_source_key: omega_control_flow::StateKey,
    statement_index: usize,
) -> bool {
    let Some(state) = input.control_flow.state_by_key(call_source_key) else {
        return false;
    };
    let Some(operations) = input.control_flow.operations.span(state.operations) else {
        return false;
    };
    operations
        .iter()
        .filter(|operation| operation.statement_index == statement_index)
        .any(|operation| match operation.expressions {
            omega_control_flow::OperationExpressionRefs::Assignment { target, .. } => matches!(
                input.control_flow.expressions.expression(target),
                psi_checked_trees::expression::ExpressionNode::Member(_)
            ),
            _ => false,
        })
}
