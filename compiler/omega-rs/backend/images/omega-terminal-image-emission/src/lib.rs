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

mod final_image_validation;
mod image_output;
mod installation;
mod instruction_loads;
mod partial_cleanup_partition;
mod scalar_call_stack;
mod scalar_cleanup_preservation;
mod scalar_conditional_call_paths;
mod scalar_conditional_regions;
mod scalar_control_cleanup;
mod scalar_stack_mutation;
mod stack_demand;
mod structural_condition_layout;
mod structural_condition_read;
mod structural_return;
mod unit_call_custody;
mod unit_stack;

pub use image_output::{
    TerminalExecutableImage, TerminalObjectContainer, can_emit_terminal_executable_image,
    emit_terminal_executable_image, emit_terminal_object_container,
};
pub use installation::*;
pub(crate) use partial_cleanup_partition::exact_partial_cleanup_partition;
pub use stack_demand::{derive_terminal_stack_demand, derive_terminal_unit_stack_demand};

use scalar_call_stack::validate_scalar_call_stack;
use scalar_cleanup_preservation::validate_scalar_cleanup_preservation;
use scalar_conditional_call_paths::{conditional_call_path, conditional_paths_are_exclusive};
use scalar_conditional_regions::{
    collect_conditional_tree_regions, division_branches_in_region, validate_division_branch_regions,
};
use scalar_control_cleanup::{cleanup_for_owner, validate_scalar_control_cleanup_evidence};
use scalar_stack_mutation::{
    aarch64_control_flow_instruction, aarch64_unsupported_sp_write, replay_scalar_mutation,
    validate_aarch64_scalar_mutation, validate_x86_scalar_mutation,
};
use structural_condition_layout::replay_boolean_field_offset;
use structural_condition_read::{
    condition_stack_depth_before, replay_aarch64_boolean_structural_read,
    replay_x86_boolean_structural_read,
};
use structural_return::validate_structural_return_record;
use unit_call_custody::{expected_projected_copy_bytes, validate_internal_unit_call_custody};
use unit_stack::{
    aarch64_stack_adjustment_at, validate_complete_unit_stack_evidence, validate_unit_call_stack,
    validate_unit_function_stack,
};

use omega_calling_conventions::ValueLocation;
use omega_object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
    entry_symbol_name,
};
use omega_target::{Architecture, NativeTarget};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalMachineCodePlan, TerminalNativeFuelAttribution,
    TerminalNativeFuelSite, TerminalPortEffectRecord, TerminalScalarConditionalBranchEvidence,
    TerminalScalarConditionalCondition, TerminalScalarControlAffineCleanupRecord,
    TerminalScalarControlFlowEvidence, TerminalScalarDivisionBranchEvidence,
    TerminalScalarJoinBranchEvidence, TerminalScalarStackEvidence, TerminalScalarStackMutation,
    TerminalStructuralReturnRecord,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalCallSiteOwner, TerminalPsiProvenance,
};
use psi_core::MachineId;
use psi_terminal::StructuralPathSegment;
use psi_terminal::TerminalPsiIdentity;
use psi_terminal_fuel::TerminalFuelSchedule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectArtifact {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    object: ObjectPlan,
    relocations: RelocationPlan,
    text_bytes: Vec<u8>,
    functions: Vec<TerminalObjectFunction>,
    fuel_attribution: Vec<TerminalObjectFuelAttribution>,
    port_effects: Vec<TerminalObjectPortEffect>,
    boundary_settlements: Vec<TerminalObjectBoundarySettlement>,
}

impl TerminalObjectArtifact {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
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

    pub fn functions(&self) -> &[TerminalObjectFunction] {
        &self.functions
    }

    pub fn entry_function(&self) -> &TerminalObjectFunction {
        self.functions
            .iter()
            .find(|function| function.machine == self.entry)
            .expect("artifact construction requires one entry function")
    }

    pub fn boundary_settlements(&self) -> &[TerminalObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn port_effects(&self) -> &[TerminalObjectPortEffect] {
        &self.port_effects
    }

    pub fn fuel_attribution(&self) -> &[TerminalObjectFuelAttribution] {
        &self.fuel_attribution
    }
}

impl omega_terminal_installation_evidence::TerminalObjectEvidence for TerminalObjectArtifact {
    fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    fn architecture(&self) -> Architecture {
        self.target.architecture
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
pub struct TerminalObjectFunction {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub symbol: ObjectSymbolHandle,
    pub text_offset: usize,
    pub byte_count: usize,
    /// Byte-validated stack facts for a completely accounted Unit body.
    pub unit_stack: Option<TerminalObjectUnitStack>,
    /// Byte-validated stack facts for a branch-free scalar body.
    pub scalar_stack: Option<TerminalObjectScalarStack>,
    pub unit_call_stacks: Vec<TerminalObjectUnitCallStack>,
    pub scalar_call_stacks: Vec<TerminalObjectScalarCallStack>,
    pub internal_unit_calls: Vec<omega_terminal_machine_code::TerminalInternalUnitCallRecord>,
    pub unit_parameters: Vec<omega_terminal_machine_code::TerminalUnitParameterRecord>,
    pub unit_parameter_homes: Vec<omega_terminal_machine_code::TerminalUnitParameterHomeRecord>,
    pub unit_affine_cleanup: Option<omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
    pub scalar_affine_cleanup: Option<omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
    /// Three byte-validated scalar cleanup records in canonical physical/DFS
    /// return-leaf order for the bounded two-decision Boolean control lane.
    pub scalar_control_affine_cleanups: Vec<TerminalScalarControlAffineCleanupRecord>,
    pub scalar_structural_parameters: Vec<omega_terminal_machine_code::TerminalUnitParameterRecord>,
    pub scalar_structural_parameter_homes:
        Vec<omega_terminal_machine_code::TerminalUnitParameterHomeRecord>,
    /// Byte-validated structural custody returned by this function, when the
    /// complete one-fragment slice applies.
    pub structural_return: Option<TerminalStructuralReturnRecord>,
}

impl TerminalObjectFunction {
    pub fn bytes<'artifact>(&self, artifact: &'artifact TerminalObjectArtifact) -> &'artifact [u8] {
        &artifact.text_bytes[self.text_offset..self.text_offset + self.byte_count]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalObjectUnitCallStack {
    pub owner: TerminalCallSiteOwner,
    pub target: MachineId,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
    pub active_frame_bytes: u32,
    pub transient_bytes: u32,
    pub caller_live_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalObjectScalarCallStack {
    pub owner: TerminalCallSiteOwner,
    pub target: MachineId,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
    pub caller_live_bytes: u32,
}

/// Stack quantities recomputed by object construction from exact validated
/// target instructions. No producer-supplied numeric peak crosses this
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalObjectUnitStack {
    pub frame_bytes: u32,
    pub local_peak_bytes: u32,
    pub stack_alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalObjectScalarStack {
    pub local_peak_bytes: u32,
    pub stack_alignment: u32,
}

/// Recomputed stack demand for the accounted terminal function slices. This
/// excludes the external entry adapter/interrupt arrival frame, which belongs
/// to installed-root realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStackDemand {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    entry: MachineId,
    ceiling_bytes: u64,
    stack_alignment: u32,
    contributing_machines: std::collections::BTreeSet<MachineId>,
}

impl TerminalStackDemand {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
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

impl omega_terminal_installation_evidence::TerminalStackDemandEvidence for TerminalStackDemand {
    fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
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
/// callers should use [`TerminalStackDemand`] and [`derive_terminal_stack_demand`].
pub type TerminalUnitStackDemand = TerminalStackDemand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectBoundarySettlement {
    pub machine: MachineId,
    pub settlement: TerminalBoundarySettlementRecord,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectPortEffect {
    pub machine: MachineId,
    pub effect: TerminalPortEffectRecord,
    /// Absolute offset in the object `.text` section.
    pub text_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectFuelAttribution {
    pub machine: MachineId,
    pub attribution: TerminalNativeFuelAttribution,
    pub text_offset: usize,
}

/// Construct a self-contained object plan and exact text carrier.
///
/// Function order is semantic-artifact order and must already be canonical by
/// `MachineId`; this boundary rejects alternate ordering rather than silently
/// normalizing it. Each function gets exactly one symbol and one retained Psi
/// provenance row.
pub fn build_terminal_object_artifact(
    plan: &TerminalMachineCodePlan,
) -> Result<TerminalObjectArtifact, TerminalObjectError> {
    if plan.functions.is_empty() {
        return Err(TerminalObjectError::EmptyPlan);
    }
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
            return Err(TerminalObjectError::NonCanonicalFunctionOrder {
                previous,
                current: function.machine,
            });
        }
        if function.bytes.is_empty() {
            return Err(TerminalObjectError::EmptyFunction(function.machine));
        }
        if function
            .internal_calls
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
        {
            return Err(TerminalObjectError::NonCanonicalInternalCallOrder(
                function.machine,
            ));
        }
        if (function.unit_affine_cleanup.is_some()
            && (function.scalar_affine_cleanup.is_some()
                || !function.scalar_control_affine_cleanups.is_empty()))
            || (function.scalar_affine_cleanup.is_some()
                && !function.scalar_control_affine_cleanups.is_empty())
        {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
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
            return Err(TerminalObjectError::ConflictingTerminalStackEvidence(
                function.machine,
            ));
        }
        if let Some(returned) = &function.structural_return {
            validate_structural_return_record(
                plan.target,
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.fuel_attribution,
                returned,
            )?;
            if function.unit_stack.is_some()
                || function.scalar_stack.is_some()
                || !function.internal_calls.is_empty()
                || !function.port_effects.is_empty()
                || !function.boundary_settlements.is_empty()
            {
                return Err(TerminalObjectError::StructuralReturnEvidenceConflict(
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
        let mut call_owner_paths = std::collections::BTreeMap::<
            TerminalCallSiteOwner,
            Vec<Option<Vec<(usize, bool)>>>,
        >::new();
        for call in &function.internal_calls {
            let owner_in_provenance = match call.owner {
                TerminalCallSiteOwner::Operation(operation) => {
                    function.provenance.operations.contains(&operation)
                }
                TerminalCallSiteOwner::CleanupAction { edge, .. } => {
                    function.provenance.edges.contains(&edge)
                }
            };
            if !owner_in_provenance {
                return Err(TerminalObjectError::InternalCallOperationNotInProvenance {
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
                && (!matches!(call.owner, TerminalCallSiteOwner::Operation(_))
                    || path.as_ref().is_none_or(|path| {
                        prior_paths.iter().any(|prior| {
                            prior
                                .as_ref()
                                .is_none_or(|prior| !conditional_paths_are_exclusive(prior, path))
                        })
                    }))
            {
                return Err(TerminalObjectError::DuplicateInternalCallOperation {
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
                    return Err(TerminalObjectError::MissingUnitCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, Some(_)) => {
                    return Err(TerminalObjectError::UnexpectedUnitCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, None) => {}
            }
            match (function.scalar_stack.as_ref(), call.scalar_stack) {
                (Some(_), Some(_)) => {}
                (Some(_), None) => {
                    return Err(TerminalObjectError::MissingScalarCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, Some(_)) => {
                    return Err(TerminalObjectError::UnexpectedScalarCallStackEvidence {
                        caller: function.machine,
                        owner: call.owner,
                    });
                }
                (None, None) => {}
            }
        }
        let is_unit_custody_relocation =
            |call: &&omega_terminal_machine_code::TerminalInternalCallRelocation| {
                call.unit_stack.is_some()
                    || ((function.scalar_affine_cleanup.is_some()
                        || cleanup_for_owner(&function.scalar_control_affine_cleanups, call.owner)
                            .is_some())
                        && matches!(call.owner, TerminalCallSiteOwner::CleanupAction { .. })
                        && call.scalar_stack.is_some())
            };
        if function.internal_unit_calls.len()
            != function
                .internal_calls
                .iter()
                .filter(is_unit_custody_relocation)
                .count()
        {
            return Err(TerminalObjectError::InvalidInternalUnitCallEvidence(
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
            .collect::<std::collections::BTreeSet<_>>();
        if custody_identities.len() != function.internal_unit_calls.len()
            || custody_identities != relocation_identities
        {
            return Err(TerminalObjectError::InvalidInternalUnitCallEvidence(
                function.machine,
            ));
        }
        let scalar_cleanup_custody = function.scalar_affine_cleanup.is_some()
            || !function.scalar_control_affine_cleanups.is_empty();
        let scalar_boundary_custody = function.boundary_settlements.iter().any(|settlement| {
            matches!(
                settlement.realization,
                TerminalBoundaryRealization::DirectPortReadU8(_)
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
            if custody.result.is_some() != target_returns_scalar {
                return Err(TerminalObjectError::InvalidInternalUnitCallEvidence(
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
                return Err(TerminalObjectError::InvalidInternalUnitCallEvidence(
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
                &function.fuel_attribution,
                &function.internal_calls,
                parameter_homes,
                validated_function_stack.as_ref(),
                unit_call_stack,
                scalar_call_stack,
                custody,
                affine_cleanup,
            )?;
        }
        match (&function.unit_stack, &function.unit_affine_cleanup) {
            (Some(_), Some(cleanup)) => validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.fuel_attribution,
                &function.unit_parameter_homes,
                &function.internal_unit_calls,
                &attachments,
                &machine_functions,
                cleanup,
                false,
            )?,
            (None, None) => {}
            _ => {
                return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
        }
        if let Some(cleanup) = &function.scalar_affine_cleanup {
            if function.unit_stack.is_some() || function.scalar_stack.is_none() {
                return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
            validate_unit_affine_cleanup(
                function.machine,
                &function.provenance,
                &function.bytes,
                &function.fuel_attribution,
                &function.scalar_structural_parameter_homes,
                &function.internal_unit_calls,
                &attachments,
                &machine_functions,
                cleanup,
                true,
            )?;
        }
        if !function.scalar_control_affine_cleanups.is_empty() {
            if function.unit_stack.is_some()
                || function.scalar_stack.is_none()
                || function.scalar_affine_cleanup.is_some()
            {
                return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                    function.machine,
                ));
            }
            for record in &function.scalar_control_affine_cleanups {
                let cleanup_end = record
                    .cleanup
                    .code_offset
                    .checked_add(record.cleanup.byte_count)
                    .ok_or(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                        function.machine,
                    ))?;
                validate_unit_affine_cleanup(
                    function.machine,
                    &function.provenance,
                    function.bytes.get(..cleanup_end).ok_or(
                        TerminalObjectError::InvalidUnitAffineCleanupEvidence(function.machine),
                    )?,
                    &function.fuel_attribution,
                    &function.scalar_structural_parameter_homes,
                    &function.internal_unit_calls,
                    &attachments,
                    &machine_functions,
                    &record.cleanup,
                    true,
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
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
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
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                function.machine,
            ));
        }
        if let Some(stack) = function.unit_stack {
            validate_complete_unit_stack_evidence(
                plan.target.architecture,
                function.machine,
                &function.bytes,
                stack,
                &function.internal_calls,
            )?;
        }
        if let Some(stack) = validated_function_stack {
            validated_unit_stacks.insert(function.machine, (stack, validated_call_stacks));
        }
        if function.fuel_attribution.windows(2).any(|pair| {
            (pair[0].operation_ordinal, pair[0].code_offset)
                >= (pair[1].operation_ordinal, pair[1].code_offset)
        }) {
            return Err(TerminalObjectError::NonCanonicalFuelAttributionOrder(
                function.machine,
            ));
        }
        let mut fuel_sites = std::collections::BTreeSet::new();
        for attribution in &function.fuel_attribution {
            let end = attribution
                .code_offset
                .checked_add(attribution.byte_count)
                .ok_or(TerminalObjectError::FuelAttributionOutsideFunction(
                    function.machine,
                ))?;
            let known = match attribution.site {
                TerminalNativeFuelSite::Operation(operation) => {
                    function.provenance.operations.contains(&operation)
                }
                TerminalNativeFuelSite::Edge(edge) => function.provenance.edges.contains(&edge),
            };
            if attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
                || attribution.units == 0
                || end > function.bytes.len()
                || !known
                || !fuel_sites.insert(attribution.site)
            {
                return Err(TerminalObjectError::InvalidFuelAttribution(
                    function.machine,
                ));
            }
        }
        if function.port_effects.windows(2).any(|pair| {
            (pair[0].code_offset, pair[0].operation_ordinal)
                >= (pair[1].code_offset, pair[1].operation_ordinal)
        }) {
            return Err(TerminalObjectError::NonCanonicalPortEffectOrder(
                function.machine,
            ));
        }
        let mut port_operations = std::collections::BTreeSet::new();
        for effect in &function.port_effects {
            let end = effect.code_offset.checked_add(effect.byte_count).ok_or(
                TerminalObjectError::PortEffectOutsideFunction {
                    machine: function.machine,
                    operation: effect.psi_operation,
                },
            )?;
            if end > function.bytes.len() || effect.byte_count == 0 {
                return Err(TerminalObjectError::PortEffectOutsideFunction {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if !function
                .provenance
                .operations
                .contains(&effect.psi_operation)
            {
                return Err(TerminalObjectError::PortEffectOperationNotInProvenance {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if !port_operations.insert(effect.psi_operation) {
                return Err(TerminalObjectError::DuplicatePortEffectOperation {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
            if plan.target.architecture != Architecture::X86_64
                || function.bytes[effect.code_offset..end]
                    != omega_x86_encoding::encode_immediate_port_write(effect.port, effect.value)
            {
                return Err(TerminalObjectError::PortEffectBytesMismatch {
                    machine: function.machine,
                    operation: effect.psi_operation,
                });
            }
        }
        if function.boundary_settlements.windows(2).any(|pair| {
            (pair[0].code_offset, pair[0].operation_ordinal)
                >= (pair[1].code_offset, pair[1].operation_ordinal)
        }) {
            return Err(TerminalObjectError::NonCanonicalBoundarySettlementOrder(
                function.machine,
            ));
        }
        let mut settlement_operations = std::collections::BTreeSet::new();
        for settlement in &function.boundary_settlements {
            if settlement.code_offset > function.bytes.len() {
                return Err(TerminalObjectError::BoundarySettlementOutsideFunction {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if !function
                .provenance
                .operations
                .contains(&settlement.psi_operation)
            {
                return Err(
                    TerminalObjectError::BoundarySettlementOperationNotInProvenance {
                        machine: function.machine,
                        operation: settlement.psi_operation,
                    },
                );
            }
            if !settlement_operations.insert(settlement.psi_operation) {
                return Err(TerminalObjectError::DuplicateBoundarySettlementOperation {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if settlement.arguments.iter().any(|argument| {
                argument.path.iter().any(
                    |segment| matches!(segment, StructuralPathSegment::Field(identity) if identity.is_empty()),
                )
            }) {
                return Err(TerminalObjectError::InvalidBoundarySettlementArgumentPath {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            if settlement.completion_receipts.iter().any(|receipt| {
                usize::try_from(receipt.argument_index)
                    .map_or(true, |index| index >= settlement.arguments.len())
            }) {
                return Err(TerminalObjectError::InvalidCompletionReceiptArgumentIndex {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
            let valid_realization = match settlement.realization {
                TerminalBoundaryRealization::MetadataOnlyPort(realization) => {
                    settlement.byte_count == 0
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
                TerminalBoundaryRealization::DirectPortReadU8(realization) => {
                    let expected =
                        omega_x86_encoding::encode_immediate_port_read_u8(realization.port);
                    settlement.byte_count == expected.len()
                        && plan.target.architecture == Architecture::X86_64
                        && settlement
                            .code_offset
                            .checked_add(settlement.byte_count)
                            .and_then(|end| function.bytes.get(settlement.code_offset..end))
                            == Some(expected.as_slice())
                        && function.unit_stack.is_none()
                        && function.scalar_stack.is_some()
                        && settlement.arguments.iter().all(|argument| {
                            argument.path.is_empty()
                                && function
                                    .scalar_structural_parameters
                                    .iter()
                                    .any(|parameter| parameter.place == argument.place)
                        })
                }
            };
            if !valid_realization {
                return Err(TerminalObjectError::BoundaryRealizationMismatch {
                    machine: function.machine,
                    operation: settlement.psi_operation,
                });
            }
        }
        previous = Some(function.machine);
        saw_entry |= function.machine == plan.entry;
        text_size = text_size
            .checked_add(function.bytes.len())
            .ok_or(TerminalObjectError::TextSizeOverflow)?;
    }
    if !saw_entry {
        return Err(TerminalObjectError::EntryFunctionMissing(plan.entry));
    }

    let mut object = ObjectPlan::with_capacity(plan.target, 1, plan.functions.len());
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });

    let mut text_bytes = Vec::with_capacity(text_size);
    let mut functions = Vec::with_capacity(plan.functions.len());
    let mut fuel_attribution = Vec::new();
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
        for attribution in &function.fuel_attribution {
            fuel_attribution.push(TerminalObjectFuelAttribution {
                machine: function.machine,
                attribution: *attribution,
                text_offset: text_offset
                    .checked_add(attribution.code_offset)
                    .ok_or(TerminalObjectError::TextSizeOverflow)?,
            });
        }
        for effect in &function.port_effects {
            port_effects.push(TerminalObjectPortEffect {
                machine: function.machine,
                effect: effect.clone(),
                text_offset: text_offset
                    .checked_add(effect.code_offset)
                    .ok_or(TerminalObjectError::TextSizeOverflow)?,
            });
        }
        for settlement in &function.boundary_settlements {
            boundary_settlements.push(TerminalObjectBoundarySettlement {
                machine: function.machine,
                settlement: settlement.clone(),
                text_offset: text_offset
                    .checked_add(settlement.code_offset)
                    .ok_or(TerminalObjectError::TextSizeOverflow)?,
            });
        }
        let (unit_stack, mut unit_call_stacks) = validated_unit_stacks
            .remove(&function.machine)
            .map_or((None, Vec::new()), |(stack, calls)| (Some(stack), calls));
        for call in &mut unit_call_stacks {
            call.text_offset = text_offset
                .checked_add(call.text_offset)
                .ok_or(TerminalObjectError::TextSizeOverflow)?;
        }
        let (scalar_stack, mut scalar_call_stacks) = validated_scalar_stacks
            .remove(&function.machine)
            .map_or((None, Vec::new()), |(stack, calls)| (Some(stack), calls));
        for call in &mut scalar_call_stacks {
            call.text_offset = text_offset
                .checked_add(call.text_offset)
                .ok_or(TerminalObjectError::TextSizeOverflow)?;
        }
        functions.push(TerminalObjectFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: function.provenance.clone(),
            symbol,
            text_offset,
            byte_count: function.bytes.len(),
            unit_stack,
            scalar_stack,
            unit_call_stacks,
            scalar_call_stacks,
            internal_unit_calls: function.internal_unit_calls.clone(),
            unit_parameters: function.unit_parameters.clone(),
            unit_parameter_homes: function.unit_parameter_homes.clone(),
            unit_affine_cleanup: function.unit_affine_cleanup.clone(),
            scalar_affine_cleanup: function.scalar_affine_cleanup.clone(),
            scalar_control_affine_cleanups: function.scalar_control_affine_cleanups.clone(),
            scalar_structural_parameters: function.scalar_structural_parameters.clone(),
            scalar_structural_parameter_homes: function.scalar_structural_parameter_homes.clone(),
            structural_return: function.structural_return.clone(),
        });
    }

    let mut relocations = RelocationPlan::with_record_capacity(
        plan.target,
        plan.functions
            .iter()
            .map(|function| function.internal_calls.len())
            .sum(),
    );
    for (function, emitted) in plan.functions.iter().zip(&functions) {
        for call in &function.internal_calls {
            let target_symbol = symbols_by_machine.get(&call.target).copied().ok_or(
                TerminalObjectError::UnknownInternalCallTarget {
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
                .ok_or(TerminalObjectError::TextSizeOverflow)?;
            let origin = match call.owner {
                TerminalCallSiteOwner::Operation(operation) => {
                    RelocationOrigin::SemanticOperation {
                        function_symbol_handle: emitted.symbol,
                        operation_identity: operation.get(),
                    }
                }
                TerminalCallSiteOwner::CleanupAction { edge, .. } => {
                    RelocationOrigin::SemanticEdge {
                        function_symbol_handle: emitted.symbol,
                        edge_identity: edge.get(),
                    }
                }
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

    Ok(TerminalObjectArtifact {
        terminal_psi: plan.terminal_psi,
        target: plan.target,
        entry: plan.entry,
        object,
        relocations,
        text_bytes,
        functions,
        fuel_attribution,
        port_effects,
        boundary_settlements,
    })
}

fn validate_unit_affine_cleanup(
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    fuel: &[TerminalNativeFuelAttribution],
    parameter_homes: &[omega_terminal_machine_code::TerminalUnitParameterHomeRecord],
    internal_unit_calls: &[omega_terminal_machine_code::TerminalInternalUnitCallRecord],
    attachments: &std::collections::BTreeMap<MachineId, Option<psi_core::StructuralTypeId>>,
    functions: &std::collections::BTreeMap<
        MachineId,
        &omega_terminal_machine_code::TerminalMachineCodeFunction,
    >,
    cleanup: &omega_terminal_machine_code::TerminalUnitAffineCleanupRecord,
    allow_mixed_nominal_roots: bool,
) -> Result<(), TerminalObjectError> {
    let invalid = || TerminalObjectError::InvalidUnitAffineCleanupEvidence(machine);
    let end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or_else(invalid)?;
    let local_places = cleanup
        .locals
        .iter()
        .map(|(_, place, _)| place.id)
        .collect::<Vec<_>>();
    let expected_local_prefix = local_places.iter().rev().copied().collect::<Vec<_>>();
    let transferred_roots = internal_unit_calls
        .iter()
        .flat_map(|call| &call.arguments)
        .filter(|argument| argument.path.is_empty())
        .map(|argument| argument.place)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_parameter_suffix = parameter_homes
        .iter()
        .rev()
        .filter(|home| {
            home.multiplicity == psi_terminal::StructuralMultiplicity::Affine
                && !transferred_roots.contains(&home.place)
        })
        .map(|home| home.place)
        .collect::<Vec<_>>();
    let local_operations = cleanup
        .locals
        .iter()
        .map(|(operation, _, _)| *operation)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_root_actions = expected_local_prefix
        .iter()
        .copied()
        .chain(expected_parameter_suffix.iter().copied())
        .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
        .collect::<Vec<_>>();
    let expected_local_actions = expected_local_prefix
        .iter()
        .copied()
        .map(psi_terminal::TerminalAffineCleanupAction::DiscardRoot)
        .collect::<Vec<_>>();
    let exact_nominal_target = |nominal: &psi_terminal::NominalAffineCleanup| {
        if nominal.cleanup_receiver.is_some() || !nominal.requirement_obligations.is_empty() {
            return (None, false);
        }
        let cleanup_function = functions.get(&nominal.cleanup_machine).copied();
        let cleanup_body_is_exact = cleanup_function.is_some_and(|function| {
            let calls = &function.internal_unit_calls;
            let call_owners = calls
                .iter()
                .map(|call| call.owner)
                .collect::<std::collections::BTreeSet<_>>();
            let call_targets = calls
                .iter()
                .map(|call| call.target)
                .collect::<std::collections::BTreeSet<_>>();
            function.attachment == Some(nominal.structural_type)
                && function.unit_stack.is_some()
                && function.scalar_stack.is_none()
                && function.unit_parameters.is_empty()
                && function.unit_parameter_homes.is_empty()
                && function
                    .unit_affine_cleanup
                    .as_ref()
                    .is_some_and(|return_cleanup| {
                        return_cleanup.locals.is_empty() && return_cleanup.actions.is_empty()
                    })
                && call_owners.len() == calls.len()
                && call_targets.len() == calls.len()
                && calls.iter().enumerate().all(|(ordinal, call)| {
                    matches!(call.owner, TerminalCallSiteOwner::Operation(operation)
                        if function.provenance.operations.get(ordinal) == Some(&operation))
                        && call.operation_ordinal == ordinal
                        && call.result.is_none()
                        && call.arguments.is_empty()
                        && call.claim_transfers.is_empty()
                        && functions.get(&call.target).is_some_and(|helper| {
                            helper.attachment.is_some()
                                && helper.unit_stack.is_some()
                                && helper.scalar_stack.is_none()
                                && helper.unit_parameters.is_empty()
                                && helper.unit_parameter_homes.is_empty()
                                && helper.internal_unit_calls.is_empty()
                                && helper.unit_affine_cleanup.as_ref().is_some_and(
                                    |return_cleanup| {
                                        return_cleanup.locals.is_empty()
                                            && return_cleanup.actions.is_empty()
                                    },
                                )
                        })
                })
                && calls.windows(2).all(|pair| {
                    pair[0]
                        .code_offset
                        .checked_add(pair[0].byte_count)
                        .is_some_and(|end| end <= pair[1].code_offset)
                })
        });
        (cleanup_function, cleanup_body_is_exact)
    };
    let action_shape_invalid = if cleanup.actions == expected_root_actions {
        cleanup
            .actions
            .iter()
            .filter_map(|action| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => Some(*place),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != cleanup.actions.len()
    } else if matches!(
        cleanup.actions.get(expected_local_actions.len()),
        Some(psi_terminal::TerminalAffineCleanupAction::DiscardResidual(
            _
        ))
    ) {
        let residual_actions = &cleanup.actions[expected_local_actions.len()..];
        let residuals = residual_actions
            .iter()
            .filter_map(|action| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual) => {
                    Some(residual)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let residual_root = residuals.first().map(|residual| residual.place);
        let parameter_type = residual_root.and_then(|place| {
            parameter_homes
                .iter()
                .find(|parameter| parameter.place == place)
                .map(|parameter| parameter.structural_type)
        });
        let moved = internal_unit_calls
            .iter()
            .flat_map(|call| &call.arguments)
            .filter(|argument| {
                Some(argument.place) == residual_root
                    && Some(argument.root_structural_type) == parameter_type
            })
            .map(|argument| (argument.path.as_slice(), argument.structural_type))
            .collect::<Vec<_>>();
        cleanup.actions[..expected_local_actions.len()] != expected_local_actions
            || residuals.len() != residual_actions.len()
            || residuals.is_empty()
            || residual_root.is_none_or(|root| expected_parameter_suffix.as_slice() != [root])
            || parameter_type.is_none()
            || residuals.iter().any(|residual| {
                Some(residual.place) != residual_root
                    || residual.path.is_empty()
                    || residual.path.iter().any(|segment| {
                        !matches!(segment,
                            psi_terminal::StructuralPathSegment::Field(identity)
                                if !identity.is_empty())
                    })
                    || parameter_type == Some(residual.structural_type)
            })
            || residuals.iter().enumerate().any(|(index, residual)| {
                residuals[..index].iter().any(|earlier| {
                    residual.path.starts_with(&earlier.path)
                        || earlier.path.starts_with(&residual.path)
                })
            })
            || moved.is_empty()
            || moved.iter().any(|(path, _)| {
                path.is_empty()
                    || path.iter().any(|segment| {
                        !matches!(segment,
                            psi_terminal::StructuralPathSegment::Field(identity)
                                if !identity.is_empty())
                    })
                    || residuals.iter().any(|residual| {
                        path.starts_with(&residual.path) || residual.path.starts_with(path)
                    })
            })
            || moved.iter().enumerate().any(|(index, (path, _))| {
                moved[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
            })
            || parameter_type.is_none_or(|root_type| {
                !exact_partial_cleanup_partition(
                    &cleanup.structural_types,
                    root_type,
                    &moved,
                    &residuals,
                )
            })
    } else {
        let nominal = cleanup
            .actions
            .iter()
            .enumerate()
            .filter_map(|(ordinal, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    Some((ordinal, cleanup))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if nominal.is_empty()
            || (!allow_mixed_nominal_roots && nominal.len() != cleanup.actions.len())
            || !cleanup.locals.is_empty()
            || parameter_homes.len() != cleanup.actions.len()
            || parameter_homes
                .iter()
                .rev()
                .zip(&cleanup.actions)
                .any(|(home, action)| match action {
                    psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                        *place != home.place
                            || home.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                    }
                    psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                        home.place != nominal.place
                            || home.structural_type != nominal.structural_type
                            || home.multiplicity != psi_terminal::StructuralMultiplicity::Affine
                            || !bounded_nominal_receiver_shape(home.shape)
                            || (home.shape.byte_size == 0 && !home.source.locations.is_empty())
                            || (home.shape.byte_size != 0 && home.source.locations.is_empty())
                            || attachments.get(&nominal.cleanup_machine)
                                != Some(&Some(nominal.structural_type))
                    }
                    psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
                })
        {
            true
        } else {
            let targets = nominal
                .iter()
                .map(|(_, nominal)| exact_nominal_target(nominal))
                .collect::<Vec<_>>();
            let executable_ordinals = targets
                .iter()
                .zip(&nominal)
                .filter_map(|((function, _), (action_ordinal, _))| {
                    function
                        .is_some_and(|function| !function.internal_unit_calls.is_empty())
                        .then_some(*action_ordinal)
                })
                .collect::<Vec<_>>();
            let cleanup_calls = internal_unit_calls
                .iter()
                .filter(|call| {
                    matches!(
                        call.owner,
                        TerminalCallSiteOwner::CleanupAction { edge, .. }
                            if edge == cleanup.psi_edge
                    )
                })
                .collect::<Vec<_>>();
            let ordered_executable_spans = executable_ordinals
                .iter()
                .map(|ordinal| {
                    let action_ordinal = u32::try_from(*ordinal).ok()?;
                    let nominal =
                        cleanup
                            .actions
                            .get(*ordinal)
                            .and_then(|action| match action {
                                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                                    nominal,
                                ) => Some(nominal),
                                _ => None,
                            })?;
                    let call = cleanup_calls.iter().find(|call| {
                        call.owner
                            == TerminalCallSiteOwner::CleanupAction {
                                edge: cleanup.psi_edge,
                                action_ordinal,
                            }
                            && call.target == nominal.cleanup_machine
                    })?;
                    Some((
                        call.code_offset,
                        call.code_offset.checked_add(call.byte_count)?,
                    ))
                })
                .collect::<Option<Vec<_>>>();
            targets.iter().any(|(_, body_exact)| !body_exact)
                || cleanup_calls.len() != executable_ordinals.len()
                || ordered_executable_spans.is_none_or(|spans| {
                    spans
                        .windows(2)
                        .any(|pair| pair[0].0 >= pair[1].0 || pair[0].1 > pair[1].0)
                })
                || executable_ordinals.iter().any(|ordinal| {
                    let Ok(action_ordinal) = u32::try_from(*ordinal) else {
                        return true;
                    };
                    let Some(psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
                        cleanup.actions.get(*ordinal)
                    else {
                        return true;
                    };
                    cleanup_calls
                        .iter()
                        .filter(|call| {
                            call.owner
                                == TerminalCallSiteOwner::CleanupAction {
                                    edge: cleanup.psi_edge,
                                    action_ordinal,
                                }
                                && call.target == nominal.cleanup_machine
                                && call.arguments.is_empty()
                                && call.claim_transfers.is_empty()
                                && call.code_offset >= cleanup.code_offset
                                && call
                                    .code_offset
                                    .checked_add(call.byte_count)
                                    .is_some_and(|call_end| call_end <= end)
                        })
                        .count()
                        != 1
                })
        }
    };
    if cleanup.byte_count == 0
        || end != bytes.len()
        || !provenance.edges.contains(&cleanup.psi_edge)
        || local_operations.len() != cleanup.locals.len()
        || cleanup.locals.iter().enumerate().any(
            |(ordinal, (operation, place, structural_type))| {
                !provenance.operations.contains(operation)
                    || !matches!(
                        place.kind,
                        psi_core::StructuralPlaceKind::TrivialAffineLocal {
                            declaration_ordinal,
                            structural_type: local_type,
                        } if usize::try_from(declaration_ordinal) == Ok(ordinal)
                            && local_type == structural_type.id
                    )
                    || !matches!(
                        structural_type.shape,
                        psi_terminal::StructuralTypeShape::Record { ref fields }
                            if fields.is_empty()
                    )
                    || fuel
                        .iter()
                        .filter(|attribution| {
                            attribution.site == TerminalNativeFuelSite::Operation(*operation)
                                && attribution.byte_count == 0
                        })
                        .count()
                        != 1
            },
        )
        || action_shape_invalid
        || fuel
            .iter()
            .filter(|attribution| {
                attribution.site == TerminalNativeFuelSite::Edge(cleanup.psi_edge)
                    && attribution.code_offset == cleanup.code_offset
                    && attribution.byte_count == cleanup.byte_count
            })
            .count()
            != 1
    {
        return Err(invalid());
    }
    Ok(())
}

fn bounded_nominal_receiver_shape(shape: omega_calling_conventions::ValueShape) -> bool {
    shape == omega_calling_conventions::ValueShape::integer(0, 1)
        || shape.class == omega_calling_conventions::ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size % shape.alignment == 0
}

#[allow(clippy::too_many_arguments)]
fn validate_boolean_shared_convergence_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    decisions: &[TerminalScalarConditionalBranchEvidence],
    joins: &[TerminalScalarJoinBranchEvidence],
    structural_conditions: &[omega_terminal_machine_code::TerminalBooleanStructuralConditionEvidence],
    merge_offset: usize,
    cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
    parameter_homes: &[omega_terminal_machine_code::TerminalUnitParameterHomeRecord],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    let invalid = || TerminalObjectError::InvalidScalarConditionalEvidence {
        machine,
        offset: decisions
            .first()
            .map_or(0, |decision| decision.branch_offset),
    };
    if decisions.is_empty()
        || joins.len() != decisions.len()
        || decisions
            .windows(2)
            .any(|pair| pair[0].branch_offset >= pair[1].branch_offset)
        || joins
            .windows(2)
            .any(|pair| pair[0].join_offset >= pair[1].join_offset)
        || merge_offset >= bytes.len()
        || evidence.cleanup_preservation.is_none()
        || evidence
            .mutations
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(invalid());
    }
    let shared_cleanup = cleanup.ok_or_else(invalid)?;
    let mut structural_types = std::collections::BTreeMap::new();
    if shared_cleanup.structural_types.is_empty()
        || shared_cleanup
            .structural_types
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        || shared_cleanup.structural_types.iter().any(|declaration| {
            structural_types
                .insert(declaration.id, declaration)
                .is_some()
        })
    {
        return Err(invalid());
    }
    let mut prefixes = Vec::with_capacity(decisions.len());
    let mut leaves = Vec::with_capacity(decisions.len() + 1);
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        0,
        merge_offset,
        decisions,
        &mut prefixes,
        &mut leaves,
    )?;
    if leaves.len() != decisions.len() + 1 {
        return Err(invalid());
    }
    let expression_prefixes = prefixes
        .iter()
        .filter(|(_, _, condition)| *condition == TerminalScalarConditionalCondition::Expression)
        .map(|(start, end, _)| (*start, *end))
        .collect::<std::collections::BTreeSet<_>>();
    let mut previous_end = None;
    let mut structural_identity = None;
    let mut operations = std::collections::BTreeSet::new();
    for condition in structural_conditions {
        let end = condition
            .code_offset
            .checked_add(condition.byte_count)
            .ok_or_else(invalid)?;
        if condition.reads.is_empty()
            || condition.byte_count == 0
            || condition.byte_count != condition.bytes.len()
            || end > merge_offset
            || !expression_prefixes.contains(&(condition.code_offset, end))
            || previous_end.is_some_and(|previous| previous > condition.code_offset)
            || bytes.get(condition.code_offset..end) != Some(condition.bytes.as_slice())
        {
            return Err(invalid());
        }
        previous_end = Some(end);
        let mut previous_read_end = None;
        for read in &condition.reads {
            let read_end = read
                .code_offset
                .checked_add(read.byte_count)
                .ok_or_else(invalid)?;
            let identity = (read.source, read.field, read.field_byte_offset);
            if structural_identity.is_some_and(|expected| expected != identity)
                || !operations.insert(read.psi_operation)
                || read.byte_count == 0
                || read.code_offset < condition.code_offset
                || read_end > end
                || previous_read_end.is_some_and(|previous| previous > read.code_offset)
            {
                return Err(invalid());
            }
            previous_read_end = Some(read_end);
            let mut homes = parameter_homes
                .iter()
                .filter(|home| home.place == read.source);
            let home = homes.next().ok_or_else(invalid)?;
            if homes.next().is_some()
                || home.byte_offset != 0
                || home.shape != home.source.shape
                || home.indirect
                    != matches!(
                        home.source.locations.as_slice(),
                        [ValueLocation::Indirect { .. }]
                    )
            {
                return Err(invalid());
            }
            let (canonical_offset, canonical_shape) =
                replay_boolean_field_offset(home.structural_type, read.field, &structural_types)
                    .ok_or_else(invalid)?;
            if read.field_byte_offset != canonical_offset || home.shape != canonical_shape {
                return Err(invalid());
            }
            let stack_depth =
                condition_stack_depth_before(evidence, condition.code_offset, read.code_offset)
                    .ok_or_else(invalid)?;
            let expected = match architecture {
                Architecture::X86_64 => {
                    replay_x86_boolean_structural_read(&home.source, canonical_offset, stack_depth)
                }
                Architecture::Aarch64 => replay_aarch64_boolean_structural_read(
                    &home.source,
                    canonical_offset,
                    stack_depth,
                ),
            }
            .ok_or_else(invalid)?;
            if expected.len() != read.byte_count
                || bytes.get(read.code_offset..read_end) != Some(expected.as_slice())
            {
                return Err(invalid());
            }
            structural_identity = Some(identity);
        }
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        validate_internal_call_site(architecture, machine, bytes, *call)?;
        let call_start = match architecture {
            Architecture::X86_64 => call.offset - 1,
            Architecture::Aarch64 => call.offset,
        };
        call_sites.insert(call_start, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut peak = 0;
    for (start, end, condition) in prefixes {
        let prefix_peak = replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            end,
            false,
            &mut claimed,
            &mut call_sites,
            condition == TerminalScalarConditionalCondition::Expression,
            evidence,
            &mut validated_calls,
            None,
        )?;
        if condition == TerminalScalarConditionalCondition::Parameter && prefix_peak != 0 {
            return Err(invalid());
        }
        peak = peak.max(prefix_peak);
    }
    for (index, (start, end)) in leaves.into_iter().enumerate() {
        let value_end = if let Some(join) = joins.get(index) {
            let join_end = join
                .join_offset
                .checked_add(join.join_byte_count)
                .ok_or_else(invalid)?;
            if join.join_offset < start || join_end != end {
                return Err(invalid());
            }
            match architecture {
                Architecture::X86_64 => {
                    let instruction = decode_exact_x86_instruction(
                        machine,
                        bytes,
                        join.join_offset,
                        join.join_byte_count,
                    )?;
                    if instruction.mnemonic() != iced_x86::Mnemonic::Jmp
                        || usize::try_from(instruction.near_branch_target()).ok()
                            != Some(merge_offset)
                    {
                        return Err(invalid());
                    }
                }
                Architecture::Aarch64 => {
                    if join.join_byte_count != 4 || !join.join_offset.is_multiple_of(4) {
                        return Err(invalid());
                    }
                    let encoded = u32::from_le_bytes(
                        bytes[join.join_offset..join_end]
                            .try_into()
                            .map_err(|_| invalid())?,
                    );
                    let words = merge_offset
                        .checked_sub(join.join_offset)
                        .filter(|distance| distance.is_multiple_of(4))
                        .map(|distance| distance / 4)
                        .and_then(|words| u32::try_from(words).ok())
                        .filter(|words| *words <= 0x01ff_ffff)
                        .ok_or_else(invalid)?;
                    if encoded != 0x1400_0000 | words {
                        return Err(invalid());
                    }
                }
            }
            join.join_offset
        } else {
            if index != decisions.len() || end != merge_offset {
                return Err(invalid());
            }
            end
        };
        peak = peak.max(replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            value_end,
            false,
            &mut claimed,
            &mut call_sites,
            false,
            evidence,
            &mut validated_calls,
            None,
        )?);
    }
    peak = peak.max(replay_scalar_conditional_region(
        architecture,
        machine,
        bytes,
        merge_offset,
        bytes.len(),
        true,
        &mut claimed,
        &mut call_sites,
        true,
        evidence,
        &mut validated_calls,
        cleanup,
    )?);
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((&offset, call)) = call_sites.first_key_value() {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset,
        });
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

fn validate_scalar_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    scalar_affine_cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
    scalar_control_affine_cleanups: &[TerminalScalarControlAffineCleanupRecord],
    scalar_structural_parameter_homes: &[omega_terminal_machine_code::TerminalUnitParameterHomeRecord],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    if evidence.stack_alignment != 16 {
        return Err(TerminalObjectError::InvalidScalarStackAlignment {
            machine,
            alignment: evidence.stack_alignment,
        });
    }
    if let TerminalScalarControlFlowEvidence::BooleanSharedConvergence {
        decisions,
        joins,
        structural_conditions,
        merge_offset,
    } = &evidence.control_flow
    {
        if scalar_affine_cleanup.is_none() || !scalar_control_affine_cleanups.is_empty() {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ));
        }
        return validate_boolean_shared_convergence_stack(
            architecture,
            machine,
            bytes,
            calls,
            evidence,
            decisions,
            joins,
            structural_conditions,
            *merge_offset,
            scalar_affine_cleanup,
            scalar_structural_parameter_homes,
        );
    }
    if let TerminalScalarControlFlowEvidence::ConditionalTree {
        decisions,
        crash_leaves,
        branches,
    } = &evidence.control_flow
    {
        if scalar_affine_cleanup.is_some()
            || crash_leaves.iter().any(|crash| *crash) && !scalar_control_affine_cleanups.is_empty()
        {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ));
        }
        return validate_conditional_tree_scalar_stack(
            architecture,
            machine,
            bytes,
            calls,
            evidence,
            decisions,
            crash_leaves,
            branches,
            scalar_control_affine_cleanups,
        );
    }
    if let TerminalScalarControlFlowEvidence::LinearWithDivisionBranches { ref branches } =
        evidence.control_flow
    {
        if scalar_affine_cleanup.is_some() || !scalar_control_affine_cleanups.is_empty() {
            return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ));
        }
        return validate_linear_scalar_division_stack(
            architecture,
            machine,
            bytes,
            calls,
            evidence,
            branches,
        );
    }
    if !scalar_control_affine_cleanups.is_empty() {
        return Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
            machine,
        ));
    }
    if evidence
        .mutations
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        validate_internal_call_site(architecture, machine, bytes, *call)?;
        let call_start = match architecture {
            Architecture::X86_64 => call.offset - 1,
            Architecture::Aarch64 => call.offset,
        };
        call_sites.insert(call_start, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    match architecture {
        Architecture::X86_64 => {
            let mut decoder =
                iced_x86::Decoder::with_ip(64, bytes, 0, iced_x86::DecoderOptions::NONE);
            let mut info_factory = iced_x86::InstructionInfoFactory::new();
            let mut saw_return = false;
            while decoder.can_decode() {
                let instruction = decoder.decode();
                let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
                if instruction.is_invalid() {
                    return Err(TerminalObjectError::InvalidScalarInstructionEncoding {
                        machine,
                        offset,
                    });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if offset.checked_add(instruction.len()) != Some(bytes.len()) || saw_return {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Call {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if instruction.flow_control() != iced_x86::FlowControl::Next {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                let stack_kind = match instruction.mnemonic() {
                    iced_x86::Mnemonic::Sub
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(true)
                    }
                    iced_x86::Mnemonic::Add
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(false)
                    }
                    iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop => Some(false),
                    _ => None,
                };
                if stack_kind.is_some() {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                    continue;
                }
                let info = info_factory.info(&instruction);
                if info.used_registers().iter().any(|register| {
                    matches!(
                        register.register(),
                        iced_x86::Register::RSP
                            | iced_x86::Register::ESP
                            | iced_x86::Register::SP
                            | iced_x86::Register::SPL
                    ) && matches!(
                        register.access(),
                        iced_x86::OpAccess::Write
                            | iced_x86::OpAccess::CondWrite
                            | iced_x86::OpAccess::ReadWrite
                            | iced_x86::OpAccess::ReadCondWrite
                    )
                }) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
            if !saw_return {
                return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
            }
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(TerminalObjectError::InvalidScalarInstructionEncoding {
                    machine,
                    offset: bytes.len() - bytes.len() % 4,
                });
            }
            let mut saw_return = false;
            for offset in (0..bytes.len()).step_by(4) {
                let encoded = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("four-byte AArch64 word"),
                );
                if encoded == 0xd65f_03c0 {
                    if offset + 4 != bytes.len() || saw_return {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if encoded == 0x9400_0000 {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if aarch64_control_flow_instruction(encoded) {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_aarch64_scalar_mutation(machine, encoded, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                } else if aarch64_unsupported_sp_write(encoded) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
            if !saw_return {
                return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
            }
        }
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((_, call)) = call_sites.first_key_value() {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset: call.offset,
        });
    }
    if depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

fn validate_linear_scalar_division_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    branches: &[TerminalScalarDivisionBranchEvidence],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    if architecture != Architecture::X86_64 || branches.is_empty() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence { machine, offset: 0 });
    }
    if evidence
        .mutations
        .windows(2)
        .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        validate_internal_call_site(architecture, machine, bytes, *call)?;
        call_sites.insert(call.offset - 1, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut cursor = 0;
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    for branch in branches {
        let branch_end = branch
            .branch_offset
            .checked_add(branch.branch_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            })?;
        let join_end = branch
            .join_offset
            .checked_add(branch.join_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.join_offset,
            })?;
        if cursor > branch.branch_offset
            || branch.branch_offset >= branch_end
            || branch_end > branch.join_offset
            || join_end != branch.ordinary_arm_offset
            || branch.ordinary_arm_offset >= branch.merge_offset
            || branch.merge_offset > bytes.len()
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        let conditional = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.branch_offset,
            branch.branch_byte_count,
        )?;
        let join = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.join_offset,
            branch.join_byte_count,
        )?;
        if conditional.mnemonic() != iced_x86::Mnemonic::Jne
            || conditional.flow_control() != iced_x86::FlowControl::ConditionalBranch
            || usize::try_from(conditional.near_branch_target()).ok()
                != Some(branch.ordinary_arm_offset)
            || join.mnemonic() != iced_x86::Mnemonic::Jmp
            || join.flow_control() != iced_x86::FlowControl::UnconditionalBranch
            || usize::try_from(join.near_branch_target()).ok() != Some(branch.merge_offset)
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            cursor,
            branch.branch_offset,
            false,
            &mut claimed,
            &mut call_sites,
            evidence,
            &mut validated_calls,
            &mut depth,
            &mut peak,
        )?;
        let branch_depth = depth;
        let mut special_depth = branch_depth;
        let mut special_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch_end,
            branch.join_offset,
            false,
            &mut claimed,
            &mut call_sites,
            evidence,
            &mut validated_calls,
            &mut special_depth,
            &mut special_peak,
        )?;
        let mut ordinary_depth = branch_depth;
        let mut ordinary_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch.ordinary_arm_offset,
            branch.merge_offset,
            false,
            &mut claimed,
            &mut call_sites,
            evidence,
            &mut validated_calls,
            &mut ordinary_depth,
            &mut ordinary_peak,
        )?;
        if special_depth != ordinary_depth {
            return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
        }
        depth = special_depth;
        peak = special_peak.max(ordinary_peak);
        cursor = branch.merge_offset;
    }
    replay_x86_scalar_linear_region(
        machine,
        bytes,
        cursor,
        bytes.len(),
        true,
        &mut claimed,
        &mut call_sites,
        evidence,
        &mut validated_calls,
        &mut depth,
        &mut peak,
    )?;
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((&offset, call)) = call_sites.first_key_value() {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset,
        });
    }
    if depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

fn decode_exact_x86_instruction(
    machine: MachineId,
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<iced_x86::Instruction, TerminalObjectError> {
    let end = offset
        .checked_add(byte_count)
        .filter(|end| *end <= bytes.len())
        .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence { machine, offset })?;
    let mut decoder = iced_x86::Decoder::with_ip(
        64,
        &bytes[offset..end],
        offset as u64,
        iced_x86::DecoderOptions::NONE,
    );
    let instruction = decoder.decode();
    if instruction.is_invalid() || instruction.len() != byte_count || decoder.can_decode() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence { machine, offset });
    }
    Ok(instruction)
}

#[allow(clippy::too_many_arguments)]
fn replay_x86_scalar_linear_region(
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
    depth: &mut u32,
    peak: &mut u32,
) -> Result<(), TerminalObjectError> {
    if start > end || end > bytes.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let mut decoder = iced_x86::Decoder::with_ip(
        64,
        &bytes[start..end],
        start as u64,
        iced_x86::DecoderOptions::NONE,
    );
    let mut info_factory = iced_x86::InstructionInfoFactory::new();
    let mut saw_return = false;
    while decoder.can_decode() {
        let instruction = decoder.decode();
        let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
        if instruction.is_invalid() {
            return Err(TerminalObjectError::InvalidScalarInstructionEncoding { machine, offset });
        }
        if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
            if !require_return || offset.checked_add(instruction.len()) != Some(end) || saw_return {
                return Err(TerminalObjectError::NonLinearScalarControlFlow { machine, offset });
            }
            saw_return = true;
            continue;
        }
        if instruction.mnemonic() == iced_x86::Mnemonic::Call {
            let call = call_sites
                .remove(&offset)
                .ok_or(TerminalObjectError::UntypedScalarInternalCall { machine, offset })?;
            let call_evidence =
                call.scalar_stack
                    .ok_or(TerminalObjectError::MissingScalarCallStackEvidence {
                        caller: machine,
                        owner: call.owner,
                    })?;
            let validated = validate_scalar_call_stack(
                Architecture::X86_64,
                machine,
                bytes,
                call,
                call_evidence,
                evidence,
                *depth,
                None,
            )?;
            *peak = (*peak).max(validated.caller_live_bytes);
            validated_calls.push(validated);
            continue;
        }
        if instruction.flow_control() != iced_x86::FlowControl::Next {
            return Err(TerminalObjectError::NonLinearScalarControlFlow { machine, offset });
        }
        let stack_mutation = matches!(
            instruction.mnemonic(),
            iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop
        ) || matches!(
            instruction.mnemonic(),
            iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub
        ) && instruction.op0_register() == iced_x86::Register::RSP
            || instruction.mnemonic() == iced_x86::Mnemonic::Lea
                && instruction.op0_register() == iced_x86::Register::RSP;
        if stack_mutation {
            let mutation = claimed
                .remove(&offset)
                .ok_or(TerminalObjectError::UnclaimedScalarStackMutation { machine, offset })?;
            validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
            replay_scalar_mutation(machine, offset, mutation.kind, depth, peak)?;
            continue;
        }
        let info = info_factory.info(&instruction);
        if info.used_registers().iter().any(|register| {
            matches!(
                register.register(),
                iced_x86::Register::RSP
                    | iced_x86::Register::ESP
                    | iced_x86::Register::SP
                    | iced_x86::Register::SPL
            ) && matches!(
                register.access(),
                iced_x86::OpAccess::Write
                    | iced_x86::OpAccess::CondWrite
                    | iced_x86::OpAccess::ReadWrite
                    | iced_x86::OpAccess::ReadCondWrite
            )
        }) {
            return Err(TerminalObjectError::UnsupportedScalarStackMutation { machine, offset });
        }
    }
    if require_return != saw_return {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_x86_scalar_division_region(
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    branches: &[TerminalScalarDivisionBranchEvidence],
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
) -> Result<u32, TerminalObjectError> {
    if branches.is_empty() || start > end || end > bytes.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let mut cursor = start;
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    for branch in branches {
        let branch_end = branch
            .branch_offset
            .checked_add(branch.branch_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            })?;
        let join_end = branch
            .join_offset
            .checked_add(branch.join_byte_count)
            .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.join_offset,
            })?;
        if cursor > branch.branch_offset
            || branch.branch_offset < start
            || branch.branch_offset >= branch_end
            || branch_end > branch.join_offset
            || join_end != branch.ordinary_arm_offset
            || branch.ordinary_arm_offset >= branch.merge_offset
            || branch.merge_offset > end
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        let conditional = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.branch_offset,
            branch.branch_byte_count,
        )?;
        let join = decode_exact_x86_instruction(
            machine,
            bytes,
            branch.join_offset,
            branch.join_byte_count,
        )?;
        if conditional.mnemonic() != iced_x86::Mnemonic::Jne
            || conditional.flow_control() != iced_x86::FlowControl::ConditionalBranch
            || usize::try_from(conditional.near_branch_target()).ok()
                != Some(branch.ordinary_arm_offset)
            || join.mnemonic() != iced_x86::Mnemonic::Jmp
            || join.flow_control() != iced_x86::FlowControl::UnconditionalBranch
            || usize::try_from(join.near_branch_target()).ok() != Some(branch.merge_offset)
        {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: branch.branch_offset,
            });
        }
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            cursor,
            branch.branch_offset,
            false,
            claimed,
            call_sites,
            evidence,
            validated_calls,
            &mut depth,
            &mut peak,
        )?;
        let branch_depth = depth;
        let mut special_depth = branch_depth;
        let mut special_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch_end,
            branch.join_offset,
            false,
            claimed,
            call_sites,
            evidence,
            validated_calls,
            &mut special_depth,
            &mut special_peak,
        )?;
        let mut ordinary_depth = branch_depth;
        let mut ordinary_peak = peak;
        replay_x86_scalar_linear_region(
            machine,
            bytes,
            branch.ordinary_arm_offset,
            branch.merge_offset,
            false,
            claimed,
            call_sites,
            evidence,
            validated_calls,
            &mut ordinary_depth,
            &mut ordinary_peak,
        )?;
        if special_depth != ordinary_depth {
            return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
        }
        depth = special_depth;
        peak = special_peak.max(ordinary_peak);
        cursor = branch.merge_offset;
    }
    replay_x86_scalar_linear_region(
        machine,
        bytes,
        cursor,
        end,
        require_return,
        claimed,
        call_sites,
        evidence,
        validated_calls,
        &mut depth,
        &mut peak,
    )?;
    if depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(peak)
}

#[allow(clippy::too_many_arguments)]
fn replay_scalar_conditional_region_with_divisions(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    division_branches: &[TerminalScalarDivisionBranchEvidence],
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    allow_calls: bool,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
) -> Result<u32, TerminalObjectError> {
    if division_branches.is_empty() {
        return replay_scalar_conditional_region(
            architecture,
            machine,
            bytes,
            start,
            end,
            require_return,
            claimed,
            call_sites,
            allow_calls,
            evidence,
            validated_calls,
            None,
        );
    }
    if architecture != Architecture::X86_64 || !allow_calls {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    replay_x86_scalar_division_region(
        machine,
        bytes,
        start,
        end,
        require_return,
        division_branches,
        claimed,
        call_sites,
        evidence,
        validated_calls,
    )
}

#[allow(clippy::too_many_arguments)]
fn replay_scalar_conditional_terminal_region(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    crash: bool,
    division_branches: &[TerminalScalarDivisionBranchEvidence],
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
) -> Result<u32, TerminalObjectError> {
    if !crash {
        return replay_scalar_conditional_region_with_divisions(
            architecture,
            machine,
            bytes,
            start,
            end,
            true,
            division_branches,
            claimed,
            call_sites,
            true,
            evidence,
            validated_calls,
        );
    }
    if !division_branches.is_empty() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let crash_bytes: &[u8] = match architecture {
        Architecture::X86_64 => &[0x0f, 0x0b],
        Architecture::Aarch64 => &[0x00, 0x00, 0x20, 0xd4],
    };
    let crash_offset = end.checked_sub(crash_bytes.len()).ok_or(
        TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        },
    )?;
    if crash_offset < start || bytes.get(crash_offset..end) != Some(crash_bytes) {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: crash_offset,
        });
    }
    replay_scalar_conditional_region(
        architecture,
        machine,
        bytes,
        start,
        crash_offset,
        false,
        claimed,
        call_sites,
        true,
        evidence,
        validated_calls,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_conditional_tree_scalar_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    evidence: &TerminalScalarStackEvidence,
    decisions: &[TerminalScalarConditionalBranchEvidence],
    crash_leaves: &[bool],
    division_branches: &[TerminalScalarDivisionBranchEvidence],
    cleanups: &[TerminalScalarControlAffineCleanupRecord],
) -> Result<
    (
        TerminalObjectScalarStack,
        Vec<TerminalObjectScalarCallStack>,
    ),
    TerminalObjectError,
> {
    if decisions.is_empty()
        || crash_leaves.len() != decisions.len() + 1
        || !cleanups.is_empty() && cleanups.len() != crash_leaves.len()
        || !cleanups.is_empty() && crash_leaves.iter().any(|crash| *crash)
        || !cleanups.is_empty() && !division_branches.is_empty()
        || evidence.cleanup_preservation.is_some()
        || decisions
            .windows(2)
            .any(|pair| pair[0].branch_offset >= pair[1].branch_offset)
        || evidence
            .mutations
            .windows(2)
            .any(|pair| pair[0].offset >= pair[1].offset)
    {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: decisions.first().map_or(0, |branch| branch.branch_offset),
        });
    }
    let mut prefixes = Vec::with_capacity(decisions.len());
    let mut leaves = Vec::with_capacity(crash_leaves.len());
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        0,
        bytes.len(),
        decisions,
        &mut prefixes,
        &mut leaves,
    )?;
    if leaves.len() != crash_leaves.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: decisions[0].branch_offset,
        });
    }
    let mut division_regions = prefixes
        .iter()
        .map(|(start, end, _)| (*start, *end))
        .collect::<Vec<_>>();
    division_regions.extend(leaves.iter().copied());
    validate_division_branch_regions(machine, division_branches, &division_regions)?;

    let mut claimed = evidence
        .mutations
        .iter()
        .map(|mutation| (mutation.offset, *mutation))
        .collect::<std::collections::BTreeMap<_, _>>();
    if claimed.len() != evidence.mutations.len() {
        return Err(TerminalObjectError::NonCanonicalScalarStackMutationOrder(
            machine,
        ));
    }
    let mut call_sites = std::collections::BTreeMap::new();
    for call in calls {
        validate_internal_call_site(architecture, machine, bytes, *call)?;
        let call_start = match architecture {
            Architecture::X86_64 => call.offset - 1,
            Architecture::Aarch64 => call.offset,
        };
        call_sites.insert(call_start, *call);
    }
    let mut validated_calls = Vec::with_capacity(calls.len());
    let mut peak = 0;
    for (start, end, condition) in prefixes {
        let prefix_peak = replay_scalar_conditional_region_with_divisions(
            architecture,
            machine,
            bytes,
            start,
            end,
            false,
            division_branches_in_region(division_branches, start, end),
            &mut claimed,
            &mut call_sites,
            condition == TerminalScalarConditionalCondition::Expression,
            evidence,
            &mut validated_calls,
        )?;
        if condition == TerminalScalarConditionalCondition::Parameter && prefix_peak != 0 {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: end,
            });
        }
        peak = peak.max(prefix_peak);
    }
    for (index, (start, end)) in leaves.into_iter().enumerate() {
        let leaf_peak = if let Some(cleanup) = cleanups.get(index) {
            replay_scalar_conditional_region(
                architecture,
                machine,
                bytes,
                start,
                end,
                true,
                &mut claimed,
                &mut call_sites,
                true,
                evidence,
                &mut validated_calls,
                Some(&cleanup.cleanup),
            )?
        } else {
            replay_scalar_conditional_terminal_region(
                architecture,
                machine,
                bytes,
                start,
                end,
                crash_leaves[index],
                division_branches_in_region(division_branches, start, end),
                &mut claimed,
                &mut call_sites,
                evidence,
                &mut validated_calls,
            )?
        };
        peak = peak.max(leaf_peak);
    }
    if let Some((&offset, _)) = claimed.first_key_value() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if let Some((&offset, call)) = call_sites.first_key_value() {
        return Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine,
            owner: call.owner,
            offset,
        });
    }
    Ok((
        TerminalObjectScalarStack {
            local_peak_bytes: peak,
            stack_alignment: evidence.stack_alignment,
        },
        validated_calls,
    ))
}

fn replay_scalar_conditional_region(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    require_return: bool,
    claimed: &mut std::collections::BTreeMap<usize, TerminalScalarStackMutation>,
    call_sites: &mut std::collections::BTreeMap<
        usize,
        omega_terminal_machine_code::TerminalInternalCallRelocation,
    >,
    allow_calls: bool,
    evidence: &TerminalScalarStackEvidence,
    validated_calls: &mut Vec<TerminalObjectScalarCallStack>,
    scalar_affine_cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
) -> Result<u32, TerminalObjectError> {
    if start > end || end > bytes.len() {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: start,
        });
    }
    let mut depth = 0_u32;
    let mut peak = 0_u32;
    let mut saw_return = false;
    match architecture {
        Architecture::X86_64 => {
            let mut decoder = iced_x86::Decoder::with_ip(
                64,
                &bytes[start..end],
                start as u64,
                iced_x86::DecoderOptions::NONE,
            );
            let mut info_factory = iced_x86::InstructionInfoFactory::new();
            while decoder.can_decode() {
                let instruction = decoder.decode();
                let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
                if instruction.is_invalid() {
                    return Err(TerminalObjectError::InvalidScalarInstructionEncoding {
                        machine,
                        offset,
                    });
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Ret {
                    if !require_return
                        || offset.checked_add(instruction.len()) != Some(end)
                        || saw_return
                    {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if instruction.mnemonic() == iced_x86::Mnemonic::Call {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    match call.owner {
                        TerminalCallSiteOwner::Operation(operation) if !allow_calls => {
                            return Err(TerminalObjectError::ScalarConditionalCallOutsideArm {
                                machine,
                                operation,
                                offset,
                            });
                        }
                        TerminalCallSiteOwner::Operation(_) if allow_calls => {}
                        TerminalCallSiteOwner::CleanupAction { edge, .. }
                            if allow_calls
                                && scalar_affine_cleanup
                                    .is_some_and(|cleanup| cleanup.psi_edge == edge) => {}
                        _ => {
                            return Err(TerminalObjectError::UntypedScalarInternalCall {
                                machine,
                                offset,
                            });
                        }
                    }
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if instruction.flow_control() != iced_x86::FlowControl::Next {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                let stack_mutation = matches!(
                    instruction.mnemonic(),
                    iced_x86::Mnemonic::Push | iced_x86::Mnemonic::Pop
                ) || matches!(
                    instruction.mnemonic(),
                    iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub
                ) && instruction.op0_register()
                    == iced_x86::Register::RSP
                    || instruction.mnemonic() == iced_x86::Mnemonic::Lea
                        && instruction.op0_register() == iced_x86::Register::RSP;
                if stack_mutation {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_x86_scalar_mutation(machine, bytes, &instruction, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                    continue;
                }
                let info = info_factory.info(&instruction);
                if info.used_registers().iter().any(|register| {
                    matches!(
                        register.register(),
                        iced_x86::Register::RSP
                            | iced_x86::Register::ESP
                            | iced_x86::Register::SP
                            | iced_x86::Register::SPL
                    ) && matches!(
                        register.access(),
                        iced_x86::OpAccess::Write
                            | iced_x86::OpAccess::CondWrite
                            | iced_x86::OpAccess::ReadWrite
                            | iced_x86::OpAccess::ReadCondWrite
                    )
                }) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
        }
        Architecture::Aarch64 => {
            if !start.is_multiple_of(4) || !end.is_multiple_of(4) {
                return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                    machine,
                    offset: start,
                });
            }
            for offset in (start..end).step_by(4) {
                let encoded = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .expect("four-byte AArch64 word"),
                );
                if encoded == 0xd65f_03c0 {
                    if !require_return || offset + 4 != end || saw_return {
                        return Err(TerminalObjectError::NonLinearScalarControlFlow {
                            machine,
                            offset,
                        });
                    }
                    saw_return = true;
                    continue;
                }
                if encoded == 0x9400_0000 {
                    let call = call_sites.remove(&offset).ok_or(
                        TerminalObjectError::UntypedScalarInternalCall { machine, offset },
                    )?;
                    match call.owner {
                        TerminalCallSiteOwner::Operation(operation) if !allow_calls => {
                            return Err(TerminalObjectError::ScalarConditionalCallOutsideArm {
                                machine,
                                operation,
                                offset,
                            });
                        }
                        TerminalCallSiteOwner::Operation(_) if allow_calls => {}
                        TerminalCallSiteOwner::CleanupAction { edge, .. }
                            if allow_calls
                                && scalar_affine_cleanup
                                    .is_some_and(|cleanup| cleanup.psi_edge == edge) => {}
                        _ => {
                            return Err(TerminalObjectError::UntypedScalarInternalCall {
                                machine,
                                offset,
                            });
                        }
                    }
                    let call_evidence = call.scalar_stack.ok_or(
                        TerminalObjectError::MissingScalarCallStackEvidence {
                            caller: machine,
                            owner: call.owner,
                        },
                    )?;
                    let validated = validate_scalar_call_stack(
                        architecture,
                        machine,
                        bytes,
                        call,
                        call_evidence,
                        evidence,
                        depth,
                        scalar_affine_cleanup,
                    )?;
                    peak = peak.max(validated.caller_live_bytes);
                    validated_calls.push(validated);
                    continue;
                }
                if aarch64_control_flow_instruction(encoded) {
                    return Err(TerminalObjectError::NonLinearScalarControlFlow {
                        machine,
                        offset,
                    });
                }
                if aarch64_stack_adjustment_at(bytes, offset) {
                    let mutation = claimed.remove(&offset).ok_or(
                        TerminalObjectError::UnclaimedScalarStackMutation { machine, offset },
                    )?;
                    validate_aarch64_scalar_mutation(machine, encoded, mutation)?;
                    replay_scalar_mutation(machine, offset, mutation.kind, &mut depth, &mut peak)?;
                } else if aarch64_unsupported_sp_write(encoded) {
                    return Err(TerminalObjectError::UnsupportedScalarStackMutation {
                        machine,
                        offset,
                    });
                }
            }
        }
    }
    if require_return != saw_return || depth != 0 {
        return Err(TerminalObjectError::MissingBalancedScalarReturn(machine));
    }
    Ok(peak)
}

fn validate_internal_call_site(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    call: omega_terminal_machine_code::TerminalInternalCallRelocation,
) -> Result<(RelocationKind, usize), TerminalObjectError> {
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
        return Err(TerminalObjectError::InvalidInternalCallSite {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalObjectError {
    EmptyPlan,
    NonCanonicalFunctionOrder {
        previous: MachineId,
        current: MachineId,
    },
    EmptyFunction(MachineId),
    NonCanonicalInternalCallOrder(MachineId),
    NonCanonicalFuelAttributionOrder(MachineId),
    FuelAttributionOutsideFunction(MachineId),
    InvalidFuelAttribution(MachineId),
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
        owner: TerminalCallSiteOwner,
        offset: usize,
    },
    InvalidInternalUnitCallEvidence(MachineId),
    InvalidUnitAffineCleanupEvidence(MachineId),
    InternalCallOperationNotInProvenance {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
    },
    DuplicateInternalCallOperation {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
    },
    MissingUnitCallStackEvidence {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
    },
    UnexpectedUnitCallStackEvidence {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
    },
    MissingScalarCallStackEvidence {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
    },
    UnexpectedScalarCallStackEvidence {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
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
        owner: TerminalCallSiteOwner,
        offset: usize,
    },
    MisalignedScalarCalleeEntry {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
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
        owner: TerminalCallSiteOwner,
    },
    InvalidUnitStackEncoding {
        machine: MachineId,
        owner: Option<TerminalCallSiteOwner>,
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
        owner: TerminalCallSiteOwner,
        caller_live_bytes: u32,
    },
    MissingX86UnitCallStackAdjustment {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
    },
    MissingAarch64UnitReturnLink {
        caller: MachineId,
        operation: Option<psi_core::OperationId>,
    },
    UnaccountedTerminalStack(MachineId),
    TerminalStackCycle(MachineId),
    TerminalStackCompositionOverflow {
        caller: MachineId,
        owner: TerminalCallSiteOwner,
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
    BoundaryRealizationMismatch {
        machine: MachineId,
        operation: psi_core::OperationId,
    },
    EntryFunctionMissing(MachineId),
    TextSizeOverflow,
}

impl std::fmt::Display for TerminalObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalObjectError {}
