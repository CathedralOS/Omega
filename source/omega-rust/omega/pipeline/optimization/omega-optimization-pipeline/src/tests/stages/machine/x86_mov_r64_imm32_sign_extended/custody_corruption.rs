//! Authenticated one-field corruption coverage for generic machine custody.

use omega_optimization_core::OptimizationSelectionIdentity;

use crate::tests::*;

#[test]
fn every_post_allocation_custody_field_rejects_after_outer_reauthentication() {
    let mut realization = staged_realization();
    let canonical = realization
        .manifest()
        .record()
        .post_allocation_machine_optimization
        .expect("the exact rule must publish generic machine custody");
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(&realization)
            .unwrap(),
        *realization.custody()
    );

    for field in CustodyField::ALL {
        let corrupted = field.corrupt(canonical);
        let record = realization.manifest_mut().record_mut();
        record.post_allocation_machine_optimization = Some(corrupted);
        record.identity = record.recomputed_identity();
        assert_eq!(
            FunctionRelativeOptimizationRealizationManifest::decode(&record.encode()),
            Ok(record.clone()),
            "the {field:?} mutation must have a valid outer envelope before source replay",
        );
        assert_eq!(
            validate_post_allocation_machine_function_relative_realization_custody(&realization),
            Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
            "independent source replay must reject the authenticated {field:?} mutation",
        );
    }

    let record = realization.manifest_mut().record_mut();
    record.post_allocation_machine_optimization = Some(canonical);
    record.identity = record.recomputed_identity();
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(&realization)
            .unwrap(),
        *realization.custody()
    );
}

#[derive(Debug, Clone, Copy)]
enum CustodyField {
    Optimization,
    ArtifactIdentity,
    Selections,
    PostAllocationSelections,
    Source,
    ActionCount,
    BaselineBytes,
    SelectedBytes,
}

impl CustodyField {
    const ALL: [Self; 8] = [
        Self::Optimization,
        Self::ArtifactIdentity,
        Self::Selections,
        Self::PostAllocationSelections,
        Self::Source,
        Self::ActionCount,
        Self::BaselineBytes,
        Self::SelectedBytes,
    ];

    fn corrupt(
        self,
        canonical: PostAllocationMachineOptimizationCustody,
    ) -> PostAllocationMachineOptimizationCustody {
        let mut optimization = canonical.optimization();
        let mut artifact_identity = canonical.artifact_identity();
        let mut selections = canonical.selections();
        let mut phase_selections = canonical.post_allocation_machine_selections();
        let mut source = canonical.source();
        let mut action_count = canonical.action_count();
        let mut baseline_bytes = canonical.baseline_bytes();
        let mut selected_bytes = canonical.selected_bytes();

        match self {
            Self::Optimization => {
                optimization = Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1;
            }
            Self::ArtifactIdentity => artifact_identity[0] ^= 1,
            Self::Selections => selections = OptimizationSelectionIdentity::from_bytes([0xa1; 32]),
            Self::PostAllocationSelections => {
                phase_selections = OptimizationSelectionIdentity::from_bytes([0xa2; 32]);
            }
            Self::Source => {
                source =
                    omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([0xa3; 32]);
            }
            Self::ActionCount => action_count = action_count.checked_add(1).unwrap(),
            Self::BaselineBytes => baseline_bytes = baseline_bytes.checked_add(1).unwrap(),
            Self::SelectedBytes => selected_bytes = selected_bytes.checked_add(1).unwrap(),
        }

        PostAllocationMachineOptimizationCustody::from_parts(
            optimization,
            artifact_identity,
            selections,
            phase_selections,
            source,
            action_count,
            baseline_bytes,
            selected_bytes,
        )
    }
}

/// Test-only canonical construction. This helper grants no production
/// admission or policy authority.
fn staged_realization() -> StagedPostAllocationMachineFunctionRelativeRealization {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine_fixture = conditional_immediate_machine(
        18_401,
        integer_type,
        [u128::from(i32::MAX as u32), u128::from(u64::MAX)],
    );
    let module = conditional_immediate_module(machine_fixture.id, vec![machine_fixture]);
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&ProofBundle {
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections = OptimizationSelections::new([
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let optimization =
        stage_optimized_post_allocation_machine_optimization(&homes, &machine).unwrap();
    stage_post_allocation_machine_function_relative_realization(homes, machine, optimization)
        .unwrap()
}
