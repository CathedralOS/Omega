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
//!
//! The checked instruction edge is the sole minting boundary for
//! [`InterruptTablePublicationReceipt`]. On x86-64 this is the provider-only
//! `lidt` contract: the edge replays the exercised consumer authority's exact
//! bound identity, its installed-realization scope, and both required scope
//! legs — processor table control and table publication — then replays the
//! declared pseudo-descriptor operand against the carrier's exact established
//! destination before minting the receipt. The answer accounts for the
//! operand's descriptor read, the contract's fixed `r10` scratch clobber, and
//! the descriptor-table register state installed on a published answer;
//! declined attempts mint no state.
//!
//! Descriptor-table materialization and its verdict are consumer-owned.
//! The package's authored table and gate layouts stage the declared constant
//! fields, the generic post-handoff writer seals each member's entry offset
//! into the produced image, and the consumer's authored validator — not any
//! compiler-held model of the encoding — scans the produced slots and mints
//! the verdict that warrants the [`EstablishedInterruptTable`]. What this
//! ledger keeps is the custody the verdict cannot carry: member admission
//! against installed-root records, the publication carrier that binds the
//! established value to this exact table, and the dispatch edge through the
//! published member rows.
//!
//! A published table is also the dispatch boundary for hardware arrivals:
//! `begin_published_interrupt_entry` resolves the reported vector through the
//! member set sealed at publication and enters the armed member's retained
//! root through the ledger's ordinary admission edge, so a provider receipt
//! can mint entry obligations only for an arrival whose gate actually
//! reached hardware.
//!
//! This file owns the ledger and its phases. `gate_descriptors.rs` carries
//! gate descriptors, member plans and profiles, `established_tables.rs`
//! established tables and members, `publications.rs` publications,
//! receipts, refusals, scopes and executed publications and
//! `descriptor_operands.rs` descriptor operands and table state.

mod descriptor_operands;
mod established_tables;
mod gate_descriptors;
mod publications;
#[cfg(test)]
mod tests;

pub use descriptor_operands::{
    INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES, InterruptDescriptorTableState,
    InterruptTableDescriptorOperand,
};
pub use established_tables::{
    EstablishedInterruptTable, InterruptTableEstablishedMember, InterruptTableMember,
};
pub use gate_descriptors::{
    InterruptTableGateDescriptor, InterruptTableMemberPlan, InterruptTableObligation,
    InterruptTableProfile, X86_64_GATE_DESCRIPTOR_BYTES, X86_64_IST_SLOT_LIMIT,
};
pub use publications::{
    ExecutedInterruptTablePublication, InterruptTableProviderError, InterruptTablePublication,
    InterruptTablePublicationAuthority, InterruptTablePublicationOutcome,
    InterruptTablePublicationReceipt, InterruptTablePublicationRefusal,
    InterruptTablePublicationScope, PublishedInterruptTable,
};

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{EntryControl, EntryStack};
use executable_installation::{ArtifactId, InstalledCodeId};

use crate::interrupts::interrupt_entries::{
    InterruptEntryObligations, InterruptEntryReceipt, InterruptEntryStartError,
};
use crate::{
    ExternalRootDiagnostic, InstalledExternalRoot, InstalledRootLedger,
    InterruptTablePublicationAuthorityId, InterruptTablePublicationId,
    InterruptTablePublicationReceiptId,
};

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
            descriptor: plan.descriptor,
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

    /// Admit one hardware-dispatched interrupt entry that arrived through
    /// this table's currently published member rows.
    ///
    /// Publication is the edge that makes member roots reachable to hardware
    /// arrivals, so this is the dispatch route for the timer and the fatal
    /// exception entries alike: the reported `vector` resolves through the
    /// member set sealed at publication, and the armed member's retained
    /// root handle enters the ordinary admission edge
    /// ([`InstalledRootLedger::begin_interrupt_entry`]), which rejoins the
    /// receipt against the installed root's exact evidence. An arrival
    /// reported before this table's carrier published, on a vector the
    /// published rows do not arm, or against a root ledger that is not this
    /// table's installed realization rejects with the receipt returned; a
    /// receipt that does not bind the resolved member's exact installed root
    /// rejects inside the ordinary edge.
    pub fn begin_published_interrupt_entry(
        &self,
        ledger: &mut InstalledRootLedger,
        vector: u8,
        receipt: InterruptEntryReceipt,
    ) -> Result<InterruptEntryObligations, InterruptEntryStartError> {
        if !matches!(self.phase, InterruptTablePhase::Published { .. }) {
            return Err(InterruptEntryStartError::unrouted(
                receipt,
                "interrupt entry cannot arrive before this table's publication reaches hardware",
            ));
        }
        if ledger.installed_code() != self.installed_code || ledger.artifact() != self.artifact {
            return Err(InterruptEntryStartError::unrouted(
                receipt,
                "interrupt entry arrival resolves against a different installed realization",
            ));
        }
        let Some(member) = self.members.get(&vector) else {
            return Err(InterruptEntryStartError::unrouted(
                receipt,
                "the reported vector is not an armed member of the published interrupt table",
            ));
        };
        ledger.begin_interrupt_entry_as(
            member.root(),
            receipt,
            matches!(member.obligation, InterruptTableObligation::FatalException),
        )
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
