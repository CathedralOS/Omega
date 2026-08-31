use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisSet, OptimizationPassIdentity, OptimizationRuleContract,
};
use omega_optimization_unit::reconstruct_psi_optimization_unit_seed;
use psi_core::FuelScheduleIdentity;
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::*;

const RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1";

fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(value).unwrap()
}

fn contract(identity: TotalScalarIdentityKind) -> OptimizationRuleContract {
    let domain = match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerAddZeroRight
        | TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => RULE_DOMAIN,
        TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount
        | TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => {
            b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1"
        }
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => {
            b"omega.psi-rule.live-obligation-free-wrapping-integer-multiply-zero-annihilation.v1"
        }
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerAddZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => {
            b"omega.psi-rule.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1"
        }
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
        | TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => {
            b"omega.psi-rule.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1"
        }
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => {
            b"omega.psi-rule.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1"
        }
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft
        | TotalScalarIdentityKind::IntegerBitwiseAndZeroRight
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft
        | TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => {
            b"omega.psi-rule.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1"
        }
    };
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(domain),
        OptimizationPassIdentity::from_canonical_bytes(b"test-gvn-pass"),
        1,
        AnalysisSet::new([
            AnalysisKind::ScalarConstants,
            AnalysisKind::UseDefinition,
            AnalysisKind::EffectSummaries,
        ]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .unwrap()
}

fn fixture(identity: TotalScalarIdentityKind) -> (PsiOptimizationUnit, TotalScalarIdentityRewrite) {
    let machine = id(1, MachineId::new);
    let block = id(2, BlockId::new);
    let input_value = id(3, ValueId::new);
    let neutral = id(4, ValueId::new);
    let result = id(5, ValueId::new);
    let literal_operation = id(6, OperationId::new);
    let arithmetic_operation = id(7, OperationId::new);
    let return_edge = id(8, EdgeId::new);
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let count_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let (law_value, law_operand_type, operation, replacement) = match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::WrappingIntegerAdd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::WrappingIntegerAddZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::WrappingIntegerAdd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::WrappingIntegerSubtract {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft => (
            IntegerValue::Unsigned(1),
            scalar_type,
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight => (
            IntegerValue::Unsigned(1),
            scalar_type,
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            neutral,
        ),
        TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            neutral,
        ),
        TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount => (
            IntegerValue::Signed(0),
            count_type,
            AbstractOperation::WrappingIntegerShiftLeft {
                psi_operation: arithmetic_operation,
                result,
                value_type: scalar_type,
                count_type,
                value: input_value,
                count: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount => (
            IntegerValue::Signed(0),
            count_type,
            AbstractOperation::WrappingIntegerShiftRight {
                psi_operation: arithmetic_operation,
                result,
                value_type: scalar_type,
                count_type,
                value: input_value,
                count: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::SaturatingIntegerAdd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::SaturatingIntegerAddZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::SaturatingIntegerAdd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::SaturatingIntegerSubtract {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft => (
            IntegerValue::Unsigned(1),
            scalar_type,
            AbstractOperation::SaturatingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight => (
            IntegerValue::Unsigned(1),
            scalar_type,
            AbstractOperation::SaturatingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::SaturatingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            neutral,
        ),
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::SaturatingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            neutral,
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft => (
            IntegerValue::Unsigned(u128::from(u32::MAX)),
            scalar_type,
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight => (
            IntegerValue::Unsigned(u128::from(u32::MAX)),
            scalar_type,
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::IntegerBitwiseOr {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::IntegerBitwiseOrZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::IntegerBitwiseOr {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::IntegerBitwiseXor {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            input_value,
        ),
        TotalScalarIdentityKind::IntegerBitwiseXorZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::IntegerBitwiseXor {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            neutral,
        ),
        TotalScalarIdentityKind::IntegerBitwiseAndZeroRight => (
            IntegerValue::Unsigned(0),
            scalar_type,
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            neutral,
        ),
        TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft => (
            IntegerValue::Unsigned(u128::from(u32::MAX)),
            scalar_type,
            AbstractOperation::IntegerBitwiseOr {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: neutral,
                right: input_value,
            },
            neutral,
        ),
        TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight => (
            IntegerValue::Unsigned(u128::from(u32::MAX)),
            scalar_type,
            AbstractOperation::IntegerBitwiseOr {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            neutral,
        ),
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([42; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![AbstractParameter {
                    value: input_value,
                    scalar_type: ScalarType::Integer(scalar_type),
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(scalar_type),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: literal_operation,
                        result: neutral,
                        scalar_type: ScalarType::Integer(law_operand_type),
                        value: law_value,
                    },
                    operation,
                    AbstractOperation::Return {
                        psi_edge: return_edge,
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(scalar_type),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let patch = TotalScalarIdentityRewrite {
        location: NodeLocation {
            machine,
            block,
            node: 1,
        },
        source_operation: arithmetic_operation,
        result,
        replacement,
        scalar_type,
        identity,
    };
    (unit, patch)
}

fn candidate(
    unit: &PsiOptimizationUnit,
    patch: TotalScalarIdentityRewrite,
    mutate_accounting: bool,
    fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
) -> PsiRewriteCandidate {
    candidate_with_contract(
        unit,
        patch,
        mutate_accounting,
        fact,
        contract(patch.identity),
    )
}

fn candidate_with_contract(
    unit: &PsiOptimizationUnit,
    patch: TotalScalarIdentityRewrite,
    mutate_accounting: bool,
    fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
    rule_contract: OptimizationRuleContract,
) -> PsiRewriteCandidate {
    let function = &unit.functions[0];
    let O::IntegerConstant {
        psi_operation: support,
        result: neutral,
        value,
        ..
    } = function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    let definition = scalar_value_definition(function, neutral).unwrap();
    let ValueDefinitionSite::Node { block, node } = definition.site else {
        unreachable!()
    };
    assert!(matches!(
        function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .unwrap()
        .nodes[usize::try_from(node).unwrap()]
        .operation,
        O::IntegerConstant { result, .. } if result == neutral
    ));
    let expected_fact = literal_scalar_constant_fact_identity(
        unit.identity,
        function.machine,
        definition,
        ScalarConstantValue::Integer(value),
        support,
    )
    .unwrap();
    let (blocks, mut provenance) =
        reconstruct_total_scalar_identity_accounting(function, patch).unwrap();
    if mutate_accounting {
        provenance[0].fuel[0].units += 1;
    }
    PsiRewriteCandidate::new_total_scalar_identity(
        unit.identity,
        rule_contract,
        blocks,
        provenance,
        fact.unwrap_or(expected_fact),
        -1,
        patch,
    )
    .unwrap()
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
            &candidate(
                &saturating_unit,
                saturating_patch,
                false,
                Some(foreign),
            ),
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
            &candidate(
                &saturating_unit,
                wrong_saturating_replacement,
                false,
                None,
            ),
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
            &candidate_with_contract(
                &bitwise_unit,
                bitwise_patch,
                false,
                None,
                wrapping_contract,
            ),
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
            &candidate(
                &absorbing_unit,
                wrong_absorbing_replacement,
                false,
                None,
            ),
        ),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    );
}
