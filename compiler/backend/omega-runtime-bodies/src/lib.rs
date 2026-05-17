mod collection;
mod context;
mod lookups;
mod model;

use collection::build_dispatch_body;
pub use context::RuntimeDispatchBodyContext;
pub use model::{
    RuntimeDispatchBody, RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
    RuntimeDispatchBodyPlan,
};
use omega_core::arena::Arena;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use std::sync::Arc;

pub fn build_runtime_dispatch_body_plan(
    context: RuntimeDispatchBodyContext,
) -> RuntimeDispatchBodyPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_dispatch_body_plan_with_workers(Arc::new(context), workers.handle())
}

pub fn build_runtime_dispatch_body_plan_with_workers(
    context: Arc<RuntimeDispatchBodyContext>,
    workers: WorkerPoolHandle,
) -> RuntimeDispatchBodyPlan {
    if context.state_dispatch.states.is_empty() {
        return RuntimeDispatchBodyPlan::default();
    }

    let state_count = context.state_dispatch.states.len();
    let context_for_bodies = Arc::clone(&context);
    let collected_bodies = workers.map_ordered(state_count, move |index| {
        let dispatch_state = context_for_bodies
            .state_dispatch
            .states
            .storage_slice()
            .get(index)
            .expect("runtime-body worker index should be in range");

        build_dispatch_body(&context_for_bodies, dispatch_state)
    });

    let mut plan = RuntimeDispatchBodyPlan {
        bodies: Arena::with_capacity(state_count),
        ..RuntimeDispatchBodyPlan::default()
    };

    for collected_body in collected_bodies {
        let operations = plan
            .operations
            .insert_many(collected_body.operations.iter().map(|(_, operation)| {
                RuntimeDispatchBodyOperation {
                    kind: match operation.kind {
                        RuntimeDispatchBodyOperationKind::LocalStorage {
                            symbol,
                            ref name,
                            type_symbol,
                            type_reference,
                            invariant_names,
                        } => RuntimeDispatchBodyOperationKind::LocalStorage {
                            symbol,
                            name: name.clone(),
                            type_symbol,
                            type_reference: plan.type_references.copy_from(
                                &collected_body.type_references,
                                &collected_body.expressions,
                                &mut plan.expressions,
                                type_reference,
                            ),
                            invariant_names: plan.invariant_names.insert_many(
                                collected_body
                                    .invariant_names
                                    .span_or_empty(invariant_names)
                                    .iter()
                                    .cloned(),
                            ),
                        },
                        RuntimeDispatchBodyOperationKind::StateCallResult {
                            role,
                            call_ordinal,
                            target_key,
                            value,
                        } => RuntimeDispatchBodyOperationKind::StateCallResult {
                            role,
                            call_ordinal,
                            target_key,
                            value: plan
                                .expressions
                                .copy_from(&collected_body.expressions, value),
                        },
                        ref kind => kind.clone(),
                    },
                    source_key: operation.source_key,
                    statement_index: operation.statement_index,
                }
            }));

        plan.bodies.insert(RuntimeDispatchBody {
            key: collected_body.key,
            dispatch_index: collected_body.dispatch_index,
            operations,
        });
    }

    plan
}
