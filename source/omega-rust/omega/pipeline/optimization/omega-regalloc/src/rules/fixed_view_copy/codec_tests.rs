use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_optimization_unit::{FuelSettlement, PsiProvenance};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use psi_core::{FuelScheduleIdentity, OperationId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::*;

fn plan(policy: FixedViewCopyPolicy) -> FixedViewCopyPlan {
    let (_, _, _, copy, mut function) =
        crate::rules::fixed_view_copy::compute::tests::computed_shared_fixture();
    let operation = OperationId::new(1).unwrap();
    function.provenance.operations.push(operation);
    function.blocks[0].instructions[0]
        .provenance
        .operations
        .push(operation);
    function.blocks[0].instructions[0]
        .provenance
        .fuel
        .push(FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 7,
        });
    FixedViewCopyPlan {
        source_selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
        source_ranges: LiveRangeIdentity::from_bytes([2; 32]),
        source_legality: AllocationLegalityIdentity::from_bytes([3; 32]),
        register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
        allocator_availability: AllocatorAvailabilityIdentity::from_bytes([5; 32]),
        policy,
        budget: OptimizationWorkBudget::new(3, 3, 3, 3, 1).unwrap(),
        usage: OptimizationWorkUsage {
            rule_evaluations: 1,
            candidates: 2,
            validation_steps: 2,
            commits: 1,
            iterations: 1,
        },
        copies: vec![copy],
        transformed: SelectedInstructionPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([6; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: omega_target::NativeTarget::linux_x64(),
            entry: function.machine,
            functions: vec![function],
            structural_unit_functions: Vec::new(),
        },
    }
}

#[test]
fn artifact_round_trips_both_policies_and_full_transformed_custody() {
    for policy in [
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
    ] {
        let plan = plan(policy);
        let decoded = FixedViewCopyPlan::decode(&plan.encode()).unwrap();
        assert_eq!(decoded, plan);
        assert_eq!(decoded.copies[0].destinations.len(), 2);
        assert_eq!(
            decoded.transformed.functions[0].blocks[0].instructions[0]
                .provenance
                .fuel[0]
                .units,
            7
        );
    }
}

#[test]
fn artifact_rejects_corruption_truncation_trailing_and_closed_tags() {
    let encoded = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).encode();
    let mut identity_tamper = encoded.clone();
    identity_tamper[12] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&identity_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );
    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&wrong_magic),
        Err(FixedViewCopyDecodeError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&5_u32.to_le_bytes());
    assert_eq!(
        FixedViewCopyPlan::decode(&wrong_version),
        Err(FixedViewCopyDecodeError::UnsupportedVersion(5))
    );
    let mut policy_tag = encoded.clone();
    let policy_offset = 8 + 4 + 32 + (5 * 32);
    policy_tag[policy_offset] = 99;
    assert_eq!(
        FixedViewCopyPlan::decode(&policy_tag),
        Err(FixedViewCopyDecodeError::UnknownPolicy(99))
    );
    let mut source_identity_tamper = encoded.clone();
    source_identity_tamper[44] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&source_identity_tamper),
        Err(FixedViewCopyDecodeError::IdentityMismatch)
    );
    let mut cursor = Cursor::new(&encoded);
    cursor.take(44 + (5 * 32) + 1 + 40 + 40).unwrap();
    let copy_count = cursor.length().unwrap();
    for _ in 0..copy_count {
        decode_copy(&mut cursor).unwrap();
    }
    let mut transformed_identity_tamper = encoded.clone();
    transformed_identity_tamper[cursor.offset] ^= 1;
    assert_eq!(
        FixedViewCopyPlan::decode(&transformed_identity_tamper),
        Err(FixedViewCopyDecodeError::TransformedIdentityMismatch)
    );
    assert_eq!(
        FixedViewCopyPlan::decode(&encoded[..encoded.len() - 1]),
        Err(FixedViewCopyDecodeError::Truncated)
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        FixedViewCopyPlan::decode(&trailing),
        Err(FixedViewCopyDecodeError::TrailingBytes)
    );
    assert_eq!(
        decode_fixed_site(&mut Cursor::new(&[9])),
        Err(FixedViewCopyDecodeError::UnknownFixedSite(9))
    );
    assert_eq!(
        decode_kind(&mut Cursor::new(&[10])),
        Err(FixedViewCopyDecodeError::UnknownInstructionKind(10))
    );
}
