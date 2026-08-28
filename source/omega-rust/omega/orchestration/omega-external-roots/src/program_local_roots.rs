use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use omega_effects::{
    ComponentEraEntryLedger, ComponentEraLedgerId, ProgramLocalRootEpochLease,
    ProgramLocalRootEpochLeaseId,
};
use omega_executable_installation::{ArtifactId, InstalledCodeId};
use psi_core::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use psi_language_semantics::content::{CanonicalIntervalSet, NaturalInterval};
use psi_numerics::bignum::BigInt;
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
    argument_index: u32,
    source_parameter_position: u32,
    qualification_identity: String,
    carrier_identity: String,
    projection: psi_core::ContentProjectionIdentity,
    algebra: ContentAlgebra,
    per_occurrence_capacity: ContentProjectionExpression,
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

    pub const fn projection(&self) -> psi_core::ContentProjectionIdentity {
        self.projection
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn per_occurrence_capacity(&self) -> &ContentProjectionExpression {
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
    pub argument_index: u32,
    pub source_parameter_position: u32,
    pub qualification_identity: String,
    pub carrier_identity: String,
    pub schema_identity: u64,
    pub algebra: ContentAlgebra,
    pub per_occurrence_capacity: ContentProjectionExpression,
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

/// Report identity for one concrete activation of an installed entry bridge.
///
/// This number is not authority. The non-clonable epoch runtime and the exact
/// installed-root evidence retained by [`InstalledProgramLocalRootSubject`]
/// are what make an activation eligible to establish a root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootEntryInvocationId(u64);

impl ProgramLocalRootEntryInvocationId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExternalRootDiagnostic> {
        if identity == 0 {
            return Err(ExternalRootDiagnostic(
                "normalized program-local entry invocation identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Report identity of the exact runtime place occupying one installed entry
/// parameter. It distinguishes activations and places but carries no authority
/// by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootSubjectPlaceId(u64);

impl ProgramLocalRootSubjectPlaceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExternalRootDiagnostic> {
        if identity == 0 {
            return Err(ExternalRootDiagnostic(
                "normalized program-local subject-place identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Which compiler-checked scalar projection supplies one symbolic capacity
/// leaf. The distinction is semantic even when two leaves use the same field
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgramLocalRootScalarSource {
    SubjectField,
    RuntimeScalarEmbedding,
}

/// One bridge-observed proof-natural scalar used to instantiate the verified
/// per-occurrence capacity expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLocalRootScalarBinding {
    source: ProgramLocalRootScalarSource,
    path: Vec<String>,
    value: BigInt,
}

impl ProgramLocalRootScalarBinding {
    pub fn subject_field(
        path: impl IntoIterator<Item = impl Into<String>>,
        value: BigInt,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::new(ProgramLocalRootScalarSource::SubjectField, path, value)
    }

    pub fn runtime_scalar_embedding(
        path: impl IntoIterator<Item = impl Into<String>>,
        value: BigInt,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::new(
            ProgramLocalRootScalarSource::RuntimeScalarEmbedding,
            path,
            value,
        )
    }

    fn new(
        source: ProgramLocalRootScalarSource,
        path: impl IntoIterator<Item = impl Into<String>>,
        value: BigInt,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let path = path.into_iter().map(Into::into).collect::<Vec<_>>();
        if path.is_empty() || path.iter().any(String::is_empty) {
            return Err(ExternalRootDiagnostic(
                "program-local capacity scalar path must contain only nonempty segments".into(),
            ));
        }
        if value.is_negative() {
            return Err(ExternalRootDiagnostic(
                "program-local capacity scalar observation must be a proof-natural".into(),
            ));
        }
        Ok(Self {
            source,
            path,
            value,
        })
    }

    pub const fn source(&self) -> ProgramLocalRootScalarSource {
        self.source
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }

    pub const fn value(&self) -> &BigInt {
        &self.value
    }
}

type ProgramLocalRootScalarKey = (ProgramLocalRootScalarSource, Vec<String>);

/// Single-use subject observation emitted by a generated installed-entry
/// bridge. It borrows the exact installed root and records the semantic and ABI
/// parameter positions; an ordinary call has no such installed-root binding.
#[derive(Debug)]
pub struct InstalledProgramLocalRootSubject<'root, 'code> {
    root: &'root InstalledExternalRoot<'code>,
    invocation: ProgramLocalRootEntryInvocationId,
    argument_index: u32,
    source_parameter_position: u32,
    qualification_identity: String,
    carrier_identity: String,
    subject_place: ProgramLocalRootSubjectPlaceId,
    scalars: BTreeMap<ProgramLocalRootScalarKey, BigInt>,
}

impl<'root, 'code> InstalledProgramLocalRootSubject<'root, 'code> {
    #[allow(clippy::too_many_arguments)]
    pub fn from_generated_entry(
        root: &'root InstalledExternalRoot<'code>,
        invocation: ProgramLocalRootEntryInvocationId,
        argument_index: u32,
        source_parameter_position: u32,
        qualification_identity: impl Into<String>,
        carrier_identity: impl Into<String>,
        subject_place: ProgramLocalRootSubjectPlaceId,
        scalars: impl IntoIterator<Item = ProgramLocalRootScalarBinding>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let qualification_identity = qualification_identity.into();
        let carrier_identity = carrier_identity.into();
        if qualification_identity.is_empty() || carrier_identity.is_empty() {
            return Err(ExternalRootDiagnostic(
                "program-local installed subject requires nonempty qualification and carrier identities"
                    .into(),
            ));
        }
        if root
            .evidence
            .root
            .boundary
            .plan()
            .call
            .parameters
            .get(argument_index as usize)
            .is_none()
            || !root
                .evidence
                .root
                .candidate
                .entry_claims
                .iter()
                .any(|claim| {
                    claim.parameter_index == argument_index as usize
                        && claim.domain == qualification_identity
                })
        {
            return Err(ExternalRootDiagnostic(
                "program-local installed subject does not name an exact qualified entry ABI parameter"
                    .into(),
            ));
        }
        let mut scalar_map = BTreeMap::new();
        for scalar in scalars {
            let key = (scalar.source, scalar.path);
            if scalar_map.insert(key, scalar.value).is_some() {
                return Err(ExternalRootDiagnostic(
                    "program-local installed subject repeats one capacity scalar observation"
                        .into(),
                ));
            }
        }
        Ok(Self {
            root,
            invocation,
            argument_index,
            source_parameter_position,
            qualification_identity,
            carrier_identity,
            subject_place,
            scalars: scalar_map,
        })
    }

    pub const fn invocation(&self) -> ProgramLocalRootEntryInvocationId {
        self.invocation
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

    pub const fn subject_place(&self) -> ProgramLocalRootSubjectPlaceId {
        self.subject_place
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
    established_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
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
            established_occurrences: BTreeSet::new(),
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
                    argument_index: schema.argument_index,
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
                    argument_index: occurrence.argument_index,
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
                    argument_index: prebinding.argument_index,
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

    /// Establish one exact pending cohort member from a generated installed-entry
    /// subject. Every symbolic scalar is replayed against the verified schema
    /// before the occurrence is removed from the runtime, so rejection returns
    /// the complete subject binding and mints no lineage.
    pub fn establish<'root, 'subject, 'code>(
        &mut self,
        runtime: &mut ProgramLocalRootEpochRuntime<'root, 'code>,
        lifecycle: &ComponentEraEntryLedger,
        subject: InstalledProgramLocalRootSubject<'subject, 'code>,
    ) -> Result<
        EstablishedProgramLocalRoot<'root, 'code>,
        Box<ProgramLocalRootEstablishmentError<'subject, 'code>>,
    > {
        match self.establish_batch(runtime, lifecycle, [subject]) {
            Ok(mut roots) => Ok(roots
                .pop()
                .expect("one subject establishes exactly one program-local root")),
            Err(error) => {
                let diagnostic = error.diagnostic().clone();
                let [subject]: [InstalledProgramLocalRootSubject<'subject, 'code>; 1] = error
                    .into_subjects()
                    .try_into()
                    .expect("one-subject establishment returns exactly one subject");
                Err(Box::new(ProgramLocalRootEstablishmentError {
                    subject,
                    diagnostic,
                }))
            }
        }
    }

    /// Establish a finite set of installed-entry subjects in one transaction.
    /// Every member, scalar roster, evaluated capacity, and lifecycle lease is
    /// validated before any pending occurrence is removed. Rejection returns
    /// every subject in source order and leaves the epoch runtime unchanged.
    pub fn establish_batch<'root, 'subject, 'code>(
        &mut self,
        runtime: &mut ProgramLocalRootEpochRuntime<'root, 'code>,
        lifecycle: &ComponentEraEntryLedger,
        subjects: impl IntoIterator<Item = InstalledProgramLocalRootSubject<'subject, 'code>>,
    ) -> Result<
        Vec<EstablishedProgramLocalRoot<'root, 'code>>,
        Box<ProgramLocalRootBatchEstablishmentError<'subject, 'code>>,
    > {
        let subjects = subjects.into_iter().collect::<Vec<_>>();
        let reject = |subjects, diagnostic: &str| {
            Err(Box::new(ProgramLocalRootBatchEstablishmentError {
                subjects,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };
        if subjects.is_empty() {
            return reject(
                subjects,
                "program-local batch establishment requires at least one installed subject",
            );
        }
        if runtime.installed_required_slots != self.installed_required_slots
            || runtime.identity.installed_code() != self.installed_required_slots.installed_code()
        {
            return reject(
                subjects,
                "program-local runtime does not belong to this exact installation ledger",
            );
        }
        if lifecycle.identity() != runtime.identity.lifecycle_ledger()
            || lifecycle.current_era() != Some(runtime.identity.lifecycle_epoch())
        {
            return reject(
                subjects,
                "program-local establishment is not executing in the exact current lifecycle epoch",
            );
        }

        let mut selected = BTreeSet::new();
        let mut validated = Vec::with_capacity(subjects.len());
        for subject in &subjects {
            let matches = runtime
                .pending
                .values()
                .filter(|occurrence| {
                    occurrence.prebinding.matches_root(subject.root)
                        && occurrence.prebinding.argument_index == subject.argument_index
                        && occurrence.prebinding.source_parameter_position
                            == subject.source_parameter_position
                        && occurrence.prebinding.qualification_identity
                            == subject.qualification_identity
                        && occurrence.prebinding.carrier_identity == subject.carrier_identity
                })
                .map(|occurrence| occurrence.identity)
                .collect::<Vec<_>>();
            let [identity] = matches.as_slice() else {
                return reject(
                    subjects,
                    if matches.is_empty() {
                        "program-local installed subject matches no pending exact cohort occurrence"
                    } else {
                        "program-local installed subject ambiguously matches several pending cohort occurrences"
                    },
                );
            };
            let identity = *identity;
            if !selected.insert(identity) {
                return reject(
                    subjects,
                    "program-local batch repeats one exact pending cohort occurrence",
                );
            }
            let occurrence = runtime
                .pending
                .get(&identity)
                .expect("matching occurrence remains pending during validation");
            if !self.active_occurrences.contains(&identity)
                || self.established_occurrences.contains(&identity)
                || lifecycle
                    .validate_program_local_root_epoch_lease(&occurrence.epoch_lease)
                    .is_err()
                || occurrence.epoch_lease.ledger() != lifecycle.identity()
                || occurrence.epoch_lease.era_identity() != runtime.identity.lifecycle_epoch()
            {
                return reject(
                    subjects,
                    "program-local cohort occurrence is not a live unestablished member of this epoch",
                );
            }

            let expected_scalars =
                capacity_scalar_keys(&occurrence.prebinding.per_occurrence_capacity);
            let supplied_scalars = subject.scalars.keys().cloned().collect::<BTreeSet<_>>();
            if expected_scalars != supplied_scalars {
                return reject(
                    subjects,
                    "program-local installed subject omits or adds a verified capacity scalar",
                );
            }
            let capacity = match evaluate_capacity(
                &occurrence.prebinding.per_occurrence_capacity,
                &subject.scalars,
            ) {
                Ok(capacity) => capacity,
                Err(diagnostic) => {
                    return Err(Box::new(ProgramLocalRootBatchEstablishmentError {
                        subjects,
                        diagnostic,
                    }));
                }
            };
            if !capacity_matches_algebra(&capacity, &occurrence.prebinding.algebra) {
                return reject(
                    subjects,
                    "program-local evaluated capacity does not match its verified content algebra",
                );
            }
            validated.push((identity, capacity));
        }

        Ok(subjects
            .into_iter()
            .zip(validated)
            .map(|(subject, (identity, capacity))| {
                let InstalledProgramLocalRootSubject {
                    root: _,
                    invocation,
                    argument_index: _,
                    source_parameter_position: _,
                    qualification_identity: _,
                    carrier_identity: _,
                    subject_place,
                    scalars,
                } = subject;
                let occurrence = runtime
                    .pending
                    .remove(&identity)
                    .expect("validated occurrence remains pending until the batch commit point");
                let fresh = self.established_occurrences.insert(identity);
                debug_assert!(fresh, "validated occurrence establishes exactly once");
                EstablishedProgramLocalRoot {
                    occurrence,
                    invocation,
                    subject_place,
                    scalar_observations: scalars,
                    capacity,
                }
            })
            .collect())
    }

    /// Retire one established root and release its exact lifecycle hold. A
    /// failed release reconstructs and returns the complete root account.
    pub fn retire_established<'root, 'code>(
        &mut self,
        root: EstablishedProgramLocalRoot<'root, 'code>,
        lifecycle: &mut ComponentEraEntryLedger,
    ) -> Result<RetiredProgramLocalRootOccurrence, Box<ProgramLocalRootRetirementError<'root, 'code>>>
    {
        let EstablishedProgramLocalRoot {
            occurrence,
            invocation,
            subject_place,
            scalar_observations,
            capacity,
        } = root;
        let identity = occurrence.identity;
        if !self.established_occurrences.contains(&identity) {
            return Err(Box::new(ProgramLocalRootRetirementError {
                root: EstablishedProgramLocalRoot {
                    occurrence,
                    invocation,
                    subject_place,
                    scalar_observations,
                    capacity,
                },
                diagnostic: ExternalRootDiagnostic(
                    "program-local root retirement names no exact established occurrence".into(),
                ),
            }));
        }
        match self.retire(occurrence, lifecycle) {
            Ok(retired) => {
                let removed = self.established_occurrences.remove(&identity);
                debug_assert!(removed, "retired established occurrence remains recorded");
                Ok(retired)
            }
            Err(error) => Err(Box::new(ProgramLocalRootRetirementError {
                root: EstablishedProgramLocalRoot {
                    occurrence: (*error).into_occurrence(),
                    invocation,
                    subject_place,
                    scalar_observations,
                    capacity,
                },
                diagnostic: ExternalRootDiagnostic(
                    "program-local established root could not release its exact lifecycle lease"
                        .into(),
                ),
            })),
        }
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

fn capacity_scalar_keys(
    expression: &ContentProjectionExpression,
) -> BTreeSet<ProgramLocalRootScalarKey> {
    fn visit(scalar: &ContentProjectionScalar, keys: &mut BTreeSet<ProgramLocalRootScalarKey>) {
        match scalar {
            ContentProjectionScalar::SubjectField(path) => {
                keys.insert((ProgramLocalRootScalarSource::SubjectField, path.clone()));
            }
            ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                keys.insert((
                    ProgramLocalRootScalarSource::RuntimeScalarEmbedding,
                    path.clone(),
                ));
            }
            ContentProjectionScalar::Natural(_) => {}
            ContentProjectionScalar::Successor(inner) => visit(inner, keys),
            ContentProjectionScalar::Add(left, right)
            | ContentProjectionScalar::Subtract(left, right)
            | ContentProjectionScalar::Multiply(left, right) => {
                visit(left, keys);
                visit(right, keys);
            }
        }
    }

    let mut keys = BTreeSet::new();
    match expression {
        ContentProjectionExpression::IntervalSet(members) => {
            for (start, end) in members {
                visit(start, &mut keys);
                visit(end, &mut keys);
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            visit(magnitude, &mut keys);
        }
    }
    keys
}

fn evaluate_capacity_scalar(
    scalar: &ContentProjectionScalar,
    bindings: &BTreeMap<ProgramLocalRootScalarKey, BigInt>,
) -> Result<BigInt, ExternalRootDiagnostic> {
    let value = match scalar {
        ContentProjectionScalar::SubjectField(path) => bindings
            .get(&(ProgramLocalRootScalarSource::SubjectField, path.clone()))
            .cloned()
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "program-local subject is missing a verified field observation".into(),
                )
            })?,
        ContentProjectionScalar::RuntimeScalarEmbedding(path) => bindings
            .get(&(
                ProgramLocalRootScalarSource::RuntimeScalarEmbedding,
                path.clone(),
            ))
            .cloned()
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "program-local subject is missing a verified runtime scalar embedding".into(),
                )
            })?,
        ContentProjectionScalar::Natural(value) => BigInt::from_decimal_str(value)
            .filter(|value| !value.is_negative())
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "program-local capacity schema contains a non-natural literal".into(),
                )
            })?,
        ContentProjectionScalar::Successor(inner) => {
            evaluate_capacity_scalar(inner, bindings)?.add(&BigInt::from_u64(1))
        }
        ContentProjectionScalar::Add(left, right) => evaluate_capacity_scalar(left, bindings)?
            .add(&evaluate_capacity_scalar(right, bindings)?),
        ContentProjectionScalar::Subtract(left, right) => {
            let left = evaluate_capacity_scalar(left, bindings)?;
            let right = evaluate_capacity_scalar(right, bindings)?;
            if right > left {
                return Err(ExternalRootDiagnostic(
                    "program-local exact natural subtraction lacks its lower-bound proof".into(),
                ));
            }
            left.sub(&right)
        }
        ContentProjectionScalar::Multiply(left, right) => evaluate_capacity_scalar(left, bindings)?
            .mul(&evaluate_capacity_scalar(right, bindings)?),
    };
    if value.is_negative() {
        return Err(ExternalRootDiagnostic(
            "program-local capacity evaluation produced a negative proof-natural".into(),
        ));
    }
    Ok(value)
}

fn evaluate_capacity(
    expression: &ContentProjectionExpression,
    bindings: &BTreeMap<ProgramLocalRootScalarKey, BigInt>,
) -> Result<EstablishedProgramLocalRootCapacity, ExternalRootDiagnostic> {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => {
            let mut evaluated = Vec::with_capacity(members.len());
            for (start, end) in members {
                let start = evaluate_capacity_scalar(start, bindings)?;
                let end = evaluate_capacity_scalar(end, bindings)?;
                evaluated.push(NaturalInterval::new(start, end).map_err(|error| {
                    ExternalRootDiagnostic(format!(
                        "program-local interval capacity is invalid: {error:?}"
                    ))
                })?);
            }
            Ok(EstablishedProgramLocalRootCapacity::IntervalSet(
                CanonicalIntervalSet::new(evaluated).map_err(|error| {
                    ExternalRootDiagnostic(format!(
                        "program-local interval capacity is not a separated canonical set: {error:?}"
                    ))
                })?,
            ))
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            Ok(EstablishedProgramLocalRootCapacity::CountedQuantity(
                evaluate_capacity_scalar(magnitude, bindings)?,
            ))
        }
    }
}

fn capacity_matches_algebra(
    capacity: &EstablishedProgramLocalRootCapacity,
    algebra: &ContentAlgebra,
) -> bool {
    matches!(
        (capacity, algebra.kind),
        (
            EstablishedProgramLocalRootCapacity::IntervalSet(_),
            ContentAlgebraKind::IntervalSet
        ) | (
            EstablishedProgramLocalRootCapacity::CountedQuantity(_),
            ContentAlgebraKind::CountedQuantity
        )
    )
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
    argument_index: u32,
    source_parameter_position: u32,
    qualification_identity: String,
    carrier_identity: String,
    schema_identity: u64,
    algebra: ContentAlgebra,
    per_occurrence_capacity: ContentProjectionExpression,
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

    pub const fn schema_identity(&self) -> u64 {
        self.schema_identity
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
    identity: InstalledProgramLocalRootEpochCohortId,
    installed_required_slots: InstalledRequiredRootSlotClosure,
    aggregates: Vec<ProgramLocalRootEpochAggregate>,
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
    occurrence: InstalledProgramLocalRootOccurrence<'root, 'code>,
    invocation: ProgramLocalRootEntryInvocationId,
    subject_place: ProgramLocalRootSubjectPlaceId,
    scalar_observations: BTreeMap<ProgramLocalRootScalarKey, BigInt>,
    capacity: EstablishedProgramLocalRootCapacity,
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
    identity: InstalledProgramLocalRootEpochCohortId,
    installed_required_slots: InstalledRequiredRootSlotClosure,
    pending: BTreeMap<
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
    subject: InstalledProgramLocalRootSubject<'root, 'code>,
    diagnostic: ExternalRootDiagnostic,
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
    subjects: Vec<InstalledProgramLocalRootSubject<'root, 'code>>,
    diagnostic: ExternalRootDiagnostic,
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
    root: EstablishedProgramLocalRoot<'root, 'code>,
    diagnostic: ExternalRootDiagnostic,
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

#[cfg(test)]
mod capacity_evaluation_tests {
    use super::*;

    #[test]
    fn interval_capacity_evaluates_each_runtime_path_without_scalar_multiplication() {
        let expression = ContentProjectionExpression::IntervalSet(vec![(
            ContentProjectionScalar::RuntimeScalarEmbedding(vec!["base".into()]),
            ContentProjectionScalar::Add(
                Box::new(ContentProjectionScalar::RuntimeScalarEmbedding(vec![
                    "base".into(),
                ])),
                Box::new(ContentProjectionScalar::SubjectField(vec!["length".into()])),
            ),
        )]);
        let bindings = BTreeMap::from([
            (
                (
                    ProgramLocalRootScalarSource::RuntimeScalarEmbedding,
                    vec!["base".into()],
                ),
                BigInt::from_u64(100),
            ),
            (
                (
                    ProgramLocalRootScalarSource::SubjectField,
                    vec!["length".into()],
                ),
                BigInt::from_u64(8),
            ),
        ]);

        let EstablishedProgramLocalRootCapacity::IntervalSet(capacity) =
            evaluate_capacity(&expression, &bindings).expect("exact interval capacity")
        else {
            panic!("interval expression evaluates as an interval set")
        };
        let [member] = capacity.members() else {
            panic!("one exact interval member")
        };
        assert_eq!(member.start(), &BigInt::from_u64(100));
        assert_eq!(member.end(), &BigInt::from_u64(108));
    }

    #[test]
    fn exact_natural_subtraction_rejects_underflow() {
        let expression =
            ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Subtract(
                Box::new(ContentProjectionScalar::Natural("2".into())),
                Box::new(ContentProjectionScalar::Natural("3".into())),
            ));
        assert!(
            evaluate_capacity(&expression, &BTreeMap::new())
                .expect_err("unproved natural subtraction must reject")
                .0
                .contains("lower-bound proof")
        );
    }
}
