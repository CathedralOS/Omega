use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperationPlan;
use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationExecutionPhase, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationSelections, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, TargetCostModelIdentity, TransformationLedgerIdentity,
};
use omega_optimization_policy::{BaselineDecisionLog, BaselineDecisionLogDecodeError};
use omega_optimization_unit::{
    InvalidPsiTransformationLedger, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput;
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizationUnitValidationError, validate_transformed_psi_optimization_unit,
    validate_verified_psi_optimization_unit,
};

/// Validator-owned receipt for one optimized-unit to abstract-plan projection.
///
/// This is a custody identity, not the final native realization identity. The
/// final unit is independently identified by its canonical content; the
/// transformation ledger separately retains the accepted rewrite history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedOptimizedAbstractPlanProjection {
    psi: TerminalPsiIdentity,
    fuel_schedule: FuelScheduleIdentity,
    initial_unit: OptimizationUnitIdentity,
    final_unit: OptimizationUnitIdentity,
    /// Complete source-visible suite requested by the root build.
    selections: OptimizationSelectionIdentity,
    /// Exact selection subset whose Psi passes this receipt validates.
    psi_selections: OptimizationSelectionIdentity,
    ledger: TransformationLedgerIdentity,
    bundle: omega_optimization_core::OptimizationIdentityBundleIdentity,
    validator: OptimizationValidatorIdentity,
}

impl ValidatedOptimizedAbstractPlanProjection {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
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

    pub const fn psi_selections(self) -> OptimizationSelectionIdentity {
        self.psi_selections
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

    /// Domain-separated custody identity of every independently validated
    /// source, revision, selection, ledger, bundle, and validator field.
    /// This is suitable for downstream joins but grants no physical-emission
    /// or publication authority.
    pub fn identity(self) -> OptimizedAbstractPlanProjectionIdentity {
        let mut canonical = Vec::with_capacity(272);
        canonical.extend_from_slice(&self.psi.vocabulary_marker.get().to_le_bytes());
        canonical.extend_from_slice(self.psi.program_fingerprint.as_bytes());
        canonical.extend_from_slice(&self.fuel_schedule.marker().to_le_bytes());
        canonical.extend_from_slice(&self.initial_unit.bytes());
        canonical.extend_from_slice(&self.final_unit.bytes());
        canonical.extend_from_slice(&self.selections.bytes());
        canonical.extend_from_slice(&self.psi_selections.bytes());
        canonical.extend_from_slice(&self.ledger.bytes());
        canonical.extend_from_slice(&self.bundle.bytes());
        canonical.extend_from_slice(&self.validator.bytes());
        OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(&canonical)
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
    PsiSelectionProjectionMismatch,
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
    SourceCustodyMismatch,
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
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    projected: &AbstractOperationPlan,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    expected_rule_set: OptimizationRuleSetIdentity,
    expected_cost_model: TargetCostModelIdentity,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
) -> Result<ValidatedOptimizedAbstractPlanProjection, OptimizedAbstractPlanProjectionError> {
    validate_transformed_psi_optimization_unit(input, final_unit)
        .map_err(OptimizedAbstractPlanProjectionError::FinalUnit)?;

    let initial = omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input.clone(),
        final_unit.fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractPlanProjectionError::InitialUnitProjection)?;
    validate_verified_psi_optimization_unit(&initial)
        .map_err(|_| OptimizedAbstractPlanProjectionError::InitialUnitProjection)?;
    let initial_identity = initial.unit().identity;

    let replayed_ledger = PsiTransformationLedger::new(
        ledger.psi(),
        ledger.fuel_schedule(),
        ledger.input(),
        ledger.output(),
        ledger.records().to_vec(),
    )
    .map_err(OptimizedAbstractPlanProjectionError::LedgerReplay)?;
    if &replayed_ledger != ledger {
        return Err(OptimizedAbstractPlanProjectionError::LedgerIdentityMismatch);
    }
    if ledger.psi() != input.plan().psi {
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
    validate_source_custody(initial.unit(), final_unit, ledger)?;

    if bundle.selections() != selections.identity() {
        return Err(OptimizedAbstractPlanProjectionError::SelectionIdentityMismatch);
    }
    if *psi_selections != selections.for_phase(OptimizationExecutionPhase::Psi) {
        return Err(OptimizedAbstractPlanProjectionError::PsiSelectionProjectionMismatch);
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

    validate_manifests(pass_manifests, expected_rule_set, ledger)?;
    validate_projection_shape(input.plan(), final_unit, projected)?;

    Ok(ValidatedOptimizedAbstractPlanProjection {
        psi: final_unit.psi,
        fuel_schedule: final_unit.fuel_schedule,
        initial_unit: initial_identity,
        final_unit: final_unit.identity,
        selections: selections.identity(),
        psi_selections: psi_selections.identity(),
        ledger: ledger.identity(),
        bundle: bundle.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.optimized-abstract-plan-projection.v32",
        ),
    })
}

fn source_occurrence_map(
    unit: &PsiOptimizationUnit,
) -> Option<BTreeMap<(psi_core::MachineId, PsiProvenance), BTreeMap<PsiRealizationSite, u64>>> {
    let mut result = BTreeMap::<_, BTreeMap<_, _>>::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_site = PsiRealizationSite::Node(omega_optimization_unit::NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).ok()?,
                });
                for settlement in &node.fuel {
                    if result
                        .entry((function.machine, settlement.site))
                        .or_default()
                        .insert(node_site, settlement.units)
                        .is_some()
                    {
                        return None;
                    }
                }
                for edge in &node.successors {
                    let edge_site = PsiRealizationSite::Edge {
                        machine: function.machine,
                        edge: edge.psi_edge,
                    };
                    for settlement in &edge.fuel {
                        if result
                            .entry((function.machine, settlement.site))
                            .or_default()
                            .insert(edge_site, settlement.units)
                            .is_some()
                        {
                            return None;
                        }
                    }
                }
            }
        }
    }
    Some(result)
}

fn validate_source_custody(
    initial: &PsiOptimizationUnit,
    final_unit: &PsiOptimizationUnit,
    ledger: &PsiTransformationLedger,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    let mut ledger_pruned = ledger
        .records()
        .iter()
        .flat_map(|record| record.pruned_machines.iter().copied())
        .collect::<Vec<_>>();
    ledger_pruned.sort_unstable();
    if ledger_pruned != final_unit.pruned_machines {
        return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
    }
    let mut current = source_occurrence_map(initial)
        .ok_or(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)?;
    let final_sources = source_occurrence_map(final_unit)
        .ok_or(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)?;
    for record in ledger.records() {
        let mut by_input = BTreeMap::<PsiRealizationSite, Vec<_>>::new();
        for row in &record.provenance {
            by_input.entry(row.input).or_default().push(row);
        }
        for (input_site, rows) in &by_input {
            let machine = input_site.machine();
            let expected = rows[0]
                .fuel
                .iter()
                .map(|settlement| (settlement.site, settlement.units))
                .collect::<BTreeMap<_, _>>();
            if rows.iter().any(|row| {
                row.input != *input_site
                    || row.sources.iter().copied().collect::<BTreeSet<_>>()
                        != expected.keys().copied().collect()
                    || row
                        .fuel
                        .iter()
                        .map(|settlement| (settlement.site, settlement.units))
                        .collect::<BTreeMap<_, _>>()
                        != expected
            }) || expected.iter().any(|(source, units)| {
                current
                    .get(&(machine, *source))
                    .and_then(|occurrences| occurrences.get(input_site))
                    != Some(units)
            }) {
                return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
            }
            for source in expected.keys() {
                let occurrences = current
                    .get_mut(&(machine, *source))
                    .expect("input occurrence was checked");
                occurrences.remove(input_site);
                if occurrences.is_empty() {
                    current.remove(&(machine, *source));
                }
            }
        }
        for row in &record.provenance {
            let ProvenanceDisposition::RealizedAt(output_site) = row.disposition else {
                continue;
            };
            let machine = output_site.machine();
            for settlement in &row.fuel {
                if current
                    .entry((machine, settlement.site))
                    .or_default()
                    .insert(output_site, settlement.units)
                    .is_some()
                {
                    return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
                }
            }
        }
    }
    if current != final_sources {
        return Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch);
    }
    Ok(())
}

pub(super) fn validate_manifests(
    manifests: &[OptimizationPassManifestRecord],
    expected_rule_set: OptimizationRuleSetIdentity,
    ledger: &PsiTransformationLedger,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    let flattened_rules = manifests
        .iter()
        .flat_map(|manifest| manifest.ordered_rules().iter().copied())
        .collect::<Vec<_>>();
    let flattened_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&flattened_rules)
        .map_err(|_| OptimizedAbstractPlanProjectionError::ManifestRuleSetMismatch)?;
    if flattened_rule_set != expected_rule_set {
        return Err(OptimizedAbstractPlanProjectionError::ManifestRuleSetMismatch);
    }
    if manifests.is_empty() && (!ledger.records().is_empty() || !flattened_rules.is_empty()) {
        return Err(OptimizedAbstractPlanProjectionError::ManifestPresenceMismatch);
    }
    let mut revision = ledger.input();
    let mut ledger_index = 0usize;
    for manifest in manifests {
        if OptimizationPassManifestRecord::decode(&manifest.encode())
            .ok()
            .as_ref()
            != Some(manifest)
        {
            return Err(OptimizedAbstractPlanProjectionError::ManifestCodecMismatch);
        }
        if manifest.input() != revision {
            return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
        }
        let decisions = manifest.decisions();
        let mut decision_index = 0usize;
        while decision_index < decisions.len() {
            let input = decisions[decision_index].input();
            if input != revision {
                return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
            }
            let group_end = decisions[decision_index..]
                .iter()
                .position(|decision| decision.input() != input)
                .map_or(decisions.len(), |offset| decision_index + offset);
            let applied = decisions[decision_index..group_end]
                .iter()
                .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
                .collect::<Vec<_>>();
            if applied.len() > 1 {
                return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
            }
            if let Some(decision) = applied.first() {
                let record = ledger
                    .records()
                    .get(ledger_index)
                    .ok_or(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch)?;
                if decision.input() != record.input
                    || decision.candidate() != record.candidate
                    || decision.rule() != record.rule
                    || decision.validator() != Some(record.validator)
                {
                    return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
                }
                revision = record.output;
                ledger_index += 1;
            }
            decision_index = group_end;
        }
        if manifest.output() != revision {
            return Err(OptimizedAbstractPlanProjectionError::ManifestRevisionMismatch);
        }
    }
    if revision != ledger.output() || ledger_index != ledger.records().len() {
        return Err(OptimizedAbstractPlanProjectionError::ManifestLedgerMismatch);
    }
    Ok(())
}

fn validate_projection_shape(
    source: &AbstractOperationPlan,
    final_unit: &PsiOptimizationUnit,
    projected: &AbstractOperationPlan,
) -> Result<(), OptimizedAbstractPlanProjectionError> {
    if projected.psi != source.psi
        || final_unit.psi != source.psi
        || final_unit.entry != source.entry
        || projected.entry != final_unit.entry
        || final_unit.structural_types != source.structural_types
        || projected.structural_types != source.structural_types
        || final_unit.boundary_machines != source.boundary_machines
        || projected.boundary_machines != source.boundary_machines
        || final_unit.provider_candidates != source.provider_candidates
        || projected.provider_candidates != source.provider_candidates
    {
        return Err(OptimizedAbstractPlanProjectionError::ImmutablePlanMetadataMismatch);
    }
    if !source_function_roster_partition_is_exact(source, final_unit)
        || projected.functions.len() != final_unit.functions.len()
        || projected
            .functions
            .iter()
            .map(|function| function.machine)
            .ne(final_unit.functions.iter().map(|function| function.machine))
    {
        return Err(OptimizedAbstractPlanProjectionError::SourceFunctionRosterMismatch);
    }
    for (unit_function, projected_function) in final_unit.functions.iter().zip(&projected.functions)
    {
        let Some(source_function) = source
            .functions
            .iter()
            .find(|source| source.machine == unit_function.machine)
        else {
            return Err(OptimizedAbstractPlanProjectionError::SourceFunctionRosterMismatch);
        };
        if projected_function.attachment != source_function.attachment
            || unit_function.attachment != source_function.attachment
            || projected_function.structural_parameters != source_function.structural_parameters
            || unit_function.structural_parameters != source_function.structural_parameters
            || projected_function.result != source_function.result
            || unit_function.result != source_function.result
            || projected_function.entry_claims != source_function.entry_claims
            || unit_function.entry_claim_declarations != source_function.entry_claims
            || projected_function.published_service_ceiling
                != source_function.published_service_ceiling
            || unit_function.published_service_ceiling != source_function.published_service_ceiling
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

fn source_function_roster_partition_is_exact(
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    let active = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    let pruned = unit
        .pruned_machines
        .iter()
        .map(|row| (row.source_ordinal, row.machine))
        .collect::<BTreeMap<_, _>>();
    if unit
        .pruned_machines
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || active.len() != unit.functions.len()
        || active.len() + pruned.len() != source.functions.len()
    {
        return false;
    }
    let mut active_order = unit.functions.iter().map(|function| function.machine);
    for (ordinal, source_function) in source.functions.iter().enumerate() {
        if active.contains(&source_function.machine) {
            if active_order.next() != Some(source_function.machine) {
                return false;
            }
        } else if u32::try_from(ordinal)
            .ok()
            .and_then(|ordinal| pruned.get(&ordinal).copied())
            != Some(source_function.machine)
        {
            return false;
        }
    }
    active_order.next().is_none()
}

fn same_reconstructible_projection(
    reconstructed: &PsiOptimizationUnit,
    final_unit: &PsiOptimizationUnit,
) -> bool {
    reconstructed.psi == final_unit.psi
        && reconstructed.fuel_schedule == final_unit.fuel_schedule
        && reconstructed.entry == final_unit.entry
        && reconstructed.structural_types == final_unit.structural_types
        && reconstructed.boundary_machines == final_unit.boundary_machines
        && reconstructed.provider_candidates == final_unit.provider_candidates
        && reconstructed.functions.len() == final_unit.functions.len()
        && reconstructed
            .functions
            .iter()
            .zip(&final_unit.functions)
            .all(|(left, right)| {
                left.machine == right.machine
                    && left.attachment == right.attachment
                    && left.entry == right.entry
                    && left.parameters == right.parameters
                    && left.structural_parameters == right.structural_parameters
                    && left.result == right.result
                    && left.declared_places == right.declared_places
                    && left.entry_claim_declarations == right.entry_claim_declarations
                    && left.entry_claims == right.entry_claims
                    && left.published_service_ceiling == right.published_service_ceiling
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
                                    && left.successors.len() == right.successors.len()
                                    && left.successors.iter().zip(&right.successors).all(
                                        |(left, right)| {
                                            left.psi_edge == right.psi_edge
                                                && left.target == right.target
                                                && left.bindings == right.bindings
                                        },
                                    )
                                    && left.ownership == right.ownership
                            })
                    })
            })
}

#[cfg(test)]
mod tests {
    use omega_abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
        AbstractOperationPlan,
    };
    use omega_optimization_core::{
        OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationValidatorIdentity,
    };
    use omega_optimization_unit::{
        FuelSettlement, NodeLocation, ProvenanceRewrite, PsiTransformationRecord,
        reconstruct_psi_optimization_unit_seed,
    };
    use psi_core::{BlockId, EdgeId, MachineId};
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    use super::*;

    fn receipt() -> ValidatedOptimizedAbstractPlanProjection {
        ValidatedOptimizedAbstractPlanProjection {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(2).unwrap(),
            initial_unit: OptimizationUnitIdentity::from_canonical_bytes(b"initial"),
            final_unit: OptimizationUnitIdentity::from_canonical_bytes(b"final"),
            selections: OptimizationSelectionIdentity::from_bytes([3; 32]),
            psi_selections: OptimizationSelectionIdentity::from_bytes([4; 32]),
            ledger: TransformationLedgerIdentity::from_canonical_bytes(b"ledger"),
            bundle:
                omega_optimization_core::OptimizationIdentityBundleIdentity::from_canonical_bytes(
                    b"bundle",
                ),
            validator: OptimizationValidatorIdentity::from_canonical_bytes(b"validator"),
        }
    }

    fn custody_unit() -> PsiOptimizationUnit {
        let machine = MachineId::new(41).unwrap();
        let block = BlockId::new(42).unwrap();
        reconstruct_psi_optimization_unit_seed(
            &AbstractOperationPlan {
                psi: TerminalPsiIdentity {
                    vocabulary_marker: VocabularyMarker::CURRENT,
                    program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
                },
                entry: machine,
                structural_types: Vec::new(),
                boundary_machines: Vec::new(),
                provider_candidates: Vec::new(),
                functions: vec![AbstractFunction {
                    machine,
                    attachment: None,
                    entry: block,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    result: AbstractFunctionResult::Unit,
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    block_entries: vec![AbstractBlockEntry {
                        block,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    }],
                    operations: vec![AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(44).unwrap(),
                        cleanup_actions: Vec::new(),
                    }],
                }],
            },
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap()
    }

    fn custody_record(
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        disposition: ProvenanceDisposition,
        source: PsiProvenance,
    ) -> omega_optimization_unit::PsiTransformationRecord {
        PsiTransformationRecord {
            rule: OptimizationRuleIdentity::from_canonical_bytes(b"custody-rule"),
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(&output.bytes()),
            validator: OptimizationValidatorIdentity::from_canonical_bytes(b"custody-validator"),
            input,
            output,
            pruned_machines: Vec::new(),
            provenance: vec![ProvenanceRewrite {
                input: disposition.site(),
                disposition,
                sources: vec![source],
                fuel: vec![FuelSettlement {
                    site: source,
                    units: 1,
                }],
            }],
        }
    }

    #[test]
    fn projection_identity_binds_every_validated_custody_field() {
        let baseline = receipt();
        let changed = [
            ValidatedOptimizedAbstractPlanProjection {
                psi: TerminalPsiIdentity {
                    program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
                    ..baseline.psi
                },
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                fuel_schedule: FuelScheduleIdentity::new(9).unwrap(),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                initial_unit: OptimizationUnitIdentity::from_canonical_bytes(b"initial-drift"),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                final_unit: OptimizationUnitIdentity::from_canonical_bytes(b"final-drift"),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                selections: OptimizationSelectionIdentity::from_bytes([9; 32]),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                psi_selections: OptimizationSelectionIdentity::from_bytes([9; 32]),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                ledger: TransformationLedgerIdentity::from_canonical_bytes(b"ledger-drift"),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                bundle: omega_optimization_core::OptimizationIdentityBundleIdentity::from_canonical_bytes(
                    b"bundle-drift",
                ),
                ..baseline
            },
            ValidatedOptimizedAbstractPlanProjection {
                validator: OptimizationValidatorIdentity::from_canonical_bytes(b"validator-drift"),
                ..baseline
            },
        ];

        assert_eq!(baseline.identity(), receipt().identity());
        for corrupted in changed {
            assert_ne!(baseline.identity(), corrupted.identity());
        }
    }

    #[test]
    fn source_custody_is_an_exact_final_or_unreachable_partition() {
        let initial = custody_unit();
        let location = NodeLocation {
            machine: initial.functions[0].machine,
            block: initial.functions[0].blocks[0].id,
            node: 0,
        };
        let source = initial.functions[0].blocks[0].nodes[0].provenance[0];
        let mut final_unit = initial.clone();
        final_unit.functions[0].blocks[0].nodes.clear();
        final_unit.identity =
            omega_optimization_unit::recompute_psi_optimization_unit_identity(&final_unit);
        let record = custody_record(
            initial.identity,
            final_unit.identity,
            ProvenanceDisposition::ProvenUnreachableAt(PsiRealizationSite::Node(location)),
            source,
        );
        let ledger = PsiTransformationLedger::new(
            initial.psi,
            initial.fuel_schedule,
            initial.identity,
            final_unit.identity,
            vec![record.clone()],
        )
        .unwrap();
        validate_source_custody(&initial, &final_unit, &ledger).unwrap();

        assert_eq!(
            validate_source_custody(&initial, &initial, &ledger),
            Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)
        );

        let mut wrong_units = record;
        wrong_units.provenance[0].fuel[0].units = 2;
        let wrong_ledger = PsiTransformationLedger::new(
            initial.psi,
            initial.fuel_schedule,
            initial.identity,
            final_unit.identity,
            vec![wrong_units],
        )
        .unwrap();
        assert_eq!(
            validate_source_custody(&initial, &final_unit, &wrong_ledger),
            Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)
        );
    }

    #[test]
    fn source_custody_rejects_resurrection_after_unreachability() {
        let initial = custody_unit();
        let location = NodeLocation {
            machine: initial.functions[0].machine,
            block: initial.functions[0].blocks[0].id,
            node: 0,
        };
        let source = initial.functions[0].blocks[0].nodes[0].provenance[0];
        let mut final_unit = initial.clone();
        final_unit.functions[0].blocks[0].nodes.clear();
        final_unit.identity =
            omega_optimization_unit::recompute_psi_optimization_unit_identity(&final_unit);
        let middle = OptimizationUnitIdentity::from_canonical_bytes(b"custody-middle");
        let removed = custody_record(
            initial.identity,
            middle,
            ProvenanceDisposition::ProvenUnreachableAt(PsiRealizationSite::Node(location)),
            source,
        );
        let resurrected = custody_record(
            middle,
            final_unit.identity,
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
            source,
        );
        let ledger = PsiTransformationLedger::new(
            initial.psi,
            initial.fuel_schedule,
            initial.identity,
            final_unit.identity,
            vec![removed, resurrected],
        )
        .unwrap();
        assert_eq!(
            validate_source_custody(&initial, &final_unit, &ledger),
            Err(OptimizedAbstractPlanProjectionError::SourceCustodyMismatch)
        );
    }
}
