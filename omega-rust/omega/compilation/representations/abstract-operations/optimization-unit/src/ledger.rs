/*
We record the ordered revision history of an optimization run in
`PsiTransformationLedger` alongside the immutable pass and commit evidence in
`AbstractOptimizationEvidence`. Constructing or decoding a ledger validates that
every `PsiTransformationRecord` forms an unbroken `OptimizationUnitIdentity` chain,
carries unique candidate and pruned-machine ordinals, and settles non-zero logical
work matching its provenance sources before sealing `TransformationLedgerIdentity`.
*/

use std::collections::BTreeSet;

use optimization_core::{
    BaselineDecisionLog, ExternalDecisionLog, OptimizationCandidateIdentity,
    OptimizationIdentityBundle, OptimizationPassIdentity, OptimizationPassManifestRecord,
    OptimizationRuleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget, TransformationLedgerIdentity,
    invalid_error_display,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, WorkScheduleIdentity};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    Cursor, NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiCandidateDeclaration,
    PsiProvenance, PsiRealizationSite, WorkSettlement,
};

const LEDGER_MAGIC: &[u8] = b"omega.psi-transformation-ledger.v4\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPsiTransformationLedger {
    BrokenRevisionChain,
    FinalRevisionMismatch,
    DuplicateCandidate,
    EmptyProvenance,
    NonCanonicalProvenance,
    WorkProvenanceMismatch,
    ZeroWorkSettlement,
    NonCanonicalMachineRoster,
    DuplicatePrunedMachine,
    DuplicatePrunedSourceOrdinal,
}

invalid_error_display!(InvalidPsiTransformationLedger, "Psi transformation ledger");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiTransformationLedgerDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVocabulary(u16),
    InvalidWorkSchedule,
    InvalidSemanticIdentity,
    UnknownProvenanceTag(u8),
    UnknownRealizationSiteTag(u8),
    UnknownDispositionTag(u8),
    LengthOverflow,
    TrailingBytes,
    InvalidLedger(InvalidPsiTransformationLedger),
}

invalid_error_display!(
    PsiTransformationLedgerDecodeError,
    "Psi transformation-ledger encoding"
);

optimization_core::take_error_from_adapter!(PsiTransformationLedgerDecodeError);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrunedMachineCustody {
    pub machine: MachineId,
    pub source_ordinal: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiTransformationRecord {
    pub rule: OptimizationRuleIdentity,
    pub candidate: OptimizationCandidateIdentity,
    pub validator: OptimizationValidatorIdentity,
    pub input: OptimizationUnitIdentity,
    pub output: OptimizationUnitIdentity,
    pub pruned_machines: Vec<PrunedMachineCustody>,
    pub provenance: Vec<ProvenanceRewrite>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiTransformationLedger {
    pub(super) identity: TransformationLedgerIdentity,
    pub(super) psi: TerminalPsiIdentity,
    pub(super) work_schedule: WorkScheduleIdentity,
    pub(super) input: OptimizationUnitIdentity,
    pub(super) output: OptimizationUnitIdentity,
    pub(super) records: Vec<PsiTransformationRecord>,
}
impl PsiTransformationLedger {
    pub const fn identity(&self) -> TransformationLedgerIdentity {
        self.identity
    }
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }
    pub const fn work_schedule(&self) -> WorkScheduleIdentity {
        self.work_schedule
    }
    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }
    pub const fn output(&self) -> OptimizationUnitIdentity {
        self.output
    }
    pub fn records(&self) -> &[PsiTransformationRecord] {
        &self.records
    }
}

impl PsiTransformationLedger {
    pub fn new(
        psi: TerminalPsiIdentity,
        work_schedule: WorkScheduleIdentity,
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        records: Vec<PsiTransformationRecord>,
    ) -> Result<Self, InvalidPsiTransformationLedger> {
        let mut revision = input;
        let mut candidates = BTreeSet::new();
        let mut pruned_machines = BTreeSet::new();
        let mut pruned_source_ordinals = BTreeSet::new();
        for record in &records {
            if record.input != revision || record.input == record.output {
                return Err(InvalidPsiTransformationLedger::BrokenRevisionChain);
            }
            if !candidates.insert(record.candidate) {
                return Err(InvalidPsiTransformationLedger::DuplicateCandidate);
            }
            validate_provenance(&record.provenance)?;
            if record
                .pruned_machines
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(InvalidPsiTransformationLedger::NonCanonicalMachineRoster);
            }
            for custody in &record.pruned_machines {
                if !pruned_machines.insert(custody.machine) {
                    return Err(InvalidPsiTransformationLedger::DuplicatePrunedMachine);
                }
                if !pruned_source_ordinals.insert(custody.source_ordinal) {
                    return Err(InvalidPsiTransformationLedger::DuplicatePrunedSourceOrdinal);
                }
            }
            revision = record.output;
        }
        if revision != output {
            return Err(InvalidPsiTransformationLedger::FinalRevisionMismatch);
        }
        let identity = TransformationLedgerIdentity::from_canonical_bytes(&encode_ledger(
            psi,
            work_schedule,
            input,
            output,
            &records,
        ));
        Ok(Self {
            identity,
            psi,
            work_schedule,
            input,
            output,
            records,
        })
    }
}

pub(super) fn validate_provenance(
    rows: &[ProvenanceRewrite],
) -> Result<(), InvalidPsiTransformationLedger> {
    if rows.is_empty() || rows.iter().any(|row| row.sources.is_empty()) {
        return Err(InvalidPsiTransformationLedger::EmptyProvenance);
    }
    if rows
        .windows(2)
        .any(|pair| pair[0].canonical_sort_key() >= pair[1].canonical_sort_key())
    {
        return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
    }
    for group in rows.chunk_by(|left, right| left.input == right.input) {
        if group.len() > 1
            && (group.iter().any(|row| !row.disposition.is_realized())
                || group
                    .iter()
                    .skip(1)
                    .any(|row| row.sources != group[0].sources || row.work != group[0].work))
        {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
    }
    for row in rows {
        let sources = row.sources.iter().copied().collect::<BTreeSet<_>>();
        if sources.len() != row.sources.len() {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
        if row.input.machine() != row.disposition.site().machine() {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
        let work = row
            .work
            .iter()
            .map(|settlement| settlement.site)
            .collect::<BTreeSet<_>>();
        if work.len() != row.work.len() || work != sources {
            return Err(InvalidPsiTransformationLedger::WorkProvenanceMismatch);
        }
        if row.work.iter().any(|settlement| settlement.units == 0) {
            return Err(InvalidPsiTransformationLedger::ZeroWorkSettlement);
        }
    }
    Ok(())
}

impl PsiTransformationLedger {
    pub fn encode(&self) -> Vec<u8> {
        encode_ledger(
            self.psi(),
            self.work_schedule(),
            self.input(),
            self.output(),
            self.records(),
        )
    }
}
pub(super) fn encode_ledger(
    psi: TerminalPsiIdentity,
    work_schedule: WorkScheduleIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    records: &[PsiTransformationRecord],
) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(LEDGER_MAGIC);
    crate::codec_bytes::push_encoded(&mut encoded, psi.vocabulary_marker.get().to_le_bytes());
    encoded.extend_from_slice(psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut encoded, work_schedule.marker().to_le_bytes());
    encoded.extend_from_slice(&input.bytes());
    encoded.extend_from_slice(&output.bytes());
    encode_len(&mut encoded, records.len());
    for record in records {
        encoded.extend_from_slice(&record.rule.bytes());
        encoded.extend_from_slice(&record.candidate.bytes());
        encoded.extend_from_slice(&record.validator.bytes());
        encoded.extend_from_slice(&record.input.bytes());
        encoded.extend_from_slice(&record.output.bytes());
        encode_len(&mut encoded, record.pruned_machines.len());
        for custody in &record.pruned_machines {
            crate::codec_bytes::push_encoded(&mut encoded, custody.machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(&mut encoded, custody.source_ordinal.to_le_bytes());
        }
        encode_len(&mut encoded, record.provenance.len());
        for row in &record.provenance {
            encode_realization_site(&mut encoded, row.input);
            let (tag, site) = match row.disposition {
                ProvenanceDisposition::RealizedAt(site)
                | ProvenanceDisposition::ProvenUnreachableAt(site) => {
                    (row.disposition.canonical_tag(), site)
                }
            };
            encoded.push(tag);
            encode_realization_site(&mut encoded, site);
            encode_len(&mut encoded, row.sources.len());
            for source in &row.sources {
                encode_provenance(&mut encoded, *source);
            }
            encode_len(&mut encoded, row.work.len());
            for settlement in &row.work {
                encode_provenance(&mut encoded, settlement.site);
                crate::codec_bytes::push_encoded(&mut encoded, settlement.units.to_le_bytes());
            }
        }
    }
    encoded
}
fn encode_provenance(encoded: &mut Vec<u8>, provenance: PsiProvenance) {
    match provenance {
        PsiProvenance::Operation(operation) => {
            encoded.push(1);
            crate::codec_bytes::push_encoded(encoded, operation.get().to_le_bytes());
        }
        PsiProvenance::Edge(edge) => {
            encoded.push(2);
            crate::codec_bytes::push_encoded(encoded, edge.get().to_le_bytes());
        }
    }
}
fn encode_realization_site(encoded: &mut Vec<u8>, site: PsiRealizationSite) {
    match site {
        PsiRealizationSite::Node(location) => {
            encoded.push(1);
            crate::codec_bytes::push_encoded(encoded, location.machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(encoded, location.block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(encoded, location.node.to_le_bytes());
        }
        PsiRealizationSite::Edge { machine, edge } => {
            encoded.push(2);
            crate::codec_bytes::push_encoded(encoded, machine.get().to_le_bytes());
            crate::codec_bytes::push_encoded(encoded, edge.get().to_le_bytes());
        }
    }
}
fn encode_len(encoded: &mut Vec<u8>, length: usize) {
    encoded.extend_from_slice(
        &u64::try_from(length)
            .expect("Psi transformation ledger length fits u64")
            .to_le_bytes(),
    );
}

impl PsiTransformationLedger {
    pub fn decode(encoded: &[u8]) -> Result<Self, PsiTransformationLedgerDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(LEDGER_MAGIC.len())? != LEDGER_MAGIC {
            return Err(PsiTransformationLedgerDecodeError::WrongMagic);
        }
        let vocabulary = cursor.u16()?;
        let vocabulary_marker = VocabularyMarker::new(vocabulary).ok_or(
            PsiTransformationLedgerDecodeError::UnsupportedVocabulary(vocabulary),
        )?;
        let program_fingerprint = SemanticFingerprint::from_bytes(cursor.array()?);
        let work_schedule = WorkScheduleIdentity::new(cursor.u32()?)
            .ok_or(PsiTransformationLedgerDecodeError::InvalidWorkSchedule)?;
        let input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let output = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let record_count = cursor.length()?;
        let mut records = Vec::with_capacity(record_count.min(cursor.remaining()));
        for _ in 0..record_count {
            let rule = OptimizationRuleIdentity::from_bytes(cursor.array()?);
            let candidate = OptimizationCandidateIdentity::from_bytes(cursor.array()?);
            let validator = OptimizationValidatorIdentity::from_bytes(cursor.array()?);
            let record_input = OptimizationUnitIdentity::from_bytes(cursor.array()?);
            let record_output = OptimizationUnitIdentity::from_bytes(cursor.array()?);
            let pruned_count = cursor.length()?;
            let mut pruned_machines = Vec::with_capacity(pruned_count.min(cursor.remaining()));
            for _ in 0..pruned_count {
                let machine = semantic_id(cursor.u64()?, MachineId::new)?;
                let source_ordinal = cursor.u32()?;
                pruned_machines.push(PrunedMachineCustody {
                    machine,
                    source_ordinal,
                });
            }
            let provenance_count = cursor.length()?;
            let mut provenance = Vec::with_capacity(provenance_count.min(cursor.remaining()));
            for _ in 0..provenance_count {
                let input = decode_realization_site(&mut cursor)?;
                let disposition_tag = cursor.byte()?;
                let site = decode_realization_site(&mut cursor)?;
                let disposition = match disposition_tag {
                    1 => ProvenanceDisposition::RealizedAt(site),
                    2 => ProvenanceDisposition::ProvenUnreachableAt(site),
                    tag => {
                        return Err(PsiTransformationLedgerDecodeError::UnknownDispositionTag(
                            tag,
                        ));
                    }
                };
                let source_count = cursor.length()?;
                let mut sources = Vec::with_capacity(source_count.min(cursor.remaining()));
                for _ in 0..source_count {
                    sources.push(decode_provenance(&mut cursor)?);
                }
                let work_count = cursor.length()?;
                let mut work = Vec::with_capacity(work_count.min(cursor.remaining()));
                for _ in 0..work_count {
                    work.push(WorkSettlement {
                        site: decode_provenance(&mut cursor)?,
                        units: cursor.u64()?,
                    });
                }
                provenance.push(ProvenanceRewrite {
                    input,
                    disposition,
                    sources,
                    work,
                });
            }
            records.push(PsiTransformationRecord {
                rule,
                candidate,
                validator,
                input: record_input,
                output: record_output,
                pruned_machines,
                provenance,
            });
        }
        if cursor.remaining() != 0 {
            return Err(PsiTransformationLedgerDecodeError::TrailingBytes);
        }
        Self::new(
            TerminalPsiIdentity {
                vocabulary_marker,
                program_fingerprint,
            },
            work_schedule,
            input,
            output,
            records,
        )
        .map_err(PsiTransformationLedgerDecodeError::InvalidLedger)
    }
}
fn decode_realization_site(
    cursor: &mut Cursor<'_>,
) -> Result<PsiRealizationSite, PsiTransformationLedgerDecodeError> {
    let tag = cursor.byte()?;
    let machine = semantic_id(cursor.u64()?, MachineId::new)?;
    match tag {
        1 => Ok(PsiRealizationSite::Node(NodeLocation {
            machine,
            block: semantic_id(cursor.u64()?, BlockId::new)?,
            node: cursor.u32()?,
        })),
        2 => Ok(PsiRealizationSite::Edge {
            machine,
            edge: semantic_id(cursor.u64()?, EdgeId::new)?,
        }),
        tag => Err(PsiTransformationLedgerDecodeError::UnknownRealizationSiteTag(tag)),
    }
}
fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<PsiProvenance, PsiTransformationLedgerDecodeError> {
    match cursor.byte()? {
        1 => Ok(PsiProvenance::Operation(semantic_id(
            cursor.u64()?,
            OperationId::new,
        )?)),
        2 => Ok(PsiProvenance::Edge(semantic_id(
            cursor.u64()?,
            EdgeId::new,
        )?)),
        tag => Err(PsiTransformationLedgerDecodeError::UnknownProvenanceTag(
            tag,
        )),
    }
}
fn semantic_id<T>(
    raw: u64,
    constructor: impl FnOnce(u64) -> Option<T>,
) -> Result<T, PsiTransformationLedgerDecodeError> {
    constructor(raw).ok_or(PsiTransformationLedgerDecodeError::InvalidSemanticIdentity)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationCommit {
    pub rule: OptimizationRuleIdentity,
    pub candidate: OptimizationCandidateIdentity,
    pub validator: OptimizationValidatorIdentity,
    pub input: OptimizationUnitIdentity,
    pub output: OptimizationUnitIdentity,
    pub predicted_cost_delta: i64,
    pub pruned_machines: Vec<crate::PrunedMachineCustody>,
    pub provenance: Vec<ProvenanceRewrite>,
    pub declaration: PsiCandidateDeclaration,
}
impl PsiOptimizationCommit {
    pub const fn declaration(&self) -> &PsiCandidateDeclaration {
        &self.declaration
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiValidatedCandidateDeclaration {
    pub pass: OptimizationPassIdentity,
    pub declaration: PsiCandidateDeclaration,
    pub validator: OptimizationValidatorIdentity,
}
impl PsiValidatedCandidateDeclaration {
    pub const fn pass(&self) -> OptimizationPassIdentity {
        self.pass
    }
    pub const fn declaration(&self) -> &PsiCandidateDeclaration {
        &self.declaration
    }
    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }
}
pub type OptimizationRunUsage = optimization_core::OptimizationWorkUsage;
#[derive(Debug, Clone)]
pub struct AbstractOptimizationEvidence {
    pub selections: OptimizationSelections,
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub commits: Vec<PsiOptimizationCommit>,
    pub validated_candidates: Vec<PsiValidatedCandidateDeclaration>,
    pub usage: OptimizationRunUsage,
    pub decisions: BaselineDecisionLog,
    pub external_decisions: ExternalDecisionLog,
    pub pass_manifests: Vec<OptimizationPassManifestRecord>,
    pub transformation_ledger: PsiTransformationLedger,
    pub identity_bundle: OptimizationIdentityBundle,
}
