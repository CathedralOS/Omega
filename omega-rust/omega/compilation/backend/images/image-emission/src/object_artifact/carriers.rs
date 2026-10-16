//! The sealed object-artifact carriers: the artifact, its functions, call
//! stacks, settlements, foreign calls, port effects and code attribution.

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
    pub(crate) physical_entry: Option<crate::PhysicalEntryBinding>,
    /// Primitive storage operations require retained initialization and access
    /// evidence even when no borrowed call exposes their local residence.
    pub(crate) requires_graph_storage_replay: bool,
    /// Complete common-pipeline replay inputs, independent of the current object.
    pub(crate) fragment_replay: Option<crate::function_fragments::replay::FragmentReplay>,
    /// Unlinked root projection retained by independent composition replay.
    pub(crate) composition_source: Option<std::sync::Arc<ObjectArtifact>>,
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
    pub(crate) functions: Vec<ObjectFunction>,
    pub(crate) private_functions: Vec<ObjectCompilerPrivateFunction>,
    pub(crate) semantic_code_attribution: Vec<ObjectCodeAttribution>,
    pub(crate) port_effects: Vec<ObjectPortEffect>,
    pub(crate) boundary_settlements: Vec<ObjectBoundarySettlement>,
    pub(crate) foreign_calls: Vec<ObjectForeignCall>,
    /// The read-only image storage of every byte literal the program's code
    /// addresses, placed after the compiler's code; `None` without literals.
    pub(crate) byte_literal_pool: Option<ObjectByteLiteralPool>,
    pub(crate) implementation_table_pool:
        Option<crate::function_fragments::implementation_tables::ObjectImplementationTablePool>,
}

/// The byte-literal pool: one object-kind text symbol over the deduplicated
/// literal octets, zero-padded to a 16-byte boundary so later text stays
/// aligned. Every literal address field binds this symbol with the literal's
/// pool offset as its addend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectByteLiteralPool {
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
    pub addresses: Vec<ObjectByteLiteralAddress>,
}

/// One literal address in the program text and the octets it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectByteLiteralAddress {
    pub machine: MachineId,
    pub literal: machine_code::ByteLiteralReference,
    /// Text-section coordinates of the instruction's unresolved field(s).
    pub fields: machine_code::ByteLiteralAddressFields,
    /// Offset of the literal's octets from the pool symbol.
    pub pool_offset: usize,
    pub byte_count: usize,
}

impl ObjectArtifact {
    pub const fn physical_entry_binding(&self) -> Option<&crate::PhysicalEntryBinding> {
        self.physical_entry.as_ref()
    }

    pub const fn byte_literal_pool(&self) -> Option<&ObjectByteLiteralPool> {
        self.byte_literal_pool.as_ref()
    }

    pub const fn hosted_receiver_binding(&self) -> Option<&HostedReceiverBinding> {
        self.hosted_receiver.as_ref()
    }
    #[cfg(any(test, feature = "test-support"))]
    #[cfg(any(test, feature = "test-support"))]
    #[cfg(any(test, feature = "test-support"))]
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

    /// Audit the immutable portable root against the actual installed entry.
    /// The retained deployment selection came from native admission, not an
    /// architecture-derived tag or a choice among otherwise valid members.
    pub fn validate_terminal_entry(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        let identity = artifact.view().identity();
        let module = artifact.semantic_module();
        if self.psi != identity || module.machine(self.entry).is_none() {
            return Err("native object entry has no exact Terminal body or identity");
        }
        let installed_entry = self
            .fragment_replay
            .as_ref()
            .map(|replay| replay.selected_terminal_entry(artifact, self.entry, self.target))
            .transpose()?
            .flatten();
        let selected_entry = if let Some(entry) = installed_entry {
            entry
        } else {
            match module.entry {
                terminal_psi::TargetMachineCallable::Machine(machine) => machine,
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    return Err("native family entry requires retained deployment selection");
                }
            }
        };
        if selected_entry != self.entry {
            return Err("native object entry substituted the exact installed Terminal member");
        }
        Ok(())
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

    pub fn functions(&self) -> &[ObjectFunction] {
        &self.functions
    }

    /// The function carrying the given machine, when one exists. Functions
    /// are keyed by machine id throughout the artifact; consumers name a
    /// missing entry in their own error.
    pub fn function_on_machine(&self, machine: MachineId) -> Option<&ObjectFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn private_functions(&self) -> &[ObjectCompilerPrivateFunction] {
        &self.private_functions
    }

    pub(crate) fn entry_function(&self) -> &ObjectFunction {
        self.function_on_machine(self.entry)
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
        self.function_on_machine(machine)
            .map(|function| function.text_offset)
    }
}

/// The object function keyed `machine` in a function roster.
pub(crate) fn find_object_function(
    functions: &[ObjectFunction],
    machine: MachineId,
) -> Option<&ObjectFunction> {
    functions
        .iter()
        .find(|function| function.machine == machine)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>,
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

impl ObjectFunction {
    /// Every internal Unit call's arguments, in call order.
    pub fn call_arguments(
        &self,
    ) -> impl Iterator<Item = &machine_code::InternalUnitCallArgumentRecord> + '_ {
        machine_code::unit_call_arguments(&self.internal_unit_calls)
    }
}

impl ObjectFunction {
    /// The scalar structural parameter arriving at `place`, when declared.
    pub fn scalar_structural_parameter(
        &self,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<&machine_code::UnitParameterRecord> {
        self.scalar_structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
    }

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
    /// Image entry geometry and ABI only. Full call/frame records remain local
    /// in `body`, never mixed with rebased image coordinates in a fake function.
    pub function: ObjectPrivateFunctionEntry,
    /// The complete artifact-local function, import, data and evidence namespace.
    pub body: std::sync::Arc<ObjectArtifact>,
    /// Physical native entry; absent only for identity transport to the body.
    pub entry_bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
    pub text_offset: usize,
    pub data_offset: usize,
    pub symbol_bindings: arena::Arena<PrivateObjectSymbolBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectPrivateFunctionEntry {
    pub machine: MachineId,
    pub scalar_abi: Option<target_operations::ScalarFunctionAbi>,
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
}

impl ObjectPrivateFunctionEntry {
    pub(crate) fn from_function(
        function: &ObjectFunction,
        symbol: ObjectSymbolHandle,
        text_offset: usize,
    ) -> Self {
        Self {
            machine: function.machine,
            scalar_abi: function.scalar_abi.clone(),
            symbol,
            text_offset,
            byte_count: function.byte_count,
        }
    }

    pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
        &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivateObjectSymbolBinding {
    pub local: ObjectSymbolHandle,
    pub linked: ObjectSymbolHandle,
}

/// One independently emitted callback body selected by its caller-scoped identity.
pub struct PrivateFunctionObject {
    pub identity: function_identity::MachineFunctionIdentity,
    pub body: std::sync::Arc<ObjectArtifact>,
    pub entry_bridge: Option<machine_emission::native_entry_bridge::NativeEntryBridge>,
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
/// construction has independently replayed its instruction and entry mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectForeignCall {
    pub machine: MachineId,
    pub owner: CallSiteOwner,
    /// Ordinal of the exact normalized foreign operation owning the complete
    /// semantic-code attribution interval.
    pub operation_ordinal: usize,
    pub binding: target_operations::NormalizedForeignCallBinding,
    pub provider_execution: machine_code::ProviderExecutionRecord,
    /// Exact physical caller frontier independently reconstructed from emitted
    /// stack instructions before the opaque foreign leaf begins.
    pub caller_live_bytes: u32,
    /// Exact scalar argument materializations retained from machine emission.
    /// Every code offset is rebased to absolute object `.text`.
    pub scalar_arguments: Vec<machine_code::ForeignCallScalarArgumentRecord>,
    /// Exact compiler-private callback addresses materialized before this
    /// call, in the selected roster's order. Every byte offset has been
    /// rebased to absolute object `.text`.
    pub callback_addresses: Vec<machine_code::CallbackAddressMaterialization>,
    /// Result custody retained from machine emission. Its code offset is
    /// rebased to absolute object `.text`, like `text_offset` below.
    pub scalar_result: Option<machine_code::ForeignCallScalarResultRecord>,
    /// Absolute object-text intervals proving complete MXCSR preservation for
    /// this returning x86 foreign call.
    pub x86_floating_control: Option<machine_code::X86ForeignCallFloatingControlRecord>,
    /// Absolute object-text intervals proving complete FPCR preservation for
    /// this returning AArch64 foreign call.
    pub aarch64_floating_control: Option<machine_code::Aarch64ForeignCallFloatingControlRecord>,
    /// Absolute object-text offset of the import relocation field, or of the
    /// complete supervisor-entry instruction for a kernel binding.
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
