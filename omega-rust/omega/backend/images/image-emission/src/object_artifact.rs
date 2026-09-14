//! Object-artifact construction: the sealed carrier types, the public
//! builder entry points, and the single validating construction pass that
//! replays every retained machine-code record against final bytes before
//! sealing text, data, symbols, and relocations.

mod call_sites;
mod private_functions;
pub(crate) mod replay;
pub(crate) mod stack_demand;

use call_sites::{
    validate_foreign_call_floating_control, validate_foreign_call_site,
    validate_foreign_scalar_arguments, validate_internal_call_site,
};
use private_functions::validate_private_functions;

use crate::HostedReceiverBinding;

use replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
use replay::boundary::completion_receipts::{CompletionCustodyError, validate_completion_custody};
use replay::boundary::result_placement::boundary_result_is_exact;
use replay::boundary::runtime_scalar_custody::hosted_write_byte_custody_is_exact;
use replay::dynamic::conformance::{validate_dynamic_calls, validate_stored_dynamic_calls};
use replay::dynamic::forwarded_descriptor::{
    ValidatedForwardedDynamicApplication, validate_forwarded_dynamic_descriptors,
};
use replay::dynamic::forwarded_parameter::validate_forwarded_dynamic_parameter_calls;
use replay::scalar::cleanup_preservation::validate_scalar_cleanup_preservation;
use replay::scalar::conditional_call_paths::{
    conditional_call_path, conditional_paths_are_exclusive,
};
use replay::scalar::control_cleanup::{
    cleanup_for_owner, validate_scalar_control_cleanup_evidence,
};
use replay::scalar::stack::validate_scalar_stack;
use replay::scalar::structural_scalar_field_store::validate_scalar_structural_scalar_field_stores;
use replay::structural::affine_projected_calls::{
    exact_fully_consumed_affine_parameter, exact_partially_consumed_affine_parameter,
};
use replay::structural::return_record::validate_structural_return_record;
use replay::unit::affine_cleanup::validate_unit_affine_cleanup;
use replay::unit::call_custody::{
    structural_result_matches_return, validate_internal_unit_call_custody,
    validate_mixed_structural_scalar_abi, validate_unit_affine_scalar_records,
};
use replay::unit::dynamic_descriptor_join::validate_unit_dynamic_descriptor_join;
use replay::unit::installed_provider_scalar_call::validate_installed_provider_unit_scalar_calls;
use replay::unit::scalar_call_custody::validate_internal_unit_scalar_calls;
use replay::unit::stack::{
    validate_complete_unit_stack_evidence, validate_foreign_unit_call_stack,
    validate_unit_call_stack, validate_unit_function_stack,
};
use replay::unit::structural_scalar_field_store::validate_unit_structural_scalar_field_stores;
use replay::unit::write_only_primitive_store::validate_unit_write_only_primitive_stores;

use machine_code::{
    BoundarySettlementRecord, CompilerPrivateMachineCodeFunction, MachineCodePlan,
    MachineCodePlanWithPrivateFunctions, PortEffectRecord, ScalarControlAffineCleanupRecord,
    SemanticCodeAttribution, SemanticCodeSite, StructuralReturnRecord,
};
use object_file::{
    FunctionSymbolPlan, NormalizedImportPlan, ObjectPlan, ObjectSymbolHandle, RelocationKind,
    RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind, SectionPlan, SymbolKind,
    SymbolPlan, SymbolSection, entry_symbol_name, normalized_foreign_import_symbol_name,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget};
use target_operations::{BoundaryRealization, CallSiteOwner, TerminalPsiProvenance};
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

/// Construct a self-contained object plan and exact text carrier.
///
/// Function order is semantic-artifact order and must already be canonical by
/// `MachineId`; this boundary rejects alternate ordering rather than silently
/// normalizing it. Each function gets exactly one symbol and one retained Psi
/// provenance row.
pub fn build_object_artifact(plan: &MachineCodePlan) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(plan, &[], None, None)
}

/// Construct an object that owns semantic program functions and a disjoint,
/// placement-identified compiler-private callback-function roster.
pub fn build_object_artifact_with_private_functions(
    plan: &MachineCodePlanWithPrivateFunctions,
) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(&plan.plan, &plan.private_functions, None, None)
}

/// Construct the bounded source-free object seam for feature-requiring scalar
/// x86 FMA. The profile is explicit because `NativeTarget` deliberately
/// collapses Windows and UEFI x86-64 physical layouts.
pub fn build_feature_required_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    profile: target::TargetProfile,
) -> Result<ObjectArtifact, ObjectError> {
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(plan, &[], Some(profile), None)
}

/// Consume exact deployment-feature and differential authority while building
/// an object whose generic F32/F64 FMA slots may enter executable emission.
/// Ordinary and feature-required-only builders retain their fail-closed
/// baseline behavior.
pub fn build_admitted_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    provider: target::AdmittedX86ScalarFmaProvider,
) -> Result<ObjectArtifact, ObjectError> {
    if !provider.has_canonical_identity() {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }
    if provider.profile().native_target() != plan.target
        || plan
            .functions
            .iter()
            .flat_map(|function| &function.x86_scalar_fma)
            .any(|fragment| {
                let slot = match fragment.format {
                    machine_code::X86ScalarFmaFormat::Binary32 => {
                        target::X86ScalarFmaSlot::Binary32
                    }
                    machine_code::X86ScalarFmaFormat::Binary64 => {
                        target::X86ScalarFmaSlot::Binary64
                    }
                };
                !provider.admits(fragment.requirement, slot)
            })
    {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(
        plan,
        &[],
        Some(provider.profile()),
        Some(provider),
    )
}

pub(crate) fn same_dynamic_table_application(
    left: &terminal_psi::ClosedConformanceApplication,
    right: &terminal_psi::ClosedConformanceApplication,
) -> bool {
    left.commitment == right.commitment
        && left.declaration_identity == right.declaration_identity
        && left.telescope == right.telescope
        && left.subject_identity == right.subject_identity
        && left.trait_identity == right.trait_identity
        && left.trait_lifetime_arguments == right.trait_lifetime_arguments
        && left.trait_arguments == right.trait_arguments
        && left.realization_callables == right.realization_callables
        && left.rows == right.rows
        && left.report_fingerprint == right.report_fingerprint
}

fn build_object_artifact_with_x86_feature_profile(
    plan: &MachineCodePlan,
    private_functions: &[CompilerPrivateMachineCodeFunction],
    x86_feature_profile: Option<target::TargetProfile>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<ObjectArtifact, ObjectError> {
    if plan.functions.is_empty() {
        return Err(ObjectError::EmptyPlan);
    }
    let forwarded_dynamic_applications =
        validate_forwarded_dynamic_descriptors(plan.target, &plan.functions)?;
    validate_forwarded_dynamic_parameter_calls(plan.target, &plan.functions)?;
    let validated_private_functions = validate_private_functions(plan.target, private_functions)?;
    let mut previous = None;
    let mut saw_entry = false;
    let mut text_size = 0usize;
    let mut validated_unit_stacks = std::collections::BTreeMap::new();
    let mut validated_scalar_stacks = std::collections::BTreeMap::new();
    let mut validated_foreign_call_stacks = std::collections::BTreeMap::new();
    let attachments = plan
        .functions
        .iter()
        .map(|function| (function.machine, function.attachment))
        .collect::<std::collections::BTreeMap<_, _>>();
    let machine_functions = plan
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    for function in &plan.functions {
        // Legacy machine plans have only assigned stack homes. Incoming ABI
        // pointer custody is admitted through the independently replayed
        // shared-fragment object boundary, including call-free functions.
        if function
            .unit_parameter_homes
            .iter()
            .chain(&function.scalar_structural_parameter_homes)
            .any(|home| {
                matches!(
                    home.location,
                    machine_code::StructuralSourceLocation::IncomingIndirectPointer { .. }
                        | machine_code::StructuralSourceLocation::IncomingIndirectStackPointer { .. }
                        | machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                )
            })
            || function.internal_unit_calls.iter().any(|call| {
                call.arguments.iter().any(|argument| {
                    matches!(
                        argument.source_location,
                        machine_code::StructuralSourceLocation::IncomingIndirectStackPointer { .. }
                            | machine_code::StructuralSourceLocation::IncomingBorrowedPointer { .. }
                    )
                })
            })
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        validate_mixed_structural_scalar_abi(plan.target, function)?;
        validate_unit_dynamic_descriptor_join(plan.target, function)?;
        if let Some(previous) = previous
            && previous >= function.machine
        {
            return Err(ObjectError::NonCanonicalFunctionOrder {
                previous,
                current: function.machine,
            });
        }
        if function.bytes.is_empty() {
            return Err(ObjectError::EmptyFunction(function.machine));
        }
        crate::object_artifact::replay::x86_fma::validate_x86_scalar_fma_function(
            plan.target,
            x86_feature_profile,
            x86_scalar_fma_provider,
            function,
        )?;
        if function
            .internal_calls
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
        {
            return Err(ObjectError::NonCanonicalInternalCallOrder(function.machine));
        }
        if (function.unit_affine_cleanup.is_some()
            && (function.scalar_affine_cleanup.is_some()
                || !function.scalar_control_affine_cleanups.is_empty()))
            || (function.scalar_affine_cleanup.is_some()
                && !function.scalar_control_affine_cleanups.is_empty())
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if function
            .foreign_calls
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
        {
            return Err(ObjectError::NonCanonicalForeignCallOrder(function.machine));
        }
        let mut foreign_owners = std::collections::BTreeSet::new();
        let mut foreign_floating_control_slot = None;
        let mut prior_foreign_floating_control_end = None;
        for call in &function.foreign_calls {
            let owner_in_provenance = match call.owner {
                CallSiteOwner::Operation(operation) => {
                    function.provenance.operations.contains(&operation)
                }
                CallSiteOwner::CleanupAction { edge, .. } => {
                    function.provenance.edges.contains(&edge)
                }
            };
            if !owner_in_provenance {
                return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            if !foreign_owners.insert(call.owner) {
                return Err(ObjectError::DuplicateForeignCallOwner {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            if call.locator.target().native_target() != plan.target {
                return Err(ObjectError::ForeignCallTargetMismatch {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            if call.same_stack_contribution.provider_plan_report_identity()
                != call.provider_execution.provider_plan_report_identity
            {
                return Err(ObjectError::ForeignStackProviderPlanMismatch {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            if function
                .internal_calls
                .iter()
                .any(|internal| internal.offset == call.offset)
            {
                return Err(ObjectError::ForeignCallOverlapsInternalCall {
                    caller: function.machine,
                    offset: call.offset,
                });
            }
            validate_foreign_call_site(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                call,
            )?;
            validate_foreign_call_floating_control(plan.target, function, call)?;
            let control = match plan.target.architecture {
                Architecture::X86_64 => call.x86_floating_control.map(|control| {
                    (
                        control.saved_slot_byte_offset,
                        control.save_offset,
                        control.restore_offset,
                        control.restore_byte_count,
                    )
                }),
                Architecture::Aarch64 => call.aarch64_floating_control.map(|control| {
                    (
                        control.saved_slot_byte_offset,
                        control.save_offset,
                        control.restore_offset,
                        control.restore_byte_count,
                    )
                }),
            };
            if let Some((slot, save_offset, restore_offset, restore_byte_count)) = control {
                if foreign_floating_control_slot
                    .replace(slot)
                    .is_some_and(|prior| prior != slot)
                    || prior_foreign_floating_control_end.is_some_and(|end| end > save_offset)
                {
                    return Err(ObjectError::InvalidForeignCallFloatingControl {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                prior_foreign_floating_control_end =
                    Some(restore_offset.checked_add(restore_byte_count).ok_or(
                        ObjectError::InvalidForeignCallFloatingControl {
                            caller: function.machine,
                            owner: call.owner,
                        },
                    )?);
            }
            validate_foreign_scalar_arguments(plan.target, function, call)?;
        }
        let mut validated_function_stack = function
            .unit_stack
            .map(|stack| {
                validate_unit_function_stack(
                    plan.target.architecture,
                    function.machine,
                    &function.bytes,
                    stack,
                    0,
                )
            })
            .transpose()?;
        if function.boundary_settlements.iter().any(|settlement| {
            hosted_write_byte_custody_is_exact(
                plan.target,
                settlement,
                &function.boundary_settlements,
                &function.unit_integer_constants,
                &function.unit_scalar_homes,
                |home, consumer_ordinal, consumer_offset| {
                    crate::object_artifact::replay::unit::scalar_call_custody::exact_preceding_internal_unit_scalar_home_producer_count(
                        &function.internal_unit_scalar_calls,
                        home,
                        consumer_ordinal,
                        consumer_offset,
                    )
                },
                Some(&function.bytes),
            )
        }) {
            let stack = validated_function_stack
                .as_mut()
                .ok_or(ObjectError::UnaccountedTerminalStack(function.machine))?;
            stack.local_peak_bytes = stack
                .frame_bytes
                .checked_add(16)
                .ok_or(ObjectError::UnaccountedTerminalStack(function.machine))?;
        }
        if function.unit_stack.is_some() && function.scalar_stack.is_some() {
            return Err(ObjectError::ConflictingTerminalStackEvidence(
                function.machine,
            ));
        }
        if let Some(returned) = function.structural_call_scalar_return
            && !(function.unit_stack.is_some()
                && function.scalar_stack.is_none()
                && function.provenance.operations.as_slice() == [returned.psi_operation]
                && function.provenance.edges.as_slice() == [returned.psi_edge]
                && matches!(
                    (
                        function.internal_unit_calls.as_slice(),
                        function.semantic_code_attribution.as_slice(),
                        function.unit_affine_cleanup.as_ref(),
                    ),
                    ([call], [call_attribution, return_attribution], Some(cleanup))
                        if call.owner == CallSiteOwner::Operation(returned.psi_operation)
                            && call.target == returned.callee
                            && call.operation_ordinal == 0
                            && call.result == Some(returned.scalar_type)
                            && call.semantic_result.as_ref().is_some_and(|result| {
                                result.value == returned.source_value
                                    && result.scalar_type == returned.scalar_type
                            })
                            && call_attribution.site
                                == SemanticCodeSite::Operation(returned.psi_operation)
                            && call_attribution.operation_ordinal == call.operation_ordinal
                            && call_attribution.code_offset == call.code_offset
                            && call_attribution.byte_count == call.byte_count
                            && return_attribution.site == SemanticCodeSite::Edge(returned.psi_edge)
                            && return_attribution.operation_ordinal == 1
                            && return_attribution.code_offset == cleanup.code_offset
                            && return_attribution.byte_count == cleanup.byte_count
                            && cleanup.psi_edge == returned.psi_edge
                            && cleanup.locals.is_empty()
                            && cleanup.actions.is_empty()
                ))
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        if let Some(returned) = &function.structural_return {
            validate_structural_return_record(
                plan.target,
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                returned,
            )?;
            if function.unit_stack.is_some()
                || function.scalar_stack.is_some()
                || !function.internal_calls.is_empty()
                || !function.port_effects.is_empty()
                || !function.boundary_settlements.is_empty()
            {
                return Err(ObjectError::StructuralReturnEvidenceConflict(
                    function.machine,
                ));
            }
        }
        if let Some(stack) = &function.scalar_stack {
            validate_scalar_cleanup_preservation(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                stack,
                function.scalar_affine_cleanup.as_ref(),
            )?;
            validate_scalar_control_cleanup_evidence(
                plan.target.architecture,
                function.machine,
                &function.provenance,
                &function.bytes,
                stack,
                &function.scalar_control_affine_cleanups,
            )?;
            validated_scalar_stacks.insert(
                function.machine,
                validate_scalar_stack(
                    plan.target.architecture,
                    function.machine,
                    &function.bytes,
                    &function.internal_calls,
                    &function.dynamic_parameter_calls,
                    &function.provenance,
                    &function.semantic_code_attribution,
                    stack,
                    function.scalar_affine_cleanup.as_ref(),
                    &function.scalar_control_affine_cleanups,
                    &function.scalar_structural_parameter_homes,
                )?,
            );
        }
        let mut validated_call_stacks = Vec::new();
        let mut call_owner_paths =
            std::collections::BTreeMap::<CallSiteOwner, Vec<Option<Vec<(usize, bool)>>>>::new();
        for call in &function.internal_calls {
            let owner_in_provenance = match call.owner {
                CallSiteOwner::Operation(operation) => {
                    function.provenance.operations.contains(&operation)
                }
                CallSiteOwner::CleanupAction { edge, .. } => {
                    function.provenance.edges.contains(&edge)
                }
            };
            if !owner_in_provenance {
                return Err(ObjectError::InternalCallOperationNotInProvenance {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            let path = conditional_call_path(
                plan.target.architecture,
                &function.bytes,
                function.scalar_stack.as_ref(),
                call,
            );
            let prior_paths = call_owner_paths.entry(call.owner).or_default();
            if !prior_paths.is_empty()
                && (!matches!(call.owner, CallSiteOwner::Operation(_))
                    || path.as_ref().is_none_or(|path| {
                        prior_paths.iter().any(|prior| {
                            prior
                                .as_ref()
                                .is_none_or(|prior| !conditional_paths_are_exclusive(prior, path))
                        })
                    }))
            {
                return Err(ObjectError::DuplicateInternalCallOperation {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            prior_paths.push(path);
            match (function.unit_stack, call.unit_stack) {
                (Some(_), Some(call_stack)) => {
                    let validated = validate_unit_call_stack(
                        plan.target.architecture,
                        function.machine,
                        &function.bytes,
                        *call,
                        function.unit_stack.expect("Unit stack evidence exists"),
                        validated_function_stack.expect("validated Unit stack exists"),
                        call_stack,
                    )?;
                    let function_stack = validated_function_stack
                        .as_mut()
                        .expect("validated Unit stack exists");
                    function_stack.local_peak_bytes = function_stack
                        .local_peak_bytes
                        .max(validated.caller_live_bytes);
                    validated_call_stacks.push(validated);
                }
                (Some(_), None) => {
                    return Err(ObjectError::MissingUnitCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, Some(_)) => {
                    return Err(ObjectError::UnexpectedUnitCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, None) => {}
            }
            match (function.scalar_stack.as_ref(), call.scalar_stack) {
                (Some(_), Some(_)) => {}
                (Some(_), None) => {
                    return Err(ObjectError::MissingScalarCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, Some(_)) => {
                    return Err(ObjectError::UnexpectedScalarCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, None) => {}
            }
        }
        for call in &function.foreign_calls {
            let Some(stack) = function.unit_stack else {
                return Err(ObjectError::UnexpectedUnitCallStackEvidence {
                    caller: function.machine,
                    owner: call.owner,
                });
            };
            let caller_live_bytes = validate_foreign_unit_call_stack(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                call,
                stack,
                validated_function_stack.expect("validated Unit stack exists"),
            )?;
            let function_stack = validated_function_stack
                .as_mut()
                .expect("validated Unit stack exists");
            let admitted_alignment = call.same_stack_contribution.alignment();
            if admitted_alignment > u64::from(function_stack.stack_alignment) {
                return Err(ObjectError::UnsupportedForeignStackAlignment {
                    caller: function.machine,
                    owner: call.owner,
                    admitted_alignment,
                    physical_alignment: function_stack.stack_alignment,
                });
            }
            function_stack.local_peak_bytes =
                function_stack.local_peak_bytes.max(caller_live_bytes);
            validated_foreign_call_stacks.insert((function.machine, call.owner), caller_live_bytes);
        }
        let is_unit_custody_relocation = |call: &&machine_code::InternalCallRelocation| {
            call.unit_stack.is_some()
                || ((function.scalar_affine_cleanup.is_some()
                    || cleanup_for_owner(&function.scalar_control_affine_cleanups, call.owner)
                        .is_some())
                    && matches!(call.owner, CallSiteOwner::CleanupAction { .. })
                    && call.scalar_stack.is_some())
        };
        let unit_custody_count = function
            .internal_unit_calls
            .len()
            .checked_add(function.internal_unit_scalar_calls.len())
            .and_then(|count| count.checked_add(function.forwarded_dynamic_descriptor_calls.len()))
            .and_then(|count| {
                count.checked_add(
                    function
                        .forwarded_dynamic_parameter_calls
                        .iter()
                        .filter(|call| {
                            matches!(
                                call.call_stack,
                                machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(_)
                            )
                        })
                        .count(),
                )
            })
            .and_then(|count| {
                count.checked_add(function.installed_provider_unit_scalar_calls.len())
            })
            .ok_or(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ))?;
        if unit_custody_count
            != function
                .internal_calls
                .iter()
                .filter(is_unit_custody_relocation)
                .count()
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        let relocation_identities = function
            .internal_calls
            .iter()
            .filter(is_unit_custody_relocation)
            .map(|call| (call.owner, call.target))
            .collect::<std::collections::BTreeSet<_>>();
        let custody_identities = function
            .internal_unit_calls
            .iter()
            .map(|call| (call.owner, call.target))
            .chain(
                function
                    .internal_unit_scalar_calls
                    .iter()
                    .map(|call| (call.owner, call.target)),
            )
            .chain(
                function
                    .forwarded_dynamic_descriptor_calls
                    .iter()
                    .map(|call| (CallSiteOwner::Operation(call.psi_operation), call.callee)),
            )
            .chain(
                function
                    .forwarded_dynamic_parameter_calls
                    .iter()
                    .filter_map(|call| {
                        matches!(
                            call.call_stack,
                            machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(_)
                        )
                        .then_some((CallSiteOwner::Operation(call.psi_operation), call.callee))
                    }),
            )
            .chain(
                function
                    .installed_provider_unit_scalar_calls
                    .iter()
                    .map(|call| (call.owner, call.provider.candidate)),
            )
            .collect::<std::collections::BTreeSet<_>>();
        if custody_identities.len() != unit_custody_count
            || custody_identities != relocation_identities
        {
            return Err(ObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        let scalar_cleanup_custody = function.scalar_affine_cleanup.is_some()
            || !function.scalar_control_affine_cleanups.is_empty();
        let scalar_boundary_custody = function.boundary_settlements.iter().any(|settlement| {
            matches!(
                settlement.realization,
                BoundaryRealization::DirectPortReadU8(_)
                    | BoundaryRealization::HostedExitProcessI32(_)
            )
        });
        let scalar_custody = scalar_cleanup_custody
            || scalar_boundary_custody
            || function.mixed_structural_scalar_abi.is_some()
            || !function.scalar_structural_scalar_field_stores.is_empty();
        let parameter_homes = if scalar_custody {
            function.scalar_structural_parameter_homes.as_slice()
        } else {
            function.unit_parameter_homes.as_slice()
        };
        let default_affine_cleanup = if let Some(cleanup) = function.scalar_affine_cleanup.as_ref()
        {
            Some(cleanup)
        } else {
            function.unit_affine_cleanup.as_ref()
        };
        let continuation_discards =
            crate::object_artifact::replay::unit::continuations::validate_function(function)?;
        if !function.unit_continuations.is_empty()
            && crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::validate_shape(
                plan.target,
                parameter_homes,
                function.parameter_abi.as_ref(),
                true,
                validated_function_stack
                    .as_ref()
                    .map_or(0, |stack| stack.frame_bytes),
            )
            .is_none_or(|end| {
                validated_function_stack.as_ref().is_none_or(|stack| {
                    end > stack.frame_bytes
                        || (function
                            .internal_unit_calls
                            .iter()
                            .all(|call| call.structural_result.is_none())
                            && !crate::object_artifact::replay::unit::call_custody::result_home::exact_frame(
                                plan.target,
                                end,
                                stack.frame_bytes,
                                function
                                    .unit_stack
                                    .and_then(|stack| stack.aarch64_return_link)
                                    .map(|link| link.frame_byte_offset),
                            ))
                })
            })
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if !function.unit_continuations.is_empty()
            && !crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::exact_prologue(
                plan.target,
                &function.bytes,
                function.parameter_abi.as_ref(),
                parameter_homes,
                validated_function_stack
                    .as_ref()
                    .map_or(0, |stack| stack.frame_bytes),
                function
                    .semantic_code_attribution
                    .first()
                    .map_or(usize::MAX, |row| row.code_offset),
            )
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        let fully_consumed_affine_parameter = exact_fully_consumed_affine_parameter(
            parameter_homes,
            &function.internal_unit_calls,
            default_affine_cleanup,
        );
        let partially_consumed_affine_parameter = exact_partially_consumed_affine_parameter(
            parameter_homes,
            &function.internal_unit_calls,
            default_affine_cleanup,
        );
        for custody in &function.internal_unit_calls {
            let target_returns_scalar = machine_functions
                .get(&custody.target)
                .copied()
                .is_some_and(|target| {
                    target.scalar_stack.is_some() || target.structural_call_scalar_return.is_some()
                });
            let target_structural_return = machine_functions
                .get(&custody.target)
                .and_then(|target| target.structural_return.as_ref());
            let structural_result_valid =
                match (&custody.structural_result, target_structural_return) {
                    (None, None) => true,
                    (Some(result), Some(target)) => {
                        custody.result.is_none() && structural_result_matches_return(result, target)
                    }
                    _ => false,
                };
            if custody.result.is_some() != target_returns_scalar
                || custody
                    .semantic_result
                    .as_ref()
                    .map(|result| result.scalar_type)
                    != custody.result
                || !structural_result_valid
                || (custody.structural_result.is_some() && target_returns_scalar)
                || machine_functions
                    .get(&custody.target)
                    .is_some_and(|target| {
                        target
                            .structural_call_scalar_return
                            .is_some_and(|returned| custody.result != Some(returned.scalar_type))
                    })
            {
                return Err(ObjectError::InvalidInternalUnitCallEvidence(
                    function.machine,
                ));
            }
            let unit_call_stack = validated_call_stacks
                .iter()
                .find(|call| call.owner == custody.owner && call.target == custody.target);
            let scalar_call_stack =
                validated_scalar_stacks
                    .get(&function.machine)
                    .and_then(|(_, calls)| {
                        calls.iter().find(|call| {
                            call.owner == custody.owner && call.target == custody.target
                        })
                    });
            if unit_call_stack.is_none() == scalar_call_stack.is_none() {
                return Err(ObjectError::InvalidInternalUnitCallEvidence(
                    function.machine,
                ));
            }
            let affine_cleanup =
                cleanup_for_owner(&function.scalar_control_affine_cleanups, custody.owner)
                    .or_else(|| {
                        crate::object_artifact::replay::unit::continuations::cleanup_for_call(
                            &function.unit_continuations,
                            custody.operation_ordinal,
                        )
                    })
                    .or(default_affine_cleanup);
            if !function.unit_continuations.is_empty()
                && custody
                    .arguments
                    .iter()
                    .any(|argument| !argument.path.is_empty())
                && machine_functions.get(&custody.target).is_none_or(|callee| {
                    callee.scalar_abi.is_some()
                        || default_affine_cleanup.is_none_or(|cleanup| {
                            !crate::object_artifact::replay::unit::continuations::exact_projected_callee(
                                custody,
                                &callee.unit_parameters,
                                callee.unit_affine_cleanup.as_ref(),
                                cleanup,
                                &callee.semantic_code_attribution,
                            )
                        })
                })
            {
                return Err(ObjectError::InvalidInternalUnitCallEvidence(
                    function.machine,
                ));
            }
            validate_internal_unit_call_custody(
                plan.target,
                function,
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.internal_calls,
                &function.internal_unit_calls,
                parameter_homes,
                validated_function_stack.as_ref(),
                unit_call_stack,
                scalar_call_stack,
                machine_functions
                    .get(&custody.target)
                    .and_then(|callee| callee.parameter_abi.as_ref()),
                machine_functions
                    .get(&custody.target)
                    .map_or(&[][..], |callee| callee.unit_parameters.as_slice()),
                machine_functions
                    .get(&custody.target)
                    .and_then(|callee| callee.mixed_structural_scalar_abi.as_ref()),
                target_structural_return,
                custody,
                affine_cleanup,
                fully_consumed_affine_parameter
                    || custody
                        .arguments
                        .iter()
                        .any(|argument| continuation_discards.contains(&argument.place)),
            )?;
        }
        validate_unit_affine_scalar_records(function)?;
        validate_internal_unit_scalar_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
            &validated_call_stacks,
        )?;
        validate_installed_provider_unit_scalar_calls(
            plan.target,
            function,
            &machine_functions,
            &validated_call_stacks,
        )?;
        let dynamic_peak = validate_dynamic_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
        )?;
        let stored_dynamic_peak = validate_stored_dynamic_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
        )?;
        if let Some(stack) = validated_function_stack.as_mut() {
            stack.local_peak_bytes = stack
                .local_peak_bytes
                .max(dynamic_peak)
                .max(stored_dynamic_peak);
        }
        validate_unit_write_only_primitive_stores(
            plan.target,
            function,
            validated_function_stack.as_ref(),
        )?;
        validate_unit_structural_scalar_field_stores(
            plan.target,
            function,
            validated_function_stack.as_ref(),
        )?;
        validate_scalar_structural_scalar_field_stores(plan.target, function)?;
        match (&function.unit_stack, &function.unit_affine_cleanup) {
            (Some(_), Some(cleanup)) => validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.unit_parameter_homes,
                &function.internal_unit_calls,
                &function.boundary_settlements,
                &attachments,
                &machine_functions,
                cleanup,
                false,
                fully_consumed_affine_parameter,
                partially_consumed_affine_parameter,
                &continuation_discards,
            )?,
            (None, None) => {}
            _ => {
                return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
        }
        if let Some(cleanup) = &function.scalar_affine_cleanup {
            if function.unit_stack.is_some() || function.scalar_stack.is_none() {
                return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
            validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.scalar_structural_parameter_homes,
                &function.internal_unit_calls,
                &function.boundary_settlements,
                &attachments,
                &machine_functions,
                cleanup,
                true,
                false,
                false,
                &[],
            )?;
        }
        if !function.scalar_control_affine_cleanups.is_empty() {
            if function.unit_stack.is_some()
                || function.scalar_stack.is_none()
                || function.scalar_affine_cleanup.is_some()
            {
                return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
            for record in &function.scalar_control_affine_cleanups {
                let cleanup_end = record
                    .cleanup
                    .code_offset
                    .checked_add(record.cleanup.byte_count)
                    .ok_or(ObjectError::InvalidUnitAffineCleanupEvidence(
                        function.machine,
                    ))?;
                validate_unit_affine_cleanup(
                    function.machine,
                    &function.provenance,
                    function.bytes.get(..cleanup_end).ok_or(
                        ObjectError::InvalidUnitAffineCleanupEvidence(function.machine),
                    )?,
                    &function.semantic_code_attribution,
                    &function.scalar_structural_parameter_homes,
                    &function.internal_unit_calls,
                    &function.boundary_settlements,
                    &attachments,
                    &machine_functions,
                    &record.cleanup,
                    true,
                    false,
                    false,
                    &[],
                )?;
            }
        }
        if function.unit_parameters.len() != function.unit_parameter_homes.len()
            || function
                .unit_parameters
                .iter()
                .zip(&function.unit_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.access != home.access
                        || parameter.shape != home.shape
                })
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if function.scalar_structural_parameters.len()
            != function.scalar_structural_parameter_homes.len()
            || function
                .scalar_structural_parameters
                .iter()
                .zip(&function.scalar_structural_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.access != home.access
                        || parameter.shape != home.shape
                })
            || (!scalar_custody
                && (!function.scalar_structural_parameters.is_empty()
                    || !function.scalar_structural_parameter_homes.is_empty()))
        {
            return Err(ObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if let Some(stack) = function.unit_stack {
            let inline_data = function
                .boundary_settlements
                .iter()
                .filter(|settlement| {
                    linux_write_line_custody_is_exact(
                        plan.target,
                        settlement,
                        Some(&function.bytes),
                    )
                })
                .flat_map(|settlement| &settlement.byte_sequence_arguments)
                .filter_map(|argument| {
                    argument
                        .data_offset
                        .checked_add(argument.data_byte_count)
                        .map(|end| argument.data_offset..end)
                })
                .collect::<Vec<_>>();
            validate_complete_unit_stack_evidence(
                plan.target,
                function.machine,
                &function.bytes,
                stack,
                &function.internal_calls,
                &function.foreign_calls,
                &function.dynamic_calls,
                &function.stored_dynamic_calls,
                &function.boundary_settlements,
                &function.unit_integer_constants,
                &function.unit_scalar_homes,
                &function.internal_unit_scalar_calls,
                &inline_data,
            )?;
        }
        if let Some(stack) = validated_function_stack {
            validated_unit_stacks.insert(function.machine, (stack, validated_call_stacks));
        }
        if function.semantic_code_attribution.windows(2).any(|pair| {
            (pair[0].operation_ordinal, pair[0].code_offset)
                >= (pair[1].operation_ordinal, pair[1].code_offset)
        }) {
            return Err(ObjectError::NonCanonicalSemanticCodeAttributionOrder(
                function.machine,
            ));
        }
        let mut attribution_sites = std::collections::BTreeSet::new();
        for attribution in &function.semantic_code_attribution {
            let end = attribution
                .code_offset
                .checked_add(attribution.byte_count)
                .ok_or(ObjectError::SemanticCodeAttributionOutsideFunction(
                    function.machine,
                ))?;
            let known = match attribution.site {
                SemanticCodeSite::Operation(operation) => {
                    function.provenance.operations.contains(&operation)
                }
                SemanticCodeSite::Edge(edge) => function.provenance.edges.contains(&edge),
            };
            if end > function.bytes.len() || !known || !attribution_sites.insert(attribution.site) {
                return Err(ObjectError::InvalidSemanticCodeAttribution(
                    function.machine,
                ));
            }
        }
        if function.port_effects.windows(2).any(|pair| {
            (pair[0].code_offset, pair[0].operation_ordinal)
                >= (pair[1].code_offset, pair[1].operation_ordinal)
        }) {
            return Err(ObjectError::NonCanonicalPortEffectOrder(function.machine));
        }
        let mut port_operations = std::collections::BTreeSet::new();
        for effect in &function.port_effects {
            let end = effect.code_offset.checked_add(effect.byte_count).ok_or(
                ObjectError::PortEffectOutsideFunction {
                    machine: function.machine,
                    operation: effect.psi_operation,
                },
            )?;
            if end > function.bytes.len() || effect.byte_count == 0 {
                return Err(ObjectError::PortEffectOutsideFunction {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if !function
                .provenance
                .operations
                .contains(&effect.psi_operation)
            {
                return Err(ObjectError::PortEffectOperationNotInProvenance {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if !port_operations.insert(effect.psi_operation) {
                return Err(ObjectError::DuplicatePortEffectOperation {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if plan.target.architecture != Architecture::X86_64
                || function.bytes[effect.code_offset..end]
                    != x86_encoding::encode_immediate_port_write(effect.port, effect.value)
            {
                return Err(ObjectError::PortEffectBytesMismatch {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
        }
        if function.boundary_settlements.windows(2).any(|pair| {
            (pair[0].code_offset, pair[0].operation_ordinal)
                >= (pair[1].code_offset, pair[1].operation_ordinal)
        }) {
            return Err(ObjectError::NonCanonicalBoundarySettlementOrder(
                function.machine,
            ));
        }
        let mut settlement_operations = std::collections::BTreeSet::new();
        for settlement in &function.boundary_settlements {
            if settlement.code_offset > function.bytes.len() {
                return Err(ObjectError::BoundarySettlementOutsideFunction {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if !function
                .provenance
                .operations
                .contains(&settlement.psi_operation)
            {
                return Err(ObjectError::BoundarySettlementOperationNotInProvenance {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if !settlement_operations.insert(settlement.psi_operation) {
                return Err(ObjectError::DuplicateBoundarySettlementOperation {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if let Err(error) = validate_completion_custody(settlement) {
                return Err(match error {
                    CompletionCustodyError::ArgumentPath => {
                        ObjectError::InvalidBoundarySettlementArgumentPath {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::ReceiptArgumentIndex => {
                        ObjectError::InvalidCompletionReceiptArgumentIndex {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::ReceiptCustody => {
                        ObjectError::InvalidCompletionReceiptCustody {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::ProviderCustody => {
                        ObjectError::InvalidCompletionProviderCustody {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                });
            }
            let valid_realization = match settlement.realization {
                BoundaryRealization::MetadataOnlyPort(realization) => {
                    settlement.scalar_arguments.is_empty()
                        && settlement.runtime_scalar_arguments.is_empty()
                        && settlement.byte_sequence_arguments.is_empty()
                        && settlement.byte_count == 0
                        && function
                            .port_effects
                            .iter()
                            .filter(|effect| {
                                effect.psi_operation == realization.effect_operation
                                    && effect.service == realization.service
                                    && effect.port == realization.port
                                    && effect.value == realization.value
                                    && effect.operation_ordinal.checked_add(1)
                                        == Some(settlement.operation_ordinal)
                                    && effect.code_offset.checked_add(effect.byte_count)
                                        == Some(settlement.code_offset)
                            })
                            .count()
                            == 1
                }
                BoundaryRealization::ClaimCompletionOnly(_) => {
                    settlement.scalar_arguments.is_empty()
                        && settlement.runtime_scalar_arguments.is_empty()
                        && settlement.byte_sequence_arguments.is_empty()
                        && settlement.native_result.is_unit()
                        && settlement.byte_count == 0
                }
                BoundaryRealization::DirectPortReadU8(realization) => {
                    let expected = x86_encoding::encode_immediate_port_read_u8(realization.port);
                    let exact_return_edge =
                        settlement.native_result.scalar().is_some_and(|result| {
                            let Some(return_ordinal) = settlement.operation_ordinal.checked_add(1)
                            else {
                                return false;
                            };
                            let Some(return_offset) =
                                settlement.code_offset.checked_add(settlement.byte_count)
                            else {
                                return false;
                            };
                            function
                                .semantic_code_attribution
                                .iter()
                                .filter(|attribution| {
                                    attribution.site == SemanticCodeSite::Edge(result.return_edge)
                                        && attribution.operation_ordinal == return_ordinal
                                        && attribution.code_offset == return_offset
                                        && attribution.byte_count == 1
                                })
                                .count()
                                == 1
                                && function.bytes.get(return_offset) == Some(&0xc3)
                        });
                    settlement.scalar_arguments.is_empty()
                        && settlement.runtime_scalar_arguments.is_empty()
                        && settlement.byte_sequence_arguments.is_empty()
                        && settlement.byte_count == expected.len()
                        && plan.target.architecture == Architecture::X86_64
                        && settlement
                            .code_offset
                            .checked_add(settlement.byte_count)
                            .and_then(|end| function.bytes.get(settlement.code_offset..end))
                            == Some(expected.as_slice())
                        && function.unit_stack.is_none()
                        && function.scalar_stack.is_some()
                        && exact_return_edge
                        && settlement.arguments.iter().all(|argument| {
                            argument.path.is_empty()
                                && function
                                    .scalar_structural_parameters
                                    .iter()
                                    .any(|parameter| parameter.place == argument.place)
                        })
                }
                BoundaryRealization::LinuxWriteLine(_) => {
                    linux_write_line_custody_is_exact(
                        plan.target,
                        settlement,
                        Some(&function.bytes),
                    ) && function.unit_stack.is_some()
                        && function.scalar_stack.is_none()
                }
                BoundaryRealization::HostedExitProcessI32(_) => {
                    if settlement.runtime_scalar_arguments.iter().any(|argument| matches!(argument.source, machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. })) {
                        crate::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::bytes_are_exact(plan.target, settlement, &function.bytes)
                            && function.scalar_stack.is_none()
                            && settlement.operation_ordinal.checked_add(1).is_some_and(|ordinal| function.semantic_code_attribution.iter().filter(|row| {
                                matches!(row.site, SemanticCodeSite::Edge(_))
                                    && row.operation_ordinal == ordinal
                                    && row.byte_count == 0
                                    && settlement.code_offset.checked_add(settlement.byte_count) == Some(row.code_offset)
                            }).count() == 1)
                    } else {
                    let [argument] = settlement.scalar_arguments.as_slice() else {
                        return Err(ObjectError::BoundaryRealizationMismatch {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        });
                    };
                    let i32_type = semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Signed,
                        32,
                    )
                    .expect("i32 is valid");
                    let value = match (argument.scalar_type, argument.immediate) {
                        (
                            semantic_vocabulary::ScalarType::Integer(actual),
                            semantic_vocabulary::IntegerValue::Signed(value),
                        ) if actual == i32_type => i32::try_from(value).ok(),
                        _ => None,
                    };
                    let expected_destination =
                        if !target_operations::HostedExitProcessI32Realization::supports_target(
                            plan.target,
                        ) {
                            None
                        } else {
                            match plan.target.architecture {
                                Architecture::X86_64 => {
                                    Some(calling_conventions::MachineRegister::X86Rdi)
                                }
                                Architecture::Aarch64 => {
                                    Some(calling_conventions::MachineRegister::Aarch64X(0))
                                }
                            }
                        };
                    let expected = value.and_then(|value| match plan.target.architecture {
                        Architecture::X86_64 => {
                            Some(isa_x86_64::encode_hosted_exit_process_i32(value))
                        }
                        Architecture::Aarch64 => {
                            isa_aarch64::encode_hosted_exit_process_i32(plan.target, value).ok()
                        }
                    });
                    let exact_nominal_tail = settlement
                        .operation_ordinal
                        .checked_add(1)
                        .is_some_and(|tail_ordinal| {
                            function
                                .semantic_code_attribution
                                .iter()
                                .filter(|attribution| {
                                    matches!(attribution.site, SemanticCodeSite::Edge(_))
                                        && attribution.operation_ordinal == tail_ordinal
                                        && attribution.code_offset
                                            == settlement
                                                .code_offset
                                                .saturating_add(settlement.byte_count)
                                        && (function.unit_stack.is_some()
                                            || (attribution.byte_count == 0
                                                && attribution.code_offset == function.bytes.len()))
                                })
                                .count()
                                == 1
                        });
                    expected.is_some_and(|expected| {
                        settlement.byte_count == expected.len()
                            && settlement.byte_count != 0
                            && settlement
                                .code_offset
                                .checked_add(settlement.byte_count)
                                .and_then(|end| function.bytes.get(settlement.code_offset..end))
                                == Some(expected.as_slice())
                    }) && expected_destination == Some(argument.destination)
                        && settlement.runtime_scalar_arguments.is_empty()
                        && settlement.arguments.is_empty()
                        && settlement.byte_sequence_arguments.is_empty()
                        && settlement.native_result.is_unit()
                        && function.scalar_stack.is_none()
                        && exact_nominal_tail
                    }
                }
                BoundaryRealization::HostedWriteByteI32(_) => {
                    hosted_write_byte_custody_is_exact(
                        plan.target,
                        settlement,
                        &function.boundary_settlements,
                        &function.unit_integer_constants,
                        &function.unit_scalar_homes,
                        |home, consumer_ordinal, consumer_offset| {
                            crate::object_artifact::replay::unit::scalar_call_custody::exact_preceding_internal_unit_scalar_home_producer_count(
                                &function.internal_unit_scalar_calls,
                                home,
                                consumer_ordinal,
                                consumer_offset,
                            )
                        },
                        Some(&function.bytes),
                    ) && function.unit_stack.is_some()
                        && function.scalar_stack.is_none()
                }
                BoundaryRealization::HostedReadByte(_) => {
                    let expected = settlement.native_result.structural().and_then(|result| {
                        let payload = result
                            .home_byte_offset
                            .checked_add(u32::from(result.layout.payload_byte_offset))?;
                        match plan.target {
                            target if target == NativeTarget::linux_x64() => isa_x86_64::encode_linux_read_byte_to_stack(
                                result.home_byte_offset,
                                payload,
                            )
                            .ok(),
                            target if target == NativeTarget::linux_arm64() => isa_aarch64::encode_linux_read_byte_to_stack(
                                result.home_byte_offset,
                                payload,
                            )
                            .ok(),
                            target if target == NativeTarget::macos_arm64() => isa_aarch64::encode_macos_read_byte_to_stack(
                                result.home_byte_offset,
                                payload,
                            )
                            .ok(),
                            _ => None,
                        }
                    });
                    settlement.scalar_arguments.is_empty()
                        && settlement.runtime_scalar_arguments.is_empty()
                        && settlement.arguments.is_empty()
                        && settlement.byte_sequence_arguments.is_empty()
                        && expected.as_ref().is_some_and(|expected| {
                            settlement.byte_count == expected.len()
                                && settlement
                                    .code_offset
                                    .checked_add(settlement.byte_count)
                                    .and_then(|end| function.bytes.get(settlement.code_offset..end))
                                    == Some(expected.as_slice())
                        })
                        && function.unit_stack.is_some()
                        && function.scalar_stack.is_none()
                }
            };
            if !valid_realization
                || !boundary_result_is_exact(
                    plan.target,
                    settlement.realization,
                    &settlement.native_result,
                )
            {
                return Err(ObjectError::BoundaryRealizationMismatch {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
        }
        previous = Some(function.machine);
        saw_entry |= function.machine == plan.entry;
        text_size = text_size
            .checked_add(function.bytes.len())
            .ok_or(ObjectError::TextSizeOverflow)?;
    }
    if !saw_entry {
        return Err(ObjectError::EntryFunctionMissing(plan.entry));
    }
    for private in &validated_private_functions {
        text_size = text_size
            .checked_add(private.machine.function.bytes.len())
            .ok_or(ObjectError::TextSizeOverflow)?;
    }
    for application in &forwarded_dynamic_applications {
        for adapter in &application.adapters {
            text_size = text_size
                .checked_add(adapter.bytes.len())
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
    }

    let dynamic_applications = collect_dynamic_applications(plan)?;

    let foreign_call_count = plan
        .functions
        .iter()
        .map(|function| function.foreign_calls.len())
        .sum::<usize>();
    let symbol_capacity = plan
        .functions
        .len()
        .saturating_add(private_functions.len())
        .saturating_add(foreign_call_count)
        .saturating_add(dynamic_applications.len())
        .saturating_add(forwarded_dynamic_applications.len())
        .saturating_add(
            forwarded_dynamic_applications
                .iter()
                .map(|application| application.adapters.len())
                .sum::<usize>(),
        );
    let mut object = if private_functions.is_empty() {
        ObjectPlan::with_capacity(plan.target, 1, symbol_capacity)
    } else {
        ObjectPlan::with_capacities(plan.target, 1, symbol_capacity, private_functions.len())
    };
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });
    let dynamic_data_size =
        dynamic_data_section_size(&dynamic_applications, &forwarded_dynamic_applications)?;
    if dynamic_data_size != 0 {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Data,
            size: dynamic_data_size,
            alignment: 8,
        });
    }

    let mut text_bytes = Vec::with_capacity(text_size);
    let mut functions = Vec::with_capacity(plan.functions.len());
    let mut object_private_functions = Vec::with_capacity(private_functions.len());
    let mut semantic_code_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
    let mut foreign_calls = Vec::with_capacity(foreign_call_count);
    let mut symbols_by_machine = std::collections::BTreeMap::new();
    for function in &plan.functions {
        let text_offset = text_bytes.len();
        text_bytes.extend_from_slice(&function.bytes);
        let is_entry = function.machine == plan.entry;
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: if is_entry {
                entry_symbol_name(plan.target)
            } else {
                format!("omega_terminal_machine_{}", function.machine.get())
            },
            section: SymbolSection::Section(SectionKind::Text),
            offset: text_offset,
            size: function.bytes.len(),
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        if is_entry {
            object.layout.entry_symbol = symbol;
        }
        symbols_by_machine.insert(function.machine, symbol);
        for attribution in &function.semantic_code_attribution {
            semantic_code_attribution.push(ObjectCodeAttribution {
                machine: function.machine,
                attribution: *attribution,
                text_offset: text_offset
                    .checked_add(attribution.code_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        for effect in &function.port_effects {
            port_effects.push(ObjectPortEffect {
                machine: function.machine,
                effect: effect.clone(),
                text_offset: text_offset
                    .checked_add(effect.code_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        for settlement in &function.boundary_settlements {
            boundary_settlements.push(ObjectBoundarySettlement {
                machine: function.machine,
                settlement: settlement.clone(),
                text_offset: text_offset
                    .checked_add(settlement.code_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        for call in &function.foreign_calls {
            let caller_live_bytes = validated_foreign_call_stacks
                .remove(&(function.machine, call.owner))
                .expect("foreign stack validation precedes object projection");
            let scalar_result = call
                .scalar_result
                .clone()
                .map(|mut result| {
                    result.code_offset = text_offset
                        .checked_add(result.code_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(result)
                })
                .transpose()?;
            let scalar_arguments = call
                .scalar_arguments
                .iter()
                .cloned()
                .map(|mut argument| {
                    argument.code_offset = text_offset
                        .checked_add(argument.code_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(argument)
                })
                .collect::<Result<Vec<_>, ObjectError>>()?;
            let x86_floating_control = call
                .x86_floating_control
                .map(|mut control| {
                    control.save_offset = text_offset
                        .checked_add(control.save_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    control.restore_offset = text_offset
                        .checked_add(control.restore_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(control)
                })
                .transpose()?;
            let aarch64_floating_control = call
                .aarch64_floating_control
                .map(|mut control| {
                    control.save_offset = text_offset
                        .checked_add(control.save_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    control.restore_offset = text_offset
                        .checked_add(control.restore_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    Ok(control)
                })
                .transpose()?;
            let callback_address = call
                .callback_address
                .clone()
                .map(|mut callback| {
                    callback.code_offset = text_offset
                        .checked_add(callback.code_offset)
                        .ok_or(ObjectError::TextSizeOverflow)?;
                    match &mut callback.encoding {
                        machine_code::CallbackAddressEncoding::X86_64Relative32 {
                            relocation_offset,
                        } => {
                            *relocation_offset = text_offset
                                .checked_add(*relocation_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?;
                        }
                        machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_offset,
                            page_offset_relocation_offset,
                        } => {
                            *page_relocation_offset = text_offset
                                .checked_add(*page_relocation_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?;
                            *page_offset_relocation_offset = text_offset
                                .checked_add(*page_offset_relocation_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?;
                        }
                    }
                    Ok(callback)
                })
                .transpose()?;
            foreign_calls.push(ObjectForeignCall {
                machine: function.machine,
                owner: call.owner,
                operation_ordinal: call.operation_ordinal,
                locator: call.locator.clone(),
                provider_execution: call.provider_execution,
                boundary_entry_plan: call.boundary_entry_plan.clone(),
                caller_live_bytes,
                same_stack_contribution: call.same_stack_contribution.clone(),
                scalar_arguments,
                callback_address,
                scalar_result,
                x86_floating_control,
                aarch64_floating_control,
                text_offset: text_offset
                    .checked_add(call.offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
            });
        }
        let (unit_stack, mut unit_call_stacks) = validated_unit_stacks
            .remove(&function.machine)
            .map_or((None, Vec::new()), |(stack, calls)| (Some(stack), calls));
        for call in &mut unit_call_stacks {
            call.text_offset = text_offset
                .checked_add(call.text_offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
        let (scalar_stack, mut scalar_call_stacks) = validated_scalar_stacks
            .remove(&function.machine)
            .map_or((None, Vec::new()), |(stack, calls)| (Some(stack), calls));
        for call in &mut scalar_call_stacks {
            call.text_offset = text_offset
                .checked_add(call.text_offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
        functions.push(ObjectFunction {
            machine: function.machine,
            attachment: function.attachment,
            scalar_abi: function.scalar_abi.clone(),
            mixed_structural_scalar_abi: function.mixed_structural_scalar_abi.clone(),
            structural_call_scalar_return: function.structural_call_scalar_return,
            parameter_abi: function.parameter_abi.clone(),
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
            x86_scalar_fma: function.x86_scalar_fma.clone(),
            x86_scalar_fma_occurrences: function.x86_scalar_fma_occurrences.clone(),
            x86_floating_control: function.x86_floating_control,
            unit_stack,
            scalar_stack,
            unit_call_stacks,
            scalar_call_stacks,
            internal_unit_calls: function.internal_unit_calls.clone(),
            internal_unit_scalar_calls: function.internal_unit_scalar_calls.clone(),
            installed_provider_unit_scalar_calls: function
                .installed_provider_unit_scalar_calls
                .clone(),
            dynamic_calls: function.dynamic_calls.clone(),
            stored_dynamic_calls: function.stored_dynamic_calls.clone(),
            dynamic_parameter_calls: function.dynamic_parameter_calls.clone(),
            forwarded_dynamic_parameter_calls: function.forwarded_dynamic_parameter_calls.clone(),
            forwarded_dynamic_descriptor_calls: function.forwarded_dynamic_descriptor_calls.clone(),
            unit_scalar_homes: function.unit_scalar_homes.clone(),
            unit_integer_constants: function.unit_integer_constants.clone(),
            unit_affine_scalar_records: function.unit_affine_scalar_records.clone(),
            unit_structural_scalar_field_stores: function
                .unit_structural_scalar_field_stores
                .clone(),
            unit_write_only_primitive_stores: function.unit_write_only_primitive_stores.clone(),
            scalar_structural_scalar_field_stores: function
                .scalar_structural_scalar_field_stores
                .clone(),
            unit_parameters: function.unit_parameters.clone(),
            unit_parameter_homes: function.unit_parameter_homes.clone(),
            unit_continuations: function.unit_continuations.clone(),
            unit_affine_cleanup: function.unit_affine_cleanup.clone(),
            scalar_affine_cleanup: function.scalar_affine_cleanup.clone(),
            scalar_control_affine_cleanups: function.scalar_control_affine_cleanups.clone(),
            scalar_structural_parameters: function.scalar_structural_parameters.clone(),
            scalar_structural_parameter_homes: function.scalar_structural_parameter_homes.clone(),
            structural_return: function.structural_return.clone(),
        });
    }

    let mut forwarded_dynamic_descriptor_adapters = Vec::new();
    let mut forwarded_adapter_symbols = std::collections::BTreeMap::new();
    for application in &forwarded_dynamic_applications {
        for adapter in &application.adapters {
            let target_symbol = symbols_by_machine
                .get(&adapter.identity.realization)
                .copied()
                .ok_or(ObjectError::UnknownDynamicConformanceTarget(
                    adapter.identity.realization,
                ))?;
            let text_offset = text_bytes.len();
            text_bytes.extend_from_slice(&adapter.bytes);
            let commitment = adapter
                .identity
                .application
                .as_bytes()
                .into_iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: format!(
                    "omega_forwarded_descriptor_adapter_{commitment}_{}_{}",
                    adapter.identity.row_index,
                    adapter.identity.realization.get()
                ),
                section: SymbolSection::Section(SectionKind::Text),
                offset: text_offset,
                size: adapter.bytes.len(),
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            if forwarded_adapter_symbols
                .insert(adapter.identity.clone(), symbol)
                .is_some()
            {
                return Err(ObjectError::DuplicateForwardedDynamicDescriptorAdapter);
            }
            forwarded_dynamic_descriptor_adapters.push(ObjectForwardedDynamicDescriptorAdapter {
                record: adapter.clone(),
                symbol,
                target_symbol,
                text_offset,
                byte_count: adapter.bytes.len(),
            });
        }
    }

    let mut data_bytes = Vec::with_capacity(dynamic_data_size);
    let mut dynamic_conformance_tables = Vec::with_capacity(dynamic_applications.len());
    for (table_index, application) in dynamic_applications.into_iter().enumerate() {
        if application.rows.is_empty() {
            return Err(ObjectError::InvalidDynamicConformanceTable);
        }
        let data_offset = data_bytes.len();
        let byte_count = application
            .rows
            .len()
            .checked_mul(8)
            .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?;
        data_bytes.resize(
            data_bytes
                .len()
                .checked_add(byte_count)
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
            0,
        );
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!(
                "omega_dynamic_conformance_table_{table_index}_{}",
                application.report_fingerprint
            ),
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let slots = application
            .rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let (target, target_symbol) = match &row.realization_callable_identity {
                    Some(callable_identity) => {
                        let matching = application
                            .realization_callables
                            .iter()
                            .filter(|callable| {
                                callable.source_callable_identity == *callable_identity
                            })
                            .collect::<Vec<_>>();
                        let [callable] = matching.as_slice() else {
                            return Err(ObjectError::InvalidDynamicConformanceTable);
                        };
                        let target_symbol =
                            symbols_by_machine.get(&callable.machine).copied().ok_or(
                                ObjectError::UnknownDynamicConformanceTarget(callable.machine),
                            )?;
                        (Some(callable.machine), Some(target_symbol))
                    }
                    None => (None, None),
                };
                Ok(ObjectDynamicConformanceSlot {
                    row_index: u32::try_from(row_index)
                        .map_err(|_| ObjectError::DynamicConformanceDataSizeOverflow)?,
                    realization_callable_identity: row.realization_callable_identity.clone(),
                    target,
                    target_symbol,
                    data_offset: data_offset
                        .checked_add(
                            row_index
                                .checked_mul(8)
                                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                        )
                        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectError>>()?;
        dynamic_conformance_tables.push(ObjectDynamicConformanceTable {
            application,
            symbol,
            data_offset,
            byte_count,
            slots,
        });
    }

    let mut forwarded_dynamic_descriptor_tables =
        Vec::with_capacity(forwarded_dynamic_applications.len());
    for application in &forwarded_dynamic_applications {
        if application.adapters.is_empty() {
            return Err(ObjectError::InvalidForwardedDynamicDescriptorTable);
        }
        let data_offset = data_bytes.len();
        let byte_count = application
            .adapters
            .len()
            .checked_mul(8)
            .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?;
        data_bytes.resize(
            data_bytes
                .len()
                .checked_add(byte_count)
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
            0,
        );
        let commitment = application
            .application
            .commitment
            .as_bytes()
            .into_iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: format!("omega_forwarded_descriptor_table_{commitment}"),
            section: SymbolSection::Section(SectionKind::Data),
            offset: data_offset,
            size: byte_count,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });
        let slots = application
            .adapters
            .iter()
            .enumerate()
            .map(|(row_index, adapter)| {
                let adapter_symbol = forwarded_adapter_symbols
                    .get(&adapter.identity)
                    .copied()
                    .ok_or(ObjectError::InvalidForwardedDynamicDescriptorTable)?;
                Ok(ObjectForwardedDynamicDescriptorSlot {
                    row_index: u32::try_from(row_index)
                        .map_err(|_| ObjectError::DynamicConformanceDataSizeOverflow)?,
                    adapter: adapter.identity.clone(),
                    adapter_symbol,
                    data_offset: data_offset
                        .checked_add(
                            row_index
                                .checked_mul(8)
                                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                        )
                        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)?,
                })
            })
            .collect::<Result<Vec<_>, ObjectError>>()?;
        forwarded_dynamic_descriptor_tables.push(ObjectForwardedDynamicDescriptorTable {
            application: application.application.clone(),
            symbol,
            data_offset,
            byte_count,
            slots,
        });
    }

    for private in validated_private_functions {
        let text_offset = text_bytes.len();
        text_bytes.extend_from_slice(&private.machine.function.bytes);
        if object
            .layout
            .symbols
            .iter()
            .any(|(_, symbol)| symbol.name == private.machine.private_symbol.as_ref())
        {
            return Err(ObjectError::PrivateFunctionSymbolCollision);
        }
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: private.machine.private_symbol.to_string(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: text_offset,
            size: private.machine.function.bytes.len(),
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.function_symbols.insert(FunctionSymbolPlan {
            identity: private.machine.identity,
            symbol,
        });
        let mut function = private.function;
        function.symbol = symbol;
        function.text_offset = text_offset;
        object_private_functions.push(ObjectCompilerPrivateFunction {
            identity: private.machine.identity,
            source_psi: private.machine.source_psi,
            function,
        });
    }

    let mut import_symbols = Vec::<(target::NormalizedForeignLocator, ObjectSymbolHandle)>::new();
    for function in &plan.functions {
        for call in &function.foreign_calls {
            if import_symbols
                .iter()
                .any(|(locator, _)| locator == &call.locator)
            {
                continue;
            }
            if import_symbols.iter().any(|(locator, _)| {
                locator.non_authoritative_compatibility_fingerprint()
                    == call.locator.non_authoritative_compatibility_fingerprint()
            }) {
                return Err(ObjectError::ForeignLocatorIdentityCollision {
                    caller: function.machine,
                    owner: call.owner,
                });
            }
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: normalized_foreign_import_symbol_name(&call.locator),
                section: SymbolSection::None,
                offset: 0,
                size: 0,
                kind: SymbolKind::Import,
                import_library: String::new(),
            });
            object.layout.normalized_imports.push(NormalizedImportPlan {
                symbol,
                locator: call.locator.clone(),
            });
            import_symbols.push((call.locator.clone(), symbol));
        }
    }

    let mut relocations = RelocationPlan::with_record_capacity(
        plan.target,
        relocation_record_capacity(
            plan,
            &dynamic_conformance_tables,
            &forwarded_dynamic_descriptor_tables,
            &forwarded_dynamic_descriptor_adapters,
        ),
    );
    for table in &dynamic_conformance_tables {
        for slot in &table.slots {
            let Some(target_symbol) = slot.target_symbol else {
                continue;
            };
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: table.symbol,
                },
                section: SectionKind::Data,
                offset: slot.data_offset,
                byte_width: 8,
                symbol_handle: target_symbol,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
    for table in &forwarded_dynamic_descriptor_tables {
        for slot in &table.slots {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: table.symbol,
                },
                section: SectionKind::Data,
                offset: slot.data_offset,
                byte_width: 8,
                symbol_handle: slot.adapter_symbol,
                addend: 0,
                kind: RelocationKind::Absolute64,
            });
        }
    }
    for adapter in &forwarded_dynamic_descriptor_adapters {
        let (offset, kind) = match plan.target.architecture {
            Architecture::X86_64 => (
                adapter
                    .text_offset
                    .checked_add(adapter.record.direct_call_offset)
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or(ObjectError::TextSizeOverflow)?,
                RelocationKind::X86_64Relative32,
            ),
            Architecture::Aarch64 => (
                adapter
                    .text_offset
                    .checked_add(adapter.record.direct_call_offset)
                    .ok_or(ObjectError::TextSizeOverflow)?,
                RelocationKind::Aarch64Branch26,
            ),
        };
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Materialization {
                object_symbol_handle: adapter.symbol,
            },
            section: SectionKind::Text,
            offset,
            byte_width: 4,
            symbol_handle: adapter.target_symbol,
            addend: 0,
            kind,
        });
    }
    for (function, emitted) in plan.functions.iter().zip(&functions) {
        for call in &function.dynamic_calls {
            let table = dynamic_conformance_tables
                .iter()
                .find(|table| {
                    table.application.commitment == call.dynamic_dispatch.application.commitment
                        && same_dynamic_table_application(
                            &table.application,
                            &call.dynamic_dispatch.application,
                        )
                })
                .ok_or(ObjectError::InvalidDynamicConformanceTable)?;
            let origin = RelocationOrigin::SemanticOperation {
                function_symbol_handle: emitted.symbol,
                operation_identity: call.psi_operation.get(),
            };
            let mut push_address =
                |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                    relocations.push_record(RelocationRecord {
                        origin,
                        section: SectionKind::Text,
                        offset: emitted
                            .text_offset
                            .checked_add(local_offset)
                            .ok_or(ObjectError::TextSizeOverflow)?,
                        byte_width: 4,
                        symbol_handle: table.symbol,
                        addend: 0,
                        kind,
                    });
                    Ok(())
                };
            match call.table_address.encoding {
                machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => push_address(relocation_offset, RelocationKind::X86_64Relative32)?,
                machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    push_address(page_relocation_offset, RelocationKind::Aarch64Page21)?;
                    push_address(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        for call in &function.stored_dynamic_calls {
            let establishment = &call.establishment;
            let table = dynamic_conformance_tables
                .iter()
                .find(|table| {
                    table.application.commitment == establishment.stored.application.commitment
                        && same_dynamic_table_application(
                            &table.application,
                            &establishment.stored.application,
                        )
                })
                .ok_or(ObjectError::InvalidDynamicConformanceTable)?;
            let origin = RelocationOrigin::SemanticOperation {
                function_symbol_handle: emitted.symbol,
                operation_identity: establishment.psi_operation.get(),
            };
            let mut push_address =
                |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                    relocations.push_record(RelocationRecord {
                        origin,
                        section: SectionKind::Text,
                        offset: emitted
                            .text_offset
                            .checked_add(local_offset)
                            .ok_or(ObjectError::TextSizeOverflow)?,
                        byte_width: 4,
                        symbol_handle: table.symbol,
                        addend: 0,
                        kind,
                    });
                    Ok(())
                };
            match establishment.table_address.encoding {
                machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => push_address(relocation_offset, RelocationKind::X86_64Relative32)?,
                machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                    page_relocation_offset,
                    page_offset_relocation_offset,
                } => {
                    push_address(page_relocation_offset, RelocationKind::Aarch64Page21)?;
                    push_address(
                        page_offset_relocation_offset,
                        RelocationKind::Aarch64PageOffset12,
                    )?;
                }
            }
        }
        for call in &function.forwarded_dynamic_descriptor_calls {
            for argument in &call.dynamic_arguments {
                let application = match &argument.custody.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        application,
                        ..
                    }
                    | abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } => application,
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {
                        return Err(ObjectError::InvalidForwardedDynamicDescriptorEvidence {
                            caller: function.machine,
                            operation: call.psi_operation,
                        });
                    }
                };
                let table = forwarded_dynamic_descriptor_tables
                    .iter()
                    .find(|table| {
                        table.application.commitment == application.commitment
                            && same_dynamic_table_application(&table.application, application)
                    })
                    .ok_or(ObjectError::InvalidForwardedDynamicDescriptorTable)?;
                let origin = RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: call.psi_operation.get(),
                };
                let mut push_address =
                    |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                        relocations.push_record(RelocationRecord {
                            origin,
                            section: SectionKind::Text,
                            offset: emitted
                                .text_offset
                                .checked_add(local_offset)
                                .ok_or(ObjectError::TextSizeOverflow)?,
                            byte_width: 4,
                            symbol_handle: table.symbol,
                            addend: 0,
                            kind,
                        });
                        Ok(())
                    };
                match argument.table_address.encoding {
                    machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => push_address(relocation_offset, RelocationKind::X86_64Relative32)?,
                    machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => {
                        push_address(page_relocation_offset, RelocationKind::Aarch64Page21)?;
                        push_address(
                            page_offset_relocation_offset,
                            RelocationKind::Aarch64PageOffset12,
                        )?;
                    }
                }
            }
        }
        for call in &function.internal_calls {
            let target_symbol = symbols_by_machine.get(&call.target).copied().ok_or(
                ObjectError::UnknownInternalCallTarget {
                    caller: function.machine,
                    target: call.target,
                },
            )?;
            let (kind, byte_width) = validate_internal_call_site(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                *call,
            )?;
            let offset = emitted
                .text_offset
                .checked_add(call.offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
            let origin = match call.owner {
                CallSiteOwner::Operation(operation) => RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: operation.get(),
                },
                CallSiteOwner::CleanupAction { edge, .. } => RelocationOrigin::SemanticEdge {
                    function_symbol_handle: emitted.symbol,
                    edge_identity: edge.get(),
                },
            };
            relocations.push_record(RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle: target_symbol,
                addend: 0,
                kind,
            });
        }
        for call in &function.foreign_calls {
            let origin = match call.owner {
                CallSiteOwner::Operation(operation) => RelocationOrigin::SemanticOperation {
                    function_symbol_handle: emitted.symbol,
                    operation_identity: operation.get(),
                },
                CallSiteOwner::CleanupAction { edge, .. } => RelocationOrigin::SemanticEdge {
                    function_symbol_handle: emitted.symbol,
                    edge_identity: edge.get(),
                },
            };
            if let Some(callback) = &call.callback_address {
                let Some((callback_symbol, callback_symbol_plan)) =
                    object_file::object_function_symbol(&object, callback.target.callback_function)
                else {
                    return Err(ObjectError::MissingCallbackPrivateFunction {
                        caller: function.machine,
                        owner: call.owner,
                    });
                };
                let matching_private = object_private_functions
                    .iter()
                    .filter(|private| private.identity == callback.target.callback_function)
                    .collect::<Vec<_>>();
                let [private] = matching_private.as_slice() else {
                    return Err(ObjectError::MissingCallbackPrivateFunction {
                        caller: function.machine,
                        owner: call.owner,
                    });
                };
                if private.function.symbol != callback_symbol
                    || private.function.text_offset != callback_symbol_plan.offset
                    || private.function.byte_count != callback_symbol_plan.size
                {
                    return Err(ObjectError::MissingCallbackPrivateFunction {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                let mut push_callback_relocation =
                    |local_offset: usize, kind: RelocationKind| -> Result<(), ObjectError> {
                        let offset = emitted
                            .text_offset
                            .checked_add(local_offset)
                            .ok_or(ObjectError::TextSizeOverflow)?;
                        relocations.push_record(RelocationRecord {
                            origin,
                            section: SectionKind::Text,
                            offset,
                            byte_width: 4,
                            symbol_handle: callback_symbol,
                            addend: 0,
                            kind,
                        });
                        Ok(())
                    };
                match callback.encoding {
                    machine_code::CallbackAddressEncoding::X86_64Relative32 {
                        relocation_offset,
                    } => push_callback_relocation(
                        relocation_offset,
                        RelocationKind::X86_64Relative32,
                    )?,
                    machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                        page_relocation_offset,
                        page_offset_relocation_offset,
                    } => {
                        push_callback_relocation(
                            page_relocation_offset,
                            RelocationKind::Aarch64Page21,
                        )?;
                        push_callback_relocation(
                            page_offset_relocation_offset,
                            RelocationKind::Aarch64PageOffset12,
                        )?;
                    }
                }
            }
            let target_symbol = import_symbols
                .iter()
                .find_map(|(locator, symbol)| (locator == &call.locator).then_some(*symbol))
                .ok_or(ObjectError::MissingForeignImportSymbol {
                    caller: function.machine,
                    owner: call.owner,
                })?;
            let (kind, byte_width) = validate_foreign_call_site(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                call,
            )?;
            let offset = emitted
                .text_offset
                .checked_add(call.offset)
                .ok_or(ObjectError::TextSizeOverflow)?;
            relocations.push_record(RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle: target_symbol,
                addend: 0,
                kind,
            });
        }
    }

    Ok(ObjectArtifact {
        hosted_receiver: None,
        requires_graph_storage_replay: false,
        fragment_replay: None,
        psi: plan.psi,
        target: plan.target,
        x86_feature_profile,
        x86_scalar_fma_provider,
        entry: plan.entry,
        object,
        relocations,
        text_bytes,
        data_bytes,
        dynamic_conformance_tables,
        forwarded_dynamic_descriptor_adapters,
        forwarded_dynamic_descriptor_tables,
        functions,
        private_functions: object_private_functions,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        foreign_calls,
    })
}

/// Collect each distinct dynamic-conformance application addressed by a
/// direct or stored dynamic call, rejecting commitment collisions.
fn collect_dynamic_applications(
    plan: &MachineCodePlan,
) -> Result<Vec<terminal_psi::ClosedConformanceApplication>, ObjectError> {
    let mut dynamic_applications = Vec::<terminal_psi::ClosedConformanceApplication>::new();
    for application in plan
        .functions
        .iter()
        .flat_map(|function| &function.dynamic_calls)
        .map(|call| &call.dynamic_dispatch.application)
        .chain(
            plan.functions
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .map(|call| &call.establishment.stored.application),
        )
    {
        if let Some(existing) = dynamic_applications
            .iter()
            .find(|existing| existing.commitment == application.commitment)
        {
            if !same_dynamic_table_application(existing, application) {
                return Err(ObjectError::DynamicConformanceCommitmentCollision);
            }
        } else {
            dynamic_applications.push(application.clone());
        }
    }
    Ok(dynamic_applications)
}

/// Size of the `.data` section holding the dynamic-conformance and forwarded
/// descriptor tables, eight bytes per slot.
fn dynamic_data_section_size(
    dynamic_applications: &[terminal_psi::ClosedConformanceApplication],
    forwarded_dynamic_applications: &[ValidatedForwardedDynamicApplication],
) -> Result<usize, ObjectError> {
    dynamic_applications
        .iter()
        .try_fold(0usize, |size, application| {
            application
                .rows
                .len()
                .checked_mul(8)
                .and_then(|bytes| size.checked_add(bytes))
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
        })?
        .checked_add(forwarded_dynamic_applications.iter().try_fold(
            0usize,
            |size, application| {
                application
                    .adapters
                    .len()
                    .checked_mul(8)
                    .and_then(|bytes| size.checked_add(bytes))
                    .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
            },
        )?)
        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
}

/// Exact relocation-record capacity across internal, foreign, callback,
/// dynamic-table, and forwarded-descriptor relocations.
fn relocation_record_capacity(
    plan: &MachineCodePlan,
    dynamic_conformance_tables: &[ObjectDynamicConformanceTable],
    forwarded_dynamic_descriptor_tables: &[ObjectForwardedDynamicDescriptorTable],
    forwarded_dynamic_descriptor_adapters: &[ObjectForwardedDynamicDescriptorAdapter],
) -> usize {
    let ordinary_relocation_count = plan
        .functions
        .iter()
        .map(|function| function.internal_calls.len() + function.foreign_calls.len())
        .sum::<usize>();
    let callback_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.foreign_calls)
        .filter_map(|call| call.callback_address.as_ref())
        .map(|callback| match callback.encoding {
            machine_code::CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let dynamic_table_relocation_count = dynamic_conformance_tables
        .iter()
        .flat_map(|table| &table.slots)
        .filter(|slot| slot.target_symbol.is_some())
        .count();
    let forwarded_table_relocation_count = forwarded_dynamic_descriptor_tables
        .iter()
        .map(|table| table.slots.len())
        .sum::<usize>();
    let forwarded_adapter_relocation_count = forwarded_dynamic_descriptor_adapters.len();
    let dynamic_address_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.dynamic_calls)
        .map(|call| match call.table_address.encoding {
            machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let stored_dynamic_address_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.stored_dynamic_calls)
        .map(|call| match call.establishment.table_address.encoding {
            machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    let forwarded_address_relocation_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.forwarded_dynamic_descriptor_calls)
        .flat_map(|call| &call.dynamic_arguments)
        .map(|argument| match argument.table_address.encoding {
            machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. } => 1,
            machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. } => 2,
        })
        .sum::<usize>();
    ordinary_relocation_count
        .saturating_add(callback_relocation_count)
        .saturating_add(dynamic_table_relocation_count)
        .saturating_add(dynamic_address_relocation_count)
        .saturating_add(stored_dynamic_address_relocation_count)
        .saturating_add(forwarded_table_relocation_count)
        .saturating_add(forwarded_adapter_relocation_count)
        .saturating_add(forwarded_address_relocation_count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectError {
    EmptyPlan,
    DynamicConformanceCommitmentCollision,
    DynamicConformanceDataSizeOverflow,
    InvalidDynamicConformanceTable,
    InvalidForwardedDynamicDescriptorEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidDynamicParameterCallEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidForwardedDynamicParameterCallEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    ForwardedDynamicDescriptorCommitmentCollision,
    DuplicateForwardedDynamicDescriptorAdapter,
    InvalidForwardedDynamicDescriptorTable,
    UnknownDynamicConformanceTarget(MachineId),
    NonCanonicalFunctionOrder {
        previous: MachineId,
        current: MachineId,
    },
    EmptyFunction(MachineId),
    TooManyPrivateFunctions,
    InvalidPrivateFunctionIdentity,
    EmptyPrivateFunctionSymbol,
    PrivateFunctionSymbolCollision,
    InvalidPrivateFunctionAbi,
    InvalidPrivateFunctionBody,
    UnsupportedPrivateFunctionBody,
    MissingX86ScalarFmaFragment,
    InvalidX86ScalarFmaProviderAdmission,
    MissingX86ScalarFmaProfile(MachineId),
    X86ScalarFmaUnsupportedTarget(MachineId),
    NonCanonicalX86ScalarFmaOrder(MachineId),
    InvalidX86ScalarFmaInterval {
        machine: MachineId,
        offset: usize,
    },
    InvalidX86ScalarFmaEncoding {
        machine: MachineId,
        offset: usize,
    },
    InvalidX86ScalarFmaCustody {
        machine: MachineId,
        offset: usize,
    },
    InvalidX86ScalarFmaSemanticCustody(MachineId),
    InvalidX86ScalarFmaFloatingControl(MachineId),
    MissingX86ScalarFmaCustody {
        machine: MachineId,
        offset: usize,
    },
    NonCanonicalInternalCallOrder(MachineId),
    NonCanonicalForeignCallOrder(MachineId),
    NonCanonicalSemanticCodeAttributionOrder(MachineId),
    SemanticCodeAttributionOutsideFunction(MachineId),
    InvalidSemanticCodeAttribution(MachineId),
    NonCanonicalPortEffectOrder(MachineId),
    NonCanonicalBoundarySettlementOrder(MachineId),
    InvalidStructuralReturnEvidence(MachineId),
    StructuralReturnEvidenceConflict(MachineId),
    StructuralReturnBytesMismatch(MachineId),
    UnknownInternalCallTarget {
        caller: MachineId,
        target: MachineId,
    },
    InvalidInternalCallSite {
        caller: MachineId,
        owner: CallSiteOwner,
        offset: usize,
    },
    InvalidForeignCallSite {
        caller: MachineId,
        owner: CallSiteOwner,
        offset: usize,
    },
    InvalidForeignCallArgument {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidDynamicCallEvidence {
        caller: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidUnitDynamicDescriptorJoin(MachineId),
    InvalidCallbackAddressCustody {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingCallbackPrivateFunction {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidForeignCallFloatingControl {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    ForeignCallOwnerNotInProvenance {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    DuplicateForeignCallOwner {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    ForeignCallTargetMismatch {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    ForeignStackProviderPlanMismatch {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    UnsupportedForeignStackAlignment {
        caller: MachineId,
        owner: CallSiteOwner,
        admitted_alignment: u64,
        physical_alignment: u32,
    },
    ForeignCallOverlapsInternalCall {
        caller: MachineId,
        offset: usize,
    },
    ForeignLocatorIdentityCollision {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingForeignImportSymbol {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidInternalUnitCallEvidence(MachineId),
    InvalidInternalUnitScalarCallEvidence(MachineId),
    InvalidUnitScalarFunctionAbi(MachineId),
    InvalidInstalledProviderUnitScalarCallEvidence(MachineId),
    InvalidUnitWriteOnlyPrimitiveStoreEvidence(MachineId),
    InvalidUnitStructuralScalarFieldStoreEvidence(MachineId),
    InvalidScalarStructuralScalarFieldStoreEvidence(MachineId),
    InvalidUnitAffineCleanupEvidence(MachineId),
    InternalCallOperationNotInProvenance {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    DuplicateInternalCallOperation {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingUnitCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    UnexpectedUnitCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingScalarCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    UnexpectedScalarCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidUnitStackAlignment {
        machine: MachineId,
        alignment: u32,
    },
    ConflictingTerminalStackEvidence(MachineId),
    InvalidScalarStackAlignment {
        machine: MachineId,
        alignment: u32,
    },
    InvalidScalarConditionalEvidence {
        machine: MachineId,
        offset: usize,
    },
    ScalarConditionalCallOutsideArm {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
        offset: usize,
    },
    UntypedScalarInternalCall {
        machine: MachineId,
        offset: usize,
    },
    InvalidScalarCallStackEvidence {
        caller: MachineId,
        owner: CallSiteOwner,
        offset: usize,
    },
    MisalignedScalarCalleeEntry {
        caller: MachineId,
        owner: CallSiteOwner,
        caller_live_bytes: u32,
    },
    NonCanonicalScalarStackMutationOrder(MachineId),
    InvalidScalarInstructionEncoding {
        machine: MachineId,
        offset: usize,
    },
    NonLinearScalarControlFlow {
        machine: MachineId,
        offset: usize,
    },
    UnclaimedScalarStackMutation {
        machine: MachineId,
        offset: usize,
    },
    UnsupportedScalarStackMutation {
        machine: MachineId,
        offset: usize,
    },
    InvalidScalarStackEvidence {
        machine: MachineId,
        offset: usize,
    },
    MissingBalancedScalarReturn(MachineId),
    ScalarStackArithmeticOverflow(MachineId),
    ScalarStackReleaseExceedsAllocation {
        machine: MachineId,
        offset: usize,
    },
    UnitCallStackArithmeticOverflow {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    InvalidUnitStackEncoding {
        machine: MachineId,
        owner: Option<CallSiteOwner>,
        offset: usize,
    },
    InvalidUnitInstructionEncoding {
        machine: MachineId,
        offset: usize,
    },
    DuplicateUnitStackAdjustment(MachineId),
    UnclaimedUnitStackAdjustment {
        machine: MachineId,
        offset: usize,
    },
    UnclaimedUnitStackMutation {
        machine: MachineId,
        offset: usize,
    },
    MisalignedUnitCalleeEntry {
        caller: MachineId,
        owner: CallSiteOwner,
        caller_live_bytes: u32,
    },
    MissingX86UnitCallStackAdjustment {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    MissingAarch64UnitReturnLink {
        caller: MachineId,
        operation: Option<semantic_vocabulary::OperationId>,
    },
    UnaccountedTerminalStack(MachineId),
    TerminalStackCycle(MachineId),
    TerminalStackCompositionOverflow {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    PortEffectOutsideFunction {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    PortEffectOperationNotInProvenance {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    DuplicatePortEffectOperation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    PortEffectBytesMismatch {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    BoundarySettlementOutsideFunction {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    BoundarySettlementOperationNotInProvenance {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    DuplicateBoundarySettlementOperation {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidBoundarySettlementArgumentPath {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidCompletionReceiptArgumentIndex {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidCompletionReceiptCustody {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    InvalidCompletionProviderCustody {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: semantic_vocabulary::OperationId,
    },
    EntryFunctionMissing(MachineId),
    TextSizeOverflow,
}

impl std::fmt::Display for ObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ObjectError {}
