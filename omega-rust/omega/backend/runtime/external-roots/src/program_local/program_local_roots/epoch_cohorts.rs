//! Epoch cohort members, aggregates, snapshots and coexistence reports.

use crate::program_local::program_local_roots::{
    EstablishedProgramLocalRootCapacity, InstalledProgramLocalRootOccurrenceId,
    ProgramLocalRootPrebindingId, ProgramLocalRootSchemaDigest,
};
use crate::{ExternalRootDiagnostic, InstalledExternalRoot, InstalledRequiredRootSlotClosure};
use effects::{
    ComponentEraEntryLedger, ComponentEraLedgerId, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseId,
};
use executable_installation::{ArtifactId, InstalledCodeId};
use semantic_vocabulary::{ContentAlgebra, ContentProjectionExpression};
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use terminal_psi::TerminalPsiIdentity;

/// Non-authoritative inputs for one member of an epoch cohort. Construction
/// packages existing custody only; no occurrence exists until the complete
/// cohort is validated and committed atomically.
#[derive(Debug)]
pub struct ProgramLocalRootCohortMember<'root, 'code> {
    pub(crate) prebinding: ProgramLocalRootPrebindingId,
    pub(crate) root: &'root InstalledExternalRoot<'code>,
    pub(crate) epoch_lease: ProgramLocalRootEpochLease,
}

impl<'root, 'code> ProgramLocalRootCohortMember<'root, 'code> {
    pub fn new(
        prebinding: ProgramLocalRootPrebindingId,
        root: &'root InstalledExternalRoot<'code>,
        epoch_lease: ProgramLocalRootEpochLease,
    ) -> Self {
        Self {
            prebinding,
            root,
            epoch_lease,
        }
    }

    pub const fn prebinding(&self) -> ProgramLocalRootPrebindingId {
        self.prebinding
    }

    pub const fn epoch_lease_identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.epoch_lease.identity()
    }

    pub fn into_parts(
        self,
    ) -> (
        ProgramLocalRootPrebindingId,
        &'root InstalledExternalRoot<'code>,
        ProgramLocalRootEpochLease,
    ) {
        (self.prebinding, self.root, self.epoch_lease)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstalledProgramLocalRootEpochCohortId {
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) lifecycle_ledger: ComponentEraLedgerId,
    pub(crate) lifecycle_epoch: u64,
}

impl InstalledProgramLocalRootEpochCohortId {
    pub const fn installed_code(self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn lifecycle_ledger(self) -> ComponentEraLedgerId {
        self.lifecycle_ledger
    }

    pub const fn lifecycle_epoch(self) -> u64 {
        self.lifecycle_epoch
    }
}

/// One exact aggregate schema derived from the closed epoch cohort. Capacity
/// expressions remain per occurrence: subject-dependent and interval content
/// cannot be replaced by blind scalar multiplication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootEpochAggregate {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) artifact: ArtifactId,
    pub(crate) requirement_identity: String,
    pub(crate) argument_index: u32,
    pub(crate) source_parameter_position: u32,
    pub(crate) qualification_identity: String,
    pub(crate) carrier_identity: String,
    pub(crate) schema_digest: ProgramLocalRootSchemaDigest,
    pub(crate) schema_compatibility_report_identity: u64,
    pub(crate) algebra: ContentAlgebra,
    pub(crate) per_occurrence_capacity: ContentProjectionExpression,
    pub(crate) occurrence_identities: Vec<InstalledProgramLocalRootOccurrenceId>,
}

impl ProgramLocalRootEpochAggregate {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn argument_index(&self) -> u32 {
        self.argument_index
    }

    pub const fn source_parameter_position(&self) -> u32 {
        self.source_parameter_position
    }

    pub fn qualification_identity(&self) -> &str {
        &self.qualification_identity
    }

    pub fn carrier_identity(&self) -> &str {
        &self.carrier_identity
    }

    pub const fn schema_digest(&self) -> ProgramLocalRootSchemaDigest {
        self.schema_digest
    }

    pub const fn schema_compatibility_report_identity(&self) -> u64 {
        self.schema_compatibility_report_identity
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn per_occurrence_capacity(&self) -> &ContentProjectionExpression {
        &self.per_occurrence_capacity
    }

    pub fn occurrence_identities(
        &self,
    ) -> impl ExactSizeIterator<Item = InstalledProgramLocalRootOccurrenceId> + '_ {
        self.occurrence_identities.iter().copied()
    }

    pub fn cardinality(&self) -> NonZeroU64 {
        NonZeroU64::new(
            u64::try_from(self.occurrence_identities.len())
                .expect("sealed program-local cohort cardinality fits u64"),
        )
        .expect("sealed aggregate group is nonempty")
    }
}

/// Exact reconstructed capacity of one live program-local aggregate schema
/// group in one sealed epoch cohort of one installed artifact instance.
///
/// The retained aggregate row reports the group's complete live established
/// membership — a retired member has already left the epoch's demand — while
/// `capacity` is the verifier-derived composition of every member's evaluated
/// per-occurrence capacity: the exact counted sum or separated interval union.
/// This is accounting evidence for the artifact instance and lifecycle epoch,
/// not minting, lifecycle, or row-equality authority.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "program-local aggregate capacity reports are exact accounting evidence"]
pub struct ProgramLocalRootEpochAggregateCapacity {
    pub(crate) cohort: InstalledProgramLocalRootEpochCohortId,
    pub(crate) aggregate: ProgramLocalRootEpochAggregate,
    pub(crate) capacity: EstablishedProgramLocalRootCapacity,
}

impl ProgramLocalRootEpochAggregateCapacity {
    pub const fn cohort(&self) -> InstalledProgramLocalRootEpochCohortId {
        self.cohort
    }

    /// The group's symbolic row rebound to its live established membership.
    pub const fn aggregate(&self) -> &ProgramLocalRootEpochAggregate {
        &self.aggregate
    }

    /// The reconstructed evaluated capacity: exact sum over counted-quantity
    /// members or the separated union of interval-set members.
    pub const fn capacity(&self) -> &EstablishedProgramLocalRootCapacity {
        &self.capacity
    }
}

/// Cloneable reporting snapshot of one exact sealed program-local epoch
/// cohort. This value retains the exact installed required-slot closure,
/// verifier-derived aggregate rows, and cohort identity, but carries no
/// occurrence, lifecycle lease, lineage, or establishment authority. The
/// closure keeps even an empty row set bound to its installed artifact and
/// installation scope.
///
/// Construction remains private to the sealed cohort/runtime owners so a
/// consumer cannot present authored aggregate rows as installation evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "program-local aggregate snapshots are reporting evidence only"]
pub struct ProgramLocalRootEpochAggregateSnapshot {
    pub(crate) identity: InstalledProgramLocalRootEpochCohortId,
    pub(crate) installed_required_slots: InstalledRequiredRootSlotClosure,
    pub(crate) aggregates: Vec<ProgramLocalRootEpochAggregate>,
}

impl ProgramLocalRootEpochAggregateSnapshot {
    pub const fn identity(&self) -> InstalledProgramLocalRootEpochCohortId {
        self.identity
    }

    pub const fn installed_required_slots(&self) -> &InstalledRequiredRootSlotClosure {
        &self.installed_required_slots
    }

    pub fn aggregates(&self) -> impl ExactSizeIterator<Item = &ProgramLocalRootEpochAggregate> {
        self.aggregates.iter()
    }
}

/// Exact, non-authoritative union of the program-local aggregate snapshots for
/// every era currently retained by one component lifecycle ledger.
///
/// Rows remain attributed to their epoch and preserve their original content
/// algebra and symbolic per-occurrence expression. This report deliberately
/// does not multiply cardinalities or reduce unlike algebras to a scalar:
/// deployment policy may compose those exact rows, but reporting cannot invent
/// a resource interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "program-local coexistence reports are exact audit inputs"]
pub struct ProgramLocalRootCoexistenceReport {
    lifecycle_ledger: ComponentEraLedgerId,
    epoch_snapshots: Vec<ProgramLocalRootEpochAggregateSnapshot>,
}

impl ProgramLocalRootCoexistenceReport {
    pub const fn lifecycle_ledger(&self) -> ComponentEraLedgerId {
        self.lifecycle_ledger
    }

    pub fn epoch_snapshots(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProgramLocalRootEpochAggregateSnapshot> {
        self.epoch_snapshots.iter()
    }

    pub fn aggregates(&self) -> impl Iterator<Item = (u64, &ProgramLocalRootEpochAggregate)> {
        self.epoch_snapshots.iter().flat_map(|snapshot| {
            let epoch = snapshot.identity.lifecycle_epoch;
            snapshot
                .aggregates
                .iter()
                .map(move |aggregate| (epoch, aggregate))
        })
    }
}

/// Compose the exact root-demand report for one live component-era set.
///
/// The lifecycle ledger supplies the complete live-era roster. Every supplied
/// snapshot must have been derived from a sealed cohort for that exact ledger,
/// and every live era must appear exactly once, including eras whose snapshot
/// contains no aggregate rows. A stale or partial roster rejects instead of
/// silently understating coexistence demand.
pub fn compose_program_local_root_coexistence_report<'snapshot>(
    lifecycle: &ComponentEraEntryLedger,
    snapshots: impl IntoIterator<Item = &'snapshot ProgramLocalRootEpochAggregateSnapshot>,
) -> Result<ProgramLocalRootCoexistenceReport, ExternalRootDiagnostic> {
    let live_epochs = lifecycle
        .live_eras()
        .map(|(epoch, _, _)| epoch)
        .collect::<BTreeSet<_>>();
    let mut supplied_epochs = BTreeSet::new();
    let mut normalized = Vec::new();

    for snapshot in snapshots {
        let identity = snapshot.identity;
        if identity.lifecycle_ledger != lifecycle.identity() {
            return Err(ExternalRootDiagnostic(
                "program-local coexistence snapshot belongs to another lifecycle ledger".into(),
            ));
        }
        if !live_epochs.contains(&identity.lifecycle_epoch) {
            return Err(ExternalRootDiagnostic(
                "program-local coexistence snapshot belongs to a non-live lifecycle epoch".into(),
            ));
        }
        if !supplied_epochs.insert(identity.lifecycle_epoch) {
            return Err(ExternalRootDiagnostic(
                "program-local coexistence report repeats one lifecycle epoch".into(),
            ));
        }
        if snapshot.installed_required_slots.installed_code() != identity.installed_code {
            return Err(ExternalRootDiagnostic(
                "program-local coexistence snapshot substitutes its installed-code closure".into(),
            ));
        }

        let mut occurrence_identities = BTreeSet::new();
        for aggregate in &snapshot.aggregates {
            if aggregate.artifact != snapshot.installed_required_slots.artifact() {
                return Err(ExternalRootDiagnostic(
                    "program-local coexistence aggregate substitutes its installed artifact".into(),
                ));
            }
            for occurrence in &aggregate.occurrence_identities {
                if occurrence.prebinding.installed_code != identity.installed_code
                    || occurrence.lifecycle_ledger != identity.lifecycle_ledger
                    || occurrence.lifecycle_epoch != identity.lifecycle_epoch
                {
                    return Err(ExternalRootDiagnostic(
                        "program-local coexistence aggregate contains a cross-cohort occurrence"
                            .into(),
                    ));
                }
                if !occurrence_identities.insert(*occurrence) {
                    return Err(ExternalRootDiagnostic(
                        "program-local coexistence aggregate repeats one occurrence".into(),
                    ));
                }
            }
        }
        normalized.push(snapshot.clone());
    }

    if supplied_epochs != live_epochs {
        return Err(ExternalRootDiagnostic(
            "program-local coexistence report omits or adds a live lifecycle epoch".into(),
        ));
    }
    normalized.sort_by_key(|snapshot| snapshot.identity.lifecycle_epoch);
    Ok(ProgramLocalRootCoexistenceReport {
        lifecycle_ledger: lifecycle.identity(),
        epoch_snapshots: normalized,
    })
}
