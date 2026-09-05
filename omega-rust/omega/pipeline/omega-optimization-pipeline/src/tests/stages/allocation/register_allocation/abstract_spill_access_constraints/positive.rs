use crate::tests::*;

use super::{
    super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle},
    fixture::{EXACT_USAGE, build, constrain, exact_budget},
};

#[test]
fn both_recursive_paths_gain_exact_block_local_dependencies_on_both_targets() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for constructor in [
            reload_bundle as fn(NativeTarget) -> Bundle,
            original_bundle as fn(NativeTarget) -> Bundle,
        ] {
            let source = build(constructor, target);
            let first = constrain(&source, exact_budget()).unwrap();
            let second = constrain(&source, exact_budget()).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.receipt().function_count(), 1);
            assert_eq!(first.receipt().placement_count(), 6);
            assert_eq!(first.receipt().dependency_count(), 12);
            assert_eq!(first.receipt().stored_value_dependency_count(), 3);
            assert_eq!(first.receipt().declared_barrier_count(), 2);
            assert_eq!(first.receipt().overlapping_slice_dependency_count(), 7);
            assert_eq!(first.receipt().max_spill_area_bytes(), 16);
            assert_eq!(first.receipt().usage(), EXACT_USAGE);
            assert_eq!(
                first.receipt().abstract_spill_memory_effects(),
                source.effects.receipt().identity(),
            );
            assert_eq!(
                first.receipt().identity(),
                omega_selected_instructions_to_register_homes::abstract_spill_access_constraint_plan_identity(first.plan()),
            );
            let function = &first.plan().functions[0];
            assert_eq!(
                function
                    .placements
                    .iter()
                    .map(|row| (
                        row.pseudo.ordinal,
                        row.block_ordinal,
                        row.point.0,
                        row.kind,
                        row.spill_area_offset,
                    ))
                    .collect::<Vec<_>>(),
                vec![
                    (0, 0, 9, omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Write, 0),
                    (1, 1, 12, omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Write, 8),
                    (2, 2, 12, omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Read, 0),
                    (3, 3, 14, omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Write, 0),
                    (4, 4, 14, omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Read, 8),
                    (5, 5, 16, omega_selected_instructions_to_register_homes::AbstractSpillAccessKind::Read, 0),
                ],
            );
            assert_edges(function);
        }
    }
}

fn assert_edges(
    function: &omega_selected_instructions_to_register_homes::FunctionAbstractSpillAccessConstraints,
) {
    for (before, after) in [(0, 2), (1, 4), (3, 5)] {
        assert!(has(function, before, after, |reason| {
            matches!(
            reason,
            omega_selected_instructions_to_register_homes::AbstractSpillAccessDependencyReason::StoredValue { .. }
        )
        }));
    }
    for (before, after) in [(1, 2), (3, 4)] {
        assert!(has(function, before, after, |reason| {
            matches!(
            reason,
            omega_selected_instructions_to_register_homes::AbstractSpillAccessDependencyReason::DeclaredBeforeReload
        )
        }));
    }
    for (before, after, offset) in [
        (0, 2, 0),
        (0, 3, 0),
        (0, 5, 0),
        (1, 4, 8),
        (2, 3, 0),
        (2, 5, 0),
        (3, 5, 0),
    ] {
        assert!(has(function, before, after, |reason| matches!(
            reason,
            omega_selected_instructions_to_register_homes::AbstractSpillAccessDependencyReason::OverlappingAbstractSlice {
                spill_area_offset,
                size_bytes: 8,
            } if *spill_area_offset == offset
        )));
    }
}

fn has(
    function: &omega_selected_instructions_to_register_homes::FunctionAbstractSpillAccessConstraints,
    before: u32,
    after: u32,
    reason: impl Fn(
        &omega_selected_instructions_to_register_homes::AbstractSpillAccessDependencyReason,
    ) -> bool,
) -> bool {
    function.dependencies.iter().any(|edge| {
        edge.before.ordinal == before && edge.after.ordinal == after && reason(&edge.reason)
    })
}
