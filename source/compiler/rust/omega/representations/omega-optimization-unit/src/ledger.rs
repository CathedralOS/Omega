use std::collections::BTreeSet;

use omega_optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, TransformationLedgerIdentity,
};
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{FuelSettlement, NodeLocation, ProvenanceRewrite, PsiProvenance};

const LEDGER_MAGIC: &[u8] = b"omega.psi-transformation-ledger.v1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiTransformationRecord {
    pub rule: OptimizationRuleIdentity,
    pub candidate: OptimizationCandidateIdentity,
    pub validator: OptimizationValidatorIdentity,
    pub input: OptimizationUnitIdentity,
    pub output: OptimizationUnitIdentity,
    pub provenance: Vec<ProvenanceRewrite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiTransformationLedger {
    identity: TransformationLedgerIdentity,
    terminal_psi: TerminalPsiIdentity,
    fuel_schedule: FuelScheduleIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    records: Vec<PsiTransformationRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPsiTransformationLedger {
    BrokenRevisionChain,
    FinalRevisionMismatch,
    DuplicateCandidate,
    EmptyProvenance,
    NonCanonicalProvenance,
    FuelProvenanceMismatch,
}

impl std::fmt::Display for InvalidPsiTransformationLedger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi transformation ledger: {self:?}")
    }
}

impl std::error::Error for InvalidPsiTransformationLedger {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiTransformationLedgerDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVocabulary(u16),
    InvalidFuelSchedule,
    InvalidSemanticIdentity,
    UnknownProvenanceTag(u8),
    LengthOverflow,
    TrailingBytes,
    InvalidLedger(InvalidPsiTransformationLedger),
}

impl std::fmt::Display for PsiTransformationLedgerDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Psi transformation-ledger encoding: {self:?}"
        )
    }
}

impl std::error::Error for PsiTransformationLedgerDecodeError {}

impl PsiTransformationLedger {
    pub fn new(
        terminal_psi: TerminalPsiIdentity,
        fuel_schedule: FuelScheduleIdentity,
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        records: Vec<PsiTransformationRecord>,
    ) -> Result<Self, InvalidPsiTransformationLedger> {
        let mut revision = input;
        let mut candidates = BTreeSet::new();
        for record in &records {
            if record.input != revision || record.input == record.output {
                return Err(InvalidPsiTransformationLedger::BrokenRevisionChain);
            }
            if !candidates.insert(record.candidate) {
                return Err(InvalidPsiTransformationLedger::DuplicateCandidate);
            }
            validate_provenance(&record.provenance)?;
            revision = record.output;
        }
        if revision != output {
            return Err(InvalidPsiTransformationLedger::FinalRevisionMismatch);
        }
        let identity = TransformationLedgerIdentity::from_canonical_bytes(&encode_ledger(
            terminal_psi,
            fuel_schedule,
            input,
            output,
            &records,
        ));
        Ok(Self {
            identity,
            terminal_psi,
            fuel_schedule,
            input,
            output,
            records,
        })
    }

    pub const fn identity(&self) -> TransformationLedgerIdentity {
        self.identity
    }

    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
        self.fuel_schedule
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

    pub fn encode(&self) -> Vec<u8> {
        encode_ledger(
            self.terminal_psi,
            self.fuel_schedule,
            self.input,
            self.output,
            &self.records,
        )
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PsiTransformationLedgerDecodeError> {
        let mut cursor = LedgerCursor::new(encoded);
        if cursor.take(LEDGER_MAGIC.len())? != LEDGER_MAGIC {
            return Err(PsiTransformationLedgerDecodeError::WrongMagic);
        }
        let vocabulary = u16::from_le_bytes(cursor.array()?);
        let vocabulary_marker = VocabularyMarker::new(vocabulary).ok_or(
            PsiTransformationLedgerDecodeError::UnsupportedVocabulary(vocabulary),
        )?;
        let program_fingerprint = SemanticFingerprint::from_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(u32::from_le_bytes(cursor.array()?))
            .ok_or(PsiTransformationLedgerDecodeError::InvalidFuelSchedule)?;
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
            let provenance_count = cursor.length()?;
            let mut provenance = Vec::with_capacity(provenance_count.min(cursor.remaining()));
            for _ in 0..provenance_count {
                let machine = semantic_id(u64::from_le_bytes(cursor.array()?), MachineId::new)?;
                let block = semantic_id(u64::from_le_bytes(cursor.array()?), BlockId::new)?;
                let node = u32::from_le_bytes(cursor.array()?);
                let source_count = cursor.length()?;
                let mut sources = Vec::with_capacity(source_count.min(cursor.remaining()));
                for _ in 0..source_count {
                    sources.push(decode_provenance(&mut cursor)?);
                }
                let fuel_count = cursor.length()?;
                let mut fuel = Vec::with_capacity(fuel_count.min(cursor.remaining()));
                for _ in 0..fuel_count {
                    fuel.push(FuelSettlement {
                        site: decode_provenance(&mut cursor)?,
                        units: u64::from_le_bytes(cursor.array()?),
                    });
                }
                provenance.push(ProvenanceRewrite {
                    output: NodeLocation {
                        machine,
                        block,
                        node,
                    },
                    sources,
                    fuel,
                });
            }
            records.push(PsiTransformationRecord {
                rule,
                candidate,
                validator,
                input: record_input,
                output: record_output,
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
            fuel_schedule,
            input,
            output,
            records,
        )
        .map_err(PsiTransformationLedgerDecodeError::InvalidLedger)
    }
}

fn validate_provenance(rows: &[ProvenanceRewrite]) -> Result<(), InvalidPsiTransformationLedger> {
    if rows.is_empty() || rows.iter().any(|row| row.sources.is_empty()) {
        return Err(InvalidPsiTransformationLedger::EmptyProvenance);
    }
    if rows.windows(2).any(|pair| pair[0].output >= pair[1].output) {
        return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
    }
    for row in rows {
        let sources = row.sources.iter().copied().collect::<BTreeSet<_>>();
        if sources.len() != row.sources.len() {
            return Err(InvalidPsiTransformationLedger::NonCanonicalProvenance);
        }
        let fuel = row
            .fuel
            .iter()
            .map(|settlement| settlement.site)
            .collect::<BTreeSet<_>>();
        if fuel.len() != row.fuel.len() || fuel != sources {
            return Err(InvalidPsiTransformationLedger::FuelProvenanceMismatch);
        }
    }
    Ok(())
}

fn encode_ledger(
    terminal_psi: TerminalPsiIdentity,
    fuel_schedule: FuelScheduleIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    records: &[PsiTransformationRecord],
) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(LEDGER_MAGIC);
    encoded.extend_from_slice(&terminal_psi.vocabulary_marker.get().to_le_bytes());
    encoded.extend_from_slice(terminal_psi.program_fingerprint.as_bytes());
    encoded.extend_from_slice(&fuel_schedule.marker().to_le_bytes());
    encoded.extend_from_slice(&input.bytes());
    encoded.extend_from_slice(&output.bytes());
    encode_len(&mut encoded, records.len());
    for record in records {
        encoded.extend_from_slice(&record.rule.bytes());
        encoded.extend_from_slice(&record.candidate.bytes());
        encoded.extend_from_slice(&record.validator.bytes());
        encoded.extend_from_slice(&record.input.bytes());
        encoded.extend_from_slice(&record.output.bytes());
        encode_len(&mut encoded, record.provenance.len());
        for row in &record.provenance {
            encoded.extend_from_slice(&row.output.machine.get().to_le_bytes());
            encoded.extend_from_slice(&row.output.block.get().to_le_bytes());
            encoded.extend_from_slice(&row.output.node.to_le_bytes());
            encode_len(&mut encoded, row.sources.len());
            for source in &row.sources {
                encode_provenance(&mut encoded, *source);
            }
            encode_len(&mut encoded, row.fuel.len());
            for settlement in &row.fuel {
                encode_provenance(&mut encoded, settlement.site);
                encoded.extend_from_slice(&settlement.units.to_le_bytes());
            }
        }
    }
    encoded
}

fn encode_provenance(encoded: &mut Vec<u8>, provenance: PsiProvenance) {
    match provenance {
        PsiProvenance::Operation(operation) => {
            encoded.push(1);
            encoded.extend_from_slice(&operation.get().to_le_bytes());
        }
        PsiProvenance::Edge(edge) => {
            encoded.push(2);
            encoded.extend_from_slice(&edge.get().to_le_bytes());
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

fn decode_provenance(
    cursor: &mut LedgerCursor<'_>,
) -> Result<PsiProvenance, PsiTransformationLedgerDecodeError> {
    match cursor.byte()? {
        1 => Ok(PsiProvenance::Operation(semantic_id(
            u64::from_le_bytes(cursor.array()?),
            OperationId::new,
        )?)),
        2 => Ok(PsiProvenance::Edge(semantic_id(
            u64::from_le_bytes(cursor.array()?),
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

struct LedgerCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> LedgerCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], PsiTransformationLedgerDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PsiTransformationLedgerDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(PsiTransformationLedgerDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PsiTransformationLedgerDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PsiTransformationLedgerDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, PsiTransformationLedgerDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, PsiTransformationLedgerDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PsiTransformationLedgerDecodeError::LengthOverflow)
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FuelSettlement, NodeLocation};
    use psi_core::{BlockId, MachineId, OperationId};
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    fn record(input: &[u8], output: &[u8]) -> PsiTransformationRecord {
        let source = PsiProvenance::Operation(OperationId::new(3).unwrap());
        PsiTransformationRecord {
            rule: OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(output),
            validator: OptimizationValidatorIdentity::from_canonical_bytes(b"validator"),
            input: OptimizationUnitIdentity::from_canonical_bytes(input),
            output: OptimizationUnitIdentity::from_canonical_bytes(output),
            provenance: vec![ProvenanceRewrite {
                output: NodeLocation {
                    machine: MachineId::new(1).unwrap(),
                    block: BlockId::new(2).unwrap(),
                    node: 0,
                },
                sources: vec![source],
                fuel: vec![FuelSettlement {
                    site: source,
                    units: 1,
                }],
            }],
        }
    }

    fn source() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
        }
    }

    #[test]
    fn canonical_ledger_binds_revision_chain_and_fuel_map() {
        let first = record(b"input", b"middle");
        let second = record(b"middle", b"output");
        let ledger = PsiTransformationLedger::new(
            source(),
            FuelScheduleIdentity::new(1).unwrap(),
            first.input,
            second.output,
            vec![first.clone(), second.clone()],
        )
        .unwrap();
        let replay = PsiTransformationLedger::new(
            source(),
            FuelScheduleIdentity::new(1).unwrap(),
            first.input,
            second.output,
            vec![first, second],
        )
        .unwrap();
        assert_eq!(ledger.identity(), replay.identity());
        assert_eq!(ledger.records().len(), 2);
        assert_eq!(
            PsiTransformationLedger::decode(&ledger.encode()),
            Ok(ledger.clone())
        );

        let mut trailing = ledger.encode();
        trailing.push(0);
        assert_eq!(
            PsiTransformationLedger::decode(&trailing),
            Err(PsiTransformationLedgerDecodeError::TrailingBytes)
        );
        let mut wrong_magic = ledger.encode();
        wrong_magic[0] ^= 1;
        assert_eq!(
            PsiTransformationLedger::decode(&wrong_magic),
            Err(PsiTransformationLedgerDecodeError::WrongMagic)
        );
        let mut wrong_vocabulary = ledger.encode();
        wrong_vocabulary[LEDGER_MAGIC.len()..LEDGER_MAGIC.len() + 2]
            .copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            PsiTransformationLedger::decode(&wrong_vocabulary),
            Err(PsiTransformationLedgerDecodeError::UnsupportedVocabulary(
                u16::MAX
            ))
        );
        let encoded = ledger.encode();
        let truncated = &encoded[..encoded.len() - 1];
        assert_eq!(
            PsiTransformationLedger::decode(truncated),
            Err(PsiTransformationLedgerDecodeError::Truncated)
        );
    }

    #[test]
    fn broken_chain_and_fuel_mapping_reject() {
        let row = record(b"input", b"output");
        assert_eq!(
            PsiTransformationLedger::new(
                source(),
                FuelScheduleIdentity::new(1).unwrap(),
                OptimizationUnitIdentity::from_canonical_bytes(b"other"),
                row.output,
                vec![row.clone()],
            ),
            Err(InvalidPsiTransformationLedger::BrokenRevisionChain)
        );
        let mut bad_fuel = row;
        bad_fuel.provenance[0].fuel.clear();
        assert_eq!(
            PsiTransformationLedger::new(
                source(),
                FuelScheduleIdentity::new(1).unwrap(),
                bad_fuel.input,
                bad_fuel.output,
                vec![bad_fuel],
            ),
            Err(InvalidPsiTransformationLedger::FuelProvenanceMismatch)
        );
    }
}
