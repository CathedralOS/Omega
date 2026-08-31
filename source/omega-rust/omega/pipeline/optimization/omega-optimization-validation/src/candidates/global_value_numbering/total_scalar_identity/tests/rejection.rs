use omega_optimization_unit::TotalScalarIdentityKind;
use psi_core::ValueId;

use crate::OptimizationUnitValidationError;

use super::super::validate_total_scalar_identity_candidate;
use super::fixtures::{candidate, candidate_with_contract, contract, fixture, id};

#[test]
fn independent_validator_rejects_kind_fact_and_accounting_corruption() {
    let (unit, patch) = fixture(TotalScalarIdentityKind::WrappingIntegerAddZeroLeft);
    let mut wrong_kind = patch;
    wrong_kind.identity = TotalScalarIdentityKind::WrappingIntegerAddZeroRight;
    assert_eq!(
        validate_total_scalar_identity_candidate(&unit, &candidate(&unit, wrong_kind, false, None)),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );

    let foreign = omega_optimization_core::ScalarConstantFactIdentity::from_canonical_bytes(
        b"foreign law literal",
    );
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &unit,
            &candidate(&unit, patch, false, Some(foreign))
        ),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    );
    assert_eq!(
        validate_total_scalar_identity_candidate(&unit, &candidate(&unit, patch, true, None)),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );

    let (shift_unit, shift_patch) =
        fixture(TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount);
    let arithmetic_contract = contract(TotalScalarIdentityKind::WrappingIntegerAddZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &shift_unit,
            &candidate_with_contract(&shift_unit, shift_patch, false, None, arithmetic_contract,),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let mut wrong_shift_kind = shift_patch;
    wrong_shift_kind.identity = TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &shift_unit,
            &candidate(&shift_unit, wrong_shift_kind, false, None),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let shift_contract = contract(TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &unit,
            &candidate_with_contract(&unit, patch, false, None, shift_contract),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let (annihilation_unit, annihilation_patch) =
        fixture(TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft);
    let shift_contract = contract(TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &annihilation_unit,
            &candidate_with_contract(
                &annihilation_unit,
                annihilation_patch,
                false,
                None,
                shift_contract,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let mut wrong_annihilation_side = annihilation_patch;
    wrong_annihilation_side.identity = TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &annihilation_unit,
            &candidate(&annihilation_unit, wrong_annihilation_side, false, None),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let mut wrong_annihilation_replacement = annihilation_patch;
    wrong_annihilation_replacement.replacement = id(3, ValueId::new);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &annihilation_unit,
            &candidate(
                &annihilation_unit,
                wrong_annihilation_replacement,
                false,
                None,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let annihilation_contract = contract(TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &unit,
            &candidate_with_contract(&unit, patch, false, None, annihilation_contract),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let (saturating_unit, saturating_patch) =
        fixture(TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft);
    let wrapping_contract = contract(TotalScalarIdentityKind::WrappingIntegerAddZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_unit,
            &candidate_with_contract(
                &saturating_unit,
                saturating_patch,
                false,
                None,
                wrapping_contract,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let mut wrong_saturating_side = saturating_patch;
    wrong_saturating_side.identity = TotalScalarIdentityKind::SaturatingIntegerAddZeroRight;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_unit,
            &candidate(&saturating_unit, wrong_saturating_side, false, None),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_unit,
            &candidate(&saturating_unit, saturating_patch, false, Some(foreign),),
        ),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch),
    );
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_unit,
            &candidate(&saturating_unit, saturating_patch, true, None),
        ),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch),
    );

    let mut wrong_saturating_replacement = saturating_patch;
    wrong_saturating_replacement.replacement = id(4, ValueId::new);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_unit,
            &candidate(&saturating_unit, wrong_saturating_replacement, false, None,),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let saturating_contract = contract(TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &unit,
            &candidate_with_contract(&unit, patch, false, None, saturating_contract),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let (saturating_annihilation_unit, saturating_annihilation_patch) =
        fixture(TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_annihilation_unit,
            &candidate_with_contract(
                &saturating_annihilation_unit,
                saturating_annihilation_patch,
                false,
                None,
                saturating_contract,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let mut wrong_saturating_annihilation_side = saturating_annihilation_patch;
    wrong_saturating_annihilation_side.identity =
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_annihilation_unit,
            &candidate(
                &saturating_annihilation_unit,
                wrong_saturating_annihilation_side,
                false,
                None,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let wrapping_annihilation_contract =
        contract(TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &saturating_annihilation_unit,
            &candidate_with_contract(
                &saturating_annihilation_unit,
                saturating_annihilation_patch,
                false,
                None,
                wrapping_annihilation_contract,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let (bitwise_unit, bitwise_patch) =
        fixture(TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft);
    let saturating_annihilation_contract =
        contract(TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &bitwise_unit,
            &candidate_with_contract(
                &bitwise_unit,
                bitwise_patch,
                false,
                None,
                saturating_annihilation_contract,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let mut wrong_bitwise_side = bitwise_patch;
    wrong_bitwise_side.identity = TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &bitwise_unit,
            &candidate(&bitwise_unit, wrong_bitwise_side, false, None),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let mut wrong_bitwise_operation = bitwise_patch;
    wrong_bitwise_operation.identity = TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &bitwise_unit,
            &candidate(&bitwise_unit, wrong_bitwise_operation, false, None),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let wrapping_contract = contract(TotalScalarIdentityKind::WrappingIntegerAddZeroLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &bitwise_unit,
            &candidate_with_contract(&bitwise_unit, bitwise_patch, false, None, wrapping_contract,),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let (absorbing_unit, absorbing_patch) =
        fixture(TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft);
    let neutral_contract = contract(TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &absorbing_unit,
            &candidate_with_contract(
                &absorbing_unit,
                absorbing_patch,
                false,
                None,
                neutral_contract,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    );

    let mut wrong_absorbing_side = absorbing_patch;
    wrong_absorbing_side.identity = TotalScalarIdentityKind::IntegerBitwiseAndZeroRight;
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &absorbing_unit,
            &candidate(&absorbing_unit, wrong_absorbing_side, false, None),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );

    let mut wrong_absorbing_replacement = absorbing_patch;
    wrong_absorbing_replacement.replacement = id(3, ValueId::new);
    assert_eq!(
        validate_total_scalar_identity_candidate(
            &absorbing_unit,
            &candidate(&absorbing_unit, wrong_absorbing_replacement, false, None,),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );
}
