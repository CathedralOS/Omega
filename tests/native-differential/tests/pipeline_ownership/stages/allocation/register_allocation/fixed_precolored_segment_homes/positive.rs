use crate::tests::*;

use super::fixture::{assign, exact_usage, generous_budget, source, validate};

#[test]
fn forwarded_conditional_assigns_exact_segment_domains_without_claiming_movement() {
    for (target, source_name, destination_name) in [
        (NativeTarget::linux_x64(), "rsi", "rax"),
        (NativeTarget::linux_arm64(), "x1", "x0"),
    ] {
        let fixture = source(target);
        let first = assign(&fixture, generous_budget()).unwrap();
        let second = assign(&fixture, generous_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().target(), target);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().structural_unit_function_count(), 0);
        assert_eq!(first.receipt().domain_count(), 6);
        assert_eq!(first.receipt().assignment_count(), 8);
        assert_eq!(first.receipt().usage(), exact_usage(target));
        assert_eq!(
            first.receipt().identity(),
            register_homes::fixed_precolored_segment_home_plan_identity(first.plan())
        );

        let environment = fixture
            .source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let named = |name| environment.physical().model().view_named(name).unwrap().id;
        let assignments = &first.plan().functions[0].assignments;
        assert_eq!(assignments.len(), 8);
        let forwarded = assignments
            .iter()
            .filter(|assignment| assignment.virtual_register == VirtualRegisterId(3))
            .collect::<Vec<_>>();
        assert_eq!(forwarded.len(), 3);
        assert!(
            assignments
                .iter()
                .any(|row| row.virtual_register == VirtualRegisterId(1)
                    && row.view == named(source_name))
        );
        assert!(
            assignments
                .iter()
                .filter(|row| row.virtual_register.0 >= 4)
                .all(|row| row.view == named(destination_name))
        );
        assert_eq!(forwarded[0].view, forwarded[1].view);
        assert_eq!(forwarded[0].view, forwarded[2].view);
        assert_eq!(
            forwarded[0].allocation_domain,
            forwarded[1].allocation_domain
        );
        assert_eq!(
            forwarded[0].allocation_domain,
            forwarded[2].allocation_domain
        );
        assert_eq!(
            forwarded[1].allocation_domain,
            forwarded[2].allocation_domain
        );

        let replayed = validate(&fixture, first.plan().clone()).unwrap();
        assert_eq!(replayed, first);
        assert_eq!(
            first.receipt().split_requirements(),
            fixture.requirements.receipt().identity()
        );
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
