//! Admitted provider and callback facts consumed by target lowering.
//! These carry execution evidence, not another program representation.

use calling_conventions::NativePlace;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use target_operations::BoundarySettlementRealization;

/// Untrusted native callback proposal. Projection retains complete application
/// data, but only native admission can establish its selected method authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackArgumentProposal {
    pub registrar_invokes_callback: bool,
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    /// The destination selected by the registrar. For a field destination,
    /// `application` carries the root record pointer and `field_placements`
    /// describes every part of the handler address written into that record.
    pub destination: NativePlace,
    pub field_placements: Option<calling_conventions::PrivateCallbackPlacements>,
    pub application: calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    pub registrar_context: calling_conventions::CallbackMaterializationContext,
    /// Nonempty compiler-origin application-v3 commitment projection.
    pub registrar_application_commitment: [u8; 32],
    pub registrar_application: calling_conventions::BoundaryCallingApplication,
    pub binder: calling_conventions::StaticMachineBinderId,
}

/// A checked handler's proposed target placement at an installation operation.
/// It retains executable identity and the complete target plan, but supplies
/// no table backing, arrival authority or permission to publish an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInterruptNativeEntryProposal {
    pub operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub placement: backend_plan::NativeCallbackPlacement,
}

/// An installed entry whose exact placement passed installation admission.
/// The proposal remains readable but cannot be replaced or mutated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledInterruptEntry {
    proposal: InstalledInterruptNativeEntryProposal,
    entry: AdmittedCallbackEntry,
}
impl AdmittedInstalledInterruptEntry {
    pub(super) fn admit(
        proposal: InstalledInterruptNativeEntryProposal,
        entry: AdmittedCallbackEntry,
    ) -> Self {
        Self { proposal, entry }
    }
    pub fn entry(&self) -> &AdmittedCallbackEntry {
        &self.entry
    }
}
impl std::ops::Deref for AdmittedInstalledInterruptEntry {
    type Target = InstalledInterruptNativeEntryProposal;
    fn deref(&self) -> &Self::Target {
        &self.proposal
    }
}

/// The exact retained operands and checked continuations of one installation.
/// Native lowering allocates the root in this operation's activation; the
/// nonreturning work call keeps these ordinary loans alive after publication.
/// This record is constructed only after operation admission, never from a
/// callback's type or from equal-looking table bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledInterruptScope {
    pub(super) service: semantic_vocabulary::ServiceId,
    pub(super) operation: OperationId,
    pub(super) boundary: BoundaryMachineId,
    pub(super) arguments: [terminal_psi::StructuralArgument; 6],
    pub(super) table_type: semantic_vocabulary::StructuralTypeId,
    pub(super) context_type: semantic_vocabulary::StructuralTypeId,
    pub(super) validator: semantic_vocabulary::MachineId,
    pub(super) work: semantic_vocabulary::MachineId,
    pub(super) completion: semantic_vocabulary::MachineId,
    pub(super) acknowledgement_application: u32,
    pub(super) fatal_entries: Vec<AdmittedFatalInterruptEntry>,
    pub(super) anchor: Option<AdmittedInterruptContextAnchor>,
    pub(super) installation_assumptions:
        Vec<installation_evidence::InstallationEnvironmentAssumption>,
}

/// The designated timer gate used to recover the installation's live root.
/// Its stack class does not determine its hardware IST selector. Arrival reads
/// that selector from this exact gate after the installer has checked its lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInterruptContextAnchor {
    pub(super) application: u32,
    pub(super) function: function_identity::MachineFunctionIdentity,
    pub(super) destination: NativePlace,
    pub(super) gate_byte_offset: u32,
    pub(super) table_bytes: u32,
}

impl AdmittedInterruptContextAnchor {
    pub const fn application(&self) -> u32 {
        self.application
    }
    pub const fn function(&self) -> function_identity::MachineFunctionIdentity {
        self.function
    }
    pub fn destination(&self) -> &NativePlace {
        &self.destination
    }
    pub const fn gate_byte_offset(&self) -> u32 {
        self.gate_byte_offset
    }
    pub const fn table_bytes(&self) -> u32 {
        self.table_bytes
    }
}

/// A nonreturning handler that borrows this installation's retained context.
/// Its body identity is distinct from the timer anchor used to find that context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedFatalInterruptEntry {
    pub(super) application: u32,
    pub(super) gate_byte_offset: u32,
    pub(super) machine: semantic_vocabulary::MachineId,
}

impl AdmittedFatalInterruptEntry {
    pub const fn gate_byte_offset(&self) -> u32 {
        self.gate_byte_offset
    }
    pub const fn application(&self) -> u32 {
        self.application
    }

    pub const fn machine(&self) -> semantic_vocabulary::MachineId {
        self.machine
    }
}

impl AdmittedInstalledInterruptScope {
    /// A retained caller assumption, never a measured or proved CPU fact.
    pub fn bootstrap_assumption(
        &self,
    ) -> Option<&installation_evidence::BootstrapEnvironmentAssumption> {
        self.installation_assumptions
            .iter()
            .find_map(|row| row.bootstrap())
    }

    pub fn interrupt_delivery_assumption(
        &self,
    ) -> Option<&installation_evidence::X86InterruptDeliveryAssumption> {
        self.installation_assumptions
            .iter()
            .find_map(|row| row.interrupt_delivery())
    }

    pub fn installation_assumptions(
        &self,
    ) -> &[installation_evidence::InstallationEnvironmentAssumption] {
        &self.installation_assumptions
    }

    pub fn context_anchor(&self) -> Option<&AdmittedInterruptContextAnchor> {
        self.anchor.as_ref()
    }
    pub const fn service(&self) -> semantic_vocabulary::ServiceId {
        self.service
    }
    pub const fn operation(&self) -> OperationId {
        self.operation
    }
    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }
    pub const fn arguments(&self) -> &[terminal_psi::StructuralArgument; 6] {
        &self.arguments
    }
    pub const fn table_type(&self) -> semantic_vocabulary::StructuralTypeId {
        self.table_type
    }
    pub const fn context_type(&self) -> semantic_vocabulary::StructuralTypeId {
        self.context_type
    }
    pub const fn validator(&self) -> semantic_vocabulary::MachineId {
        self.validator
    }
    pub const fn work(&self) -> semantic_vocabulary::MachineId {
        self.work
    }
    pub const fn completion(&self) -> semantic_vocabulary::MachineId {
        self.completion
    }
    pub const fn acknowledgement_application(&self) -> u32 {
        self.acknowledgement_application
    }
    pub fn fatal_entries(&self) -> &[AdmittedFatalInterruptEntry] {
        &self.fatal_entries
    }
}

/// A helper's independently checked token route to one retained installation.
/// The operation may be shared by several roots; the actual acknowledgement
/// selects its live root at runtime rather than a symbol-keyed global cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledInterruptHelper {
    pub(super) installation: OperationId,
    pub(super) callback_application: u32,
    pub(super) operation: OperationId,
    pub(super) kind: language_core::static_interrupts::StaticInterruptOperation,
}

impl AdmittedInstalledInterruptHelper {
    pub const fn installation(&self) -> OperationId {
        self.installation
    }
    pub const fn callback_application(&self) -> u32 {
        self.callback_application
    }
    pub const fn operation(&self) -> OperationId {
        self.operation
    }
    pub const fn kind(&self) -> language_core::static_interrupts::StaticInterruptOperation {
        self.kind
    }
}

/// Immutable callback admitted for the exact verified optimization input.
/// No public constructor or mutable access exists. Cloning cannot attach a row
/// to another input; target lowering borrows the input's sealed cohort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackArgument {
    proposal: NativeCallbackArgumentProposal,
    entry: AdmittedCallbackEntry,
}
impl AdmittedNativeCallbackArgument {
    pub(super) fn admit(
        proposal: NativeCallbackArgumentProposal,
        entry: AdmittedCallbackEntry,
    ) -> Self {
        Self { proposal, entry }
    }
    pub fn entry(&self) -> &AdmittedCallbackEntry {
        &self.entry
    }
}

/// The handler ABI selected at admission, before an object or adapter exists.
/// It remains separate from the public emitted metadata that later audits join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedCallbackEntry {
    pub(super) psi: terminal_psi::TerminalPsiIdentity,
    pub(super) target: target::NativeTarget,
    pub(super) machine: semantic_vocabulary::MachineId,
    pub(super) inbound: calling_conventions::BoundaryEntryPlan,
    pub(super) inbound_ordinals: Vec<u32>,
}
impl AdmittedCallbackEntry {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(&self) -> target::NativeTarget {
        self.target
    }
    pub const fn machine(&self) -> semantic_vocabulary::MachineId {
        self.machine
    }
    pub fn inbound(&self) -> &calling_conventions::BoundaryEntryPlan {
        &self.inbound
    }
    pub fn inbound_ordinals(&self) -> &[u32] {
        &self.inbound_ordinals
    }
}
impl std::ops::Deref for AdmittedNativeCallbackArgument {
    type Target = NativeCallbackArgumentProposal;
    fn deref(&self) -> &Self::Target {
        &self.proposal
    }
}

/// One boundary realization sourced from a validated, admitted provider
/// execution or the consuming lowerer's closed compiler-builtin catalog.
#[derive(Debug, Clone)]
pub struct AdmittedBoundarySettlement<'execution> {
    pub boundary: BoundaryMachineId,
    pub execution: AdmittedBoundaryExecution<'execution>,
    pub realization: BoundarySettlementRealization,
}

#[derive(Debug, Clone, Copy)]
pub enum AdmittedBoundaryExecution<'execution> {
    Provider(&'execution dyn installation_evidence::ProviderExecutionEvidence),
    CompilerBuiltin(super::SettledCompilerBuiltin),
}
