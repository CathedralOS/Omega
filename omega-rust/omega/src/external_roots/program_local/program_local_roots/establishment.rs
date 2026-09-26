//! Established program-local roots, epoch runtimes, cohorts and retirement.

use crate::external_roots::program_local::program_local_roots::occurrences::ProgramLocalRootScalarKey;
use crate::external_roots::program_local::program_local_roots::{
    InstalledProgramLocalRootEpochCohortId, InstalledProgramLocalRootOccurrence,
    InstalledProgramLocalRootOccurrenceId, InstalledProgramLocalRootSubject,
    ProgramLocalRootCohortMember, ProgramLocalRootEntryInvocationId,
    ProgramLocalRootEpochAggregate, ProgramLocalRootEpochAggregateSnapshot,
    ProgramLocalRootInstalledPrebinding, ProgramLocalRootScalarSource,
    ProgramLocalRootSubjectPlaceId,
};
use crate::external_roots::{ExternalRootDiagnostic, InstalledRequiredRootSlotClosure};
use abstract_operations_to_target_operations::effects::ProgramLocalRootEpochLeaseId;
use language_semantics::content::CanonicalIntervalSet;
use numerics::bignum::BigInt;
use std::collections::BTreeMap;

/// Report identity of one freshly established program-local lineage. The exact
/// authority remains the non-clonable account retaining the full installed
/// occurrence; this copyable identity is never accepted as minting evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootLineageId {
    occurrence: InstalledProgramLocalRootOccurrenceId,
}

impl ProgramLocalRootLineageId {
    pub const fn occurrence(self) -> InstalledProgramLocalRootOccurrenceId {
        self.occurrence
    }
}

/// Exact runtime instantiation of one verified symbolic capacity expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstablishedProgramLocalRootCapacity {
    IntervalSet(CanonicalIntervalSet),
    CountedQuantity(BigInt),
}

impl EstablishedProgramLocalRootCapacity {
    pub const fn interval_set(&self) -> Option<&CanonicalIntervalSet> {
        match self {
            Self::IntervalSet(value) => Some(value),
            Self::CountedQuantity(_) => None,
        }
    }

    pub const fn counted_quantity(&self) -> Option<&BigInt> {
        match self {
            Self::IntervalSet(_) => None,
            Self::CountedQuantity(value) => Some(value),
        }
    }
}

/// One exact fresh program-local content account. It owns the lifecycle lease
/// through its installed occurrence and therefore prevents epoch quiescence
/// until the account (and, later, every derived descendant) is retired.
#[derive(Debug)]
pub struct EstablishedProgramLocalRoot<'root, 'code> {
    pub(crate) occurrence: InstalledProgramLocalRootOccurrence<'root, 'code>,
    pub(crate) invocation: ProgramLocalRootEntryInvocationId,
    pub(crate) subject_place: ProgramLocalRootSubjectPlaceId,
    pub(crate) scalar_observations: BTreeMap<ProgramLocalRootScalarKey, BigInt>,
    pub(crate) capacity: EstablishedProgramLocalRootCapacity,
}

impl EstablishedProgramLocalRoot<'_, '_> {
    pub const fn lineage(&self) -> ProgramLocalRootLineageId {
        ProgramLocalRootLineageId {
            occurrence: self.occurrence.identity,
        }
    }

    pub const fn occurrence_identity(&self) -> InstalledProgramLocalRootOccurrenceId {
        self.occurrence.identity
    }

    pub const fn invocation(&self) -> ProgramLocalRootEntryInvocationId {
        self.invocation
    }

    pub const fn subject_place(&self) -> ProgramLocalRootSubjectPlaceId {
        self.subject_place
    }

    pub const fn prebinding(&self) -> &ProgramLocalRootInstalledPrebinding {
        &self.occurrence.prebinding
    }

    pub const fn capacity(&self) -> &EstablishedProgramLocalRootCapacity {
        &self.capacity
    }

    pub fn scalar_observations(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ProgramLocalRootScalarSource, &[String], &BigInt)> {
        self.scalar_observations
            .iter()
            .map(|((source, path), value)| (source, path.as_slice(), value))
    }
}

/// Runtime owner of the still-dormant members of one exact sealed epoch
/// cohort. Individual entry activations remove one exact occurrence only after
/// their complete subject/capacity validation succeeds.
#[derive(Debug)]
pub struct ProgramLocalRootEpochRuntime<'root, 'code> {
    pub(crate) identity: InstalledProgramLocalRootEpochCohortId,
    pub(crate) installed_required_slots: InstalledRequiredRootSlotClosure,
    pub(crate) pending: BTreeMap<
        InstalledProgramLocalRootOccurrenceId,
        InstalledProgramLocalRootOccurrence<'root, 'code>,
    >,
    aggregates: Vec<ProgramLocalRootEpochAggregate>,
}

impl<'root, 'code> ProgramLocalRootEpochRuntime<'root, 'code> {
    pub const fn identity(&self) -> InstalledProgramLocalRootEpochCohortId {
        self.identity
    }

    pub const fn installed_required_slots(&self) -> &InstalledRequiredRootSlotClosure {
        &self.installed_required_slots
    }

    pub fn pending_occurrences(
        &self,
    ) -> impl ExactSizeIterator<Item = &InstalledProgramLocalRootOccurrence<'root, 'code>> {
        self.pending.values()
    }

    pub fn aggregates(&self) -> impl ExactSizeIterator<Item = &ProgramLocalRootEpochAggregate> {
        self.aggregates.iter()
    }

    /// Snapshot the exact cohort aggregate rows for reporting without exposing
    /// any dormant occurrence or establishment authority.
    pub fn aggregate_snapshot(&self) -> ProgramLocalRootEpochAggregateSnapshot {
        ProgramLocalRootEpochAggregateSnapshot {
            identity: self.identity,
            installed_required_slots: self.installed_required_slots.clone(),
            aggregates: self.aggregates.clone(),
        }
    }

    /// Cancel every still-dormant occurrence without establishing authority.
    /// The returned occurrences may only be retired through their installation
    /// ledger; cancellation never reopens cohort sealing or same-epoch use.
    pub fn cancel(self) -> Vec<InstalledProgramLocalRootOccurrence<'root, 'code>> {
        self.pending.into_values().collect()
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootEstablishmentError<'root, 'code> {
    pub(crate) subject: InstalledProgramLocalRootSubject<'root, 'code>,
    pub(crate) diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalRootEstablishmentError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_subject(self) -> InstalledProgramLocalRootSubject<'root, 'code> {
        self.subject
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootBatchEstablishmentError<'root, 'code> {
    pub(crate) subjects: Vec<InstalledProgramLocalRootSubject<'root, 'code>>,
    pub(crate) diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalRootBatchEstablishmentError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_subjects(self) -> Vec<InstalledProgramLocalRootSubject<'root, 'code>> {
        self.subjects
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootRetirementError<'root, 'code> {
    pub(crate) root: EstablishedProgramLocalRoot<'root, 'code>,
    pub(crate) diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalRootRetirementError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_root(self) -> EstablishedProgramLocalRoot<'root, 'code> {
        self.root
    }
}

/// Non-clonable, exact program-local occurrence cohort for one installed
/// artifact and lifecycle epoch. Consuming it into the epoch runtime is the
/// sole route to runtime subject/capacity establishment and fresh lineage.
#[derive(Debug)]
pub struct InstalledProgramLocalRootEpochCohort<'root, 'code> {
    pub(crate) identity: InstalledProgramLocalRootEpochCohortId,
    pub(crate) installed_required_slots: InstalledRequiredRootSlotClosure,
    pub(crate) occurrences: Vec<InstalledProgramLocalRootOccurrence<'root, 'code>>,
    pub(crate) aggregates: Vec<ProgramLocalRootEpochAggregate>,
}

impl<'root, 'code> InstalledProgramLocalRootEpochCohort<'root, 'code> {
    pub const fn identity(&self) -> InstalledProgramLocalRootEpochCohortId {
        self.identity
    }

    pub const fn installed_required_slots(&self) -> &InstalledRequiredRootSlotClosure {
        &self.installed_required_slots
    }

    pub fn occurrences(
        &self,
    ) -> impl ExactSizeIterator<Item = &InstalledProgramLocalRootOccurrence<'root, 'code>> {
        self.occurrences.iter()
    }

    pub fn aggregates(&self) -> impl ExactSizeIterator<Item = &ProgramLocalRootEpochAggregate> {
        self.aggregates.iter()
    }

    /// Snapshot the exact cohort aggregate rows for reporting without exposing
    /// any installed occurrence or lifecycle authority.
    pub fn aggregate_snapshot(&self) -> ProgramLocalRootEpochAggregateSnapshot {
        ProgramLocalRootEpochAggregateSnapshot {
            identity: self.identity,
            installed_required_slots: self.installed_required_slots.clone(),
            aggregates: self.aggregates.clone(),
        }
    }

    /// Consume the exact sealed cohort into its runtime owner. No loose mint
    /// tokens are produced: dormant occurrences remain inside the runtime until
    /// one exact generated-entry subject establishes them or the runtime is
    /// explicitly cancelled for retirement.
    pub fn into_runtime(self) -> ProgramLocalRootEpochRuntime<'root, 'code> {
        ProgramLocalRootEpochRuntime {
            identity: self.identity,
            installed_required_slots: self.installed_required_slots,
            pending: self
                .occurrences
                .into_iter()
                .map(|occurrence| (occurrence.identity, occurrence))
                .collect(),
            aggregates: self.aggregates,
        }
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootCohortSealError<'root, 'code> {
    pub(crate) members: Vec<ProgramLocalRootCohortMember<'root, 'code>>,
    pub(crate) diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalRootCohortSealError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_members(self) -> Vec<ProgramLocalRootCohortMember<'root, 'code>> {
        self.members
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetiredProgramLocalRootOccurrence {
    pub(crate) identity: InstalledProgramLocalRootOccurrenceId,
    pub(crate) epoch_lease: ProgramLocalRootEpochLeaseId,
}

impl RetiredProgramLocalRootOccurrence {
    pub const fn identity(self) -> InstalledProgramLocalRootOccurrenceId {
        self.identity
    }

    pub const fn epoch_lease(self) -> ProgramLocalRootEpochLeaseId {
        self.epoch_lease
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootOccurrenceRetirementError<'root, 'code> {
    pub(crate) occurrence: InstalledProgramLocalRootOccurrence<'root, 'code>,
    pub(crate) diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalRootOccurrenceRetirementError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_occurrence(self) -> InstalledProgramLocalRootOccurrence<'root, 'code> {
        self.occurrence
    }
}
