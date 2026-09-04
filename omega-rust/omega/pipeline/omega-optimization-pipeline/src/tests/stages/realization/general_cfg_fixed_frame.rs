use crate::tests::*;
use omega_machine_code::FunctionFragmentControlProvenance;

#[test]
fn nonzero_frames_reflow_three_block_returns_through_callable_publication() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (applied, prologue, epilogue, source_displacement, source_byte_count) =
            staged_application(target);
        assert_eq!(applied.receipt().framed_function_count(), 1);
        assert_eq!(applied.receipt().epilogue_application_count(), 2);
        let application = &applied.application().functions[0];
        assert_eq!(application.epilogues.len(), 2);
        let function = &applied.fragments().functions[0];
        assert_eq!(function.blocks.len(), 3);
        assert_eq!(
            function.byte_count,
            source_byte_count + prologue.len() as u64 + 2 * epilogue.len() as u64
        );
        assert_eq!(&function.bytes[..prologue.len()], prologue);
        for site in &application.epilogues {
            let start = usize::try_from(site.function_offset).unwrap();
            assert_eq!(&function.bytes[start..start + epilogue.len()], epilogue);
            let returned = function
                .blocks
                .iter()
                .find(|block| block.block == site.block)
                .unwrap()
                .instructions
                .iter()
                .find(|row| row.instruction == site.return_instruction)
                .unwrap();
            assert_eq!(returned.offset, site.function_offset + site.byte_count);
            assert!(matches!(
                returned.control,
                FunctionFragmentControlProvenance::Return { psi_return_edge }
                    if psi_return_edge == site.psi_return_edge
            ));
        }
        let branch_row = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.branch.is_some())
            .unwrap();
        let branch = branch_row.branch.as_deref().unwrap();
        assert_eq!(
            branch.when_fallthrough_offset,
            branch_row.offset + branch_row.bytes.len() as u64
        );
        assert_eq!(
            branch.byte_displacement,
            source_displacement + epilogue.len() as i64
        );

        let application_identity = applied.receipt().identity();
        let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
        let object = stage_optimized_relocation_free_object_container(text).unwrap();
        let (semantic, proof) = conditional_exact_binary_artifact(false);
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap();
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
        assert_eq!(callable.entry().returns.len(), 2);
        let StagedOptimizedObjectTextSectionSource::FixedFrame(fixed) =
            callable.source().source().source()
        else {
            panic!("general CFG callable must retain fixed-frame custody")
        };
        assert!(matches!(
            fixed.manifest().record().source_custody,
            FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 { application }
                if application == application_identity
        ));
    }
}

#[test]
fn independent_replay_rejects_reauthenticated_site_and_branch_corruption() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (mut site_corruption, ..) = staged_application(target);
        site_corruption.corrupt_first_epilogue_site_for_test();
        assert_eq!(
            validate_function_fragment_frame_application(&site_corruption),
            Err(FunctionFragmentFrameApplicationError::ArtifactMismatch)
        );

        let (mut branch_corruption, ..) = staged_application(target);
        branch_corruption.corrupt_first_branch_byte_for_test();
        assert!(matches!(
            validate_function_fragment_frame_application(&branch_corruption),
            Err(FunctionFragmentFrameApplicationError::X86_64Branch(_, _)
                | FunctionFragmentFrameApplicationError::Aarch64Branch(_, _))
        ));
    }
}

#[test]
fn frame_application_rejects_a_non_fixed_fragment_source() {
    let target = NativeTarget::linux_x64();
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap(),
            selected_lowering_budget(),
        )
        .unwrap(),
    )
    .unwrap();
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let source = {
        let source = (physical).into_function_fragment_emission_source();
        assert!(matches!(
            &source,
            StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(_)
        ));
        source
    };
    let fragments = stage_optimized_function_fragment_emission(source).unwrap();
    assert!(matches!(
        stage_function_fragment_frame_application(fragments),
        Err(FunctionFragmentFrameApplicationError::SourceKindMismatch)
    ));
}

fn staged_application(
    target: NativeTarget,
) -> (
    StagedFunctionFragmentFrameApplication,
    Vec<u8>,
    Vec<u8>,
    i64,
    u64,
) {
    let selected = staged_exact_add_conditional(target);
    let ranges = stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap();
    let legality = match target.architecture {
        omega_target::Architecture::X86_64 => stage_optimized_allocation_legality(ranges).unwrap(),
        omega_target::Architecture::Aarch64 => {
            let environment = ranges
                .liveness_stage()
                .selected_stage()
                .register_environment();
            let views = ["x0", "x19"]
                .into_iter()
                .map(|name| environment.physical().model().view_named(name).unwrap().id)
                .collect();
            let availability = materialize_allocator_availability(
                environment.identity(),
                target,
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                environment.allocation_constraint_keys(),
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views },
            )
            .unwrap();
            stage_optimized_allocation_legality_with_availability(ranges, availability).unwrap()
        }
    };
    let homes = stage_optimized_register_homes(legality)
        .unwrap_or_else(|error| panic!("{target:?} home assignment failed: {error:?}"));
    let realization = stage_fixed_frame_function_relative_realization(
        homes,
        OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000).unwrap(),
    )
    .unwrap();
    let protocol = realization.protocol().plan();
    let protocol_row = protocol.functions.first().unwrap();
    let prologue = protocol_row
        .prologue
        .bytes(&protocol.bytes)
        .unwrap()
        .to_vec();
    let epilogue = protocol_row
        .epilogue
        .bytes(&protocol.bytes)
        .unwrap()
        .to_vec();
    assert!(!prologue.is_empty());
    assert!(!epilogue.is_empty());
    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(Box::new(realization)),
    )
    .unwrap();
    let source_function = fragments.fragments().functions.first().unwrap();
    assert_eq!(source_function.blocks.len(), 3);
    let source_displacement = source_function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|row| row.branch.as_deref())
        .unwrap()
        .byte_displacement;
    let source_byte_count = source_function.byte_count;
    (
        stage_function_fragment_frame_application(fragments).unwrap(),
        prologue,
        epilogue,
        source_displacement,
        source_byte_count,
    )
}
