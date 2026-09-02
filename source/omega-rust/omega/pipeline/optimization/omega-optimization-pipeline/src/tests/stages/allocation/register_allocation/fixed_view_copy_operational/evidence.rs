//! Segment-home roots are mandatory and independently replayed.

use crate::tests::*;
use omega_regalloc::{
    FixedPrecoloredIntervalPlanIdentity, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedViewCopySourceEvidence,
};

use super::fixture::{run, targets};

fn generous_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000, 1_000, 1_000, 1_000, 1_000).unwrap()
}

fn replay(
    staged: &StagedOptimizedFixedViewCopies,
    plan: omega_regalloc::FixedViewCopyPlan,
) -> Result<omega_regalloc::ValidatedFixedViewCopies, FixedViewCopyError> {
    let source = staged.source_segment_home_stage();
    let legality = source.source_legality_stage();
    let selected = legality
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected.register_environment();
    validate_fixed_view_copies(
        selected.selected(),
        legality.live_range_stage().ranges(),
        legality.legality(),
        source.fixed_intervals(),
        source.split_requirements(),
        source.segment_homes(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        plan,
    )
}

#[test]
fn shared_entry_copy_consumes_every_authenticated_boundary_and_binds_all_three_roots() {
    for target in targets() {
        let staged = run(target, generous_budget()).unwrap();
        let source = staged.source_segment_home_stage();
        let expected = FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals: source.fixed_intervals().receipt().identity(),
            split_requirements: source.split_requirements().receipt().identity(),
            segment_homes: source.segment_homes().receipt().identity(),
        };
        assert_eq!(staged.copies().plan().source_evidence, expected);
        assert_eq!(staged.copies().receipt().source_evidence(), expected);
        assert_eq!(
            staged.custody().fixed_intervals(),
            source.fixed_intervals().receipt().identity()
        );
        assert_eq!(
            staged.custody().split_requirements(),
            source.split_requirements().receipt().identity()
        );
        assert_eq!(
            staged.custody().segment_homes(),
            source.segment_homes().receipt().identity()
        );
        assert_eq!(
            source
                .split_requirements()
                .receipt()
                .incompatible_fixed_use_boundary_count(),
            2
        );
        assert_eq!(staged.copies().plan().copies.len(), 1);
        assert_eq!(staged.copies().plan().copies[0].destinations.len(), 2);
        let forwarded = source.segment_homes().plan().functions[0]
            .assignments
            .iter()
            .filter(|assignment| assignment.virtual_register == VirtualRegisterId(1))
            .collect::<Vec<_>>();
        assert_eq!(forwarded.len(), 3);
        let copy = &staged.copies().plan().copies[0];
        assert_eq!(copy.from_view, forwarded[0].view);
        assert_eq!(copy.to_view, forwarded[1].view);
        assert_eq!(copy.destinations[0].view, forwarded[1].view);
        assert_eq!(copy.destinations[1].view, forwarded[2].view);
        assert_ne!(
            forwarded[0].allocation_domain,
            forwarded[1].allocation_domain
        );
        assert_ne!(
            forwarded[0].allocation_domain,
            forwarded[2].allocation_domain
        );
        assert_eq!(
            omega_regalloc::FixedViewCopyPlan::decode(&staged.copies().plan().encode()).unwrap(),
            *staged.copies().plan()
        );
    }
}

#[test]
fn legacy_and_each_segment_source_identity_fail_closed() {
    let staged = run(NativeTarget::linux_x64(), generous_budget()).unwrap();

    let mut legacy = staged.copies().plan().clone();
    legacy.source_evidence = FixedViewCopySourceEvidence::LegacyLegalityTransitionsV1;
    assert_eq!(
        replay(&staged, legacy),
        Err(FixedViewCopyError::LegacySourceEvidence)
    );

    for axis in 0..3 {
        let mut corrupted = staged.copies().plan().clone();
        let FixedViewCopySourceEvidence::FixedPrecoloredSegmentHomesV1 {
            fixed_intervals,
            split_requirements,
            segment_homes,
        } = &mut corrupted.source_evidence
        else {
            panic!("current fixed-view-copy plans require segment-home evidence")
        };
        match axis {
            0 => *fixed_intervals = FixedPrecoloredIntervalPlanIdentity::from_bytes([41; 32]),
            1 => {
                *split_requirements =
                    FixedPrecoloredSplitRequirementPlanIdentity::from_bytes([42; 32])
            }
            2 => *segment_homes = FixedPrecoloredSegmentHomePlanIdentity::from_bytes([43; 32]),
            _ => unreachable!(),
        }
        assert_eq!(
            replay(&staged, corrupted),
            Err(FixedViewCopyError::SegmentEvidenceMismatch)
        );
    }
}

#[test]
fn cross_target_segment_evidence_fails_closed() {
    let x64 = run(NativeTarget::linux_x64(), generous_budget()).unwrap();
    let arm64 = run(NativeTarget::linux_arm64(), generous_budget()).unwrap();
    let mut mixed = x64.copies().plan().clone();
    mixed.source_evidence = arm64.copies().plan().source_evidence;
    assert_eq!(
        replay(&x64, mixed),
        Err(FixedViewCopyError::SegmentEvidenceMismatch)
    );
}
