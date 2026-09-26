//! Interrupt table publications, receipts, refusals, scopes, authorities
//! and executed publications.

use crate::executable_installation::{ArtifactId, InstalledCode, InstalledCodeId};
use crate::external_roots::interrupts::interrupt_table::{
    EstablishedInterruptTable, INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES,
    InterruptDescriptorTableState, InterruptTableDescriptorOperand,
    InterruptTableEstablishedMember,
};
use crate::external_roots::{
    ExternalRootDiagnostic, InterruptTableEstablishmentId, InterruptTableProfileId,
    InterruptTablePublicationAuthorityId, InterruptTablePublicationId,
    InterruptTablePublicationReceiptId,
};
use abstract_operations_to_target_operations::calling_conventions::MachineRegister;
use std::collections::{BTreeMap, BTreeSet};
use target::Architecture;
use terminal_psi::extents::Extent;

/// The exact carrier handed to the provider edge that performs the checked
/// table publication. It binds the established value, the complete admitted
/// member set, the supplied publication authority, and the installed
/// realization; a completion receipt is only meaningful against this carrier.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptTablePublication {
    pub(crate) publication: InterruptTablePublicationId,
    pub(crate) authority: InterruptTablePublicationAuthorityId,
    pub(crate) established: EstablishedInterruptTable,
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
///
/// Receipts are minted only inside this module by the checked
/// publication-instruction edge
/// ([`InterruptTablePublication::execute_checked_publication`]): external code
/// cannot forge a receipt for a carrier it never executed under the bound
/// authority and operand contract.
#[derive(Debug, PartialEq, Eq)]
pub struct InterruptTablePublicationReceipt {
    pub(crate) identity: InterruptTablePublicationReceiptId,
    pub(crate) publication: InterruptTablePublicationId,
    pub(crate) authority: InterruptTablePublicationAuthorityId,
    pub(crate) published: bool,
}

impl InterruptTablePublicationReceipt {
    pub(crate) fn from_provider(
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

    /// Whether the provider's answer published the table (`true`) or declined
    /// the attempt (`false`).
    pub const fn published(&self) -> bool {
        self.published
    }
}

/// Evidence that one carrier published the complete declared table: the
/// member roots are now reachable through the established descriptor table.
/// The retained destination extent stays linear for the table's life.
#[derive(Debug, PartialEq, Eq)]
pub struct PublishedInterruptTable {
    pub(crate) publication: InterruptTablePublicationId,
    pub(crate) receipt: InterruptTablePublicationReceiptId,
    pub(crate) authority: InterruptTablePublicationAuthorityId,
    pub(crate) establishment: InterruptTableEstablishmentId,
    pub(crate) profile: InterruptTableProfileId,
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) artifact: ArtifactId,
    pub(crate) destination: Extent,
    pub(crate) members: BTreeMap<u8, InterruptTableEstablishedMember>,
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
    pub(crate) publication: InterruptTablePublicationId,
    pub(crate) receipt: InterruptTablePublicationReceiptId,
    pub(crate) established: EstablishedInterruptTable,
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

/// Closed scope legs one consumer descriptor-table publication authority may
/// declare.
///
/// The checked publication instruction both changes the executing
/// processor's descriptor-table register and makes the established table
/// reachable to hardware arrivals, so the provider edge requires the
/// exercised authority to declare both legs. A token covering only one leg
/// cannot lawfully answer a carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptTablePublicationScope {
    /// Change the executing processor's descriptor-table register state.
    ProcessorTableControl,
    /// Make an established interrupt table reachable to hardware arrivals.
    TablePublication,
}

/// Consumer-issued authority exercised at the checked publication
/// instruction.
///
/// Issuance policy is the consumer's: the provider edge cannot mint, widen,
/// or split authorities. It replays that the exercised token names the exact
/// identity the carrier bound at issuance, declares both required scope
/// legs, and binds the carrier's installed realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterruptTablePublicationAuthority {
    identity: InterruptTablePublicationAuthorityId,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    scopes: BTreeSet<InterruptTablePublicationScope>,
}

impl InterruptTablePublicationAuthority {
    /// The consumer's issuance edge. An authority declaring no scope leg can
    /// never answer a carrier, so it rejects here rather than at execution.
    pub fn from_consumer(
        identity: InterruptTablePublicationAuthorityId,
        installed_code: InstalledCodeId,
        artifact: ArtifactId,
        scopes: impl IntoIterator<Item = InterruptTablePublicationScope>,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let scopes = scopes.into_iter().collect::<BTreeSet<_>>();
        if scopes.is_empty() {
            return Err(ExternalRootDiagnostic(
                "interrupt-table publication authority declares no scope".into(),
            ));
        }
        Ok(Self {
            identity,
            installed_code,
            artifact,
            scopes,
        })
    }

    pub const fn identity(&self) -> InterruptTablePublicationAuthorityId {
        self.identity
    }

    /// Whether the consumer declared this scope leg on the exercised token.
    pub fn covers(&self, scope: InterruptTablePublicationScope) -> bool {
        self.scopes.contains(&scope)
    }
}

/// Minted-once evidence that the checked provider edge answered one issued
/// carrier.
///
/// The bundle retains the consumed carrier and operand custody, records the
/// instruction contract's fixed scratch clobber (the x86-64 `lidt` contract
/// stages its operand pointer through `r10`), and — only when the provider
/// published — the descriptor-table register state the instruction
/// installed. The minted receipt is meaningful only to
/// [`InterruptTableLedger::complete_interrupt_table_publication`], which
/// still requires it to name the exact carrier.
#[derive(Debug, PartialEq, Eq)]
pub struct ExecutedInterruptTablePublication {
    carrier: InterruptTablePublication,
    receipt: InterruptTablePublicationReceipt,
    operand: InterruptTableDescriptorOperand,
    scratch_clobber: MachineRegister,
    installed_state: Option<InterruptDescriptorTableState>,
}

impl ExecutedInterruptTablePublication {
    pub const fn carrier(&self) -> &InterruptTablePublication {
        &self.carrier
    }

    /// The minted receipt naming the exact carrier and exercised authority.
    pub const fn receipt(&self) -> &InterruptTablePublicationReceipt {
        &self.receipt
    }

    /// The accounted operand read the instruction consumed.
    pub const fn operand(&self) -> &InterruptTableDescriptorOperand {
        &self.operand
    }

    /// The instruction contract's fixed scratch clobber.
    pub const fn scratch_clobber(&self) -> MachineRegister {
        self.scratch_clobber
    }

    /// The descriptor-table register state the instruction installed;
    /// present exactly when the provider published the table.
    pub const fn installed_state(&self) -> Option<InterruptDescriptorTableState> {
        self.installed_state
    }

    /// Whether the provider published the table rather than declining the
    /// attempt.
    pub const fn is_published(&self) -> bool {
        self.installed_state.is_some()
    }

    /// Decompose for ledger completion: the carrier and its minted receipt
    /// go to
    /// [`InterruptTableLedger::complete_interrupt_table_publication`], the
    /// operand site returns to provider scratch custody, and any installed
    /// register state remains consumer-visible evidence.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        InterruptTablePublication,
        InterruptTablePublicationReceipt,
        InterruptTableDescriptorOperand,
        Option<InterruptDescriptorTableState>,
    ) {
        (
            self.carrier,
            self.receipt,
            self.operand,
            self.installed_state,
        )
    }
}

/// Rejected provider-edge execution. The consumed carrier and operand are
/// returned so a corrected attempt can be built without re-issuing the
/// publication.
#[derive(Debug)]
pub struct InterruptTableProviderError {
    carrier: InterruptTablePublication,
    operand: InterruptTableDescriptorOperand,
    diagnostic: ExternalRootDiagnostic,
}

impl InterruptTableProviderError {
    pub const fn diagnostic(&self) -> &ExternalRootDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InterruptTablePublication, InterruptTableDescriptorOperand) {
        (self.carrier, self.operand)
    }
}

impl InterruptTablePublication {
    /// Execute the checked descriptor-table-load provider edge for this
    /// issued carrier — on x86-64, the provider-only `lidt` contract.
    ///
    /// This is the sole minting boundary for
    /// [`InterruptTablePublicationReceipt`]. The edge replays, in order:
    /// the carrier's installed realization and the x86-64 instruction
    /// contract against `installed_code`; the exercised authority's bound
    /// identity, realization scope, and both required scope legs; and the
    /// declared pseudo-descriptor against the exact established
    /// destination, including its accounted 10-byte read site in the
    /// table's address space and non-aliasing between the operand read and
    /// the published table.
    ///
    /// `published == false` declines the attempt under the same checks —
    /// the minted refusal receipt still names this exact carrier and
    /// authority, so the ledger can return the established value for a
    /// retry under a fresh publication identity.
    pub fn execute_checked_publication(
        self,
        installed_code: &InstalledCode,
        authority: &InterruptTablePublicationAuthority,
        operand: InterruptTableDescriptorOperand,
        receipt: InterruptTablePublicationReceiptId,
        published: bool,
    ) -> Result<ExecutedInterruptTablePublication, Box<InterruptTableProviderError>> {
        let reject = |diagnostic: &str,
                      carrier: InterruptTablePublication,
                      operand: InterruptTableDescriptorOperand| {
            Err(Box::new(InterruptTableProviderError {
                carrier,
                operand,
                diagnostic: ExternalRootDiagnostic(diagnostic.into()),
            }))
        };

        if installed_code.identity() != self.established.installed_code
            || installed_code.artifact() != self.established.artifact
        {
            return reject(
                "checked table publication cannot answer a carrier for a different installed realization",
                self,
                operand,
            );
        }
        if installed_code.architecture() != Architecture::X86_64 {
            return reject(
                "the x86-64 descriptor-table-load contract cannot publish a foreign-architecture table",
                self,
                operand,
            );
        }
        if authority.identity != self.authority {
            return reject(
                "the exercised authority is not the carrier's bound publication authority",
                self,
                operand,
            );
        }
        if authority.installed_code != self.established.installed_code
            || authority.artifact != self.established.artifact
        {
            return reject(
                "the exercised authority is scoped to a different installed realization",
                self,
                operand,
            );
        }
        if !authority.covers(InterruptTablePublicationScope::ProcessorTableControl)
            || !authority.covers(InterruptTablePublicationScope::TablePublication)
        {
            return reject(
                "the exercised authority does not declare both processor table control and table publication",
                self,
                operand,
            );
        }
        if operand.site.length() != INTERRUPT_TABLE_DESCRIPTOR_OPERAND_BYTES {
            return reject(
                "the descriptor operand read site is not the 10-byte pseudo-descriptor",
                self,
                operand,
            );
        }
        if operand.site.address_space() != self.established.destination.address_space() {
            return reject(
                "the descriptor operand read site is outside the published table's address space",
                self,
                operand,
            );
        }
        let operand_end = operand.site.base().checked_add(operand.site.length());
        let table_end = self
            .established
            .destination
            .base()
            .checked_add(self.established.destination.length());
        let aliases = match (operand_end, table_end) {
            (Some(operand_end), Some(table_end)) => {
                operand.site.base() < table_end && self.established.destination.base() < operand_end
            }
            _ => true,
        };
        if aliases {
            return reject(
                "the descriptor operand read must not alias the published table",
                self,
                operand,
            );
        }
        let names_destination = self
            .established
            .destination
            .length()
            .checked_sub(1)
            .is_some_and(|limit| {
                u64::from(operand.limit) == limit
                    && operand.base == self.established.destination.base()
            });
        if !names_destination {
            return reject(
                "the declared pseudo-descriptor does not name the exact established destination",
                self,
                operand,
            );
        }

        let receipt = InterruptTablePublicationReceipt::from_provider(receipt, &self, published);
        let installed_state = published.then_some(InterruptDescriptorTableState {
            publication: self.publication,
            base: self.established.destination.base(),
            limit: operand.limit,
        });
        Ok(ExecutedInterruptTablePublication {
            carrier: self,
            receipt,
            operand,
            scratch_clobber: MachineRegister::X86R10,
            installed_state,
        })
    }
}
