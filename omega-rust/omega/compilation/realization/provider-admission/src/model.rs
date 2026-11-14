// Request inputs and admitted provider, interrupt, and callback carriers
// consumed by target lowering.
//
// `ProviderAdmissionRequest`, `NativeProviderSettlement`,
// `NativeBoundaryRealization`, and `NativeCompilerBuiltinSettlement` describe
// the invocation-local inputs that native realization supplies to admission.
// The admitted records (`AdmittedInstalledInterruptEntry`,
// `AdmittedInstalledInterruptScope`, `AdmittedNativeCallbackArgument`,
// `AdmittedCallbackEntry`, and `AdmittedBoundarySettlement`) carry sealed
// execution evidence rather than another program representation; their fields
// are constructed only after admission checks succeed.

use calling_conventions::NativePlace;
use installation_evidence::ProviderExecutionEvidence;
use semantic_vocabulary::{BoundaryMachineId, OperationId};
use target_operations::{BoundaryRealization, BoundarySettlementRealization};

use super::{TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy};

#[derive(Debug, Clone, Copy)]
pub enum NativeBoundaryRealization<'execution> {
    Builtin(BoundaryRealization),
    NormalizedForeignCall(&'execution task_plans::AdmittedSameStackContribution),
    Component {
        call: crate::ComponentCallFacts<'execution>,
        authority: &'execution super::AdmittedNativeCallableAuthority,
        locator: &'execution target::NormalizedComponentLocator,
        same_stack: &'execution task_plans::AdmittedSameStackContribution,
    },
    SystemCall, // Direct kernel entry with no foreign callee on the receiver stack.
}

impl<'execution> From<BoundaryRealization> for NativeBoundaryRealization<'execution> {
    fn from(realization: BoundaryRealization) -> Self {
        Self::Builtin(realization)
    }
}

macro_rules! builtin_native_realization_conversion {
    ($realization:ty) => {
        impl<'execution> From<$realization> for NativeBoundaryRealization<'execution> {
            fn from(realization: $realization) -> Self {
                Self::Builtin(realization.into())
            }
        }
    };
}

builtin_native_realization_conversion!(target_operations::MetadataOnlyPortRealization);
builtin_native_realization_conversion!(target_operations::DirectPortReadU8Realization);
builtin_native_realization_conversion!(target_operations::LinuxWriteLineRealization);
builtin_native_realization_conversion!(target_operations::HostedExitProcessI32Realization);
builtin_native_realization_conversion!(target_operations::HostedReadByteRealization);
builtin_native_realization_conversion!(target_operations::ClaimCompletionOnlyRealization);

#[derive(Debug, Clone, Copy)]
pub struct NativeProviderSettlement<'execution> {
    pub provider_execution: &'execution dyn ProviderExecutionEvidence,
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan, // Complete selected-plan evidence; report identity alone cannot authorize a plan.
    pub realization: NativeBoundaryRealization<'execution>,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCompilerBuiltinSettlement<'execution> {
    pub requirement_identity: &'execution str,
    pub provider_plan: &'execution effects::provider_plan::ProviderPlan,
    pub semantic_schema: &'execution effects::provider_plan::ServiceSchema, // Contract authenticated by intrinsic classification before callable-identity replacement.
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub settlement: crate::provider_admission::TerminalCompilerBuiltinSettlement,
}

#[derive(Clone, Copy)]
pub struct ProviderAdmissionRequest<'request, 'execution> {
    pub profile: target::TargetProfile,
    pub bootstrap_entry: semantic_vocabulary::MachineId,
    pub program_provider_context:
        Option<&'request std::sync::Arc<super::AdmittedProgramProviderContext>>,
    pub installation_assumptions:
        &'request [installation_evidence::InstallationEnvironmentAssumption],
    pub selected_provider_plans: &'request effects::SelectedProviderPlanFacts,
    pub checked_adapters: &'request [crate::SelectedProviderAdapter],
    pub external_binding_rows: &'request [calling_conventions::ExternalBindingRow],
    pub settlements: &'request [NativeProviderSettlement<'execution>],
    pub compiler_builtins: &'request [NativeCompilerBuiltinSettlement<'request>],
    pub native_callbacks: &'request [crate::NativeCallbackArgumentProposal],
    pub installed_interrupts: &'request [crate::InstalledInterruptNativeEntryProposal],
    pub callback_occurrences: &'request [backend_plan::TerminalCallbackOccurrenceProposal],
    pub installed_calling: &'request [calling_conventions::BoundaryCallingApplication],
    pub terminal_authority_policy: &'request TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: &'request Option<TerminalAuthorityPermissionPolicy>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

impl ProviderAdmissionRequest<'_, '_> {
    pub fn target(&self) -> target::NativeTarget {
        self.profile.native_target()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackArgumentProposal {
    pub registrar_invokes_callback: bool,
    pub terminal_operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub destination: NativePlace,
    pub field_placements: Option<calling_conventions::PrivateCallbackPlacements>,
    pub application: calling_conventions::NativeParameterApplication,
    pub registrar_boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    pub registrar_context: calling_conventions::CallbackMaterializationContext,
    pub registrar_application_commitment: [u8; 32], // Nonempty compiler-origin application-v3 commitment projection.
    pub registrar_application: calling_conventions::BoundaryCallingApplication,
    pub binder: calling_conventions::StaticMachineBinderId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledInterruptNativeEntryProposal {
    pub operation: OperationId,
    pub placement_index: usize,
    pub callback_function: function_identity::MachineFunctionIdentity,
    pub placement: backend_plan::NativeCallbackPlacement,
}

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
