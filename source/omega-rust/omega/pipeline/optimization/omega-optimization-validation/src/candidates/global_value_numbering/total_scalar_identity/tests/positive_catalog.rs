use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;

use super::super::validate_total_scalar_identity_candidate;
use super::fixtures::{candidate, fixture};

#[test]
fn all_twenty_six_total_rows_replay_without_proof_custody() {
    let identities = [
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
        TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
        TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
        TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
        TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
        TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
        TotalScalarIdentityKind::SaturatingIntegerAddZeroRight,
        TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
        TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
        TotalScalarIdentityKind::IntegerBitwiseOrZeroRight,
        TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft,
        TotalScalarIdentityKind::IntegerBitwiseXorZeroRight,
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
        TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
        TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
        TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
    ];
    for identity in identities {
        let (unit, patch) = fixture(identity);
        let candidate = candidate(&unit, patch, false, None);
        assert!(candidate.accepted_obligation_witness().is_none());
        assert_eq!(candidate.consumed_facts().len(), 1);
        let accepted = validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
        assert_eq!(
            accepted.unit().accepted_obligation_facts,
            unit.accepted_obligation_facts
        );
        let O::Return { value, .. } = accepted.unit().functions[0].blocks[0].nodes[1].operation
        else {
            panic!("identity receiver remains a return")
        };
        assert_eq!(value, patch.replacement);
    }
}
