//! Opaque callback custody across successful and rejected native realization.

use crate::tests::fixtures::hosted::{hosted_custody, paired_calling_plan_parts};
use crate::{
    NativeProgramEntrySettlement, NativeRealizationRequest,
    current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_permission_policy, realize_native_artifact_with_callback_custody,
};

#[test]
fn native_realization_returns_exact_ordered_callback_custody_on_success() {
    let (artifact, _, source, plans) = hosted_custody();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let custody = vec![(11u64, "first"), (29u64, "second")];

    let realized = realize_native_artifact_with_callback_custody(
        artifact,
        NativeRealizationRequest {
            checked_scope: None,
            prepared_input: None,
            target: target::NativeTarget::windows_x64(),
            image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            terminal_authority_permission_policy: Some(
                current_terminal_authority_permission_policy(),
            ),
            program_entry: NativeProgramEntrySettlement::new(
                &source,
                Some(paired_calling_plan_parts(&plans)),
                &[],
            ),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
        },
        custody.clone(),
    )
    .expect("opaque callback custody crosses successful native realization");

    assert_eq!(realized.callback_custody(), &custody);
    realized
        .artifact()
        .as_direct()
        .expect("direct image requested")
        .validate()
        .expect("source-free native artifact remains independently valid");
    let (_, returned) = realized.into_parts();
    assert_eq!(returned, custody);
}

#[test]
fn native_realization_rejection_returns_callback_custody_without_reordering() {
    let (artifact, _, source, plans) = hosted_custody();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let swapped = vec![(29u64, "second"), (11u64, "first")];
    let interpreter = target::normalize_elf_interpreter_plan(
        b"/lib/ld-linux-x86-64.so.2".to_vec(),
        target::TargetProfile::LinuxX64,
    )
    .unwrap();

    let rejected = realize_native_artifact_with_callback_custody(
        artifact,
        NativeRealizationRequest {
            checked_scope: None,
            prepared_input: None,
            target: target::NativeTarget::linux_x64(),
            image_request: image_emission::ExecutableImageEmissionRequest::dynamic_elf(
                interpreter.clone(),
            ),
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            terminal_authority_permission_policy: Some(
                current_terminal_authority_permission_policy(),
            ),
            program_entry: NativeProgramEntrySettlement::new(
                &source,
                Some(paired_calling_plan_parts(&plans)),
                &[],
            ),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
        },
        swapped.clone(),
    )
    .expect_err("ProgramEntry target drift rejects native realization");

    assert_eq!(rejected.diagnostics().len(), 1);
    assert!(
        rejected.diagnostics()[0]
            .message
            .contains("native artifact ProgramEntry custody failed")
    );
    assert_eq!(rejected.callback_custody(), &swapped);
    let (realization, returned) = rejected.into_parts();
    assert_eq!(returned, swapped);
    let (image_request, _) = realization.into_parts();
    let image_emission::ExecutableImageEmissionRequest::DynamicElf {
        interpreter: returned,
    } = image_request
    else {
        panic!("rejection must retain dynamic image custody");
    };
    assert_eq!(returned, interpreter);
}

/// One checked `u64 -> u64` identity body lowered through the same isolated
/// callback production the native proposal uses: bounded callback lowering,
/// empty Psi optimization, canonical Terminal packaging. The artifact is the
/// real replayable input the thunk settlement carries.
fn callback_thunk_artifact() -> (
    terminal_codec::CanonicalTerminalArtifact,
    lowered_psi::CallbackTerminalLoweringReceipt,
) {
    let checked = crate::tests::fixtures::checked_source::checked(
        "data Callback {}\nmachine Callback::identity(input: u64) -> u64 { input }",
    );
    let graph = &checked.facts.flow.terminal_scalar_graphs.machines[0];
    let lowered = checked_trees_to_lowered_psi::lower_bounded_callback_identity_machine(
        &checked,
        graph.machine,
        graph.states[0].state,
    )
    .expect("bounded callback body lowers");
    let optimized = lowered_psi_to_lowered_psi::run_psi_optimization(
        lowered.terminal,
        optimization::PsiOptimizationSelections::default(),
    )
    .expect("callback body survives empty Psi optimization");
    let artifact = lowered_psi_to_terminal_psi::finalize_terminal_artifact(&optimized)
        .expect("callback body canonicalizes");
    (artifact, lowered.receipt)
}

fn callback_boundary_entry_plan(
    target: target::NativeTarget,
) -> calling_conventions::ValidatedBoundaryEntryPlan {
    calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8)],
            result: Some(calling_conventions::ValueShape::integer(8, 8)),
        },
    )
    .expect("one-u64 callback boundary")
}

fn callback_thunk_settlement<'artifact>(
    artifact: &'artifact terminal_codec::CanonicalTerminalArtifact,
    boundary_entry_plan: &'artifact calling_conventions::BoundaryEntryPlan,
    receipt: lowered_psi::CallbackTerminalLoweringReceipt,
    placement_index: usize,
    private_symbol: &'artifact str,
) -> crate::NativeCallbackThunkSettlement<'artifact> {
    crate::NativeCallbackThunkSettlement {
        terminal_operation: semantic_vocabulary::OperationId::new(61).unwrap(),
        placement_index,
        callback_function: function_identity::MachineFunctionIdentity::callback_thunk(
            function_identity::StateKey {
                machine: receipt.source_machine,
                state: receipt.source_entry,
                segment_index: 0,
            },
            placement_index,
        )
        .expect("callback thunk identity"),
        private_symbol,
        artifact,
        lowering_receipt: receipt,
        boundary_entry_plan,
    }
}

fn callback_thunks_request<'a>(
    target: target::NativeTarget,
    source: &'a program_entry_plan::SelectedProgramEntrySourceSignature,
    plans: &'a build_evaluation::SelectedProgramEntryCallingPlans,
    profile: &'a proof_admission::AdmissionProfile,
    optimizations: &'a optimization_core::PostTerminalOptimizationSelections,
    providers: &'a effects::SelectedProviderPlanFacts,
    callback_thunks: &'a [crate::NativeCallbackThunkSettlement<'a>],
) -> NativeRealizationRequest<'a> {
    NativeRealizationRequest {
        checked_scope: None,
        prepared_input: None,
        target,
        image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
        profile,
        terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
        terminal_authority_permission_policy: Some(current_terminal_authority_permission_policy()),
        program_entry: NativeProgramEntrySettlement::new(
            source,
            Some(paired_calling_plan_parts(plans)),
            &[],
        ),
        optimization_selections: optimizations,
        selected_provider_plans: providers,
        external_binding_rows: &[],
        settlements: &[],
        compiler_builtins: &[],
        boundary_application_coverage: None,
        ieee_float_fma: &[],
        native_callbacks: &[],
        callback_thunks,
    }
}

#[test]
fn callback_thunks_materialize_private_functions_into_the_realized_object() {
    let (artifact, _, source, plans) = hosted_custody();
    let target = target::NativeTarget::windows_x64();
    let (thunk_artifact, receipt) = callback_thunk_artifact();
    let boundary = callback_boundary_entry_plan(target);
    let thunks = [
        callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            0,
            "__omega_private_callback_0",
        ),
        callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            1,
            "__omega_private_callback_1",
        ),
    ];
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();

    let realized = crate::realize_native_artifact(
        artifact,
        NativeRealizationRequest {
            checked_scope: None,
            prepared_input: None,
            target,
            image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            terminal_authority_permission_policy: Some(
                current_terminal_authority_permission_policy(),
            ),
            program_entry: NativeProgramEntrySettlement::new(
                &source,
                Some(paired_calling_plan_parts(&plans)),
                &[],
            ),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &thunks,
        },
    )
    .expect("the two-slot registrar materializes inside one realization")
    .into_direct()
    .expect("direct image requested");

    let object = realized.object();
    let [first, second] = object.private_functions() else {
        panic!("the object retains both compiler-private functions");
    };
    for (carrier, settlement) in [(first, &thunks[0]), (second, &thunks[1])] {
        assert_eq!(carrier.identity, settlement.callback_function);
        assert_eq!(
            carrier.source_psi,
            thunk_artifact.manifest().semantic(),
            "the carrier names the thunk artifact's Terminal identity",
        );
        let (_, private_plan) =
            object_file::object_function_symbol(object.object(), carrier.identity)
                .expect("the private identity resolves to one function symbol");
        assert_eq!(private_plan.name, settlement.private_symbol);
        assert_eq!(private_plan.kind, object_file::SymbolKind::Function);
        assert_eq!(
            private_plan.section,
            object_file::SymbolSection::Section(object_file::SectionKind::Text),
        );
        assert_eq!(carrier.function.text_offset, private_plan.offset);
        assert_eq!(carrier.function.byte_count, private_plan.size);
        assert!(
            !carrier.bytes(object).is_empty(),
            "the private function carries emitted machine-code bytes",
        );
        assert_eq!(
            &object.text_bytes()[private_plan.offset..private_plan.offset + private_plan.size],
            carrier.bytes(object),
            "the symbol's text range is the emitted thunk body",
        );
    }
    assert!(
        second.function.text_offset >= first.function.text_offset + first.function.byte_count,
        "private slots append in placement order after the program text",
    );
    realized.validate().expect("the sealed artifact replays");
}

#[test]
fn callback_thunk_materialization_rejects_foreign_or_duplicate_identities() {
    let (artifact, _, source, plans) = hosted_custody();
    let target = target::NativeTarget::windows_x64();
    let (thunk_artifact, receipt) = callback_thunk_artifact();
    let boundary = callback_boundary_entry_plan(target);
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();

    // A non-thunk identity is a foreign materialization request.
    let foreign = crate::NativeCallbackThunkSettlement {
        callback_function: function_identity::MachineFunctionIdentity::source(
            function_identity::StateKey {
                machine: receipt.source_machine,
                state: receipt.source_entry,
                segment_index: 0,
            },
        ),
        ..callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            0,
            "__omega_private_callback_foreign",
        )
    };
    let rejected = crate::realize_native_artifact(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
            .expect("canonical replay"),
        callback_thunks_request(
            target,
            &source,
            &plans,
            &profile,
            &optimizations,
            &providers,
            &[foreign],
        ),
    )
    .expect_err("a non-callback identity is not a private materialization");
    assert!(
        rejected.diagnostics().iter().any(|diagnostic| diagnostic
            .message
            .contains("InvalidPrivateFunctionIdentity")),
        "{:?}",
        rejected.diagnostics(),
    );

    // Two slots may not carry the same placement identity.
    let duplicate = [
        callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            0,
            "__omega_private_callback_a",
        ),
        callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            0,
            "__omega_private_callback_b",
        ),
    ];
    let rejected = crate::realize_native_artifact(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
            .expect("canonical replay"),
        callback_thunks_request(
            target,
            &source,
            &plans,
            &profile,
            &optimizations,
            &providers,
            &duplicate,
        ),
    )
    .expect_err("duplicate placement identities reject");
    assert!(
        rejected.diagnostics().iter().any(|diagnostic| diagnostic
            .message
            .contains("InvalidPrivateFunctionIdentity")),
        "{:?}",
        rejected.diagnostics(),
    );

    // A distinct placement may not reuse another slot's private symbol.
    let collision = [
        callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            0,
            "__omega_private_callback_same",
        ),
        callback_thunk_settlement(
            &thunk_artifact,
            boundary.plan(),
            receipt,
            1,
            "__omega_private_callback_same",
        ),
    ];
    let rejected = crate::realize_native_artifact(
        artifact,
        callback_thunks_request(
            target,
            &source,
            &plans,
            &profile,
            &optimizations,
            &providers,
            &collision,
        ),
    )
    .expect_err("duplicate private symbols reject");
    assert!(
        rejected.diagnostics().iter().any(|diagnostic| diagnostic
            .message
            .contains("PrivateFunctionSymbolCollision")),
        "{:?}",
        rejected.diagnostics(),
    );
}
