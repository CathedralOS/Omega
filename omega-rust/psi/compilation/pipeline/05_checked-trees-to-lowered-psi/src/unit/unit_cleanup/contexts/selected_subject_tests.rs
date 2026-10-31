//! The original ordered search remains the oracle for raw subject selection.

use super::SelectedCleanupSubjects;
use checked_trees::{
    CheckedNormalCleanupAction, CheckedTrees, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use symbols::SymbolHandle;

fn cleanup(machine: SymbolHandle, marker: u64) -> CheckedUnitNominalAffineCleanupPlan {
    CheckedUnitNominalAffineCleanupPlan {
        consumed_claims: Vec::new(),
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
        path: Vec::new(),
        type_identity: Default::default(),
        cleanup_machine: machine,
        cleanup_state: SymbolHandle::from_parts(42, 1),
        cleanup_contract_report_fingerprint: marker,
        case_conditions: Default::default(),
    }
}

#[test]
fn first_subjects_match_original_search_without_visiting_an_unused_tail() {
    let first = SymbolHandle::from_parts(7, 1);
    let next_generation = SymbolHandle::from_parts(7, 2);
    let other = SymbolHandle::from_parts(8, 1);
    let absent = SymbolHandle::from_parts(9, 1);
    let mut checked = CheckedTrees::default();
    checked
        .facts
        .values
        .scalar_computations
        .temporary_cleanup_actions
        .insert_many(
            [
                cleanup(first, 11),
                cleanup(next_generation, 22),
                cleanup(first, 33),
                cleanup(other, 44),
            ]
            .into_iter()
            .map(CheckedNormalCleanupAction::InvokeNominal),
        );
    let mut subjects = SelectedCleanupSubjects::new(&checked);
    for source in [other, next_generation, first, absent, first] {
        let expected =
            super::super::selected_cleanups(&checked).find(|row| row.cleanup_machine == source);
        assert_eq!(
            subjects.first(source).map(std::ptr::from_ref),
            expected.map(std::ptr::from_ref),
        );
    }
    assert_eq!(
        subjects
            .first(first)
            .unwrap()
            .cleanup_contract_report_fingerprint,
        11
    );
    assert_eq!(
        subjects
            .first(next_generation)
            .unwrap()
            .cleanup_contract_report_fingerprint,
        22
    );
    assert!(
        SelectedCleanupSubjects::new(&CheckedTrees::default())
            .first(first)
            .is_none()
    );

    // An early match must leave an invalid later row unobserved. Reusing that
    // first match must not advance either; a missing lookup reaches the same
    // invalid tail as the original search.
    let row = super::super::selected_cleanups(&checked).next().unwrap();
    let visits = Cell::new(0);
    let rows = std::iter::once(row)
        .inspect(|_| visits.set(visits.get() + 1))
        .chain(std::iter::once_with(|| panic!("unvisited cleanup tail")));
    let mut prefix = SelectedCleanupSubjects::from_rows(&checked, rows);
    assert!(std::ptr::eq(prefix.first(first).unwrap(), row));
    assert!(std::ptr::eq(prefix.first(first).unwrap(), row));
    assert_eq!(visits.get(), 1);
    assert!(catch_unwind(AssertUnwindSafe(|| prefix.first(absent))).is_err());
    assert!(
        catch_unwind(|| {
            std::iter::once(row)
                .chain(std::iter::once_with(|| panic!("unvisited cleanup tail")))
                .find(|candidate| candidate.cleanup_machine == absent)
        })
        .is_err()
    );
}
