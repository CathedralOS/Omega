//! Removal of unused scalar computations, grouped by their semantic safety proof.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
};
use omega_optimization_unit::{
    DeadScalarNodeRewrite, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use psi_core::{BlockId, OperationId, ValueId};

use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

use super::{DEAD_PURE_SCALAR_PASS_NAME, PROOF_CHECK_ELISION_PASS_NAME, accepted_obligation_fact};

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadScalarLiteralEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct DeadUnconditionallyTotalScalarEliminationRule;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProofCertifiedDeadScalarEliminationRule;

#[derive(Debug, Clone, Copy)]
enum DeadScalarFamily {
    Literal,
    UnconditionallyTotal,
    ProofCertified,
}

impl DeadScalarLiteralEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(DEAD_PURE_SCALAR_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DeadScalarLiteralEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(unit, analyses, Self::contract(), DeadScalarFamily::Literal)
    }
}

impl DeadUnconditionallyTotalScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-unconditionally-total-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(DEAD_PURE_SCALAR_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for DeadUnconditionallyTotalScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(
            unit,
            analyses,
            Self::contract(),
            DeadScalarFamily::UnconditionallyTotal,
        )
    }
}

impl ProofCertifiedDeadScalarEliminationRule {
    pub fn contract() -> OptimizationRuleContract {
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.dead-unused-proof-certified-scalar-elimination.v1",
            ),
            OptimizationPassIdentity::from_canonical_bytes(PROOF_CHECK_ELISION_PASS_NAME),
            1,
            AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
            AnalysisInvalidationSet::new([
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            OptimizationSafetyClass::ProofCertified,
        )
        .expect("built-in rule has nonzero version")
    }
}

impl PsiOptimizationRule for ProofCertifiedDeadScalarEliminationRule {
    fn contract(&self) -> OptimizationRuleContract {
        Self::contract()
    }

    fn propose(
        &self,
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        propose_dead_scalar_nodes(
            unit,
            analyses,
            Self::contract(),
            DeadScalarFamily::ProofCertified,
        )
    }
}

fn propose_dead_scalar_nodes(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    family: DeadScalarFamily,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ValueLiveness(liveness)) = analyses.get(AnalysisKind::ValueLiveness)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ValueLiveness,
        ));
    };
    let Some(AnalysisProduct::EffectSummaries(effects)) =
        analyses.get(AnalysisKind::EffectSummaries)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::EffectSummaries,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some((source_operation, result, scalar_type)) =
                    dead_scalar_shape(&node.operation, family)
                else {
                    continue;
                };
                let Some(next) = block.nodes.get(node_index + 1) else {
                    continue;
                };
                if next
                    .provenance
                    .iter()
                    .any(|source| node.provenance.contains(source))
                {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node index fits u32");
                let live = liveness
                    .blocks
                    .iter()
                    .find(|row| row.machine == function.machine && row.block == block.id)
                    .and_then(|row| row.nodes.iter().find(|row| row.node == node_index));
                let effect = effects.nodes.iter().find(|row| {
                    row.machine == function.machine
                        && row.block == block.id
                        && row.node == node_index
                });
                if live.is_none_or(|row| row.exit.contains(&result))
                    || effect.is_none_or(|row| {
                        row.revision != unit.identity
                            || row.class != crate::EffectClass::PureScalar
                            || row.observable != crate::EffectKnowledge::No
                            || row.structural_state != crate::EffectKnowledge::No
                            || row.crash != crate::EffectKnowledge::No
                            || row.suspension != crate::EffectKnowledge::No
                    })
                {
                    continue;
                }
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                let Some((affected_blocks, provenance)) =
                    dead_scalar_node_accounting(function, location)
                else {
                    continue;
                };
                let patch = DeadScalarNodeRewrite {
                    location,
                    source_operation,
                    result,
                    scalar_type,
                };
                let candidate = if matches!(family, DeadScalarFamily::ProofCertified) {
                    PsiRewriteCandidate::new_proof_certified_dead_scalar_node(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        accepted_obligation_fact(unit, function.machine, source_operation)?,
                        -1,
                        patch,
                    )
                } else {
                    PsiRewriteCandidate::new_dead_scalar_node(
                        unit.identity,
                        contract,
                        affected_blocks,
                        provenance,
                        -1,
                        patch,
                    )
                };
                candidates.push(candidate.map_err(RuleProposalError::InvalidCandidate)?);
            }
        }
    }
    Ok(candidates)
}

fn dead_scalar_shape(
    operation: &O,
    family: DeadScalarFamily,
) -> Option<(OperationId, ValueId, psi_core::ScalarType)> {
    match (family, operation) {
        (
            DeadScalarFamily::Literal,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((*psi_operation, *result, *scalar_type)),
        (
            DeadScalarFamily::Literal,
            O::BooleanConstant {
                psi_operation,
                result,
                ..
            },
        ) => Some((*psi_operation, *result, psi_core::ScalarType::Boolean)),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::BooleanNot {
                psi_operation,
                result,
                ..
            }
            | O::BooleanEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerEqual {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessThan {
                psi_operation,
                result,
                ..
            }
            | O::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            },
        ) => Some((*psi_operation, *result, psi_core::ScalarType::Boolean)),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*scalar_type),
        )),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::IntegerWiden {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*target_type),
        )),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::WrappingIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                ..
            }
            | O::WrappingIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*value_type),
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::IntegerExactCast {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*target_type),
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::ExactIntegerShiftLeft {
                psi_operation,
                result,
                value_type,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                result,
                value_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*value_type),
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::ExactIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            psi_core::ScalarType::Integer(*scalar_type),
        )),
        _ => None,
    }
}

fn dead_scalar_node_accounting(
    function: &omega_optimization_unit::PsiOptimizationFunction,
    location: NodeLocation,
) -> Option<(Vec<BlockId>, Vec<ProvenanceRewrite>)> {
    let block_position = function
        .blocks
        .iter()
        .position(|block| block.id == location.block)?;
    let node_position = usize::try_from(location.node).ok()?;
    let block = &function.blocks[block_position];
    let removed = block.nodes.get(node_position)?;
    block.nodes.get(node_position.checked_add(1)?)?;
    let output_receiver = PsiRealizationSite::Node(location);
    let mut provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(location),
        disposition: ProvenanceDisposition::RealizedAt(output_receiver),
        sources: removed.provenance.clone(),
        fuel: removed.fuel.clone(),
    }];
    for (index, node) in block.nodes.iter().enumerate().skip(node_position + 1) {
        if node.provenance.is_empty() {
            continue;
        }
        let old = NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(index).ok()?,
        };
        let new = NodeLocation {
            node: old.node.checked_sub(1)?,
            ..old
        };
        provenance.push(ProvenanceRewrite {
            input: PsiRealizationSite::Node(old),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(new)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    let mut affected = vec![block.id];
    for later in function.blocks.iter().skip(block_position + 1) {
        affected.push(later.id);
        for (index, node) in later.nodes.iter().enumerate() {
            if node.provenance.is_empty() {
                continue;
            }
            let site = PsiRealizationSite::Node(NodeLocation {
                machine: function.machine,
                block: later.id,
                node: u32::try_from(index).ok()?,
            });
            provenance.push(ProvenanceRewrite {
                input: site,
                disposition: ProvenanceDisposition::RealizedAt(site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
    }
    affected.sort();
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Some((affected, provenance))
}
