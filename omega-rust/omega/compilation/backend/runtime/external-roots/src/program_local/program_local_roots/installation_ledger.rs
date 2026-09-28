//! The program-local root installation ledger and capacity evaluation.

use crate::program_local::program_local_roots::occurrences::ProgramLocalRootScalarKey;
use crate::program_local::program_local_roots::prebindings::{
    CountKey, LifecycleFamilyKey, program_local_root_schema_digest,
};
use crate::program_local::program_local_roots::{
    EstablishedProgramLocalRoot, EstablishedProgramLocalRootCapacity,
    InstalledProgramLocalRootEpochCohort, InstalledProgramLocalRootEpochCohortId,
    InstalledProgramLocalRootOccurrence, InstalledProgramLocalRootOccurrenceId,
    InstalledProgramLocalRootSubject, ProgramLocalEntryActivation,
    ProgramLocalRootBatchEstablishmentError, ProgramLocalRootCohortMember,
    ProgramLocalRootCohortSealError, ProgramLocalRootEpochAggregate,
    ProgramLocalRootEpochAggregateCapacity, ProgramLocalRootEpochRuntime,
    ProgramLocalRootEstablishmentError, ProgramLocalRootInstalledPrebinding,
    ProgramLocalRootInstalledPrebindingCount, ProgramLocalRootOccurrenceRetirementError,
    ProgramLocalRootPrebindingId, ProgramLocalRootRetirementError, ProgramLocalRootScalarSource,
    RetiredProgramLocalRootOccurrence,
};
use crate::root_entry::root_admission::bind_terminal_function;
use crate::{
    ExternalRootDiagnostic, InstalledExternalRoot, InstalledRequiredRootSlotClosure,
    InstalledRootLedger, ObjectEvidence,
};
use effects::{ComponentEraEntryLedger, ComponentEraLedgerId};
use language_semantics::content::{CanonicalIntervalSet, NaturalInterval};
use numerics::bignum::BigInt;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionExpression, ContentProjectionScalar,
};
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;
use terminal_codec::VerifiedProgramLocalRootProducerCatalog;

/// Installation-owned ledger for the non-minting occurrence prebinding.
///
/// The ledger derives the complete eligible set itself: callers cannot choose
/// which required slots enter the aggregate. One derivation enumerates every
/// statically enumerable installed parameter occurrence against the sealed
/// required-slot closure, so the later epoch cohort's expected rows are
/// installation evidence rather than a trusted caller roster.
#[derive(Debug)]
pub struct ProgramLocalRootInstallationLedger {
    installed_required_slots: InstalledRequiredRootSlotClosure,
    prebindings: BTreeMap<ProgramLocalRootPrebindingId, ProgramLocalRootInstalledPrebinding>,
    eligible_prebindings_derived: bool,
    prebindings_frozen: bool,
    lifecycle_bindings: BTreeMap<LifecycleFamilyKey, ComponentEraLedgerId>,
    active_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
    established_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
    used_occurrences: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
    sealed_epoch_cohorts: BTreeSet<(ComponentEraLedgerId, u64)>,
    cleanup_occupancies: BTreeSet<InstalledProgramLocalRootOccurrenceId>,
}

impl ProgramLocalRootInstallationLedger {
    fn from_installed_required_slots(
        installed_required_slots: InstalledRequiredRootSlotClosure,
    ) -> Self {
        Self {
            installed_required_slots,
            prebindings: BTreeMap::new(),
            eligible_prebindings_derived: false,
            prebindings_frozen: false,
            lifecycle_bindings: BTreeMap::new(),
            active_occurrences: BTreeSet::new(),
            established_occurrences: BTreeSet::new(),
            used_occurrences: BTreeSet::new(),
            sealed_epoch_cohorts: BTreeSet::new(),
            cleanup_occupancies: BTreeSet::new(),
        }
    }

    pub const fn installed_required_slots(&self) -> &InstalledRequiredRootSlotClosure {
        &self.installed_required_slots
    }

    /// Derive the complete eligible prebinding set from the sealed required
    /// slot closure, one verified producer catalog, and the exact installed
    /// artifact.
    ///
    /// The enumerable set is not a caller roster. The caller must present the
    /// live handle of every root occupying a sealed required slot: a missing
    /// member leaves an exact installed parameter occurrence unenumerated, and
    /// a runtime-open or substituted root is not part of the closure. Either
    /// failure rejects transactionally before any prebinding commits. The
    /// derivation then joins every authorized schema on each retained required
    /// root's exact requirement to its live slot, provider execution,
    /// admission, and artifact occurrence, so the epoch cohort's expected set
    /// cannot silently understate this installed artifact instance.
    pub fn derive_eligible_prebindings<'root, 'code: 'root, TerminalArtifact: ObjectEvidence>(
        &mut self,
        catalog: &VerifiedProgramLocalRootProducerCatalog,
        artifact: &TerminalArtifact,
        roots: impl IntoIterator<Item = &'root InstalledExternalRoot<'code>>,
    ) -> Result<Vec<ProgramLocalRootInstalledPrebinding>, ExternalRootDiagnostic> {
        if self.prebindings_frozen {
            return Err(ExternalRootDiagnostic(
                "program-local root prebindings are frozen after epoch-cohort sealing".into(),
            ));
        }
        if self.eligible_prebindings_derived {
            return Err(ExternalRootDiagnostic(
                "the complete eligible program-local prebinding set was already derived".into(),
            ));
        }
        let mut presented = BTreeMap::new();
        for root in roots {
            let Some(installed_slot) = self.installed_required_slots.slot(root.slot) else {
                return Err(ExternalRootDiagnostic(
                    "program-local root enumeration names a slot outside the sealed required closure"
                        .into(),
                ));
            };
            if self.installed_required_slots.installed_code() != root.installed_code.identity()
                || !installed_slot.matches_root(root)
            {
                return Err(ExternalRootDiagnostic(
                    "program-local root enumeration substituted the installed required root".into(),
                ));
            }
            if presented.insert(root.slot, root).is_some() {
                return Err(ExternalRootDiagnostic(
                    "program-local root enumeration repeats one installed required slot".into(),
                ));
            }
        }
        if presented.len() != self.installed_required_slots.slots().len() {
            return Err(ExternalRootDiagnostic(
                "program-local root enumeration omits a sealed required root slot".into(),
            ));
        }
        let psi = catalog.terminal_psi();
        if artifact.psi() != psi {
            return Err(ExternalRootDiagnostic(
                "program-local root catalog does not match the terminal artifact identity".into(),
            ));
        }
        let mut pending = Vec::new();
        for installed_slot in self.installed_required_slots.slots() {
            let root = presented
                .get(&installed_slot.required().slot())
                .copied()
                .expect("the complete presented required-root set was verified");
            let requirement_identity = &root.evidence.root.candidate.requirement_identity;
            let schemas = catalog
                .schemas()
                .iter()
                .filter(|schema| schema.boundary_requirement_identity() == requirement_identity)
                .collect::<Vec<_>>();
            if schemas.is_empty() {
                continue;
            }
            let text_offset = artifact
                .function_text_offset(catalog.terminal_entry())
                .ok_or_else(|| {
                    ExternalRootDiagnostic(
                        "terminal artifact has no installed entry for the program-local root catalog"
                            .into(),
                    )
                })?;
            bind_terminal_function(
                artifact,
                root.installed_code,
                root.evidence.root.candidate.entry,
                text_offset,
            )?;
            let mut local_schema_keys = BTreeSet::new();
            for verified_schema in schemas {
                let schema = verified_schema.schema();
                let schema_digest = program_local_root_schema_digest(verified_schema);
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
                if !local_schema_keys.insert((schema.source_parameter_position, schema_digest)) {
                    return Err(ExternalRootDiagnostic(
                        "program-local root producer schemas repeat one semantic occurrence".into(),
                    ));
                }
                let identity = ProgramLocalRootPrebindingId {
                    installed_code: root.installed_code.identity(),
                    root: root.root,
                    slot: root.slot,
                    schema_digest,
                };
                debug_assert!(
                    !self.prebindings.contains_key(&identity),
                    "the eligible set is derived at most once"
                );
                pending.push((
                    identity,
                    ProgramLocalRootInstalledPrebinding {
                        identity,
                        psi,
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
                        schema_compatibility_report_identity: schema.compatibility_report_identity,
                        algebra: schema.algebra.clone(),
                        per_occurrence_capacity: schema.capacity.clone(),
                    },
                ));
            }
        }

        let joined = pending
            .iter()
            .map(|(_, occurrence)| occurrence.clone())
            .collect();
        for (key, occurrence) in pending {
            self.prebindings.insert(key, occurrence);
        }
        self.eligible_prebindings_derived = true;
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
                occurrence.psi.vocabulary_marker.get(),
                *occurrence.psi.program_fingerprint.as_bytes(),
                occurrence.identity.installed_code,
                occurrence.identity.schema_digest,
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
                    psi: occurrence.psi,
                    installed_code: occurrence.identity.installed_code,
                    artifact: occurrence.artifact,
                    requirement_identity: occurrence.requirement_identity,
                    argument_index: occurrence.argument_index,
                    source_parameter_position: occurrence.source_parameter_position,
                    qualification_identity: occurrence.qualification_identity,
                    carrier_identity: occurrence.carrier_identity,
                    schema_digest: occurrence.identity.schema_digest,
                    schema_compatibility_report_identity: occurrence
                        .schema_compatibility_report_identity,
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
        if !self.eligible_prebindings_derived {
            return reject(
                members,
                "program-local root epoch cohort precedes the derived eligible prebinding set",
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
                || member.epoch_lease.artifact_occurrence_digest()
                    != canonical
                        .installed_root_evidence
                        .installed_code
                        .occurrence_digest()
                || member
                    .epoch_lease
                    .artifact_instance_compatibility_report_identity()
                    != canonical.identity.installed_code.normalized_identity()
            {
                return reject(
                    members,
                    "program-local root cohort lease does not bind the exact requirement and installed artifact occurrence",
                );
            }
            let lifecycle_family = (
                canonical.identity.installed_code,
                canonical.identity.schema_digest,
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
                prebinding.psi.vocabulary_marker.get(),
                *prebinding.psi.program_fingerprint.as_bytes(),
                prebinding.identity.installed_code,
                prebinding.identity.schema_digest,
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
                    psi: prebinding.psi,
                    artifact: prebinding.artifact,
                    requirement_identity: prebinding.requirement_identity,
                    argument_index: prebinding.argument_index,
                    source_parameter_position: prebinding.source_parameter_position,
                    qualification_identity: prebinding.qualification_identity,
                    carrier_identity: prebinding.carrier_identity,
                    schema_digest: prebinding.identity.schema_digest,
                    schema_compatibility_report_identity: prebinding
                        .schema_compatibility_report_identity,
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
    /// subject observed under the presented live activation. Every symbolic
    /// scalar is replayed against the verified schema before the occurrence is
    /// removed from the runtime, so rejection returns the complete subject
    /// binding and mints no lineage.
    pub fn establish<'root, 'subject, 'code>(
        &mut self,
        runtime: &mut ProgramLocalRootEpochRuntime<'root, 'code>,
        lifecycle: &ComponentEraEntryLedger,
        activation: &ProgramLocalEntryActivation,
        subject: InstalledProgramLocalRootSubject<'subject, 'code>,
    ) -> Result<
        EstablishedProgramLocalRoot<'root, 'code>,
        Box<ProgramLocalRootEstablishmentError<'subject, 'code>>,
    > {
        match self.establish_batch(runtime, lifecycle, activation, [subject]) {
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
    ///
    /// The presented activation is the only authority that can mint subject
    /// invocation identities: it must have been entered on this exact
    /// lifecycle ledger for the cohort's exact epoch, and every subject must
    /// have been observed under it. A subject stamped by another activation —
    /// or an activation that entered a different ledger or era — rejects, so
    /// establishment cannot be claimed under a redirected or replayed entry
    /// invocation.
    pub fn establish_batch<'root, 'subject, 'code>(
        &mut self,
        runtime: &mut ProgramLocalRootEpochRuntime<'root, 'code>,
        lifecycle: &ComponentEraEntryLedger,
        activation: &ProgramLocalEntryActivation,
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
        if activation.ledger() != lifecycle.identity()
            || activation.era_identity() != runtime.identity.lifecycle_epoch()
        {
            return reject(
                subjects,
                "program-local establishment activation is not entered on the cohort's exact lifecycle ledger and epoch",
            );
        }
        let activation_invocation = activation.invocation();

        let mut selected = BTreeSet::new();
        let mut validated = Vec::with_capacity(subjects.len());
        for subject in &subjects {
            if subject.invocation != activation_invocation {
                return reject(
                    subjects,
                    "program-local installed subject was not observed under the presented entry activation",
                );
            }
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

    /// Track a receiver extent whose cleanup must occupy its hosted
    /// installation ledger extent through completion.
    ///
    /// The occupancy marker follows the occurrence's active lifetime: it can
    /// be recorded only on an occurrence that established under this ledger
    /// and is still active — a pending occurrence has not proven occupancy,
    /// a retired one has already completed — and retirement discharges it.
    /// Tracked extents keep contributing their aggregate accounting while
    /// occupied; the marker is evidence, not an additional capacity row.
    pub fn track_receiver_cleanup_occupancy(
        &mut self,
        occurrence: InstalledProgramLocalRootOccurrenceId,
    ) -> Result<(), ExternalRootDiagnostic> {
        if !self.established_occurrences.contains(&occurrence)
            || !self.active_occurrences.contains(&occurrence)
        {
            return Err(ExternalRootDiagnostic(
                "program-local receiver cleanup occupancy requires an exact active established occurrence"
                    .into(),
            ));
        }
        self.cleanup_occupancies.insert(occurrence);
        Ok(())
    }

    /// The occurrences whose receiver cleanup currently occupies its hosted
    /// installation ledger extent.
    pub fn cleanup_occupancies(
        &self,
    ) -> impl Iterator<Item = &InstalledProgramLocalRootOccurrenceId> {
        self.cleanup_occupancies.iter()
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
        // Completion discharges the cleanup occupancy: a retired extent no
        // longer occupies the ledger through its receiver's cleanup.
        self.cleanup_occupancies.remove(&identity);
        let fresh = self.used_occurrences.insert(identity);
        debug_assert!(fresh, "active occurrence was not already retired");
        Ok(RetiredProgramLocalRootOccurrence {
            identity,
            epoch_lease: lease_identity,
        })
    }

    /// Reconstruct the exact aggregate capacity of one live aggregate schema
    /// group in one sealed epoch cohort of this installed artifact instance.
    ///
    /// Reconstruction is accounting evidence, not minting: every presented
    /// member must be an exact occurrence still established under this ledger,
    /// and the presented set must equal the group's complete live membership
    /// for its lifecycle cohort. An omitted, pending, retired, repeated,
    /// substituted, or cross-group member rejects, so the derived capacity
    /// cannot be understated or supplied ambiently. Equal evaluated capacities
    /// across distinct occurrences each contribute — row equality grants no
    /// merging authority — and interval members whose evaluated ranges overlap
    /// reject rather than silently share one range. Retired occurrences leave
    /// the live aggregate; a still-pending member blocks it.
    pub fn reconstruct_aggregate_capacity<'members, 'root: 'members, 'code: 'root>(
        &self,
        lifecycle: &ComponentEraEntryLedger,
        roots: impl IntoIterator<Item = &'members EstablishedProgramLocalRoot<'root, 'code>>,
    ) -> Result<ProgramLocalRootEpochAggregateCapacity, ExternalRootDiagnostic> {
        let roots = roots.into_iter().collect::<Vec<_>>();
        if roots.is_empty() {
            return Err(ExternalRootDiagnostic(
                "program-local aggregate capacity reconstruction requires at least one exact established member".into(),
            ));
        }

        let mut presented = BTreeSet::new();
        let mut cohort_key = None;
        let mut group_key = None;
        let mut canonical = None;
        for root in &roots {
            let identity = root.occurrence_identity();
            if !self.established_occurrences.contains(&identity) {
                return Err(ExternalRootDiagnostic(
                    "program-local aggregate capacity reconstruction names no exact established occurrence of this installation".into(),
                ));
            }
            if !presented.insert(identity) {
                return Err(ExternalRootDiagnostic(
                    "program-local aggregate capacity reconstruction repeats one exact occurrence"
                        .into(),
                ));
            }
            let member_cohort = (identity.lifecycle_ledger, identity.lifecycle_epoch);
            if let Some(cohort) = cohort_key {
                if cohort != member_cohort {
                    return Err(ExternalRootDiagnostic(
                        "program-local aggregate capacity reconstruction spans distinct lifecycle cohorts".into(),
                    ));
                }
            } else {
                cohort_key = Some(member_cohort);
            }
            let Some(prebinding) = self.prebindings.get(&identity.prebinding) else {
                return Err(ExternalRootDiagnostic(
                    "program-local aggregate capacity reconstruction names no canonical installed prebinding".into(),
                ));
            };
            if prebinding != root.prebinding() {
                return Err(ExternalRootDiagnostic(
                    "program-local aggregate capacity reconstruction substituted the canonical installed prebinding".into(),
                ));
            }
            let member_key = (
                prebinding.psi.vocabulary_marker.get(),
                *prebinding.psi.program_fingerprint.as_bytes(),
                prebinding.identity.installed_code,
                prebinding.identity.schema_digest,
            );
            if let Some(key) = group_key {
                if key != member_key {
                    return Err(ExternalRootDiagnostic(
                        "program-local aggregate capacity reconstruction spans distinct aggregate schemas".into(),
                    ));
                }
            } else {
                group_key = Some(member_key);
            }
            if canonical.is_none() {
                canonical = Some(prebinding);
            }
            if lifecycle
                .validate_program_local_root_epoch_lease(&root.occurrence.epoch_lease)
                .is_err()
                || root.occurrence.epoch_lease.ledger() != identity.lifecycle_ledger
                || root.occurrence.epoch_lease.era_identity() != identity.lifecycle_epoch
            {
                return Err(ExternalRootDiagnostic(
                    "program-local aggregate capacity member is not held by a live lease in its exact lifecycle epoch".into(),
                ));
            }
            if !capacity_matches_algebra(root.capacity(), &prebinding.algebra) {
                return Err(ExternalRootDiagnostic(
                    "program-local aggregate capacity member does not match its verified content algebra".into(),
                ));
            }
        }

        let cohort_key = cohort_key.expect("nonempty reconstruction has one lifecycle cohort");
        let group_key = group_key.expect("nonempty reconstruction has one aggregate schema");
        let canonical = canonical.expect("nonempty reconstruction has one canonical prebinding");
        if !self.sealed_epoch_cohorts.contains(&cohort_key) {
            return Err(ExternalRootDiagnostic(
                "program-local aggregate capacity reconstruction names no sealed epoch cohort"
                    .into(),
            ));
        }
        if lifecycle.identity() != cohort_key.0
            || !lifecycle.live_eras().any(|(era, _, _)| era == cohort_key.1)
        {
            return Err(ExternalRootDiagnostic(
                "program-local aggregate capacity reconstruction is not in a live epoch of the exact lifecycle ledger".into(),
            ));
        }

        let expected = self
            .active_occurrences
            .iter()
            .filter(|identity| {
                identity.lifecycle_ledger == cohort_key.0
                    && identity.lifecycle_epoch == cohort_key.1
                    && self
                        .prebindings
                        .get(&identity.prebinding)
                        .is_some_and(|prebinding| {
                            (
                                prebinding.psi.vocabulary_marker.get(),
                                *prebinding.psi.program_fingerprint.as_bytes(),
                                prebinding.identity.installed_code,
                                prebinding.identity.schema_digest,
                            ) == group_key
                        })
            })
            .copied()
            .collect::<BTreeSet<_>>();
        if presented != expected {
            return Err(ExternalRootDiagnostic(
                "program-local aggregate capacity reconstruction omits, substitutes, or adds a live member of the exact aggregate schema group".into(),
            ));
        }

        let capacity = match canonical.algebra.kind {
            ContentAlgebraKind::CountedQuantity => {
                let mut total = BigInt::zero();
                for root in &roots {
                    let Some(quantity) = root.capacity().counted_quantity() else {
                        return Err(ExternalRootDiagnostic(
                            "program-local counted aggregate member has no evaluated counted quantity".into(),
                        ));
                    };
                    total = total.add(quantity);
                }
                EstablishedProgramLocalRootCapacity::CountedQuantity(total)
            }
            ContentAlgebraKind::IntervalSet => {
                let mut sets = Vec::with_capacity(roots.len());
                for root in &roots {
                    let Some(set) = root.capacity().interval_set() else {
                        return Err(ExternalRootDiagnostic(
                            "program-local interval aggregate member has no evaluated interval set"
                                .into(),
                        ));
                    };
                    sets.push(set);
                }
                EstablishedProgramLocalRootCapacity::IntervalSet(
                    CanonicalIntervalSet::separate(sets).map_err(|error| {
                        ExternalRootDiagnostic(format!(
                            "program-local interval aggregate members cannot compose one separated set: {error:?}"
                        ))
                    })?,
                )
            }
        };

        Ok(ProgramLocalRootEpochAggregateCapacity {
            cohort: InstalledProgramLocalRootEpochCohortId {
                installed_code: canonical.identity.installed_code,
                lifecycle_ledger: cohort_key.0,
                lifecycle_epoch: cohort_key.1,
            },
            aggregate: ProgramLocalRootEpochAggregate {
                psi: canonical.psi,
                artifact: canonical.artifact,
                requirement_identity: canonical.requirement_identity.clone(),
                argument_index: canonical.argument_index,
                source_parameter_position: canonical.source_parameter_position,
                qualification_identity: canonical.qualification_identity.clone(),
                carrier_identity: canonical.carrier_identity.clone(),
                schema_digest: canonical.identity.schema_digest,
                schema_compatibility_report_identity: canonical
                    .schema_compatibility_report_identity,
                algebra: canonical.algebra.clone(),
                per_occurrence_capacity: canonical.per_occurrence_capacity.clone(),
                occurrence_identities: presented.into_iter().collect(),
            },
            capacity,
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

pub(crate) fn evaluate_capacity(
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
