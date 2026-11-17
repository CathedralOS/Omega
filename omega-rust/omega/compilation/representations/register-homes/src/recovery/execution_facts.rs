//! Recover the complete callable result and its exact preparation namespace.

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, PrePhysicalOptimizationManifestIdentity};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target_vocabulary::{NativeExecutionFacts, NativeExecutionFactsError, NativeExecutionRoots};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::fixed_view_copy_codec::{Cursor, decode_id};
use super::program_contracts::decode_rows;
use crate::FixedViewCopyDecodeError as Error;

pub(super) fn decode(cursor: &mut Cursor<'_>) -> Result<NativeExecutionFacts, Error> {
    let domain = target_vocabulary::execution_facts::NATIVE_EXECUTION_FACTS_DOMAIN;
    if cursor.take(domain.len())? != domain {
        return Err(Error::InvalidExecutionFacts(
            NativeExecutionFactsError::Invalid("execution facts have another wire domain"),
        ));
    }
    let program_fingerprint = SemanticFingerprint::from_bytes(cursor.array()?);
    let marker = cursor.u16()?;
    let vocabulary_marker =
        VocabularyMarker::new(marker).ok_or(Error::InvalidVocabulary(marker))?;
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let schedule = cursor.u32()?;
    let work_schedule =
        WorkScheduleIdentity::new(schedule).ok_or(Error::InvalidWorkSchedule(schedule))?;
    let pre_physical = PrePhysicalOptimizationManifestIdentity::from_bytes(cursor.array()?);
    let entry = decode_id(cursor, MachineId::new)?;
    let main = decode_summary(cursor)?;
    // Entry ID, payload length and the four minimum summary roster counts.
    let entries = decode_rows(cursor, 32, |cursor| {
        Ok((decode_id(cursor, MachineId::new)?, decode_summary(cursor)?))
    })?;
    NativeExecutionFacts::seal(
        NativeExecutionRoots {
            psi: TerminalPsiIdentity {
                program_fingerprint,
                vocabulary_marker,
            },
            optimization_unit,
            work_schedule,
            pre_physical,
            entry,
        },
        main,
        entries,
    )
    .map_err(Error::InvalidExecutionFacts)
}

fn decode_summary(
    cursor: &mut Cursor<'_>,
) -> Result<Arc<target_vocabulary::execution_facts::ReachableCallableInvocations>, Error> {
    let length = cursor.length()?;
    terminal_wire::decode_callable_invocations(cursor.take(length)?)
        .map(Arc::new)
        .map_err(|cause| Error::InvalidExecutionFacts(NativeExecutionFactsError::Wire(cause)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use target_vocabulary::execution_facts::ReachableCallableInvocations;

    #[test]
    fn execution_facts_roundtrip_keeps_each_entry_and_refuses_missing_or_oversized_rosters() {
        let entry = MachineId::new(1).unwrap();
        let callback = MachineId::new(2).unwrap();
        let main = Arc::new(ReachableCallableInvocations {
            roots: vec![entry, callback],
            reached_machines: vec![entry, callback],
            invocations: Vec::new(),
            inputs: Vec::new(),
        });
        let callback_summary = Arc::new(ReachableCallableInvocations {
            roots: vec![callback],
            reached_machines: vec![callback],
            invocations: Vec::new(),
            inputs: Vec::new(),
        });
        let roots = NativeExecutionRoots {
            psi: TerminalPsiIdentity {
                program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
                vocabulary_marker: VocabularyMarker::CURRENT,
            },
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            pre_physical: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(
                b"prephysical",
            ),
            entry,
        };
        let facts = NativeExecutionFacts::seal(
            roots,
            Arc::clone(&main),
            vec![(callback, callback_summary)],
        )
        .unwrap();
        let mut bytes = Vec::new();
        facts.encode(&mut bytes);
        let mut cursor = Cursor::new(&bytes);
        let recovered = decode(&mut cursor).unwrap();
        assert_eq!(cursor.remaining(), 0);
        assert_eq!(facts, recovered);
        assert_eq!(facts.identity_bytes(), recovered.identity_bytes());
        assert_eq!(
            recovered.callable_invocations_from(callback).unwrap().roots,
            [callback]
        );
        for prefix in 0..bytes.len() {
            assert!(decode(&mut Cursor::new(&bytes[..prefix])).is_err());
        }
        let entries = target_vocabulary::execution_facts::NATIVE_EXECUTION_FACTS_DOMAIN.len()
            + 32
            + 2
            + 32
            + 4
            + 32
            + 8
            + 8
            + terminal_wire::encode_callable_invocations(&main)
                .unwrap()
                .len();
        let mut missing = bytes[..entries].to_vec();
        missing.extend_from_slice(&0_u64.to_le_bytes());
        assert!(matches!(
            decode(&mut Cursor::new(&missing)),
            Err(Error::InvalidExecutionFacts(
                NativeExecutionFactsError::Invalid(
                    "callable entry summaries differ from the activated roots"
                )
            ))
        ));
        let mut oversized = bytes.clone();
        oversized[entries..entries + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            decode(&mut Cursor::new(&oversized)),
            Err(Error::Truncated | Error::LengthOverflow)
        ));
        let mut duplicate = bytes.clone();
        duplicate[entries..entries + 8].copy_from_slice(&2_u64.to_le_bytes());
        duplicate.extend_from_slice(&bytes[entries + 8..]);
        assert!(matches!(
            decode(&mut Cursor::new(&duplicate)),
            Err(Error::InvalidExecutionFacts(
                NativeExecutionFactsError::Invalid(
                    "callable entry summaries differ from the activated roots"
                )
            ))
        ));
    }
}
