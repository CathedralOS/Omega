use std::fmt::Write;

use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationExecutionPhase, OptimizationFactReference,
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_optimization_policy::BaselineDecisionLog;
use omega_optimization_unit::{
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput;
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::ValidatedOptimizedAbstractPlanProjection;

const PRE_PHYSICAL_MANIFEST_MAGIC: &[u8; 8] = b"OMGPPM\0\0";
const PRE_PHYSICAL_MANIFEST_VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationManifestStage {
    /// Abstract-plan projection is independently validated. Target selection,
    /// allocation, frame/spill accounting, emission, and publication are not.
    PrePhysicalAbstractPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalOptimizationDataStatus {
    UnavailableBeforePhysicalRealization,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizationStructuralStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub nodes: u64,
    pub scalar_definitions: u64,
    pub scalar_uses: u64,
    pub optimization_facts: u64,
    pub ownership_frontier_facts: u64,
}

/// Structured, non-publication manifest for the largest independently
/// validated optimizer state available before target/physical realization.
///
/// Public fields make the record serializable and testable, but do not grant
/// authority. Downstream custody accepts only the validated wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrePhysicalOptimizationManifest {
    pub identity: PrePhysicalOptimizationManifestIdentity,
    pub stage: OptimizationManifestStage,
    pub physical_data: PhysicalOptimizationDataStatus,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub initial_unit: OptimizationUnitIdentity,
    pub final_unit: OptimizationUnitIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    /// Complete source-visible suite requested by the root build.
    pub selections: OptimizationSelections,
    /// Exact selection subset executed and validated in this Psi-stage record.
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub decision_log: BaselineDecisionLog,
    pub pass_manifests: Vec<OptimizationPassManifestRecord>,
    pub transformation_ledger: PsiTransformationLedger,
    pub identity_bundle: OptimizationIdentityBundle,
    pub source_statistics: OptimizationStructuralStatistics,
    pub optimized_statistics: OptimizationStructuralStatistics,
}

impl PrePhysicalOptimizationManifest {
    pub fn recomputed_identity(&self) -> PrePhysicalOptimizationManifestIdentity {
        pre_physical_manifest_identity(self)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_artifact_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(PRE_PHYSICAL_MANIFEST_MAGIC);
        encoded.extend_from_slice(&PRE_PHYSICAL_MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PrePhysicalOptimizationManifestDecodeError> {
        let mut cursor = ManifestCursor::new(encoded);
        if cursor.take(8)? != PRE_PHYSICAL_MANIFEST_MAGIC {
            return Err(PrePhysicalOptimizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != PRE_PHYSICAL_MANIFEST_VERSION {
            return Err(PrePhysicalOptimizationManifestDecodeError::UnsupportedVersion(version));
        }
        let identity = PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? {
            1 => OptimizationManifestStage::PrePhysicalAbstractPlan,
            tag => {
                return Err(PrePhysicalOptimizationManifestDecodeError::UnknownStage(
                    tag,
                ));
            }
        };
        let physical_data = match cursor.byte()? {
            1 => PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
            tag => {
                return Err(PrePhysicalOptimizationManifestDecodeError::UnknownPhysicalStatus(tag));
            }
        };
        let vocabulary = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = psi_terminal::VocabularyMarker::new(vocabulary)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::UnsupportedVocabulary(vocabulary))?;
        let program_fingerprint = psi_terminal::SemanticFingerprint::from_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(u32::from_le_bytes(cursor.array()?))
            .ok_or(PrePhysicalOptimizationManifestDecodeError::InvalidFuelSchedule)?;
        let initial_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let final_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let projection = OptimizedAbstractPlanProjectionIdentity::from_bytes(cursor.array()?);
        let selections = OptimizationSelections::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidSelections)?;
        let psi_selections = OptimizationSelections::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidSelections)?;
        let budget_per_pass = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidWorkBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidWorkUsage)?;
        let decision_log = BaselineDecisionLog::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidDecisionLog)?;
        let pass_count = cursor.length()?;
        let mut pass_manifests = Vec::with_capacity(pass_count.min(cursor.remaining()));
        for _ in 0..pass_count {
            pass_manifests.push(
                OptimizationPassManifestRecord::decode(cursor.length_prefixed()?)
                    .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidPassManifest)?,
            );
        }
        let transformation_ledger = PsiTransformationLedger::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidTransformationLedger)?;
        let identity_bundle = OptimizationIdentityBundle::decode(cursor.length_prefixed()?)
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::InvalidIdentityBundle)?;
        let source_statistics = decode_statistics(&mut cursor)?;
        let optimized_statistics = decode_statistics(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(PrePhysicalOptimizationManifestDecodeError::TrailingBytes);
        }
        let manifest = Self {
            identity,
            stage,
            physical_data,
            psi: TerminalPsiIdentity {
                vocabulary_marker,
                program_fingerprint,
            },
            fuel_schedule,
            initial_unit,
            final_unit,
            projection,
            selections,
            psi_selections,
            budget_per_pass,
            usage,
            decision_log,
            pass_manifests,
            transformation_ledger,
            identity_bundle,
            source_statistics,
            optimized_statistics,
        };
        if manifest.identity != manifest.recomputed_identity() {
            return Err(PrePhysicalOptimizationManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }

    /// Deterministic human projection. Rendering is deliberately downstream of
    /// the structured record and cannot affect optimization decisions or bytes.
    pub fn render_text(&self) -> String {
        let mut output = String::new();
        writeln!(output, "Omega pre-physical optimization manifest").unwrap();
        writeln!(output, "stage: pre-physical abstract plan").unwrap();
        writeln!(
            output,
            "physical data: unavailable before physical realization"
        )
        .unwrap();
        writeln!(output, "manifest identity: {}", hex(&self.identity.bytes())).unwrap();
        writeln!(
            output,
            "source Terminal Psi: {}",
            hex(self.psi.program_fingerprint.as_bytes())
        )
        .unwrap();
        writeln!(output, "initial unit: {}", hex(&self.initial_unit.bytes())).unwrap();
        writeln!(output, "final unit: {}", hex(&self.final_unit.bytes())).unwrap();
        writeln!(output, "projection: {}", hex(&self.projection.bytes())).unwrap();
        writeln!(
            output,
            "rule set: {}",
            hex(&self.identity_bundle.rule_set().bytes())
        )
        .unwrap();
        writeln!(
            output,
            "target-neutral cost model: {}",
            hex(&self.identity_bundle.target_cost_model().bytes())
        )
        .unwrap();
        writeln!(
            output,
            "decision log: {}",
            self.identity_bundle
                .decision_log()
                .map(|identity| hex(&identity.bytes()))
                .unwrap_or_else(|| "absent".into())
        )
        .unwrap();
        writeln!(
            output,
            "transformation ledger: {}",
            hex(&self.transformation_ledger.identity().bytes())
        )
        .unwrap();
        let selected = self
            .selections
            .as_slice()
            .iter()
            .map(|selection| selection.build_case_name())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "requested selections: {selected}").unwrap();
        let psi_selected = self
            .psi_selections
            .as_slice()
            .iter()
            .map(|selection| selection.build_case_name())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "completed Psi selections: {psi_selected}").unwrap();
        writeln!(output, "passes: {}", self.pass_manifests.len()).unwrap();
        for (pass_index, pass) in self.pass_manifests.iter().enumerate() {
            writeln!(
                output,
                "pass[{pass_index}]: identity={}, input={}, output={}, rules={}, decisions={}",
                hex(&pass.pass().bytes()),
                hex(&pass.input().bytes()),
                hex(&pass.output().bytes()),
                pass.ordered_rules().len(),
                pass.decisions().len(),
            )
            .unwrap();
            for (rule_index, rule) in pass.ordered_rules().iter().enumerate() {
                writeln!(output, "  rule[{rule_index}]: {}", hex(&rule.bytes())).unwrap();
            }
            for (decision_index, decision) in pass.decisions().iter().enumerate() {
                writeln!(
                    output,
                    "  decision[{decision_index}]: candidate={}, rule={}, verdict={:?}, validator={}, facts={}",
                    hex(&decision.candidate().bytes()),
                    hex(&decision.rule().bytes()),
                    decision.verdict(),
                    decision
                        .validator()
                        .map(|identity| hex(&identity.bytes()))
                        .unwrap_or_else(|| "absent".into()),
                    decision.consumed_facts().len(),
                )
                .unwrap();
                for fact in decision.consumed_facts() {
                    writeln!(output, "    fact: {}", render_fact(*fact)).unwrap();
                }
            }
        }
        let (applied, skipped, rejected) = decision_counts(&self.pass_manifests);
        writeln!(
            output,
            "candidate verdicts: applied={applied}, skipped={skipped}, rejected={rejected}"
        )
        .unwrap();
        writeln!(
            output,
            "work usage: rules={}, candidates={}, validations={}, commits={}, iterations={}",
            self.usage.rule_evaluations,
            self.usage.candidates,
            self.usage.validation_steps,
            self.usage.commits,
            self.usage.iterations,
        )
        .unwrap();
        writeln!(
            output,
            "source structure: functions={}, blocks={}, nodes={}",
            self.source_statistics.functions,
            self.source_statistics.blocks,
            self.source_statistics.nodes,
        )
        .unwrap();
        writeln!(
            output,
            "optimized structure: functions={}, blocks={}, nodes={}",
            self.optimized_statistics.functions,
            self.optimized_statistics.blocks,
            self.optimized_statistics.nodes,
        )
        .unwrap();
        let realized = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|rewrite| rewrite.disposition.is_realized())
            .count();
        let proven_unreachable = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|rewrite| !rewrite.disposition.is_realized())
            .count();
        let proven_unreachable_sources = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .filter(|rewrite| !rewrite.disposition.is_realized())
            .map(|rewrite| rewrite.sources.len())
            .sum::<usize>();
        writeln!(
            output,
            "provenance/fuel records: transformations={}, realized={}, proven-unreachable={}, proven-unreachable-sources={}",
            self.transformation_ledger.records().len(),
            realized,
            proven_unreachable,
            proven_unreachable_sources,
        )
        .unwrap();
        for (record_index, record) in self.transformation_ledger.records().iter().enumerate() {
            writeln!(
                output,
                "ledger[{record_index}]: candidate={}, rule={}, input={}, output={}",
                hex(&record.candidate.bytes()),
                hex(&record.rule.bytes()),
                hex(&record.input.bytes()),
                hex(&record.output.bytes()),
            )
            .unwrap();
            for rewrite in &record.provenance {
                render_provenance_rewrite(&mut output, rewrite);
            }
        }
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPrePhysicalOptimizationManifest {
    record: PrePhysicalOptimizationManifest,
}

impl ValidatedPrePhysicalOptimizationManifest {
    pub const fn record(&self) -> &PrePhysicalOptimizationManifest {
        &self.record
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePhysicalOptimizationManifestError {
    InitialUnitProjection,
    StructuralStatisticsOverflow,
    ProjectionMismatch,
    SelectionMismatch,
    DecisionLogMismatch,
    LedgerMismatch,
    PassManifestCodecMismatch,
    PassRevisionMismatch,
    WorkUsageOverflow,
    WorkUsageMismatch,
    WorkBudgetExceeded,
    ContentMismatch,
}

impl std::fmt::Display for PrePhysicalOptimizationManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-physical optimization manifest: {self:?}"
        )
    }
}

impl std::error::Error for PrePhysicalOptimizationManifestError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePhysicalOptimizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownPhysicalStatus(u8),
    UnsupportedVocabulary(u16),
    InvalidFuelSchedule,
    LengthOverflow,
    InvalidSelections,
    InvalidWorkBudget,
    InvalidWorkUsage,
    InvalidDecisionLog,
    InvalidPassManifest,
    InvalidTransformationLedger,
    InvalidIdentityBundle,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for PrePhysicalOptimizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-physical manifest encoding: {self:?}"
        )
    }
}

impl std::error::Error for PrePhysicalOptimizationManifestDecodeError {}

#[allow(clippy::too_many_arguments)]
pub fn project_pre_physical_optimization_manifest(
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<ValidatedPrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
    let mut record = expected_record(
        input,
        final_unit,
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )?;
    record.identity = record.recomputed_identity();
    validate_pre_physical_optimization_manifest(
        &record,
        input,
        final_unit,
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn validate_pre_physical_optimization_manifest(
    candidate: &PrePhysicalOptimizationManifest,
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<ValidatedPrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
    validate_joins(
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )?;
    let mut expected = expected_record(
        input,
        final_unit,
        selections,
        psi_selections,
        budget_per_pass,
        usage,
        decisions,
        pass_manifests,
        ledger,
        bundle,
        projection,
    )?;
    expected.identity = expected.recomputed_identity();
    if candidate != &expected || candidate.identity != candidate.recomputed_identity() {
        return Err(PrePhysicalOptimizationManifestError::ContentMismatch);
    }
    Ok(ValidatedPrePhysicalOptimizationManifest {
        record: candidate.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
fn expected_record(
    input: &VerifiedPsiOptimizationInput,
    final_unit: &PsiOptimizationUnit,
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<PrePhysicalOptimizationManifest, PrePhysicalOptimizationManifestError> {
    let initial = omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input.clone(),
        final_unit.fuel_schedule,
    )
    .map_err(|_| PrePhysicalOptimizationManifestError::InitialUnitProjection)?;
    Ok(PrePhysicalOptimizationManifest {
        identity: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: OptimizationManifestStage::PrePhysicalAbstractPlan,
        physical_data: PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization,
        psi: final_unit.psi,
        fuel_schedule: final_unit.fuel_schedule,
        initial_unit: initial.unit().identity,
        final_unit: final_unit.identity,
        projection: projection.identity(),
        selections: selections.clone(),
        psi_selections: psi_selections.clone(),
        budget_per_pass,
        usage,
        decision_log: decisions.clone(),
        pass_manifests: pass_manifests.to_vec(),
        transformation_ledger: ledger.clone(),
        identity_bundle: bundle,
        source_statistics: structural_statistics(initial.unit())?,
        optimized_statistics: structural_statistics(final_unit)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_joins(
    selections: &OptimizationSelections,
    psi_selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    usage: OptimizationWorkUsage,
    decisions: &BaselineDecisionLog,
    pass_manifests: &[OptimizationPassManifestRecord],
    ledger: &PsiTransformationLedger,
    bundle: OptimizationIdentityBundle,
    projection: ValidatedOptimizedAbstractPlanProjection,
) -> Result<(), PrePhysicalOptimizationManifestError> {
    if bundle.selections() != selections.identity()
        || projection.selections() != selections.identity()
    {
        return Err(PrePhysicalOptimizationManifestError::SelectionMismatch);
    }
    if *psi_selections != selections.for_phase(OptimizationExecutionPhase::Psi)
        || projection.psi_selections() != psi_selections.identity()
    {
        return Err(PrePhysicalOptimizationManifestError::SelectionMismatch);
    }
    if bundle.decision_log() != Some(decisions.identity) {
        return Err(PrePhysicalOptimizationManifestError::DecisionLogMismatch);
    }
    if BaselineDecisionLog::decode(&decisions.encode())
        .ok()
        .as_ref()
        != Some(decisions)
    {
        return Err(PrePhysicalOptimizationManifestError::DecisionLogMismatch);
    }
    if bundle.transformation_ledger() != ledger.identity()
        || projection.ledger() != ledger.identity()
        || projection.bundle() != bundle.identity()
        || projection.initial_unit() != ledger.input()
        || projection.final_unit() != ledger.output()
        || projection.psi() != ledger.psi()
        || projection.fuel_schedule() != ledger.fuel_schedule()
    {
        return Err(PrePhysicalOptimizationManifestError::LedgerMismatch);
    }
    super::projection::validate_manifests(pass_manifests, bundle.rule_set(), ledger)
        .map_err(|_| PrePhysicalOptimizationManifestError::PassRevisionMismatch)?;
    let mut revision = ledger.input();
    let mut aggregate = OptimizationWorkUsage::default();
    for pass in pass_manifests {
        if OptimizationPassManifestRecord::decode(&pass.encode())
            .ok()
            .as_ref()
            != Some(pass)
        {
            return Err(PrePhysicalOptimizationManifestError::PassManifestCodecMismatch);
        }
        if pass.input() != revision {
            return Err(PrePhysicalOptimizationManifestError::PassRevisionMismatch);
        }
        revision = pass.output();
        if !pass.work_usage().within(budget_per_pass) {
            return Err(PrePhysicalOptimizationManifestError::WorkBudgetExceeded);
        }
        aggregate = add_usage(aggregate, pass.work_usage())?;
    }
    if revision != ledger.output() {
        return Err(PrePhysicalOptimizationManifestError::PassRevisionMismatch);
    }
    if aggregate != usage {
        return Err(PrePhysicalOptimizationManifestError::WorkUsageMismatch);
    }
    Ok(())
}

fn add_usage(
    left: OptimizationWorkUsage,
    right: OptimizationWorkUsage,
) -> Result<OptimizationWorkUsage, PrePhysicalOptimizationManifestError> {
    Ok(OptimizationWorkUsage {
        rule_evaluations: left
            .rule_evaluations
            .checked_add(right.rule_evaluations)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        candidates: left
            .candidates
            .checked_add(right.candidates)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        validation_steps: left
            .validation_steps
            .checked_add(right.validation_steps)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        commits: left
            .commits
            .checked_add(right.commits)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
        iterations: left
            .iterations
            .checked_add(right.iterations)
            .ok_or(PrePhysicalOptimizationManifestError::WorkUsageOverflow)?,
    })
}

fn structural_statistics(
    unit: &PsiOptimizationUnit,
) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestError> {
    let count = |value: usize| {
        u64::try_from(value)
            .map_err(|_| PrePhysicalOptimizationManifestError::StructuralStatisticsOverflow)
    };
    Ok(OptimizationStructuralStatistics {
        functions: count(unit.functions.len())?,
        blocks: count(
            unit.functions
                .iter()
                .map(|function| function.blocks.len())
                .sum(),
        )?,
        nodes: count(
            unit.functions
                .iter()
                .flat_map(|function| &function.blocks)
                .map(|block| block.nodes.len())
                .sum(),
        )?,
        scalar_definitions: count(
            unit.functions
                .iter()
                .map(|function| {
                    function.parameters.len()
                        + function
                            .blocks
                            .iter()
                            .map(|block| {
                                block.parameters.len()
                                    + block
                                        .nodes
                                        .iter()
                                        .map(|node| node.definitions.len())
                                        .sum::<usize>()
                            })
                            .sum::<usize>()
                })
                .sum(),
        )?,
        scalar_uses: count(
            unit.functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.nodes)
                .map(|node| node.uses.len())
                .sum(),
        )?,
        optimization_facts: count(
            unit.functions
                .iter()
                .map(|function| function.facts.len())
                .sum(),
        )?,
        ownership_frontier_facts: count(unit.ownership_frontier_facts.len())?,
    })
}

fn pre_physical_manifest_identity(
    manifest: &PrePhysicalOptimizationManifest,
) -> PrePhysicalOptimizationManifestIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.pre-physical-optimization-manifest.v28\0");
    canonical.extend_from_slice(&encode_manifest_content(manifest));
    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(&canonical)
}

fn encode_manifest_content(manifest: &PrePhysicalOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        OptimizationManifestStage::PrePhysicalAbstractPlan => 1,
    });
    canonical.push(match manifest.physical_data {
        PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization => 1,
    });
    canonical.extend_from_slice(&manifest.psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(manifest.psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&manifest.fuel_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&manifest.initial_unit.bytes());
    canonical.extend_from_slice(&manifest.final_unit.bytes());
    canonical.extend_from_slice(&manifest.projection.bytes());
    encode_bytes(&mut canonical, &manifest.selections.encode());
    encode_bytes(&mut canonical, &manifest.psi_selections.encode());
    canonical.extend_from_slice(&manifest.budget_per_pass.encode());
    canonical.extend_from_slice(&manifest.usage.encode());
    encode_bytes(&mut canonical, &manifest.decision_log.encode());
    encode_len(&mut canonical, manifest.pass_manifests.len());
    for pass in &manifest.pass_manifests {
        encode_bytes(&mut canonical, &pass.encode());
    }
    canonical.extend_from_slice(&manifest.transformation_ledger.identity().bytes());
    encode_bytes(&mut canonical, &manifest.identity_bundle.encode());
    encode_statistics(&mut canonical, manifest.source_statistics);
    encode_statistics(&mut canonical, manifest.optimized_statistics);
    canonical
}

/// Artifact encoding contains the ledger itself so decoding can reconstruct the
/// complete report. The manifest identity intentionally continues to bind the
/// ledger by its semantic identity, keeping artifact framing independent from
/// the optimization result's established identity domain.
fn encode_manifest_artifact_content(manifest: &PrePhysicalOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        OptimizationManifestStage::PrePhysicalAbstractPlan => 1,
    });
    canonical.push(match manifest.physical_data {
        PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization => 1,
    });
    canonical.extend_from_slice(&manifest.psi.vocabulary_marker.get().to_le_bytes());
    canonical.extend_from_slice(manifest.psi.program_fingerprint.as_bytes());
    canonical.extend_from_slice(&manifest.fuel_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&manifest.initial_unit.bytes());
    canonical.extend_from_slice(&manifest.final_unit.bytes());
    canonical.extend_from_slice(&manifest.projection.bytes());
    encode_bytes(&mut canonical, &manifest.selections.encode());
    encode_bytes(&mut canonical, &manifest.psi_selections.encode());
    canonical.extend_from_slice(&manifest.budget_per_pass.encode());
    canonical.extend_from_slice(&manifest.usage.encode());
    encode_bytes(&mut canonical, &manifest.decision_log.encode());
    encode_len(&mut canonical, manifest.pass_manifests.len());
    for pass in &manifest.pass_manifests {
        encode_bytes(&mut canonical, &pass.encode());
    }
    encode_bytes(&mut canonical, &manifest.transformation_ledger.encode());
    encode_bytes(&mut canonical, &manifest.identity_bundle.encode());
    encode_statistics(&mut canonical, manifest.source_statistics);
    encode_statistics(&mut canonical, manifest.optimized_statistics);
    canonical
}

fn encode_statistics(bytes: &mut Vec<u8>, statistics: OptimizationStructuralStatistics) {
    for value in [
        statistics.functions,
        statistics.blocks,
        statistics.nodes,
        statistics.scalar_definitions,
        statistics.scalar_uses,
        statistics.optimization_facts,
        statistics.ownership_frontier_facts,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn decode_statistics(
    cursor: &mut ManifestCursor<'_>,
) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestDecodeError> {
    Ok(OptimizationStructuralStatistics {
        functions: u64::from_le_bytes(cursor.array()?),
        blocks: u64::from_le_bytes(cursor.array()?),
        nodes: u64::from_le_bytes(cursor.array()?),
        scalar_definitions: u64::from_le_bytes(cursor.array()?),
        scalar_uses: u64::from_le_bytes(cursor.array()?),
        optimization_facts: u64::from_le_bytes(cursor.array()?),
        ownership_frontier_facts: u64::from_le_bytes(cursor.array()?),
    })
}

fn encode_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value);
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("canonical manifest length fits u64")
            .to_le_bytes(),
    );
}

fn decision_counts(manifests: &[OptimizationPassManifestRecord]) -> (usize, usize, usize) {
    manifests
        .iter()
        .flat_map(OptimizationPassManifestRecord::decisions)
        .fold(
            (0, 0, 0),
            |(applied, skipped, rejected), decision| match decision.verdict() {
                OptimizationCandidateVerdict::Applied => (applied + 1, skipped, rejected),
                OptimizationCandidateVerdict::Skipped(_) => (applied, skipped + 1, rejected),
                OptimizationCandidateVerdict::Rejected(_) => (applied, skipped, rejected + 1),
            },
        )
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

struct ManifestCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> ManifestCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], PrePhysicalOptimizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PrePhysicalOptimizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, PrePhysicalOptimizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, PrePhysicalOptimizationManifestDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PrePhysicalOptimizationManifestDecodeError::LengthOverflow)
    }

    fn length_prefixed(
        &mut self,
    ) -> Result<&'encoded [u8], PrePhysicalOptimizationManifestDecodeError> {
        let length = self.length()?;
        self.take(length)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}

fn render_fact(fact: OptimizationFactReference) -> String {
    match fact {
        OptimizationFactReference::ScalarConstant(identity) => {
            format!("scalar-constant:{}", hex(&identity.bytes()))
        }
        OptimizationFactReference::AcceptedObligation(identity) => {
            format!("accepted-obligation:{}", hex(&identity.bytes()))
        }
        OptimizationFactReference::OwnershipFrontier(identity) => {
            format!("ownership-frontier:{}", hex(&identity.bytes()))
        }
        OptimizationFactReference::ValueRange(identity) => {
            format!("value-range:{}", hex(&identity.bytes()))
        }
    }
}

fn render_provenance(provenance: PsiProvenance) -> String {
    match provenance {
        PsiProvenance::Operation(operation) => format!("operation:{}", operation.get()),
        PsiProvenance::Edge(edge) => format!("edge:{}", edge.get()),
    }
}

fn render_provenance_rewrite(output: &mut String, rewrite: &ProvenanceRewrite) {
    let (label, site) = match rewrite.disposition {
        ProvenanceDisposition::RealizedAt(site) => ("realized-at", site),
        ProvenanceDisposition::ProvenUnreachableAt(site) => ("proven-unreachable-at", site),
    };
    writeln!(
        output,
        "  input: {}",
        render_realization_site(rewrite.input)
    )
    .unwrap();
    writeln!(output, "  {label}: {}", render_realization_site(site)).unwrap();
    for source in &rewrite.sources {
        writeln!(output, "    source: {}", render_provenance(*source)).unwrap();
    }
    for fuel in &rewrite.fuel {
        match rewrite.disposition {
            ProvenanceDisposition::RealizedAt(_) => writeln!(
                output,
                "    source-scheduled-fuel: {} units={} runtime-charge={}",
                render_provenance(fuel.site),
                fuel.units,
                fuel.units,
            ),
            ProvenanceDisposition::ProvenUnreachableAt(_) => writeln!(
                output,
                "    source-scheduled-fuel: {} units={} runtime-charge=none reason=proven-unreachable",
                render_provenance(fuel.site),
                fuel.units,
            ),
        }
        .unwrap();
    }
}

fn render_realization_site(site: PsiRealizationSite) -> String {
    match site {
        PsiRealizationSite::Node(location) => format!(
            "node:machine={},block={},node={}",
            location.machine.get(),
            location.block.get(),
            location.node
        ),
        PsiRealizationSite::Edge { machine, edge } => {
            format!("edge:machine={},edge={}", machine.get(), edge.get())
        }
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_unit::{FuelSettlement, NodeLocation};
    use psi_core::{BlockId, EdgeId, MachineId};

    use super::*;

    #[test]
    fn human_projection_names_ownership_frontier_facts_explicitly() {
        let identity = omega_optimization_core::OwnershipFrontierFactIdentity::from_canonical_bytes(
            b"ownership-render-test",
        );
        assert_eq!(
            render_fact(OptimizationFactReference::OwnershipFrontier(identity)),
            format!("ownership-frontier:{}", hex(&identity.bytes()))
        );
    }

    #[test]
    fn human_projection_names_value_range_facts_explicitly() {
        let identity = omega_optimization_core::ValueRangeFactIdentity::from_canonical_bytes(
            b"value-range-render-test",
        );
        assert_eq!(
            render_fact(OptimizationFactReference::ValueRange(identity)),
            format!("value-range:{}", hex(&identity.bytes()))
        );
    }

    #[test]
    fn human_projection_distinguishes_charged_and_unreachable_source_fuel() {
        let location = NodeLocation {
            machine: MachineId::new(1).unwrap(),
            block: BlockId::new(2).unwrap(),
            node: 3,
        };
        let site = PsiRealizationSite::Node(location);
        let source = PsiProvenance::Edge(EdgeId::new(4).unwrap());
        let render = |disposition| {
            let mut text = String::new();
            render_provenance_rewrite(
                &mut text,
                &ProvenanceRewrite {
                    input: site,
                    disposition,
                    sources: vec![source],
                    fuel: vec![FuelSettlement {
                        site: source,
                        units: 1,
                    }],
                },
            );
            text
        };
        let realized = render(ProvenanceDisposition::RealizedAt(site));
        assert!(realized.contains("realized-at: node:machine=1,block=2,node=3"));
        assert!(realized.contains("runtime-charge=1"));
        let unreachable = render(ProvenanceDisposition::ProvenUnreachableAt(site));
        assert!(unreachable.contains("proven-unreachable-at: node:machine=1,block=2,node=3"));
        assert!(unreachable.contains("runtime-charge=none reason=proven-unreachable"));
    }
}
