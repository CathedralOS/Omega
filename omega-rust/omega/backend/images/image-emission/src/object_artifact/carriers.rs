//! The sealed object-artifact carriers: the artifact, its functions,
//! dynamic conformance and descriptor tables, call stacks, settlements,
//! foreign calls, port effects and code attribution.

use crate::HostedReceiverBinding;
use machine_code::{
    BoundarySettlementRecord, PortEffectRecord, ScalarControlAffineCleanupRecord,
    SemanticCodeAttribution, StructuralReturnRecord,
};
use object_file::{ObjectPlan, ObjectSymbolHandle, RelocationPlan};
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use target_operations::{CallSiteOwner, TerminalPsiProvenance};
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectArtifact {
    pub(crate) hosted_receiver: Option<HostedReceiverBinding>,
    /// Primitive storage operations require retained initialization and access
    /// evidence even when no borrowed call exposes their local residence.
    pub(crate) requires_graph_storage_replay: bool,
    /// Complete common-pipeline replay inputs, independent of the current object.
    pub(crate) fragment_replay: Option<crate::function_fragments::replay::FragmentReplay>,
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) target: NativeTarget,
    /// Exact deployment profile for the source-free feature-requiring x86 FMA
    /// seam. Ordinary object construction retains `None` and rejects FMA.
    pub(crate) x86_feature_profile: Option<target::TargetProfile>,
    /// Consumed feature/differential authority for generic x86 FMA slots.
    /// The feature-required mechanics seam retains `None` and remains
    /// non-executable.
    pub(crate) x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub(crate) entry: MachineId,
    pub(crate) object: ObjectPlan,
    pub(crate) relocations: RelocationPlan,
    pub(crate) text_bytes: Vec<u8>,
    pub(crate) data_bytes: Vec<u8>,
    pub(crate) dynamic_conformance_tables: Vec<ObjectDynamicConformanceTable>,
    pub(crate) forwarded_dynamic_descriptor_adapters: Vec<ObjectForwardedDynamicDescriptorAdapter>,
    pub(crate) forwarded_dynamic_descriptor_tables: Vec<ObjectForwardedDynamicDescriptorTable>,
    pub(crate) functions: Vec<ObjectFunction>,
    pub(crate) private_functions: Vec<ObjectCompilerPrivateFunction>,
    pub(crate) semantic_code_attribution: Vec<ObjectCodeAttribution>,
    pub(crate) port_effects: Vec<ObjectPortEffect>,
    pub(crate) boundary_settlements: Vec<ObjectBoundarySettlement>,
    pub(crate) foreign_calls: Vec<ObjectForeignCall>,
}

impl ObjectArtifact {
    pub const fn hosted_receiver_binding(&self) -> Option<&HostedReceiverBinding> {
        self.hosted_receiver.as_ref()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn clear_fragment_replay_for_test(&mut self) {
        self.fragment_replay = None;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn semantic_code_attribution_mut_for_test(&mut self) -> &mut Vec<ObjectCodeAttribution> {
        &mut self.semantic_code_attribution
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn functions_mut_for_test(&mut self) -> &mut Vec<ObjectFunction> {
        &mut self.functions
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn boundary_settlements_mut_for_test(&mut self) -> &mut Vec<ObjectBoundarySettlement> {
        &mut self.boundary_settlements
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn text_bytes_mut_for_test(&mut self) -> &mut Vec<u8> {
        &mut self.text_bytes
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn object_mut_for_test(&mut self) -> &mut ObjectPlan {
        &mut self.object
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn relocations_mut_for_test(&mut self) -> &mut RelocationPlan {
        &mut self.relocations
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn foreign_calls_mut_for_test(&mut self) -> &mut Vec<ObjectForeignCall> {
        &mut self.foreign_calls
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn x86_feature_profile(&self) -> Option<target::TargetProfile> {
        self.x86_feature_profile
    }

    pub const fn x86_scalar_fma_provider(&self) -> Option<target::AdmittedX86ScalarFmaProvider> {
        self.x86_scalar_fma_provider
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn object(&self) -> &ObjectPlan {
        &self.object
    }

    pub const fn relocations(&self) -> &RelocationPlan {
        &self.relocations
    }

    pub fn text_bytes(&self) -> &[u8] {
        &self.text_bytes
    }

    pub fn data_bytes(&self) -> &[u8] {
        &self.data_bytes
    }

    pub fn dynamic_conformance_tables(&self) -> &[ObjectDynamicConformanceTable] {
        &self.dynamic_conformance_tables
    }

    pub fn forwarded_dynamic_descriptor_adapters(
        &self,
    ) -> &[ObjectForwardedDynamicDescriptorAdapter] {
        &self.forwarded_dynamic_descriptor_adapters
    }

    pub fn forwarded_dynamic_descriptor_tables(&self) -> &[ObjectForwardedDynamicDescriptorTable] {
        &self.forwarded_dynamic_descriptor_tables
    }

    pub fn functions(&self) -> &[ObjectFunction] {
        &self.functions
    }

    pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
        &self.private_functions
    }

    pub fn entry_function(&self) -> &ObjectFunction {
        self.functions
            .iter()
            .find(|function| function.machine == self.entry)
            .expect("artifact construction requires one entry function")
    }

    pub fn boundary_settlements(&self) -> &[ObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn foreign_calls(&self) -> &[ObjectForeignCall] {
        &self.foreign_calls
    }

    pub fn port_effects(&self) -> &[ObjectPortEffect] {
        &self.port_effects
    }

    pub fn semantic_code_attribution(&self) -> &[ObjectCodeAttribution] {
        &self.semantic_code_attribution
    }
}

impl installation_evidence::ObjectEvidence for ObjectArtifact {
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    fn target(&self) -> NativeTarget {
        self.target
    }

    fn text_bytes(&self) -> &[u8] {
        &self.text_bytes
    }

    fn function_text_offset(&self, machine: MachineId) -> Option<usize> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
            .map(|function| function.text_offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub scalar_abi: Option<target_operations::ScalarFunctionAbi>,
    pub mixed_structural_scalar_abi: Option<target_operations::MixedStructuralScalarFunctionAbi>,
    pub structural_call_scalar_return: Option<machine_code::StructuralCallScalarReturnEvidence>,
    pub parameter_abi: Option<machine_code::ParameterFunctionAbiRecord>,
    pub provenance: TerminalPsiProvenance,
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
    /// Independently replayed feature requirements for exact scalar FMA3
    /// intervals. These remain requirements, not executable admission.
    pub x86_scalar_fma: Vec<machine_code::X86ScalarFmaFragment>,
    pub x86_scalar_fma_occurrences: Vec<machine_code::X86ScalarFmaOccurrenceRecord>,
    pub x86_floating_control: Option<machine_code::X86FloatingControlRecord>,
    /// Byte-validated stack facts for a completely accounted Unit body.
    pub unit_stack: Option<ObjectUnitStack>,
    /// Byte-validated stack facts for a branch-free scalar body.
    pub scalar_stack: Option<ObjectScalarStack>,
    pub unit_call_stacks: Vec<ObjectUnitCallStack>,
    pub scalar_call_stacks: Vec<ObjectScalarCallStack>,
    pub internal_unit_calls: Vec<machine_code::InternalUnitCallRecord>,
    pub internal_unit_scalar_calls: Vec<machine_code::InternalUnitScalarCallRecord>,
    pub installed_provider_unit_scalar_calls:
        Vec<machine_code::InstalledProviderUnitScalarCallRecord>,
    pub dynamic_calls: Vec<machine_code::DynamicCallRecord>,
    pub stored_dynamic_calls: Vec<machine_code::StoredDynamicCallRecord>,
    pub dynamic_parameter_calls: Vec<machine_code::DynamicParameterCallRecord>,
    pub forwarded_dynamic_parameter_calls: Vec<machine_code::ForwardedDynamicParameterCallRecord>,
    pub forwarded_dynamic_descriptor_calls: Vec<machine_code::ForwardedDynamicDescriptorCallRecord>,
    pub unit_scalar_homes: Vec<machine_code::UnitScalarHomeRecord>,
    pub unit_integer_constants: Vec<machine_code::UnitIntegerConstantRecord>,
    pub unit_affine_scalar_records: Vec<machine_code::UnitAffineScalarRecordEstablishmentRecord>,
    pub unit_structural_scalar_field_stores:
        Vec<machine_code::UnitStructuralScalarFieldStoreRecord>,
    pub unit_write_only_primitive_stores: Vec<machine_code::UnitWriteOnlyPrimitiveStoreRecord>,
    pub scalar_structural_scalar_field_stores:
        Vec<machine_code::ScalarStructuralScalarFieldStoreRecord>,
    pub unit_parameters: Vec<machine_code::UnitParameterRecord>,
    pub unit_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
    pub unit_continuations: Vec<machine_code::UnitContinuationRecord>,
    pub unit_affine_cleanup: Option<machine_code::UnitAffineCleanupRecord>,
    pub scalar_affine_cleanup: Option<machine_code::UnitAffineCleanupRecord>,
    /// Three byte-validated scalar cleanup records in canonical physical/DFS
    /// return-leaf order for the bounded two-decision Boolean control lane.
    pub scalar_control_affine_cleanups: Vec<ScalarControlAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<machine_code::UnitParameterRecord>,
    pub scalar_structural_parameter_homes: Vec<machine_code::UnitParameterHomeRecord>,
    /// Byte-validated structural custody returned by this function, when the
    /// complete one-fragment slice applies.
    pub structural_return: Option<StructuralReturnRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDynamicConformanceTable {
    pub application: terminal_psi::ClosedConformanceApplication,
    pub symbol: ObjectSymbolHandle,
    pub data_offset: usize,
    pub byte_count: usize,
    pub slots: Vec<ObjectDynamicConformanceSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDynamicConformanceSlot {
    pub row_index: u32,
    pub realization_callable_identity: Option<String>,
    pub target: Option<MachineId>,
    pub target_symbol: Option<ObjectSymbolHandle>,
    pub data_offset: usize,
}

/// One independently replayed erased-to-concrete bridge emitted outside the
/// Terminal machine namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectForwardedDynamicDescriptorAdapter {
    pub record: machine_code::ForwardedDynamicDescriptorAdapterRecord,
    pub symbol: ObjectSymbolHandle,
    pub target_symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
}

impl ObjectForwardedDynamicDescriptorAdapter {
    pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
        &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
    }
}

/// Role-specific descriptor table whose slots address erased-ABI adapters,
/// never concrete realization functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectForwardedDynamicDescriptorTable {
    pub application: terminal_psi::ClosedConformanceApplication,
    pub symbol: ObjectSymbolHandle,
    pub data_offset: usize,
    pub byte_count: usize,
    pub slots: Vec<ObjectForwardedDynamicDescriptorSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectForwardedDynamicDescriptorSlot {
    pub row_index: u32,
    pub adapter: machine_code::ForwardedDynamicDescriptorAdapterIdentity,
    pub adapter_symbol: ObjectSymbolHandle,
    pub data_offset: usize,
}

impl ObjectFunction {
    pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
        &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
    }
}

/// One independently validated compiler-private callback function in the
/// object's text section. Its artifact-local `MachineId` remains nested under
/// this carrier and never joins the semantic program-function namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectCompilerPrivateFunction {
    pub identity: function_identity::MachineFunctionIdentity,
    pub source_psi: TerminalPsiIdentity,
    pub function: ObjectFunction,
}

impl ObjectCompilerPrivateFunction {
    pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
        self.function.bytes(artifact)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectUnitCallStack {
    pub owner: CallSiteOwner,
    pub target: MachineId,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
    pub active_frame_bytes: u32,
    pub transient_bytes: u32,
    pub caller_live_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectScalarCallStack {
    pub owner: CallSiteOwner,
    pub target: MachineId,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
    pub caller_live_bytes: u32,
}

/// Stack quantities recomputed by object construction from exact validated
/// target instructions. No producer-supplied numeric peak crosses this
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectUnitStack {
    pub frame_bytes: u32,
    pub local_peak_bytes: u32,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectScalarStack {
    pub local_peak_bytes: u32,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectBoundarySettlement {
    pub machine: MachineId,
    pub settlement: BoundarySettlementRecord,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
}

/// Source-free custody for one normalized foreign call retained after object
/// construction has independently replayed its instruction and relocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectForeignCall {
    pub machine: MachineId,
    pub owner: CallSiteOwner,
    /// Ordinal of the exact normalized foreign operation owning the complete
    /// semantic-code attribution interval.
    pub operation_ordinal: usize,
    pub locator: target::NormalizedForeignLocator,
    pub provider_execution: machine_code::ProviderExecutionRecord,
    pub boundary_entry_plan: calling_conventions::BoundaryEntryPlan,
    /// Exact physical caller frontier independently reconstructed from emitted
    /// stack instructions before the opaque foreign leaf begins.
    pub caller_live_bytes: u32,
    /// Sealed admitted demand for the opaque same-stack foreign leaf.
    pub same_stack_contribution: task_plans::AdmittedSameStackContribution,
    /// Exact scalar argument materializations retained from machine emission.
    /// Every code offset is rebased to absolute object `.text`.
    pub scalar_arguments: Vec<machine_code::ForeignCallScalarArgumentRecord>,
    /// Exact compiler-private callback address materialized before this call.
    /// Every byte offset has been rebased to absolute object `.text`.
    pub callback_address: Option<machine_code::CallbackAddressMaterialization>,
    /// Result custody retained from machine emission. Its code offset is
    /// rebased to absolute object `.text`, like `text_offset` below.
    pub scalar_result: Option<machine_code::ForeignCallScalarResultRecord>,
    /// Absolute object-text intervals proving complete MXCSR preservation for
    /// this returning x86 foreign call.
    pub x86_floating_control: Option<machine_code::X86ForeignCallFloatingControlRecord>,
    /// Absolute object-text intervals proving complete FPCR preservation for
    /// this returning AArch64 foreign call.
    pub aarch64_floating_control: Option<machine_code::Aarch64ForeignCallFloatingControlRecord>,
    /// Absolute offset of the mutable relocation field in object `.text`.
    pub text_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPortEffect {
    pub machine: MachineId,
    pub effect: PortEffectRecord,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectCodeAttribution {
    pub machine: MachineId,
    pub attribution: SemanticCodeAttribution,
    pub text_offset: usize,
}
