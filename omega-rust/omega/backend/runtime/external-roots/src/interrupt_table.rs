//! Interrupt-table installation custody over an installed root ledger.
//!
//! Installing one exception or timer entry as an external root does not make
//! it hardware-reachable: the descriptor table that names it must be written,
//! consumer-validated, and published through a checked instruction such as
//! `lidt`. This module owns the compiler-side custody boundary between those
//! events. A consumer-authored [`InterruptTableProfile`] declares the complete
//! vector set the table must carry — every fatal exception entry plus any
//! acknowledged external interrupt such as the timer — and each member's own
//! dedicated critical stack class. The ledger admits each member only against
//! its exact installed-root record, retains the member's linear
//! [`InstalledExternalRoot`] handle so the entry cannot be removed while the
//! table accounts it, and refuses to issue the publication carrier until the
//! declared set is complete.
//!
//! Publication follows the hardware-materialization split: the consumer's
//! [`EstablishedInterruptTable`] is the already-validated table value bound to
//! this exact member set and destination, and the separately supplied
//! publication authority is bound into the issued carrier. The ledger mints
//! neither the bytes nor the authority. The provider that performs the checked
//! publication instruction answers with a receipt naming the exact carrier; a
//! refusal returns the established value so a fresh carrier can retry, while a
//! published table keeps every member pinned for as long as the ledger lives.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{EntryControl, EntryStack};
use executable_installation::{ArtifactId, InstalledCodeId};
use extents::Extent;
use layout_plans::EntryStubId;

use crate::{
    ExternalRootDiagnostic, ExternalRootId, InstalledExternalRoot, InstalledRootLedger,
    InterruptTableEstablishmentId, InterruptTableProfileId, InterruptTablePublicationAuthorityId,
    InterruptTablePublicationId, InterruptTablePublicationReceiptId, RootSlotId,
};

/// The settlement contract one declared table member owes per arrival.
///
/// The compiler does not choose which vectors are fatal or which controller
/// protocol an interrupt settles; the consumer declares the obligation per
/// member and admission checks that the installed root's boundary can honor
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptTableObligation {
    /// A fatal exception entry records the fault and never resumes ordinary
    /// work. No controller acknowledgement is minted for it: the installed
    /// root must not carry an acknowledgement policy or parameter.
    FatalException,
    /// An external interrupt entry — the timer root's shape — whose handler
    /// acknowledges, records, and wakes ordinary work. The installed root
    /// must mint a `Pending` acknowledgement that its exit settles.
    AcknowledgedInterrupt,
}

/// One declared member of the consumer's table plan: the vector the table
/// owner will route and the dedicated critical stack class its entries must
/// arrive on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableMemberPlan {
    pub vector: u8,
    pub dedicated_stack_class: u16,
    pub obligation: InterruptTableObligation,
}

/// Normalized consumer-authored plan for one interrupt table.
///
/// The member set is the complete declared coverage: publication cannot be
/// issued until every declared vector is admitted, and admitted members'
/// stack classes must be distinct so two fatal entries can never be
/// accounted onto one critical stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptTableProfile {
    identity: InterruptTableProfileId,
    members: BTreeMap<u8, InterruptTableMemberPlan>,
}

impl InterruptTableProfile {
    pub fn new(
        identity: InterruptTableProfileId,
        members: impl IntoIterator<Item = InterruptTableMemberPlan>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let mut declared = BTreeMap::new();
        let mut classes = BTreeSet::new();
        for member in members {
            if declared.insert(member.vector, member).is_some() {
                return Err(ExternalRootDiagnostic(format!(
                    "interrupt-table profile declares vector {} more than once",
                    member.vector
                )));
            }
            if !classes.insert(member.dedicated_stack_class) {
                return Err(ExternalRootDiagnostic(format!(
                    "interrupt-table profile accounts dedicated stack class {} to more than one vector",
                    member.dedicated_stack_class
                )));
            }
        }
        if declared.is_empty() {
            return Err(ExternalRootDiagnostic(
                "interrupt-table profile declares no member vectors".into(),
            ));
        }
        Ok(Self {
            identity,
            members: declared,
        })
    }

    pub const fn identity(&self) -> InterruptTableProfileId {
        self.identity
    }

    pub fn member(&self, vector: u8) -> Option<&InterruptTableMemberPlan> {
        self.members.get(&vector)
    }

    pub fn members(&self) -> impl ExactSizeIterator<Item = &InterruptTableMemberPlan> {
        self.members.values()
    }
}

/// The member set one consumer-established table value claims to describe.
/// The root and entry identities are replayed against the admitted member
/// records before publication; a stale or substituted row rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptTableEstablishedMember {
    pub root: ExternalRootId,
    pub entry: EntryStubId,
}

/// The consumer-established table value: the descriptor-table bytes the
/// owner's semantic validator accepted for this exact profile, member set,
/// and destination.
///
/// Gate-kind, selector, privilege, IST, reserved-bit, and base/limit policy
/// belong to the consumer's validator; the compiler binds the established
/// identity, its claimed member rows, and the occupied destination so the
/// issued carrier cannot describe a different table. The destination extent
/// is linear resource evidence, so the value itself cannot be cloned.
#[derive(Debug, PartialEq, Eq)]
pub struct EstablishedInterruptTable {
    establishment: InterruptTableEstablishmentId,
    profile: InterruptTableProfileId,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    members: BTreeMap<u8, InterruptTableEstablishedMember>,
    destination: Extent,
}

impl EstablishedInterruptTable {
    pub fn from_consumer(
        establishment: InterruptTableEstablishmentId,
        profile: &InterruptTableProfile,
        installed_code: InstalledCodeId,
        artifact: ArtifactId,
        members: impl IntoIterator<Item = (u8, InterruptTableEstablishedMember)>,
        destination: Extent,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let members: BTreeMap<u8, InterruptTableEstablishedMember> = members.into_iter().collect();
        if members.is_empty() {
            return Err(ExternalRootDiagnostic(
                "established interrupt table describes no member rows".into(),
            ));
        }
        if destination.length() == 0 {
            return Err(ExternalRootDiagnostic(
                "established interrupt table occupies an empty destination extent".into(),
            ));
        }
        Ok(Self {
            establishment,
            profile: profile.identity(),
            installed_code,
            artifact,
            members,
            destination,
        })
    }

    pub const fn establishment(&self) -> InterruptTableEstablishmentId {
        self.establishment
    }

    pub const fn destination(&self) -> &Extent {
        &self.destination
    }
}

/// One admitted table member. The retained linear root handle is the custody
/// pin: removal consumes `InstalledExternalRoot`, so the member's entry
/// cannot leave the installed ledger while the table holds it.
#[derive(Debug)]
pub struct InterruptTableMember<'code> {
    vector: u8,
    dedicated_stack_class: u16,
    obligation: InterruptTableObligation,
    entry: EntryStubId,
    slot: RootSlotId,
    root: InstalledExternalRoot<'code>,
}

impl<'code> InterruptTableMember<'code> {
    pub const fn vector(&self) -> u8 {
        self.vector
    }

    pub const fn dedicated_stack_class(&self) -> u16 {
        self.dedicated_stack_class
    }

    pub const fn obligation(&self) -> InterruptTableObligation {
        self.obligation
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn slot(&self) -> RootSlotId {
        self.slot
    }

    /// Borrow the retained member handle. The interrupt-entry path consumes
    /// only a shared borrow, so the timer member's provider receipts can be
    /// admitted without releasing the pin.
    pub const fn root(&self) -> &InstalledExternalRoot<'code> {
        &self.root
    }
}

/// The exact carrier handed to the provider edge that performs the checked
/// table publication. It binds the established value, the complete admitted
/// member set, the supplied publication authority, and the installed
/// realization; a completion receipt is only meaningful against this carrier.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptTablePublication {
    publication: InterruptTablePublicationId,
    authority: InterruptTablePublicationAuthorityId,
    established: EstablishedInterruptTable,
}

impl InterruptTablePublication {
    pub const fn publication(&self) -> InterruptTablePublicationId {
        self.publication
    }

    /// The consumer-supplied authority bound into this carrier. The provider
    /// edge that runs the checked publication instruction must validate its
    /// scope; the ledger only binds which exact authority was exercised.
    pub const fn authority(&self) -> InterruptTablePublicationAuthorityId {
        self.authority
    }

    pub const fn establishment(&self) -> InterruptTableEstablishmentId {
        self.established.establishment
    }

    /// The established destination the checked instruction loads from.
    pub const fn destination(&self) -> &Extent {
        &self.established.destination
    }

    /// The sealed vector-to-root-and-entry rows the established value
    /// described and the ledger verified against admitted custody.
    pub fn members(
        &self,
    ) -> impl ExactSizeIterator<Item = (u8, InterruptTableEstablishedMember)> + '_ {
        self.established
            .members
            .iter()
            .map(|(vector, member)| (*vector, *member))
    }
}

/// Provider receipt answering one issued publication carrier.
/// `published == false` is a refused attempt: the carrier is consumed and the
/// established value returns to the ledger's admitting phase for a retry.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptTablePublicationReceipt {
    identity: InterruptTablePublicationReceiptId,
    publication: InterruptTablePublicationId,
    authority: InterruptTablePublicationAuthorityId,
    published: bool,
}

impl InterruptTablePublicationReceipt {
    pub fn from_provider(
        identity: InterruptTablePublicationReceiptId,
        carrier: &InterruptTablePublication,
        published: bool,
    ) -> Self {
        Self {
            identity,
            publication: carrier.publication,
            authority: carrier.authority,
            published,
        }
    }

    pub const fn identity(&self) -> InterruptTablePublicationReceiptId {
        self.identity
    }
}

/// Evidence that one carrier published the complete declared table: the
/// member roots are now reachable through the established descriptor table.
/// The retained destination extent stays linear for the table's life.
#[derive(Debug, PartialEq, Eq)]
pub struct PublishedInterruptTable {
    publication: InterruptTablePublicationId,
    receipt: InterruptTablePublicationReceiptId,
    authority: InterruptTablePublicationAuthorityId,
    establishment: InterruptTableEstablishmentId,
    profile: InterruptTableProfileId,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    destination: Extent,
    members: BTreeMap<u8, InterruptTableEstablishedMember>,
}

impl PublishedInterruptTable {
    pub const fn publication(&self) -> InterruptTablePublicationId {
        self.publication
    }

    pub const fn receipt(&self) -> InterruptTablePublicationReceiptId {
        self.receipt
    }

    pub const fn establishment(&self) -> InterruptTableEstablishmentId {
        self.establishment
    }

    pub const fn destination(&self) -> &Extent {
        &self.destination
    }

    pub fn members(
        &self,
    ) -> impl ExactSizeIterator<Item = (u8, InterruptTableEstablishedMember)> + '_ {
        self.members
            .iter()
            .map(|(vector, member)| (*vector, *member))
    }
}

/// A refused publication attempt. The consumed carrier's established value is
/// returned so a fresh publication id can retry the same validated table.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptTablePublicationRefusal {
    publication: InterruptTablePublicationId,
    receipt: InterruptTablePublicationReceiptId,
    established: EstablishedInterruptTable,
}

impl InterruptTablePublicationRefusal {
    pub const fn publication(&self) -> InterruptTablePublicationId {
        self.publication
    }

    pub const fn receipt(&self) -> InterruptTablePublicationReceiptId {
        self.receipt
    }

    pub fn into_established(self) -> EstablishedInterruptTable {
        self.established
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InterruptTablePublicationOutcome {
    Published(PublishedInterruptTable),
    Refused(InterruptTablePublicationRefusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterruptTablePhase {
    Admitting,
    PublicationIssued {
        publication: InterruptTablePublicationId,
    },
    Published {
        publication: InterruptTablePublicationId,
        receipt: InterruptTablePublicationReceiptId,
    },
}

/// Installation-custody ledger for one interrupt table's complete member set.
///
/// Member admission replays the installed-root ledger's retained records —
/// the ledger mints no roots, vectors, or stack classes itself. Retained
/// member handles pin every admitted entry against removal for the table's
/// life; `close` returns them rather than dropping custody silently.
#[derive(Debug)]
pub struct InterruptTableLedger<'code> {
    profile: InterruptTableProfile,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    members: BTreeMap<u8, InterruptTableMember<'code>>,
    issued_publications: BTreeSet<InterruptTablePublicationId>,
    phase: InterruptTablePhase,
}

impl<'code> InterruptTableLedger<'code> {
    /// Open the table account against one exact installed-root ledger. All
    /// later member, established-value, and carrier evidence must resolve to
    /// this ledger's installed realization.
    pub fn new(profile: InterruptTableProfile, ledger: &InstalledRootLedger) -> Self {
        Self {
            profile,
            installed_code: ledger.installed_code(),
            artifact: ledger.artifact(),
            members: BTreeMap::new(),
            issued_publications: BTreeSet::new(),
            phase: InterruptTablePhase::Admitting,
        }
    }

    pub const fn profile(&self) -> &InterruptTableProfile {
        &self.profile
    }

    pub fn member(&self, vector: u8) -> Option<&InterruptTableMember<'code>> {
        self.members.get(&vector)
    }

    pub fn members(&self) -> impl ExactSizeIterator<Item = &InterruptTableMember<'code>> {
        self.members.values()
    }

    /// Whether every declared vector is admitted. Publication requires the
    /// complete set: an interrupt table with a missing fatal entry is not a
    /// partial table, it is an unhandled trap.
    pub fn is_complete(&self) -> bool {
        self.members.len() == self.profile.members.len()
    }

    /// Admit one declared vector's installed root into the table.
    ///
    /// The root must be live in `ledger`, match the declared dedicated stack
    /// class and obligation, and exit through interrupt return. On rejection
    /// the linear root handle comes back so the caller retains its custody.
    pub fn admit_interrupt_table_member(
        &mut self,
        ledger: &InstalledRootLedger,
        vector: u8,
        root: InstalledExternalRoot<'code>,
    ) -> Result<&InterruptTableMember<'code>, Box<InterruptTableAdmissionError<'code>>> {
        let reject = |diagnostic: &str, root: InstalledExternalRoot<'code>| {
            Err(Box::new(InterruptTableAdmissionError {
                vector,
                root,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        if self.phase != InterruptTablePhase::Admitting {
            return reject(
                "interrupt-table membership is sealed once a publication carrier is issued",
                root,
            );
        }
        if ledger.installed_code() != self.installed_code || ledger.artifact() != self.artifact {
            return reject(
                "interrupt-table member names a different installed-code occurrence",
                root,
            );
        }
        let Some(plan) = self.profile.member(vector) else {
            return reject(
                "interrupt-table profile declares no member at this vector",
                root,
            );
        };
        if self.members.contains_key(&vector) {
            return reject("interrupt-table vector is already admitted", root);
        }
        let Some(record) = ledger.record(root.root()) else {
            return reject(
                "interrupt-table member is not an installed root of this ledger",
                root,
            );
        };
        let exact_member = record.slot == root.slot()
            && record.installed_code == self.installed_code
            && record.artifact == self.artifact
            && root.installed_code() == self.installed_code
            && ledger
                .root_evidence
                .get(&record.root)
                .is_some_and(|evidence| evidence == &root.evidence);
        if !exact_member {
            return reject(
                "interrupt-table member handle does not bind the ledger's exact retained root",
                root,
            );
        }
        if record.boundary.call.entry_control != EntryControl::InterruptReturn {
            return reject(
                "interrupt-table member does not exit through interrupt return",
                root,
            );
        }
        let arrives_on_declared_class = match record.boundary.state.stack {
            EntryStack::Dedicated { class } => class == plan.dedicated_stack_class,
            EntryStack::Interrupted | EntryStack::ProviderSelected => false,
        };
        if !arrives_on_declared_class {
            return reject(
                "interrupt-table member does not arrive on its declared dedicated critical stack class",
                root,
            );
        }
        let obligation_matches = match plan.obligation {
            InterruptTableObligation::FatalException => {
                record.acknowledgement_policy.is_none()
                    && record.acknowledgement_parameter_index.is_none()
            }
            InterruptTableObligation::AcknowledgedInterrupt => {
                record.acknowledgement_policy.is_some()
                    && record.acknowledgement_parameter_index.is_some()
            }
        };
        if !obligation_matches {
            return reject(
                "interrupt-table member's acknowledgement contract does not honor its declared obligation",
                root,
            );
        }

        let member = InterruptTableMember {
            vector,
            dedicated_stack_class: plan.dedicated_stack_class,
            obligation: plan.obligation,
            entry: record.entry,
            slot: record.slot,
            root,
        };
        self.members.insert(vector, member);
        Ok(self
            .members
            .get(&vector)
            .expect("admitted interrupt-table member remains retained"))
    }

    /// Issue the exact publication carrier the checked-instruction provider
    /// edge consumes. Issuance requires the complete declared member set and
    /// an established table value describing exactly those members on this
    /// installed realization; any drift rejects with custody returned.
    pub fn begin_interrupt_table_publication(
        &mut self,
        ledger: &InstalledRootLedger,
        established: EstablishedInterruptTable,
        publication: InterruptTablePublicationId,
        authority: InterruptTablePublicationAuthorityId,
    ) -> Result<InterruptTablePublication, Box<InterruptTablePublicationError>> {
        let reject = |diagnostic: &str, established: EstablishedInterruptTable| {
            Err(Box::new(InterruptTablePublicationError {
                established,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        if self.phase != InterruptTablePhase::Admitting {
            return reject(
                "interrupt-table publication is already issued or published",
                established,
            );
        }
        if ledger.installed_code() != self.installed_code || ledger.artifact() != self.artifact {
            return reject(
                "interrupt-table publication names a different installed-code occurrence",
                established,
            );
        }
        if self.issued_publications.contains(&publication) {
            return reject(
                "interrupt-table publication identity was already issued",
                established,
            );
        }
        if !self.is_complete() {
            return reject(
                "interrupt-table publication requires the complete declared member set",
                established,
            );
        }
        if established.profile != self.profile.identity
            || established.installed_code != self.installed_code
            || established.artifact != self.artifact
        {
            return reject(
                "established interrupt table does not bind this profile and installed realization",
                established,
            );
        }
        if established.members.len() != self.members.len()
            || established.members.iter().any(|(vector, member)| {
                self.members.get(vector).is_none_or(|admitted| {
                    admitted.root.root() != member.root || admitted.entry != member.entry
                })
            })
        {
            return reject(
                "established interrupt table's member rows do not equal the admitted set",
                established,
            );
        }
        let all_still_installed = self.members.values().all(|member| {
            ledger.record(member.root.root()).is_some_and(|record| {
                record.slot == member.slot
                    && record.entry == member.entry
                    && ledger
                        .root_evidence
                        .get(&record.root)
                        .is_some_and(|evidence| evidence == &member.root.evidence)
            })
        });
        if !all_still_installed {
            return reject(
                "an admitted interrupt-table member is no longer installed",
                established,
            );
        }

        self.issued_publications.insert(publication);
        self.phase = InterruptTablePhase::PublicationIssued { publication };
        Ok(InterruptTablePublication {
            publication,
            authority,
            established,
        })
    }

    /// Accept the provider receipt answering one exact issued carrier. A
    /// refusal consumes the carrier, returns its established value, and
    /// reopens admission-phase custody so a fresh carrier can retry.
    pub fn complete_interrupt_table_publication(
        &mut self,
        ledger: &InstalledRootLedger,
        carrier: InterruptTablePublication,
        receipt: InterruptTablePublicationReceipt,
    ) -> Result<InterruptTablePublicationOutcome, Box<InterruptTableCompletionError>> {
        let reject = |diagnostic: &str,
                      carrier: InterruptTablePublication,
                      receipt: InterruptTablePublicationReceipt| {
            Err(Box::new(InterruptTableCompletionError {
                carrier,
                receipt,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        let exact_carrier = carrier.established.profile == self.profile.identity
            && carrier.established.installed_code == self.installed_code
            && carrier.established.artifact == self.artifact
            && self.issued_publications.contains(&carrier.publication)
            && matches!(
                self.phase,
                InterruptTablePhase::PublicationIssued { publication } if publication == carrier.publication
            );
        if !exact_carrier {
            return reject(
                "interrupt-table publication carrier is foreign, replayed, or out of order",
                carrier,
                receipt,
            );
        }
        if receipt.publication != carrier.publication || receipt.authority != carrier.authority {
            return reject(
                "interrupt-table publication receipt does not name the exact issued carrier",
                carrier,
                receipt,
            );
        }
        if ledger.installed_code() != self.installed_code || ledger.artifact() != self.artifact {
            return reject(
                "interrupt-table publication completes against a different installed-code occurrence",
                carrier,
                receipt,
            );
        }

        if receipt.published {
            self.phase = InterruptTablePhase::Published {
                publication: carrier.publication,
                receipt: receipt.identity,
            };
            Ok(InterruptTablePublicationOutcome::Published(
                PublishedInterruptTable {
                    publication: carrier.publication,
                    receipt: receipt.identity,
                    authority: carrier.authority,
                    establishment: carrier.established.establishment,
                    profile: carrier.established.profile,
                    installed_code: carrier.established.installed_code,
                    artifact: carrier.established.artifact,
                    destination: carrier.established.destination,
                    members: carrier.established.members,
                },
            ))
        } else {
            self.phase = InterruptTablePhase::Admitting;
            Ok(InterruptTablePublicationOutcome::Refused(
                InterruptTablePublicationRefusal {
                    publication: carrier.publication,
                    receipt: receipt.identity,
                    established: carrier.established,
                },
            ))
        }
    }

    /// Close the table account and return every retained member handle. The
    /// returned publication evidence stays accurate about whether the table
    /// reached hardware; a published table's members remain reachable only
    /// while their returned handles stay held.
    pub fn close(self) -> InterruptTableClose<'code> {
        let publication = match self.phase {
            InterruptTablePhase::Published {
                publication,
                receipt,
            } => Some((publication, receipt)),
            InterruptTablePhase::Admitting | InterruptTablePhase::PublicationIssued { .. } => None,
        };
        InterruptTableClose {
            members: self.members,
            publication,
        }
    }
}

/// Custody returned when an [`InterruptTableLedger`] closes: every retained
/// member handle plus the publication record, if any.
#[derive(Debug)]
pub struct InterruptTableClose<'code> {
    members: BTreeMap<u8, InterruptTableMember<'code>>,
    publication: Option<(
        InterruptTablePublicationId,
        InterruptTablePublicationReceiptId,
    )>,
}

impl<'code> InterruptTableClose<'code> {
    pub fn members(&self) -> impl ExactSizeIterator<Item = &InterruptTableMember<'code>> {
        self.members.values()
    }

    pub const fn publication(
        &self,
    ) -> Option<(
        InterruptTablePublicationId,
        InterruptTablePublicationReceiptId,
    )> {
        self.publication
    }

    pub fn into_member_roots(self) -> Vec<InstalledExternalRoot<'code>> {
        self.members
            .into_values()
            .map(|member| member.root)
            .collect()
    }
}

#[derive(Debug)]
pub struct InterruptTableAdmissionError<'code> {
    vector: u8,
    root: InstalledExternalRoot<'code>,
    diagnostic: ExternalRootDiagnostic,
}

impl<'code> InterruptTableAdmissionError<'code> {
    pub const fn vector(&self) -> u8 {
        self.vector
    }

    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_root(self) -> InstalledExternalRoot<'code> {
        self.root
    }
}

#[derive(Debug)]
pub struct InterruptTablePublicationError {
    established: EstablishedInterruptTable,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptTablePublicationError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_established(self) -> EstablishedInterruptTable {
        self.established
    }
}

#[derive(Debug)]
pub struct InterruptTableCompletionError {
    carrier: InterruptTablePublication,
    receipt: InterruptTablePublicationReceipt,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptTableCompletionError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InterruptTablePublication, InterruptTablePublicationReceipt) {
        (self.carrier, self.receipt)
    }
}

#[cfg(test)]
#[path = "interrupt_table_tests.rs"]
mod tests;
