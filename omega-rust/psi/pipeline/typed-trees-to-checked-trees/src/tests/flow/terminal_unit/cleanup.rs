//! Fixtures shared by the unit cleanup tests.

mod affine_residuals;
mod fixed_array_construction;
mod nominal_cleanup_requirements;
mod results;
mod unit_and_scalar_cleanup;

use crate::tests::flow::terminal_unit::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, Multiplicity, machine_named,
};

fn fixed_cleanup_path(indexes: &[u64]) -> Vec<CheckedUnitStructuralPathSegment> {
    indexes
        .iter()
        .copied()
        .map(CheckedUnitStructuralPathSegment::FixedIndex)
        .collect()
}

fn assert_token_cleanup_partition(
    checked: &checked_trees::CheckedTrees,
    machine: &str,
    moved_paths: &[Vec<CheckedUnitStructuralPathSegment>],
    residuals: &[(Vec<CheckedUnitStructuralPathSegment>, String)],
) {
    let plan = checked
        .facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .for_machine(machine_named(checked, machine))
        .unwrap_or_else(|| panic!("missing partial cleanup for {machine}"));
    assert!(plan.machine.entry_claims.is_empty(), "{machine}");
    let [parameter] = plan.machine.structural_parameters.as_slice() else {
        panic!("{machine} requires exactly one structural root")
    };
    assert_eq!(
        parameter.access,
        checked_trees::CheckedStructuralAccess::Owned,
        "{machine}"
    );
    assert_eq!(parameter.multiplicity, Multiplicity::Affine, "{machine}");
    assert!(parameter.qualifications.is_empty(), "{machine}");
    assert_eq!(
        plan.machine.operations.len(),
        moved_paths.len() + 1,
        "{machine}"
    );
    for (ordinal, (operation, expected_path)) in
        plan.machine.operations.iter().zip(moved_paths).enumerate()
    {
        let CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            structural_arguments,
            scalar_arguments,
            claim_transfers,
            ..
        } = operation
        else {
            panic!("{machine} must preserve each authored Unit call")
        };
        assert_eq!(
            usize::try_from(coordinate.statement_index),
            Ok(ordinal),
            "{machine}"
        );
        assert_eq!(coordinate.call_ordinal, 0, "{machine}");
        assert!(
            scalar_arguments.is_empty() && claim_transfers.is_empty(),
            "{machine}"
        );
        let [argument] = structural_arguments.as_slice() else {
            panic!("{machine} must move one projection per call")
        };
        assert_eq!(argument.source_parameter_index(), Some(0), "{machine}");
        assert_eq!(
            argument.access,
            checked_trees::CheckedStructuralAccess::Owned,
            "{machine}"
        );
        assert_eq!(
            &argument.path, expected_path,
            "{machine}: authored move order"
        );
        assert_eq!(argument.type_identity, "named(name(Token))", "{machine}");
    }
    let Some(CheckedUnitEffectOperationPlan::Complete {
        statement_index,
        trivial_affine_local_discard_ordinals,
        trivial_affine_discards,
    }) = plan.machine.operations.last()
    else {
        panic!("{machine} requires the checked Unit return")
    };
    assert_eq!(
        usize::try_from(*statement_index),
        Ok(moved_paths.len()),
        "{machine}"
    );
    assert!(
        trivial_affine_local_discard_ordinals.is_empty() && trivial_affine_discards.is_empty(),
        "{machine}: no whole-root cleanup after projected moves"
    );
    let expected = residuals
        .iter()
        .map(
            |(path, type_identity)| checked_trees::CheckedUnitPartialAffineDiscardPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: 0,
                },
                path: path.clone(),
                type_identity: type_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(
        plan.residual_affine_discards, expected,
        "{machine}: exact maximal residual paths, types, and cleanup order"
    );
}
