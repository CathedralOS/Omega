//! Callable reach and preparation roots used after the working program is gone.
//!
//! Preparation supplies the actual fixed-point results and each independent
//! entry's result. Native consumers share those records and their canonical
//! bytes; none of them reopens the analysis graph or invents an empty target set.

use std::sync::Arc;

use optimization_core::digest_encoder::DomainDigestEncoder;
use optimization_core::{OptimizationUnitIdentity, PrePhysicalOptimizationManifestIdentity};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use terminal_psi::TerminalPsiIdentity;
pub use terminal_semantics::callable_values::ReachableCallableInvocations;
use terminal_semantics::callable_values::{CallableValuePossibilities, ReachedCallableTarget};

pub const NATIVE_EXECUTION_FACTS_DOMAIN: &[u8] = b"omega.native-execution-facts.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeExecutionRoots {
    pub psi: TerminalPsiIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub pre_physical: PrePhysicalOptimizationManifestIdentity,
    pub entry: MachineId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeExecutionFactsError {
    Invalid(&'static str),
    Wire(terminal_wire::CodecError),
}

impl std::fmt::Display for NativeExecutionFactsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::Wire(cause) => write!(formatter, "callable result cannot be encoded: {cause}"),
        }
    }
}

impl std::error::Error for NativeExecutionFactsError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecutionFacts {
    rows: Arc<ExecutionRows>,
    identity: [u8; 32],
}

#[derive(Debug, PartialEq, Eq)]
struct ExecutionRows {
    roots: NativeExecutionRoots,
    main: Arc<ReachableCallableInvocations>,
    entries: Vec<(MachineId, Arc<ReachableCallableInvocations>)>,
    main_bytes: Vec<u8>,
    entry_bytes: Vec<Vec<u8>>,
}

impl NativeExecutionFacts {
    /// Freeze query output, checking its readable ordering and exact entry roster.
    /// This is not authority to execute: source correspondence still checks the
    /// results against the independently retained source and registrar roots.
    pub fn seal(
        roots: NativeExecutionRoots,
        main: Arc<ReachableCallableInvocations>,
        entries: Vec<(MachineId, Arc<ReachableCallableInvocations>)>,
    ) -> Result<Self, NativeExecutionFactsError> {
        validate_summary(&main)?;
        if main.roots.binary_search(&roots.entry).is_err() {
            return Err(NativeExecutionFactsError::Invalid(
                "callable main summary omits its entry",
            ));
        }
        if entries.windows(2).any(|rows| rows[0].0 >= rows[1].0)
            || !main
                .roots
                .iter()
                .copied()
                .filter(|entry| *entry != roots.entry)
                .eq(entries.iter().map(|row| row.0))
        {
            return Err(NativeExecutionFactsError::Invalid(
                "callable entry summaries differ from the activated roots",
            ));
        }
        let main_bytes = terminal_wire::encode_callable_invocations(&main)
            .map_err(NativeExecutionFactsError::Wire)?;
        let mut entry_bytes = Vec::with_capacity(entries.len());
        for (entry, summary) in &entries {
            validate_summary(summary)?;
            if summary.roots.binary_search(entry).is_err() {
                return Err(NativeExecutionFactsError::Invalid(
                    "callable entry summary omits its own entry",
                ));
            }
            entry_bytes.push(
                terminal_wire::encode_callable_invocations(summary)
                    .map_err(NativeExecutionFactsError::Wire)?,
            );
        }
        let rows = ExecutionRows {
            roots,
            main,
            entries,
            main_bytes,
            entry_bytes,
        };
        let mut digest = DomainDigestEncoder::new(b"");
        encode(&rows, |bytes| digest.raw(bytes));
        Ok(Self {
            rows: Arc::new(rows),
            identity: digest.finish(),
        })
    }

    pub fn roots(&self) -> NativeExecutionRoots {
        self.rows.roots
    }

    pub fn callable_invocations(&self) -> &ReachableCallableInvocations {
        &self.rows.main
    }

    pub fn entry_callable_invocations(&self) -> &[(MachineId, Arc<ReachableCallableInvocations>)] {
        &self.rows.entries
    }

    pub fn callable_invocations_from(
        &self,
        entry: MachineId,
    ) -> Option<&ReachableCallableInvocations> {
        if entry == self.rows.roots.entry {
            return Some(&self.rows.main);
        }
        let position = self
            .rows
            .entries
            .binary_search_by_key(&entry, |row| row.0)
            .ok()?;
        Some(&self.rows.entries[position].1)
    }

    pub const fn identity_bytes(&self) -> [u8; 32] {
        self.identity
    }

    pub fn encode(&self, bytes: &mut Vec<u8>) {
        encode(&self.rows, |part| bytes.extend_from_slice(part));
    }
}

fn ordered<T: Ord>(rows: &[T]) -> bool {
    rows.windows(2).all(|pair| pair[0] < pair[1])
}

fn validate_values(values: &CallableValuePossibilities) -> Result<(), NativeExecutionFactsError> {
    if !ordered(&values.descriptors)
        || !ordered(&values.inputs)
        || !ordered(&values.unresolved_ceilings)
    {
        return Err(NativeExecutionFactsError::Invalid(
            "callable value possibilities are unordered or repeated",
        ));
    }
    Ok(())
}

fn validate_summary(
    summary: &ReachableCallableInvocations,
) -> Result<(), NativeExecutionFactsError> {
    if !ordered(&summary.roots)
        || !ordered(&summary.reached_machines)
        || summary
            .roots
            .iter()
            .any(|root| summary.reached_machines.binary_search(root).is_err())
        || summary.invocations.windows(2).any(|rows| {
            (rows[0].caller, rows[0].invocation.site) >= (rows[1].caller, rows[1].invocation.site)
        })
        || summary.inputs.windows(2).any(|rows| rows[0].0 >= rows[1].0)
    {
        return Err(NativeExecutionFactsError::Invalid(
            "callable summary roots or rows are unordered, missing or repeated",
        ));
    }
    for row in &summary.invocations {
        if summary.reached_machines.binary_search(&row.caller).is_err()
            || !ordered(&row.selected_machines)
        {
            return Err(NativeExecutionFactsError::Invalid(
                "callable invocation has a foreign caller or repeated targets",
            ));
        }
        if let ReachedCallableTarget::Callable(values) = &row.target {
            validate_values(values)?;
        }
    }
    for (_, values) in &summary.inputs {
        validate_values(values)?;
    }
    Ok(())
}

fn encode(rows: &ExecutionRows, mut write: impl FnMut(&[u8])) {
    write(NATIVE_EXECUTION_FACTS_DOMAIN);
    write(rows.roots.psi.program_fingerprint.as_bytes());
    write(&rows.roots.psi.vocabulary_marker.get().to_le_bytes());
    write(&rows.roots.optimization_unit.bytes());
    write(&rows.roots.work_schedule.marker().to_le_bytes());
    write(&rows.roots.pre_physical.bytes());
    write(&rows.roots.entry.get().to_le_bytes());
    write(&(rows.main_bytes.len() as u64).to_le_bytes());
    write(&rows.main_bytes);
    write(&(rows.entries.len() as u64).to_le_bytes());
    for ((entry, _), bytes) in rows.entries.iter().zip(&rows.entry_bytes) {
        write(&entry.get().to_le_bytes());
        write(&(bytes.len() as u64).to_le_bytes());
        write(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn roots(entry: MachineId) -> NativeExecutionRoots {
        NativeExecutionRoots {
            psi: TerminalPsiIdentity {
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            },
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"test unit"),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            pre_physical: PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(
                b"test prephysical",
            ),
            entry,
        }
    }

    #[test]
    fn sealed_execution_facts_share_the_results_and_bind_preparation_roots() {
        let entry = MachineId::new(1).unwrap();
        let main = Arc::new(ReachableCallableInvocations {
            roots: vec![entry],
            reached_machines: vec![entry],
            invocations: Vec::new(),
            inputs: Vec::new(),
        });
        let facts =
            NativeExecutionFacts::seal(roots(entry), Arc::clone(&main), Vec::new()).unwrap();
        assert!(Arc::ptr_eq(&facts.rows.main, &main));
        assert!(Arc::ptr_eq(&facts.rows, &facts.clone().rows));
        let mut bytes = Vec::new();
        facts.encode(&mut bytes);
        assert_eq!(
            facts.identity_bytes(),
            optimization_core::digest_encoder::content_identity(&bytes)
        );
        let mut changed = roots(entry);
        changed.pre_physical =
            PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"other prephysical");
        let changed = NativeExecutionFacts::seal(changed, main, Vec::new()).unwrap();
        assert_ne!(facts.identity_bytes(), changed.identity_bytes());
        assert!(
            facts
                .callable_invocations_from(MachineId::new(2).unwrap())
                .is_none()
        );
    }

    #[test]
    fn missing_activated_entry_summary_is_not_an_empty_invocation_context() {
        let entry = MachineId::new(1).unwrap();
        let callback = MachineId::new(2).unwrap();
        let main = Arc::new(ReachableCallableInvocations {
            roots: vec![entry, callback],
            reached_machines: vec![entry, callback],
            invocations: Vec::new(),
            inputs: Vec::new(),
        });
        assert_eq!(
            NativeExecutionFacts::seal(roots(entry), main, Vec::new()).unwrap_err(),
            NativeExecutionFactsError::Invalid(
                "callable entry summaries differ from the activated roots"
            )
        );
    }
}
