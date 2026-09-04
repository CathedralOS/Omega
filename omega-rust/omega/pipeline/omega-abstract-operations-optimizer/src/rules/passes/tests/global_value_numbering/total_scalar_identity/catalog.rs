//! Registry placement and opt-in custody for the seven total scalar identity rows.

use super::*;

#[test]
fn total_identity_rules_are_disabled_by_default_and_cataloged_once() {
    assert!(
        built_in_psi_registry(&OptimizationSelections::default())
            .unwrap()
            .is_empty()
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let identities = registry
        .contracts()
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    let contract = WrappingNeutralArithmeticIdentityRule::contract();
    assert_eq!(
        contract.pass(),
        OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.global-value-numbering.v14",
        )
    );
    let expected = contract.identity();
    assert_eq!(identities.get(9), Some(&expected));
    assert_eq!(
        identities
            .into_iter()
            .filter(|identity| *identity == expected)
            .count(),
        1
    );
    let shift = WrappingShiftZeroCountIdentityRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(10).map(|row| row.identity()),
        Some(shift)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == shift)
            .count(),
        1
    );
    let annihilation = WrappingMultiplyZeroAnnihilationRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(11).map(|row| row.identity()),
        Some(annihilation)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == annihilation)
            .count(),
        1
    );
    let saturating = SaturatingNeutralArithmeticIdentityRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(12).map(|row| row.identity()),
        Some(saturating)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == saturating)
            .count(),
        1
    );
    let saturating_annihilation = SaturatingMultiplyZeroAnnihilationRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(13).map(|row| row.identity()),
        Some(saturating_annihilation)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == saturating_annihilation)
            .count(),
        1
    );
    let bitwise_neutral = BitwiseNeutralLiteralIdentityRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(14).map(|row| row.identity()),
        Some(bitwise_neutral)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == bitwise_neutral)
            .count(),
        1
    );
    let bitwise_absorbing = BitwiseAbsorbingLiteralIdentityRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(15).map(|row| row.identity()),
        Some(bitwise_absorbing)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == bitwise_absorbing)
            .count(),
        1
    );
}
