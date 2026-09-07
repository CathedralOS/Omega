use crate::tests::*;

use super::fixture::{ARM64_EXACT_USAGE, X64_EXACT_USAGE, analyze, exact_budget, source};

#[test]
fn forwarded_conditional_keeps_abi_transfers_outside_semantic_value_domains() {
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
        assert_eq!(first.receipt().register_count(), 6);
        assert_eq!(first.receipt().fragment_count(), 8);
        assert_eq!(first.receipt().segment_count(), 8);
        assert_eq!(first.receipt().incompatible_fixed_use_boundary_count(), 0);
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
            register_homes::fixed_precolored_split_requirement_plan_identity(first.plan())
        );
        let environment = fixture
            .source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let named = |name| environment.physical().model().view_named(name).unwrap().id;
        let registers = &first.plan().functions[0].registers;
        assert_eq!(
            registers[1].fragments[0].segments[0].candidates,
            [named(entry_name)]
        );
        let forwarded = &registers[3];
        assert_eq!(forwarded.fragments.len(), 3);
        for fragment in &forwarded.fragments {
            assert_eq!(fragment.segments.len(), 1);
            assert!(fragment.segments[0].candidates.len() > 1);
        }
        for fragment in &forwarded.fragments[1..] {
            assert!(matches!(
                fragment.segments[0].opening,
                register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { .. }
            ));
        }
        for returned in &registers[4..] {
            assert_eq!(
                returned.fragments[0].segments[0].candidates,
                [named(result_name)]
            );
        }

        let replayed =
            selected_instructions_to_register_homes::validate_fixed_precolored_split_requirements(
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
