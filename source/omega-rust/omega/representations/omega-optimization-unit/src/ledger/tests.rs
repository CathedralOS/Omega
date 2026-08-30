//! Transformation-ledger construction, codec, and corruption coverage.

use super::*;
use crate::{FuelSettlement, NodeLocation, ProvenanceDisposition};
use psi_core::{BlockId, MachineId, OperationId};
use psi_terminal::{SemanticFingerprint, VocabularyMarker};

fn record(input: &[u8], output: &[u8]) -> PsiTransformationRecord {
    let source = PsiProvenance::Operation(OperationId::new(3).unwrap());
    let site = PsiRealizationSite::Node(NodeLocation {
        machine: MachineId::new(1).unwrap(),
        block: BlockId::new(2).unwrap(),
        node: 0,
    });
    PsiTransformationRecord {
        rule: OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        candidate: OptimizationCandidateIdentity::from_canonical_bytes(output),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(b"validator"),
        input: OptimizationUnitIdentity::from_canonical_bytes(input),
        output: OptimizationUnitIdentity::from_canonical_bytes(output),
        pruned_machines: Vec::new(),
        provenance: vec![ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: vec![source],
            fuel: vec![FuelSettlement {
                site: source,
                units: 1,
            }],
        }],
    }
}

fn disposition_record(input: &[u8], output: &[u8]) -> PsiTransformationRecord {
    let location = NodeLocation {
        machine: MachineId::new(1).unwrap(),
        block: BlockId::new(2).unwrap(),
        node: 0,
    };
    let realized_site = PsiRealizationSite::Node(location);
    let unreachable_site = PsiRealizationSite::Node(NodeLocation {
        node: 1,
        ..location
    });
    let realized = PsiProvenance::Operation(OperationId::new(3).unwrap());
    let unreachable = PsiProvenance::Operation(OperationId::new(4).unwrap());
    PsiTransformationRecord {
        rule: OptimizationRuleIdentity::from_canonical_bytes(b"disposition-rule"),
        candidate: OptimizationCandidateIdentity::from_canonical_bytes(output),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(b"disposition-validator"),
        input: OptimizationUnitIdentity::from_canonical_bytes(input),
        output: OptimizationUnitIdentity::from_canonical_bytes(output),
        pruned_machines: Vec::new(),
        provenance: vec![
            ProvenanceRewrite {
                input: realized_site,
                disposition: ProvenanceDisposition::RealizedAt(realized_site),
                sources: vec![realized],
                fuel: vec![FuelSettlement {
                    site: realized,
                    units: 1,
                }],
            },
            ProvenanceRewrite {
                input: unreachable_site,
                disposition: ProvenanceDisposition::ProvenUnreachableAt(unreachable_site),
                sources: vec![unreachable],
                fuel: vec![FuelSettlement {
                    site: unreachable,
                    units: 1,
                }],
            },
        ],
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
fn machine_roster_custody_round_trips_and_rejects_duplicates() {
    let mut row = record(b"input", b"middle");
    row.pruned_machines.push(PrunedMachineCustody {
        machine: MachineId::new(7).unwrap(),
        source_ordinal: 1,
    });
    let ledger = PsiTransformationLedger::new(
        source(),
        FuelScheduleIdentity::new(1).unwrap(),
        row.input,
        row.output,
        vec![row.clone()],
    )
    .unwrap();
    assert_eq!(
        PsiTransformationLedger::decode(&ledger.encode()),
        Ok(ledger)
    );

    let mut second = record(b"middle", b"output");
    second.pruned_machines = row.pruned_machines.clone();
    assert_eq!(
        PsiTransformationLedger::new(
            source(),
            FuelScheduleIdentity::new(1).unwrap(),
            row.input,
            second.output,
            vec![row, second],
        ),
        Err(InvalidPsiTransformationLedger::DuplicatePrunedMachine)
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

#[test]
fn mixed_dispositions_round_trip_and_reject_corruption() {
    let row = disposition_record(b"input", b"output");
    let ledger = PsiTransformationLedger::new(
        source(),
        FuelScheduleIdentity::new(1).unwrap(),
        row.input,
        row.output,
        vec![row.clone()],
    )
    .unwrap();
    assert_eq!(
        PsiTransformationLedger::decode(&ledger.encode()),
        Ok(ledger.clone())
    );

    let mut unknown_disposition = ledger.encode();
    let disposition_offset = {
        let mut cursor = LedgerCursor::new(&unknown_disposition);
        cursor.take(LEDGER_MAGIC.len()).unwrap();
        cursor.take(2 + 32 + 4 + 32 + 32).unwrap();
        assert_eq!(cursor.length().unwrap(), 1);
        cursor.take(32 * 5).unwrap();
        assert_eq!(cursor.length().unwrap(), 0);
        assert_eq!(cursor.length().unwrap(), 2);
        cursor.take(1 + 8 + 8 + 4).unwrap();
        cursor.offset
    };
    unknown_disposition[disposition_offset] = 99;
    assert_eq!(
        PsiTransformationLedger::decode(&unknown_disposition),
        Err(PsiTransformationLedgerDecodeError::UnknownDispositionTag(
            99
        ))
    );

    let mut noncanonical = row.clone();
    noncanonical.provenance.swap(0, 1);
    assert_eq!(
        PsiTransformationLedger::new(
            source(),
            FuelScheduleIdentity::new(1).unwrap(),
            noncanonical.input,
            noncanonical.output,
            vec![noncanonical],
        ),
        Err(InvalidPsiTransformationLedger::NonCanonicalProvenance)
    );

    let mut duplicate = row.clone();
    duplicate.provenance[1].input = duplicate.provenance[0].input;
    duplicate.provenance[1].sources = duplicate.provenance[0].sources.clone();
    duplicate.provenance[1].fuel = duplicate.provenance[0].fuel.clone();
    assert_eq!(
        PsiTransformationLedger::new(
            source(),
            FuelScheduleIdentity::new(1).unwrap(),
            duplicate.input,
            duplicate.output,
            vec![duplicate],
        ),
        Err(InvalidPsiTransformationLedger::NonCanonicalProvenance)
    );

    let mut zero = row;
    zero.provenance[1].fuel[0].units = 0;
    assert_eq!(
        PsiTransformationLedger::new(
            source(),
            FuelScheduleIdentity::new(1).unwrap(),
            zero.input,
            zero.output,
            vec![zero],
        ),
        Err(InvalidPsiTransformationLedger::ZeroFuelSettlement)
    );
}
