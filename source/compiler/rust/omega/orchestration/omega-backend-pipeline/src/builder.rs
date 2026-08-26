use super::callback_thunks::plan_callback_thunks;
use super::entry::resolve_backend_entry_point;
use super::skeleton::{BackendPlanSkeletonInput, build_backend_plan_skeleton};
use super::timing::record_backend_phase;
use omega_abstract_operations_to_target_operations::build_target_operation_plan;
use omega_assigned_target_operations_to_machine_instructions::build_machine_instructions;
use omega_backend_plan::{BackendPlan, BoundNominalCallbackPlacement};
use omega_calling_conventions::build_host_abi_plan;
use omega_control_flow::ControlFlowPlan;
use omega_control_flow_to_abstract_operations::{
    AbstractOperationLoweringInput, build_abstract_operation_plan,
};
use omega_core::parallel::WorkerPoolHandle;
use omega_data_planning::build_target_data_plan_with_dynamic_conformances;
use omega_layout::build_layout_plan;
use omega_machine_emission::{MachineEmissionInput, emit_machine_bytes};
use omega_object_file::entry_symbol_name;
use omega_object_file_planning::{ObjectPlanningInput, build_object_plan};
use omega_platform_interface::build_host_call_plan_with_workers;
use omega_relocations::{RelocationPlanningInput, build_relocation_plan};
use omega_runtime_abi::build_runtime_abi_plan;
use omega_runtime_bodies::{
    RuntimeDispatchBodyContext, build_runtime_dispatch_body_plan_with_workers,
};
use omega_runtime_branching::{RuntimeBranchingContext, build_runtime_branching_call_plan};
use omega_runtime_dispatch_loop::{
    RuntimeDispatchLoopContext, build_runtime_dispatch_loop_plan_with_workers,
};
use omega_runtime_storage::{
    RuntimeStorageContext, build_runtime_storage_plan_with_workers, reserve_entry_argument_spill,
    reserve_entry_indirect_result_pointer, reserve_entry_result_scratch,
    reserve_host_argument_scratch, reserve_wire_nested_scratch, runtime_frame_storage_alignment,
    runtime_frame_storage_size,
};
use omega_runtime_text::build_runtime_text_plan;
use omega_state_calls::{
    StateCallLowering, StateCallPlan, StateCallPlanningContext, StateCallRole,
    build_alias_flow_plan, build_state_call_plan_with_workers,
};
use omega_state_dispatch::{StateDispatchContext, build_state_dispatch_plan_with_workers};
use omega_state_graph::{
    RuntimeFlowPlan, RuntimeStateCallEdge, build_runtime_flow_plan,
    build_runtime_flow_plan_with_state_calls,
};
use omega_state_guards::build_state_guard_plan;
use omega_state_storage::{StateStoragePlanningContext, build_state_storage_plan_with_workers};
use omega_state_values::{StateValuePlanningContext, build_state_value_plan_with_workers};
use omega_target::{NativeTarget, TargetProfile};
use omega_target_operations_to_assigned_target_operations::build_assigned_target_operations;
use psi_arena::Arena;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(super) fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<CheckedTrees>,
    selected_provider_plans: Arc<omega_effects::SelectedProviderPlanFacts>,
    entry_machine_name: Option<&str>,
    entry_boundary_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    callback_placements: Arc<[BoundNominalCallbackPlacement]>,
    target_profile: TargetProfile,
    freestanding: bool,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    let target = target_profile.native_target();
    let entry_point = resolve_backend_entry_point(&program, entry_machine_name)?;
    let callback_thunks = plan_callback_thunks(&control_flow, &callback_placements)?;
    let mut phase_timings = Arena::new();
    // A freestanding build trusts no ambient host boundary packages: start
    // from an empty ABI plan, so there are no implicit bindings, platform
    // lowerings, or downstream import thunks. Image subsystem is orthogonal.
    let host_abi = record_backend_phase(&mut phase_timings, "host abi", || {
        if freestanding {
            // Source external leaves become bindings and call lowerings
            // (including table dispatch for UEFI protocols).
            omega_calling_conventions::build_freestanding_abi_plan(target, external_binding_rows)
        } else {
            // Hosted targets consume selected external leaves additively;
            // colliding with a built-in operation is a loud error. Only rows
            // whose target identifier resolves to this compile target apply.
            let target_rows: Vec<_> = external_binding_rows
                .iter()
                .filter(|row| {
                    TargetProfile::from_omega_target_name(Some(&row.target_name))
                        .is_ok_and(|row_profile| row_profile == target_profile)
                })
                .cloned()
                .collect();
            let mut plan = build_host_abi_plan(target);
            omega_calling_conventions::merge_external_binding_rows(&mut plan, &target_rows)?;
            Ok(plan)
        }
    })
    .map_err(Diagnostic::error)?;
    let host_abi = Arc::new(host_abi);
    let host_call_program = Arc::clone(&program);
    let layout_program = Arc::clone(&program);
    let host_call_abi = Arc::clone(&host_abi);
    let host_call_workers = workers.clone();
    let (layouts, host_calls) =
        record_backend_phase(&mut phase_timings, "layout/host calls", || {
            workers.join2(
                move || build_layout_plan(&layout_program, target),
                move || {
                    build_host_call_plan_with_workers(
                        host_call_program,
                        target,
                        host_call_abi,
                        host_call_workers,
                    )
                },
            )
        });
    let layouts = layouts?;
    let host_calls = host_calls?;
    let host_calls = Arc::new(host_calls);
    // The field model's offset resolution: attached external leaves carry the
    // table type and field names; the layout plan owns their byte offsets.
    // Resolve before any phase copies bindings out of the ABI plan.
    let host_abi = record_backend_phase(&mut phase_timings, "vtable field offsets", || {
        resolve_vtable_field_offsets(host_abi, &layouts)
    })
    .map_err(Diagnostic::error)?;
    let entry_key = control_flow
        .state_key_by_symbols(entry_point.machine_symbol, entry_point.state_symbol)
        .ok_or_else(|| Diagnostic::error("unknown runtime entry state"))?;
    let seed_runtime_flow = record_backend_phase(&mut phase_timings, "runtime flow/seed", || {
        build_runtime_flow_plan(&control_flow, entry_key)
    })?;
    let seed_runtime_flow = Arc::new(seed_runtime_flow);
    let state_calls = Arc::new(record_backend_phase(
        &mut phase_timings,
        "state calls/seed",
        || {
            build_state_call_plan_with_workers(
                Arc::new(StateCallPlanningContext {
                    control_flow: Arc::clone(&control_flow),
                    host_calls: Arc::clone(&host_calls),
                    runtime_flow: Arc::clone(&seed_runtime_flow),
                }),
                workers.clone(),
            )
        },
    ));
    // FIXPOINT (2026-07-08l): dispatch edges and the state-call plan feed each
    // other -- `dispatch_state_call_edges` filters on `required`, which the
    // plan computes from the runtime flow, which is built from the edges. One
    // round bakes the SEED flow's under-approximated `required` into the edge
    // set: a root-level call routes, but the callee's INTERIOR calls (e.g.
    // create_dir_all -> mkall_walk -> mkall_copy) are only marked required by
    // the rebuild AFTER the edges were fixed, so looping interior callees
    // never dispatched. Iterate until the edge set stabilizes -- monotone
    // (flow states only grow, so `required` only grows, so edges only grow;
    // equal COUNT therefore means equal SET) and bounded by the static call
    // count, so it terminates; an empty first round breaks immediately (the
    // old zero-edge short-circuit). Rebuilding the plan each round also
    // re-lowers every call against the clone graph (the historical stale
    // InlineBranching double-lowering fix).
    let mut runtime_flow = seed_runtime_flow;
    let mut state_calls = state_calls;
    let mut previous_edge_count = 0usize;
    loop {
        let runtime_state_call_edges =
            dispatch_state_call_edges(state_calls.as_ref(), &control_flow);
        if runtime_state_call_edges.len() == previous_edge_count {
            break;
        }
        previous_edge_count = runtime_state_call_edges.len();
        runtime_flow = Arc::new(record_backend_phase(
            &mut phase_timings,
            "runtime flow/state calls",
            || {
                build_runtime_flow_plan_with_state_calls(
                    &control_flow,
                    entry_key,
                    &runtime_state_call_edges,
                )
            },
        )?);
        state_calls = Arc::new(record_backend_phase(
            &mut phase_timings,
            "state calls",
            || {
                build_state_call_plan_with_workers(
                    Arc::new(StateCallPlanningContext {
                        control_flow: Arc::clone(&control_flow),
                        host_calls: Arc::clone(&host_calls),
                        runtime_flow: Arc::clone(&runtime_flow),
                    }),
                    workers.clone(),
                )
            },
        ));
    }
    let runtime_flow = runtime_flow;
    let state_calls = state_calls;
    let state_dispatch = record_backend_phase(&mut phase_timings, "state dispatch", || {
        build_state_dispatch_plan_with_workers(
            Arc::new(StateDispatchContext::from_runtime_flow(Arc::clone(
                &runtime_flow,
            ))),
            workers.clone(),
        )
    });
    let mut backend_plan = build_backend_plan_skeleton(BackendPlanSkeletonInput {
        target_profile,
        target,
        host_abi: Arc::clone(&host_abi),
        host_calls: Arc::try_unwrap(host_calls).unwrap_or_else(|host_calls| (*host_calls).clone()),
        control_flow: Arc::clone(&control_flow),
        runtime_flow: Arc::clone(&runtime_flow),
        state_dispatch,
        state_guards: Default::default(),
        layouts,
        entry_key,
        entry_boundary_plan,
        callback_placements,
        callback_thunks,
        phase_timings,
    });
    let mut phase_timings = std::mem::take(&mut backend_plan.phase_timings);
    backend_plan.state_calls = state_calls;
    backend_plan.alias_flow = record_backend_phase(&mut phase_timings, "alias flow", || {
        build_alias_flow_plan(backend_plan.state_calls.as_ref())
    });
    let state_storage_program = Arc::clone(&program);
    let state_values_program = Arc::clone(&program);
    let state_storage_context = Arc::new(StateStoragePlanningContext {
        control_flow: Arc::clone(&control_flow),
        runtime_flow: Arc::clone(&backend_plan.runtime_flow),
        state_calls: Arc::clone(&backend_plan.state_calls),
    });
    let state_values_context = Arc::new(StateValuePlanningContext {
        runtime_flow: Arc::clone(&backend_plan.runtime_flow),
        state_calls: Arc::clone(&backend_plan.state_calls),
    });
    // These planners both fan out over the shared worker pool. Running their
    // outer calls concurrently makes the phase impossible to attribute and
    // lets the small storage pass contend with the recursive value
    // simplifier. Keep the planners individually parallel, but time their
    // independent outer phases in order.
    let state_storage = record_backend_phase(&mut phase_timings, "state storage", || {
        build_state_storage_plan_with_workers(
            state_storage_program,
            state_storage_context,
            workers.clone(),
        )
    });
    let state_values = record_backend_phase(&mut phase_timings, "state values", || {
        build_state_value_plan_with_workers(
            state_values_program,
            state_values_context,
            workers.clone(),
        )
    });
    backend_plan.state_storage = Arc::new(state_storage);
    backend_plan.state_values = state_values;
    backend_plan.runtime_bodies = Arc::new(record_backend_phase(
        &mut phase_timings,
        "runtime bodies",
        || {
            build_runtime_dispatch_body_plan_with_workers(
                Arc::new(RuntimeDispatchBodyContext::new(
                    Arc::clone(&program),
                    Arc::clone(&control_flow),
                    Arc::clone(&backend_plan.host_calls),
                    Arc::clone(&backend_plan.state_dispatch),
                    Arc::clone(&backend_plan.state_calls),
                    Arc::clone(&backend_plan.state_storage),
                )),
                workers.clone(),
            )
        },
    ));
    let runtime_storage_context = Arc::new(RuntimeStorageContext::new(
        Arc::clone(&program),
        Arc::clone(&control_flow),
        Arc::clone(&backend_plan.layouts),
        Arc::clone(&backend_plan.runtime_bodies),
        Arc::clone(&backend_plan.state_calls),
        Arc::clone(&backend_plan.state_storage),
        backend_plan.target,
    ));
    let runtime_storage_workers = workers.clone();
    backend_plan.runtime_storage =
        record_backend_phase(&mut phase_timings, "runtime storage", || {
            build_runtime_storage_plan_with_workers(
                runtime_storage_context,
                runtime_storage_workers,
            )
        });
    // Each dispatch body lays its frame slots out from offset 0, so a caller's
    // slots and a dispatched callee's slots would otherwise share offsets and
    // clobber each other (e.g. a `&mut` out-parameter that is live across the
    // call). Give each call-context (a specialized clone) a disjoint frame
    // region, stacked so any caller/callee pair is disjoint. States in the same
    // context still share their range (they are never simultaneously live), so
    // entry-only programs are unchanged.
    stack_runtime_storage_by_call_context(&mut backend_plan.runtime_storage, &runtime_flow);
    // Reserve the wire nested-message staging scratch ABOVE the final layout
    // (chapter 20): the encoder stages a nested sub-message here before
    // replaying it (length varint + copy) into the caller's out buffer, and
    // the decoder keeps the sub-region end bound in the same slots.
    reserve_wire_nested_scratch(&mut backend_plan.runtime_storage, &program);
    reserve_host_argument_scratch(
        &mut backend_plan.runtime_storage,
        host_computed_scalar_argument_slot_count(&program, &backend_plan.host_calls),
    );
    // Reserve the entry-argument spill (the bytes handoff `run(&self, args:
    // &[u8])`) ABOVE every other reservation -- args's slice descriptor points
    // at the spilled registers for the program's whole life, so nothing may
    // reuse those bytes.
    reserve_entry_argument_spill(&mut backend_plan.runtime_storage, backend_plan.entry_key);
    reserve_entry_indirect_result_pointer(
        &mut backend_plan.runtime_storage,
        entry_may_need_native_indirect_result_pointer(
            &program,
            &backend_plan.layouts,
            backend_plan.entry_key,
            backend_plan.target,
        ),
    );
    reserve_entry_result_scratch(
        &mut backend_plan.runtime_storage,
        entry_native_expression_result_layout(
            &program,
            &backend_plan.layouts,
            &control_flow,
            backend_plan.entry_key,
        ),
    );
    // Observability: dump the absolute frame-slot layout (which logical slot lives
    // at which runtime byte offset) to stderr when OMEGA_DUMP_SLOTS is set. Inert
    // by default -- env unset is zero output and zero behavior change. Mirrors the
    // `slots.txt` build artifact (see render_frame_slot_table).
    if std::env::var("OMEGA_DUMP_SLOTS").is_ok() {
        eprint!(
            "{}",
            render_frame_slot_table(&backend_plan.runtime_storage, &runtime_flow)
        );
    }
    // PER-INSTANCE RECEIVER BASES (TASKS_FS "Stolen work #2", phase 3): one
    // table, indexed by dispatch index (== the runtime-flow state's arena
    // index), consumed by guard-operand layout, selection, and the
    // contained-receiver fence -- a single prediction site. Dispatch-route
    // consumers only serve when routing agrees (the fence's discipline).
    backend_plan.receiver_bases = compute_receiver_bases(
        &runtime_flow,
        &backend_plan.state_calls,
        &backend_plan.layouts,
    );
    // The guard-operand resolver needs the same dispatch-index -> context table
    // the stacking pass used: a slot lookup that falls back across contexts
    // reads a DIFFERENT INLINING's frame region (the second wrapper dir-walk's
    // tail guard read the first walk's `i`/`path.len` -- the repeated-slice-arg
    // miscompile).
    backend_plan.state_contexts = state_context_table(&runtime_flow);
    backend_plan.state_guards = record_backend_phase(&mut phase_timings, "state guards", || {
        build_state_guard_plan(
            &program,
            backend_plan.state_dispatch.as_ref(),
            &backend_plan.control_flow,
            &backend_plan.layouts,
            &backend_plan.runtime_storage,
            backend_plan.entry_key.machine,
            &backend_plan.receiver_bases,
            &backend_plan.state_contexts,
        )
    })
    .into();
    let runtime_loop_context = Arc::new(RuntimeDispatchLoopContext::from_parts(
        !backend_plan.state_dispatch.states.is_empty(),
        Arc::clone(&backend_plan.state_dispatch),
        backend_plan.entry_key,
        Arc::clone(&backend_plan.state_guards),
        Arc::clone(&backend_plan.runtime_bodies),
    ));
    let runtime_loop_workers = workers.clone();
    backend_plan.runtime_dispatch_loop =
        record_backend_phase(&mut phase_timings, "runtime loop", || {
            build_runtime_dispatch_loop_plan_with_workers(
                runtime_loop_context,
                runtime_loop_workers,
            )
        });
    backend_plan.runtime_branching_calls =
        record_backend_phase(&mut phase_timings, "runtime branching", || {
            build_runtime_branching_call_plan(&RuntimeBranchingContext {
                program: program.as_ref(),
                control_flow: &backend_plan.control_flow,
                host_calls: &backend_plan.host_calls,
                runtime_bodies: &backend_plan.runtime_bodies,
                state_calls: &backend_plan.state_calls,
                state_dispatch: &backend_plan.state_dispatch,
                state_guards: &backend_plan.state_guards,
                state_storage: &backend_plan.state_storage,
            })
        });
    backend_plan.runtime_text = record_backend_phase(&mut phase_timings, "runtime text", || {
        build_runtime_text_plan(&backend_plan.host_calls, &backend_plan.state_storage)
    });
    backend_plan.data = record_backend_phase(&mut phase_timings, "target data", || {
        build_target_data_plan_with_dynamic_conformances(
            program.as_ref(),
            &backend_plan.host_calls,
            &backend_plan.state_storage,
            &backend_plan.state_values,
            &backend_plan.runtime_branching_calls,
            &backend_plan.runtime_text,
            &backend_plan.state_calls,
            build_runtime_abi_plan(backend_plan.target),
        )
    })?;
    backend_plan.abstract_data = (&backend_plan.data).into();
    backend_plan.abstract_operations =
        record_backend_phase(&mut phase_timings, "abstract operations", || {
            let runtime_abi = build_runtime_abi_plan(backend_plan.target);
            build_abstract_operation_plan(&AbstractOperationLoweringInput {
                target: backend_plan.target,
                freestanding,
                receiver_bases: &backend_plan.receiver_bases,
                state_contexts: &backend_plan.state_contexts,
                runtime_abi: &runtime_abi,
                entry_key: backend_plan.entry_key,
                entry_boundary_plan: backend_plan.entry_boundary_plan.as_ref(),
                entry_symbol: entry_symbol_name(backend_plan.target).into(),
                callback_placements: &backend_plan.callback_placements,
                callback_thunks: &backend_plan.callback_thunks,
                program: program.as_ref(),
                selected_provider_plans: selected_provider_plans.as_ref(),
                control_flow: &backend_plan.control_flow,
                host_abi: &backend_plan.host_abi,
                host_calls: &backend_plan.host_calls,
                state_calls: &backend_plan.state_calls,
                alias_flow: &backend_plan.alias_flow,
                state_storage: &backend_plan.state_storage,
                runtime_flow: &backend_plan.runtime_flow,
                runtime_bodies: &backend_plan.runtime_bodies,
                runtime_branching_calls: &backend_plan.runtime_branching_calls,
                runtime_dispatch_loop: &backend_plan.runtime_dispatch_loop,
                runtime_storage: &backend_plan.runtime_storage,
                runtime_text: &backend_plan.runtime_text,
                state_guards: &backend_plan.state_guards,
                layouts: &backend_plan.layouts,
                data: &backend_plan.abstract_data,
            })
        })?;
    backend_plan.target_operations =
        record_backend_phase(&mut phase_timings, "target operations", || {
            build_target_operation_plan(
                backend_plan.target,
                &backend_plan.host_abi,
                &backend_plan.host_calls,
                &backend_plan.abstract_operations,
            )
        });
    backend_plan.assigned_target_operations =
        record_backend_phase(&mut phase_timings, "assigned target operations", || {
            build_assigned_target_operations(&backend_plan.target_operations)
        });
    backend_plan.machine_instructions =
        record_backend_phase(&mut phase_timings, "machine instructions", || {
            build_machine_instructions(&backend_plan.assigned_target_operations)
        })?;
    backend_plan.encoded_machine =
        record_backend_phase(&mut phase_timings, "machine emission", || {
            emit_machine_bytes(MachineEmissionInput {
                target: backend_plan.target,
                assigned_target_operations: &backend_plan.assigned_target_operations,
                machine_instructions: &backend_plan.machine_instructions,
                host_abi: &backend_plan.host_abi,
                data: &backend_plan.data,
                terminal_dispatch_index: backend_plan.runtime_dispatch_loop.terminal_dispatch_index,
            })
        })?;
    backend_plan.object = record_backend_phase(&mut phase_timings, "object plan", || {
        build_object_plan(ObjectPlanningInput {
            target: backend_plan.target,
            host_abi: &backend_plan.host_abi,
            layouts: &backend_plan.layouts,
            entry_machine_symbol: backend_plan.entry_key.machine,
            entry_machine_name: backend_plan.entry_machine_name(),
            // Until a compiler-generated wrapper has a real body, the physical
            // object entry remains the exact selected source continuation.
            entry_function_identity: omega_control_flow::MachineFunctionIdentity::source(
                backend_plan.entry_key,
            ),
            encoded_machine: &backend_plan.encoded_machine,
            data: &backend_plan.data,
            runtime_frame_size: runtime_frame_storage_size(&backend_plan.runtime_storage),
            runtime_frame_alignment: runtime_frame_storage_alignment(&backend_plan.runtime_storage),
        })
    })?;
    backend_plan.relocations = record_backend_phase(&mut phase_timings, "relocations", || {
        build_relocation_plan(RelocationPlanningInput {
            target: backend_plan.target,
            instructions: &backend_plan.target_operations,
            assigned_target_operations: &backend_plan.assigned_target_operations,
            encoded_machine: &backend_plan.encoded_machine,
            data: &backend_plan.data,
            object: &backend_plan.object,
            host_abi: &backend_plan.host_abi,
            entry_machine_name: backend_plan.entry_machine_name(),
        })
    })?;
    backend_plan.phase_timings = phase_timings;

    Ok(backend_plan)
}

/// Conservatively reserve a word for native indirect aggregate results.
/// AAPCS64 HFAs may still use vector registers above 16 bytes; normalized
/// instruction selection recognizes those and simply leaves this reservation
/// unused.
fn entry_may_need_native_indirect_result_pointer(
    program: &CheckedTrees,
    layouts: &omega_layout::LayoutPlan,
    entry_key: omega_control_flow::StateKey,
    target: NativeTarget,
) -> bool {
    if !matches!(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        omega_calling_conventions::CallingPolicy::Aapcs64
            | omega_calling_conventions::CallingPolicy::MicrosoftX64
            | omega_calling_conventions::CallingPolicy::SystemVAMD64
    ) {
        return false;
    }
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == entry_key.machine)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == entry_key.state)
    else {
        return false;
    };
    let result_symbol = program.type_reference_symbol(state.return_type);
    let policy = omega_calling_conventions::CallingPolicy::native_for_target(target);
    layouts.data_layouts.iter().any(|(_, layout)| {
        layout.symbol == result_symbol
            && if policy == omega_calling_conventions::CallingPolicy::MicrosoftX64 {
                !matches!(layout.layout.size, 1 | 2 | 4 | 8)
            } else {
                layout.layout.size > 16
            }
    })
}

fn host_computed_scalar_argument_slot_count(
    program: &CheckedTrees,
    host_calls: &omega_platform_interface::HostCallPlan,
) -> usize {
    host_calls
        .calls
        .iter()
        .filter_map(|(_, call)| {
            let arguments = host_calls.arguments.span(call.arguments)?;
            arguments
                .iter()
                .any(|argument| {
                    matches!(
                        argument.kind,
                        omega_platform_interface::HostCallArgumentKind::Expression(expression)
                            if match host_calls.expressions.expression(expression) {
                                psi_checked_trees::expression::ExpressionNode::Binary(_)
                                | psi_checked_trees::expression::ExpressionNode::Cast(_)
                                | psi_checked_trees::expression::ExpressionNode::Indexed(_) => true,
                                psi_checked_trees::expression::ExpressionNode::Call(call) => [
                                    psi_symbols::BuiltinFunction::Max,
                                    psi_symbols::BuiltinFunction::Min,
                                    psi_symbols::BuiltinFunction::Sqrt,
                                    psi_symbols::BuiltinFunction::FloatIsNan,
                                ]
                                .into_iter()
                                .any(|builtin| {
                                    program.symbols.builtin_function_symbol(builtin)
                                        == Some(call.target_symbol)
                                }),
                                _ => false,
                            }
                    )
                })
                .then_some(arguments.len())
        })
        .max()
        .unwrap_or(0)
}

fn entry_native_expression_result_layout(
    program: &CheckedTrees,
    layouts: &omega_layout::LayoutPlan,
    control_flow: &ControlFlowPlan,
    entry_key: omega_control_flow::StateKey,
) -> Option<(usize, usize)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == entry_key.machine)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == entry_key.state)?;
    let result_symbol = program.type_reference_symbol(state.return_type);
    let has_scratch_terminal = control_flow.transitions.iter().any(|(_, transition)| {
        if !transition.expressions.target_value.is_valid()
            || !matches!(
                transition.target,
                omega_control_flow::PlannedTransitionTarget::Terminal
            )
        {
            return false;
        }
        match control_flow
            .expressions
            .expression(transition.expressions.target_value)
        {
            psi_checked_trees::expression::ExpressionNode::Binary(_)
            | psi_checked_trees::expression::ExpressionNode::Cast(_)
            | psi_checked_trees::expression::ExpressionNode::Float(_)
            | psi_checked_trees::expression::ExpressionNode::Indexed(_)
            | psi_checked_trees::expression::ExpressionNode::StructLiteral(_)
            | psi_checked_trees::expression::ExpressionNode::Unary(_) => true,
            psi_checked_trees::expression::ExpressionNode::Call(call) => [
                psi_symbols::BuiltinFunction::Max,
                psi_symbols::BuiltinFunction::Min,
                psi_symbols::BuiltinFunction::Sqrt,
                psi_symbols::BuiltinFunction::FloatIsNan,
            ]
            .into_iter()
            .any(|builtin| {
                program.symbols.builtin_function_symbol(builtin) == Some(call.target_symbol)
            }),
            psi_checked_trees::expression::ExpressionNode::Name(path) => {
                path.head_symbol == result_symbol
            }
            psi_checked_trees::expression::ExpressionNode::Member(member) => {
                matches!(
                    control_flow.expressions.expression(member.receiver),
                    psi_checked_trees::expression::ExpressionNode::Name(path)
                        if path.symbol == result_symbol
                )
            }
            _ => false,
        }
    });
    if !has_scratch_terminal {
        return None;
    }
    if let Some(byte_size) = program
        .primitive_type_reference(state.return_type)
        .and_then(|primitive| primitive.scalar_byte_size())
    {
        return Some((byte_size, byte_size));
    }
    layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == result_symbol)
        .map(|(_, layout)| (layout.layout.size, layout.layout.alignment))
}

/// Give each call-context (specialized clone) -- and each STATE within a context
/// -- a disjoint frame region so live slots survive both the dispatched calls a
/// caller makes and the sibling-state transitions within one machine. Slots are
/// built per dispatch body from offset 0; this shifts every slot by its
/// (context, state) group's cumulative base.
///
/// The dispatch-index -> call-context table. A frame slot's `dispatch_index` is
/// the state's ARENA INDEX in the runtime flow (see omega-state-dispatch:
/// `dispatch_index = handle.arena_index()`). Index by that arena index
/// explicitly -- iteration position is NOT guaranteed to equal arena index, and
/// conflating them assigns a state's slots to the wrong context's region (a
/// caller and a dispatched callee then overlap and clobber each other's live
/// values). Shared by the frame-region stacking below and the guard-operand
/// resolver (which must never read a slot across contexts).
fn state_context_table(runtime_flow: &RuntimeFlowPlan) -> Vec<u32> {
    let mut contexts: Vec<u32> = Vec::new();
    for (handle, state) in runtime_flow.states.iter() {
        let index = handle.arena_index() as usize;
        if index >= contexts.len() {
            contexts.resize(index + 1, 0);
        }
        contexts[index] = state.context.0;
    }
    contexts
}

/// States within one context must NOT share a range: a state's local stays live
/// across a sibling transition whenever its address escapes (`&mut local` passed
/// as a transition argument). Overlaying siblings let the successor's guard
/// call-results and inlined callee-chain locals land INSIDE the predecessor's
/// still-referenced local, so a write through the forwarded `&mut` clobbered the
/// successor's guard result (and vice versa) -- the dungeon "generation stalls
/// after the first should_carve" bug.
fn stack_runtime_storage_by_call_context(
    storage: &mut omega_runtime_storage::RuntimeStoragePlan,
    runtime_flow: &RuntimeFlowPlan,
) {
    let contexts = state_context_table(runtime_flow);
    if contexts.is_empty() {
        return;
    }
    let context_of = |dispatch_index: u32| -> usize {
        contexts.get(dispatch_index as usize).copied().unwrap_or(0) as usize
    };

    // The largest single state's frame (PRE-stacking, every body lays out from 0)
    // bounds any one transition's packed argument footprint, so it is a safe size
    // for the argument-staging scratch reserved below.
    let max_state_extent = storage
        .frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0);
    let scratch_alignment = storage
        .frame_slots
        .iter()
        .map(|(_, slot)| slot.alignment.max(1))
        .max()
        .unwrap_or(1);

    // Group by (context, dispatch_index) and stack the groups into disjoint
    // regions ordered context-major (ROOT's states first; context ids are minted
    // parent-before-child, so any caller's region precedes its callees').
    let mut groups: std::collections::BTreeMap<(usize, u32), (usize, usize)> =
        std::collections::BTreeMap::new();
    for (_, slot) in storage.frame_slots.iter() {
        let key = (context_of(slot.dispatch_index), slot.dispatch_index);
        let entry = groups.entry(key).or_insert((0usize, 1usize));
        entry.0 = entry.0.max(slot.byte_offset + slot.byte_size);
        entry.1 = entry.1.max(slot.alignment.max(1));
    }

    let mut bases: std::collections::BTreeMap<(usize, u32), usize> =
        std::collections::BTreeMap::new();
    let mut next_base = 0usize;
    for (key, (size, alignment)) in &groups {
        let base = next_base.next_multiple_of(*alignment);
        bases.insert(*key, base);
        next_base = base + size;
    }

    let handles: Vec<_> = storage
        .frame_slots
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    for handle in handles {
        let slot = storage.frame_slots.get_mut(handle);
        let key = (context_of(slot.dispatch_index), slot.dispatch_index);
        let base = bases.get(&key).copied().unwrap_or(0);
        slot.byte_offset = slot.byte_offset.saturating_add(base);
    }

    // Reserve a scratch region ABOVE every stacked context. Argument materialization
    // stages a same-context transition's arguments here (source -> scratch -> target)
    // when the source and target slots overlap, so a write cannot clobber a source
    // that a later argument still needs (a slice/scalar copy cycle).
    if max_state_extent > 0 {
        storage.frame_scratch_base = next_base.next_multiple_of(scratch_alignment);
        storage.frame_scratch_size = max_state_extent;
    }
}

/// Render the absolute frame-slot layout as a plain-text table: one line per
/// frame slot mapping a logical slot (machine/state/param/local) to the runtime
/// byte range it occupies inside the `omega_runtime_frame_storage` region.
///
/// This is the inert-by-default observability side-table. It is emitted to stderr
/// when `OMEGA_DUMP_SLOTS` is set, and written to `slots.txt` in the build dir on
/// the compile-to-disk path. Offsets are POST call-context stacking (the absolute
/// region-relative offsets the running image uses). The region base itself is a
/// relocation resolved in the image; see the header comment in the output for how
/// to recover it. Sorted by (context, dispatch_index, byte_offset).
pub fn render_frame_slot_table(
    storage: &omega_runtime_storage::RuntimeStoragePlan,
    runtime_flow: &RuntimeFlowPlan,
) -> String {
    use omega_runtime_storage::RuntimeFrameSlotKind;

    // dispatch_index (a state's arena index in the runtime flow) -> CallContext id.
    let mut contexts: Vec<u32> = Vec::new();
    for (handle, state) in runtime_flow.states.iter() {
        let index = handle.arena_index() as usize;
        if index >= contexts.len() {
            contexts.resize(index + 1, 0);
        }
        contexts[index] = state.context.0;
    }
    let context_of = |dispatch_index: u32| -> u32 {
        contexts.get(dispatch_index as usize).copied().unwrap_or(0)
    };

    let kind_label = |kind: &RuntimeFrameSlotKind| -> String {
        match kind {
            RuntimeFrameSlotKind::Parameter => "param".to_owned(),
            RuntimeFrameSlotKind::DynamicReceiver { .. } => "dyn-receiver".to_owned(),
            RuntimeFrameSlotKind::DynamicResultScratch { .. } => "dyn-result".to_owned(),
            RuntimeFrameSlotKind::LocalStorage => "local".to_owned(),
            RuntimeFrameSlotKind::StateCallResult {
                role, call_ordinal, ..
            } => format!("call-result({role:?}#{call_ordinal})"),
        }
    };

    // Collect, then sort by (context, dispatch_index, byte_offset) for stable,
    // human-scannable output.
    let mut rows: Vec<(u32, u32, &omega_runtime_storage::RuntimeFrameSlot)> = storage
        .frame_slots
        .iter()
        .map(|(_, slot)| (context_of(slot.dispatch_index), slot.dispatch_index, slot))
        .collect();
    rows.sort_by_key(|(context, dispatch_index, slot)| {
        (*context, *dispatch_index, slot.byte_offset)
    });

    let mut output = String::new();
    output.push_str("# Omega frame-slot layout (region: omega_runtime_frame_storage)\n");
    output.push_str(
        "# absolute runtime address of a slot = (relocated region base) + byte_offset.\n",
    );
    output.push_str(
        "# region base = the imm64 of the `movabsq $imm64,%r15` (frame storage) the dispatch\n",
    );
    output.push_str(
        "#   loop loads, OR the address of the `omega_runtime_frame_storage` symbol in the image.\n",
    );
    output.push_str(
        "# cdb (no-ASLR image base 0x140000000): bp <code>; g; dd (base+byte_offset) L<words>.\n",
    );
    output.push_str(&format!(
        "# frame_scratch_base={} frame_scratch_size={}\n",
        storage.frame_scratch_base, storage.frame_scratch_size
    ));
    output.push_str(&format!("# slots: {}\n", rows.len()));
    output.push_str(
        "# context  dispatch  stmt  machine#  state#  seg  kind                  name                  type                  offset    end       size\n",
    );

    for (context, dispatch_index, slot) in &rows {
        // `usize::MAX` is the sentinel statement index for a parameter slot (it has
        // no owning statement); render it as `-` for readability.
        let stmt = if slot.statement_index == usize::MAX {
            "-".to_owned()
        } else {
            slot.statement_index.to_string()
        };
        output.push_str(&format!(
            "{:<8}  {:<8}  {:<4}  {:<8}  {:<6}  {:<3}  {:<20}  {:<20}  {:<20}  {:<8}  {:<8}  {}\n",
            context,
            dispatch_index,
            stmt,
            slot.source_key.machine.arena_index(),
            slot.source_key.state.arena_index(),
            slot.source_key.segment_index,
            kind_label(&slot.kind),
            slot.name.as_str(),
            slot.type_name.as_ref(),
            slot.byte_offset,
            slot.byte_offset + slot.byte_size,
            slot.byte_size,
        ));
    }

    output
}

fn dispatch_state_call_edges(
    state_calls: &StateCallPlan,
    control_flow: &ControlFlowPlan,
) -> Vec<RuntimeStateCallEdge> {
    state_calls
        .calls
        .iter()
        .filter_map(|(_, state_call)| {
            // Lower a statement state call as a dispatch transition (rather than
            // inline-expanding it) when it has no arguments OR when its callee
            // contains a loop. Inlining a callee that loops would unroll it, which
            // mishandles the loop-carried variable; dispatching makes the callee's
            // states real dispatch cases (the loop a back-edge) with stable
            // parameter slots. Acyclic calls keep inlining (works, and the
            // dispatch path doesn't yet carry return values for them).
            //
            // A LOOPING callee must dispatch REGARDLESS of role: a value-position
            // call (`let n = count(s[1..], acc+1)`) whose callee loops would
            // otherwise inline-UNROLL and return 0. Dispatching it makes the loop
            // a real back-edge, and the dispatch terminal writes the callee's
            // return value back to the caller's call-result slot (see
            // select_runtime_dispatch_call_result_return). Non-looping Statement
            // calls still inline (acyclic, works).
            //
            // NOTE: dispatching NON-looping value calls broadly regresses ~13
            // canaries -- the inline-branching value path handles shapes (binary
            // operands, reference/slice-element results, aliases, multi-arm) the
            // dispatch return-write does not yet serve. A value call with a
            // runtime-guarded arm (`let n = classify(s)`) is still mishandled
            // inline (returns 0); fixing it needs the dispatch return-write to
            // cover those shapes first, or a runtime value-select in the inline
            // path. See [[inline-branching-value-runtime-guard]].
            (state_call.required
                && (state_call_target_loops(control_flow, state_calls, state_call.target_key)
                    || (state_call.role == StateCallRole::Statement
                        && matches!(
                            state_call.lowering,
                            StateCallLowering::InlineBranching | StateCallLowering::InlineExpansion
                        ))))
            .then_some(RuntimeStateCallEdge {
                source_key: state_call.source_key,
                statement_index: state_call.statement_index,
                call_ordinal: state_call.call_ordinal,
                target_key: state_call.target_key,
                is_value: state_call.role != StateCallRole::Statement,
            })
        })
        .collect()
}

/// Whether a loop (a back-edge: some state reachable from itself) is reachable
/// from the called state, following BOTH intra-machine transitions AND state
/// calls into other machines. A call must dispatch if its callee transitively
/// reaches a loop -- otherwise an inlined ancestor would unroll the chain and the
/// loop's dispatch back-edge would be unreachable. With per-call-context
/// specialization the whole reachable chain is cloned and dispatched, so the
/// callee's terminal still returns to this call site's continuation.
fn state_call_target_loops(
    control_flow: &ControlFlowPlan,
    state_calls: &StateCallPlan,
    start: omega_control_flow::StateKey,
) -> bool {
    fn visit(
        control_flow: &ControlFlowPlan,
        state_calls: &StateCallPlan,
        key: omega_control_flow::StateKey,
        on_path: &mut Vec<omega_control_flow::StateKey>,
        finished: &mut Vec<omega_control_flow::StateKey>,
    ) -> bool {
        if on_path.contains(&key) {
            return true;
        }
        if finished.contains(&key) {
            return false;
        }
        on_path.push(key);
        let mut loops = false;
        if let Some(state) = control_flow
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.key == key)
        {
            for transition in control_flow
                .transitions
                .span(state.transitions)
                .into_iter()
                .flatten()
            {
                match &transition.target {
                    omega_control_flow::PlannedTransitionTarget::SelfTarget => loops = true,
                    omega_control_flow::PlannedTransitionTarget::State { key: target, .. } => {
                        loops = visit(control_flow, state_calls, *target, on_path, finished);
                    }
                    // A NESTED target (`true -> mkall_copy(args)` -- a machine-
                    // name re-entry or a sibling machine's entry) resolves by
                    // STATE SYMBOL, overapproximated to every machine owning a
                    // matching state (the reentrant fence's own discipline: a
                    // false-positive loop only forces dispatch, which is always
                    // safe; missing one leaves a looping callee spliced). This
                    // was the original Nested-blindness: mkall_step's only loop
                    // is via a Nested transition into the re-entrant mkall_copy.
                    omega_control_flow::PlannedTransitionTarget::Nested {
                        state_symbol, ..
                    } => {
                        if state_symbol.is_valid() {
                            for (_, machine) in control_flow.machines.iter() {
                                for state in control_flow.states.span(machine.states).unwrap_or(&[])
                                {
                                    if state.key.state == *state_symbol
                                        && visit(
                                            control_flow,
                                            state_calls,
                                            state.key,
                                            on_path,
                                            finished,
                                        )
                                    {
                                        loops = true;
                                        break;
                                    }
                                }
                                if loops {
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
                if loops {
                    break;
                }
            }
        }
        // Follow state calls made from this state into their callee, so a loop
        // reached only through a call chain still forces dispatch.
        if !loops {
            for (_, call) in state_calls.calls.iter() {
                if call.required
                    && call.source_key == key
                    && visit(
                        control_flow,
                        state_calls,
                        call.target_key,
                        on_path,
                        finished,
                    )
                {
                    loops = true;
                    break;
                }
            }
        }
        on_path.pop();
        if !loops {
            finished.push(key);
        }
        loops
    }

    visit(
        control_flow,
        state_calls,
        start,
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

/// See `BackendPlan::receiver_bases`. Slice-2 scope: bases COMPOSE through
/// the parent-context chain (`context_call_sites` carries the minting
/// caller's own context), so NON-entry callers serve too: a context's base
/// is its parent's base plus the receiver path's offset within the CALLER's
/// machine layout (`omega_layout::field_path_offset`, the walk the
/// contained-receiver fence agrees with by consulting this very table).
/// Conservative edges, all of which leave the entry `None` (= the by-type
/// fallback, exactly today's behavior, and the fence keeps refusing what it
/// refused): `self`/static/empty receivers, a parent that itself did not
/// compose, and an unresolvable path. ZERO-SIZE callee machines emit `None`
/// deliberately -- the receiver owns no storage, so every machine-storage
/// read in its clones is CALLER-owned and an override could only
/// mis-rebase them (the dungeon regression of attempt #2, 2026-07-11a).
fn compute_receiver_bases(
    runtime_flow: &omega_state_graph::RuntimeFlowPlan,
    state_calls: &StateCallPlan,
    layouts: &omega_layout::LayoutPlan,
) -> Vec<Option<usize>> {
    let machine_layout_of = |machine: psi_symbols::SymbolHandle| {
        layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == machine)
            .map(|(_, machine_layout)| machine_layout)
    };

    // Per-CONTEXT absolute bases (entry-frame byte offsets), parent-first:
    // a context's parent is always minted before it, so one forward scan
    // sees every parent finished.
    let sites = &runtime_flow.context_call_sites;
    let mut context_bases: Vec<Option<usize>> = vec![None; sites.len()];
    let mut context_anchors: Vec<Option<usize>> = vec![None; sites.len()];
    let mut context_parameters: Vec<Vec<(String, usize)>> = vec![Vec::new(); sites.len()];
    if let Some(root) = context_bases.first_mut() {
        *root = Some(0); // ROOT: the entry machine's own region.
    }
    if let Some(root) = context_anchors.first_mut() {
        *root = Some(0);
    }
    for index in 1..sites.len() {
        let (call_key, statement_index, call_ordinal, parent) = sites[index];
        let parent_index = parent.0 as usize;
        let Some(parent_base) = context_anchors.get(parent_index).copied().flatten() else {
            continue;
        };
        let Some(state_call) = state_calls.calls.iter().map(|(_, call)| call).find(|call| {
            call.source_key == call_key
                && call.statement_index == statement_index
                && call.call_ordinal == call_ordinal
        }) else {
            continue;
        };
        let receiver_name = state_call.receiver_name.as_str();
        let Some(caller_layout) = machine_layout_of(call_key.machine) else {
            continue;
        };
        let parent_parameters = context_parameters
            .get(parent_index)
            .cloned()
            .unwrap_or_default();
        context_parameters[index] = bind_context_parameters(
            state_calls,
            state_call,
            layouts,
            caller_layout,
            parent_base,
            &parent_parameters,
        );
        if receiver_name == "self" {
            // A machine-to-machine SELF call (D10) runs on the CALLER's own
            // region: inherit the parent's composed base -- this is what lets
            // a named-receiver dispatch reached THROUGH self-call hops
            // (`holder.run()` -> `self.step()` -> `second.drain()`) keep
            // composing. Only when the attached data genuinely matches;
            // anything else keeps the by-type fallback.
            let same_data =
                machine_layout_of(state_call.target_key.machine).is_some_and(|callee_layout| {
                    callee_layout.attached_data.is_some()
                        && callee_layout.attached_data == caller_layout.attached_data
                });
            if same_data {
                context_bases[index] = Some(parent_base);
                context_anchors[index] = Some(parent_base);
                if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                    eprintln!(
                        "CTXBASE: ctx {index} self-inherit parent {} (base {parent_base})",
                        parent.0,
                    );
                }
            }
            continue;
        }
        if receiver_name.is_empty() {
            // A receiverless FREE machine has no storage region of its own,
            // but its mutable parameters still name caller storage. Preserve
            // the caller anchor for descendants while keeping this context's
            // emitted receiver override absent.
            if machine_layout_of(state_call.target_key.machine)
                .is_some_and(|layout| layout.attached_data.is_none())
            {
                context_anchors[index] = Some(parent_base);
            }
            continue; // static/receiverless: keep the by-type fallback
        }
        let segments = state_calls
            .receiver_path_segments
            .span(state_call.receiver_path)
            .unwrap_or(&[]);
        let field_segments = match segments.first() {
            Some(root) if root.as_str() == "self" => &segments[1..],
            _ => segments,
        };
        let offset_in_caller = if field_segments.len() <= 1 {
            context_parameters
                .get(parent_index)
                .and_then(|parameters| {
                    parameters
                        .iter()
                        .find(|(name, _)| name == receiver_name)
                        .map(|(_, absolute)| absolute.saturating_sub(parent_base))
                })
                .or_else(|| {
                    let field = field_segments.first().unwrap_or(&state_call.receiver_name);
                    omega_layout::field_path_offset(
                        layouts,
                        caller_layout.fields,
                        std::slice::from_ref(field),
                    )
                })
        } else {
            omega_layout::field_path_offset(layouts, caller_layout.fields, field_segments)
        };
        context_bases[index] = offset_in_caller.map(|offset| parent_base + offset);
        context_anchors[index] = context_bases[index];
        if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
            eprintln!(
                "CTXBASE: ctx {index} site m{} s{} seg{} stmt {statement_index} parent {} \
                 (base {parent_base}) receiver {receiver_name} -> {:?}",
                call_key.machine.arena_index(),
                call_key.state.arena_index(),
                call_key.segment_index,
                parent.0,
                context_bases[index],
            );
        }
    }
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
        for index in 1..sites.len() {
            if context_bases[index].is_none() {
                let (call_key, statement_index, call_ordinal, parent) = sites[index];
                eprintln!(
                    "CTXBASE: ctx {index} UNRESOLVED site m{} s{} seg{} stmt {statement_index} \
                     call {call_ordinal} \
                     parent {}",
                    call_key.machine.arena_index(),
                    call_key.state.arena_index(),
                    call_key.segment_index,
                    parent.0,
                );
            }
        }
    }

    // Emit indexed by ARENA INDEX (== dispatch_index; 1-based, 0 = the
    // invalid handle), NOT by iteration position -- every consumer looks up
    // `receiver_bases[dispatch_index]`. The positional collect() this
    // replaces was off by one and masked only because adjacent clone states
    // usually share a context (caught 2026-07-11b by the non-entry probe).
    let mut bases: Vec<Option<usize>> = vec![None; runtime_flow.states.len() + 1];
    for (handle, state) in runtime_flow.states.iter() {
        let index = handle.arena_index() as usize;
        if index >= bases.len() {
            bases.resize(index + 1, None);
        }
        let Some(base) = context_bases
            .get(state.context.0 as usize)
            .copied()
            .flatten()
        else {
            continue;
        };
        let Some(machine_layout) = machine_layout_of(state.key.machine) else {
            continue;
        };
        if machine_layout.layout.size == 0 {
            continue; // zero-size receiver: nothing to serve (see above)
        }
        bases[index] = Some(base);
    }
    bases
}

fn bind_context_parameters(
    state_calls: &StateCallPlan,
    call: &omega_state_calls::StateCall,
    layouts: &omega_layout::LayoutPlan,
    caller_layout: &omega_layout::MachineLayout,
    caller_base: usize,
    caller_parameters: &[(String, usize)],
) -> Vec<(String, usize)> {
    let Some(arguments) = state_calls.arguments.span(call.arguments) else {
        return Vec::new();
    };
    let mut bindings = Vec::new();
    for argument in arguments {
        if argument.kind != omega_state_calls::StateCallArgumentKind::MutableAlias {
            continue;
        }
        let mut segments = Vec::new();
        if !collect_state_call_expression_path(
            &state_calls.expressions,
            argument.expression,
            &mut segments,
        ) {
            continue;
        }
        let fields = match segments.first() {
            Some(root) if root.as_str() == "self" => &segments[1..],
            _ => segments.as_slice(),
        };
        let absolute = match fields {
            [single] => caller_parameters
                .iter()
                .find(|(name, _)| name == single.as_str())
                .map(|(_, base)| *base)
                .or_else(|| {
                    omega_layout::field_path_offset(
                        layouts,
                        caller_layout.fields,
                        std::slice::from_ref(*single),
                    )
                    .map(|offset| caller_base + offset)
                }),
            [] => None,
            path => {
                let owned = path
                    .iter()
                    .map(|segment| (*segment).clone())
                    .collect::<Vec<_>>();
                omega_layout::field_path_offset(layouts, caller_layout.fields, &owned)
                    .map(|offset| caller_base + offset)
            }
        };
        if let Some(absolute) = absolute {
            bindings.push((argument.parameter_name.as_str().to_owned(), absolute));
        }
    }
    bindings
}

fn collect_state_call_expression_path<'plan>(
    expressions: &'plan psi_checked_trees::expression::ExpressionTable,
    expression: psi_checked_trees::expression::ExpressionHandle,
    segments: &mut Vec<&'plan psi_checked_trees::name::Identifier>,
) -> bool {
    use psi_checked_trees::expression::ExpressionNode;
    match expressions.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            collect_state_call_expression_path(expressions, inner.target, segments)
        }
        ExpressionNode::Name(path) => {
            segments.extend(expressions.name_path_members(path.members));
            true
        }
        ExpressionNode::Member(member) => {
            if !collect_state_call_expression_path(expressions, member.receiver, segments) {
                return false;
            }
            segments.push(&member.member);
            true
        }
        _ => false,
    }
}

/// Resolve every table-field binding's byte offset from its attached provider
/// type's layout. Programs with no field-model rows pass through untouched.
/// Unknown types or fields fail before an unresolved mechanism reaches an
/// encoder.
fn resolve_vtable_field_offsets(
    host_abi: std::sync::Arc<omega_calling_conventions::HostAbiPlan>,
    layouts: &omega_layout::LayoutPlan,
) -> Result<std::sync::Arc<omega_calling_conventions::HostAbiPlan>, String> {
    use omega_calling_conventions::HostBindingMechanism;

    let needs_resolution = host_abi.bindings.iter().any(|(_, binding)| {
        matches!(
            binding.mechanism,
            HostBindingMechanism::VtableField { .. } | HostBindingMechanism::TableFunction { .. }
        )
    });
    if !needs_resolution {
        return Ok(host_abi);
    }

    let mut plan = std::sync::Arc::try_unwrap(host_abi).unwrap_or_else(|shared| (*shared).clone());
    let binding_handles: Vec<_> = plan.bindings.iter().map(|(handle, _)| handle).collect();
    for handle in binding_handles {
        let (table, field, is_table_function) = match plan.bindings.get(handle).mechanism.clone() {
            HostBindingMechanism::VtableField { table, field, .. } => (table, field, false),
            HostBindingMechanism::TableFunction { table, field, .. } => (table, field, true),
            _ => continue,
        };
        let Some(data_layout) = layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name.as_str() == table.as_ref())
            .map(|(_, data_layout)| data_layout)
        else {
            return Err(format!(
                "external table binding `{table}` has no data layout -- declare \
                 `data {table} {{ ... }}` with its fn-ptr fields in spec order",
            ));
        };
        let omega_layout::DataShape::Record { fields } = &data_layout.shape else {
            return Err(format!(
                "external table binding `{table}` must use a plain record of \
                 fn-ptr fields (case-bearing data cannot be a foreign vtable)",
            ));
        };
        let Some(field_layout) = layouts.fields.span(*fields).and_then(|candidates| {
            candidates
                .iter()
                .find(|candidate| candidate.name.as_str() == field.as_ref())
        }) else {
            return Err(format!(
                "external table binding `{table}` has no field `{field}` -- the Binding \
                 case must name one of its declared fn-ptr fields",
            ));
        };
        let resolved_offset = field_layout.offset;
        plan.bindings.get_mut(handle).mechanism = if is_table_function {
            HostBindingMechanism::TableFunction {
                table,
                field,
                byte_offset: resolved_offset,
            }
        } else {
            HostBindingMechanism::VtableField {
                table,
                field,
                byte_offset: resolved_offset,
            }
        };
    }
    Ok(std::sync::Arc::new(plan))
}
