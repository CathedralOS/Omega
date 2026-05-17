mod body;
mod context;
mod layout;
mod model;

pub use context::RuntimeStorageContext;
pub use model::{RuntimeFrameSlot, RuntimeFrameSlotKind, RuntimeStoragePlan, RuntimeStorageWrite};

use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use std::sync::Arc;

use body::build_runtime_storage_body_plan;

pub fn build_runtime_storage_plan(context: RuntimeStorageContext) -> RuntimeStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_storage_plan_with_workers(Arc::new(context), workers.handle())
}

pub fn build_runtime_storage_plan_with_workers(
    context: Arc<RuntimeStorageContext>,
    workers: WorkerPoolHandle,
) -> RuntimeStoragePlan {
    if context.runtime_bodies.bodies.is_empty() {
        return RuntimeStoragePlan::default();
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

    let mut plan = RuntimeStoragePlan::default();

    for body_plan in body_plans {
        plan.frame_slots.insert_many(
            body_plan
                .frame_slots
                .iter()
                .map(|(_, frame_slot)| frame_slot.clone()),
        );
        for (_, write) in body_plan.writes.iter() {
            plan.writes.append(RuntimeStorageWrite {
                target: plan
                    .expressions
                    .copy_from(&body_plan.expressions, write.target),
                value: plan
                    .expressions
                    .copy_from(&body_plan.expressions, write.value),
                ..write.clone()
            });
        }
    }

    plan
}

pub fn runtime_frame_storage_size(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0)
}

pub fn runtime_frame_storage_alignment(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.alignment)
        .max()
        .unwrap_or(1)
}
