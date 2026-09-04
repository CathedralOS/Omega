use crate::tests::*;

use super::fixture::{EXACT_USAGE, analyze, exact_budget, source};

#[test]
fn selected_fixed_constraints_become_exact_precolored_point_intervals() {
    for (target, boolean_entry, integer_entry, integer_result) in [
        (NativeTarget::linux_x64(), "rdi", "rsi", "rax"),
        (NativeTarget::linux_arm64(), "x0", "x1", "x0"),
    ] {
        let source = source(target);
        let first = analyze(&source, exact_budget()).unwrap();
        let second = analyze(&source, exact_budget()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().usage(), EXACT_USAGE);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().structural_unit_function_count(), 0);
        assert_eq!(first.receipt().inspected_register_count(), 2);
        assert_eq!(first.receipt().interval_count(), 4);
        assert_eq!(first.receipt().entry_interval_count(), 2);
        assert_eq!(first.receipt().operand_interval_count(), 2);
        assert_eq!(
            first.receipt().ranges(),
            source.live_range_stage().ranges().receipt().identity()
        );
        assert_eq!(
            first.receipt().legality(),
            source.legality().receipt().identity()
        );
        assert_eq!(
            first.receipt().identity(),
            omega_regalloc::fixed_precolored_interval_plan_identity(first.plan()),
        );

        let environment = source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let named = |name| environment.physical().model().view_named(name).unwrap().id;
        let rows = &first.plan().functions[0].intervals;
        assert_eq!(
            rows.iter()
                .map(|row| (
                    row.virtual_register.0,
                    row.block.0,
                    row.start.0,
                    row.end.0,
                    row.view
                ))
                .collect::<Vec<_>>(),
            vec![
                (0, 0, 0, 1, named(boolean_entry)),
                (1, 0, 0, 1, named(integer_entry)),
                (1, 1, 4, 5, named(integer_result)),
                (1, 2, 6, 7, named(integer_result)),
            ],
        );
        assert!(
            rows[..2]
                .iter()
                .all(|row| matches!(row.site, omega_regalloc::VirtualFixedConstraintSite::Entry))
        );
        assert!(rows[2..].iter().all(|row| matches!(
            row.site,
            omega_regalloc::VirtualFixedConstraintSite::Operand { .. }
        )));
    }
}
