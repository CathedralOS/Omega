use std::sync::Arc;

use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, Optimization, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    OptimizationSelections,
};
use omega_optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceRewrite,
    PsiOptimizationUnit, PsiRewriteCandidate,
};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{IntegerValue, MachineId, OperationId, ValueId};

use crate::{
    AnalysisProduct, OrderedRuleRegistry, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    RuleRegistryError, ScalarConstant, ScalarConstantAnalysis,
};

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v1";

fn exact_integer_contract(rule_name: &[u8]) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
    .expect("built-in rule has nonzero version")
}

macro_rules! exact_integer_rule {
    ($name:ident, $rule_name:literal, $kind:expr) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                exact_integer_contract($rule_name)
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose_exact_integer_constants(unit, analyses, Self::contract(), $kind)
            }
        }
    };
}

exact_integer_rule!(
    ExactIntegerAddConstantsRule,
    b"omega.psi-rule.exact-integer-add-constants.v1",
    ExactBinaryKind::Add
);
exact_integer_rule!(
    ExactIntegerSubtractConstantsRule,
    b"omega.psi-rule.exact-integer-subtract-constants.v1",
    ExactBinaryKind::Subtract
);
exact_integer_rule!(
    ExactIntegerMultiplyConstantsRule,
    b"omega.psi-rule.exact-integer-multiply-constants.v1",
    ExactBinaryKind::Multiply
);

fn propose_exact_integer_constants(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: ExactBinaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(shape) = exact_binary_shape(&node.operation) else {
                    continue;
                };
                if shape.kind != kind {
                    continue;
                }
                let Some((left_value, left_support)) =
                    integer_constant(constants, function.machine, shape.left)
                else {
                    continue;
                };
                let Some((right_value, right_support)) =
                    integer_constant(constants, function.machine, shape.right)
                else {
                    continue;
                };
                let Some(constant) = shape.evaluate(left_value, right_value) else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            output: location,
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        IntegerEvaluationWitness {
                            left_support,
                            right_support,
                        },
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: shape.source,
                            result: shape.result,
                            scalar_type: shape.scalar_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

struct ExactBinaryShape {
    source: OperationId,
    result: ValueId,
    scalar_type: psi_core::IntegerType,
    left: ValueId,
    right: ValueId,
    kind: ExactBinaryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactBinaryKind {
    Add,
    Subtract,
    Multiply,
}

impl ExactBinaryShape {
    fn evaluate(&self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        match self.kind {
            ExactBinaryKind::Add => self.scalar_type.exact_add(left, right),
            ExactBinaryKind::Subtract => self.scalar_type.exact_sub(left, right),
            ExactBinaryKind::Multiply => self.scalar_type.exact_mul(left, right),
        }
    }
}

fn exact_binary_shape(operation: &O) -> Option<ExactBinaryShape> {
    let (source, result, scalar_type, left, right, kind) = match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ExactBinaryKind::Add,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ExactBinaryKind::Subtract,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            ExactBinaryKind::Multiply,
        ),
        _ => return None,
    };
    Some(ExactBinaryShape {
        source,
        result,
        scalar_type,
        left,
        right,
        kind,
    })
}

fn integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, OperationId)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Integer(value) => Some((value, fact.support)),
                ScalarConstant::Boolean(_) => None,
            })
    })
}

pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    if let Some(unsupported) = selections
        .as_slice()
        .iter()
        .find(|optimization| **optimization != Optimization::SparseConditionalConstantPropagation)
    {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    let mut rules = Vec::<Arc<dyn PsiOptimizationRule>>::new();
    if selections.contains(Optimization::SparseConditionalConstantPropagation) {
        rules.push(Arc::new(ExactIntegerAddConstantsRule));
        rules.push(Arc::new(ExactIntegerSubtractConstantsRule));
        rules.push(Arc::new(ExactIntegerMultiplyConstantsRule));
    }
    OrderedRuleRegistry::new(rules)
}

#[cfg(test)]
pub(crate) mod tests {
    use omega_optimization_unit::reconstruct_psi_optimization_unit_seed;
    use omega_optimization_validation::validate_integer_evaluation_candidate;
    use omega_terminal_abstract_operations::{
        TerminalAbstractBlockEntry, TerminalAbstractFunction, TerminalAbstractFunctionResult,
        TerminalAbstractOperation, TerminalAbstractOperationPlan, TerminalAbstractResult,
    };
    use psi_core::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
        OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use super::*;
    use crate::compute_analysis;

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    pub(crate) fn exact_add_unit() -> PsiOptimizationUnit {
        exact_chain_unit(false)
    }

    pub(crate) fn dependent_exact_chain_unit() -> PsiOptimizationUnit {
        exact_chain_unit(true)
    }

    fn exact_chain_unit(include_multiply: bool) -> PsiOptimizationUnit {
        let machine = id(301, MachineId::new);
        let block = id(302, BlockId::new);
        let left = id(303, ValueId::new);
        let right = id(304, ValueId::new);
        let sum = id(305, ValueId::new);
        let product = id(311, ValueId::new);
        let result = if include_multiply { product } else { sum };
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let mut operations = vec![
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: id(306, OperationId::new),
                result: left,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(7),
            },
            TerminalAbstractOperation::IntegerConstant {
                psi_operation: id(307, OperationId::new),
                result: right,
                scalar_type: ScalarType::Integer(integer),
                value: IntegerValue::Unsigned(8),
            },
            TerminalAbstractOperation::ExactIntegerAdd {
                psi_operation: id(308, OperationId::new),
                obligation: id(309, ObligationId::new),
                result: sum,
                scalar_type: integer,
                left,
                right,
            },
        ];
        if include_multiply {
            operations.push(TerminalAbstractOperation::ExactIntegerMultiply {
                psi_operation: id(312, OperationId::new),
                obligation: id(313, ObligationId::new),
                result: product,
                scalar_type: integer,
                left: sum,
                right,
            });
        }
        operations.push(TerminalAbstractOperation::Return {
            psi_edge: id(310, EdgeId::new),
            result,
            value: result,
            scalar_type: ScalarType::Integer(integer),
            cleanup_actions: Vec::new(),
        });
        reconstruct_psi_optimization_unit_seed(
            &TerminalAbstractOperationPlan {
                terminal_psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![TerminalAbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: TerminalAbstractFunctionResult::Scalar(TerminalAbstractResult {
                        value: result,
                        scalar_type: ScalarType::Integer(integer),
                    }),
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![TerminalAbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations,
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn selected_builtin_proposes_one_independently_validated_exact_fold() {
        let unit = exact_add_unit();
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let products = vec![constants];
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        assert_eq!(registry.len(), 3);
        let mut dispatched = 0usize;
        let mut candidates = Vec::new();
        for rule in registry.iter() {
            dispatched += 1;
            candidates.extend(
                rule.propose(&unit, RuleAnalysisView::new(&products))
                    .unwrap(),
            );
        }
        assert_eq!(dispatched, registry.len());
        assert_eq!(candidates.len(), 1);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(15),
                ..
            }
        ));
    }

    #[test]
    fn absent_selection_registers_nothing_and_missing_analysis_fails_closed() {
        let unit = exact_add_unit();
        assert!(
            built_in_psi_registry(&OptimizationSelections::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            ExactIntegerAddConstantsRule.propose(&unit, RuleAnalysisView::new(&[])),
            Err(RuleProposalError::MissingAnalysis(
                AnalysisKind::ScalarConstants
            ))
        );
        let unsupported = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        assert!(matches!(
            built_in_psi_registry(&unsupported),
            Err(RuleRegistryError::UnsupportedOptimization(
                Optimization::ControlFlowCleanup
            ))
        ));
    }
}
