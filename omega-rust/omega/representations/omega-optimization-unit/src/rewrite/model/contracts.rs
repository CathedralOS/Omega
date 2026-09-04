use super::super::*;
use super::cfg_rewrite_plans::*;
use super::foundations::*;
use super::scalar_evaluation::*;
use super::scalar_rewrite_plans::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRewriteDecisionPoint {
    Node(NodeLocation),
    MachineSet(Vec<MachineId>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiRewritePatch {
    ReplaceIntegerOperationWithConstant(IntegerConstantRewrite),
    ReplaceBooleanOperationWithConstant(BooleanConstantRewrite),
    RemoveRedundantBlockParameter(RedundantBlockParameterRewrite),
    FoldConstantConditional(ConstantConditionalRewrite),
    ThreadLinearEmptyBlock(LinearEmptyBlockRewrite),
    ThreadPathQualifiedEmptyBlock(PathQualifiedEmptyBlockRewrite),
    MergeAdjacentBlock(AdjacentBlockMergeRewrite),
    MergeNonAdjacentBlock(NonAdjacentBlockMergeRewrite),
    FuseSharedTerminalJump(SharedJumpFusionRewrite),
    RemoveDeadScalarNode(DeadScalarNodeRewrite),
    EliminateLocalScalarCommonSubexpression(LocalScalarCommonSubexpressionRewrite),
    EliminateDominatedScalarCommonSubexpression(DominatingScalarCommonSubexpressionRewrite),
    EliminatePhiTranslatedScalarCommonSubexpression(PhiTranslatedScalarGvnRewrite),
    EliminateProofCertifiedScalarIdentity(ProofCertifiedScalarIdentityRewrite),
    PruneUnreachablePrivateMachines(UnreachablePrivateMachinesRewrite),
    EliminateTotalScalarIdentity(TotalScalarIdentityRewrite),
}

impl PsiRewritePatch {
    pub fn pruned_machine_custody(&self) -> &[crate::PrunedMachineCustody] {
        match self {
            Self::PruneUnreachablePrivateMachines(patch) => &patch.machines,
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(in crate::rewrite) enum PsiRewriteWitness {
    ScalarEvaluation(ScalarEvaluationWitness),
    RedundantBlockParameter(RedundantBlockParameterWitness),
    AcceptedObligation(AcceptedObligationFactIdentity),
    ProofCertifiedScalarIdentity {
        constant_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
    },
    TotalScalarIdentity {
        constant_fact: ScalarConstantFactIdentity,
    },
    OwnershipFrontiers(OwnershipFrontierWitness),
    StructuralIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiRewriteCandidate {
    pub(in crate::rewrite) identity: OptimizationCandidateIdentity,
    pub(in crate::rewrite) input: OptimizationUnitIdentity,
    pub(in crate::rewrite) rule: OptimizationRuleIdentity,
    pub(in crate::rewrite) decision_point: PsiRewriteDecisionPoint,
    pub(in crate::rewrite) affected_blocks: Vec<BlockId>,
    pub(in crate::rewrite) required_analyses: AnalysisSet,
    pub(in crate::rewrite) invalidated_analyses: AnalysisInvalidationSet,
    pub(in crate::rewrite) safety_class: OptimizationSafetyClass,
    pub(in crate::rewrite) substitutions: Vec<ScalarSubstitution>,
    pub(in crate::rewrite) provenance: Vec<ProvenanceRewrite>,
    pub(in crate::rewrite) witness: PsiRewriteWitness,
    pub(in crate::rewrite) predicted_cost_delta: i64,
    pub(in crate::rewrite) patch: PsiRewritePatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiRewriteCandidateError {
    EmptyAffectedRegion,
    NonCanonicalAffectedRegion,
    DecisionPointOutsideRegion,
    NonCanonicalSubstitutions,
    EmptyProvenanceSource,
    NonCanonicalProvenance,
    FuelProvenanceMismatch,
    PatchDecisionPointMismatch,
    EmptyIncomingBindings,
    NonCanonicalIncomingBindings,
    NonCanonicalOwnershipFrontierWitness,
    BlockParameterSubstitutionMismatch,
    ProofWitnessSafetyMismatch,
}

impl std::fmt::Display for PsiRewriteCandidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi rewrite candidate: {self:?}")
    }
}

impl std::error::Error for PsiRewriteCandidateError {}
