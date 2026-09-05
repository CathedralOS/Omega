use crate::FunctionFragmentReplayInputs;
use crate::tests::*;

pub(super) fn staged_callable_object_artifact(
    target: NativeTarget,
    selected_lowering: bool,
) -> StagedValidatedOptimizedObjectArtifact {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let layout = match target.architecture {
        target::Architecture::X86_64 => Optimization::X86RelaxConditionalBranchesToRel8V1,
        target::Architecture::Aarch64 => {
            Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
        }
    };
    let selections = if selected_lowering {
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate, layout])
            .unwrap()
    } else {
        OptimizationSelections::new([layout]).unwrap()
    };
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let source = {
        let source = (physical).into_function_fragment_emission_source();
        assert!(matches!(
            source.replay_for_test(),
            FunctionFragmentReplayInputs::X86Rel8Direct(_)
                | FunctionFragmentReplayInputs::PostAllocationMachine(_)
                | FunctionFragmentReplayInputs::SelectedLowering(_)
        ));
        source
    };
    let fragments = stage_optimized_function_fragment_emission(source).unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
        .unwrap()
}

fn staged_active_resident_callable_object_artifact(
    target: NativeTarget,
) -> StagedValidatedOptimizedObjectArtifact {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let realization = (physical)
        .into_allocation_recovery_for_test()
        .unwrap_or_else(|| {
            panic!("the root-build rematerialization selection must retain its owning realization")
        });
    let fragments = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::AllocationRecovery(realization).into(),
    )
    .unwrap();
    let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
        .unwrap()
}

#[test]
fn active_resident_root_build_reaches_object_artifact_and_ordinary_callable_on_both_isas() {
    use calling_conventions::{CallingPolicy, MachineRegister};

    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
    ])
    .unwrap()
    .identity();
    for (target, policy, parameter, result) in [
        (
            NativeTarget::linux_x64(),
            CallingPolicy::SystemVAMD64,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rax,
        ),
        (
            NativeTarget::windows_x64(),
            CallingPolicy::MicrosoftX64,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rax,
        ),
        (
            NativeTarget::linux_arm64(),
            CallingPolicy::Aapcs64,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(0),
        ),
        (
            NativeTarget::macos_arm64(),
            CallingPolicy::Aapcs64,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(0),
        ),
    ] {
        let artifact = staged_active_resident_callable_object_artifact(target);
        let object_stage = artifact.source();
        let text_stage = object_stage.source();
        let StagedOptimizedObjectTextSectionSource::Direct(direct_text_stage) = text_stage else {
            panic!("active-resident publication must retain direct text custody")
        };
        let fragment_stage = direct_text_stage.source();
        let FunctionFragmentReplayInputs::AllocationRecovery(realization) =
            fragment_stage.source().replay_for_test()
        else {
            panic!("object custody must retain the rematerialization realization")
        };
        let current = realization.allocation().current();
        let rematerialization = realization
            .allocation()
            .rematerialization_proof_for_test()
            .unwrap();
        let AllocationEvidence::ActiveResidentRematerialization(_) = current.evidence() else {
            panic!("fixture must retain rematerialization evidence")
        };
        let fresh = rematerialization.plan().functions[0]
            .action
            .as_ref()
            .expect("the exact root-build family must apply one rematerialization")
            .fresh_materialize;
        let emitted_fresh = text_stage.text_section().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == fresh)
            .expect("the object text section must retain the fresh materialization span");
        assert_ne!(emitted_fresh.byte_count, 0);
        assert_eq!(
            validate_optimized_function_fragment_emission(fragment_stage).unwrap(),
            fragment_stage.custody()
        );
        assert_eq!(
            validate_optimized_relocation_free_text_section(direct_text_stage).unwrap(),
            direct_text_stage.custody()
        );
        assert_eq!(
            validate_optimized_relocation_free_object_container(object_stage).unwrap(),
            object_stage.custody()
        );
        assert_eq!(fragment_stage.manifest().record().selections, selections);
        assert_eq!(text_stage.manifest().record().selections, selections);
        assert_eq!(object_stage.manifest().record().selections, selections);
        assert_eq!(artifact.artifact().selections, selections);
        assert_eq!(
            realization
                .manifest()
                .record()
                .allocation_recovery_selections,
            selections
        );
        assert_eq!(
            fragment_stage.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        );
        assert_eq!(
            current
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization.receipt().identity(),
                )
            ]
        );
        assert_eq!(object_stage.object().relocation_record_count, 0);
        assert_eq!(object_stage.object().symbols.len(), 1);
        assert_eq!(
            object_stage.object().text_section.bytes,
            text_stage.text_section().bytes
        );
        assert_eq!(
            object_stage.object().semantic_entry_symbol,
            object_stage.object().symbols[0].symbol
        );
        assert_eq!(
            object_stage.object().symbols[0].linkage,
            object_file::RelocationFreeObjectSymbolLinkage::ObjectLocalV1
        );
        assert_eq!(
            object_stage.object().symbols[0].role,
            object_file::RelocationFreeObjectSymbolRole::SemanticEntryV1
        );
        assert_ne!(object_stage.object().symbols[0].name, "main");
        assert_ne!(object_stage.object().symbols[0].name, "_main");
        assert_eq!(
            artifact.artifact().pre_physical_manifest,
            fragment_stage
                .source()
                .pre_physical_manifest()
                .record()
                .identity
        );
        assert_eq!(
            artifact.artifact().post_allocation_manifest,
            fragment_stage
                .source()
                .post_allocation_manifest()
                .record()
                .identity
        );
        assert_eq!(
            validate_optimized_object_artifact(&artifact).unwrap(),
            artifact.custody()
        );
        assert_eq!(
            OptimizedObjectArtifactRecord::decode(&artifact.artifact().encode()).unwrap(),
            *artifact.artifact()
        );
        assert_eq!(
            OptimizedObjectArtifactManifest::decode(&artifact.manifest().record().encode())
                .unwrap(),
            *artifact.manifest().record()
        );
        let artifact_report = optimization_pipeline_report_from_object_artifact(&artifact);
        assert_eq!(
            artifact_report.function_fragment().unwrap().source_kind,
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        );
        assert!(artifact_report.ordinary_callable_entry().is_none());

        let staged = stage_validated_optimized_ordinary_callable_entry(artifact)
            .expect("the rematerialized semantic entry remains an ordinary callable");
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged).unwrap(),
            staged.custody()
        );
        let entry = staged.entry();
        assert_eq!(entry.selections, selections);
        assert_eq!(entry.calling_policy, policy);
        assert_eq!(entry.parameters.len(), 1);
        assert_eq!(entry.parameters[0].abi_register, parameter);
        assert_eq!(
            entry.parameters[0].fixed_view,
            entry.parameters[0].assigned_view
        );
        assert_eq!(entry.result.abi_register, result);
        assert_eq!(entry.returns.len(), 2);
        assert!(entry.returns.iter().all(|returned| {
            returned.view == entry.result.view
                && returned.storage_units == entry.result.storage_units
        }));
        assert_eq!(
            entry.disposition,
            OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&entry.encode().unwrap()).unwrap(),
            *entry
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&staged.manifest().record().encode())
                .unwrap(),
            *staged.manifest().record()
        );
        let report = optimization_pipeline_report_from_ordinary_callable_entry(&staged);
        assert_eq!(
            report.function_fragment().unwrap().source_kind,
            FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
        );
        assert_eq!(
            report.object_container().unwrap().identity,
            staged.source().source().manifest().record().identity
        );
        assert_eq!(
            report.object_artifact().unwrap().artifact,
            staged.source().artifact().identity
        );
        assert_eq!(
            report.ordinary_callable_entry().unwrap().entry,
            entry.identity
        );
        let human = report
            .render_human_text(OptimizationReportRequest::EmitHumanText)
            .unwrap();
        assert!(human.contains("external process entry bridge: required"));
        assert!(human.contains("publication: unavailable"));
    }
}

#[test]
fn ordinary_callable_entry_replays_target_abi_and_edge_specific_results() {
    use calling_conventions::{CallingPolicy, MachineRegister};

    for (target, policy, parameter, result) in [
        (
            NativeTarget::linux_x64(),
            CallingPolicy::SystemVAMD64,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rax,
        ),
        (
            NativeTarget::windows_x64(),
            CallingPolicy::MicrosoftX64,
            MachineRegister::X86Rcx,
            MachineRegister::X86Rax,
        ),
        (
            NativeTarget::linux_arm64(),
            CallingPolicy::Aapcs64,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(0),
        ),
        (
            NativeTarget::macos_arm64(),
            CallingPolicy::Aapcs64,
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(0),
        ),
    ] {
        let artifact = staged_callable_object_artifact(target, false);
        let object_identity = artifact.source().object().identity;
        let object_bytes = artifact.source().container().bytes.clone();
        let staged = stage_validated_optimized_ordinary_callable_entry(artifact)
            .expect("ordinary callable classification");
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged).unwrap(),
            staged.custody()
        );
        let entry = staged.entry();
        assert_eq!(entry.calling_policy, policy);
        assert_eq!(entry.parameters.len(), 1);
        assert_eq!(entry.parameters[0].abi_register, parameter);
        assert_eq!(
            entry.parameters[0].fixed_view,
            entry.parameters[0].assigned_view
        );
        assert_eq!(entry.result.abi_register, result);
        assert_eq!(entry.returns.len(), 2);
        assert_ne!(entry.returns[0].value, entry.returns[1].value);
        assert_ne!(
            entry.returns[0].virtual_register,
            entry.returns[1].virtual_register
        );
        assert!(entry.returns.iter().all(|returned| {
            returned.view == entry.result.view
                && returned.storage_units == entry.result.storage_units
        }));
        assert_eq!(staged.source().source().object().identity, object_identity);
        assert_eq!(staged.source().source().container().bytes, object_bytes);
        assert_eq!(staged.source().source().object().relocation_record_count, 0);
        assert_ne!(entry.semantic_entry_symbol_name, "main");
        assert_ne!(entry.semantic_entry_symbol_name, "_main");
        assert_eq!(
            OptimizedOrdinaryCallableEntryRecord::decode(&entry.encode().unwrap()).unwrap(),
            *entry
        );
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&staged.manifest().record().encode())
                .unwrap(),
            *staged.manifest().record()
        );
    }
}

#[test]
fn ordinary_callable_entry_accepts_both_selected_lowering_compositions_and_reports_opaquely() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        std::thread::Builder::new()
            .name("ordinary-callable-custody-replay".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || {
                let staged = stage_validated_optimized_ordinary_callable_entry(
                    staged_callable_object_artifact(target, true),
                )
                .unwrap();
                let artifact_identity = staged.source().artifact().identity;
                let container_identity = staged.source().source().container().identity;
                let container_bytes = staged.source().source().container().bytes.clone();
                assert_eq!(
                    validate_optimized_ordinary_callable_entry(&staged).unwrap(),
                    staged.custody()
                );
                let prior = optimization_pipeline_report_from_object_artifact(staged.source());
                assert!(prior.ordinary_callable_entry().is_none());
                let report = optimization_pipeline_report_from_ordinary_callable_entry(&staged);
                assert_eq!(
                    report.ordinary_callable_entry(),
                    Some(staged.manifest().record())
                );
                assert!(
                    report
                        .render_human_text(OptimizationReportRequest::Suppressed)
                        .is_none()
                );
                let text = report
                    .render_human_text(OptimizationReportRequest::EmitHumanText)
                    .unwrap();
                assert!(text.contains("external process entry bridge: required"));
                assert!(text.contains("publication: unavailable"));
                assert_eq!(staged.source().artifact().identity, artifact_identity);
                assert_eq!(
                    staged.source().source().container().identity,
                    container_identity
                );
                assert_eq!(staged.source().source().container().bytes, container_bytes);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[test]
fn ordinary_callable_entry_rejects_record_manifest_and_codec_corruption() {
    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    staged.entry_mut().returns[0].value = ValueId::new(99_991).unwrap();
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::RecordMismatch)
    );

    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    staged.entry_mut().parameters[0].storage_units.clear();
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::RecordMismatch)
    );

    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    staged.entry_mut().semantic_entry_symbol_name = "main".to_owned();
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::RecordMismatch)
    );

    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    staged.entry_mut().exit_policy = WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1;
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::RecordMismatch)
    );

    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    staged.manifest_mut().record_mut().return_count += 1;
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch)
    );

    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    staged.corrupt_custody_manifest_for_test();
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::ReceiptMismatch)
    );

    let staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    let mut wrong_magic = staged.entry().encode().unwrap();
    wrong_magic[0] ^= 1;
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&wrong_magic),
        Err(OptimizedOrdinaryCallableEntryDecodeError::WrongMagic)
    );
    let mut wrong_version = staged.manifest().record().encode();
    wrong_version[8..12].copy_from_slice(&5_u32.to_le_bytes());
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&wrong_version),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(5))
    );
    let mut legacy_version = staged.manifest().record().encode();
    legacy_version[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&legacy_version),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(1))
    );
    let mut trailing = staged.entry().encode().unwrap();
    trailing.push(0);
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&trailing),
        Err(OptimizedOrdinaryCallableEntryDecodeError::TrailingBytes)
    );
    let mut stale = staged.entry().encode().unwrap();
    stale[12] ^= 1;
    assert_eq!(
        OptimizedOrdinaryCallableEntryRecord::decode(&stale),
        Err(OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch)
    );
}
