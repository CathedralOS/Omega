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
mod final_image_validation;
mod image_output;
mod installation;
mod instruction_loads;
mod native_fuel;
mod partial_cleanup_partition;
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
mod unit_stack;

pub use image_output::{
    TerminalExecutableImage, TerminalNativeFuelExecutableImage, TerminalObjectContainer,
    can_emit_terminal_executable_image, emit_terminal_executable_image,
    emit_terminal_native_fuel_executable_image, emit_terminal_native_fuel_object_container,
    emit_terminal_object_container,
};
pub use installation::*;
pub use native_fuel::{
    TerminalNativeFuelValidationError, ValidatedTerminalNativeFuelArtifact,
    ValidatedTerminalNativeFuelFunction, validate_terminal_native_fuel_plan,
};
pub(crate) use partial_cleanup_partition::exact_partial_cleanup_partition;
pub use stack_demand::{derive_terminal_stack_demand, derive_terminal_unit_stack_demand};

use boundary_results::boundary_result_is_exact;
use byte_sequence_custody::linux_write_line_custody_is_exact;
use completion_receipts::{CompletionCustodyError, validate_completion_custody};
use scalar_cleanup_preservation::validate_scalar_cleanup_preservation;
use scalar_conditional_call_paths::{conditional_call_path, conditional_paths_are_exclusive};
use scalar_control_cleanup::{cleanup_for_owner, validate_scalar_control_cleanup_evidence};
use scalar_stack::validate_scalar_stack;
use structural_return::validate_structural_return_record;
use unit_affine_cleanup::validate_unit_affine_cleanup;
use unit_call_custody::{expected_projected_copy_bytes, validate_internal_unit_call_custody};
use unit_stack::{
    validate_complete_unit_stack_evidence, validate_unit_call_stack, validate_unit_function_stack,
};

use omega_object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
    entry_symbol_name,
};
use omega_target::{Architecture, NativeTarget};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalMachineCodePlan, TerminalNativeFuelAttribution,
    TerminalNativeFuelSite, TerminalPortEffectRecord, TerminalScalarControlAffineCleanupRecord,
    TerminalStructuralReturnRecord,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalCallSiteOwner, TerminalPsiProvenance,
};
use psi_core::MachineId;
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

    fn fuel_attribution(
        &self,
    ) -> Vec<omega_terminal_installation_evidence::TerminalFuelAttributionEvidence> {
        self.fuel_attribution
            .iter()
            .map(|row| omega_terminal_installation_evidence::TerminalFuelAttributionEvidence {
                machine: row.machine,
                schedule: row.attribution.schedule,
                site: match row.attribution.site {
                    TerminalNativeFuelSite::Operation(operation) => {
                        omega_terminal_installation_evidence::TerminalFuelAttributionSite::Operation(
                            operation,
                        )
                    }
                    TerminalNativeFuelSite::Edge(edge) => {
                        omega_terminal_installation_evidence::TerminalFuelAttributionSite::Edge(edge)
                    }
                },
                units: row.attribution.units,
                operation_ordinal: row.attribution.operation_ordinal,
                text_offset: row.text_offset,
                byte_count: row.attribution.byte_count,
            })
            .collect()
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
                    | TerminalBoundaryRealization::LinuxExitGroupI32(_)
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
                &inline_data,
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
            if let Err(error) = validate_completion_custody(settlement) {
                return Err(match error {
                    CompletionCustodyError::InvalidArgumentPath => {
                        TerminalObjectError::InvalidBoundarySettlementArgumentPath {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::InvalidReceiptArgumentIndex => {
                        TerminalObjectError::InvalidCompletionReceiptArgumentIndex {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::InvalidReceiptCustody => {
                        TerminalObjectError::InvalidCompletionReceiptCustody {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                    CompletionCustodyError::InvalidProviderCustody => {
                        TerminalObjectError::InvalidCompletionProviderCustody {
                            machine: function.machine,
                            operation: settlement.psi_operation,
                        }
                    }
                });
            }
            let valid_realization = match settlement.realization {
                TerminalBoundaryRealization::MetadataOnlyPort(realization) => {
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
                TerminalBoundaryRealization::DirectPortReadU8(realization) => {
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
                                .fuel_attribution
                                .iter()
                                .filter(|attribution| {
                                    attribution.site
                                        == TerminalNativeFuelSite::Edge(result.return_edge)
                                        && attribution.units == 1
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
                TerminalBoundaryRealization::LinuxWriteLine(_) => {
                    linux_write_line_custody_is_exact(
                        plan.target,
                        settlement,
                        Some(&function.bytes),
                    ) && function.unit_stack.is_some()
                        && function.scalar_stack.is_none()
                }
                TerminalBoundaryRealization::LinuxExitGroupI32(_) => {
                    let [argument] = settlement.scalar_arguments.as_slice() else {
                        return Err(TerminalObjectError::BoundaryRealizationMismatch {
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
                                .fuel_attribution
                                .iter()
                                .filter(|attribution| {
                                    matches!(attribution.site, TerminalNativeFuelSite::Edge(_))
                                        && attribution.units == 1
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

impl std::fmt::Display for TerminalObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalObjectError {}
