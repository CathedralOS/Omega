use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperation as O, AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use optimization_unit::{
    NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate, ScalarConstantValue,
    TotalScalarIdentityKind, TotalScalarIdentityRewrite, ValueDefinitionSite,
    literal_scalar_constant_fact_identity, reconstruct_psi_optimization_unit_seed,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::candidates::{reconstruct_total_scalar_identity_accounting, scalar_value_definition};

const RULE_DOMAIN: &[u8] =
    b"omega.psi-rule.live-obligation-free-wrapping-integer-neutral-arithmetic-identity-elimination.v1";

pub(super) fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(value).unwrap()
}

pub(super) fn contract(identity: TotalScalarIdentityKind) -> OptimizationRuleContract {
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

pub(super) fn fixture(
    identity: TotalScalarIdentityKind,
) -> (PsiOptimizationUnit, TotalScalarIdentityRewrite) {
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

pub(super) fn candidate(
    unit: &PsiOptimizationUnit,
    patch: TotalScalarIdentityRewrite,
    mutate_accounting: bool,
    fact: Option<optimization_core::ScalarConstantFactIdentity>,
) -> PsiRewriteCandidate {
    candidate_with_contract(
        unit,
        patch,
        mutate_accounting,
        fact,
        contract(patch.identity),
    )
}

pub(super) fn candidate_with_contract(
    unit: &PsiOptimizationUnit,
    patch: TotalScalarIdentityRewrite,
    mutate_accounting: bool,
    fact: Option<optimization_core::ScalarConstantFactIdentity>,
    rule_contract: OptimizationRuleContract,
) -> PsiRewriteCandidate {
    candidate_with_contract_and_cost(unit, patch, mutate_accounting, fact, rule_contract, -1)
}

pub(super) fn candidate_with_contract_and_cost(
    unit: &PsiOptimizationUnit,
    patch: TotalScalarIdentityRewrite,
    mutate_accounting: bool,
    fact: Option<optimization_core::ScalarConstantFactIdentity>,
    rule_contract: OptimizationRuleContract,
    predicted_cost_delta: i64,
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
        predicted_cost_delta,
        patch,
    )
    .unwrap()
}
