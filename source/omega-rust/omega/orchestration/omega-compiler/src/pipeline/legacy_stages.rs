//! Quarantined pre-Terminal backend stages.
//!
//! This module begins at the obsolete checked-Psi to `StateGraph` seam. It is
//! retained only while remaining canaries move to Terminal realization and is
//! deleted with `LegacyDriver`; production native artifacts never enter it.

use super::stages::CheckedProgramSurface;
use crate::pipeline::stage::{
    ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS, ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS,
    BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD, CHECKED_TREES_TO_STATE_GRAPH,
    CONTROL_FLOW_TO_ABSTRACT_OPERATIONS, STATE_GRAPH_TO_CONTROL_FLOW,
    TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS,
};
use crate::pipeline::timing::CompileTimings;
use omega_control_flow::ControlFlowPlan;
use omega_emission_planning::{EmissionPlanningInput, build_emission_plan};
use omega_object_file::SectionKind;
use omega_state_graph::StateGraph;
use psi_diagnostics::Diagnostic;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) struct BackendPlanningSurface {
    pub(super) plan: omega_backend_plan::BackendPlan,
}

pub(super) fn checked_trees_to_state_graph(
    checked: &CheckedProgramSurface,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<StateGraph, Vec<Diagnostic>> {
    timings.record(CHECKED_TREES_TO_STATE_GRAPH, || {
        omega_checked_trees_to_state_graph::build_state_graph_with_workers(
            Arc::clone(&checked.program),
            workers,
        )
        .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn state_graph_to_control_flow(
    state_graph: StateGraph,
    timings: &mut CompileTimings,
) -> Result<ControlFlowPlan, Vec<Diagnostic>> {
    timings.record(STATE_GRAPH_TO_CONTROL_FLOW, || {
        omega_state_graph_to_control_flow::build_control_flow_plan_owned(state_graph)
            .map_err(|diagnostic| vec![diagnostic])
    })
}

pub(super) fn control_flow_to_backend_plan(
    checked: CheckedProgramSurface,
    entry_machine_name: Option<&str>,
    entry_boundary_plan: Option<omega_calling_conventions::BoundaryEntryPlan>,
    target_name: Option<&str>,
    freestanding: bool,
    control_flow: ControlFlowPlan,
    workers: omega_core::parallel::WorkerPoolHandle,
    timings: &mut CompileTimings,
) -> Result<BackendPlanningSurface, Vec<Diagnostic>> {
    let target_profile = omega_target::TargetProfile::from_omega_target_name(target_name)
        .map_err(|diagnostic| vec![diagnostic])?;

    let plan = omega_backend_pipeline::build_backend_plan_from_control_flow_with_workers(
        checked.program,
        checked.selected_provider_plans,
        entry_machine_name,
        entry_boundary_plan,
        checked.callback_placements,
        target_profile,
        freestanding,
        &checked.external_binding_rows,
        Arc::new(control_flow),
        workers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;

    record_backend_phase_as_stage(
        timings,
        &plan,
        "abstract operations",
        CONTROL_FLOW_TO_ABSTRACT_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "target operations",
        ABSTRACT_OPERATIONS_TO_TARGET_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "assigned target operations",
        TARGET_OPERATIONS_TO_ASSIGNED_TARGET_OPERATIONS,
    )?;
    record_backend_phase_as_stage(
        timings,
        &plan,
        "machine instructions",
        ASSIGNED_TARGET_OPERATIONS_TO_MACHINE_INSTRUCTIONS,
    )?;

    Ok(BackendPlanningSurface { plan })
}

fn plan_emission(plan: &omega_backend_plan::BackendPlan) -> omega_artifacts::EmissionPlan {
    let mut emission = build_emission_plan(&EmissionPlanningInput {
        receiver_bases: &plan.receiver_bases,
        state_contexts: &plan.state_contexts,
        target: plan.target,
        entry_key: plan.entry_key,
        host_abi: &plan.host_abi,
        host_calls: &plan.host_calls,
        state_calls: &plan.state_calls,
        state_storage: &plan.state_storage,
        state_values: &plan.state_values,
        data: &plan.data,
        instructions: &plan.target_operations,
        control_flow: &plan.control_flow,
        runtime_flow: &plan.runtime_flow,
        runtime_bodies: &plan.runtime_bodies,
        runtime_branching_calls: &plan.runtime_branching_calls,
        runtime_dispatch_loop: &plan.runtime_dispatch_loop,
        runtime_storage: &plan.runtime_storage,
        runtime_text: &plan.runtime_text,
        state_guards: &plan.state_guards,
        layouts: &plan.layouts,
        machine_instructions: &plan.machine_instructions,
        encoded_machine: &plan.encoded_machine,
        object: &plan.object,
        relocations: &plan.relocations,
    });
    retain_callback_thunk_emission_blockers(
        &mut emission,
        &plan.callback_placements,
        &plan.callback_thunks,
        &plan.callback_private_relocations,
        &plan.encoded_machine,
        &plan.object,
    );
    emission
}

/// Callback planning is authority, not evidence that native code exists. Keep
/// image emission fail-closed until every private identity owns one exact
/// encoded function and one matching private text symbol.
fn retain_callback_thunk_emission_blockers(
    emission: &mut omega_artifacts::EmissionPlan,
    callback_placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
    callback_thunks: &[omega_backend_plan::CallbackThunkPlan],
    callback_private_relocations: &[omega_backend_plan::CallbackPrivateRelocationDemand],
    encoded_machine: &omega_machine_bytes::EncodedMachinePlan,
    object: &omega_object_file::ObjectPlan,
) {
    if !callback_private_relocations.is_empty()
        || callback_placements
            .iter()
            .any(|placement| placement.private_materialization.is_some())
    {
        if let Err(error) = omega_backend_plan::replay_callback_private_relocation_demands(
            callback_placements,
            callback_thunks,
            callback_private_relocations,
        ) {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback private relocation demand",
                &format!("address-free callback relocation demand replay failed: {error}"),
            ));
        }
    }
    let mut placement_thunk_counts = vec![0usize; callback_placements.len()];
    let mut private_identity_counts = HashMap::<&str, usize>::new();
    for thunk in callback_thunks {
        if let Some(count) = placement_thunk_counts.get_mut(thunk.placement_index) {
            *count += 1;
        }
        *private_identity_counts
            .entry(thunk.private_symbol.as_ref())
            .or_default() += 1;
    }
    for (placement_index, placement) in callback_placements.iter().enumerate() {
        let thunk_count = placement_thunk_counts[placement_index];
        if thunk_count != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "validated callback placement {placement_index} for `{}` resolves to {thunk_count} private thunk plans; exactly one is required",
                    placement.canonical_requirement_overload
                ),
            ));
        }
    }

    for thunk in callback_thunks {
        let Some(placement) = callback_placements.get(thunk.placement_index) else {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` cites missing placement row {}",
                    thunk.private_symbol, thunk.placement_index
                ),
            ));
            continue;
        };
        if let Err(error) = omega_backend_plan::validate_bound_nominal_callback_placement(placement)
        {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` retained an invalid target calling plan: {error}",
                    thunk.private_symbol
                ),
            ));
            continue;
        }
        let placement_identity = omega_backend_plan::callback_placement_binding_identity(placement);
        if thunk.placement_identity != placement_identity {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` placement identity drifted from placement row {}",
                    thunk.private_symbol, thunk.placement_index
                ),
            ));
            continue;
        }
        let selected_entry = omega_control_flow::StateKey {
            machine: placement.selected_machine,
            state: placement.selected_entry,
            segment_index: 0,
        };
        if thunk.entry_key != selected_entry {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` targets {:?}, not placement row {} selected machine/entry {:?}",
                    thunk.private_symbol,
                    thunk.entry_key,
                    thunk.placement_index,
                    selected_entry,
                ),
            ));
            continue;
        }
        let thunk_identity_count = private_identity_counts[thunk.private_symbol.as_ref()];
        if thunk_identity_count != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback identity `{}` occurs {thunk_identity_count} times; exactly one is required",
                    thunk.private_symbol
                ),
            ));
            continue;
        }
        let canonical_private_symbol =
            omega_backend_plan::canonical_callback_private_symbol(placement);
        if thunk.private_symbol != canonical_private_symbol {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` does not match placement row {} canonical identity `{canonical_private_symbol}`",
                    thunk.private_symbol, thunk.placement_index
                ),
            ));
            continue;
        }
        if !thunk.entry_key.is_valid() {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` has an invalid selected-entry key",
                    thunk.private_symbol
                ),
            ));
            continue;
        }
        let canonical_function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(
                thunk.entry_key,
                thunk.placement_index,
            );
        if canonical_function_identity != Some(thunk.function_identity) {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` has function identity {:?}, not the canonical identity for placement row {} and selected entry {:?}",
                    thunk.private_symbol,
                    thunk.function_identity,
                    thunk.placement_index,
                    thunk.entry_key
                ),
            ));
            continue;
        }

        let encoded_functions = encoded_machine
            .code
            .functions
            .iter()
            .filter(|(_, function)| function.symbol.as_ref() == thunk.private_symbol.as_ref())
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        if encoded_functions.len() != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` resolves to {} encoded functions; exactly one is required",
                    thunk.private_symbol,
                    encoded_functions.len()
                ),
            ));
            continue;
        }
        let encoded = encoded_functions[0];
        if encoded.identity != thunk.function_identity {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` encoded function targets {:?}, not its selected entry {:?}",
                    thunk.private_symbol, encoded.identity, thunk.entry_key
                ),
            ));
            continue;
        }

        let symbols = object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.name == thunk.private_symbol.as_ref())
            .map(|(_, symbol)| symbol)
            .collect::<Vec<_>>();
        if symbols.len() != 1 {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` resolves to {} object symbols; exactly one is required",
                    thunk.private_symbol,
                    symbols.len()
                ),
            ));
            continue;
        }
        let symbol = symbols[0];
        let Some((_, identity_symbol)) =
            omega_object_file::object_function_symbol(object, thunk.function_identity)
        else {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` has no exact object-function binding for identity {:?}",
                    thunk.private_symbol, thunk.function_identity
                ),
            ));
            continue;
        };
        if identity_symbol.name != thunk.private_symbol.as_ref() {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback identity {:?} binds object symbol `{}`, not `{}`",
                    thunk.function_identity, identity_symbol.name, thunk.private_symbol
                ),
            ));
            continue;
        }
        if symbol.kind != omega_object_file::SymbolKind::Function
            || symbol.section
                != omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text)
            || symbol.offset != encoded.byte_offset
            || symbol.size != encoded.byte_count
        {
            emission.blockers.insert(omega_artifacts::emission_blocker(
                "callback thunk emission",
                &format!(
                    "planned private callback `{}` object symbol does not match its encoded function interval",
                    thunk.private_symbol
                ),
            ));
        }
    }
}

pub(super) fn ensure_emission_ready(
    emission_plan: &omega_artifacts::EmissionPlan,
) -> Result<(), Vec<Diagnostic>> {
    if emission_plan.blockers.is_empty() {
        return Ok(());
    }

    Err(emission_plan
        .blockers
        .iter()
        .map(|(_, blocker)| Diagnostic::error(format!("{}: {}", blocker.stage, blocker.reason)))
        .collect())
}

pub(super) fn backend_plan_to_native_image_payload(
    backend: &BackendPlanningSurface,
    subsystem: u16,
    timings: &mut CompileTimings,
) -> Result<
    (
        omega_artifacts::EmissionPlan,
        crate::pipeline::emitted_program::EmittedProgram,
    ),
    Vec<Diagnostic>,
> {
    timings.record(BACKEND_PLAN_TO_NATIVE_IMAGE_PAYLOAD, || {
        let emission_plan = plan_emission(&backend.plan);
        ensure_emission_ready(&emission_plan)?;
        let plan = &backend.plan;
        omega_backend_plan::build_callback_installation_manifest(plan).map_err(|error| {
            vec![Diagnostic::error(format!(
                "callback installation manifest replay failed: {}",
                error.0
            ))]
        })?;
        let text_bytes = plan.encoded_machine.code.bytes.storage_slice().to_vec();
        let emitted = crate::pipeline::emitted_program::EmittedProgram {
            target: plan.target,
            subsystem,
            planned_text_bytes: object_text_size(&plan.object),
            callback_placement_identity_fingerprint:
                omega_backend_plan::callback_thunk_placement_identity_fingerprint(
                    &plan.callback_thunks,
                ),
            object: plan.object.clone(),
            relocations: plan.relocations.clone(),
            encoded_machine_code: plan.encoded_machine.code.clone(),
            encoded_machine_semantics: plan.encoded_machine.semantics.clone(),
            text_bytes,
            data_bytes: plan.data.bytes.storage_slice().to_vec(),
        };
        Ok((emission_plan, emitted))
    })
}

fn object_text_size(object: &omega_object_file::ObjectPlan) -> usize {
    object
        .layout
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Text)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn record_backend_phase_as_stage(
    timings: &mut CompileTimings,
    plan: &omega_backend_plan::BackendPlan,
    backend_phase: &str,
    stage: crate::pipeline::stage::StageMeta,
) -> Result<(), Vec<Diagnostic>> {
    let Some((_, phase_timing)) = plan
        .phase_timings
        .iter()
        .find(|(_, phase_timing)| phase_timing.phase == backend_phase)
    else {
        return Err(vec![Diagnostic::error(format!(
            "backend phase `{backend_phase}` was not recorded for {}",
            stage.label()
        ))]);
    };

    timings.add_completed(
        stage,
        phase_timing.microseconds,
        phase_timing.allocations.clone(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy};
    use omega_control_flow::StateKey;
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    use omega_target::NativeTarget;

    fn empty_emission(target: NativeTarget) -> omega_artifacts::EmissionPlan {
        omega_artifacts::EmissionPlan {
            image_format: target.object_format,
            entry_symbol: String::new(),
            sections: 0,
            symbols: 0,
            host_bindings: 0,
            host_calls: 0,
            data_bytes: 0,
            selected_instructions: 0,
            instruction_operands: 0,
            machine_code_bytes: 0,
            encoded_machine_bytes: 0,
            relocations: 0,
            blockers: psi_arena::Arena::new(),
        }
    }

    fn state_key(state: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    fn thunk(
        entry_key: StateKey,
        placement: &omega_backend_plan::BoundNominalCallbackPlacement,
    ) -> omega_backend_plan::CallbackThunkPlan {
        let function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(entry_key, 0)
                .unwrap_or_default();
        let private_symbol = omega_backend_plan::canonical_callback_private_symbol(placement);
        let root_schedule = Arc::new(
            omega_backend_plan::plan_callback_root_schedule(
                0,
                placement,
                entry_key,
                function_identity,
                Arc::clone(&private_symbol),
            )
            .expect("valid callback fixture root schedule"),
        );
        omega_backend_plan::CallbackThunkPlan {
            placement_index: 0,
            placement_identity: omega_backend_plan::callback_placement_binding_identity(placement),
            entry_key,
            function_identity,
            private_symbol,
            root_schedule,
        }
    }

    fn placement(entry_key: StateKey) -> omega_backend_plan::BoundNominalCallbackPlacement {
        let validated = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty callback entry plan");
        omega_backend_plan::BoundNominalCallbackPlacement {
            site: psi_checked_trees::NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_arena_index(9),
            ),
            registration_operation: SymbolHandle::from_arena_index(3),
            static_machine_ordinal: 0,
            selected_machine: entry_key.machine,
            selected_entry: entry_key.state,
            satisfaction_trait: SymbolHandle::from_arena_index(4),
            satisfaction_requirement: SymbolHandle::from_arena_index(5),
            canonical_requirement_overload: "Handler::call".to_owned(),
            boundary_calling_plan_fingerprint: validated.contract_fingerprint(),
            boundary_entry_plan: validated.plan().clone(),
            private_materialization: None,
        }
    }

    fn encoded_machine(
        target: NativeTarget,
        keys: &[StateKey],
        symbol: &str,
    ) -> omega_machine_bytes::EncodedMachinePlan {
        let mut encoded =
            omega_machine_bytes::EncodedMachinePlan::with_capacity(target, keys.len(), 0, 0);
        for key in keys {
            encoded
                .code
                .functions
                .insert(omega_machine_bytes::EncodedMachineFunction {
                    symbol: Arc::from(symbol),
                    identity: omega_control_flow::MachineFunctionIdentity::callback_thunk(*key, 0)
                        .unwrap(),
                    byte_offset: 7,
                    byte_count: 11,
                    instructions: Default::default(),
                });
        }
        encoded
    }

    fn object_with_symbols(
        target: NativeTarget,
        thunk: &omega_backend_plan::CallbackThunkPlan,
        symbols: &[(usize, usize)],
    ) -> omega_object_file::ObjectPlan {
        let mut object = omega_object_file::ObjectPlan::with_capacity(target, 0, symbols.len());
        for (symbol_index, (offset, size)) in symbols.iter().enumerate() {
            let symbol = object.layout.symbols.insert(omega_object_file::SymbolPlan {
                name: thunk.private_symbol.to_string(),
                section: omega_object_file::SymbolSection::Section(
                    omega_object_file::SectionKind::Text,
                ),
                offset: *offset,
                size: *size,
                kind: omega_object_file::SymbolKind::Function,
                import_library: String::new(),
            });
            if symbol_index == 0 {
                object
                    .layout
                    .function_symbols
                    .insert(omega_object_file::FunctionSymbolPlan {
                        identity: thunk.function_identity,
                        symbol,
                    });
            }
        }
        object
    }

    fn callback_blockers(
        placements: &[omega_backend_plan::BoundNominalCallbackPlacement],
        thunks: &[omega_backend_plan::CallbackThunkPlan],
        encoded: &omega_machine_bytes::EncodedMachinePlan,
        object: &omega_object_file::ObjectPlan,
    ) -> Vec<String> {
        let mut emission = empty_emission(encoded.target);
        retain_callback_thunk_emission_blockers(
            &mut emission,
            placements,
            thunks,
            &[],
            encoded,
            object,
        );
        emission
            .blockers
            .iter()
            .map(|(_, blocker)| blocker.reason.clone())
            .collect()
    }

    #[test]
    fn callback_thunk_emission_accepts_one_exact_function_and_private_symbol() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let blockers = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[key], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );
        assert!(blockers.is_empty(), "{blockers:?}");
    }

    #[test]
    fn callback_thunk_emission_rejects_invalid_or_redirected_entry_keys() {
        let target = NativeTarget::host();
        let mut invalid_placement = placement(state_key(2));
        let mut invalid_thunk = thunk(state_key(2), &invalid_placement);
        invalid_placement.selected_machine = StateKey::default().machine;
        invalid_placement.selected_entry = StateKey::default().state;
        invalid_thunk.entry_key = StateKey::default();
        invalid_thunk.function_identity = Default::default();
        invalid_thunk.placement_identity =
            omega_backend_plan::callback_placement_binding_identity(&invalid_placement);
        invalid_thunk.private_symbol =
            omega_backend_plan::canonical_callback_private_symbol(&invalid_placement);
        let invalid = callback_blockers(
            &[invalid_placement],
            std::slice::from_ref(&invalid_thunk),
            &encoded_machine(target, &[state_key(2)], &invalid_thunk.private_symbol),
            &object_with_symbols(target, &invalid_thunk, &[(7, 11)]),
        );
        assert_eq!(invalid.len(), 1);
        assert!(invalid[0].contains("invalid selected-entry key"));

        let redirected_placement = placement(state_key(2));
        let redirected_thunk = thunk(state_key(2), &redirected_placement);
        let redirected = callback_blockers(
            &[redirected_placement],
            std::slice::from_ref(&redirected_thunk),
            &encoded_machine(target, &[state_key(3)], &redirected_thunk.private_symbol),
            &object_with_symbols(target, &redirected_thunk, &[(7, 11)]),
        );
        assert_eq!(redirected.len(), 1);
        assert!(redirected[0].contains("not its selected entry"));
    }

    #[test]
    fn callback_thunk_emission_rejects_missing_or_duplicate_encoded_symbols() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let object = object_with_symbols(target, &thunk, &[(7, 11)]);

        let missing = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[], &thunk.private_symbol),
            &object,
        );
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("resolves to 0 encoded functions"));

        for duplicate_keys in [[key, key], [key, state_key(3)]] {
            let duplicate = callback_blockers(
                std::slice::from_ref(&placement),
                std::slice::from_ref(&thunk),
                &encoded_machine(target, &duplicate_keys, &thunk.private_symbol),
                &object,
            );
            assert_eq!(duplicate.len(), 1);
            assert!(duplicate[0].contains("resolves to 2 encoded functions"));
        }
    }

    #[test]
    fn callback_thunk_emission_rejects_object_cardinality_or_interval_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let encoded = encoded_machine(target, &[key], &thunk.private_symbol);

        for symbols in [Vec::new(), vec![(7, 11), (7, 11)]] {
            let blockers = callback_blockers(
                std::slice::from_ref(&placement),
                std::slice::from_ref(&thunk),
                &encoded,
                &object_with_symbols(target, &thunk, &symbols),
            );
            assert_eq!(blockers.len(), 1);
            assert!(blockers[0].contains("object symbols; exactly one is required"));
        }

        let drifted = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded,
            &object_with_symbols(target, &thunk, &[(7, 10)]),
        );
        assert_eq!(drifted.len(), 1);
        assert!(drifted[0].contains("does not match its encoded function interval"));
    }

    #[test]
    fn callback_thunk_emission_rejects_missing_duplicate_or_unknown_placement_joins() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let encoded = encoded_machine(target, &[key], &thunk.private_symbol);
        let object = object_with_symbols(target, &thunk, &[(7, 11)]);

        let missing = callback_blockers(std::slice::from_ref(&placement), &[], &encoded, &object);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("resolves to 0 private thunk plans"));

        let duplicate = callback_blockers(
            std::slice::from_ref(&placement),
            &[thunk.clone(), thunk.clone()],
            &encoded,
            &object,
        );
        assert!(
            duplicate
                .iter()
                .any(|blocker| blocker.contains("resolves to 2 private thunk plans"))
        );
        assert!(
            duplicate
                .iter()
                .any(|blocker| blocker.contains("occurs 2 times"))
        );

        let mut unknown = thunk;
        unknown.placement_index = 1;
        let unknown = callback_blockers(&[placement], &[unknown], &encoded, &object);
        assert!(
            unknown
                .iter()
                .any(|blocker| blocker.contains("cites missing placement row 1"))
        );
    }

    #[test]
    fn callback_thunk_emission_rejects_entry_drift_from_placement_row() {
        let target = NativeTarget::host();
        let selected = state_key(2);
        let drifted = state_key(3);
        let placement = placement(selected);
        let mut thunk = thunk(selected, &placement);
        thunk.entry_key = drifted;
        thunk.function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(drifted, 0).unwrap();

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[drifted], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("not placement row 0 selected machine/entry"));
    }

    #[test]
    fn callback_thunk_emission_rejects_selected_entry_segment_drift() {
        let target = NativeTarget::host();
        let selected = state_key(2);
        let placement = placement(selected);
        let segmented = StateKey {
            segment_index: 1,
            ..selected
        };
        let mut thunk = thunk(selected, &placement);
        thunk.entry_key = segmented;
        thunk.function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(segmented, 0).unwrap();

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[segmented], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("not placement row 0 selected machine/entry"));
    }

    #[test]
    fn callback_thunk_emission_rejects_private_symbol_drift_from_placement() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let mut thunk = thunk(key, &placement);
        thunk.private_symbol = Arc::from("__omega_callback_tampered");

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[key], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("does not match placement row 0 canonical identity"));
    }

    #[test]
    fn callback_thunk_emission_rejects_retained_boundary_plan_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let mut placement = placement(key);
        let thunk = thunk(key, &placement);
        placement.boundary_entry_plan.state.preemption =
            omega_calling_conventions::Preemption::ProviderDefined;

        let blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&thunk),
            &encoded_machine(target, &[key], &thunk.private_symbol),
            &object_with_symbols(target, &thunk, &[(7, 11)]),
        );

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("drifted from its retained fingerprint"));
    }

    #[test]
    fn callback_thunk_emission_rejects_registration_or_satisfaction_identity_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let thunk = thunk(key, &placement);
        let encoded = encoded_machine(target, &[key], &thunk.private_symbol);
        let object = object_with_symbols(target, &thunk, &[(7, 11)]);

        let mut registration_drift = placement.clone();
        registration_drift.registration_operation = SymbolHandle::from_parts(3, 2);
        let blockers = callback_blockers(
            &[registration_drift],
            std::slice::from_ref(&thunk),
            &encoded,
            &object,
        );
        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("placement identity drifted"));

        let mut satisfaction_drift = placement;
        satisfaction_drift.satisfaction_trait = SymbolHandle::from_parts(4, 2);
        let blockers = callback_blockers(
            &[satisfaction_drift],
            std::slice::from_ref(&thunk),
            &encoded,
            &object,
        );
        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("placement identity drifted"));
    }

    #[test]
    fn callback_thunk_emission_rejects_function_role_or_placement_drift() {
        let target = NativeTarget::host();
        let key = state_key(2);
        let placement = placement(key);
        let mut source_role = thunk(key, &placement);
        source_role.function_identity = omega_control_flow::MachineFunctionIdentity::source(key);

        let source_role_blockers = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&source_role),
            &encoded_machine(target, &[key], &source_role.private_symbol),
            &object_with_symbols(target, &source_role, &[(7, 11)]),
        );
        assert_eq!(source_role_blockers.len(), 1);
        assert!(source_role_blockers[0].contains("not the canonical identity"));

        let mut wrong_placement = thunk(key, &placement);
        wrong_placement.function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(key, 1).unwrap();
        let wrong_placement_blockers = callback_blockers(
            std::slice::from_ref(&placement),
            std::slice::from_ref(&wrong_placement),
            &encoded_machine(target, &[key], &wrong_placement.private_symbol),
            &object_with_symbols(target, &wrong_placement, &[(7, 11)]),
        );
        assert_eq!(wrong_placement_blockers.len(), 1);
        assert!(wrong_placement_blockers[0].contains("not the canonical identity"));

        let exact = thunk(key, &placement);
        let mut redirected_object = object_with_symbols(target, &exact, &[(7, 11)]);
        let binding = redirected_object
            .layout
            .function_symbols
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .unwrap();
        redirected_object
            .layout
            .function_symbols
            .get_mut(binding)
            .identity = omega_control_flow::MachineFunctionIdentity::source(key);
        let object_role_blockers = callback_blockers(
            &[placement],
            std::slice::from_ref(&exact),
            &encoded_machine(target, &[key], &exact.private_symbol),
            &redirected_object,
        );
        assert_eq!(object_role_blockers.len(), 1);
        assert!(object_role_blockers[0].contains("no exact object-function binding"));
    }
}
