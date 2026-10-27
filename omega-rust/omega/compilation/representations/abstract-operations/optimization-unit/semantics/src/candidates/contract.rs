//! The rule-contract roster every candidate family is admitted against.
//!
//! A candidate names the rule it belongs to, the analyses that rule requires
//! and invalidates, and the safety class it claims. The family entrance that
//! independently replays the rewrite already knows that row, so it must check
//! the candidate against it rather than against a second spelling in its own
//! file. Each entrance used to carry the whole row inline — rule domain bytes,
//! both analysis lists and the safety class, one argument per list element —
//! so a rule whose roster changed had to be corrected in the entrance, in the
//! sibling entrance that copied it, and in every family that shared the same
//! invalidation triple. A drifted copy still compiles: the candidate keeps
//! claiming its producer's rows while the replay runs on a premise the
//! candidate never named, and the disagreement surfaces as a rejected
//! candidate, not as a diagnostic pointing at the drifted row.
//!
//! The roster owns each rule's row once and [`admit`] is the one comparison.

use optimization_core::AnalysisKind::{
    CallGraph, ControlFlowGraph, Dominators, EffectSummaries, ExecutableEdges, LoopForest,
    OwnershipFrontiers, PostDominators, ScalarConstants, StronglyConnectedComponents,
    UseDefinition, ValueLiveness, ValueRanges,
};
use optimization_core::OptimizationSafetyClass::{ExactOperationSemantics, StructuralIdentity};
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleIdentity,
    OptimizationSafetyClass,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

/// One rule's candidate contract: the rule domain the candidate must name, the
/// analyses that rule requires and invalidates, the safety class it claims, and
/// the saving its rewrite predicts where the family pins one.
pub(crate) struct RuleContract {
    pub(crate) rule_domain: &'static [u8],
    pub(crate) required_analyses: &'static [AnalysisKind],
    pub(crate) invalidated_analyses: &'static [AnalysisKind],
    pub(crate) safety_class: OptimizationSafetyClass,
    pub(crate) predicted_cost_delta: Option<i64>,
}

/// Every control-flow cleanup rewrite lands edges, so all of them invalidate
/// the graph, the use-definition index and the effect summaries identically.
const CONTROL_FLOW_INVALIDATIONS: &[AnalysisKind] =
    &[ControlFlowGraph, UseDefinition, EffectSummaries];

/// Machine pruning additionally invalidates the call graph it may have emptied.
const CONTROL_FLOW_AND_CALL_GRAPH_INVALIDATIONS: &[AnalysisKind] =
    &[ControlFlowGraph, CallGraph, UseDefinition, EffectSummaries];

/// A rewrite that rebinds one value or one block parameter invalidates the
/// use-definition index and the effect summaries, nothing else.
pub(crate) const USE_DEFINITION_AND_EFFECT_SUMMARIES: &[AnalysisKind] =
    &[UseDefinition, EffectSummaries];

/// Both structural specializations replace a place with a proven constant or
/// initializer, invalidating the same four products.
const SPECIALIZATION_INVALIDATIONS: &[AnalysisKind] =
    &[UseDefinition, ScalarConstants, ValueRanges, EffectSummaries];

/// Expand the roster rows into the named consts each family entrance admits
/// against. One row per rule, read as a table: the rule domain, the analyses
/// it requires, the analyses it invalidates, its safety class, and the saving
/// its rewrite predicts (`None` where the family pins none).
#[rustfmt::skip]
macro_rules! rule_rows {
    ( $( ($name:ident, $domain:literal, $required:expr, $invalidated:expr, $safety:expr, $delta:expr) ),+ $(,)? ) => {
        $(
            pub(crate) const $name: RuleContract = RuleContract {
                rule_domain: $domain,
                required_analyses: $required,
                invalidated_analyses: $invalidated,
                safety_class: $safety,
                predicted_cost_delta: $delta,
            };
        )+
    };
}

#[rustfmt::skip]
rule_rows![
    (ADJACENT_BLOCK_MERGE, b"omega.psi-rule.adjacent-single-predecessor-block-merge.v5", &[ControlFlowGraph, Dominators, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-2)),
    (NON_ADJACENT_BLOCK_MERGE, b"omega.psi-rule.non-adjacent-unique-predecessor-block-merge.v1", &[ControlFlowGraph, Dominators, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-2)),
    (LINEAR_EMPTY_BLOCK_THREAD, b"omega.psi-rule.linear-empty-block-thread.v2", &[ControlFlowGraph, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-3)),
    (PATH_QUALIFIED_EMPTY_BLOCK_THREAD, b"omega.psi-rule.path-qualified-empty-block-thread.v1", &[ControlFlowGraph, UseDefinition, OwnershipFrontiers], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-3)),
    (SHARED_TERMINAL_JUMP_FUSION, b"omega.psi-rule.shared-terminal-jump-fusion.v2", &[ControlFlowGraph, OwnershipFrontiers, PostDominators], CONTROL_FLOW_INVALIDATIONS, StructuralIdentity, Some(-1)),
    (CONSTANT_CONDITIONAL_FOLD, b"omega.psi-rule.constant-conditional-fold.v5", &[ControlFlowGraph, ScalarConstants], CONTROL_FLOW_AND_CALL_GRAPH_INVALIDATIONS, ExactOperationSemantics, Some(-1)),
    (UNREACHABLE_PRIVATE_MACHINE_PRUNING, b"omega.psi-rule.unreachable-private-machine-pruning.v1", &[CallGraph], CONTROL_FLOW_AND_CALL_GRAPH_INVALIDATIONS, StructuralIdentity, None),
    (REDUNDANT_BLOCK_PARAMETER, b"omega.psi-rule.redundant-block-parameter.v1", &[ControlFlowGraph, Dominators, UseDefinition], USE_DEFINITION_AND_EFFECT_SUMMARIES, StructuralIdentity, Some(-1)),
    (CASE_MEMBERSHIP_SPECIALIZATION, b"omega.psi-rule.case-membership-specialization.v1", &[StronglyConnectedComponents], SPECIALIZATION_INVALIDATIONS, StructuralIdentity, None),
    (FIELD_VALUE_SPECIALIZATION, b"omega.psi-rule.field-value-specialization.v1", &[StronglyConnectedComponents], SPECIALIZATION_INVALIDATIONS, StructuralIdentity, None),
    (STATE_ARGUMENT_SPECIALIZATION, b"omega.psi-rule.state-argument-specialization.v1", &[ScalarConstants, StronglyConnectedComponents], &[ControlFlowGraph, Dominators, PostDominators, LoopForest, StronglyConnectedComponents, UseDefinition, ExecutableEdges, ScalarConstants, ValueRanges, EffectSummaries, ValueLiveness], StructuralIdentity, None),
];

/// Admit one candidate against its rule's row: the input it was built from,
/// the rule it names, both analysis rosters, the safety class it claims, and —
/// where the row pins one — the saving it predicts.
pub(crate) fn admit(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    contract: &RuleContract,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    if candidate.rule() != OptimizationRuleIdentity::from_canonical_bytes(contract.rule_domain)
        || candidate.required_analyses()
            != AnalysisSet::new(contract.required_analyses.iter().copied())
        || candidate.invalidated_analyses()
            != AnalysisInvalidationSet::new(contract.invalidated_analyses.iter().copied())
        || candidate.safety_class() != contract.safety_class
        || contract
            .predicted_cost_delta
            .is_some_and(|delta| candidate.predicted_cost_delta() != delta)
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
