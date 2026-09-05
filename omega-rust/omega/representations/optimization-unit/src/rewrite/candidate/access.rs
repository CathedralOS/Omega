//! Canonical read-only access to an admitted rewrite candidate.

use super::super::*;

impl PsiRewriteCandidate {
    pub const fn identity(&self) -> OptimizationCandidateIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        self.rule
    }

    pub const fn decision_point(&self) -> &PsiRewriteDecisionPoint {
        &self.decision_point
    }

    pub const fn node_decision_point(&self) -> Option<NodeLocation> {
        match &self.decision_point {
            PsiRewriteDecisionPoint::Node(location) => Some(*location),
            PsiRewriteDecisionPoint::MachineSet(_) => None,
        }
    }

    pub fn affected_machines(&self) -> &[MachineId] {
        match &self.decision_point {
            PsiRewriteDecisionPoint::Node(_) => &[],
            PsiRewriteDecisionPoint::MachineSet(machines) => machines,
        }
    }

    pub fn affected_blocks(&self) -> &[BlockId] {
        &self.affected_blocks
    }

    pub const fn required_analyses(&self) -> AnalysisSet {
        self.required_analyses
    }

    pub const fn invalidated_analyses(&self) -> AnalysisInvalidationSet {
        self.invalidated_analyses
    }

    pub const fn safety_class(&self) -> OptimizationSafetyClass {
        self.safety_class
    }

    pub fn substitutions(&self) -> &[ScalarSubstitution] {
        &self.substitutions
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }

    pub const fn scalar_evaluation_witness(&self) -> Option<ScalarEvaluationWitness> {
        match &self.witness {
            PsiRewriteWitness::ScalarEvaluation(witness) => Some(*witness),
            PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::AcceptedObligation(_)
            | PsiRewriteWitness::ProofCertifiedScalarIdentity { .. }
            | PsiRewriteWitness::TotalScalarIdentity { .. }
            | PsiRewriteWitness::OwnershipFrontiers(_)
            | PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub fn redundant_block_parameter_witness(&self) -> Option<&RedundantBlockParameterWitness> {
        match &self.witness {
            PsiRewriteWitness::ScalarEvaluation(_) => None,
            PsiRewriteWitness::RedundantBlockParameter(witness) => Some(witness),
            PsiRewriteWitness::AcceptedObligation(_) => None,
            PsiRewriteWitness::ProofCertifiedScalarIdentity { .. } => None,
            PsiRewriteWitness::TotalScalarIdentity { .. } => None,
            PsiRewriteWitness::OwnershipFrontiers(_) => None,
            PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub const fn accepted_obligation_witness(&self) -> Option<AcceptedObligationFactIdentity> {
        match &self.witness {
            PsiRewriteWitness::AcceptedObligation(identity) => Some(*identity),
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                obligation_fact, ..
            } => Some(*obligation_fact),
            PsiRewriteWitness::ScalarEvaluation(_)
            | PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::TotalScalarIdentity { .. }
            | PsiRewriteWitness::OwnershipFrontiers(_)
            | PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub const fn proof_certified_scalar_identity_witness(
        &self,
    ) -> Option<(ScalarConstantFactIdentity, AcceptedObligationFactIdentity)> {
        match &self.witness {
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            } => Some((*constant_fact, *obligation_fact)),
            PsiRewriteWitness::ScalarEvaluation(_)
            | PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::AcceptedObligation(_)
            | PsiRewriteWitness::TotalScalarIdentity { .. }
            | PsiRewriteWitness::OwnershipFrontiers(_)
            | PsiRewriteWitness::StructuralIdentity => None,
        }
    }

    pub const fn total_scalar_identity_witness(&self) -> Option<ScalarConstantFactIdentity> {
        match &self.witness {
            PsiRewriteWitness::TotalScalarIdentity { constant_fact } => Some(*constant_fact),
            _ => None,
        }
    }

    pub fn ownership_frontier_witness(&self) -> Option<&OwnershipFrontierWitness> {
        match &self.witness {
            PsiRewriteWitness::OwnershipFrontiers(witness) => Some(witness),
            _ => None,
        }
    }

    pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
        let mut facts = match &self.witness {
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary {
                operand_fact,
            }) => {
                vec![OptimizationFactReference::ScalarConstant(*operand_fact)]
            }
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Binary {
                left_fact,
                right_fact,
            }) => vec![
                OptimizationFactReference::ScalarConstant(*left_fact),
                OptimizationFactReference::ScalarConstant(*right_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::ProofCertifiedUnary {
                operand_fact,
                obligation_fact,
            }) => vec![
                OptimizationFactReference::ScalarConstant(*operand_fact),
                OptimizationFactReference::AcceptedObligation(*obligation_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::ProofCertifiedBinary {
                    left_fact,
                    right_fact,
                    obligation_fact,
                },
            ) => vec![
                OptimizationFactReference::ScalarConstant(*left_fact),
                OptimizationFactReference::ScalarConstant(*right_fact),
                OptimizationFactReference::AcceptedObligation(*obligation_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(
                ScalarEvaluationWitness::RangeAgainstConstant {
                    range_fact,
                    constant_fact,
                },
            ) => vec![
                OptimizationFactReference::ValueRange(*range_fact),
                OptimizationFactReference::ScalarConstant(*constant_fact),
            ],
            PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::RangeAgainstRange {
                left_range_fact,
                right_range_fact,
            }) => vec![
                OptimizationFactReference::ValueRange(*left_range_fact),
                OptimizationFactReference::ValueRange(*right_range_fact),
            ],
            PsiRewriteWitness::AcceptedObligation(identity) => {
                vec![OptimizationFactReference::AcceptedObligation(*identity)]
            }
            PsiRewriteWitness::ProofCertifiedScalarIdentity {
                constant_fact,
                obligation_fact,
            } => vec![
                OptimizationFactReference::ScalarConstant(*constant_fact),
                OptimizationFactReference::AcceptedObligation(*obligation_fact),
            ],
            PsiRewriteWitness::TotalScalarIdentity { constant_fact } => {
                vec![OptimizationFactReference::ScalarConstant(*constant_fact)]
            }
            PsiRewriteWitness::OwnershipFrontiers(witness) => witness
                .rows
                .iter()
                .map(|row| OptimizationFactReference::OwnershipFrontier(row.fact))
                .collect(),
            PsiRewriteWitness::RedundantBlockParameter(_)
            | PsiRewriteWitness::StructuralIdentity => Vec::new(),
        };
        facts.sort_unstable();
        facts.dedup();
        facts
    }

    pub const fn predicted_cost_delta(&self) -> i64 {
        self.predicted_cost_delta
    }

    pub fn patch(&self) -> PsiRewritePatch {
        self.patch.clone()
    }

    pub const fn patch_ref(&self) -> &PsiRewritePatch {
        &self.patch
    }
}
