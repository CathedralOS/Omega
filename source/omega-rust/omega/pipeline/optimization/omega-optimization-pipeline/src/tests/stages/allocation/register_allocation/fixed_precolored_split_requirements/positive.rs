use crate::tests::*;

use super::fixture::{ARM64_EXACT_USAGE, X64_EXACT_USAGE, analyze, exact_budget, source};

#[test]
fn forwarded_conditional_exposes_two_factual_fixed_use_domain_boundaries() {
    for (target, entry_name, result_name) in [
        (NativeTarget::linux_x64(), "rsi", "rax"),
        (NativeTarget::linux_arm64(), "x1", "x0"),
    ] {
        let fixture = source(target);
        let first = analyze(&fixture, exact_budget(target)).unwrap();
        let second = analyze(&fixture, exact_budget(target)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().target(), target);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().structural_unit_function_count(), 0);
        assert_eq!(first.receipt().register_count(), 2);
        assert_eq!(first.receipt().fragment_count(), 4);
        assert_eq!(first.receipt().segment_count(), 4);
        assert_eq!(first.receipt().incompatible_fixed_use_boundary_count(), 2);
        assert_eq!(
            first.receipt().usage(),
            if target == NativeTarget::linux_x64() {
                X64_EXACT_USAGE
            } else {
                ARM64_EXACT_USAGE
            }
        );
        assert_eq!(
            first.receipt().identity(),
            omega_regalloc::fixed_precolored_split_requirement_plan_identity(first.plan())
        );
        let environment = fixture
            .source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let named = |name| environment.physical().model().view_named(name).unwrap().id;
        let registers = &first.plan().functions[0].registers;
        assert_eq!(registers[0].fragments.len(), 1);
        assert!(matches!(
            registers[0].fragments[0].segments[0].opening,
            omega_regalloc::FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1
        ));
        let forwarded = &registers[1];
        assert_eq!(forwarded.fragments.len(), 3);
        assert_eq!(
            forwarded.fragments[0].segments[0].candidates,
            [named(entry_name)]
        );
        for fragment in &forwarded.fragments[1..] {
            assert_eq!(fragment.segments.len(), 1);
            assert_eq!(fragment.segments[0].candidates, [named(result_name)]);
            assert!(matches!(
                fragment.segments[0].opening,
                omega_regalloc::FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                    incoming: Some(_),
                    destination_view,
                    ..
                } if destination_view == named(result_name)
            ));
        }

        let replayed = omega_regalloc::validate_fixed_precolored_split_requirements(
            fixture.source.live_range_stage().ranges(),
            fixture.source.legality(),
            &fixture.fixed,
            first.plan().clone(),
        )
        .unwrap();
        assert_eq!(replayed, first);
        assert_eq!(
            first.receipt().fixed_intervals(),
            fixture.fixed.receipt().identity()
        );
        assert_eq!(
            first.receipt().ranges(),
            fixture
                .source
                .live_range_stage()
                .ranges()
                .receipt()
                .identity()
        );
        assert_eq!(
            first.receipt().legality(),
            fixture.source.legality().receipt().identity()
        );
    }
}
