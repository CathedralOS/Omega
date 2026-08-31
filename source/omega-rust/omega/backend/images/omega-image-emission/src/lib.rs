#![forbid(unsafe_code)]

//! Standalone object and executable-image emission for the clean terminal-Psi
//! realization lane.
//!
//! This crate consumes only owned terminal machine-code functions. It does not
//! reconstruct the legacy `EncodedMachineCode` carrier or any source-shaped
//! lowering state. Typed internal-call relocations may change only their exact
//! architecture-native immediate fields; every other compiler-authored bit and
//! every provenance-bearing function region remains final-byte validated.
//!
//! The adjacent canonical installation record is manifest metadata over the
//! resulting sealed image. It does not grant executable authority or replace
//! the separate native admission, placement, and retirement ladder.

mod boundary_results;
mod byte_sequence_custody;
mod completion_receipts;
mod dynamic_elf;
mod final_image_validation;
mod fully_consumed_affine_pair;
mod image_output;
mod installation;
#[cfg(feature = "installed-artifact")]
mod installed_artifact;
mod instruction_loads;
mod partial_cleanup_partition;
mod ranked_u32_countdown;
mod scalar_call_stack;
mod scalar_cleanup_preservation;
mod scalar_conditional_call_paths;
mod scalar_conditional_regions;
mod scalar_conditional_stack;
mod scalar_control_cleanup;
mod scalar_division_stack;
mod scalar_shared_convergence;
mod scalar_stack;
mod scalar_stack_mutation;
mod stack_demand;
mod structural_condition_layout;
mod structural_condition_read;
mod structural_return;
mod unit_affine_cleanup;
mod unit_call_custody;
mod unit_scalar_call_custody;
mod unit_stack;
mod x86_fma;

pub use dynamic_elf::{
    DynamicElfImageEmission, DynamicElfImageEmissionError, DynamicElfOrchestrationError,
    emit_admitted_dynamic_elf_image, emit_dynamic_elf_image, validate_dynamic_elf_image_emission,
};
pub use image_output::{
    ExecutableImage, ObjectContainer, ScalarCallReferenceImage, can_emit_executable_image,
    emit_executable_image, emit_object_container, emit_scalar_call_reference_linux_x86_64_image,
    validate_executable_image,
};
pub use installation::*;
#[cfg(feature = "installed-artifact")]
pub use installed_artifact::{
    InstalledArtifact, InstalledArtifactBindingError, bind_installed_artifact,
};
pub use omega_machine_code::BoundaryExecutionRecord;
pub(crate) use partial_cleanup_partition::exact_partial_cleanup_partition;
pub use stack_demand::{derive_stack_demand, derive_unit_stack_demand};

use boundary_results::boundary_result_is_exact;
use byte_sequence_custody::linux_write_line_custody_is_exact;
use completion_receipts::{CompletionCustodyError, validate_completion_custody};
use fully_consumed_affine_pair::{
    exact_fully_consumed_affine_pair, exact_partially_consumed_affine_array,
};
use scalar_cleanup_preservation::validate_scalar_cleanup_preservation;
use scalar_conditional_call_paths::{conditional_call_path, conditional_paths_are_exclusive};
use scalar_control_cleanup::{cleanup_for_owner, validate_scalar_control_cleanup_evidence};
use scalar_stack::validate_scalar_stack;
use structural_return::validate_structural_return_record;
use unit_affine_cleanup::validate_unit_affine_cleanup;
use unit_call_custody::{expected_projected_copy_bytes, validate_internal_unit_call_custody};
use unit_scalar_call_custody::validate_internal_unit_scalar_calls;
use unit_stack::{
    validate_complete_unit_stack_evidence, validate_foreign_unit_call_stack,
    validate_unit_call_stack, validate_unit_function_stack,
};

use omega_machine_code::{
    BoundarySettlementRecord, MachineCodePlan, PortEffectRecord, ScalarControlAffineCleanupRecord,
    SemanticCodeAttribution, SemanticCodeSite, StructuralReturnRecord,
};
use omega_object_file::{
    NormalizedImportPlan, ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin,
    RelocationPlan, RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan,
    SymbolSection, entry_symbol_name, normalized_foreign_import_symbol_name,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{BoundaryRealization, CallSiteOwner, TerminalPsiProvenance};
use psi_core::MachineId;
use psi_terminal::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectArtifact {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    /// Exact deployment profile for the source-free feature-requiring x86 FMA
    /// seam. Ordinary object construction retains `None` and rejects FMA.
    x86_feature_profile: Option<omega_target::TargetProfile>,
    /// Consumed feature/differential authority for generic x86 FMA slots.
    /// The feature-required mechanics seam retains `None` and remains
    /// non-executable.
    x86_scalar_fma_provider: Option<omega_target::AdmittedX86ScalarFmaProvider>,
    entry: MachineId,
    object: ObjectPlan,
    relocations: RelocationPlan,
    text_bytes: Vec<u8>,
    functions: Vec<ObjectFunction>,
    semantic_code_attribution: Vec<ObjectCodeAttribution>,
    port_effects: Vec<ObjectPortEffect>,
    boundary_settlements: Vec<ObjectBoundarySettlement>,
}

impl ObjectArtifact {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn x86_feature_profile(&self) -> Option<omega_target::TargetProfile> {
        self.x86_feature_profile
    }

    pub const fn x86_scalar_fma_provider(
        &self,
    ) -> Option<omega_target::AdmittedX86ScalarFmaProvider> {
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

    pub fn functions(&self) -> &[ObjectFunction] {
        &self.functions
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

    pub fn port_effects(&self) -> &[ObjectPortEffect] {
        &self.port_effects
    }

    pub fn semantic_code_attribution(&self) -> &[ObjectCodeAttribution] {
        &self.semantic_code_attribution
    }
}

impl omega_installation_evidence::ObjectEvidence for ObjectArtifact {
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
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub fixed_integer_scalar_abi: Option<omega_target_operations::FixedIntegerScalarFunctionAbi>,
    pub provenance: TerminalPsiProvenance,
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
    /// Independently replayed feature requirements for exact scalar FMA3
    /// intervals. These remain requirements, not executable admission.
    pub x86_scalar_fma: Vec<omega_machine_code::X86ScalarFmaFragment>,
    /// Byte-validated stack facts for a completely accounted Unit body.
    pub unit_stack: Option<ObjectUnitStack>,
    /// Byte-validated stack facts for a branch-free scalar body.
    pub scalar_stack: Option<ObjectScalarStack>,
    pub unit_call_stacks: Vec<ObjectUnitCallStack>,
    pub scalar_call_stacks: Vec<ObjectScalarCallStack>,
    pub internal_unit_calls: Vec<omega_machine_code::InternalUnitCallRecord>,
    pub internal_unit_scalar_calls: Vec<omega_machine_code::InternalUnitScalarCallRecord>,
    pub unit_scalar_homes: Vec<omega_machine_code::UnitScalarHomeRecord>,
    pub unit_integer_constants: Vec<omega_machine_code::UnitIntegerConstantRecord>,
    pub unit_parameters: Vec<omega_machine_code::UnitParameterRecord>,
    pub unit_parameter_homes: Vec<omega_machine_code::UnitParameterHomeRecord>,
    pub unit_affine_cleanup: Option<omega_machine_code::UnitAffineCleanupRecord>,
    pub scalar_affine_cleanup: Option<omega_machine_code::UnitAffineCleanupRecord>,
    /// Three byte-validated scalar cleanup records in canonical physical/DFS
    /// return-leaf order for the bounded two-decision Boolean control lane.
    pub scalar_control_affine_cleanups: Vec<ScalarControlAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<omega_machine_code::UnitParameterRecord>,
    pub scalar_structural_parameter_homes: Vec<omega_machine_code::UnitParameterHomeRecord>,
    /// Independently replayed proof, rank, ABI, frontier, and fuel custody for
    /// the exact unmetered ranked-`u32` object body.
    pub ranked_u32_countdown: Option<omega_machine_code::RankedU32CountdownMachineCodeRecord>,
    /// Byte-validated structural custody returned by this function, when the
    /// complete one-fragment slice applies.
    pub structural_return: Option<StructuralReturnRecord>,
}

impl ObjectFunction {
    pub fn bytes<'artifact>(&self, artifact: &'artifact ObjectArtifact) -> &'artifact [u8] {
        &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
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

/// Recomputed stack demand for the accounted terminal function slices. This
/// excludes the external entry adapter/interrupt arrival frame, which belongs
/// to installed-root realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackDemand {
    psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    ceiling_bytes: u64,
    stack_alignment: u32,
    contributing_machines: std::collections::BTreeSet<MachineId>,
}

impl StackDemand {
    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn ceiling_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    pub const fn stack_alignment(&self) -> u32 {
        self.stack_alignment
    }

    pub const fn contributing_machines(&self) -> &std::collections::BTreeSet<MachineId> {
        &self.contributing_machines
    }
}

impl omega_installation_evidence::StackDemandEvidence for StackDemand {
    fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    fn architecture(&self) -> Architecture {
        self.target.architecture
    }

    fn entry(&self) -> MachineId {
        self.entry
    }

    fn ceiling_bytes(&self) -> u64 {
        self.ceiling_bytes
    }

    fn stack_alignment(&self) -> u32 {
        self.stack_alignment
    }

    fn contributing_machines(&self) -> &std::collections::BTreeSet<MachineId> {
        &self.contributing_machines
    }
}

/// Compatibility name for the original Unit-only demand entry point. New
/// callers should use [`StackDemand`] and [`derive_stack_demand`].
pub type UnitStackDemand = StackDemand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectBoundarySettlement {
    pub machine: MachineId,
    pub settlement: BoundarySettlementRecord,
    /// Absolute offset in the object `.text` section.
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

/// Product-owned Linux process-entry adapter for a zero-argument scalar
/// terminal entry function. The semantic machine functions retain their exact
/// bytes and order; this separately classified suffix calls the semantic entry
/// and terminates the process with its low 32-bit result through `exit_group`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinuxX86ScalarExitShim {
    pub symbol: ObjectSymbolHandle,
    pub target_symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
    /// Absolute offset of the `call rel32` immediate in object `.text`.
    pub relocation_offset: usize,
}

pub(crate) const LINUX_X86_SCALAR_EXIT_SHIM_BYTES: [u8; 16] = [
    0xe8, 0, 0, 0, 0, // call rel32 (owned relocation to the semantic entry)
    0x89, 0xc7, // mov edi, eax
    0xb8, 0xe7, 0, 0, 0, // mov eax, 231 (exit_group)
    0x0f, 0x05, // syscall
    0x0f, 0x0b, // ud2 if the nonreturning syscall unexpectedly returns
];

/// Semantic identity of the exact published proof-free i32 scalar-call
/// reference. Binding the process adapter to this identity is necessary because
/// ordinary scalar arity is not retained by `ObjectFunction`: an unused
/// entry parameter can otherwise have byte-identical machine code.
pub(crate) const SCALAR_CALL_REFERENCE_FINGERPRINT: [u8; 32] = [
    0x02, 0x5f, 0x4b, 0x5a, 0xa3, 0xdf, 0xd7, 0x0c, 0xdc, 0x92, 0x84, 0x3c, 0x41, 0x4c, 0x0b, 0x86,
    0x91, 0x08, 0x9e, 0x09, 0x19, 0x27, 0xb5, 0x49, 0x61, 0xff, 0x45, 0x34, 0xd9, 0x47, 0x64, 0x3d,
];

/// Construct a self-contained object plan and exact text carrier.
///
/// Function order is semantic-artifact order and must already be canonical by
/// `MachineId`; this boundary rejects alternate ordering rather than silently
/// normalizing it. Each function gets exactly one symbol and one retained Psi
/// provenance row.
pub fn build_object_artifact(plan: &MachineCodePlan) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(plan, None, None)
}

/// Construct the bounded source-free object seam for feature-requiring scalar
/// x86 FMA. The profile is explicit because `NativeTarget` deliberately
/// collapses Windows and UEFI x86-64 physical layouts.
pub fn build_feature_required_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    profile: omega_target::TargetProfile,
) -> Result<ObjectArtifact, ObjectError> {
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(plan, Some(profile), None)
}

/// Consume exact deployment-feature and differential authority while building
/// an object whose generic F32/F64 FMA slots may enter executable emission.
/// Ordinary and feature-required-only builders retain their fail-closed
/// baseline behavior.
pub fn build_admitted_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    provider: omega_target::AdmittedX86ScalarFmaProvider,
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
                    omega_machine_code::X86ScalarFmaFormat::Binary32 => {
                        omega_target::X86ScalarFmaSlot::Binary32
                    }
                    omega_machine_code::X86ScalarFmaFormat::Binary64 => {
                        omega_target::X86ScalarFmaSlot::Binary64
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
    build_object_artifact_with_x86_feature_profile(plan, Some(provider.profile()), Some(provider))
}

fn build_object_artifact_with_x86_feature_profile(
    plan: &MachineCodePlan,
    x86_feature_profile: Option<omega_target::TargetProfile>,
    x86_scalar_fma_provider: Option<omega_target::AdmittedX86ScalarFmaProvider>,
) -> Result<ObjectArtifact, ObjectError> {
    if plan.functions.is_empty() {
        return Err(ObjectError::EmptyPlan);
    }
    ranked_u32_countdown::replay_ranked_u32_countdown(plan)?;
    let mut previous = None;
    let mut saw_entry = false;
    let mut text_size = 0usize;
    let mut validated_unit_stacks = std::collections::BTreeMap::new();
    let mut validated_scalar_stacks = std::collections::BTreeMap::new();
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
        x86_fma::validate_x86_scalar_fma_function(
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
            validate_foreign_scalar_arguments(
                plan.target,
                function.machine,
                &function.bytes,
                &function.semantic_code_attribution,
                call,
            )?;
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
        if function.unit_stack.is_some() && function.scalar_stack.is_some() {
            return Err(ObjectError::ConflictingTerminalStackEvidence(
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
            function_stack.local_peak_bytes =
                function_stack.local_peak_bytes.max(caller_live_bytes);
        }
        let is_unit_custody_relocation = |call: &&omega_machine_code::InternalCallRelocation| {
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
                    | BoundaryRealization::LinuxExitGroupI32(_)
            )
        });
        let scalar_custody = scalar_cleanup_custody || scalar_boundary_custody;
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
        let fully_consumed_affine_pair = exact_fully_consumed_affine_pair(
            parameter_homes,
            &function.internal_unit_calls,
            default_affine_cleanup,
        );
        let partially_consumed_affine_array = exact_partially_consumed_affine_array(
            parameter_homes,
            &function.internal_unit_calls,
            default_affine_cleanup,
        );
        for custody in &function.internal_unit_calls {
            let target_returns_scalar = machine_functions
                .get(&custody.target)
                .copied()
                .is_some_and(|target| {
                    target.scalar_stack.is_some()
                        || (target.unit_stack.is_some()
                            && target
                                .internal_unit_calls
                                .iter()
                                .any(|call| call.result.is_some()))
                });
            let target_structural_return = machine_functions
                .get(&custody.target)
                .and_then(|target| target.structural_return.as_ref());
            let structural_result_valid =
                match (&custody.structural_result, target_structural_return) {
                    (None, None) => true,
                    (Some(result), Some(target)) => {
                        custody.result.is_none()
                            && result.operation_result.structural_type
                                == target.result.structural_type
                            && result.operation_result.multiplicity == target.result.multiplicity
                            && result.operation_result.qualifications
                                == target.result.qualifications
                            && result.function_result.structural_type
                                == target.result.structural_type
                            && result.function_result.multiplicity == target.result.multiplicity
                            && result.function_result.qualifications == target.result.qualifications
                            && result.returned_claim_transfers.len() == 1
                            && target.returned_claims.as_slice()
                                == [result.returned_claim_transfers[0].callee_claim]
                            && result.operation_result.claims.len() == 1
                            && result.operation_result.claims[0].claim
                                == result.returned_claim_transfers[0].caller_claim
                            && result.returned_claims.as_slice()
                                == [result.returned_claim_transfers[0].caller_claim]
                            && result.caller_result_placement == target.result_placement
                            && result.callee_result_placement == target.result_placement
                    }
                    _ => false,
                };
            if custody.result.is_some() != target_returns_scalar
                || !structural_result_valid
                || (custody.structural_result.is_some() && target_returns_scalar)
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
                    .or(default_affine_cleanup);
            validate_internal_unit_call_custody(
                plan.target,
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.internal_calls,
                parameter_homes,
                validated_function_stack.as_ref(),
                unit_call_stack,
                scalar_call_stack,
                custody,
                affine_cleanup,
                fully_consumed_affine_pair,
            )?;
        }
        validate_internal_unit_scalar_calls(
            plan.target,
            function,
            &machine_functions,
            validated_function_stack.as_ref(),
            &validated_call_stacks,
        )?;
        match (&function.unit_stack, &function.unit_affine_cleanup) {
            (Some(_), Some(cleanup)) => validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.semantic_code_attribution,
                &function.unit_parameter_homes,
                &function.internal_unit_calls,
                &attachments,
                &machine_functions,
                cleanup,
                false,
                fully_consumed_affine_pair,
                partially_consumed_affine_array,
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
                &attachments,
                &machine_functions,
                cleanup,
                true,
                false,
                false,
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
                    &attachments,
                    &machine_functions,
                    &record.cleanup,
                    true,
                    false,
                    false,
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
                plan.target.architecture,
                function.machine,
                &function.bytes,
                stack,
                &function.internal_calls,
                &function.foreign_calls,
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
                    != omega_x86_encoding::encode_immediate_port_write(effect.port, effect.value)
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
                    CompletionCustodyError::InvalidArgumentPath => {
                        ObjectError::InvalidBoundarySettlementArgumentPath {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::InvalidReceiptArgumentIndex => {
                        ObjectError::InvalidCompletionReceiptArgumentIndex {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::InvalidReceiptCustody => {
                        ObjectError::InvalidCompletionReceiptCustody {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::InvalidProviderCustody => {
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
                        && settlement.byte_sequence_arguments.is_empty()
                        && settlement.native_result.is_none()
                        && settlement.byte_count == 0
                }
                BoundaryRealization::DirectPortReadU8(realization) => {
                    let expected =
                        omega_x86_encoding::encode_immediate_port_read_u8(realization.port);
                    let exact_return_edge =
                        settlement.native_result.as_ref().is_some_and(|result| {
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
                BoundaryRealization::LinuxExitGroupI32(_) => {
                    let [argument] = settlement.scalar_arguments.as_slice() else {
                        return Err(ObjectError::BoundaryRealizationMismatch {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        });
                    };
                    let i32_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                        .expect("i32 is valid");
                    let value = match (argument.scalar_type, argument.immediate) {
                        (
                            psi_core::ScalarType::Integer(actual),
                            psi_core::IntegerValue::Signed(value),
                        ) if actual == i32_type => i32::try_from(value).ok(),
                        _ => None,
                    };
                    let expected_destination =
                        match (plan.target.object_format, plan.target.architecture) {
                            (omega_target::ObjectFormat::Elf, Architecture::X86_64) => {
                                Some(omega_calling_conventions::MachineRegister::X86Rdi)
                            }
                            (omega_target::ObjectFormat::Elf, Architecture::Aarch64) => {
                                Some(omega_calling_conventions::MachineRegister::Aarch64X(0))
                            }
                            _ => None,
                        };
                    let expected = value.and_then(|value| match plan.target.architecture {
                        Architecture::X86_64 => {
                            Some(omega_isa_x86_64::encode_linux_exit_group_i32(value))
                        }
                        Architecture::Aarch64 => {
                            omega_isa_aarch64::encode_linux_exit_group_i32(value).ok()
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
                                        && attribution
                                            .code_offset
                                            .checked_add(attribution.byte_count)
                                            == Some(function.bytes.len())
                                        && (function.unit_stack.is_some()
                                            || attribution.byte_count == 0)
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
                        && settlement.arguments.is_empty()
                        && settlement.byte_sequence_arguments.is_empty()
                        && settlement.native_result.is_none()
                        && function.scalar_stack.is_none()
                        && exact_nominal_tail
                }
            };
            if !valid_realization
                || !boundary_result_is_exact(
                    plan.target,
                    settlement.realization,
                    settlement.native_result.as_ref(),
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

    let foreign_call_count = plan
        .functions
        .iter()
        .map(|function| function.foreign_calls.len())
        .sum::<usize>();
    let mut object = ObjectPlan::with_capacity(
        plan.target,
        1,
        plan.functions.len().saturating_add(foreign_call_count),
    );
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });

    let mut text_bytes = Vec::with_capacity(text_size);
    let mut functions = Vec::with_capacity(plan.functions.len());
    let mut semantic_code_attribution = Vec::new();
    let mut port_effects = Vec::new();
    let mut boundary_settlements = Vec::new();
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
            fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
            x86_scalar_fma: function.x86_scalar_fma.clone(),
            unit_stack,
            scalar_stack,
            unit_call_stacks,
            scalar_call_stacks,
            internal_unit_calls: function.internal_unit_calls.clone(),
            internal_unit_scalar_calls: function.internal_unit_scalar_calls.clone(),
            unit_scalar_homes: function.unit_scalar_homes.clone(),
            unit_integer_constants: function.unit_integer_constants.clone(),
            unit_parameters: function.unit_parameters.clone(),
            unit_parameter_homes: function.unit_parameter_homes.clone(),
            unit_affine_cleanup: function.unit_affine_cleanup.clone(),
            scalar_affine_cleanup: function.scalar_affine_cleanup.clone(),
            scalar_control_affine_cleanups: function.scalar_control_affine_cleanups.clone(),
            scalar_structural_parameters: function.scalar_structural_parameters.clone(),
            scalar_structural_parameter_homes: function.scalar_structural_parameter_homes.clone(),
            ranked_u32_countdown: function.ranked_u32_countdown.clone(),
            structural_return: function.structural_return.clone(),
        });
    }

    let mut import_symbols =
        Vec::<(omega_target::NormalizedForeignLocator, ObjectSymbolHandle)>::new();
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
        plan.functions
            .iter()
            .map(|function| function.internal_calls.len() + function.foreign_calls.len())
            .sum(),
    );
    for (function, emitted) in plan.functions.iter().zip(&functions) {
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
    }

    Ok(ObjectArtifact {
        psi: plan.psi,
        target: plan.target,
        x86_feature_profile,
        x86_scalar_fma_provider,
        entry: plan.entry,
        object,
        relocations,
        text_bytes,
        functions,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
    })
}

fn validate_internal_call_site(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    call: omega_machine_code::InternalCallRelocation,
) -> Result<(RelocationKind, usize), ObjectError> {
    let valid = match architecture {
        Architecture::X86_64 => {
            call.offset >= 1
                && bytes.get(call.offset - 1) == Some(&0xe8)
                && bytes.get(call.offset..call.offset.saturating_add(4)) == Some(&[0; 4])
        }
        Architecture::Aarch64 => {
            call.offset.is_multiple_of(4)
                && bytes.get(call.offset..call.offset.saturating_add(4))
                    == Some(&0x9400_0000u32.to_le_bytes())
        }
    };
    if !valid {
        return Err(ObjectError::InvalidInternalCallSite {
            caller,
            owner: call.owner,
            offset: call.offset,
        });
    }
    Ok(match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4),
    })
}

fn validate_foreign_call_site(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    call: &omega_machine_code::ForeignCallRelocation,
) -> Result<(RelocationKind, usize), ObjectError> {
    let valid = match architecture {
        Architecture::X86_64 => {
            call.offset >= 1
                && bytes.get(call.offset - 1) == Some(&0xe8)
                && bytes.get(call.offset..call.offset.saturating_add(4)) == Some(&[0; 4])
        }
        Architecture::Aarch64 => {
            call.offset.is_multiple_of(4)
                && bytes.get(call.offset..call.offset.saturating_add(4))
                    == Some(&0x9400_0000u32.to_le_bytes())
        }
    };
    if !valid {
        return Err(ObjectError::InvalidForeignCallSite {
            caller,
            owner: call.owner,
            offset: call.offset,
        });
    }
    Ok(match architecture {
        Architecture::X86_64 => (RelocationKind::X86_64Relative32, 4),
        Architecture::Aarch64 => (RelocationKind::Aarch64Branch26, 4),
    })
}

fn validate_foreign_scalar_arguments(
    target: NativeTarget,
    caller: MachineId,
    bytes: &[u8],
    semantic_code_attribution: &[SemanticCodeAttribution],
    call: &omega_machine_code::ForeignCallRelocation,
) -> Result<(), ObjectError> {
    if call.scalar_arguments.is_empty() {
        return (call.call_plan.parameters.is_empty()
            && call.call_plan.result.is_none()
            && call.call_plan.callback_materializations.is_empty()
            && call.call_plan.policy
                == omega_calling_conventions::CallingPolicy::native_for_target(target)
            && call.call_plan.entry_control
                == omega_calling_conventions::EntryControl::CallReturn)
            .then_some(())
            .ok_or(ObjectError::InvalidForeignCallArgument {
                caller,
                owner: call.owner,
            });
    }
    let shapes = call
        .scalar_arguments
        .iter()
        .map(|argument| {
            let bits = argument.scalar_type.bits();
            if argument.scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
                || !matches!(bits, 8 | 16 | 32 | 64)
                || !argument.scalar_type.admits(argument.immediate)
            {
                return Err(ObjectError::InvalidForeignCallArgument {
                    caller,
                    owner: call.owner,
                });
            }
            let byte_size = bits / 8;
            Ok(omega_calling_conventions::ValueShape::integer(
                byte_size, byte_size,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = omega_calling_conventions::CallSignature {
        parameters: shapes.clone(),
        result: None,
    };
    let expected_plan = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &signature,
    )
    .map_err(|_| ObjectError::InvalidForeignCallArgument {
        caller,
        owner: call.owner,
    })?;
    if call.call_plan != expected_plan {
        return Err(ObjectError::InvalidForeignCallArgument {
            caller,
            owner: call.owner,
        });
    }
    let call_start = match target.architecture {
        Architecture::X86_64 => call.offset.saturating_sub(1),
        Architecture::Aarch64 => call.offset,
    };
    let next_interval = call
        .unit_stack
        .outbound
        .map_or(call_start, |outbound| outbound.allocation_offset);
    let CallSiteOwner::Operation(operation) = call.owner else {
        return Err(ObjectError::InvalidForeignCallArgument {
            caller,
            owner: call.owner,
        });
    };
    let call_end = call
        .offset
        .checked_add(4)
        .ok_or(ObjectError::InvalidForeignCallArgument {
            caller,
            owner: call.owner,
        })?;
    let attributed = semantic_code_attribution
        .iter()
        .filter(|row| row.site == SemanticCodeSite::Operation(operation))
        .filter(|row| {
            row.code_offset <= call.scalar_arguments[0].code_offset
                && row
                    .code_offset
                    .checked_add(row.byte_count)
                    .is_some_and(|end| end >= call_end)
        })
        .count();
    if attributed != 1 {
        return Err(ObjectError::InvalidForeignCallArgument {
            caller,
            owner: call.owner,
        });
    }

    for (parameter_index, ((argument, shape), expected_placement)) in call
        .scalar_arguments
        .iter()
        .zip(&shapes)
        .zip(&expected_plan.parameters)
        .enumerate()
    {
        let [
            omega_calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: placed_bytes,
            },
        ] = argument.placement.locations.as_slice()
        else {
            return Err(ObjectError::InvalidForeignCallArgument {
                caller,
                owner: call.owner,
            });
        };
        if argument.parameter_index != parameter_index as u32
            || argument.placement != *expected_placement
            || argument.placement.shape != *shape
            || *placed_bytes != shape.byte_size
        {
            return Err(ObjectError::InvalidForeignCallArgument {
                caller,
                owner: call.owner,
            });
        }
        let register_number = match (target.architecture, register) {
            (Architecture::X86_64, omega_calling_conventions::MachineRegister::X86Rdi) => 7,
            (Architecture::X86_64, omega_calling_conventions::MachineRegister::X86Rsi) => 6,
            (Architecture::X86_64, omega_calling_conventions::MachineRegister::X86Rdx) => 2,
            (Architecture::X86_64, omega_calling_conventions::MachineRegister::X86Rcx) => 1,
            (Architecture::X86_64, omega_calling_conventions::MachineRegister::X86R8) => 8,
            (Architecture::X86_64, omega_calling_conventions::MachineRegister::X86R9) => 9,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(0)) => 0,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(1)) => 1,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(2)) => 2,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(3)) => 3,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(4)) => 4,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(5)) => 5,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(6)) => 6,
            (Architecture::Aarch64, omega_calling_conventions::MachineRegister::Aarch64X(7)) => 7,
            _ => {
                return Err(ObjectError::InvalidForeignCallArgument {
                    caller,
                    owner: call.owner,
                });
            }
        };
        let bits = argument.scalar_type.bits();
        let mask = if bits == 64 {
            u64::MAX
        } else {
            (1_u64 << bits) - 1
        };
        let value_bits = match (argument.scalar_type.sign(), argument.immediate) {
            (psi_core::IntegerSign::Signed, psi_core::IntegerValue::Signed(value)) => {
                value as u128 as u64
            }
            (psi_core::IntegerSign::Unsigned, psi_core::IntegerValue::Unsigned(value)) => {
                value as u64
            }
            _ => {
                return Err(ObjectError::InvalidForeignCallArgument {
                    caller,
                    owner: call.owner,
                });
            }
        } & mask;
        let mut expected_bytes = Vec::new();
        match target.architecture {
            Architecture::X86_64 if bits <= 32 => {
                if register_number >= 8 {
                    expected_bytes.push(0x41);
                }
                expected_bytes.push(0xb8 | (register_number & 7));
                expected_bytes.extend_from_slice(&(value_bits as u32).to_le_bytes());
            }
            Architecture::X86_64 => {
                expected_bytes.extend_from_slice(&[
                    0x48 | ((register_number >> 3) & 1),
                    0xb8 | (register_number & 7),
                ]);
                expected_bytes.extend_from_slice(&value_bits.to_le_bytes());
            }
            Architecture::Aarch64 => {
                for chunk in 0..4 {
                    let immediate = ((value_bits >> (chunk * 16)) & 0xffff) as u32;
                    if chunk == 0 || immediate != 0 {
                        let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
                        let instruction = base
                            | ((chunk as u32) << 21)
                            | (immediate << 5)
                            | u32::from(register_number);
                        expected_bytes.extend_from_slice(&instruction.to_le_bytes());
                    }
                }
            }
        }
        let argument_end = argument
            .code_offset
            .checked_add(argument.byte_count)
            .ok_or(ObjectError::InvalidForeignCallArgument {
                caller,
                owner: call.owner,
            })?;
        let next_interval = call
            .scalar_arguments
            .get(parameter_index + 1)
            .map_or(next_interval, |next| next.code_offset);
        if argument.byte_count != expected_bytes.len()
            || bytes.get(argument.code_offset..argument_end) != Some(expected_bytes.as_slice())
            || argument_end != next_interval
        {
            return Err(ObjectError::InvalidForeignCallArgument {
                caller,
                owner: call.owner,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectError {
    EmptyPlan,
    NonCanonicalFunctionOrder {
        previous: MachineId,
        current: MachineId,
    },
    EmptyFunction(MachineId),
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
    MissingX86ScalarFmaCustody {
        machine: MachineId,
        offset: usize,
    },
    InvalidRankedCountdown(MachineId),
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
        operation: psi_core::OperationId,
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
        operation: Option<psi_core::OperationId>,
    },
    UnaccountedTerminalStack(MachineId),
    TerminalStackCycle(MachineId),
    TerminalStackCompositionOverflow {
        caller: MachineId,
        owner: CallSiteOwner,
    },
    PortEffectOutsideFunction {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    PortEffectOperationNotInProvenance {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    DuplicatePortEffectOperation {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    PortEffectBytesMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    BoundarySettlementOutsideFunction {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    BoundarySettlementOperationNotInProvenance {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    DuplicateBoundarySettlementOperation {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    InvalidBoundarySettlementArgumentPath {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    InvalidCompletionReceiptArgumentIndex {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    InvalidCompletionReceiptCustody {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    InvalidCompletionProviderCustody {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
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
