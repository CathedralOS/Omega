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

fn contract() -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(RULE_DOMAIN),
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
    let (neutral_value, operation, replacement) = match identity {
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft => (
            IntegerValue::Unsigned(0),
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
            AbstractOperation::WrappingIntegerMultiply {
                psi_operation: arithmetic_operation,
                result,
                scalar_type,
                left: input_value,
                right: neutral,
            },
            input_value,
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
                        scalar_type: ScalarType::Integer(scalar_type),
                        value: neutral_value,
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
        contract(),
        blocks,
        provenance,
        fact.unwrap_or(expected_fact),
        -1,
        patch,
    )
    .unwrap()
}

#[test]
fn all_five_total_rows_replay_without_proof_custody() {
    let identities = [
        TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
        TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
        TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
        TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
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
        b"foreign neutral literal",
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
}
