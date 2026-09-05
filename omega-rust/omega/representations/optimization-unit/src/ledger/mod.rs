//! Optimizer module role: executable entrance. Canonical Psi transformation-ledger construction entrance.
//!
//! This entrance validates revision, candidate, pruning, and provenance
//! custody before deriving the stable ledger identity. The immutable model,
//! validation mechanics, and versioned codec descend into named leaves.

use std::collections::BTreeSet;

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, TransformationLedgerIdentity,
};
use semantic_vocabulary::{BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    FuelSettlement, NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PrunedMachineCustody,
    PsiProvenance, PsiRealizationSite,
};

mod codec;
mod error;
mod model;
mod validation;

use codec::encode_ledger;
pub use error::{InvalidPsiTransformationLedger, PsiTransformationLedgerDecodeError};
pub use model::{PsiTransformationLedger, PsiTransformationRecord};
use validation::validate_provenance;

impl PsiTransformationLedger {
    pub fn new(
        psi: TerminalPsiIdentity,
        fuel_schedule: FuelScheduleIdentity,
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
            fuel_schedule,
            input,
            output,
            &records,
        ));
        Ok(Self {
            identity,
            psi,
            fuel_schedule,
            input,
            output,
            records,
        })
    }
}

#[cfg(test)]
use codec::{LEDGER_MAGIC, LedgerCursor};
#[cfg(test)]
mod tests;
