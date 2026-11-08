/*
Proof-check elision pass (`proof_check_elision`): proof-certified scalar
identity rewrites (multiply by zero, divide/remainder by one, zero-dividend
and zero-shift elision, arithmetic identity, self-subtract, self-divide,
self-remainder, negative-one shift right, signed remainder by negative one)
and proof-certified dead scalar elimination.
*/

use abstract_operations::IntegerArithmeticOperator;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass,
};
use optimization_unit::{
    IntegerConstantRewrite, NodeLocation, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType, IntegerValue};

use crate::rules::PROOF_CHECK_ELISION_PASS_NAME;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::rules::control_flow_cleanup::node_realized_provenance;
use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

pub(crate) mod identity_rewrite {
    use abstract_operations::AbstractOperation;
    use optimization_core::{AnalysisKind, OptimizationRuleContract};
    use optimization_unit::{
        IntegerConstantRewrite, NodeLocation, ProofCertifiedScalarIdentityKind,
        ProofCertifiedScalarIdentityRewrite, PsiOptimizationUnit, PsiRewriteCandidate,
        scalar_value_definition,
    };
    use semantic_vocabulary::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};

    use crate::rules::accepted_obligations::accepted_obligation_fact;
    use crate::rules::control_flow_cleanup::node_realized_provenance;
    use crate::rules::node_elision::node_elision_accounting;
    use crate::{AnalysisProduct, RuleAnalysisView, RuleProposalError};

    pub(crate) fn propose_proof_certified_integer_constants(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        classify: fn(&AbstractOperation) -> Option<(OperationId, ValueId, IntegerType)>,
        constant: fn(IntegerType) -> IntegerValue,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let Some((psi_operation, result, scalar_type)) = classify(&node.operation)
                    else {
                        continue;
                    };
                    if !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == result
                    }) {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.node(function.machine, block.id, node_index);
                    if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
                        continue;
                    }
                    let Ok(obligation_fact) =
                        accepted_obligation_fact(unit, function.machine, psi_operation)
                    else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_integer_constant_replacement(
                            unit.identity,
                            contract,
                            vec![block.id],
                            vec![node_realized_provenance(location, node)],
                            obligation_fact,
                            -1,
                            IntegerConstantRewrite {
                                location,
                                source_operation: psi_operation,
                                result,
                                scalar_type,
                                constant: constant(scalar_type),
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }

    pub(crate) fn propose_proof_certified_scalar_identities(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        classify: fn(&AbstractOperation) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
        let mut candidates = Vec::new();
        for function in &unit.functions {
            for block in &function.blocks {
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let shapes = classify(&node.operation);
                    if shapes.is_empty() {
                        continue;
                    }
                    let node_index =
                        u32::try_from(node_index).expect("optimization node index fits u32");
                    let effect = effects.node(function.machine, block.id, node_index);
                    if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
                        continue;
                    }
                    let Some((patch_shape, constant_fact)) =
                        shapes.into_iter().find_map(|(shape, expected)| {
                            let result_type = ScalarType::Integer(shape.scalar_type);
                            if scalar_value_definition(function, shape.replacement)?.scalar_type
                                != result_type
                            {
                                return None;
                            }
                            if matches!(node.operation, AbstractOperation::IntegerArithmetic { .. })
                                && scalar_value_definition(function, shape.identity_operand)?
                                    .scalar_type
                                    != result_type
                            {
                                return None;
                            }
                            let (actual, fact) = constants.literal_integer_constant(
                                function.machine,
                                shape.identity_operand,
                            )?;
                            (actual == expected).then_some((shape, fact))
                        })
                    else {
                        continue;
                    };
                    if !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == patch_shape.result
                    }) {
                        continue;
                    }
                    let Ok(obligation_fact) = accepted_obligation_fact(
                        unit,
                        function.machine,
                        patch_shape.source_operation,
                    ) else {
                        continue;
                    };
                    let location = NodeLocation {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    };
                    let Some((affected_blocks, provenance)) =
                        node_elision_accounting(function, location, patch_shape.result)
                    else {
                        continue;
                    };
                    candidates.push(
                        PsiRewriteCandidate::new_proof_certified_scalar_identity(
                            unit.identity,
                            contract,
                            affected_blocks,
                            provenance,
                            constant_fact,
                            obligation_fact,
                            -1,
                            ProofCertifiedScalarIdentityRewrite {
                                location,
                                source_operation: patch_shape.source_operation,
                                result: patch_shape.result,
                                replacement: patch_shape.replacement,
                                scalar_type: patch_shape.scalar_type,
                                identity: patch_shape.identity,
                            },
                        )
                        .map_err(RuleProposalError::InvalidCandidate)?,
                    );
                }
            }
        }
        Ok(candidates)
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ProofCertifiedScalarIdentityShape {
        pub(crate) source_operation: OperationId,
        pub(crate) result: ValueId,
        pub(crate) replacement: ValueId,
        pub(crate) identity_operand: ValueId,
        pub(crate) scalar_type: IntegerType,
        pub(crate) identity: ProofCertifiedScalarIdentityKind,
    }
}
pub(crate) mod negative_one_shift_right {
    use abstract_operations::{AbstractOperation as O, IntegerShiftDirection, IntegerShiftPolicy};
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
    };
    use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerValue};

    use crate::rules::proof_check_elision::identity_rewrite::{
        ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
    };
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    fn proof_certified_exact_signed_integer_negative_one_shift_right_shapes(
        operation: &O,
    ) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
        let Some(operands) = operation.integer_shift() else {
            return Vec::new();
        };
        if !matches!(operands.policy, IntegerShiftPolicy::Exact { .. })
            || operands.direction != IntegerShiftDirection::Right
            || operands.value_type.carrier() != IntegerCarrier::Fixed
            || operands.value_type.sign() != IntegerSign::Signed
        {
            return Vec::new();
        }
        vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement: operands.value,
                identity_operand: operands.value,
                scalar_type: operands.value_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue,
            },
            IntegerValue::Signed(-1),
        )]
    }

    psi_rule_contract!(
        LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
        super::proof_check_elision_rule_contract(
                    b"omega.psi-rule.live-proof-certified-exact-signed-integer-negative-one-value-shift-right-elimination.v1",
                    AnalysisSet::new([
                        AnalysisKind::ScalarConstants,
                        AnalysisKind::UseDefinition,
                        AnalysisKind::EffectSummaries,
                    ]),
                ),
    );

    psi_rule_impl!(
        LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
        |unit, analyses| {
            propose_proof_certified_scalar_identities(
                unit,
                analyses,
                Self::contract(),
                proof_certified_exact_signed_integer_negative_one_shift_right_shapes,
            )
        }
    );
}
pub(crate) mod proof_certified_dead_scalar_elimination {
    use abstract_operations::{AbstractOperation as O, IntegerShiftPolicy};
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::ScalarType;

    use crate::rules::node_elision::{DeadScalarShape, propose_proof_certified_dead_scalar_nodes};
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        ProofCertifiedDeadScalarEliminationRule,
        super::proof_check_elision_rule_contract(
            b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
        ),
        propose = propose_proof_certified_dead_scalar_nodes,
        classify,
    );

    pub(crate) fn classify(operation: &O) -> Option<DeadScalarShape> {
        let (source_operation, result, scalar_type) = match operation {
            O::IntegerExactCast {
                psi_operation,
                result,
                target_type,
                ..
            } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
            O::IntegerShift {
                psi_operation,
                result,
                value_type,
                policy: IntegerShiftPolicy::Exact { .. },
                ..
            } => (*psi_operation, *result, ScalarType::Integer(*value_type)),
            O::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                ..
            } if arithmetic.obligation().is_some() => {
                (*psi_operation, *result, ScalarType::Integer(*scalar_type))
            }
            _ => return None,
        };
        Some(DeadScalarShape {
            source_operation,
            result,
            scalar_type,
        })
    }
}
pub(crate) mod remainder_by_one {
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        LiveProofCertifiedIntegerRemainderByOneEliminationRule,
        super::proof_check_elision_rule_contract(
            b"omega.psi-rule.live-proof-certified-integer-remainder-by-one-elimination.v1",
            AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
        ),
    );

    psi_rule_impl!(
        LiveProofCertifiedIntegerRemainderByOneEliminationRule,
        |unit, analyses| {
            super::propose_proof_certified_remainder_by_constant(
                unit,
                analyses,
                Self::contract(),
                false,
                |scalar_type, constant| constant == scalar_type.one(),
            )
        }
    );
}
pub(crate) mod scalar_identity_rules {
    use abstract_operations::{
        AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
        IntegerShiftDirection, IntegerShiftPolicy,
    };
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{
        ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
    };
    use semantic_vocabulary::IntegerValue;

    use crate::rules::proof_check_elision::identity_rewrite::{
        ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
    };
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    macro_rules! proof_certified_scalar_identity_rules {
        ($($(#[$meta:meta])* $name:ident = ($identity:literal, $shapes:path);)*) => {$(
            psi_rule_contract!(
                $(#[$meta])*
                $name,
                super::proof_check_elision_rule_contract(
                    $identity,
                    AnalysisSet::new([
                        AnalysisKind::ScalarConstants,
                        AnalysisKind::UseDefinition,
                        AnalysisKind::EffectSummaries,
                    ]),
                ),
                propose = propose_proof_certified_scalar_identities,
                $shapes,
            );
        )*};
    }

    proof_certified_scalar_identity_rules! {
        LiveProofCertifiedIntegerDivideByOneEliminationRule = (
            b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
            proof_certified_integer_divide_by_one_shapes
        );
        LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule = (
            b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
            proof_certified_exact_integer_multiply_by_zero_shapes
        );
        LiveProofCertifiedIntegerIdentityEliminationRule = (
            b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
            proof_certified_scalar_identity_shapes
        );
        LiveProofCertifiedIntegerZeroDividendEliminationRule = (
            b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
            proof_certified_integer_zero_dividend_shapes
        );
        LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule = (
            b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
            proof_certified_exact_integer_zero_value_shift_shapes
        );
    }

    fn proof_certified_integer_divide_by_one_shapes(
        operation: &O,
    ) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
        let Some(operands) =
            operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)
        else {
            return Vec::new();
        };
        let (source_operation, result, scalar_type, left, right, identity) = (
            operands.psi_operation,
            operands.result,
            operands.scalar_type,
            operands.left,
            operands.right,
            match operands.arithmetic.policy() {
                IntegerOverflowPolicy::Exact => {
                    ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight
                }
                IntegerOverflowPolicy::Wrapping => {
                    ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight
                }
                IntegerOverflowPolicy::Saturating => {
                    ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight
                }
            },
        );
        vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation,
                result,
                replacement: left,
                identity_operand: right,
                scalar_type,
                identity,
            },
            scalar_type.one(),
        )]
    }

    fn proof_certified_exact_integer_multiply_by_zero_shapes(
        operation: &O,
    ) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
        let Some(operands) =
            operation.integer_arithmetic_operands(IntegerArithmeticOperator::Multiply)
        else {
            return Vec::new();
        };
        if operands.arithmetic.policy() != IntegerOverflowPolicy::Exact {
            return Vec::new();
        }
        vec![
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: operands.psi_operation,
                    result: operands.result,
                    replacement: operands.left,
                    identity_operand: operands.left,
                    scalar_type: operands.scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
                },
                operands.scalar_type.zero(),
            ),
            (
                ProofCertifiedScalarIdentityShape {
                    source_operation: operands.psi_operation,
                    result: operands.result,
                    replacement: operands.right,
                    identity_operand: operands.right,
                    scalar_type: operands.scalar_type,
                    identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
                },
                operands.scalar_type.zero(),
            ),
        ]
    }

    fn proof_certified_scalar_identity_shapes(
        operation: &O,
    ) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
        use ProofCertifiedScalarIdentityKind as Kind;
        match operation {
            O::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                left,
                right,
            } if arithmetic.policy() == IntegerOverflowPolicy::Exact => {
                let zero = scalar_type.zero();
                let one = scalar_type.one();
                let laws = match arithmetic.operator() {
                    IntegerArithmeticOperator::Add => vec![
                        (*right, *left, Kind::ExactIntegerAddZeroLeft, zero),
                        (*left, *right, Kind::ExactIntegerAddZeroRight, zero),
                    ],
                    IntegerArithmeticOperator::Subtract => {
                        vec![(*left, *right, Kind::ExactIntegerSubtractZeroRight, zero)]
                    }
                    IntegerArithmeticOperator::Multiply => vec![
                        (*right, *left, Kind::ExactIntegerMultiplyOneLeft, one),
                        (*left, *right, Kind::ExactIntegerMultiplyOneRight, one),
                    ],
                    IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => {
                        Vec::new()
                    }
                };
                laws.into_iter()
                    .map(|(replacement, identity_operand, identity, constant)| {
                        (
                            ProofCertifiedScalarIdentityShape {
                                source_operation: *psi_operation,
                                result: *result,
                                replacement,
                                identity_operand,
                                scalar_type: *scalar_type,
                                identity,
                            },
                            constant,
                        )
                    })
                    .collect()
            }
            O::IntegerShift {
                psi_operation,
                result,
                value_type,
                count_type,
                policy: IntegerShiftPolicy::Exact { .. },
                direction,
                value,
                count,
            } => vec![(
                ProofCertifiedScalarIdentityShape {
                    source_operation: *psi_operation,
                    result: *result,
                    replacement: *value,
                    identity_operand: *count,
                    scalar_type: *value_type,
                    identity: match direction {
                        IntegerShiftDirection::Left => Kind::ExactIntegerShiftLeftZeroCount,
                        IntegerShiftDirection::Right => Kind::ExactIntegerShiftRightZeroCount,
                    },
                },
                count_type.zero(),
            )],
            _ => Vec::new(),
        }
    }

    fn proof_certified_integer_zero_dividend_shapes(
        operation: &O,
    ) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
        let Some(operands) = operation.integer_arithmetic() else {
            return Vec::new();
        };
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        use ProofCertifiedScalarIdentityKind as Kind;
        let identity = match (operands.arithmetic.policy(), operands.arithmetic.operator()) {
            (Policy::Exact, Operator::Divide) => Kind::ExactIntegerDivideZeroLeft,
            (Policy::Wrapping, Operator::Divide) => Kind::WrappingIntegerDivideZeroLeft,
            (Policy::Saturating, Operator::Divide) => Kind::SaturatingIntegerDivideZeroLeft,
            (Policy::Exact, Operator::Remainder) => Kind::ExactIntegerRemainderZeroLeft,
            (Policy::Wrapping, Operator::Remainder) => Kind::WrappingIntegerRemainderZeroLeft,
            (Policy::Saturating, Operator::Remainder) => Kind::SaturatingIntegerRemainderZeroLeft,
            (_, Operator::Add | Operator::Subtract | Operator::Multiply) => {
                return Vec::new();
            }
        };
        let (source_operation, result, scalar_type, left) = (
            operands.psi_operation,
            operands.result,
            operands.scalar_type,
            operands.left,
        );
        vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation,
                result,
                replacement: left,
                identity_operand: left,
                scalar_type,
                identity,
            },
            scalar_type.zero(),
        )]
    }

    fn proof_certified_exact_integer_zero_value_shift_shapes(
        operation: &O,
    ) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
        let Some(operands) = operation.integer_shift() else {
            return Vec::new();
        };
        if !matches!(operands.policy, IntegerShiftPolicy::Exact { .. }) {
            return Vec::new();
        }
        let identity = match operands.direction {
            IntegerShiftDirection::Left => {
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue
            }
            IntegerShiftDirection::Right => {
                ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue
            }
        };
        let (source_operation, result, scalar_type, value) = (
            operands.psi_operation,
            operands.result,
            operands.value_type,
            operands.value,
        );
        vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation,
                result,
                replacement: value,
                identity_operand: value,
                scalar_type,
                identity,
            },
            scalar_type.zero(),
        )]
    }
}
pub(crate) mod self_divide {
    use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType, OperationId, ValueId};

    use crate::rules::proof_check_elision::identity_rewrite::propose_proof_certified_integer_constants;
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        LiveProofCertifiedIntegerSelfDivideEliminationRule,
        super::proof_check_elision_rule_contract(
            b"omega.psi-rule.live-proof-certified-integer-self-divide-elimination.v1",
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        ),
        propose = propose_proof_certified_integer_constants,
        classify,
        IntegerType::one,
    );

    fn classify(operation: &O) -> Option<(OperationId, ValueId, IntegerType)> {
        let operands = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)?;
        if operands.left != operands.right
            || operands.scalar_type.carrier() != IntegerCarrier::Fixed
            || (operands.scalar_type.sign() == IntegerSign::Signed
                && operands.scalar_type.bits() == 1)
        {
            return None;
        }
        Some((
            operands.psi_operation,
            operands.result,
            operands.scalar_type,
        ))
    }
}
pub(crate) mod self_remainder {
    use abstract_operations::{AbstractOperation as O, IntegerArithmeticOperator};
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{IntegerType, OperationId, ValueId};

    use crate::rules::proof_check_elision::identity_rewrite::propose_proof_certified_integer_constants;
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        LiveProofCertifiedIntegerSelfRemainderEliminationRule,
        super::proof_check_elision_rule_contract(
            b"omega.psi-rule.live-proof-certified-integer-self-remainder-elimination.v1",
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        ),
        propose = propose_proof_certified_integer_constants,
        classify,
        IntegerType::zero,
    );

    fn classify(operation: &O) -> Option<(OperationId, ValueId, IntegerType)> {
        let operands =
            operation.integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)?;
        if operands.left != operands.right {
            return None;
        }
        Some((
            operands.psi_operation,
            operands.result,
            operands.scalar_type,
        ))
    }
}
pub(crate) mod self_subtract {
    use abstract_operations::{
        AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{IntegerType, OperationId, ValueId};

    use crate::rules::proof_check_elision::identity_rewrite::propose_proof_certified_integer_constants;
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
        super::proof_check_elision_rule_contract(
            b"omega.psi-rule.live-proof-certified-exact-integer-self-subtract-elimination.v1",
            AnalysisSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        ),
        propose = propose_proof_certified_integer_constants,
        classify,
        IntegerType::zero,
    );

    fn classify(operation: &O) -> Option<(OperationId, ValueId, IntegerType)> {
        let operands =
            operation.integer_arithmetic_operands(IntegerArithmeticOperator::Subtract)?;
        if !operands.arithmetic.is(
            IntegerOverflowPolicy::Exact,
            IntegerArithmeticOperator::Subtract,
        ) || operands.left != operands.right
        {
            return None;
        }
        Some((
            operands.psi_operation,
            operands.result,
            operands.scalar_type,
        ))
    }
}
pub(crate) mod signed_remainder_by_negative_one {
    use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
    use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::IntegerValue;

    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
        super::proof_check_elision_rule_contract(
                    b"omega.psi-rule.live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1",
                    AnalysisSet::new([
                        AnalysisKind::ScalarConstants,
                        AnalysisKind::UseDefinition,
                        AnalysisKind::EffectSummaries,
                    ]),
                ),
    );

    psi_rule_impl!(
        LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
        |unit, analyses| {
            super::propose_proof_certified_remainder_by_constant(
                unit,
                analyses,
                Self::contract(),
                true,
                |_, constant| constant == IntegerValue::Signed(-1),
            )
        }
    );
}

fn proof_check_elision_rule_contract(
    identity: &'static [u8],
    analyses: AnalysisSet,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        PROOF_CHECK_ELISION_PASS_NAME,
        1,
        analyses,
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        OptimizationSafetyClass::ProofCertified,
    )
}

pub(crate) use negative_one_shift_right::LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule;
pub(crate) use proof_certified_dead_scalar_elimination::ProofCertifiedDeadScalarEliminationRule;
pub(crate) use remainder_by_one::LiveProofCertifiedIntegerRemainderByOneEliminationRule;
pub(crate) use scalar_identity_rules::{
    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
    LiveProofCertifiedIntegerDivideByOneEliminationRule,
    LiveProofCertifiedIntegerIdentityEliminationRule,
    LiveProofCertifiedIntegerZeroDividendEliminationRule,
};
pub(crate) use self_divide::LiveProofCertifiedIntegerSelfDivideEliminationRule;
pub(crate) use self_remainder::LiveProofCertifiedIntegerSelfRemainderEliminationRule;
pub(crate) use self_subtract::LiveProofCertifiedExactIntegerSelfSubtractEliminationRule;
pub(crate) use signed_remainder_by_negative_one::LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule;

pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ProofCertifiedDeadScalarEliminationRule),
        BuiltInRuleRegistration::new(1, LiveProofCertifiedIntegerIdentityEliminationRule),
        BuiltInRuleRegistration::new(2, LiveProofCertifiedIntegerDivideByOneEliminationRule),
        BuiltInRuleRegistration::new(
            3,
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
        ),
        BuiltInRuleRegistration::new(4, LiveProofCertifiedIntegerZeroDividendEliminationRule),
        BuiltInRuleRegistration::new(
            5,
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
        ),
        BuiltInRuleRegistration::new(6, LiveProofCertifiedExactIntegerSelfSubtractEliminationRule),
        BuiltInRuleRegistration::new(7, LiveProofCertifiedIntegerSelfRemainderEliminationRule),
        BuiltInRuleRegistration::new(8, LiveProofCertifiedIntegerSelfDivideEliminationRule),
        BuiltInRuleRegistration::new(9, LiveProofCertifiedIntegerRemainderByOneEliminationRule),
        BuiltInRuleRegistration::new(
            10,
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
        ),
        BuiltInRuleRegistration::new(
            11,
            LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
        ),
    ]
}

fn propose_proof_certified_remainder_by_constant(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    signed_only: bool,
    expected_constant: impl Fn(IntegerType, IntegerValue) -> bool,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    let (use_definitions, effects) = analyses.use_definitions_and_effects()?;
    let mut candidates: Vec<PsiRewriteCandidate> = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(operands) = node
                    .operation
                    .integer_arithmetic_operands(IntegerArithmeticOperator::Remainder)
                else {
                    continue;
                };
                if operands.left == operands.right
                    || operands.scalar_type.carrier() != IntegerCarrier::Fixed
                    || (signed_only && operands.scalar_type.sign() != IntegerSign::Signed)
                    || !use_definitions.uses.iter().any(|(machine, use_site)| {
                        *machine == function.machine && use_site.value == operands.result
                    })
                {
                    continue;
                }
                let Some((constant, constant_fact)) =
                    constants.literal_integer_constant(function.machine, operands.right)
                else {
                    continue;
                };
                if !expected_constant(operands.scalar_type, constant)
                    || !operands.scalar_type.admits(constant)
                {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let effect = effects.node(function.machine, block.id, node_index);
                if effect.is_none_or(|row| !row.is_pure_scalar_baseline(unit.identity)) {
                    continue;
                }
                let Ok(obligation_fact) =
                    accepted_obligation_fact(unit, function.machine, operands.psi_operation)
                else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                candidates.push(
                    PsiRewriteCandidate::new_literal_proof_certified_integer_constant_replacement(
                        unit.identity,
                        contract,
                        vec![block.id],
                        vec![node_realized_provenance(location, node)],
                        constant_fact,
                        obligation_fact,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: operands.psi_operation,
                            result: operands.result,
                            scalar_type: operands.scalar_type,
                            constant: operands.scalar_type.zero(),
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}
