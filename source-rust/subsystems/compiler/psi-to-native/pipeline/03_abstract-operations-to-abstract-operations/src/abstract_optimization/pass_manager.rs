/*
Stage 03 runs the Psi optimization phase as its identity. The Psi phase runs in
the source compiler before Terminal publication (stage 06,
`source-to-psi/pipeline/06_lowered-program-to-checked-program/src/publication/optimization.rs`),
and native lowering receives only post-Terminal selections, so no Psi rule can
run here. `identity_run` records that empty run with the same identities, logs
and ledger that an empty pass schedule always produced, so the published
manifest and projection keep their bytes. The former rule engine is parked in
`_recycling/` for migration to stage 06.
*/

use crate::abstract_optimization::validation::validate_psi_context;
use optimization_core::{
    BaselineDecisionLog, BaselineDecisionLogDecodeError, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionSchemaError, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationRuleSetIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget, TargetCostModelIdentity,
    external_psi_decision_schema_identity, psi_target_neutral_decision_target_identity,
};
use optimization_unit::{
    InvalidPsiTransformationLedger, PsiOptimizationUnit, PsiTransformationLedger,
};
pub use optimization_unit::{
    OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration,
};
use optimization_unit_semantics::OptimizationUnitValidationError;
use verified_optimization_input::VerifiedPsiOptimizationInput;

pub(crate) fn baseline_psi_cost_model_identity() -> TargetCostModelIdentity {
    TargetCostModelIdentity::from_canonical_bytes(b"omega.psi-baseline-structural-cost-model.v1")
}

/// The decision context an empty Psi schedule records: the unit it started
/// from, the complete and Psi-local selections, and the empty rule set.
fn expected_context(
    source: OptimizationUnitIdentity,
    selections: &OptimizationSelections,
    phase_selections: &OptimizationSelections,
    rule_set: OptimizationRuleSetIdentity,
) -> ExternalDecisionContext {
    ExternalDecisionContext::new(
        external_psi_decision_schema_identity(),
        source,
        selections.identity(),
        phase_selections.identity(),
        psi_target_neutral_decision_target_identity(),
        rule_set,
        baseline_psi_cost_model_identity(),
    )
}

/// Record the identity Psi phase over an admitted session. `selections` is the
/// post-Terminal selection native lowering received; its Psi projection is
/// empty by construction.
pub(crate) fn identity_run(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let psi_selections =
        OptimizationSelections::new([]).expect("the empty Psi selection is duplicate-free");
    let unit = session.unit();
    let initial_identity = unit.identity;
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&[])
        .expect("the empty rule set has no duplicate rule");
    let decisions =
        BaselineDecisionLog::concatenate(&[]).map_err(OptimizationRunError::DecisionLogReplay)?;
    let external_decisions = ExternalDecisionLog::new(
        expected_context(
            initial_identity,
            selections,
            &psi_selections,
            ordered_rule_set,
        ),
        Vec::new(),
    )
    .map_err(OptimizationRunError::ExternalDecisionSchema)?;
    let ledger = PsiTransformationLedger::new(
        unit.psi,
        unit.work_schedule,
        initial_identity,
        initial_identity,
        Vec::new(),
    )
    .map_err(OptimizationRunError::InvalidTransformationLedger)?;
    let identity_bundle = OptimizationIdentityBundle::new(
        selections.identity(),
        ordered_rule_set,
        baseline_psi_cost_model_identity(),
        Some(decisions.identity),
        None,
        ledger.identity(),
    );
    Ok(OptimizationRun {
        selections: selections.clone(),
        psi_selections,
        budget_per_pass,
        session,
        commits: Vec::new(),
        validated_candidates: Vec::new(),
        usage: OptimizationRunUsage::default(),
        decisions,
        external_decisions,
        pass_manifests: Vec::new(),
        transformation_ledger: ledger,
        identity_bundle,
    })
}

#[derive(Debug)]
pub struct VerifiedPsiOptimizationSession {
    input: VerifiedPsiOptimizationInput,
    unit: PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationSession {
    /// Admit the runtime evidence and ranked cycles of the unit stage 03 built.
    /// Only the parked Psi rules read the admitted cycle components, so the
    /// session keeps the verdict and drops the components.
    pub(crate) fn admit(
        input: VerifiedPsiOptimizationInput,
        unit: PsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        validate_psi_context(&input, &unit)?;
        Ok(Self { input, unit })
    }

    pub const fn input(&self) -> &VerifiedPsiOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(self) -> (VerifiedPsiOptimizationInput, PsiOptimizationUnit) {
        (self.input, self.unit)
    }
}

#[derive(Debug)]
pub struct OptimizationRun {
    pub selections: OptimizationSelections,
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub session: VerifiedPsiOptimizationSession,
    pub commits: Vec<PsiOptimizationCommit>,
    pub validated_candidates: Vec<PsiValidatedCandidateDeclaration>,
    pub usage: OptimizationRunUsage,
    pub decisions: BaselineDecisionLog,
    pub external_decisions: ExternalDecisionLog,
    pub pass_manifests: Vec<OptimizationPassManifestRecord>,
    pub transformation_ledger: PsiTransformationLedger,
    pub identity_bundle: OptimizationIdentityBundle,
}

impl OptimizationRun {
    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn psi_selections(&self) -> &OptimizationSelections {
        &self.psi_selections
    }

    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn usage(&self) -> OptimizationRunUsage {
        self.usage
    }

    pub const fn decisions(&self) -> &BaselineDecisionLog {
        &self.decisions
    }

    pub fn pass_manifests(&self) -> &[OptimizationPassManifestRecord] {
        &self.pass_manifests
    }

    pub const fn transformation_ledger(&self) -> &PsiTransformationLedger {
        &self.transformation_ledger
    }

    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.identity_bundle
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationRunError {
    InitialValidation(OptimizationUnitValidationError),
    InvalidTransformationLedger(InvalidPsiTransformationLedger),
    DecisionLogReplay(BaselineDecisionLogDecodeError),
    ExternalDecisionSchema(ExternalDecisionSchemaError),
}

failed_display!(OptimizationRunError, "Psi optimization run failed");
