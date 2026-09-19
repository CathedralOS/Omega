//! Immutable transformation-ledger records and accessors.

use super::{
    FuelScheduleIdentity, MachineId, OptimizationCandidateIdentity, OptimizationRuleIdentity,
    OptimizationUnitIdentity, OptimizationValidatorIdentity, ProvenanceRewrite,
    TerminalPsiIdentity, TransformationLedgerIdentity,
};

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
    pub(super) fuel_schedule: FuelScheduleIdentity,
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
