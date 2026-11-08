//! Stable transformation-ledger encoding.

use super::super::{
    FuelScheduleIdentity, OptimizationUnitIdentity, ProvenanceDisposition, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger, PsiTransformationRecord, TerminalPsiIdentity,
};
use super::LEDGER_MAGIC;
impl PsiTransformationLedger {
    pub fn encode(&self) -> Vec<u8> {
        encode_ledger(
            self.psi(),
            self.fuel_schedule(),
            self.input(),
            self.output(),
            self.records(),
        )
    }
}

pub(in crate::optimization_unit::ledger) fn encode_ledger(
    psi: TerminalPsiIdentity,
    fuel_schedule: FuelScheduleIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    records: &[PsiTransformationRecord],
) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(LEDGER_MAGIC);
    crate::codec_bytes::push_encoded(&mut encoded, psi.vocabulary_marker.get().to_le_bytes());
    encoded.extend_from_slice(psi.program_fingerprint.as_bytes());
    crate::codec_bytes::push_encoded(&mut encoded, fuel_schedule.marker().to_le_bytes());
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
            encode_len(&mut encoded, row.fuel.len());
            for settlement in &row.fuel {
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
