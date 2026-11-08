/*
We define, encode, decode, and render `PrePhysicalOptimizationManifest`, the
structured report of an abstract optimization run before target lowering. The
manifest identity binds the transformation ledger by its `TransformationLedgerIdentity`
while the binary artifact (`OMGPPM\0\0`) embeds the full encoded ledger so
decoding reconstructs and re-verifies the complete report without external tables.
*/

use std::fmt::Write;

use optimization_core::{
    BaselineDecisionLog, OptimizationCandidateVerdict, OptimizationFactReference,
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity, hex,
};
use semantic_vocabulary::WorkScheduleIdentity;
use terminal_psi::TerminalPsiIdentity;

use crate::{
    Cursor, ProvenanceDisposition, ProvenanceRewrite, PsiProvenance, PsiRealizationSite,
    PsiTransformationLedger,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationManifestStage {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrePhysicalOptimizationManifest {
    pub identity: PrePhysicalOptimizationManifestIdentity,
    pub stage: OptimizationManifestStage,
    pub physical_data: PhysicalOptimizationDataStatus,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub initial_unit: OptimizationUnitIdentity,
    pub final_unit: OptimizationUnitIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub selections: OptimizationSelections,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePhysicalOptimizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownPhysicalStatus(u8),
    UnsupportedVocabulary(u16),
    InvalidWorkSchedule,
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

optimization_core::invalid_error_display!(
    PrePhysicalOptimizationManifestDecodeError,
    "pre-physical manifest encoding"
);

optimization_core::take_error_from_adapter!(PrePhysicalOptimizationManifestDecodeError);

const PRE_PHYSICAL_MANIFEST_MAGIC: &[u8; 8] = b"OMGPPM\0\0";
const PRE_PHYSICAL_MANIFEST_VERSION: u32 = 6;

impl PrePhysicalOptimizationManifest {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_manifest_artifact_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(PRE_PHYSICAL_MANIFEST_MAGIC);
        crate::codec_bytes::push_encoded(&mut encoded, PRE_PHYSICAL_MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PrePhysicalOptimizationManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != PRE_PHYSICAL_MANIFEST_MAGIC {
            return Err(PrePhysicalOptimizationManifestDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
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
        let vocabulary = cursor.u16()?;
        let vocabulary_marker = terminal_psi::VocabularyMarker::new(vocabulary)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::UnsupportedVocabulary(vocabulary))?;
        let program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes(cursor.array()?);
        let work_schedule = WorkScheduleIdentity::new(cursor.u32()?)
            .ok_or(PrePhysicalOptimizationManifestDecodeError::InvalidWorkSchedule)?;
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
            work_schedule,
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
}

pub(super) fn pre_physical_manifest_identity(
    manifest: &PrePhysicalOptimizationManifest,
) -> PrePhysicalOptimizationManifestIdentity {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.pre-physical-optimization-manifest.v32\0");
    canonical.extend_from_slice(&encode_manifest_content(manifest));
    PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(&canonical)
}

fn encode_manifest_content(manifest: &PrePhysicalOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    encode_manifest_body_prefix(&mut canonical, manifest);
    canonical.extend_from_slice(&manifest.transformation_ledger.identity().bytes());
    encode_manifest_body_suffix(&mut canonical, manifest);
    canonical
}

fn encode_manifest_artifact_content(manifest: &PrePhysicalOptimizationManifest) -> Vec<u8> {
    let mut canonical = Vec::new();
    encode_manifest_body_prefix(&mut canonical, manifest);
    encode_bytes(&mut canonical, &manifest.transformation_ledger.encode());
    encode_manifest_body_suffix(&mut canonical, manifest);
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
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}

fn decode_statistics(
    cursor: &mut Cursor<'_>,
) -> Result<OptimizationStructuralStatistics, PrePhysicalOptimizationManifestDecodeError> {
    Ok(OptimizationStructuralStatistics {
        functions: cursor.u64()?,
        blocks: cursor.u64()?,
        nodes: cursor.u64()?,
        scalar_definitions: cursor.u64()?,
        scalar_uses: cursor.u64()?,
        optimization_facts: cursor.u64()?,
        ownership_frontier_facts: cursor.u64()?,
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

fn encode_manifest_body_prefix(
    canonical: &mut Vec<u8>,
    manifest: &PrePhysicalOptimizationManifest,
) {
    canonical.push(match manifest.stage {
        OptimizationManifestStage::PrePhysicalAbstractPlan => 1,
    });
    canonical.push(match manifest.physical_data {
        PhysicalOptimizationDataStatus::UnavailableBeforePhysicalRealization => 1,
    });
    crate::codec_bytes::push_encoded(
        canonical,
        manifest.psi.vocabulary_marker.get().to_le_bytes(),
    );
    canonical.extend_from_slice(manifest.psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(canonical, manifest.work_schedule.marker().to_le_bytes());
    canonical.extend_from_slice(&manifest.initial_unit.bytes());
    canonical.extend_from_slice(&manifest.final_unit.bytes());
    canonical.extend_from_slice(&manifest.projection.bytes());
    encode_bytes(canonical, &manifest.selections.encode());
    encode_bytes(canonical, &manifest.psi_selections.encode());
    canonical.extend_from_slice(&manifest.budget_per_pass.encode());
    canonical.extend_from_slice(&manifest.usage.encode());
    encode_bytes(canonical, &manifest.decision_log.encode());
    encode_len(canonical, manifest.pass_manifests.len());
    for pass in &manifest.pass_manifests {
        encode_bytes(canonical, &pass.encode());
    }
}

fn encode_manifest_body_suffix(
    canonical: &mut Vec<u8>,
    manifest: &PrePhysicalOptimizationManifest,
) {
    encode_bytes(canonical, &manifest.identity_bundle.encode());
    encode_statistics(canonical, manifest.source_statistics);
    encode_statistics(canonical, manifest.optimized_statistics);
}

impl PrePhysicalOptimizationManifest {
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
        let (realized, proven_unreachable, proven_unreachable_sources) = self
            .transformation_ledger
            .records()
            .iter()
            .flat_map(|record| &record.provenance)
            .fold((0, 0, 0), |counts, rewrite| match rewrite.disposition {
                ProvenanceDisposition::RealizedAt(_) => (counts.0 + 1, counts.1, counts.2),
                ProvenanceDisposition::ProvenUnreachableAt(_) => {
                    (counts.0, counts.1 + 1, counts.2 + rewrite.sources.len())
                }
            });
        writeln!(
            output,
            "provenance/work records: transformations={}, realized={}, proven-unreachable={}, proven-unreachable-sources={}",
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
    for work in &rewrite.work {
        match rewrite.disposition {
            ProvenanceDisposition::RealizedAt(_) => writeln!(
                output,
                "    source-scheduled-work: {} units={} runtime-charge={}",
                render_provenance(work.site),
                work.units,
                work.units,
            ),
            ProvenanceDisposition::ProvenUnreachableAt(_) => writeln!(
                output,
                "    source-scheduled-work: {} units={} runtime-charge=none reason=proven-unreachable",
                render_provenance(work.site),
                work.units,
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
