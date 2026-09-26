use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationValidatorIdentity;
use optimization_unit::TotalScalarIdentityKind;

use super::super::validate_total_scalar_identity_candidate;
use super::fixtures::{candidate, fixture};

fn expected_validator(identity: TotalScalarIdentityKind) -> OptimizationValidatorIdentity {
    let domain = match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerAddZeroRight
        | TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => {
            b"omega.validator.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1".as_slice()
        }
        TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
        | TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => {
            b"omega.validator.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1".as_slice()
        }
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => {
            b"omega.validator.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1".as_slice()
        }
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerAddZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => {
            b"omega.validator.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1".as_slice()
        }
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => {
            b"omega.validator.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1".as_slice()
        }
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => {
            b"omega.validator.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1".as_slice()
        }
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => {
            b"omega.validator.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1".as_slice()
        }
    };
    OptimizationValidatorIdentity::from_canonical_bytes(domain)
}

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
        assert_eq!(candidate.predicted_cost_delta(), -1);
        let accepted = validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.validator(), expected_validator(identity));
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
