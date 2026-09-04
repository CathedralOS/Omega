//! Transformation-ledger decoding and canonical reconstruction.

use super::*;

impl PsiTransformationLedger {
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
            let pruned_count = cursor.length()?;
            let mut pruned_machines = Vec::with_capacity(pruned_count.min(cursor.remaining()));
            for _ in 0..pruned_count {
                let machine = semantic_id(u64::from_le_bytes(cursor.array()?), MachineId::new)?;
                let source_ordinal = u32::from_le_bytes(cursor.array()?);
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
                let fuel_count = cursor.length()?;
                let mut fuel = Vec::with_capacity(fuel_count.min(cursor.remaining()));
                for _ in 0..fuel_count {
                    fuel.push(FuelSettlement {
                        site: decode_provenance(&mut cursor)?,
                        units: u64::from_le_bytes(cursor.array()?),
                    });
                }
                provenance.push(ProvenanceRewrite {
                    input,
                    disposition,
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
            fuel_schedule,
            input,
            output,
            records,
        )
        .map_err(PsiTransformationLedgerDecodeError::InvalidLedger)
    }
}

fn decode_realization_site(
    cursor: &mut LedgerCursor<'_>,
) -> Result<PsiRealizationSite, PsiTransformationLedgerDecodeError> {
    let tag = cursor.byte()?;
    let machine = semantic_id(u64::from_le_bytes(cursor.array()?), MachineId::new)?;
    match tag {
        1 => Ok(PsiRealizationSite::Node(NodeLocation {
            machine,
            block: semantic_id(u64::from_le_bytes(cursor.array()?), BlockId::new)?,
            node: u32::from_le_bytes(cursor.array()?),
        })),
        2 => Ok(PsiRealizationSite::Edge {
            machine,
            edge: semantic_id(u64::from_le_bytes(cursor.array()?), EdgeId::new)?,
        }),
        tag => Err(PsiTransformationLedgerDecodeError::UnknownRealizationSiteTag(tag)),
    }
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
