use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationIdentityBundle, OptimizationPassManifestRecord,
    OptimizationRuleSetIdentity, OptimizationSelectionIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationValidatorIdentity, TargetCostModelIdentity,
    TransformationLedgerIdentity,
};
use omega_optimization_policy::{BaselineDecisionLog, BaselineDecisionLogDecodeError};
use omega_optimization_unit::{
    InvalidPsiTransformationLedger, PsiOptimizationUnit, PsiTransformationLedger,
};
use omega_terminal_abstract_operations::TerminalAbstractOperationPlan;
use omega_terminal_psi_to_abstract_operations::VerifiedTerminalOptimizationInput;
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizationUnitValidationError, validate_transformed_psi_optimization_unit,
    validate_verified_psi_optimization_unit,
};

/// Validator-owned receipt for one optimized-unit to abstract-plan projection.
///
/// This is a custody identity, not the final native realization identity and
/// not a claim that the history-derived unit revision is a content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedOptimizedAbstractPlanProjection {
    terminal_psi: TerminalPsiIdentity,
    fuel_schedule: FuelScheduleIdentity,
    initial_unit: OptimizationUnitIdentity,
    final_unit: OptimizationUnitIdentity,
    selections: OptimizationSelectionIdentity,
    ledger: TransformationLedgerIdentity,
    bundle: omega_optimization_core::OptimizationIdentityBundleIdentity,
    validator: OptimizationValidatorIdentity,
}

impl ValidatedOptimizedAbstractPlanProjection {
    pub const fn terminal_psi(self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }

    pub const fn initial_unit(self) -> OptimizationUnitIdentity {
        self.initial_unit
    }

    pub const fn final_unit(self) -> OptimizationUnitIdentity {
        self.final_unit
    }

    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn ledger(self) -> TransformationLedgerIdentity {
        self.ledger
    }

    pub const fn bundle(self) -> omega_optimization_core::OptimizationIdentityBundleIdentity {
        self.bundle
    }

    pub const fn validator(self) -> OptimizationValidatorIdentity {
        self.validator
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAbstractPlanProjectionError {
    FinalUnit(OptimizationUnitValidationError),
    InitialUnitProjection,
    LedgerReplay(InvalidPsiTransformationLedger),
    LedgerTerminalMismatch,
    LedgerFuelMismatch,
    LedgerInitialMismatch,
    LedgerFinalMismatch,
    SelectionIdentityMismatch,
    RuleSetIdentityMismatch,
    CostModelIdentityMismatch,
    DecisionLogIdentityMismatch,
    DecisionLogReplay(BaselineDecisionLogDecodeError),
    WorkloadProfileNotSupported,
    LedgerIdentityMismatch,
    ManifestPresenceMismatch,
    ManifestCodecMismatch,
    ManifestRevisionMismatch,
    ManifestRuleSetMismatch,
    ManifestLedgerMismatch,
    SourceFunctionRosterMismatch,
    ImmutablePlanMetadataMismatch,
    ReconstructibleProjectionMismatch,
}

impl std::fmt::Display for OptimizedAbstractPlanProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized abstract-plan projection: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAbstractPlanProjectionError {}

#[allow(clippy::too_many_arguments)]
pub fn validate_optimized_abstract_plan_projection(
    input: &VerifiedTerminalOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    projected: &TerminalAbstractOperationPlan,
    selections: &OptimizationSelections,
    expected_rule_set: OptimizationRuleSetIdentity,
    expected_cost_model: TargetCostModelIdentity,
    decisions: &BaselineDecisionLog,
    pass_manifest: Option<&OptimizationPassManifestRecord>,
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
) -> Result<ValidatedOptimizedAbstractPlanProjection, OptimizedAbstractPlanProjectionError> {
    validate_transformed_psi_optimization_unit(input, final_unit)
        .map_err(OptimizedAbstractPlanProjectionError::FinalUnit)?;

    let initial = omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input.clone(),
        final_unit.fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractPlanProjectionError::InitialUnitProjection)?;
    validate_verified_psi_optimization_unit(&initial)
        .map_err(|_| OptimizedAbstractPlanProjectionError::InitialUnitProjection)?;
    let initial_identity = initial.unit().identity;

    let replayed_ledger = PsiTransformationLedger::new(
        ledger.terminal_psi(),
        ledger.fuel_schedule(),
        ledger.input(),
        ledger.output(),
        ledger.records().to_vec(),
    )
    .map_err(OptimizedAbstractPlanProjectionError::LedgerReplay)?;
    if &replayed_ledger != ledger {
        return Err(OptimizedAbstractPlanProjectionError::LedgerIdentityMismatch);
    }
    if ledger.terminal_psi() != input.plan().terminal_psi {
        return Err(OptimizedAbstractPlanProjectionError::LedgerTerminalMismatch);
    }
    if ledger.fuel_schedule() != final_unit.fuel_schedule {
        return Err(OptimizedAbstractPlanProjectionError::LedgerFuelMismatch);
    }
    if ledger.input() != initial_identity {
        return Err(OptimizedAbstractPlanProjectionError::LedgerInitialMismatch);
    }
    if ledger.output() != final_unit.identity {
        return Err(OptimizedAbstractPlanProjectionError::LedgerFinalMismatch);
    }

    if bundle.selections() != selections.identity() {
        return Err(OptimizedAbstractPlanProjectionError::SelectionIdentityMismatch);
    }
    if bundle.rule_set() != expected_rule_set {
        return Err(OptimizedAbstractPlanProjectionError::RuleSetIdentityMismatch);
    }
    if bundle.target_cost_model() != expected_cost_model {
        return Err(OptimizedAbstractPlanProjectionError::CostModelIdentityMismatch);
    }
    if bundle.decision_log() != Some(decisions.identity) {
        return Err(OptimizedAbstractPlanProjectionError::DecisionLogIdentityMismatch);
    }
    if bundle.workload_profile().is_some() {
        return Err(OptimizedAbstractPlanProjectionError::WorkloadProfileNotSupported);
    }
    if bundle.transformation_ledger() != ledger.identity() {
        return Err(OptimizedAbstractPlanProjectionError::LedgerIdentityMismatch);
    }
    if BaselineDecisionLog::decode(&decisions.encode())
        .map_err(OptimizedAbstractPlanProjectionError::DecisionLogReplay)?
        != *decisions
    {
        return Err(OptimizedAbstractPlanProjectionError::DecisionLogIdentityMismatch);
    }

    validate_manifest(pass_manifest, expected_rule_set, ledger)?;
    validate_projection_shape(input.plan(), final_unit, projected)?;

    Ok(ValidatedOptimizedAbstractPlanProjection {
        terminal_psi: final_unit.terminal_psi,
        fuel_schedule: final_unit.fuel_schedule,
        initial_unit: initial_identity,
        final_unit: final_unit.identity,
        selections: selections.identity(),
        ledger: ledger.identity(),
        bundle: bundle.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.optimized-abstract-plan-projection.v1",
        ),
    })
}

fn validate_manifest(
    manifest: Option<&OptimizationPassManifestRecord>,
    expected_rule_set: OptimizationRuleSetIdentity,
    ledger: &PsiTransformationLedger,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    let Some(manifest) = manifest else {
        if !ledger.records().is_empty()
            || expected_rule_set
                != OptimizationRuleSetIdentity::from_ordered_rules(&[])
                    .expect("empty rule set is canonical")
        {
            return Err(OptimizedAbstractPlanProjectionError::ManifestPresenceMismatch);
        }
        return Ok(());
    };
    if OptimizationPassManifestRecord::decode(&manifest.encode())
        .ok()
        .as_ref()
        != Some(manifest)
    {
        return Err(OptimizedAbstractPlanProjectionError::ManifestCodecMismatch);
    }
    if manifest.input() != ledger.input() || manifest.output() != ledger.output() {
        return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
    }
    if manifest.ordered_rule_set() != expected_rule_set {
        return Err(OptimizedAbstractPlanProjectionError::ManifestRuleSetMismatch);
    }
    let applied = manifest
        .decisions()
        .iter()
        .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
        .collect::<Vec<_>>();
    if applied.len() != ledger.records().len()
        || ledger.records().iter().any(|record| {
            !applied.iter().any(|decision| {
                decision.input() == record.input
                    && decision.candidate() == record.candidate
                    && decision.rule() == record.rule
                    && decision.validator() == Some(record.validator)
            })
        })
    {
        return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
    }
    Ok(())
}

fn validate_projection_shape(
    source: &TerminalAbstractOperationPlan,
    final_unit: &PsiOptimizationUnit,
    projected: &TerminalAbstractOperationPlan,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    if projected.terminal_psi != source.terminal_psi
        || projected.entry != final_unit.entry
        || projected.structural_types != source.structural_types
        || projected.boundary_machines != source.boundary_machines
        || projected.provider_candidates != source.provider_candidates
    {
        return Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch);
    }
    if source.functions.len() != final_unit.functions.len()
        || projected.functions.len() != final_unit.functions.len()
        || source
            .functions
            .iter()
            .map(|function| function.machine)
            .ne(final_unit.functions.iter().map(|function| function.machine))
        || projected
            .functions
            .iter()
            .map(|function| function.machine)
            .ne(final_unit.functions.iter().map(|function| function.machine))
    {
        return Err(OptimizedAbstractPlanProjectionError::SourceFunctionRosterMismatch);
    }
    for ((source_function, unit_function), projected_function) in source
        .functions
        .iter()
        .zip(&final_unit.functions)
        .zip(&projected.functions)
    {
        if projected_function.attachment != source_function.attachment
            || projected_function.structural_parameters != source_function.structural_parameters
            || projected_function.result != source_function.result
            || projected_function.entry_claims != source_function.entry_claims
            || projected_function.published_service_ceiling
                != source_function.published_service_ceiling
            || unit_function.structural_parameters != source_function.structural_parameters
            || unit_function.entry_claims
                != source_function
                    .entry_claims
                    .iter()
                    .map(|claim| claim.claim)
                    .collect()
        {
            return Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch);
        }
    }

    let reconstructed = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        projected,
        final_unit.fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch)?;
    if !same_reconstructible_projection(&reconstructed, final_unit) {
        return Err(OptimizedAbstractPlanProjectionError::ReconstructibleProjectionMismatch);
    }
    Ok(())
}

fn same_reconstructible_projection(
    reconstructed: &PsiOptimizationUnit,
    final_unit: &PsiOptimizationUnit,
) -> bool {
    reconstructed.terminal_psi == final_unit.terminal_psi
        && reconstructed.fuel_schedule == final_unit.fuel_schedule
        && reconstructed.entry == final_unit.entry
        && reconstructed.functions.len() == final_unit.functions.len()
        && reconstructed
            .functions
            .iter()
            .zip(&final_unit.functions)
            .all(|(left, right)| {
                left.machine == right.machine
                    && left.entry == right.entry
                    && left.parameters == right.parameters
                    && left.structural_parameters == right.structural_parameters
                    && left.declared_places == right.declared_places
                    && left.entry_claims == right.entry_claims
                    && left.facts == right.facts
                    && left.blocks.len() == right.blocks.len()
                    && left.blocks.iter().zip(&right.blocks).all(|(left, right)| {
                        left.id == right.id
                            && left.parameters == right.parameters
                            && left.nodes.len() == right.nodes.len()
                            && left.nodes.iter().zip(&right.nodes).all(|(left, right)| {
                                left.operation == right.operation
                                    && left.effect == right.effect
                                    && left.definitions == right.definitions
                                    && left.uses == right.uses
                                    && left.successors == right.successors
                                    && left.ownership == right.ownership
                            })
                    })
            })
}
