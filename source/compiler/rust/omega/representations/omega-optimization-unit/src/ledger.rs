use std::collections::BTreeSet;

use omega_optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, TransformationLedgerIdentity,
};
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::{ProvenanceRewrite, PsiProvenance};

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
    encoded.extend_from_slice(b"omega.psi-transformation-ledger.v1\0");
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
