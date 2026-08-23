use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use omega_effects::{
    ComponentEraEntryLedger, ComponentEraLedgerId, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseId,
};
use omega_executable_installation::{ArtifactId, InstalledCodeId};
use psi_core::{ContentAlgebra, ProgramLocalCapacityExpression};
use psi_terminal::TerminalPsiIdentity;
use psi_terminal_codec::VerifiedProgramLocalRootProducerCatalog;

use super::{
    ExternalRootDiagnostic, ExternalRootId, InstalledExternalRoot, ProviderExecutionId,
    RootAdmissionId, RootSlotId, RootSlotOwnerId, TerminalObjectEvidence, bind_terminal_function,
};

/// Exact, opaque address of one non-authoritative installed prebinding.
///
/// The tuple is the identity. A presentation hash is insufficient here: a
/// collision must not let one installed slot stand in for another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootPrebindingId {
    installed_code: InstalledCodeId,
    root: ExternalRootId,
    slot: RootSlotId,
    schema_identity: u64,
}

impl ProgramLocalRootPrebindingId {
    pub const fn installed_code(self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn root(self) -> ExternalRootId {
        self.root
    }

    pub const fn slot(self) -> RootSlotId {
        self.slot
    }

    pub const fn schema_identity(self) -> u64 {
        self.schema_identity
    }
}

/// Non-authoritative prebinding of one portable producer schema to one exact
/// installed environment-to-program slot occurrence.
///
/// This record deliberately does not carry a lifecycle epoch and cannot mint
/// content. It closes the installation facts already available today so a
/// later lifecycle join can consume one typed occurrence instead of repeating
/// requirement, provider, artifact, and slot matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootInstalledPrebinding {
    identity: ProgramLocalRootPrebindingId,
    terminal_psi: TerminalPsiIdentity,
    installed_root_evidence: super::InstalledRootEvidence,
    owner: RootSlotOwnerId,
    artifact: ArtifactId,
    admission: RootAdmissionId,
    provider_execution: ProviderExecutionId,
    requirement_identity: String,
    source_parameter_position: u32,
    qualification_identity: String,
    carrier_identity: String,
    projection: psi_core::ContentProjectionIdentity,
    algebra: ContentAlgebra,
    per_occurrence_capacity: ProgramLocalCapacityExpression,
}

impl ProgramLocalRootInstalledPrebinding {
    pub const fn identity(&self) -> ProgramLocalRootPrebindingId {
        self.identity
    }

    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn owner(&self) -> RootSlotOwnerId {
        self.owner
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn admission(&self) -> RootAdmissionId {
        self.admission
    }

    pub const fn provider_execution(&self) -> ProviderExecutionId {
        self.provider_execution
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
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

    pub const fn projection(&self) -> psi_core::ContentProjectionIdentity {
        self.projection
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn per_occurrence_capacity(&self) -> &ProgramLocalCapacityExpression {
        &self.per_occurrence_capacity
    }

    fn matches_root(&self, root: &InstalledExternalRoot<'_>) -> bool {
        self.installed_root_evidence == root.evidence
            && self.identity.installed_code == root.installed_code.identity()
            && self.identity.root == root.root
            && self.identity.slot == root.slot
            && self.owner == root.owner
            && self.artifact == root.installed_code.artifact()
            && self.admission == root.evidence.admission
            && self.provider_execution == root.evidence.provider_execution.identity
            && self.requirement_identity == root.evidence.root.candidate.requirement_identity
    }
}

/// Exact installed-slot count derived for one schema and one installed
/// artifact occurrence. It remains prebinding evidence, not an aggregate root:
/// lifecycle epoch and authority introduction are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootInstalledPrebindingCount {
    pub terminal_psi: TerminalPsiIdentity,
    pub installed_code: InstalledCodeId,
    pub artifact: ArtifactId,
    pub requirement_identity: String,
    pub source_parameter_position: u32,
    pub qualification_identity: String,
    pub carrier_identity: String,
    pub schema_identity: u64,
    pub algebra: ContentAlgebra,
    pub per_occurrence_capacity: ProgramLocalCapacityExpression,
    pub installed_slot_count: NonZeroU64,
    pub prebinding_identities: Vec<ProgramLocalRootPrebindingId>,
}

type CountKey = (u16, [u8; 32], InstalledCodeId, u64);
type LifecycleFamilyKey = (InstalledCodeId, u64);

/// Exact lifecycle-qualified identity of one installed occurrence. A later
/// epoch is intentionally a distinct origin even when it reuses the same code
/// and slot prebinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstalledProgramLocalRootOccurrenceId {
    prebinding: ProgramLocalRootPrebindingId,
    lifecycle_ledger: ComponentEraLedgerId,
    lifecycle_epoch: u64,
}

impl InstalledProgramLocalRootOccurrenceId {
    pub const fn prebinding(self) -> ProgramLocalRootPrebindingId {
        self.prebinding
    }

    pub const fn lifecycle_ledger(self) -> ComponentEraLedgerId {
        self.lifecycle_ledger
    }

    pub const fn lifecycle_epoch(self) -> u64 {
        self.lifecycle_epoch
    }
}

/// Exact installed slot plus its non-duplicable lifecycle hold.
///
/// This is the complete per-occurrence join, but it is deliberately not yet a
/// lineage source: installation still needs a sealed finite eligible cohort.
/// Borrowing the installed root pins both that slot and its InstalledCode;
/// owning the epoch lease prevents lifecycle quiescence and retirement.
#[derive(Debug)]
pub struct InstalledProgramLocalRootOccurrence<'root, 'code> {
    identity: InstalledProgramLocalRootOccurrenceId,
    prebinding: ProgramLocalRootInstalledPrebinding,
    root: &'root InstalledExternalRoot<'code>,
    epoch_lease: ProgramLocalRootEpochLease,
}

impl InstalledProgramLocalRootOccurrence<'_, '_> {
    pub const fn identity(&self) -> InstalledProgramLocalRootOccurrenceId {
        self.identity
    }

    pub const fn prebinding(&self) -> &ProgramLocalRootInstalledPrebinding {
        &self.prebinding
    }

    pub const fn epoch_lease_identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.epoch_lease.identity()
    }

    pub const fn installed_root(&self) -> &InstalledExternalRoot<'_> {
        self.root
    }
}

/// Installation-owned ledger for the non-minting occurrence prebinding.
#[derive(Debug, Default)]
pub struct ProgramLocalRootInstallationLedger {
    prebindings: BTreeMap<ProgramLocalRootPrebindingId, ProgramLocalRootInstalledPrebinding>,
    lifecycle_bindings: BTreeMap<LifecycleFamilyKey, ComponentEraLedgerId>,
    active_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
    used_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
}

impl ProgramLocalRootInstallationLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Join every authorized schema on the installed root's exact requirement
    /// to the live slot, provider execution, admission, and artifact
    /// occurrence. The join is transactional and replay rejecting.
    pub fn prebind<TerminalArtifact: TerminalObjectEvidence>(
        &mut self,
        catalog: &VerifiedProgramLocalRootProducerCatalog,
        terminal_artifact: &TerminalArtifact,
        root: &InstalledExternalRoot<'_>,
    ) -> Result<Vec<ProgramLocalRootInstalledPrebinding>, ExternalRootDiagnostic> {
        let terminal_psi = catalog.terminal_psi();
        if terminal_artifact.terminal_psi() != terminal_psi {
            return Err(ExternalRootDiagnostic(
                "program-local root catalog does not match the terminal artifact identity".into(),
            ));
        }
        let text_offset = terminal_artifact
            .function_text_offset(catalog.terminal_entry())
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "terminal artifact has no installed entry for the program-local root catalog"
                        .into(),
                )
            })?;
        bind_terminal_function(
            terminal_artifact,
            root.installed_code,
            root.evidence.root.candidate.entry,
            text_offset,
        )?;
        let requirement_identity = &root.evidence.root.candidate.requirement_identity;
        let schemas = catalog
            .schemas()
            .iter()
            .filter(|schema| schema.boundary_requirement_identity() == requirement_identity)
            .collect::<Vec<_>>();
        let mut pending = Vec::with_capacity(schemas.len());
        let mut local_schema_keys = BTreeSet::new();
        for verified_schema in schemas {
            let schema = verified_schema.schema();
            if root
                .evidence
                .root
                .boundary
                .plan()
                .call
                .parameters
                .get(schema.argument_index as usize)
                .is_none()
                || !root
                    .evidence
                    .root
                    .candidate
                    .entry_claims
                    .iter()
                    .any(|claim| {
                        claim.parameter_index == schema.argument_index as usize
                            && claim.domain == verified_schema.qualification_identity()
                    })
            {
                return Err(ExternalRootDiagnostic(
                    "program-local root schema does not match an exact installed entry claim and ABI position"
                        .into(),
                ));
            }
            if !local_schema_keys.insert((schema.source_parameter_position, schema.identity)) {
                return Err(ExternalRootDiagnostic(
                    "program-local root producer schemas repeat one semantic occurrence".into(),
                ));
            }
            let identity = ProgramLocalRootPrebindingId {
                installed_code: root.installed_code.identity(),
                root: root.root,
                slot: root.slot,
                schema_identity: schema.identity,
            };
            if self.prebindings.contains_key(&identity) {
                return Err(ExternalRootDiagnostic(
                    "program-local root installed occurrence was already prebound".into(),
                ));
            }
            pending.push((
                identity,
                ProgramLocalRootInstalledPrebinding {
                    identity,
                    terminal_psi,
                    installed_root_evidence: root.evidence.clone(),
                    owner: root.owner,
                    artifact: root.installed_code.artifact(),
                    admission: root.evidence.admission,
                    provider_execution: root.evidence.provider_execution.identity,
                    requirement_identity: requirement_identity.clone(),
                    source_parameter_position: schema.source_parameter_position,
                    qualification_identity: verified_schema.qualification_identity().to_owned(),
                    carrier_identity: verified_schema.carrier_identity().to_owned(),
                    projection: schema.projection,
                    algebra: schema.algebra.clone(),
                    per_occurrence_capacity: schema.capacity.clone(),
                },
            ));
        }

        let joined = pending
            .iter()
            .map(|(_, occurrence)| occurrence.clone())
            .collect();
        for (key, occurrence) in pending {
            self.prebindings.insert(key, occurrence);
        }
        Ok(joined)
    }

    pub fn prebindings(&self) -> impl Iterator<Item = &ProgramLocalRootInstalledPrebinding> {
        self.prebindings.values()
    }

    /// Count distinct prebound slots from ledger state. No producer-authored
    /// cardinality or aggregate is accepted.
    pub fn counts(&self) -> Vec<ProgramLocalRootInstalledPrebindingCount> {
        let mut groups: BTreeMap<
            CountKey,
            (
                ProgramLocalRootInstalledPrebinding,
                Vec<ProgramLocalRootPrebindingId>,
            ),
        > = BTreeMap::new();
        for occurrence in self.prebindings.values() {
            let key = (
                occurrence.terminal_psi.vocabulary_marker.get(),
                *occurrence.terminal_psi.program_fingerprint.as_bytes(),
                occurrence.identity.installed_code,
                occurrence.identity.schema_identity,
            );
            groups
                .entry(key)
                .and_modify(|(_, identities)| identities.push(occurrence.identity))
                .or_insert_with(|| (occurrence.clone(), vec![occurrence.identity]));
        }
        groups
            .into_values()
            .map(|(occurrence, mut identities)| {
                identities.sort_unstable();
                ProgramLocalRootInstalledPrebindingCount {
                    terminal_psi: occurrence.terminal_psi,
                    installed_code: occurrence.identity.installed_code,
                    artifact: occurrence.artifact,
                    requirement_identity: occurrence.requirement_identity,
                    source_parameter_position: occurrence.source_parameter_position,
                    qualification_identity: occurrence.qualification_identity,
                    carrier_identity: occurrence.carrier_identity,
                    schema_identity: occurrence.identity.schema_identity,
                    algebra: occurrence.algebra,
                    per_occurrence_capacity: occurrence.per_occurrence_capacity,
                    installed_slot_count: NonZeroU64::new(
                        u64::try_from(identities.len())
                            .expect("installed occurrence count fits u64"),
                    )
                    .expect("count group is nonempty"),
                    prebinding_identities: identities,
                }
            })
            .collect()
    }

    /// Bind one canonical prebinding to an exact lifecycle occurrence.
    ///
    /// Rejection returns the non-clonable lease intact. A successful join is
    /// unique per prebinding, lifecycle ledger, and era; retirement does not
    /// make that same origin reusable.
    pub fn join<'root, 'code>(
        &mut self,
        prebinding: ProgramLocalRootPrebindingId,
        root: &'root InstalledExternalRoot<'code>,
        lifecycle: &ComponentEraEntryLedger,
        epoch_lease: ProgramLocalRootEpochLease,
    ) -> Result<InstalledProgramLocalRootOccurrence<'root, 'code>, ProgramLocalRootJoinError> {
        let reject = |epoch_lease, diagnostic: &str| ProgramLocalRootJoinError {
            prebinding,
            epoch_lease,
            diagnostic: ExternalRootDiagnostic(diagnostic.into()),
        };
        let Some(canonical) = self.prebindings.get(&prebinding).cloned() else {
            return Err(reject(
                epoch_lease,
                "program-local root join names no canonical installed prebinding",
            ));
        };
        if lifecycle
            .validate_program_local_root_epoch_lease(&epoch_lease)
            .is_err()
        {
            return Err(reject(
                epoch_lease,
                "program-local root lifecycle lease is not live in the exact current open era",
            ));
        }
        if !canonical.matches_root(root) {
            return Err(reject(
                epoch_lease,
                "program-local root join substituted the exact installed root occurrence",
            ));
        }
        if epoch_lease.entry_contract_identity() != canonical.requirement_identity
            || epoch_lease.artifact_instance_identity()
                != canonical.identity.installed_code.normalized_identity()
        {
            return Err(reject(
                epoch_lease,
                "program-local root lifecycle lease does not bind the exact requirement and installed artifact occurrence",
            ));
        }
        let lifecycle_family = (
            canonical.identity.installed_code,
            canonical.identity.schema_identity,
        );
        if self
            .lifecycle_bindings
            .get(&lifecycle_family)
            .is_some_and(|ledger| *ledger != epoch_lease.ledger())
        {
            return Err(reject(
                epoch_lease,
                "program-local root prebinding family is already bound to another lifecycle ledger",
            ));
        }
        let identity = InstalledProgramLocalRootOccurrenceId {
            prebinding,
            lifecycle_ledger: epoch_lease.ledger(),
            lifecycle_epoch: epoch_lease.era_identity(),
        };
        if self.active_occurrences.contains(&identity) || self.used_occurrences.contains(&identity)
        {
            return Err(reject(
                epoch_lease,
                "program-local root installed occurrence already joined in this lifecycle epoch",
            ));
        }
        self.lifecycle_bindings
            .entry(lifecycle_family)
            .or_insert(identity.lifecycle_ledger);
        self.active_occurrences.insert(identity);
        Ok(InstalledProgramLocalRootOccurrence {
            identity,
            prebinding: canonical,
            root,
            epoch_lease,
        })
    }

    /// Release one exact lifecycle-pinned occurrence. The replay key remains
    /// consumed after success, while a failed lifecycle release reconstructs
    /// and returns the complete occurrence.
    pub fn retire<'root, 'code>(
        &mut self,
        occurrence: InstalledProgramLocalRootOccurrence<'root, 'code>,
        lifecycle: &mut ComponentEraEntryLedger,
    ) -> Result<
        RetiredProgramLocalRootOccurrence,
        Box<ProgramLocalRootOccurrenceRetirementError<'root, 'code>>,
    > {
        if !self.active_occurrences.contains(&occurrence.identity)
            || self
                .prebindings
                .get(&occurrence.identity.prebinding)
                .is_none_or(|canonical| canonical != &occurrence.prebinding)
            || !occurrence.prebinding.matches_root(occurrence.root)
        {
            return Err(Box::new(ProgramLocalRootOccurrenceRetirementError {
                occurrence,
                diagnostic: ExternalRootDiagnostic(
                    "program-local root retirement substituted the exact active occurrence".into(),
                ),
            }));
        }

        let InstalledProgramLocalRootOccurrence {
            identity,
            prebinding,
            root,
            epoch_lease,
        } = occurrence;
        let lease_identity = epoch_lease.identity();
        if let Err(error) = lifecycle.release_program_local_root_epoch_lease(epoch_lease) {
            return Err(Box::new(ProgramLocalRootOccurrenceRetirementError {
                occurrence: InstalledProgramLocalRootOccurrence {
                    identity,
                    prebinding,
                    root,
                    epoch_lease: error.into_lease(),
                },
                diagnostic: ExternalRootDiagnostic(
                    "program-local root retirement could not release the exact lifecycle lease"
                        .into(),
                ),
            }));
        }

        let removed = self.active_occurrences.remove(&identity);
        debug_assert!(removed, "validated active occurrence remains present");
        let fresh = self.used_occurrences.insert(identity);
        debug_assert!(fresh, "active occurrence was not already retired");
        Ok(RetiredProgramLocalRootOccurrence {
            identity,
            epoch_lease: lease_identity,
        })
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootJoinError {
    prebinding: ProgramLocalRootPrebindingId,
    epoch_lease: ProgramLocalRootEpochLease,
    diagnostic: ExternalRootDiagnostic,
}

impl ProgramLocalRootJoinError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ProgramLocalRootPrebindingId, ProgramLocalRootEpochLease) {
        (self.prebinding, self.epoch_lease)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetiredProgramLocalRootOccurrence {
    identity: InstalledProgramLocalRootOccurrenceId,
    epoch_lease: ProgramLocalRootEpochLeaseId,
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
    occurrence: InstalledProgramLocalRootOccurrence<'root, 'code>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'root, 'code> ProgramLocalRootOccurrenceRetirementError<'root, 'code> {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_occurrence(self) -> InstalledProgramLocalRootOccurrence<'root, 'code> {
        self.occurrence
    }
}
