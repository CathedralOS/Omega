use crate::body::{build_runtime_storage_body_plan, build_straight_line_runtime_storage_plan};
use crate::layout::align_to;
use crate::{RuntimeStorageContext, RuntimeStoragePlan, RuntimeStorageWrite};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use psi_checked_trees::expression::ExpressionTableCapacity;
use std::sync::Arc;

pub fn build_runtime_storage_plan(context: RuntimeStorageContext) -> RuntimeStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_storage_plan_with_workers(Arc::new(context), workers.handle())
}

pub fn build_runtime_storage_plan_with_workers(
    context: Arc<RuntimeStorageContext>,
    workers: WorkerPoolHandle,
) -> RuntimeStoragePlan {
    if context.runtime_bodies.bodies.is_empty() {
        let mut plan = build_straight_line_runtime_storage_plan(&context);
        reserve_frame_scratch_region(&mut plan);
        return plan;
    }

    let body_count = context.runtime_bodies.bodies.len();
    let context_for_bodies = Arc::clone(&context);
    let body_plans = workers.map_ordered(body_count, move |index| {
        let body = context_for_bodies
            .runtime_bodies
            .bodies
            .storage_slice()
            .get(index)
            .expect("runtime-storage worker index should be in range");

        build_runtime_storage_body_plan(&context_for_bodies, body)
    });

    let expression_capacity = body_plans.iter().fold(
        ExpressionTableCapacity::default(),
        |mut capacity, body_plan| {
            capacity.saturating_add_assign(body_plan.expressions.copy_capacity());
            capacity
        },
    );
    let invariant_name_capacity = body_plans
        .iter()
        .map(|body_plan| body_plan.invariant_names.len())
        .sum();
    let frame_slot_capacity = body_plans
        .iter()
        .map(|body_plan| body_plan.frame_slots.len())
        .sum();
    let write_capacity = body_plans
        .iter()
        .map(|body_plan| body_plan.writes.len())
        .sum();
    let mut plan = RuntimeStoragePlan::with_capacities(
        expression_capacity,
        invariant_name_capacity,
        frame_slot_capacity,
        write_capacity,
    );

    for body_plan in body_plans {
        let RuntimeStoragePlan {
            expressions,
            invariant_names: _,
            frame_slots,
            writes,
            // Scratch is reserved later on the aggregate plan (stacking phase).
            frame_scratch_base: _,
            frame_scratch_size: _,
            wire_scratch_base: _,
            wire_scratch_size: _,
            host_argument_scratch_base: _,
            host_argument_scratch_size: _,
            entry_argument_spill_base: _,
            entry_argument_spill_size: _,
            entry_indirect_result_pointer_base: _,
            entry_indirect_result_pointer_size: _,
            entry_result_scratch_base: _,
            entry_result_scratch_size: _,
        } = body_plan;
        plan.frame_slots.insert_many(frame_slots.into_items());
        for write in writes.into_items() {
            plan.writes.append(RuntimeStorageWrite {
                dispatch_index: write.dispatch_index,
                source_key: write.source_key,
                statement_index: write.statement_index,
                target: plan.expressions.copy_from(&expressions, write.target),
                value: plan.expressions.copy_from(&expressions, write.value),
                mutation_kind: write.mutation_kind,
                lowering: write.lowering,
            });
        }
    }

    append_unserved_recursive_call_result_slots(&context, &mut plan);
    reserve_frame_scratch_region(&mut plan);
    plan
}

/// A DISPATCHED value call to an ENTRY-REENTERING callee (`true ->
/// weaken(...)` looping back to the callee's own entry) carries NO body
/// operation anywhere, so the per-body ops-driven allocation never creates
/// its call-result slot and the dispatch-edge write bails at `slot None`
/// (the recursive-entry value-arm fence). Allocate those slots HERE, after
/// the per-body merge, where the FULL slot set is visible: any call that
/// already has a slot (dual_accumulator's recursion), or that binds to a
/// FIELD (delivered by the edge write's machine-place fallback, which a
/// slot would HIJACK -- multi_arm), is skipped. Per-body heuristics for the
/// same purpose failed four different ways (marker-op visibility, coverage
/// mismatches, fallback hijack, served-shape overlap); only the merged view
/// answers "is this call actually unserved". The slot is placed in the
/// caller's CONTINUATION segment's dispatch namespace (the return edge
/// targets it and the caller's read executes there), at that namespace's
/// current extent.
fn append_unserved_recursive_call_result_slots(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
) {
    use omega_state_calls::StateCallRole;

    let callee_reenters_its_entry = |target_key: omega_control_flow::StateKey| -> bool {
        context
            .control_flow
            .states
            .iter()
            .filter(|(_, state)| state.key.machine == target_key.machine)
            .any(|(_, state)| {
                context
                    .control_flow
                    .transitions
                    .span(state.transitions)
                    .into_iter()
                    .flatten()
                    .any(|transition| {
                        matches!(
                            &transition.target,
                            omega_control_flow::PlannedTransitionTarget::State { key, .. }
                                if key.machine == target_key.machine
                                    && key.state == target_key.state
                        )
                    })
            })
    };

    // Collect first: allocation mutates the plan we are scanning.
    let mut unserved: Vec<(
        omega_control_flow::StateKey,
        usize,
        StateCallRole,
        usize,
        omega_control_flow::StateKey,
        u32,
    )> = Vec::new();
    for (_, state_call) in context.state_calls.calls.iter() {
        if !matches!(
            state_call.role,
            StateCallRole::AssignmentValue
                | StateCallRole::CallArgument
                | StateCallRole::TransitionArgument
                | StateCallRole::TransitionGuard
        ) {
            continue;
        }
        if !callee_reenters_its_entry(state_call.target_key) {
            continue;
        }
        // A slot anywhere means the call is served; a FIELD-bound result is
        // served slotless via the machine-place fallback.
        if plan
            .state_call_result_slot_any_role_by_ordinal(
                state_call.source_key,
                state_call.statement_index,
                state_call.call_ordinal,
            )
            .is_some()
        {
            continue;
        }
        // LET-bound vs FIELD-bound is an AST question, not a liveness one: a
        // `let r = self.f(..)` whose only later use is a call ARGUMENT has its
        // LocalStorage slot elided (the liveness scan skips later-`let` values,
        // expecting the alias fold to cover them) -- but a call-initialized
        // local is deliberately NEVER aliased (it "resolves to its call-result
        // slot"), so gating on `state_storage.locals` left exactly that shape
        // slotless and its downstream name-reads dangling (the bind-first-arg
        // face). A FIELD-bound result (`self.x = self.f(..)`, multi_arm) is an
        // Assignment statement and still returns None here, preserving the
        // machine-place-fallback serve this filter exists to protect.
        let named_result = super::body::call_result_slot_symbol_and_name(
            context,
            state_call.source_key,
            state_call.statement_index,
            state_call.role,
        );
        let binds_local =
            statement_binds_local(context, state_call.source_key, state_call.statement_index);
        if state_call.role == StateCallRole::AssignmentValue
            && named_result.is_none()
            && !binds_local
        {
            continue;
        }
        // The continuation segment's dispatch namespace: find the body whose
        // key is the caller's state at segment_index + 1 (fall back to the
        // call segment's own body when the caller has no continuation).
        let continuation_segment = context
            .state_calls
            .calls
            .iter()
            .map(|(_, call)| call)
            .filter(|call| {
                call.source_key.machine == state_call.source_key.machine
                    && call.source_key.state == state_call.source_key.state
                    && call.required
                    && callee_reenters_its_entry(call.target_key)
                    && (call.statement_index, call.call_ordinal)
                        <= (state_call.statement_index, state_call.call_ordinal)
            })
            .count();
        let continuation_dispatch = context
            .runtime_bodies
            .bodies
            .iter()
            .find(|(_, body)| {
                body.key.machine == state_call.source_key.machine
                    && body.key.state == state_call.source_key.state
                    && body.key.segment_index == continuation_segment
            })
            .or_else(|| {
                context.runtime_bodies.bodies.iter().find(|(_, body)| {
                    body.key.machine == state_call.source_key.machine
                        && body.key.state == state_call.source_key.state
                        && body.key.segment_index == state_call.source_key.segment_index
                })
            })
            .map(|(_, body)| body.dispatch_index);
        let Some(dispatch_index) = continuation_dispatch else {
            continue;
        };
        unserved.push((
            state_call.source_key,
            state_call.statement_index,
            state_call.role,
            state_call.call_ordinal,
            state_call.target_key,
            dispatch_index,
        ));
    }

    for (source_key, statement_index, role, call_ordinal, target_key, dispatch_index) in unserved {
        // Extend that dispatch namespace's frame from its current extent.
        let mut next_frame_offset = plan
            .frame_slots
            .iter()
            .filter(|(_, slot)| slot.dispatch_index == dispatch_index)
            .map(|(_, slot)| slot.byte_offset + slot.byte_size)
            .max()
            .unwrap_or(0);
        super::body::append_state_call_result_slot_for_plan(
            context,
            plan,
            &mut next_frame_offset,
            dispatch_index,
            source_key,
            statement_index,
            role,
            call_ordinal,
            target_key,
        );
    }
}

fn statement_binds_local(
    context: &RuntimeStorageContext,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
) -> bool {
    context
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)
        .and_then(|machine| {
            context
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == source_key.state)
        })
        .and_then(|state| {
            context
                .program
                .statement_table
                .statements(state.statement_nodes)
                .get(statement_index)
        })
        .is_some_and(|statement| {
            matches!(
                statement,
                psi_checked_trees::statement::StatementNode::LocalData(_)
            )
        })
}

fn reserve_frame_scratch_region(plan: &mut RuntimeStoragePlan) {
    let slots_extent = plan
        .frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0);
    if slots_extent == 0 {
        return;
    }

    let alignment = runtime_frame_storage_alignment(plan);
    plan.frame_scratch_base = align_to(slots_extent, alignment);
    plan.frame_scratch_size = slots_extent;
}

/// Reserve the wire NESTED-MESSAGE / REPEATED-FIELD scratch region (chapter
/// 20): a 16-byte `{ptr, len}` descriptor plus a staging buffer sized for
/// the largest length-delimited payload's worst-case body -- the largest of
/// the nested sub-messages' scalar bodies and the repeated fields' packed
/// element runs -- placed ABOVE every real slot and the argument-staging
/// scratch. Reserved whenever any wire schema's current era declares a
/// physically relevant field of either kind; erased fields retain semantic
/// identity but need no descriptor or staging bytes. A declared-but-never-
/// called schema overallocates a few dozen frame bytes, which is cheaper than
/// scanning every statement for encode/decode calls here. Call AFTER the frame
/// layout is final (post call-context stacking).
pub fn reserve_wire_nested_scratch(
    plan: &mut RuntimeStoragePlan,
    program: &psi_checked_trees::CheckedTrees,
) {
    use psi_checked_trees::wire::WireMember;

    let mut staging_bytes = 0usize;
    let mut needs_wire_scratch = false;
    for schema in program.wire_schemas() {
        for member in program.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            if let Some(repeated) = program.wire_field_repeated_encoding(field) {
                needs_wire_scratch = true;
                staging_bytes = staging_bytes.max(repeated.worst_case_body_bytes());
                continue;
            }
            let Some(child) = program.wire_field_nested_schema(field) else {
                continue;
            };
            let Some(child_worst) = program.wire_schema_scalar_body_worst_case(child) else {
                continue;
            };
            needs_wire_scratch = true;
            staging_bytes = staging_bytes.max(child_worst);
        }
    }
    if !needs_wire_scratch {
        return;
    }

    let occupied_extent = (plan.frame_scratch_base + plan.frame_scratch_size).max(
        plan.frame_slots
            .iter()
            .map(|(_, slot)| slot.byte_offset + slot.byte_size)
            .max()
            .unwrap_or(0),
    );
    // The descriptor's two 8-byte halves need 8-byte alignment.
    plan.wire_scratch_base = align_to(occupied_extent.max(8), 8);
    plan.wire_scratch_size = 16 + staging_bytes;
}

/// Reserve one native word per argument position for scalar expressions that
/// must be evaluated before a host operation marshals its arguments. The
/// region is reused across statements after each call consumes its values.
pub fn reserve_host_argument_scratch(plan: &mut RuntimeStoragePlan, slot_count: usize) {
    if slot_count == 0 {
        return;
    }
    let occupied_extent = (plan.wire_scratch_base + plan.wire_scratch_size)
        .max(plan.frame_scratch_base + plan.frame_scratch_size)
        .max(
            plan.frame_slots
                .iter()
                .map(|(_, slot)| slot.byte_offset + slot.byte_size)
                .max()
                .unwrap_or(0),
        );
    plan.host_argument_scratch_base = align_to(occupied_extent.max(8), 8);
    plan.host_argument_scratch_size = slot_count.saturating_mul(8);
}

/// Reserve the ENTRY-ARGUMENT SPILL region when the entry state declares the
/// bytes-handoff signature (`run(&self, args: &[u8])` -- exactly one non-self
/// parameter whose frame slot is the 16-byte `&[u8]` slice descriptor). The
/// entry prologue spills the platform's four argument registers here and binds
/// `args` = {ptr -> spill, len 32}; the region lives ABOVE every other
/// reservation so nothing later clobbers bytes `args` still references.
pub fn reserve_entry_argument_spill(
    plan: &mut RuntimeStoragePlan,
    entry_key: omega_control_flow::StateKey,
) {
    // EXACT key match (segment included): case-payload bindings are Parameter
    // slots in later segments of the entry state, not platform entry arguments.
    let mut entry_parameters = plan.frame_slots.iter().filter(|(_, slot)| {
        matches!(slot.kind, crate::RuntimeFrameSlotKind::Parameter) && slot.source_key == entry_key
    });
    let Some((_, only)) = entry_parameters.next() else {
        return;
    };
    if entry_parameters.next().is_some()
        || only.byte_size != 16
        || only.type_name.as_ref() != "&[u8]"
    {
        return;
    }

    let occupied_extent = (plan.wire_scratch_base + plan.wire_scratch_size)
        .max(plan.host_argument_scratch_base + plan.host_argument_scratch_size)
        .max(plan.frame_scratch_base + plan.frame_scratch_size)
        .max(
            plan.frame_slots
                .iter()
                .map(|(_, slot)| slot.byte_offset + slot.byte_size)
                .max()
                .unwrap_or(0),
        );
    plan.entry_argument_spill_base = align_to(occupied_extent.max(8), 8);
    plan.entry_argument_spill_size = 32;
}

/// Reserve one frame word for an incoming native indirect-result destination.
/// The caller decides whether the entry signature has that ABI shape; keeping
/// the reservation mechanism policy-neutral avoids teaching storage layout
/// how to classify calling conventions.
pub fn reserve_entry_indirect_result_pointer(plan: &mut RuntimeStoragePlan, enabled: bool) {
    if !enabled {
        return;
    }
    let occupied_extent = (plan.entry_argument_spill_base + plan.entry_argument_spill_size)
        .max(plan.host_argument_scratch_base + plan.host_argument_scratch_size)
        .max(plan.wire_scratch_base + plan.wire_scratch_size)
        .max(plan.frame_scratch_base + plan.frame_scratch_size)
        .max(
            plan.frame_slots
                .iter()
                .map(|(_, slot)| slot.byte_offset + slot.byte_size)
                .max()
                .unwrap_or(0),
        );
    plan.entry_indirect_result_pointer_base = align_to(occupied_extent.max(8), 8);
    plan.entry_indirect_result_pointer_size = 8;
}

/// Reserve a layout-sized frame place where entry-terminal expressions can
/// reuse ordinary place-writing lowering before the value is copied into its
/// normalized ABI result placement.
pub fn reserve_entry_result_scratch(plan: &mut RuntimeStoragePlan, layout: Option<(usize, usize)>) {
    let Some((byte_size, alignment)) =
        layout.filter(|(size, alignment)| *size > 0 && *alignment > 0)
    else {
        return;
    };
    let occupied_extent = (plan.entry_indirect_result_pointer_base
        + plan.entry_indirect_result_pointer_size)
        .max(plan.entry_argument_spill_base + plan.entry_argument_spill_size)
        .max(plan.host_argument_scratch_base + plan.host_argument_scratch_size)
        .max(plan.wire_scratch_base + plan.wire_scratch_size)
        .max(plan.frame_scratch_base + plan.frame_scratch_size)
        .max(
            plan.frame_slots
                .iter()
                .map(|(_, slot)| slot.byte_offset + slot.byte_size)
                .max()
                .unwrap_or(0),
        );
    plan.entry_result_scratch_base = align_to(occupied_extent, alignment);
    plan.entry_result_scratch_size = byte_size;
}

pub fn runtime_frame_storage_size(plan: &RuntimeStoragePlan) -> usize {
    let slots_extent = plan
        .frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0);
    // Include every reserved region above the real slots, including the saved
    // incoming indirect-result pointer.
    slots_extent
        .max(plan.frame_scratch_base + plan.frame_scratch_size)
        .max(plan.wire_scratch_base + plan.wire_scratch_size)
        .max(plan.host_argument_scratch_base + plan.host_argument_scratch_size)
        .max(plan.entry_argument_spill_base + plan.entry_argument_spill_size)
        .max(plan.entry_indirect_result_pointer_base + plan.entry_indirect_result_pointer_size)
        .max(plan.entry_result_scratch_base + plan.entry_result_scratch_size)
}

pub fn runtime_frame_storage_alignment(plan: &RuntimeStoragePlan) -> usize {
    let slot_alignment = plan
        .frame_slots
        .iter()
        .map(|(_, slot)| slot.alignment)
        .max()
        .unwrap_or(1);
    let reserved_alignment = if plan.entry_argument_spill_size > 0
        || plan.entry_indirect_result_pointer_size > 0
        || plan.entry_result_scratch_size > 0
        || plan.host_argument_scratch_size > 0
        || plan.wire_scratch_size > 0
    {
        8
    } else {
        1
    };
    slot_alignment.max(reserved_alignment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::wire::{WireField, WireMember, WireSchema};
    use psi_checked_trees::{CheckedTrees, name::Identifier, types::TypeReferenceNode};
    use psi_language_core::BindingRelevance;

    #[test]
    fn indirect_result_pointer_is_reserved_above_entry_spill() {
        let mut plan = RuntimeStoragePlan {
            entry_argument_spill_base: 24,
            entry_argument_spill_size: 32,
            ..RuntimeStoragePlan::default()
        };

        reserve_entry_indirect_result_pointer(&mut plan, true);

        assert_eq!(plan.entry_indirect_result_pointer_base, 56);
        assert_eq!(plan.entry_indirect_result_pointer_size, 8);
        assert_eq!(runtime_frame_storage_size(&plan), 64);
        assert_eq!(runtime_frame_storage_alignment(&plan), 8);
    }

    #[test]
    fn host_argument_scratch_reserves_one_word_per_argument() {
        let mut plan = RuntimeStoragePlan {
            wire_scratch_base: 24,
            wire_scratch_size: 19,
            ..RuntimeStoragePlan::default()
        };

        reserve_host_argument_scratch(&mut plan, 3);

        assert_eq!(plan.host_argument_scratch_base, 48);
        assert_eq!(plan.host_argument_scratch_size, 24);
        assert_eq!(runtime_frame_storage_size(&plan), 72);
        assert_eq!(runtime_frame_storage_alignment(&plan), 8);
    }

    #[test]
    fn entry_result_scratch_is_reserved_above_indirect_result_pointer() {
        let mut plan = RuntimeStoragePlan {
            entry_indirect_result_pointer_base: 56,
            entry_indirect_result_pointer_size: 8,
            ..RuntimeStoragePlan::default()
        };

        reserve_entry_result_scratch(&mut plan, Some((24, 8)));

        assert_eq!(plan.entry_result_scratch_base, 64);
        assert_eq!(plan.entry_result_scratch_size, 24);
        assert_eq!(runtime_frame_storage_size(&plan), 88);
        assert_eq!(runtime_frame_storage_alignment(&plan), 8);
    }

    #[test]
    fn erased_nested_field_does_not_reserve_wire_scratch() {
        let mut program = CheckedTrees::default();
        let child_type = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated("Child"),
            });
        let child_members = program.typed.append_wire_members(Vec::new());
        program.typed.push_wire_schema(WireSchema {
            name: Identifier::generated("Child"),
            members: child_members,
            ..WireSchema::default()
        });
        let parent_members =
            program
                .typed
                .append_wire_members(vec![WireMember::Field(WireField {
                    number: 1,
                    name: Identifier::generated("child"),
                    relevance: BindingRelevance::Erased,
                    type_reference: child_type,
                })]);
        program.typed.push_wire_schema(WireSchema {
            name: Identifier::generated("Parent"),
            members: parent_members,
            ..WireSchema::default()
        });
        let mut plan = RuntimeStoragePlan::default();

        reserve_wire_nested_scratch(&mut plan, &program);

        assert_eq!(plan.wire_scratch_size, 0);
    }
}
