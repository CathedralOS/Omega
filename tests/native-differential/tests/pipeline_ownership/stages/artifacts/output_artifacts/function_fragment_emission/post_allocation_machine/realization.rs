use crate::tests::*;

use super::cases::{ExpectedActions, FixtureKind, HostedCase};

pub(super) struct RealizedCase {
    pub(super) case: HostedCase,
    pub(super) semantic: Vec<u8>,
    pub(super) proof: Vec<u8>,
    pub(super) selections: OptimizationSelections,
    pub(super) realization: StagedPostAllocationMachineFunctionRelativeRealization,
}

pub(super) fn realize(case: HostedCase) -> RealizedCase {
    let (semantic, proof) = artifact(case.fixture);
    let selections = OptimizationSelections::new([case.rule]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        case.target,
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| {
            panic!("an exact machine rule must use the post-allocation realization route")
        });
    assert_eq!(realization.optimization().optimization(), case.rule);
    match case.actions {
        ExpectedActions::Zero => assert_eq!(realization.optimization().action_count(), 0),
        ExpectedActions::NonZero => assert!(realization.optimization().action_count() > 0),
    }
    assert_eq!(realization.manifest().record().target, case.target);
    assert_eq!(
        realization.manifest().record().selections,
        selections.identity()
    );
    assert_eq!(
        realization.exit_contract().contract().policy,
        case.exit_policy
    );
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(&realization)
            .unwrap(),
        *realization.custody()
    );

    RealizedCase {
        case,
        semantic,
        proof,
        selections,
        realization,
    }
}

fn artifact(fixture: FixtureKind) -> (Vec<u8>, Vec<u8>) {
    match fixture {
        FixtureKind::ExactBinary => conditional_exact_binary_artifact(false),
        FixtureKind::Movn => {
            conditional_active_resident_exact_add_chain_artifact_with_false_literal(
                IntegerValue::Unsigned(u64::MAX as u128),
            )
        }
        FixtureKind::XorZero => immediate_artifact(19_000, [0, 1]),
        FixtureKind::MovR32 => immediate_artifact(19_100, [1, u32::MAX.into()]),
        FixtureKind::MovR64 => {
            immediate_artifact(19_200, [u128::from(i32::MAX as u32), u128::from(u64::MAX)])
        }
    }
}

fn immediate_artifact(machine: u64, values: [u128; 2]) -> (Vec<u8>, Vec<u8>) {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine = conditional_immediate_machine(machine, integer_type, values);
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    (semantic, proof)
}
