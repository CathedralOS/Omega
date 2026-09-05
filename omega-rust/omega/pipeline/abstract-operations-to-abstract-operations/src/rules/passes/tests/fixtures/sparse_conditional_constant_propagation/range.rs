//! Proof-backed integer-range comparison fixtures.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProofRangeKind {
    Zero,
    ZeroToThree,
    Nonzero,
}

struct ProofRangeOperation {
    input_type: IntegerType,
    operation: AbstractOperation,
    goal: terminal_semantics::CanonicalScalarGoal,
}

fn proof_range_operation(
    kind: ProofRangeKind,
    scalar_type: IntegerType,
    input: ValueId,
    ranged: ValueId,
    result: ValueId,
    operation: OperationId,
    obligation: ObligationId,
) -> ProofRangeOperation {
    match kind {
        ProofRangeKind::Zero | ProofRangeKind::ZeroToThree => {
            let input_type = IntegerType::new(
                IntegerSign::Unsigned,
                if kind == ProofRangeKind::Zero { 1 } else { 4 },
            )
            .unwrap();
            ProofRangeOperation {
                input_type,
                operation: AbstractOperation::ExactIntegerShiftRight {
                    psi_operation: operation,
                    obligation,
                    result,
                    value_type: input_type,
                    count_type: scalar_type,
                    value: input,
                    count: ranged,
                },
                goal: terminal_semantics::CanonicalScalarGoal::ExactShiftCount {
                    value_type: input_type,
                    count_type: scalar_type,
                    count: semantic_vocabulary::ScalarTerm::value(
                        ranged,
                        ScalarType::Integer(scalar_type),
                    ),
                },
            }
        }
        ProofRangeKind::Nonzero => ProofRangeOperation {
            input_type: scalar_type,
            operation: AbstractOperation::WrappingIntegerDivide {
                psi_operation: operation,
                obligation,
                result,
                scalar_type,
                left: input,
                right: ranged,
            },
            goal: terminal_semantics::CanonicalScalarGoal::NonzeroDivisor {
                integer_type: scalar_type,
                divisor: semantic_vocabulary::ScalarTerm::value(
                    ranged,
                    ScalarType::Integer(scalar_type),
                ),
            },
        },
    }
}

fn attach_proof_rows(
    unit: PsiOptimizationUnit,
    machine: MachineId,
    rows: Vec<(
        OperationId,
        ObligationId,
        terminal_semantics::CanonicalScalarGoal,
    )>,
) -> PsiOptimizationUnit {
    let proof_bundle_fingerprint = [30; 32];
    let mut facts = Vec::with_capacity(rows.len());
    let mut questions = Vec::with_capacity(rows.len());
    for (operation, obligation, goal) in rows {
        let proposition = goal.kernel_proposition().unwrap();
        let proposition = terminal_codec::canonical_proposition_order_key(&proposition).unwrap();
        facts.push(AcceptedObligationFact::new(
            unit.psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            proposition.clone(),
        ));
        questions.push(ProofQuestion::new(
            unit.psi,
            proof_bundle_fingerprint,
            ProofQuestionOwner::Operation { machine, operation },
            obligation,
            ProofQuestionClass::Derivable,
            proposition,
            Vec::new(),
            Vec::new(),
            true,
        ));
    }
    let unit = attach_accepted_obligation_facts(unit, facts).unwrap();
    attach_proof_questions(unit, questions).unwrap()
}

fn range_constant_operation(
    kind: IntegerRangeComparisonKind,
    operation: OperationId,
    result: ValueId,
    ranged: ValueId,
    constant: ValueId,
) -> AbstractOperation {
    let (left, right) = match kind {
        IntegerRangeComparisonKind::RangeEqualConstant
        | IntegerRangeComparisonKind::RangeLessThanConstant
        | IntegerRangeComparisonKind::RangeLessOrEqualConstant => (ranged, constant),
        IntegerRangeComparisonKind::ConstantEqualRange
        | IntegerRangeComparisonKind::ConstantLessThanRange
        | IntegerRangeComparisonKind::ConstantLessOrEqualRange => (constant, ranged),
    };
    match kind {
        IntegerRangeComparisonKind::RangeEqualConstant
        | IntegerRangeComparisonKind::ConstantEqualRange => AbstractOperation::IntegerEqual {
            psi_operation: operation,
            result,
            left,
            right,
        },
        IntegerRangeComparisonKind::RangeLessThanConstant
        | IntegerRangeComparisonKind::ConstantLessThanRange => AbstractOperation::IntegerLessThan {
            psi_operation: operation,
            result,
            left,
            right,
        },
        IntegerRangeComparisonKind::RangeLessOrEqualConstant
        | IntegerRangeComparisonKind::ConstantLessOrEqualRange => {
            AbstractOperation::IntegerLessOrEqual {
                psi_operation: operation,
                result,
                left,
                right,
            }
        }
    }
}

pub(crate) fn range_constant_comparison_unit(
    kind: IntegerRangeComparisonKind,
    scalar_type: IntegerType,
    range_kind: ProofRangeKind,
    constant_value: IntegerValue,
) -> PsiOptimizationUnit {
    let machine = id(401, MachineId::new);
    let block = id(402, BlockId::new);
    let proof_input = id(403, ValueId::new);
    let ranged = id(404, ValueId::new);
    let proof_result = id(405, ValueId::new);
    let constant = id(406, ValueId::new);
    let result = id(407, ValueId::new);
    let proof_operation_id = id(408, OperationId::new);
    let comparison_operation = id(409, OperationId::new);
    let obligation = id(410, ObligationId::new);
    let proof = proof_range_operation(
        range_kind,
        scalar_type,
        proof_input,
        ranged,
        proof_result,
        proof_operation_id,
        obligation,
    );
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([40; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![
                    AbstractParameter {
                        value: proof_input,
                        scalar_type: ScalarType::Integer(proof.input_type),
                    },
                    AbstractParameter {
                        value: ranged,
                        scalar_type: ScalarType::Integer(scalar_type),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    proof.operation,
                    AbstractOperation::IntegerConstant {
                        psi_operation: id(411, OperationId::new),
                        result: constant,
                        scalar_type: ScalarType::Integer(scalar_type),
                        value: constant_value,
                    },
                    range_constant_operation(kind, comparison_operation, result, ranged, constant),
                    AbstractOperation::Return {
                        psi_edge: id(412, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    attach_proof_rows(
        unit,
        machine,
        vec![(proof_operation_id, obligation, proof.goal)],
    )
}

fn range_pair_operation(
    kind: IntegerRangePairComparisonKind,
    operation: OperationId,
    result: ValueId,
    left: ValueId,
    right: ValueId,
) -> AbstractOperation {
    match kind {
        IntegerRangePairComparisonKind::Equal => AbstractOperation::IntegerEqual {
            psi_operation: operation,
            result,
            left,
            right,
        },
        IntegerRangePairComparisonKind::LessThan => AbstractOperation::IntegerLessThan {
            psi_operation: operation,
            result,
            left,
            right,
        },
        IntegerRangePairComparisonKind::LessOrEqual => AbstractOperation::IntegerLessOrEqual {
            psi_operation: operation,
            result,
            left,
            right,
        },
    }
}

pub(crate) fn range_pair_comparison_unit(
    kind: IntegerRangePairComparisonKind,
    scalar_type: IntegerType,
    left_kind: ProofRangeKind,
    right_kind: ProofRangeKind,
    same_value: bool,
) -> PsiOptimizationUnit {
    if same_value {
        assert_eq!(left_kind, right_kind);
    }
    let machine = id(421, MachineId::new);
    let block = id(422, BlockId::new);
    let left_input = id(423, ValueId::new);
    let left = id(424, ValueId::new);
    let left_result = id(425, ValueId::new);
    let right_input = id(426, ValueId::new);
    let right = id(427, ValueId::new);
    let right_result = id(428, ValueId::new);
    let result = id(429, ValueId::new);
    let left_operation = id(430, OperationId::new);
    let right_operation = id(431, OperationId::new);
    let comparison_operation = id(432, OperationId::new);
    let left_obligation = id(433, ObligationId::new);
    let right_obligation = id(434, ObligationId::new);
    let left_proof = proof_range_operation(
        left_kind,
        scalar_type,
        left_input,
        left,
        left_result,
        left_operation,
        left_obligation,
    );
    let right_proof = proof_range_operation(
        right_kind,
        scalar_type,
        right_input,
        right,
        right_result,
        right_operation,
        right_obligation,
    );
    let compared_right = if same_value { left } else { right };
    let mut parameters = vec![
        AbstractParameter {
            value: left_input,
            scalar_type: ScalarType::Integer(left_proof.input_type),
        },
        AbstractParameter {
            value: left,
            scalar_type: ScalarType::Integer(scalar_type),
        },
    ];
    let mut operations = vec![left_proof.operation];
    let mut proof_rows = vec![(left_operation, left_obligation, left_proof.goal)];
    if !same_value {
        parameters.extend([
            AbstractParameter {
                value: right_input,
                scalar_type: ScalarType::Integer(right_proof.input_type),
            },
            AbstractParameter {
                value: right,
                scalar_type: ScalarType::Integer(scalar_type),
            },
        ]);
        operations.push(right_proof.operation);
        proof_rows.push((right_operation, right_obligation, right_proof.goal));
    }
    operations.extend([
        range_pair_operation(kind, comparison_operation, result, left, compared_right),
        AbstractOperation::Return {
            psi_edge: id(435, EdgeId::new),
            result,
            value: result,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        },
    ]);
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([41; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters,
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations,
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    attach_proof_rows(unit, machine, proof_rows)
}

pub(crate) fn proof_range_pair_comparison_unit() -> PsiOptimizationUnit {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    range_pair_comparison_unit(
        IntegerRangePairComparisonKind::LessOrEqual,
        scalar_type,
        ProofRangeKind::Zero,
        ProofRangeKind::ZeroToThree,
        false,
    )
}
