//! The rule rows the candidate-constructor family expands into its public
//! entrances.
//!
//! Every rule's candidate entrance carries the same roster — input identity,
//! rule contract, affected blocks, substitution list, provenance rows,
//! witness, predicted saving, patch — and differs from its siblings only in
//! the patch and witness variant it names, and in the substitution list its
//! patch implies. The family used to restate that roster in a hand-written
//! signature and an eight-line forwarding block per rule, so a rule added to
//! the crate was a copied constructor, and a missed line in one copy silently
//! changed the candidate's witness or patch. Both are hashed into the
//! candidate identity, so the drift surfaced as a mismatched identity — a
//! repair hunt rather than a compile error.
//!
//! One row per rule, one arm per roster shape. Entrances that carry a
//! precondition of their own stay hand-written beside their families.

/// Rules whose candidate carries no substitution and the structural-identity
/// witness: the patch itself is the whole evidence.
#[rustfmt::skip]
macro_rules! structural_identity_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, Vec::new(), provenance, PsiRewriteWitness::StructuralIdentity, predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

/// Rules that rebind scalars: the caller passes the substitution list, and the
/// candidate still carries the structural-identity witness.
#[rustfmt::skip]
macro_rules! substituted_structural_identity_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, substitutions: Vec<ScalarSubstitution>, provenance: Vec<ProvenanceRewrite>, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, substitutions, provenance, PsiRewriteWitness::StructuralIdentity, predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

/// Rules whose candidate carries the closed scalar evaluation the rule
/// published as its own witness.
#[rustfmt::skip]
macro_rules! scalar_evaluation_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, substitutions: Vec<ScalarSubstitution>, provenance: Vec<ProvenanceRewrite>, witness: ScalarEvaluationWitness, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, substitutions, provenance, PsiRewriteWitness::ScalarEvaluation(witness), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

/// Rules whose candidate carries one accepted obligation and no substitution:
/// the obligation is the exact evidence the replay re-derives.
#[rustfmt::skip]
macro_rules! accepted_obligation_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, obligation_fact: AcceptedObligationFactIdentity, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, Vec::new(), provenance, PsiRewriteWitness::AcceptedObligation(obligation_fact), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

/// The constant-conditional fold's witness: the unary scalar evaluation of the
/// condition fact the rule proved.
#[rustfmt::skip]
macro_rules! condition_fact_constructors {
    ( $( ($name:ident, $patch:ty, $variant:ident) ),+ $(,)? ) => { $(
        #[allow(clippy::too_many_arguments)]
        pub fn $name(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, condition_fact: ScalarConstantFactIdentity, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, Vec::new(), provenance, PsiRewriteWitness::ScalarEvaluation(ScalarEvaluationWitness::Unary { operand_fact: condition_fact }), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}

/// Common-subexpression rules whose rewrite rebinds one result to its leader.
/// One row generates both entrances — the obligation-free one and the
/// proof-certified one — because they differ only in the witness; both derive
/// the single substitution the patch implies from the patch's own fields
/// rather than from a second spelling at each call site.
#[rustfmt::skip]
macro_rules! redundant_result_pair_constructors {
    ( $( ($plain:ident, $certified:ident, $patch:ty, $variant:ident, $from:ident, $to:ident, $scalar_type:ident) ),+ $(,)? ) => { $(
        pub fn $plain(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, vec![ScalarSubstitution { from: patch.$from, to: patch.$to, scalar_type: patch.$scalar_type }], provenance, PsiRewriteWitness::StructuralIdentity, predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }

        #[allow(clippy::too_many_arguments)]
        pub fn $certified(input: OptimizationUnitIdentity, contract: OptimizationRuleContract, affected_blocks: Vec<BlockId>, provenance: Vec<ProvenanceRewrite>, obligation_fact: AcceptedObligationFactIdentity, predicted_cost_delta: i64, patch: $patch) -> Result<Self, PsiRewriteCandidateError> {
            Self::new(input, contract, affected_blocks, vec![ScalarSubstitution { from: patch.$from, to: patch.$to, scalar_type: patch.$scalar_type }], provenance, PsiRewriteWitness::AcceptedObligation(obligation_fact), predicted_cost_delta, PsiRewritePatch::$variant(patch))
        }
    )+ };
}
