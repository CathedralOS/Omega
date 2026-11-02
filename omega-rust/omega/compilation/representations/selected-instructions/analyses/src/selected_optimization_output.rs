//! What selected optimization (Omega 04) hands register allocation (Omega 05).
//!
//! A completed suite publishes its final program, the facts rebuilt over it,
//! the allocation-legality stage it was admitted from, and its ledger: the
//! suite's completion identity, the admitted source custody, and the
//! transformations it committed in order. Register allocation assigns homes
//! over the final facts and carries the ledger into its post-allocation
//! manifest; it does not read the run that produced them.

use optimization_core::{
    OptimizationSelectionIdentity, PreAllocationOptimizationCompletionIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use register_homes::PostAllocationSelectedTransformation;

use crate::{
    OwnedSelectedProgram, SelectedProgramRef, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedLiveRanges,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
};

/// How the current program was obtained.
#[derive(Debug)]
pub enum SelectedOptimizationOutput {
    /// No executed rule was selected: the staged ranges over the program
    /// instruction selection produced.
    Identity(StagedOptimizedLiveRanges),
    SelectedLowering(CompletedSelectedOptimization<SelectedLoweringOptimizationCompletionIdentity>),
    PreAllocation(CompletedSelectedOptimization<PreAllocationOptimizationCompletionIdentity>),
}

/// A completed suite's final program and facts beside its ledger.
#[derive(Debug)]
pub struct CompletedSelectedOptimization<Completion> {
    pub source: StagedOptimizedAllocationLegality,
    pub current: OwnedSelectedProgram,
    pub liveness: ValidatedLiveness,
    pub ranges: ValidatedLiveRanges,
    pub legality: ValidatedAllocationLegality,
    pub ledger: SelectedOptimizationLedger<Completion>,
}

impl<Completion> CompletedSelectedOptimization<Completion> {
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }

    /// The program the suite published.
    pub fn current(&self) -> SelectedProgramRef<'_> {
        SelectedProgramRef::new(&self.current)
    }

    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }

    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }

    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }

    pub const fn ledger(&self) -> &SelectedOptimizationLedger<Completion> {
        &self.ledger
    }
}

/// The suite's completion identity, the exact phase selections it ran, the
/// custody it was admitted with, and its committed transformations in order. An empty ledger is still a
/// completed run: the suite reached its fixed point without a rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedOptimizationLedger<Completion> {
    pub identity: Completion,
    pub phase_selections: OptimizationSelectionIdentity,
    pub source: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub transformations: Vec<PostAllocationSelectedTransformation>,
}

impl<Completion: Copy> SelectedOptimizationLedger<Completion> {
    pub const fn identity(&self) -> Completion {
        self.identity
    }

    /// The identity of the phase projection the suite executed.
    pub const fn phase_selections(&self) -> OptimizationSelectionIdentity {
        self.phase_selections
    }

    pub const fn source(&self) -> &StagedOptimizedAllocationLegalityCustodyReceipt {
        &self.source
    }

    pub fn transformations(&self) -> &[PostAllocationSelectedTransformation] {
        &self.transformations
    }
}
