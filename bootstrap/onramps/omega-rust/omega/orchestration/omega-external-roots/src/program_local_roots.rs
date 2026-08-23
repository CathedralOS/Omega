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
    ExternalRootDiagnostic, ExternalRootId, InstalledExternalRoot,
    InstalledRequiredRootSlotClosure, InstalledRootLedger, ProviderExecutionId, RootAdmissionId,
    RootSlotId, RootSlotOwnerId, TerminalObjectEvidence, bind_terminal_function,
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
#[derive(Debug)]
pub struct ProgramLocalRootInstallationLedger {
    installed_required_slots: InstalledRequiredRootSlotClosure,
    prebindings: BTreeMap<ProgramLocalRootPrebindingId, ProgramLocalRootInstalledPrebinding>,
    prebindings_frozen: bool,
    lifecycle_bindings: BTreeMap<LifecycleFamilyKey, ComponentEraLedgerId>,
    active_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
    used_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
    sealed_epoch_cohorts: BTreeSet<(ComponentEraLedgerId, u64)>,
}

impl ProgramLocalRootInstallationLedger {
    fn from_installed_required_slots(
        installed_required_slots: InstalledRequiredRootSlotClosure,
    ) -> Self {
        Self {
            installed_required_slots,
            prebindings: BTreeMap::new(),
            prebindings_frozen: false,
            lifecycle_bindings: BTreeMap::new(),
            active_occurrences: BTreeSet::new(),
            used_occurrences: BTreeSet::new(),
            sealed_epoch_cohorts: BTreeSet::new(),
        }
    }

    pub const fn installed_required_slots(&self) -> &InstalledRequiredRootSlotClosure {
        &self.installed_required_slots
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
        if self.prebindings_frozen {
            return Err(ExternalRootDiagnostic(
                "program-local root prebindings are frozen after epoch-cohort sealing".into(),
            ));
        }
        let Some(installed_slot) = self.installed_required_slots.slot(root.slot) else {
            return Err(ExternalRootDiagnostic(
                "program-local root prebinding names a slot outside the sealed required closure"
                    .into(),
            ));
        };
        if self.installed_required_slots.installed_code() != root.installed_code.identity()
            || !installed_slot.matches_root(root)
        {
            return Err(ExternalRootDiagnostic(
                "program-local root prebinding substituted the installed required root".into(),
            ));
        }
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

    /// Atomically close every eligible prebinding into one exact lifecycle
    /// cohort. Rejection returns every non-clonable lease intact; no member is
    /// committed until the complete expected set validates.
    pub fn seal_epoch_cohort<'root, 'code>(
        &mut self,
        lifecycle: &ComponentEraEntryLedger,
        members: impl IntoIterator<Item = ProgramLocalRootCohortMember<'root, 'code>>,
    ) -> Result<
        InstalledProgramLocalRootEpochCohort<'root, 'code>,
        Box<ProgramLocalRootCohortSealError<'root, 'code>>,
    > {
        let members = members.into_iter().collect::<Vec<_>>();
        let reject = |members, diagnostic: &str| {
            Err(Box::new(ProgramLocalRootCohortSealError {
                members,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };
        let Some(lifecycle_epoch) = lifecycle.current_era() else {
            return reject(
                members,
                "program-local root cohort has no current lifecycle epoch",
            );
        };
        let cohort_key = (lifecycle.identity(), lifecycle_epoch);
        if self.sealed_epoch_cohorts.contains(&cohort_key) {
            return reject(
                members,
                "program-local root epoch cohort was already sealed",
            );
        }

        let expected = self.prebindings.keys().copied().collect::<BTreeSet<_>>();
        let supplied = members
            .iter()
            .map(|member| member.prebinding)
            .collect::<BTreeSet<_>>();
        if supplied.len() != members.len() {
            return reject(
                members,
                "program-local root epoch cohort repeats one prebinding",
            );
        }
        if supplied != expected {
            return reject(
                members,
                "program-local root epoch cohort omits or adds an eligible prebinding",
            );
        }

        let mut validated = Vec::with_capacity(members.len());
        for member in &members {
            let Some(canonical) = self.prebindings.get(&member.prebinding).cloned() else {
                return reject(
                    members,
                    "program-local root cohort names no canonical installed prebinding",
                );
            };
            if lifecycle
                .validate_program_local_root_epoch_lease(&member.epoch_lease)
                .is_err()
                || member.epoch_lease.ledger() != lifecycle.identity()
                || member.epoch_lease.era_identity() != lifecycle_epoch
            {
                return reject(
                    members,
                    "program-local root cohort lease is not live in the exact current epoch ledger",
                );
            }
            if !canonical.matches_root(member.root)
                || self
                    .installed_required_slots
                    .slot(member.root.slot)
                    .is_none_or(|slot| !slot.matches_root(member.root))
            {
                return reject(
                    members,
                    "program-local root cohort substituted the exact installed required root",
                );
            }
            if member.epoch_lease.entry_contract_identity() != canonical.requirement_identity
                || member.epoch_lease.artifact_instance_identity()
                    != canonical.identity.installed_code.normalized_identity()
            {
                return reject(
                    members,
                    "program-local root cohort lease does not bind the exact requirement and installed artifact occurrence",
                );
            }
            let lifecycle_family = (
                canonical.identity.installed_code,
                canonical.identity.schema_identity,
            );
            if self
                .lifecycle_bindings
                .get(&lifecycle_family)
                .is_some_and(|ledger| *ledger != lifecycle.identity())
            {
                return reject(
                    members,
                    "program-local root prebinding family is already bound to another lifecycle ledger",
                );
            }
            let identity = InstalledProgramLocalRootOccurrenceId {
                prebinding: member.prebinding,
                lifecycle_ledger: lifecycle.identity(),
                lifecycle_epoch,
            };
            if self.active_occurrences.contains(&identity)
                || self.used_occurrences.contains(&identity)
            {
                return reject(
                    members,
                    "program-local root occurrence was already committed in this lifecycle epoch",
                );
            }
            validated.push((canonical, lifecycle_family, identity));
        }

        for (_, lifecycle_family, identity) in &validated {
            self.lifecycle_bindings
                .entry(*lifecycle_family)
                .or_insert(identity.lifecycle_ledger);
            let fresh = self.active_occurrences.insert(*identity);
            debug_assert!(fresh, "validated cohort occurrence is new");
        }
        self.prebindings_frozen = true;
        let fresh = self.sealed_epoch_cohorts.insert(cohort_key);
        debug_assert!(fresh, "validated epoch cohort is new");

        let occurrences = members
            .into_iter()
            .zip(validated)
            .map(
                |(member, (prebinding, _, identity))| InstalledProgramLocalRootOccurrence {
                    identity,
                    prebinding,
                    root: member.root,
                    epoch_lease: member.epoch_lease,
                },
            )
            .collect::<Vec<_>>();
        let mut aggregate_groups: BTreeMap<
            CountKey,
            (
                ProgramLocalRootInstalledPrebinding,
                Vec<InstalledProgramLocalRootOccurrenceId>,
            ),
        > = BTreeMap::new();
        for occurrence in &occurrences {
            let prebinding = &occurrence.prebinding;
            let key = (
                prebinding.terminal_psi.vocabulary_marker.get(),
                *prebinding.terminal_psi.program_fingerprint.as_bytes(),
                prebinding.identity.installed_code,
                prebinding.identity.schema_identity,
            );
            aggregate_groups
                .entry(key)
                .and_modify(|(_, identities)| identities.push(occurrence.identity))
                .or_insert_with(|| (prebinding.clone(), vec![occurrence.identity]));
        }
        let aggregates = aggregate_groups
            .into_values()
            .map(|(prebinding, mut occurrence_identities)| {
                occurrence_identities.sort_unstable();
                ProgramLocalRootEpochAggregate {
                    terminal_psi: prebinding.terminal_psi,
                    artifact: prebinding.artifact,
                    requirement_identity: prebinding.requirement_identity,
                    source_parameter_position: prebinding.source_parameter_position,
                    qualification_identity: prebinding.qualification_identity,
                    carrier_identity: prebinding.carrier_identity,
                    schema_identity: prebinding.identity.schema_identity,
                    algebra: prebinding.algebra,
                    per_occurrence_capacity: prebinding.per_occurrence_capacity,
                    occurrence_identities,
                }
            })
            .collect();
        Ok(InstalledProgramLocalRootEpochCohort {
            identity: InstalledProgramLocalRootEpochCohortId {
                installed_code: self.installed_required_slots.installed_code(),
                lifecycle_ledger: lifecycle.identity(),
                lifecycle_epoch,
            },
            installed_required_slots: self.installed_required_slots.clone(),
            occurrences,
            aggregates,
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

impl InstalledRootLedger {
    /// Issue the sole program-local cohort verifier for this installation.
    /// The exact required-slot closure must already be sealed. Burning the
    /// claim prevents fresh ledgers from replaying the same occurrence keys.
    pub fn claim_program_local_root_installation_ledger(
        &mut self,
    ) -> Result<ProgramLocalRootInstallationLedger, ExternalRootDiagnostic> {
        if self.program_local_root_cohort_claimed {
            return Err(ExternalRootDiagnostic(
                "program-local root cohort verifier was already issued for this installation"
                    .into(),
            ));
        }
        let installed_required_slots = self.required_root_slots.clone().ok_or_else(|| {
            ExternalRootDiagnostic(
                "program-local root cohort verifier requires a sealed required root-slot closure"
                    .into(),
            )
        })?;
        self.program_local_root_cohort_claimed = true;
        Ok(
            ProgramLocalRootInstallationLedger::from_installed_required_slots(
                installed_required_slots,
            ),
        )
    }
}

/// Non-authoritative inputs for one member of an epoch cohort. Construction
/// packages existing custody only; no occurrence exists until the complete
/// cohort is validated and committed atomically.
#[derive(Debug)]
pub struct ProgramLocalRootCohortMember<'root, 'code> {
    prebinding: ProgramLocalRootPrebindingId,
    root: &'root InstalledExternalRoot<'code>,
    epoch_lease: ProgramLocalRootEpochLease,
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
    installed_code: InstalledCodeId,
    lifecycle_ledger: ComponentEraLedgerId,
    lifecycle_epoch: u64,
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
    terminal_psi: TerminalPsiIdentity,
    artifact: ArtifactId,
    requirement_identity: String,
    source_parameter_position: u32,
    qualification_identity: String,
    carrier_identity: String,
    schema_identity: u64,
    algebra: ContentAlgebra,
    per_occurrence_capacity: ProgramLocalCapacityExpression,
    occurrence_identities: Vec<InstalledProgramLocalRootOccurrenceId>,
}

impl ProgramLocalRootEpochAggregate {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
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

    pub const fn schema_identity(&self) -> u64 {
        self.schema_identity
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn per_occurrence_capacity(&self) -> &ProgramLocalCapacityExpression {
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

/// Non-clonable, exact program-local occurrence cohort for one installed
/// artifact and lifecycle epoch. This is the first carrier from which runtime
/// subject/capacity establishment may eventually derive lineage.
#[derive(Debug)]
pub struct InstalledProgramLocalRootEpochCohort<'root, 'code> {
    identity: InstalledProgramLocalRootEpochCohortId,
    installed_required_slots: InstalledRequiredRootSlotClosure,
    occurrences: Vec<InstalledProgramLocalRootOccurrence<'root, 'code>>,
    aggregates: Vec<ProgramLocalRootEpochAggregate>,
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

    /// Explicitly reopen per-occurrence custody for retirement or the future
    /// runtime establishment transition. The ledger keeps the epoch cohort
    /// replay key sealed permanently.
    pub fn into_occurrences(self) -> Vec<InstalledProgramLocalRootOccurrence<'root, 'code>> {
        self.occurrences
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootCohortSealError<'root, 'code> {
    members: Vec<ProgramLocalRootCohortMember<'root, 'code>>,
    diagnostic: ExternalRootDiagnostic,
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
