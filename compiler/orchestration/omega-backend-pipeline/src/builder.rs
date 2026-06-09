use super::entry::resolve_backend_entry_point;
use super::skeleton::{BackendPlanSkeletonInput, build_backend_plan_skeleton};
use super::timing::record_backend_phase;
use omega_abstract_operations_to_target_operations::build_target_operation_plan;
use omega_assigned_target_operations_to_machine_instructions::build_machine_instructions;
use omega_backend_plan::BackendPlan;
use omega_calling_conventions::build_host_abi_plan;
use omega_checked_trees::CheckedTrees;
use omega_control_flow::ControlFlowPlan;
use omega_control_flow_to_abstract_operations::{
    AbstractOperationLoweringInput, build_abstract_operation_plan,
};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_data_planning::build_target_data_plan;
use omega_layout::build_layout_plan;
use omega_machine_emission::{MachineEmissionInput, emit_machine_bytes};
use omega_object_file::object_entry_symbol_name;
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
    RuntimeStorageContext, build_runtime_storage_plan_with_workers,
    runtime_frame_storage_alignment, runtime_frame_storage_size,
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
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::build_assigned_target_operations;
use std::sync::Arc;

pub(super) fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<CheckedTrees>,
    target: NativeTarget,
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    let entry_point = resolve_backend_entry_point(&program)?;
    let mut phase_timings = Arena::new();
    let host_abi = record_backend_phase(&mut phase_timings, "host abi", || {
        build_host_abi_plan(target)
    });
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
    let runtime_state_call_edges = dispatch_state_call_edges(state_calls.as_ref(), &control_flow);
    let runtime_flow = if runtime_state_call_edges.is_empty() {
        seed_runtime_flow
    } else {
        Arc::new(record_backend_phase(
            &mut phase_timings,
            "runtime flow/state calls",
            || {
                build_runtime_flow_plan_with_state_calls(
                    &control_flow,
                    entry_key,
                    &runtime_state_call_edges,
                )
            },
        )?)
    };
    // Rebuild the state-call plan against the dispatched runtime flow whenever any
    // call dispatched. The cover-check short-circuit left a dispatched value call's
    // lowering stale (still InlineBranching) so it would double-lower; rebuilding
    // re-lowers every call against the clone graph.
    let state_calls = if runtime_state_call_edges.is_empty() {
        state_calls
    } else {
        Arc::new(record_backend_phase(
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
        ))
    };
    let state_dispatch = record_backend_phase(&mut phase_timings, "state dispatch", || {
        build_state_dispatch_plan_with_workers(
            Arc::new(StateDispatchContext::from_runtime_flow(Arc::clone(
                &runtime_flow,
            ))),
            workers.clone(),
        )
    });
    let mut backend_plan = build_backend_plan_skeleton(BackendPlanSkeletonInput {
        target,
        host_abi: Arc::clone(&host_abi),
        host_calls: Arc::try_unwrap(host_calls).unwrap_or_else(|host_calls| (*host_calls).clone()),
        control_flow: Arc::clone(&control_flow),
        runtime_flow: Arc::clone(&runtime_flow),
        state_dispatch,
        state_guards: Default::default(),
        layouts,
        entry_key,
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
    let state_storage_workers = workers.clone();
    let state_values_workers = workers.clone();
    let (state_storage, state_values) =
        record_backend_phase(&mut phase_timings, "state storage/values", || {
            workers.join2(
                move || {
                    build_state_storage_plan_with_workers(
                        state_storage_program,
                        state_storage_context,
                        state_storage_workers,
                    )
                },
                move || {
                    build_state_value_plan_with_workers(
                        state_values_program,
                        state_values_context,
                        state_values_workers,
                    )
                },
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
    backend_plan.state_guards = record_backend_phase(&mut phase_timings, "state guards", || {
        build_state_guard_plan(
            &program,
            backend_plan.state_dispatch.as_ref(),
            &backend_plan.control_flow,
            &backend_plan.layouts,
            &backend_plan.runtime_storage,
            backend_plan.entry_key.machine,
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
        build_target_data_plan(
            &backend_plan.host_calls,
            &backend_plan.state_storage,
            &backend_plan.state_values,
            &backend_plan.runtime_branching_calls,
            &backend_plan.runtime_text,
        )
    });
    backend_plan.abstract_data = (&backend_plan.data).into();
    backend_plan.abstract_operations =
        record_backend_phase(&mut phase_timings, "abstract operations", || {
            let runtime_abi = build_runtime_abi_plan(backend_plan.target);
            build_abstract_operation_plan(&AbstractOperationLoweringInput {
                runtime_abi: &runtime_abi,
                entry_key: backend_plan.entry_key,
                entry_symbol: object_entry_symbol_name(&backend_plan.object).into(),
                program: program.as_ref(),
                control_flow: &backend_plan.control_flow,
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
        });
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
            entry_state_key: backend_plan.entry_key,
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

/// Give each call-context (specialized clone) a disjoint frame region so a
/// caller's live slots survive the dispatched calls it makes. Slots are built
/// per dispatch body from offset 0; this shifts every slot by its context's
/// cumulative base. States within one context keep sharing their range (they are
/// never simultaneously live), and the entry context (`ROOT`) stays at base 0, so
/// programs without dispatched calls are unchanged.
fn stack_runtime_storage_by_call_context(
    storage: &mut omega_runtime_storage::RuntimeStoragePlan,
    runtime_flow: &RuntimeFlowPlan,
) {
    // A frame slot's `dispatch_index` is the state's ARENA INDEX in the runtime
    // flow (see omega-state-dispatch: `dispatch_index = handle.arena_index()`).
    // Index `contexts` by that arena index explicitly -- iteration position is NOT
    // guaranteed to equal arena index, and conflating them assigns a state's slots
    // to the wrong context's region (a caller and a dispatched callee then overlap
    // and clobber each other's live values).
    let mut contexts: Vec<u32> = Vec::new();
    for (handle, state) in runtime_flow.states.iter() {
        let index = handle.arena_index() as usize;
        if index >= contexts.len() {
            contexts.resize(index + 1, 0);
        }
        contexts[index] = state.context.0;
    }
    let Some(&max_context) = contexts.iter().max() else {
        return;
    };
    if max_context == 0 {
        // Only the entry context: every body already lays out from 0.
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

    let context_count = max_context as usize + 1;
    let mut sizes = vec![0usize; context_count];
    let mut alignments = vec![1usize; context_count];
    for (_, slot) in storage.frame_slots.iter() {
        let context = context_of(slot.dispatch_index);
        sizes[context] = sizes[context].max(slot.byte_offset + slot.byte_size);
        alignments[context] = alignments[context].max(slot.alignment.max(1));
    }

    // Stack contexts into disjoint regions (ROOT first, at base 0). Context ids
    // are minted parent-before-child, so any caller precedes its callees.
    let mut bases = vec![0usize; context_count];
    let mut next_base = 0usize;
    for context in 0..context_count {
        bases[context] = next_base.next_multiple_of(alignments[context]);
        next_base = bases[context] + sizes[context];
    }

    let handles: Vec<_> = storage
        .frame_slots
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    for handle in handles {
        let slot = storage.frame_slots.get_mut(handle);
        let base = bases[context_of(slot.dispatch_index)];
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
                target_key: state_call.target_key,
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
                match transition.target {
                    omega_control_flow::PlannedTransitionTarget::SelfTarget => loops = true,
                    omega_control_flow::PlannedTransitionTarget::State { key: target, .. } => {
                        loops = visit(control_flow, state_calls, target, on_path, finished);
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

