use crate::body::{build_runtime_storage_body_plan, build_straight_line_runtime_storage_plan};
use crate::layout::align_to;
use crate::{RuntimeStorageContext, RuntimeStoragePlan, RuntimeStorageWrite};
use omega_checked_trees::expression::ExpressionTableCapacity;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
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
            entry_argument_spill_base: _,
            entry_argument_spill_size: _,
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

    reserve_frame_scratch_region(&mut plan);
    plan
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
/// scratch. Reserved whenever any wire schema's current era declares such a
/// field -- a declared-but-never-called schema overallocates a few dozen
/// frame bytes, which is cheaper than scanning every statement for
/// encode/decode calls here. Call AFTER the frame layout is final (post
/// call-context stacking).
pub fn reserve_wire_nested_scratch(
    plan: &mut RuntimeStoragePlan,
    program: &omega_checked_trees::CheckedTrees,
) {
    use omega_checked_trees::wire::WireMember;

    let mut staging_bytes = 0usize;
    for schema in program.wire_schemas() {
        for member in program.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if let Some(repeated) = program.wire_field_repeated_encoding(field) {
                staging_bytes = staging_bytes.max(repeated.worst_case_body_bytes());
                continue;
            }
            let Some(child) = program.wire_field_nested_schema(field) else {
                continue;
            };
            let Some(child_worst) = program.wire_schema_scalar_body_worst_case(child) else {
                continue;
            };
            staging_bytes = staging_bytes.max(child_worst);
        }
    }
    if staging_bytes == 0 {
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
        matches!(slot.kind, crate::RuntimeFrameSlotKind::Parameter)
            && slot.source_key == entry_key
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

pub fn runtime_frame_storage_size(plan: &RuntimeStoragePlan) -> usize {
    let slots_extent = plan
        .frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0);
    // Include the reserved argument-staging scratch region, which lives ABOVE all
    // real slots (see stack_runtime_storage_by_call_context), and the wire
    // nested-message scratch above that.
    slots_extent
        .max(plan.frame_scratch_base + plan.frame_scratch_size)
        .max(plan.wire_scratch_base + plan.wire_scratch_size)
        .max(plan.entry_argument_spill_base + plan.entry_argument_spill_size)
}

pub fn runtime_frame_storage_alignment(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.alignment)
        .max()
        .unwrap_or(1)
}
