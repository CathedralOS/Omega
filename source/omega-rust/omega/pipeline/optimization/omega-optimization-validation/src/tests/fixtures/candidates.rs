//! Candidate and redundant-region fixtures.

use super::*;

pub(crate) fn redundant_parameter_region_fixture() -> (
    PsiOptimizationUnit,
    PsiOptimizationUnit,
    RedundantBlockParameterRewrite,
    Vec<BlockId>,
) {
    use omega_abstract_operations::{AbstractSuccessor, ValueBinding};

    let machine = id(701, MachineId::new);
    let entry = id(702, BlockId::new);
    let merge = id(703, BlockId::new);
    let condition = id(704, ValueId::new);
    let shared = id(705, ValueId::new);
    let alternate = id(706, ValueId::new);
    let parameter = id(707, ValueId::new);
    let result = id(708, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let binding = || ValueBinding {
        parameter,
        argument: shared,
        scalar_type,
    };
    let input = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![
                    AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    },
                    AbstractParameter {
                        value: shared,
                        scalar_type,
                    },
                    AbstractParameter {
                        value: alternate,
                        scalar_type,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: merge,
                        parameters: vec![AbstractParameter {
                            value: parameter,
                            scalar_type,
                        }],
                        operation_offset: 1,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(709, EdgeId::new),
                            target: merge,
                            bindings: vec![binding()],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(710, EdgeId::new),
                            target: merge,
                            bindings: vec![binding()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation: id(711, OperationId::new),
                        obligation: id(713, psi_core::ObligationId::new),
                        result,
                        scalar_type: integer,
                        left: parameter,
                        right: alternate,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(712, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let patch = RedundantBlockParameterRewrite {
        machine,
        block: merge,
        position: 0,
        parameter,
        replacement: shared,
        scalar_type,
    };
    let affected = vec![entry, merge];
    let output = normalize_redundant_parameter_observation_input(&input, patch, &affected)
        .expect("exact structural normalization");
    (input, output, patch, affected)
}

pub(crate) fn integer_candidate(
    unit: &PsiOptimizationUnit,
    constant: IntegerValue,
) -> PsiRewriteCandidate {
    integer_candidate_with_facts(unit, constant, None, None)
}

pub(crate) fn integer_candidate_with_facts(
    unit: &PsiOptimizationUnit,
    constant: IntegerValue,
    supplied_left_fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
    supplied_obligation_fact: Option<omega_optimization_core::AcceptedObligationFactIdentity>,
) -> PsiRewriteCandidate {
    integer_candidate_with_facts_and_cost(
        unit,
        constant,
        supplied_left_fact,
        supplied_obligation_fact,
        -1,
    )
}

pub(crate) fn integer_candidate_with_facts_and_cost(
    unit: &PsiOptimizationUnit,
    constant: IntegerValue,
    supplied_left_fact: Option<omega_optimization_core::ScalarConstantFactIdentity>,
    supplied_obligation_fact: Option<omega_optimization_core::AcceptedObligationFactIdentity>,
    predicted_cost_delta: i64,
) -> PsiRewriteCandidate {
    let function = &unit.functions[0];
    let block = &function.blocks[0];
    let node = &block.nodes[2];
    let AbstractOperation::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = node.operation
    else {
        panic!("fixture contains exact add")
    };
    let location = NodeLocation {
        machine: function.machine,
        block: block.id,
        node: 2,
    };
    let contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"fold-exact-add"),
        OptimizationPassIdentity::from_canonical_bytes(b"constant-evaluation"),
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
    .unwrap();
    PsiRewriteCandidate::new_integer_evaluation(
        unit.identity,
        contract,
        vec![block.id],
        Vec::new(),
        vec![ProvenanceRewrite {
            input: PsiRealizationSite::Node(location),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        }],
        IntegerEvaluationWitness::ProofCertifiedBinary {
            left_fact: supplied_left_fact.unwrap_or_else(|| {
                literal_scalar_constant_fact_identity(
                    unit.identity,
                    function.machine,
                    scalar_value_definition(function, left).unwrap(),
                    ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
                    id(206, OperationId::new),
                )
                .unwrap()
            }),
            right_fact: literal_scalar_constant_fact_identity(
                unit.identity,
                function.machine,
                scalar_value_definition(function, right).unwrap(),
                ScalarConstantValue::Integer(IntegerValue::Unsigned(8)),
                id(207, OperationId::new),
            )
            .unwrap(),
            obligation_fact: supplied_obligation_fact
                .unwrap_or(unit.accepted_obligation_facts[0].identity),
        },
        predicted_cost_delta,
        IntegerConstantRewrite {
            location,
            source_operation: psi_operation,
            result,
            scalar_type,
            constant,
        },
    )
    .unwrap()
}
