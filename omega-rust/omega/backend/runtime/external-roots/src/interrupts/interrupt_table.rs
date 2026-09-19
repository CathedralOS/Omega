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
//! Descriptor-table materialization follows the same split. The ledger
//! derives the checked writer program covering every admitted member's sealed
//! gate-offset fragments; the consumer-declared constant fields — selector,
//! gate kind, privilege, IST slot, and the reserved-zero bytes — are the
//! staged table content that writer preserves.
//! [`InterruptTableLedger::validate_written_descriptor_table`] is the
//! consumer's semantic-validation edge: it proves the produced image came
//! from exactly this table's derived writer over the exact installed
//! realization, decodes the complete current bytes, replays each declared
//! descriptor's constant fields plus the reserved-zero regions, joins every
//! member's declared IST slot through the installed TSS to its declared
//! critical stack class, and only then mints the [`EstablishedInterruptTable`]
//! naming the exact written destination.
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

use calling_conventions::{EntryControl, EntryStack, X86_64InstalledTaskStateSegmentRealization};
use executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeId, ValidatedWrittenPostHandoffWriterDestination,
};
use extents::Extent;
use layout_plans::{
    ByteOrder, MaterializationWrite, PlacementConstraints, PlacementPhase, PostHandoffWriterPlan,
    PostHandoffWriterSource, PostHandoffWriterStep, RelocationTarget,
};
use target::Architecture;

use crate::interrupts::interrupt_entries::{
    InterruptEntryObligations, InterruptEntryReceipt, InterruptEntryStartError,
};
use crate::{
    ExternalRootDiagnostic, InstalledExternalRoot, InstalledRootLedger,
    InterruptTableEstablishmentId, InterruptTablePublicationAuthorityId,
    InterruptTablePublicationId, InterruptTablePublicationReceiptId,
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

    /// Byte length of the canonical table image: 16-byte gate slots for
    /// vectors 0 through the highest declared vector. A `lidt`
    /// pseudo-descriptor's limit is this length minus one, so undeclared
    /// vectors above the last member are unreachable rather than carried as
    /// zero-initialized padding.
    fn declared_table_bytes(&self) -> usize {
        usize::from(
            *self
                .profile
                .members
                .keys()
                .next_back()
                .expect("a constructed profile declares at least one member"),
        ) * X86_64_GATE_DESCRIPTOR_BYTES as usize
            + X86_64_GATE_DESCRIPTOR_BYTES as usize
    }

    /// Shared gate for deriving table materialization evidence: the declared
    /// member set must be fully admitted, the installed realization must be
    /// this ledger's own, and every member's entry must be an admitted entry
    /// of that artifact. The descriptor encoding is x86-64 long-mode only.
    fn require_materializable(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<(), ExternalRootDiagnostic> {
        if installed_code.identity() != self.installed_code
            || installed_code.artifact() != self.artifact
        {
            return Err(ExternalRootDiagnostic(
                "descriptor-table materialization names a different installed-code occurrence"
                    .into(),
            ));
        }
        if installed_code.architecture() != Architecture::X86_64 {
            return Err(ExternalRootDiagnostic(
                "the x86-64 gate-descriptor encoding cannot materialize a foreign-architecture table"
                    .into(),
            ));
        }
        if !self.is_complete() {
            return Err(ExternalRootDiagnostic(
                "descriptor-table materialization requires the complete declared member set".into(),
            ));
        }
        for member in self.members.values() {
            installed_code
                .selected_entry_target(member.entry)
                .map_err(|_| {
                    ExternalRootDiagnostic(format!(
                        "interrupt-table member at vector {} is not an admitted entry of this installed artifact",
                        member.vector
                    ))
                })?;
        }
        Ok(())
    }

    /// Derive the checked writer program that materializes the declared
    /// table's sealed gate offsets. Each admitted member contributes three
    /// fragments writing its resolved entry address into the descriptor's
    /// low-16, middle-16, and high-32 offset fields; the consumer-declared
    /// constant fields are staged table content this writer deliberately
    /// does not produce. The plan names no numeric address: the provider
    /// resolves each member's sealed entry target once against the exact
    /// installed realization.
    ///
    /// Derivation requires the complete declared member set so the produced
    /// image is the whole declared table, not a partial one.
    pub fn descriptor_table_writer_plan(
        &self,
        installed_code: &InstalledCode,
    ) -> Result<PostHandoffWriterPlan, ExternalRootDiagnostic> {
        self.require_materializable(installed_code)?;
        let mut steps = Vec::with_capacity(self.members.len() * 3);
        for member in self.members.values() {
            let target = RelocationTarget::Entry(member.entry);
            let base = u64::from(member.vector) * X86_64_GATE_DESCRIPTOR_BYTES;
            // offset[15:0] occupies a 16-bit container at +0, offset[31:16]
            // at +6, and offset[63:32] a 32-bit container at +8.
            for (container_byte_offset, container_width_bits, source_lsb) in [
                (base, 16_u16, 0_u16),
                (base + 6, 16, 16),
                (base + 8, 32, 32),
            ] {
                steps.push(PostHandoffWriterStep {
                    write: MaterializationWrite {
                        field: format!("interrupt-table.gate[{}].offset", member.vector),
                        target,
                        container_byte_offset,
                        container_width_bits,
                        destination_lsb: 0,
                        source_lsb,
                        width: container_width_bits,
                        stored_integer_fit: None,
                    },
                    source: PostHandoffWriterSource::Resolve(target),
                });
            }
        }
        Ok(PostHandoffWriterPlan {
            byte_len: self.declared_table_bytes(),
            byte_order: ByteOrder::LittleEndian,
            placement: PlacementConstraints::new(
                None,
                X86_64_GATE_DESCRIPTOR_BYTES,
                PlacementPhase::PostHandoff,
                None,
                None,
            )
            .map_err(|diagnostic| {
                ExternalRootDiagnostic(format!(
                    "descriptor-table placement constraints do not normalize: {}",
                    diagnostic.0
                ))
            })?,
            steps,
        })
    }

    /// The canonical staged image the provider writes into the activated
    /// destination before the checked writer runs: every declared member's
    /// constant descriptor fields — selector, IST byte, and the
    /// present/type/privilege attribute byte — with zeros everywhere else.
    /// The sealed gate offsets stay zero here; only the derived writer's
    /// fragments may fill them.
    pub fn descriptor_table_staged_image(&self) -> Result<Vec<u8>, ExternalRootDiagnostic> {
        if !self.is_complete() {
            return Err(ExternalRootDiagnostic(
                "descriptor-table materialization requires the complete declared member set".into(),
            ));
        }
        let mut image = vec![0u8; self.declared_table_bytes()];
        for member in self.members.values() {
            let base = usize::from(member.vector) * X86_64_GATE_DESCRIPTOR_BYTES as usize;
            image[base + 2..base + 4].copy_from_slice(&member.descriptor.selector.to_le_bytes());
            image[base + 4] = member.descriptor.ist_byte();
            image[base + 5] = member.descriptor.attribute_byte();
        }
        Ok(image)
    }

    /// The consumer's semantic-validation edge over the complete produced
    /// descriptor-table image. `written` must be the still-unpublished
    /// destination after its exact replay; this edge then proves the image
    /// came from this table's derived writer over this exact installed
    /// realization, decodes every descriptor slot, replays each declared
    /// member's constant fields — selector, gate kind, privilege, present
    /// bit, IST field, and the reserved-zero bytes — joins every member's
    /// declared IST slot through `tss` to its declared critical stack class,
    /// and mints the established value naming `destination` only when the
    /// complete image is the declared table.
    ///
    /// Success borrows `written` rather than consuming it: the produced
    /// destination stays unpublished under the caller's custody while the
    /// minted value is what the publication carrier binds.
    pub fn validate_written_descriptor_table(
        &self,
        installed_code: &InstalledCode,
        written: &ValidatedWrittenPostHandoffWriterDestination<'_, '_>,
        tss: &X86_64InstalledTaskStateSegmentRealization,
        establishment: InterruptTableEstablishmentId,
        destination: Extent,
    ) -> Result<EstablishedInterruptTable, ExternalRootDiagnostic> {
        let plan = self.descriptor_table_writer_plan(installed_code)?;
        let invocation = plan.lower_reusable_fragment().map_err(|diagnostic| {
            ExternalRootDiagnostic(format!(
                "descriptor-table writer does not lower its declared fragments: {}",
                diagnostic.0
            ))
        })?;
        if written.installed_code() != self.installed_code || written.artifact() != self.artifact {
            return Err(ExternalRootDiagnostic(
                "written descriptor table was produced for a different installed realization"
                    .into(),
            ));
        }
        if !written.binds_invocation(&invocation) {
            return Err(ExternalRootDiagnostic(
                "written descriptor table does not bind this table's derived writer invocation"
                    .into(),
            ));
        }
        let table_bytes = self.declared_table_bytes();
        if written.bytes().len() != table_bytes {
            return Err(ExternalRootDiagnostic(
                "written descriptor table is not the exact declared table image".into(),
            ));
        }
        if written.site().base_address != destination.base()
            || destination.length() != table_bytes as u64
        {
            return Err(ExternalRootDiagnostic(
                "the established destination does not name the exact written table extent".into(),
            ));
        }

        // The installed TSS the descriptors join through must be a canonical
        // map: real IST slots, at most one provisioned stack class each.
        let mut ist_classes = BTreeMap::new();
        for stack in &tss.interrupt_stacks {
            if stack.slot == 0 || stack.slot > X86_64_IST_SLOT_LIMIT {
                return Err(ExternalRootDiagnostic(
                    "the installed TSS names an interrupt-stack-table slot outside 1..=7".into(),
                ));
            }
            if ist_classes
                .insert(stack.slot, stack.dedicated_class)
                .is_some()
            {
                return Err(ExternalRootDiagnostic(
                    "the installed TSS repeats an interrupt-stack-table slot".into(),
                ));
            }
        }

        let bytes = written.bytes();
        let last_vector = u8::try_from(table_bytes / X86_64_GATE_DESCRIPTOR_BYTES as usize - 1)
            .expect("the declared table is bounded by the highest u8 vector");
        for vector in 0..=last_vector {
            let base = usize::from(vector) * X86_64_GATE_DESCRIPTOR_BYTES as usize;
            let slot = &bytes[base..base + X86_64_GATE_DESCRIPTOR_BYTES as usize];
            let Some(member) = self.members.get(&vector) else {
                if slot.iter().any(|byte| *byte != 0) {
                    return Err(ExternalRootDiagnostic(format!(
                        "undeclared vector {vector} carries a nonzero descriptor in the produced table"
                    )));
                }
                continue;
            };
            let descriptor = &member.descriptor;
            if slot[2..4] != descriptor.selector.to_le_bytes() {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s produced gate selector does not equal its declared value"
                )));
            }
            if slot[4] != descriptor.ist_byte() {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s produced IST field does not equal its declared value"
                )));
            }
            if slot[5] != descriptor.attribute_byte() {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s produced gate kind, privilege, or present bit does not equal its declared value"
                )));
            }
            if slot[12..16] != [0; 4] {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s produced descriptor carries nonzero reserved bytes"
                )));
            }
            let offset = u64::from(u16::from_le_bytes([slot[0], slot[1]]))
                | (u64::from(u16::from_le_bytes([slot[6], slot[7]])) << 16)
                | (u64::from(u32::from_le_bytes([slot[8], slot[9], slot[10], slot[11]])) << 32);
            if offset == 0 {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s produced descriptor carries a null entry offset"
                )));
            }
            // The declared dedicated critical stack must be reachable through
            // the gate's IST slot on this exact installed TSS — an absent IST
            // field names no dedicated stack at all.
            let Some(ist_slot) = descriptor.interrupt_stack_table_slot else {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s declared gate selects no IST slot for its dedicated critical stack"
                )));
            };
            if ist_classes.get(&ist_slot) != Some(&member.dedicated_stack_class) {
                return Err(ExternalRootDiagnostic(format!(
                    "vector {vector}'s declared IST slot does not resolve its dedicated critical stack class through the installed TSS"
                )));
            }
        }

        EstablishedInterruptTable::from_consumer(
            establishment,
            &self.profile,
            self.installed_code,
            self.artifact,
            self.members.iter().map(|(vector, member)| {
                (
                    *vector,
                    InterruptTableEstablishedMember {
                        root: member.root.root(),
                        entry: member.entry,
                    },
                )
            }),
            destination,
        )
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
