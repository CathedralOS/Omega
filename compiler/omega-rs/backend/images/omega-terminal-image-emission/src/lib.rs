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

mod installation;
mod instruction_loads;

pub use installation::*;

use instruction_loads::*;

use omega_calling_conventions::{
    IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape,
};
use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    FinalImageInput, emitted_direct_executable_output, validate_final_text_relocation_envelope,
};
use omega_object_file::{
    ObjectContainerInput, ObjectContainerOutput, ObjectPlan, ObjectSymbolHandle, RelocationKind,
    RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind, SectionPlan, SymbolKind,
    SymbolPlan, SymbolSection, emit_omega_object_container, entry_symbol_name,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_machine_code::{
    TerminalBoundarySettlementRecord, TerminalMachineCodePlan, TerminalNativeFuelAttribution,
    TerminalNativeFuelSite, TerminalPortEffectRecord, TerminalScalarCallStackEvidence,
    TerminalScalarConditionalBranchEvidence, TerminalScalarConditionalCondition,
    TerminalScalarControlAffineCleanupRecord, TerminalScalarControlFlowEvidence,
    TerminalScalarDivisionBranchEvidence, TerminalScalarJoinBranchEvidence,
    TerminalScalarStackEvidence, TerminalScalarStackMutation, TerminalScalarStackMutationKind,
    TerminalStackAdjustmentPair, TerminalStructuralReturnRecord, TerminalUnitCallStackEvidence,
    TerminalUnitStackEvidence,
};
use omega_terminal_target_operations::{
    TerminalBoundaryRealization, TerminalCallSiteOwner, TerminalPsiProvenance,
};
use psi_core::{MachineId, ScalarType};
use psi_diagnostics::Diagnostic;
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

pub(crate) fn exact_partial_cleanup_partition(
    declarations: &[psi_terminal::StructuralTypeDeclaration],
    root_type: psi_core::StructuralTypeId,
    moved: &[(
        &[psi_terminal::StructuralPathSegment],
        psi_core::StructuralTypeId,
    )],
    residuals: &[&psi_terminal::StructuralAffineDiscard],
) -> bool {
    if declarations.is_empty() || moved.is_empty() || residuals.is_empty() {
        return false;
    }
    let mut by_id = std::collections::BTreeMap::new();
    let mut identities = std::collections::BTreeSet::new();
    for declaration in declarations {
        if declaration.identity.is_empty()
            || !identities.insert(declaration.identity.as_str())
            || by_id.insert(declaration.id, declaration).is_some()
        {
            return false;
        }
    }
    if declarations.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return false;
    }
    let mut expected = Vec::new();
    if append_expected_partial_residuals(root_type, &[], moved, &by_id, &mut expected).is_none()
        || expected.len() != residuals.len()
    {
        return false;
    }
    residuals
        .iter()
        .zip(expected)
        .all(|(actual, (path, structural_type))| {
            actual.path == path && actual.structural_type == structural_type
        })
}

fn append_expected_partial_residuals(
    structural_type: psi_core::StructuralTypeId,
    prefix: &[psi_terminal::StructuralPathSegment],
    moved: &[(
        &[psi_terminal::StructuralPathSegment],
        psi_core::StructuralTypeId,
    )],
    declarations: &std::collections::BTreeMap<
        psi_core::StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
    output: &mut Vec<(
        Vec<psi_terminal::StructuralPathSegment>,
        psi_core::StructuralTypeId,
    )>,
) -> Option<()> {
    let declaration = declarations.get(&structural_type)?;
    let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(
                    field.field_type,
                    psi_terminal::StructuralFieldType::Structural(_)
                )
        })
    {
        return None;
    }
    for field in fields.iter().rev() {
        let psi_terminal::StructuralFieldType::Structural(field_type) = field.field_type else {
            return None;
        };
        let matching = moved
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(psi_terminal::StructuralPathSegment::Field(identity))
                    if identity == &field.identity)
            })
            .copied()
            .collect::<Vec<_>>();
        let mut field_path = prefix.to_vec();
        field_path.push(psi_terminal::StructuralPathSegment::Field(
            field.identity.clone(),
        ));
        if matching.is_empty() {
            output.push((field_path, field_type));
            continue;
        }
        let whole = matching
            .iter()
            .filter(|(path, _)| path.len() == 1)
            .collect::<Vec<_>>();
        if !whole.is_empty() {
            if whole.len() != 1 || matching.len() != 1 || whole[0].1 != field_type {
                return None;
            }
            continue;
        }
        let nested = matching
            .iter()
            .map(|(path, leaf)| (&path[1..], *leaf))
            .collect::<Vec<_>>();
        append_expected_partial_residuals(field_type, &field_path, &nested, declarations, output)?;
    }
    Some(())
}

fn bounded_nominal_receiver_shape(shape: omega_calling_conventions::ValueShape) -> bool {
    shape == omega_calling_conventions::ValueShape::integer(0, 1)
        || shape.class == omega_calling_conventions::ValueClass::Integer
            && shape.byte_size != 0
            && matches!(shape.alignment, 1 | 2 | 4 | 8)
            && shape.byte_size % shape.alignment == 0
}

fn validate_structural_return_record(
    target: NativeTarget,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    fuel_attribution: &[TerminalNativeFuelAttribution],
    returned: &TerminalStructuralReturnRecord,
) -> Result<(), TerminalObjectError> {
    let architecture = target.architecture;
    let expected_call_plan = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature {
            parameters: returned
                .parameter_placements
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: Some(returned.shape),
        },
    )
    .map_err(|_| TerminalObjectError::InvalidStructuralReturnEvidence(machine))?;
    let source_index = returned.parameters.first().map(|_| 0);
    let end = returned
        .code_offset
        .checked_add(returned.byte_count)
        .ok_or(TerminalObjectError::InvalidStructuralReturnEvidence(
            machine,
        ))?;
    if returned.code_offset != 0
        || end != bytes.len()
        || returned.byte_count == 0
        || fuel_attribution.len() != returned.trivial_affine_locals.len() + 1
        || returned
            .trivial_affine_locals
            .iter()
            .enumerate()
            .any(|(ordinal, (operation, _, _))| {
                fuel_attribution.get(ordinal).is_none_or(|attribution| {
                    attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
                        || attribution.site != TerminalNativeFuelSite::Operation(*operation)
                        || attribution.units != 1
                        || attribution.operation_ordinal != ordinal
                        || attribution.code_offset != 0
                        || attribution.byte_count != 0
                })
            })
        || fuel_attribution.last().is_none_or(|attribution| {
            attribution.schedule != TerminalFuelSchedule::CURRENT.identity()
                || attribution.site != TerminalNativeFuelSite::Edge(returned.psi_edge)
                || attribution.units != 1
                || attribution.operation_ordinal != returned.trivial_affine_locals.len()
                || attribution.code_offset != 0
                || attribution.byte_count != returned.byte_count
        })
        || provenance.edges.as_slice() != [returned.psi_edge]
        || provenance.operations
            != returned
                .trivial_affine_locals
                .iter()
                .map(|(operation, _, _)| *operation)
                .collect::<Vec<_>>()
        || returned.source.structural_type != returned.result.structural_type
        || returned.source.multiplicity != returned.result.multiplicity
        || returned.source.qualifications != returned.result.qualifications
        || returned.shape != returned.source_placement.shape
        || returned.shape != returned.result_placement.shape
        || returned.shape.byte_size != 8
        || returned.returned_claims.len() != 1
        || returned
            .parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| {
                parameter.is_self || usize::try_from(parameter.position) != Ok(index)
            })
        || returned.parameters.first() != Some(&returned.source)
        || returned.parameters.iter().skip(1).any(|parameter| {
            parameter.place == returned.source.place
                || parameter.place == returned.result.place
                || !parameter.qualifications.is_empty()
        })
        || returned
            .parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != returned.parameters.len()
        || returned.trivial_affine_locals.iter().enumerate().any(|(index, (_, local, local_type))| {
            !matches!(
                local.kind,
                psi_core::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    structural_type
                } if usize::try_from(declaration_ordinal) == Ok(index)
                    && structural_type == local_type.id
            ) || local.id == returned.source.place
                || local.id == returned.result.place
                || returned.parameters.iter().any(|parameter| parameter.place == local.id)
                || local_type.identity.is_empty()
                || !matches!(
                    local_type.shape,
                    psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
                )
        })
        || returned
            .trivial_affine_locals
            .iter()
            .map(|(_, local, _)| local.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != returned.trivial_affine_locals.len()
        || returned.trivial_affine_discards
            != returned
                .trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(
                    returned
                        .parameters
                        .iter()
                        .skip(1)
                        .rev()
                        .map(|parameter| parameter.place),
                )
                .collect::<Vec<_>>()
        || returned
            .parameters
            .iter()
            .skip(1)
            .any(|parameter| parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine)
        || returned.parameter_placements.len() != returned.parameters.len()
        || expected_call_plan.parameters != returned.parameter_placements
        || expected_call_plan.result.as_ref() != Some(&returned.result_placement)
        || source_index.and_then(|index| returned.parameter_placements.get(index))
            != Some(&returned.source_placement)
    {
        return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
            machine,
        ));
    }
    let expected = match architecture {
        Architecture::X86_64 => {
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: source,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.source_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: omega_calling_conventions::MachineRegister::X86Rax,
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.result_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            match source {
                omega_calling_conventions::MachineRegister::X86Rdi => &[0x48, 0x89, 0xf8, 0xc3][..],
                omega_calling_conventions::MachineRegister::X86Rcx => &[0x48, 0x89, 0xc8, 0xc3][..],
                _ => {
                    return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                        machine,
                    ));
                }
            }
        }
        Architecture::Aarch64 => {
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: omega_calling_conventions::MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.source_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            let [
                omega_calling_conventions::ValueLocation::Register {
                    register: omega_calling_conventions::MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                },
            ] = returned.result_placement.locations.as_slice()
            else {
                return Err(TerminalObjectError::InvalidStructuralReturnEvidence(
                    machine,
                ));
            };
            &[0xc0, 0x03, 0x5f, 0xd6]
        }
    };
    if bytes != expected {
        return Err(TerminalObjectError::StructuralReturnBytesMismatch(machine));
    }
    Ok(())
}

fn validate_internal_unit_call_custody(
    target: NativeTarget,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    function_bytes: &[u8],
    fuel: &[TerminalNativeFuelAttribution],
    relocations: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
    parameter_homes: &[omega_terminal_machine_code::TerminalUnitParameterHomeRecord],
    validated_function_stack: Option<&TerminalObjectUnitStack>,
    validated_call_stack: Option<&TerminalObjectUnitCallStack>,
    validated_scalar_call_stack: Option<&TerminalObjectScalarCallStack>,
    custody: &omega_terminal_machine_code::TerminalInternalUnitCallRecord,
    affine_cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
) -> Result<(), TerminalObjectError> {
    let invalid = || TerminalObjectError::InvalidInternalUnitCallEvidence(machine);
    let Some(relocation) = relocations.iter().find(|relocation| {
        relocation.owner == custody.owner
            && relocation.target == custody.target
            && (relocation.unit_stack.is_some()
                || (affine_cleanup.is_some()
                    && matches!(
                        relocation.owner,
                        TerminalCallSiteOwner::CleanupAction { .. }
                    )
                    && relocation.scalar_stack.is_some()))
    }) else {
        return Err(invalid());
    };
    if validated_call_stack.is_none() == validated_scalar_call_stack.is_none() {
        return Err(invalid());
    }
    let end = custody
        .code_offset
        .checked_add(custody.byte_count)
        .ok_or_else(invalid)?;
    let relocation_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
    let linkage_bytes = match target.architecture {
        Architecture::X86_64 => 8,
        Architecture::Aarch64 => 0,
    };
    if custody.arguments.is_empty() && custody.claim_transfers.is_empty() {
        if custody.result.is_some() {
            return Err(invalid());
        }
        let owner_valid = match custody.owner {
            TerminalCallSiteOwner::Operation(operation) => {
                provenance.operations.contains(&operation)
                    && fuel
                        .iter()
                        .filter(|attribution| {
                            attribution.site == TerminalNativeFuelSite::Operation(operation)
                                && attribution.operation_ordinal == custody.operation_ordinal
                                && attribution.code_offset == custody.code_offset
                                && attribution.byte_count == custody.byte_count
                        })
                        .count()
                        == 1
            }
            TerminalCallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                let Some(cleanup) = affine_cleanup else {
                    return Err(invalid());
                };
                let Some(psi_terminal::TerminalAffineCleanupAction::InvokeNominal(nominal)) =
                    usize::try_from(action_ordinal)
                        .ok()
                        .and_then(|ordinal| cleanup.actions.get(ordinal))
                else {
                    return Err(invalid());
                };
                let cleanup_end = cleanup
                    .code_offset
                    .checked_add(cleanup.byte_count)
                    .ok_or_else(invalid)?;
                provenance.edges.contains(&edge)
                    && cleanup.psi_edge == edge
                    && nominal.cleanup_machine == custody.target
                    && cleanup.code_offset <= custody.code_offset
                    && end <= cleanup_end
                    && fuel
                        .iter()
                        .filter(|attribution| {
                            attribution.site == TerminalNativeFuelSite::Edge(edge)
                                && attribution.operation_ordinal == custody.operation_ordinal
                                && attribution.code_offset == cleanup.code_offset
                                && attribution.byte_count == cleanup.byte_count
                        })
                        .count()
                        == 1
            }
        };
        if custody.byte_count == 0
            || custody.code_offset > relocation.offset
            || relocation_end > end
            || !owner_valid
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let validated_function_stack = validated_function_stack.ok_or_else(invalid)?;
    let validated_call_stack = validated_call_stack.ok_or_else(invalid)?;
    let expected_call_stack_bytes = validated_call_stack
        .transient_bytes
        .checked_sub(linkage_bytes)
        .ok_or_else(invalid)?;
    let TerminalCallSiteOwner::Operation(operation) = custody.owner else {
        return Err(invalid());
    };
    let expected_plan = omega_calling_conventions::evaluate_call_plan(
        omega_calling_conventions::CallingPolicy::native_for_target(target),
        &omega_calling_conventions::CallSignature {
            parameters: custody
                .arguments
                .iter()
                .map(|argument| argument.shape)
                .collect(),
            result: custody.result.map(|result| {
                let bytes = match result {
                    psi_core::ScalarType::Boolean => 1,
                    psi_core::ScalarType::Integer(integer) => integer.bits().div_ceil(8),
                };
                omega_calling_conventions::ValueShape::integer(
                    bytes,
                    bytes.next_power_of_two().min(8),
                )
            }),
        },
    )
    .map_err(|_| invalid())?;
    let projected_argument_indexes = custody
        .arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| (!argument.path.is_empty()).then_some(index))
        .collect::<std::collections::BTreeSet<_>>();
    let transferred_argument_indexes = custody
        .claim_transfers
        .iter()
        .filter_map(|transfer| usize::try_from(transfer.argument_index).ok())
        .collect::<std::collections::BTreeSet<_>>();
    let projected_home = if projected_argument_indexes.is_empty() {
        None
    } else {
        let [home] = parameter_homes else {
            return Err(invalid());
        };
        if home.byte_offset != 0
            || home.indirect
                != matches!(
                    home.source.locations.as_slice(),
                    [omega_calling_conventions::ValueLocation::Indirect { .. }]
                )
        {
            return Err(invalid());
        }
        let expected_caller_plan = omega_calling_conventions::evaluate_call_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            &omega_calling_conventions::CallSignature {
                parameters: vec![home.shape],
                result: None,
            },
        )
        .map_err(|_| invalid())?;
        if expected_caller_plan.parameters.as_slice() != [home.source.clone()] {
            return Err(invalid());
        }
        let stored_bytes = if home.indirect {
            8
        } else {
            u32::from(home.shape.byte_size)
        };
        let expected_frame_bytes = match target.architecture {
            Architecture::X86_64 => stored_bytes.next_multiple_of(16),
            Architecture::Aarch64 => stored_bytes
                .next_multiple_of(8)
                .checked_add(8)
                .map(|bytes| bytes.next_multiple_of(16))
                .ok_or_else(invalid)?,
        };
        if validated_function_stack.frame_bytes != expected_frame_bytes {
            return Err(invalid());
        }
        Some(home)
    };
    if custody.byte_count == 0
        || custody.code_offset > relocation.offset
        || relocation_end > end
        || !provenance.operations.contains(&operation)
        || fuel
            .iter()
            .filter(|attribution| {
                attribution.site == TerminalNativeFuelSite::Operation(operation)
                    && attribution.operation_ordinal == custody.operation_ordinal
                    && attribution.code_offset == custody.code_offset
                    && attribution.byte_count == custody.byte_count
            })
            .count()
            != 1
        || expected_plan.parameters.len() != custody.arguments.len()
        || custody.arguments.windows(2).any(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_none_or(|end| end > pair[1].code_offset)
        })
        || custody
            .arguments
            .iter()
            .zip(&expected_plan.parameters)
            .any(|(argument, destination)| {
                let home_mismatch = !argument.path.is_empty()
                    && projected_home.is_none_or(|home| {
                        argument.place != home.place
                            || argument.root_structural_type != home.structural_type
                            || argument.source != home.source
                            || argument.source.shape != home.shape
                            || argument.source_home_byte_offset != home.byte_offset
                    });
                argument.destination != *destination
                    || argument.call_stack_bytes != expected_call_stack_bytes
                    || home_mismatch
                    || argument.byte_count == 0
                    || argument.bytes.len() != argument.byte_count
                    || argument
                        .code_offset
                        .checked_add(argument.byte_count)
                        .and_then(|end| function_bytes.get(argument.code_offset..end))
                        != Some(argument.bytes.as_slice())
                    || (!argument.path.is_empty()
                        && expected_projected_copy_bytes(target, argument).as_deref()
                            != Some(argument.bytes.as_slice()))
                    || argument.code_offset < custody.code_offset
                    || argument
                        .code_offset
                        .checked_add(argument.byte_count)
                        .is_none_or(|argument_end| argument_end > end)
                    || argument
                        .source_byte_offset
                        .checked_add(u32::from(argument.shape.byte_size))
                        .is_none_or(|end| end > u32::from(argument.source.shape.byte_size))
                    || match argument.path.as_slice() {
                        [] => {
                            argument.source_byte_offset != 0
                                || argument.source.shape != argument.shape
                                || argument.root_structural_type != argument.structural_type
                                || argument.fixed_array_length.is_some()
                                || argument.element_stride.is_some()
                        }
                        [psi_terminal::StructuralPathSegment::FixedIndex(index)] => {
                            let expected_stride = u32::from(argument.shape.byte_size)
                                .next_multiple_of(u32::from(argument.shape.alignment));
                            let Some(length) = argument.fixed_array_length else {
                                return true;
                            };
                            let Some(stride) = argument.element_stride else {
                                return true;
                            };
                            argument.root_structural_type == argument.structural_type
                                || *index >= length
                                || stride != expected_stride
                                || u64::from(stride).checked_mul(*index)
                                    != Some(u64::from(argument.source_byte_offset))
                                || u64::from(stride).checked_mul(length)
                                    != Some(u64::from(argument.source.shape.byte_size))
                                || argument.source.shape.alignment != argument.shape.alignment
                        }
                        path @ [psi_terminal::StructuralPathSegment::Field(_), ..]
                            if path.iter().all(|segment| {
                                matches!(segment,
                                    psi_terminal::StructuralPathSegment::Field(identity)
                                        if !identity.is_empty())
                            }) =>
                        {
                            path.is_empty()
                                || argument.root_structural_type == argument.structural_type
                                || argument.fixed_array_length.is_some()
                                || argument.element_stride.is_some()
                                || !argument
                                    .source_byte_offset
                                    .is_multiple_of(u32::from(argument.shape.alignment))
                        }
                        _ => true,
                    }
            })
        || projected_argument_indexes.iter().any(|index| {
            if transferred_argument_indexes.contains(index) {
                return false;
            }
            let Some(argument) = custody.arguments.get(*index) else {
                return true;
            };
            argument.path.is_empty()
                || affine_cleanup.is_none_or(|cleanup| {
                    !cleanup.actions.iter().any(|action| {
                        matches!(action,
                            psi_terminal::TerminalAffineCleanupAction::DiscardResidual(residual)
                                if residual.place == argument.place
                                    && !residual.path.is_empty()
                                    && !residual.path.starts_with(&argument.path)
                                    && !argument.path.starts_with(&residual.path)
                                    && residual.structural_type
                                        != argument.root_structural_type)
                    })
                })
        })
        || custody.claim_transfers.iter().any(|transfer| {
            usize::try_from(transfer.argument_index)
                .map_or(true, |index| index >= custody.arguments.len())
        })
        || custody
            .claim_transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != custody.claim_transfers.len()
    {
        return Err(invalid());
    }
    Ok(())
}

fn expected_projected_copy_bytes(
    target: NativeTarget,
    argument: &omega_terminal_machine_code::TerminalInternalUnitCallArgumentRecord,
) -> Option<Vec<u8>> {
    let [
        omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = argument.destination.locations.as_slice()
    else {
        return None;
    };
    if argument.shape != omega_calling_conventions::ValueShape::integer(8, 8) {
        return None;
    }
    let home = argument
        .call_stack_bytes
        .checked_add(argument.source_home_byte_offset)?;
    match target.architecture {
        Architecture::X86_64 => {
            let destination = x86_terminal_register(*register)?;
            let mut bytes = Vec::new();
            if matches!(
                argument.source.locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                expected_x86_stack_load(&mut bytes, 11, home, 8)?;
                expected_x86_memory_load(
                    &mut bytes,
                    destination,
                    11,
                    argument.source_byte_offset,
                    8,
                )?;
            } else {
                let offset = home.checked_add(argument.source_byte_offset)?;
                expected_x86_stack_load(&mut bytes, destination, offset, 8)?;
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let destination = aarch64_terminal_register(*register)?;
            let mut instructions = Vec::new();
            if matches!(
                argument.source.locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ) {
                instructions.push(expected_aarch64_stack_load(9, home, 8)?);
                instructions.push(expected_aarch64_memory_load(
                    destination,
                    9,
                    argument.source_byte_offset,
                    8,
                )?);
            } else {
                instructions.push(expected_aarch64_stack_load(
                    destination,
                    home.checked_add(argument.source_byte_offset)?,
                    8,
                )?);
            }
            Some(
                instructions
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
        }
    }
}

fn validate_unit_function_stack(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    evidence: TerminalUnitStackEvidence,
    frame_start: usize,
) -> Result<TerminalObjectUnitStack, TerminalObjectError> {
    if evidence.stack_alignment != 16 {
        return Err(TerminalObjectError::InvalidUnitStackAlignment {
            machine,
            alignment: evidence.stack_alignment,
        });
    }
    let frame_bytes = match evidence.frame {
        Some(frame) => {
            validate_stack_adjustment_pair(architecture, machine, None, bytes, frame)?;
            if frame.allocation_offset != frame_start {
                return Err(TerminalObjectError::InvalidUnitStackEncoding {
                    machine,
                    owner: None,
                    offset: frame.allocation_offset,
                });
            }
            frame.byte_size
        }
        None => 0,
    };
    match architecture {
        Architecture::X86_64 => {
            if evidence.aarch64_return_link.is_some()
                || evidence.frame.is_some_and(|frame| {
                    frame
                        .release_offset
                        .checked_add(frame.release_byte_count)
                        .and_then(|end| end.checked_add(1))
                        != Some(bytes.len())
                })
                || bytes.last() != Some(&0xc3)
            {
                return Err(TerminalObjectError::InvalidUnitStackEncoding {
                    machine,
                    owner: None,
                    offset: bytes.len().saturating_sub(1),
                });
            }
        }
        Architecture::Aarch64 => {
            let frame =
                evidence
                    .frame
                    .ok_or(TerminalObjectError::MissingAarch64UnitReturnLink {
                        caller: machine,
                        operation: None,
                    })?;
            let link = evidence.aarch64_return_link.ok_or(
                TerminalObjectError::MissingAarch64UnitReturnLink {
                    caller: machine,
                    operation: None,
                },
            )?;
            let expected_store = aarch64_unit_link_instruction(false, link.frame_byte_offset);
            let expected_load = aarch64_unit_link_instruction(true, link.frame_byte_offset);
            if frame_bytes < 16
                || link.store_offset != frame.allocation_offset + frame.allocation_byte_count
                || link.load_offset + 4 != frame.release_offset
                || frame.release_offset + frame.release_byte_count + 4 != bytes.len()
                || bytes.get(link.store_offset..link.store_offset + 4)
                    != Some(&expected_store.to_le_bytes())
                || bytes.get(link.load_offset..link.load_offset + 4)
                    != Some(&expected_load.to_le_bytes())
                || bytes.get(bytes.len().saturating_sub(4)..)
                    != Some(&0xd65f_03c0_u32.to_le_bytes())
            {
                return Err(TerminalObjectError::MissingAarch64UnitReturnLink {
                    caller: machine,
                    operation: None,
                });
            }
        }
    }
    Ok(TerminalObjectUnitStack {
        frame_bytes,
        local_peak_bytes: frame_bytes,
        stack_alignment: evidence.stack_alignment,
    })
}

fn checked_align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}

fn replay_structural_shape(
    structural_type: psi_core::StructuralTypeId,
    declarations: &std::collections::BTreeMap<
        psi_core::StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
    cache: &mut std::collections::BTreeMap<psi_core::StructuralTypeId, ValueShape>,
    active: &mut std::collections::BTreeSet<psi_core::StructuralTypeId>,
) -> Option<ValueShape> {
    if let Some(shape) = cache.get(&structural_type) {
        return Some(*shape);
    }
    if !active.insert(structural_type) {
        return None;
    }
    let declaration = declarations.get(&structural_type)?;
    let shape = match &declaration.shape {
        psi_terminal::StructuralTypeShape::Record { fields } => {
            let mut byte_size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let field_shape =
                    replay_structural_field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                byte_size = checked_align_up(byte_size, u32::from(field_shape.alignment))?
                    .checked_add(u32::from(field_shape.byte_size))?;
            }
            byte_size = checked_align_up(byte_size, u32::from(alignment))?;
            ValueShape::integer(u16::try_from(byte_size).ok()?, alignment)
        }
        psi_terminal::StructuralTypeShape::FixedArray { element, length } => {
            if *length == 0 {
                return None;
            }
            let element = replay_structural_shape(*element, declarations, cache, active)?;
            let stride =
                checked_align_up(u32::from(element.byte_size), u32::from(element.alignment))?;
            let byte_size = u64::from(stride)
                .checked_mul(*length)
                .and_then(|size| u16::try_from(size).ok())?;
            ValueShape::integer(byte_size, element.alignment)
        }
        psi_terminal::StructuralTypeShape::Sum { .. } => return None,
    };
    active.remove(&structural_type);
    cache.insert(structural_type, shape);
    Some(shape)
}

fn replay_structural_field_shape(
    field_type: &psi_terminal::StructuralFieldType,
    declarations: &std::collections::BTreeMap<
        psi_core::StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
    cache: &mut std::collections::BTreeMap<psi_core::StructuralTypeId, ValueShape>,
    active: &mut std::collections::BTreeSet<psi_core::StructuralTypeId>,
) -> Option<ValueShape> {
    match field_type {
        psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean) => {
            Some(ValueShape::integer(1, 1))
        }
        psi_terminal::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        psi_terminal::StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        psi_terminal::StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
        psi_terminal::StructuralFieldType::ByteSequence(carrier) => {
            let byte_size = match carrier {
                psi_terminal::ByteSequenceCarrier::BorrowedView => 16,
                psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    capacity.checked_add(8)?.try_into().ok()?
                }
            };
            Some(ValueShape::integer(byte_size, 8))
        }
        psi_terminal::StructuralFieldType::Structural(nested) => {
            replay_structural_shape(*nested, declarations, cache, active)
        }
        psi_terminal::StructuralFieldType::Erased { .. } => None,
    }
}

fn replay_boolean_field_offset(
    structural_type: psi_core::StructuralTypeId,
    field: psi_core::StructuralFieldId,
    declarations: &std::collections::BTreeMap<
        psi_core::StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
) -> Option<(u32, ValueShape)> {
    let declaration = declarations.get(&structural_type)?;
    let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    let mut offset = 0_u32;
    for candidate in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let shape = replay_structural_field_shape(
            &candidate.field_type,
            declarations,
            &mut cache,
            &mut active,
        )?;
        offset = checked_align_up(offset, u32::from(shape.alignment))?;
        if candidate.id == field {
            return matches!(
                candidate.field_type,
                psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean)
            )
            .then_some((
                offset,
                replay_structural_shape(structural_type, declarations, &mut cache, &mut active)?,
            ));
        }
        offset = offset.checked_add(u32::from(shape.byte_size))?;
    }
    None
}

fn condition_stack_depth_before(
    evidence: &TerminalScalarStackEvidence,
    condition_start: usize,
    read_start: usize,
) -> Option<u32> {
    let mut depth = 0_u32;
    for mutation in evidence
        .mutations
        .iter()
        .filter(|mutation| mutation.offset >= condition_start && mutation.offset < read_start)
    {
        if mutation.offset.checked_add(mutation.byte_count)? > read_start {
            return None;
        }
        match mutation.kind {
            TerminalScalarStackMutationKind::Allocate { byte_size } => {
                depth = depth.checked_add(byte_size)?;
            }
            TerminalScalarStackMutationKind::Release { byte_size }
            | TerminalScalarStackMutationKind::X86ReleasePreservingFlags { byte_size } => {
                depth = depth.checked_sub(byte_size)?;
            }
            TerminalScalarStackMutationKind::X86Push => depth = depth.checked_add(8)?,
            TerminalScalarStackMutationKind::X86Pop => depth = depth.checked_sub(8)?,
        }
    }
    Some(depth)
}

fn replay_x86_boolean_structural_read(
    placement: &ValuePlacement,
    field_byte_offset: u32,
    stack_depth: u32,
) -> Option<Vec<u8>> {
    if field_byte_offset >= u32::from(placement.shape.byte_size) {
        return None;
    }
    let mut bytes = Vec::new();
    if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
        let base = match *pointer {
            IndirectPointerLocation::Register(register) => {
                let base = x86_terminal_register(register)?;
                (base != 0).then_some(base)?
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                let incoming = stack_byte_offset.checked_add(8)?.checked_add(stack_depth)?;
                x86_replay_rsp_load(&mut bytes, 11, incoming, 8)?;
                11
            }
        };
        x86_replay_memory_load(&mut bytes, 0, base, field_byte_offset);
    } else {
        let location = placement.locations.iter().find(|location| match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => {
                let start = u32::from(*value_byte_offset);
                field_byte_offset >= start && field_byte_offset < start + u32::from(*byte_size)
            }
            ValueLocation::Indirect { .. } => false,
        })?;
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                ..
            } => {
                let register = x86_terminal_register(register)?;
                if register == 0 {
                    return None;
                }
                bytes.extend_from_slice(&[
                    0x48 | (((register >> 3) & 1) << 2),
                    0x89,
                    0xc0 | ((register & 7) << 3),
                ]);
                let shift = (field_byte_offset - u32::from(value_byte_offset)) * 8;
                if shift != 0 {
                    bytes.extend_from_slice(&[0x48, 0xc1, 0xe8, shift as u8]);
                }
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                ..
            } => {
                let incoming = stack_byte_offset
                    .checked_add(field_byte_offset - u32::from(value_byte_offset))?
                    .checked_add(8)?
                    .checked_add(stack_depth)?;
                x86_replay_rsp_load(&mut bytes, 0, incoming, 1)?;
            }
            ValueLocation::Indirect { .. } => return None,
        }
    }
    bytes.extend_from_slice(&[0x83, 0xe0, 0x01]);
    Some(bytes)
}

fn replay_aarch64_boolean_structural_read(
    placement: &ValuePlacement,
    field_byte_offset: u32,
    stack_depth: u32,
) -> Option<Vec<u8>> {
    if field_byte_offset >= u32::from(placement.shape.byte_size) {
        return None;
    }
    let mut instructions = Vec::new();
    if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
        let base = match *pointer {
            IndirectPointerLocation::Register(register) => {
                let base = aarch64_terminal_register(register)?;
                (base != 0).then_some(base)?
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                let incoming = stack_depth.checked_add(stack_byte_offset)?;
                instructions.push(aarch64_replay_stack_load(9, incoming, 8)?);
                9
            }
        };
        instructions.push(aarch64_replay_memory_load(0, base, field_byte_offset)?);
    } else {
        let location = placement.locations.iter().find(|location| match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => {
                let start = u32::from(*value_byte_offset);
                field_byte_offset >= start && field_byte_offset < start + u32::from(*byte_size)
            }
            ValueLocation::Indirect { .. } => false,
        })?;
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                ..
            } => {
                let register = aarch64_terminal_register(register)?;
                if register == 0 {
                    return None;
                }
                let shift = (field_byte_offset - u32::from(value_byte_offset)) * 8;
                instructions.push(0xd340_fc00 | (shift << 16) | (u32::from(register) << 5));
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                ..
            } => {
                let incoming = stack_depth
                    .checked_add(stack_byte_offset)?
                    .checked_add(field_byte_offset - u32::from(value_byte_offset))?;
                instructions.push(aarch64_replay_stack_load(0, incoming, 1)?);
            }
            ValueLocation::Indirect { .. } => return None,
        }
    }
    instructions.push(0x1200_0000);
    Some(
        instructions
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    )
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

fn validate_scalar_cleanup_preservation(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    stack: &TerminalScalarStackEvidence,
    cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
) -> Result<(), TerminalObjectError> {
    let (Some(cleanup), Some(preservation)) = (cleanup, stack.cleanup_preservation) else {
        return if cleanup.is_none() && stack.cleanup_preservation.is_none() {
            Ok(())
        } else {
            Err(TerminalObjectError::InvalidUnitAffineCleanupEvidence(
                machine,
            ))
        };
    };
    validate_scalar_cleanup_preservation_record(
        architecture,
        machine,
        bytes,
        cleanup,
        preservation,
        bytes.len(),
    )
}

fn validate_scalar_control_cleanup_evidence(
    architecture: Architecture,
    machine: MachineId,
    provenance: &TerminalPsiProvenance,
    bytes: &[u8],
    stack: &TerminalScalarStackEvidence,
    records: &[TerminalScalarControlAffineCleanupRecord],
) -> Result<(), TerminalObjectError> {
    let invalid = || TerminalObjectError::InvalidUnitAffineCleanupEvidence(machine);
    let TerminalScalarControlFlowEvidence::ConditionalTree {
        ref decisions,
        ref crash_leaves,
        ref branches,
    } = stack.control_flow
    else {
        return if records.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    };
    if records.is_empty() {
        return if stack.cleanup_preservation.is_none() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    if !branches.is_empty() {
        return Err(invalid());
    }
    if crash_leaves.iter().any(|crash| *crash) {
        return Err(invalid());
    }
    let mut prefixes = Vec::new();
    let mut leaf_regions = Vec::new();
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        0,
        bytes.len(),
        decisions,
        &mut prefixes,
        &mut leaf_regions,
    )?;
    if records.len() != leaf_regions.len()
        || crash_leaves.len() != leaf_regions.len()
        || stack.cleanup_preservation.is_some()
    {
        return Err(invalid());
    }
    let mut edges = std::collections::BTreeSet::new();
    for (record, (leaf_start, leaf_end)) in records.iter().zip(leaf_regions) {
        let cleanup = &record.cleanup;
        let cleanup_end = cleanup
            .code_offset
            .checked_add(cleanup.byte_count)
            .ok_or_else(invalid)?;
        if cleanup.code_offset < leaf_start
            || cleanup_end != leaf_end
            || !edges.insert(cleanup.psi_edge)
        {
            return Err(invalid());
        }
        let position = provenance
            .edges
            .iter()
            .position(|edge| *edge == cleanup.psi_edge)
            .ok_or_else(invalid)?;
        if provenance.edges[position + 1..].contains(&cleanup.psi_edge) {
            return Err(invalid());
        }
        validate_scalar_cleanup_preservation_record(
            architecture,
            machine,
            bytes,
            cleanup,
            record.preservation,
            leaf_end,
        )?;
    }
    if records.windows(2).any(|pair| {
        pair[0]
            .cleanup
            .code_offset
            .checked_add(pair[0].cleanup.byte_count)
            .is_none_or(|end| end > pair[1].cleanup.code_offset)
    }) || records[1..].iter().any(|record| {
        record.cleanup.structural_types != records[0].cleanup.structural_types
            || record.cleanup.locals != records[0].cleanup.locals
            || record.cleanup.actions != records[0].cleanup.actions
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn cleanup_for_owner<'a>(
    records: &'a [TerminalScalarControlAffineCleanupRecord],
    owner: TerminalCallSiteOwner,
) -> Option<&'a omega_terminal_machine_code::TerminalUnitAffineCleanupRecord> {
    let TerminalCallSiteOwner::CleanupAction { edge, .. } = owner else {
        return None;
    };
    records
        .iter()
        .find(|record| record.cleanup.psi_edge == edge)
        .map(|record| &record.cleanup)
}

fn validate_scalar_cleanup_preservation_record(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    cleanup: &omega_terminal_machine_code::TerminalUnitAffineCleanupRecord,
    preservation: omega_terminal_machine_code::TerminalScalarCleanupPreservationEvidence,
    expected_end: usize,
) -> Result<(), TerminalObjectError> {
    let invalid = || TerminalObjectError::InvalidUnitAffineCleanupEvidence(machine);
    let exact_at = |offset: usize, expected: &[u8]| {
        bytes
            .get(offset..)
            .and_then(|tail| tail.get(..expected.len()))
            == Some(expected)
    };
    let frame = preservation.frame;
    let cleanup_end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or_else(invalid)?;
    validate_stack_adjustment_pair(architecture, machine, None, bytes, frame)
        .map_err(|_| invalid())?;
    if cleanup_end != expected_end
        || frame.byte_size != 16
        || frame.allocation_offset != cleanup.code_offset
        || preservation.result_byte_offset != 0
    {
        return Err(invalid());
    }
    match architecture {
        Architecture::X86_64 => {
            let store = [0x48, 0x89, 0x44, 0x24, 0x00];
            let load = [0x48, 0x8b, 0x44, 0x24, 0x00];
            let allocation_end = frame
                .allocation_offset
                .checked_add(frame.allocation_byte_count)
                .ok_or_else(invalid)?;
            let load_end = preservation
                .result_load_offset
                .checked_add(load.len())
                .ok_or_else(invalid)?;
            let release_end = frame
                .release_offset
                .checked_add(frame.release_byte_count)
                .and_then(|end| end.checked_add(1))
                .ok_or_else(invalid)?;
            if preservation.aarch64_return_link.is_some()
                || preservation.result_store_offset != allocation_end
                || !exact_at(preservation.result_store_offset, &store)
                || load_end != frame.release_offset
                || !exact_at(preservation.result_load_offset, &load)
                || release_end != expected_end
                || bytes.get(expected_end.saturating_sub(1)) != Some(&0xc3)
            {
                return Err(invalid());
            }
        }
        Architecture::Aarch64 => {
            let Some(link) = preservation.aarch64_return_link else {
                return Err(invalid());
            };
            let result_store = 0xf900_03e0_u32.to_le_bytes();
            let link_store = 0xf900_07fe_u32.to_le_bytes();
            let result_load = 0xf940_03e0_u32.to_le_bytes();
            let link_load = 0xf940_07fe_u32.to_le_bytes();
            let allocation_store = frame.allocation_offset.checked_add(4).ok_or_else(invalid)?;
            let link_store_offset = preservation
                .result_store_offset
                .checked_add(4)
                .ok_or_else(invalid)?;
            let link_load_offset = preservation
                .result_load_offset
                .checked_add(4)
                .ok_or_else(invalid)?;
            let release_offset = link.load_offset.checked_add(4).ok_or_else(invalid)?;
            let terminal_end = frame.release_offset.checked_add(8).ok_or_else(invalid)?;
            if preservation.result_store_offset != allocation_store
                || !exact_at(preservation.result_store_offset, &result_store)
                || link.frame_byte_offset != 8
                || link.store_offset != link_store_offset
                || !exact_at(link.store_offset, &link_store)
                || !exact_at(preservation.result_load_offset, &result_load)
                || link.load_offset != link_load_offset
                || !exact_at(link.load_offset, &link_load)
                || frame.release_offset != release_offset
                || terminal_end != expected_end
                || !exact_at(
                    expected_end.saturating_sub(4),
                    &0xd65f_03c0_u32.to_le_bytes(),
                )
            {
                return Err(invalid());
            }
        }
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

fn division_branches_in_region(
    branches: &[TerminalScalarDivisionBranchEvidence],
    start: usize,
    end: usize,
) -> &[TerminalScalarDivisionBranchEvidence] {
    let first = branches.partition_point(|branch| branch.branch_offset < start);
    let last = branches.partition_point(|branch| branch.branch_offset < end);
    &branches[first..last]
}

fn validate_division_branch_regions(
    machine: MachineId,
    branches: &[TerminalScalarDivisionBranchEvidence],
    regions: &[(usize, usize)],
) -> Result<(), TerminalObjectError> {
    for pair in branches.windows(2) {
        if pair[0].branch_offset >= pair[1].branch_offset {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: pair[1].branch_offset,
            });
        }
    }
    if let Some(branch) = branches.iter().find(|branch| {
        !regions
            .iter()
            .any(|(start, end)| *start <= branch.branch_offset && branch.branch_offset < *end)
    }) {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: branch.branch_offset,
        });
    }
    Ok(())
}

fn collect_conditional_tree_regions(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    start: usize,
    end: usize,
    decisions: &[TerminalScalarConditionalBranchEvidence],
    prefixes: &mut Vec<(usize, usize, TerminalScalarConditionalCondition)>,
    leaves: &mut Vec<(usize, usize)>,
) -> Result<(), TerminalObjectError> {
    let Some((root, descendants)) = decisions.split_first() else {
        if start >= end {
            return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
                machine,
                offset: start,
            });
        }
        leaves.push((start, end));
        return Ok(());
    };
    let branch_end = root
        .branch_offset
        .checked_add(root.branch_byte_count)
        .ok_or(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: root.branch_offset,
        })?;
    if root.branch_offset < start
        || branch_end >= root.false_arm_offset
        || root.false_arm_offset >= end
    {
        return Err(TerminalObjectError::InvalidScalarConditionalEvidence {
            machine,
            offset: root.branch_offset,
        });
    }
    validate_scalar_conditional_branch(
        architecture,
        root.condition,
        machine,
        bytes,
        root.branch_offset,
        root.branch_byte_count,
        root.false_arm_offset,
    )?;
    prefixes.push((start, root.branch_offset, root.condition));
    let true_decision_count =
        descendants.partition_point(|branch| branch.branch_offset < root.false_arm_offset);
    let (true_decisions, false_decisions) = descendants.split_at(true_decision_count);
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        branch_end,
        root.false_arm_offset,
        true_decisions,
        prefixes,
        leaves,
    )?;
    collect_conditional_tree_regions(
        architecture,
        machine,
        bytes,
        root.false_arm_offset,
        end,
        false_decisions,
        prefixes,
        leaves,
    )
}

fn conditional_call_path(
    architecture: Architecture,
    bytes: &[u8],
    stack: Option<&TerminalScalarStackEvidence>,
    call: &omega_terminal_machine_code::TerminalInternalCallRelocation,
) -> Option<Vec<(usize, bool)>> {
    let TerminalScalarControlFlowEvidence::ConditionalTree { decisions, .. } = &stack?.control_flow
    else {
        return None;
    };
    let call_offset = match architecture {
        Architecture::X86_64 => call.offset.checked_sub(1)?,
        Architecture::Aarch64 => call.offset,
    };
    if call_offset >= bytes.len() {
        return None;
    }
    conditional_call_path_in_region(call_offset, 0, bytes.len(), decisions, &mut Vec::new())
}

fn conditional_call_path_in_region(
    call_offset: usize,
    start: usize,
    end: usize,
    decisions: &[TerminalScalarConditionalBranchEvidence],
    path: &mut Vec<(usize, bool)>,
) -> Option<Vec<(usize, bool)>> {
    let Some((root, descendants)) = decisions.split_first() else {
        return (start <= call_offset && call_offset < end).then(|| path.clone());
    };
    if start <= call_offset && call_offset < root.branch_offset {
        return Some(path.clone());
    }
    let branch_end = root.branch_offset.checked_add(root.branch_byte_count)?;
    let true_decision_count =
        descendants.partition_point(|branch| branch.branch_offset < root.false_arm_offset);
    let (true_decisions, false_decisions) = descendants.split_at(true_decision_count);
    if branch_end <= call_offset && call_offset < root.false_arm_offset {
        path.push((root.branch_offset, true));
        let result = conditional_call_path_in_region(
            call_offset,
            branch_end,
            root.false_arm_offset,
            true_decisions,
            path,
        );
        path.pop();
        return result;
    }
    if root.false_arm_offset <= call_offset && call_offset < end {
        path.push((root.branch_offset, false));
        let result = conditional_call_path_in_region(
            call_offset,
            root.false_arm_offset,
            end,
            false_decisions,
            path,
        );
        path.pop();
        return result;
    }
    None
}

fn conditional_paths_are_exclusive(left: &[(usize, bool)], right: &[(usize, bool)]) -> bool {
    left.iter().any(|(left_decision, left_outcome)| {
        right.iter().any(|(right_decision, right_outcome)| {
            left_decision == right_decision && left_outcome != right_outcome
        })
    })
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

fn validate_scalar_conditional_branch(
    architecture: Architecture,
    condition: TerminalScalarConditionalCondition,
    machine: MachineId,
    bytes: &[u8],
    branch_offset: usize,
    branch_byte_count: usize,
    false_arm_offset: usize,
) -> Result<(), TerminalObjectError> {
    let invalid = || TerminalObjectError::InvalidScalarConditionalEvidence {
        machine,
        offset: branch_offset,
    };
    let target = match architecture {
        Architecture::X86_64 => {
            if branch_byte_count != 6
                || bytes.get(branch_offset..branch_offset.saturating_add(2)) != Some(&[0x0f, 0x84])
            {
                return Err(invalid());
            }
            let displacement = bytes
                .get(branch_offset + 2..branch_offset + 6)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(i32::from_le_bytes)
                .ok_or_else(invalid)?;
            i64::try_from(branch_offset + branch_byte_count)
                .ok()
                .and_then(|base| base.checked_add(i64::from(displacement)))
                .and_then(|target| usize::try_from(target).ok())
                .ok_or_else(invalid)?
        }
        Architecture::Aarch64 => {
            if branch_byte_count != 4 || !branch_offset.is_multiple_of(4) {
                return Err(invalid());
            }
            let encoded = bytes
                .get(branch_offset..branch_offset + 4)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(u32::from_le_bytes)
                .ok_or_else(invalid)?;
            match condition {
                TerminalScalarConditionalCondition::Parameter
                    if encoded & 0xff00_0000 != 0x3400_0000 =>
                {
                    return Err(invalid());
                }
                TerminalScalarConditionalCondition::Expression
                    if encoded & 0xff00_001f != 0x5400_0000 =>
                {
                    return Err(invalid());
                }
                _ => {}
            }
            let immediate = ((encoded >> 5) & 0x7ffff) as i32;
            let displacement = (immediate << 13 >> 13) * 4;
            i64::try_from(branch_offset)
                .ok()
                .and_then(|base| base.checked_add(i64::from(displacement)))
                .and_then(|target| usize::try_from(target).ok())
                .ok_or_else(invalid)?
        }
    };
    if target != false_arm_offset {
        return Err(invalid());
    }
    Ok(())
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

fn validate_scalar_call_stack(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    relocation: omega_terminal_machine_code::TerminalInternalCallRelocation,
    call: TerminalScalarCallStackEvidence,
    function: &TerminalScalarStackEvidence,
    replay_depth: u32,
    scalar_affine_cleanup: Option<&omega_terminal_machine_code::TerminalUnitAffineCleanupRecord>,
) -> Result<TerminalObjectScalarCallStack, TerminalObjectError> {
    let owner = relocation.owner;
    let (call_start, call_end) = match architecture {
        Architecture::X86_64 => (relocation.offset - 1, relocation.offset + 4),
        Architecture::Aarch64 => (relocation.offset, relocation.offset + 4),
    };
    if let Some(outbound) = call.outbound {
        validate_stack_adjustment_pair(architecture, caller, Some(owner), bytes, outbound)
            .map_err(|_| TerminalObjectError::InvalidScalarCallStackEvidence {
                caller,
                owner,
                offset: outbound.allocation_offset,
            })?;
        let allocation = function
            .mutations
            .iter()
            .find(|mutation| mutation.offset == outbound.allocation_offset);
        let release = function
            .mutations
            .iter()
            .find(|mutation| mutation.offset == outbound.release_offset);
        if allocation.is_none_or(|mutation| {
            mutation.byte_count != outbound.allocation_byte_count
                || mutation.kind
                    != TerminalScalarStackMutationKind::Allocate {
                        byte_size: outbound.byte_size,
                    }
        }) || release.is_none_or(|mutation| {
            mutation.byte_count != outbound.release_byte_count
                || mutation.kind
                    != TerminalScalarStackMutationKind::Release {
                        byte_size: outbound.byte_size,
                    }
        }) {
            return Err(TerminalObjectError::InvalidScalarCallStackEvidence {
                caller,
                owner,
                offset: outbound.allocation_offset,
            });
        }
        let allocation_end = outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or(TerminalObjectError::ScalarStackArithmeticOverflow(caller))?;
        if allocation_end > call_start
            || (architecture == Architecture::X86_64 && outbound.release_offset != call_end)
        {
            return Err(TerminalObjectError::InvalidScalarCallStackEvidence {
                caller,
                owner,
                offset: outbound.allocation_offset,
            });
        }
    }
    match architecture {
        Architecture::X86_64 => {
            if call.aarch64_return_link.is_some() {
                return Err(TerminalObjectError::InvalidScalarCallStackEvidence {
                    caller,
                    owner,
                    offset: call_start,
                });
            }
        }
        Architecture::Aarch64 => {
            let cleanup_lifetime_link =
                matches!(owner, TerminalCallSiteOwner::CleanupAction { .. })
                    && call.outbound.is_none()
                    && call.aarch64_return_link.is_none()
                    && replay_depth >= function.stack_alignment
                    && replay_depth.is_multiple_of(function.stack_alignment)
                    && scalar_affine_cleanup.is_some_and(|cleanup| {
                        cleanup.code_offset <= call_start
                            && cleanup
                                .code_offset
                                .checked_add(cleanup.byte_count)
                                .is_some_and(|end| call_end <= end)
                    });
            if cleanup_lifetime_link {
                // The composite scalar-return carrier keeps X30 in its
                // function-lifetime cleanup frame, so this call requires no
                // second per-call link slot.
            } else {
                let outbound =
                    call.outbound
                        .ok_or(TerminalObjectError::InvalidScalarCallStackEvidence {
                            caller,
                            owner,
                            offset: call_start,
                        })?;
                let link = call.aarch64_return_link.ok_or(
                    TerminalObjectError::InvalidScalarCallStackEvidence {
                        caller,
                        owner,
                        offset: call_start,
                    },
                )?;
                let link_end = link
                    .frame_byte_offset
                    .checked_add(8)
                    .ok_or(TerminalObjectError::ScalarStackArithmeticOverflow(caller))?;
                let link_area_end = link
                    .frame_byte_offset
                    .checked_add(16)
                    .ok_or(TerminalObjectError::ScalarStackArithmeticOverflow(caller))?;
                let allocation_end = outbound.allocation_offset + outbound.allocation_byte_count;
                if !link.frame_byte_offset.is_multiple_of(8)
                    || link_end > outbound.byte_size
                    || link_area_end != outbound.byte_size
                    || link.store_offset != allocation_end
                    || link.store_offset >= call_start
                    || link.load_offset != call_end
                    || outbound.release_offset != link.load_offset + 4
                    || bytes.get(link.store_offset..link.store_offset + 4)
                        != Some(
                            &aarch64_unit_link_instruction(false, link.frame_byte_offset)
                                .to_le_bytes(),
                        )
                    || bytes.get(link.load_offset..link.load_offset + 4)
                        != Some(
                            &aarch64_unit_link_instruction(true, link.frame_byte_offset)
                                .to_le_bytes(),
                        )
                {
                    return Err(TerminalObjectError::InvalidScalarCallStackEvidence {
                        caller,
                        owner,
                        offset: link.store_offset,
                    });
                }
            }
        }
    }
    let caller_live_bytes = replay_depth
        .checked_add(if architecture == Architecture::X86_64 {
            8
        } else {
            0
        })
        .ok_or(TerminalObjectError::ScalarStackArithmeticOverflow(caller))?;
    if !caller_live_bytes.is_multiple_of(function.stack_alignment) {
        return Err(TerminalObjectError::MisalignedScalarCalleeEntry {
            caller,
            owner,
            caller_live_bytes,
        });
    }
    Ok(TerminalObjectScalarCallStack {
        owner,
        target: relocation.target,
        text_offset: relocation.offset,
        caller_live_bytes,
    })
}

fn validate_x86_scalar_mutation(
    machine: MachineId,
    bytes: &[u8],
    instruction: &iced_x86::Instruction,
    mutation: TerminalScalarStackMutation,
) -> Result<(), TerminalObjectError> {
    let offset = mutation.offset;
    let exact = match mutation.kind {
        TerminalScalarStackMutationKind::Allocate { byte_size } => {
            instruction.mnemonic() == iced_x86::Mnemonic::Sub
                && instruction.op0_register() == iced_x86::Register::RSP
                && bytes.get(offset..offset.saturating_add(instruction.len()))
                    == Some(x86_64_stack_adjustment(byte_size, false).as_slice())
        }
        TerminalScalarStackMutationKind::Release { byte_size } => {
            instruction.mnemonic() == iced_x86::Mnemonic::Add
                && instruction.op0_register() == iced_x86::Register::RSP
                && bytes.get(offset..offset.saturating_add(instruction.len()))
                    == Some(x86_64_stack_adjustment(byte_size, true).as_slice())
        }
        TerminalScalarStackMutationKind::X86ReleasePreservingFlags { byte_size } => {
            instruction.mnemonic() == iced_x86::Mnemonic::Lea
                && instruction.op0_register() == iced_x86::Register::RSP
                && bytes.get(offset..offset.saturating_add(instruction.len()))
                    == Some(x86_64_stack_release_preserving_flags(byte_size).as_slice())
        }
        TerminalScalarStackMutationKind::X86Push => {
            instruction.mnemonic() == iced_x86::Mnemonic::Push
                && instruction.op0_kind() == iced_x86::OpKind::Register
        }
        TerminalScalarStackMutationKind::X86Pop => {
            instruction.mnemonic() == iced_x86::Mnemonic::Pop
                && instruction.op0_kind() == iced_x86::OpKind::Register
        }
    };
    if !exact || mutation.byte_count != instruction.len() {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    Ok(())
}

fn validate_aarch64_scalar_mutation(
    machine: MachineId,
    encoded: u32,
    mutation: TerminalScalarStackMutation,
) -> Result<(), TerminalObjectError> {
    let expected = match mutation.kind {
        TerminalScalarStackMutationKind::Allocate { byte_size }
            if byte_size <= 0xfff && byte_size.is_multiple_of(16) =>
        {
            Some(0xd100_03ff | (byte_size << 10))
        }
        TerminalScalarStackMutationKind::Release { byte_size }
            if byte_size <= 0xfff && byte_size.is_multiple_of(16) =>
        {
            Some(0x9100_03ff | (byte_size << 10))
        }
        _ => None,
    };
    if mutation.byte_count != 4 || expected != Some(encoded) {
        return Err(TerminalObjectError::InvalidScalarStackEvidence {
            machine,
            offset: mutation.offset,
        });
    }
    Ok(())
}

fn replay_scalar_mutation(
    machine: MachineId,
    offset: usize,
    kind: TerminalScalarStackMutationKind,
    depth: &mut u32,
    peak: &mut u32,
) -> Result<(), TerminalObjectError> {
    let (allocate, byte_size) = match kind {
        TerminalScalarStackMutationKind::Allocate { byte_size } => (true, byte_size),
        TerminalScalarStackMutationKind::Release { byte_size } => (false, byte_size),
        TerminalScalarStackMutationKind::X86ReleasePreservingFlags { byte_size } => {
            (false, byte_size)
        }
        TerminalScalarStackMutationKind::X86Push => (true, 8),
        TerminalScalarStackMutationKind::X86Pop => (false, 8),
    };
    if byte_size == 0 {
        return Err(TerminalObjectError::InvalidScalarStackEvidence { machine, offset });
    }
    if allocate {
        *depth = depth
            .checked_add(byte_size)
            .ok_or(TerminalObjectError::ScalarStackArithmeticOverflow(machine))?;
        *peak = (*peak).max(*depth);
    } else {
        *depth = depth
            .checked_sub(byte_size)
            .ok_or(TerminalObjectError::ScalarStackReleaseExceedsAllocation { machine, offset })?;
    }
    Ok(())
}

fn aarch64_control_flow_instruction(encoded: u32) -> bool {
    (encoded & 0x7c00_0000) == 0x1400_0000
        || (encoded & 0xff00_0010) == 0x5400_0000
        || (encoded & 0x7e00_0000) == 0x3400_0000
        || (encoded & 0x7e00_0000) == 0x3600_0000
        || (encoded & 0xfe00_0000) == 0xd600_0000
        || (encoded & 0xff00_0000) == 0xd400_0000
}

fn aarch64_unsupported_sp_write(encoded: u32) -> bool {
    // ADD/SUB extended-register forms may name SP as destination. Scalar
    // emission never uses them for stack allocation.
    matches!(encoded & 0xff20_001f, 0x8b20_001f | 0xcb20_001f)
        // Single-register immediate pre/post-indexed loads and stores update
        // their SP base. Scalar emission uses only unsigned-offset accesses.
        || ((encoded & 0x3b20_0000) == 0x3800_0000
            && matches!((encoded >> 10) & 3, 1 | 3)
            && ((encoded >> 5) & 31) == 31)
        // Pair pre/post-indexed loads and stores likewise update the base.
        || ((encoded & 0x3a00_0000) == 0x2800_0000
            && matches!((encoded >> 23) & 3, 1 | 3)
            && ((encoded >> 5) & 31) == 31)
}

fn validate_unit_call_stack(
    architecture: Architecture,
    caller: MachineId,
    bytes: &[u8],
    relocation: omega_terminal_machine_code::TerminalInternalCallRelocation,
    function_evidence: TerminalUnitStackEvidence,
    function: TerminalObjectUnitStack,
    call: TerminalUnitCallStackEvidence,
) -> Result<TerminalObjectUnitCallStack, TerminalObjectError> {
    validate_internal_call_site(architecture, caller, bytes, relocation)?;
    let owner = relocation.owner;
    let outbound_bytes = match call.outbound {
        Some(outbound) => {
            validate_stack_adjustment_pair(architecture, caller, Some(owner), bytes, outbound)?;
            outbound.byte_size
        }
        None => 0,
    };
    let (call_start, call_end, linkage_bytes) = match architecture {
        Architecture::X86_64 => (
            relocation.offset.saturating_sub(1),
            relocation.offset.saturating_add(4),
            8,
        ),
        Architecture::Aarch64 => (relocation.offset, relocation.offset.saturating_add(4), 0),
    };
    if architecture == Architecture::X86_64 && call.outbound.is_none() {
        return Err(TerminalObjectError::MissingX86UnitCallStackAdjustment { caller, owner });
    }
    if let Some(outbound) = call.outbound {
        let allocation_end = outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or(TerminalObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
        let frame_release = function_evidence.frame.map(|frame| frame.release_offset);
        if allocation_end > call_start
            || outbound.release_offset != call_end
            || frame_release.is_some_and(|release| {
                outbound
                    .release_offset
                    .checked_add(outbound.release_byte_count)
                    .is_none_or(|end| end > release)
            })
        {
            return Err(TerminalObjectError::InvalidUnitStackEncoding {
                machine: caller,
                owner: Some(owner),
                offset: outbound.allocation_offset,
            });
        }
    }
    let transient_bytes = outbound_bytes
        .checked_add(linkage_bytes)
        .ok_or(TerminalObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
    let caller_live_bytes = function
        .frame_bytes
        .checked_add(transient_bytes)
        .ok_or(TerminalObjectError::UnitCallStackArithmeticOverflow { caller, owner })?;
    if !caller_live_bytes.is_multiple_of(function.stack_alignment) {
        return Err(TerminalObjectError::MisalignedUnitCalleeEntry {
            caller,
            owner,
            caller_live_bytes,
        });
    }
    Ok(TerminalObjectUnitCallStack {
        owner,
        target: relocation.target,
        text_offset: relocation.offset,
        active_frame_bytes: function.frame_bytes,
        transient_bytes,
        caller_live_bytes,
    })
}

fn validate_stack_adjustment_pair(
    architecture: Architecture,
    machine: MachineId,
    owner: Option<TerminalCallSiteOwner>,
    bytes: &[u8],
    pair: TerminalStackAdjustmentPair,
) -> Result<(), TerminalObjectError> {
    if pair.allocation_offset >= pair.release_offset {
        return Err(TerminalObjectError::InvalidUnitStackEncoding {
            machine,
            owner,
            offset: pair.allocation_offset,
        });
    }
    let (allocation, release) = match architecture {
        Architecture::X86_64 => (
            x86_64_stack_adjustment(pair.byte_size, false),
            x86_64_stack_adjustment(pair.byte_size, true),
        ),
        Architecture::Aarch64 => {
            if pair.byte_size > 0xfff {
                return Err(TerminalObjectError::InvalidUnitStackEncoding {
                    machine,
                    owner,
                    offset: pair.allocation_offset,
                });
            }
            (
                (0xd100_03ff_u32 | (pair.byte_size << 10))
                    .to_le_bytes()
                    .to_vec(),
                (0x9100_03ff_u32 | (pair.byte_size << 10))
                    .to_le_bytes()
                    .to_vec(),
            )
        }
    };
    if pair.byte_size == 0
        || pair.allocation_byte_count != allocation.len()
        || pair.release_byte_count != release.len()
        || bytes
            .get(pair.allocation_offset..pair.allocation_offset.saturating_add(allocation.len()))
            != Some(allocation.as_slice())
        || bytes.get(pair.release_offset..pair.release_offset.saturating_add(release.len()))
            != Some(release.as_slice())
    {
        return Err(TerminalObjectError::InvalidUnitStackEncoding {
            machine,
            owner,
            offset: pair.allocation_offset,
        });
    }
    Ok(())
}

fn validate_complete_unit_stack_evidence(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    function: TerminalUnitStackEvidence,
    calls: &[omega_terminal_machine_code::TerminalInternalCallRelocation],
) -> Result<(), TerminalObjectError> {
    let mut claimed = std::collections::BTreeMap::new();
    let mut claim_pair = |pair: TerminalStackAdjustmentPair| {
        claimed
            .insert(pair.allocation_offset, pair.allocation_byte_count)
            .is_none()
            && claimed
                .insert(pair.release_offset, pair.release_byte_count)
                .is_none()
    };
    if function.frame.is_some_and(|frame| !claim_pair(frame)) {
        return Err(TerminalObjectError::DuplicateUnitStackAdjustment(machine));
    }
    for call in calls {
        if let Some(outbound) = call.unit_stack.and_then(|stack| stack.outbound)
            && !claim_pair(outbound)
        {
            return Err(TerminalObjectError::DuplicateUnitStackAdjustment(machine));
        }
    }

    match architecture {
        Architecture::X86_64 => {
            let mut decoder =
                iced_x86::Decoder::with_ip(64, bytes, 0, iced_x86::DecoderOptions::NONE);
            let mut info_factory = iced_x86::InstructionInfoFactory::new();
            let call_starts = calls
                .iter()
                .map(|call| call.offset.saturating_sub(1))
                .collect::<std::collections::BTreeSet<_>>();
            while decoder.can_decode() {
                let instruction = decoder.decode();
                let offset = usize::try_from(instruction.ip()).expect("function-relative x86 IP");
                if instruction.is_invalid() {
                    return Err(TerminalObjectError::InvalidUnitInstructionEncoding {
                        machine,
                        offset,
                    });
                }
                if is_x86_64_rsp_adjustment(&instruction) {
                    if claimed.remove(&offset) != Some(instruction.len()) {
                        return Err(TerminalObjectError::UnclaimedUnitStackAdjustment {
                            machine,
                            offset,
                        });
                    }
                    continue;
                }
                let info = info_factory.info(&instruction);
                let writes_stack_pointer = info.used_registers().iter().any(|register| {
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
                });
                if writes_stack_pointer
                    && !is_expected_x86_64_linkage_instruction(
                        &instruction,
                        offset,
                        bytes.len(),
                        &call_starts,
                    )
                {
                    return Err(TerminalObjectError::UnclaimedUnitStackMutation {
                        machine,
                        offset,
                    });
                }
            }
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(TerminalObjectError::InvalidUnitInstructionEncoding {
                    machine,
                    offset: bytes.len() - (bytes.len() % 4),
                });
            }
            for offset in (0..bytes.len()).step_by(4) {
                if aarch64_stack_adjustment_at(bytes, offset) {
                    if claimed.remove(&offset) != Some(4) {
                        return Err(TerminalObjectError::UnclaimedUnitStackAdjustment {
                            machine,
                            offset,
                        });
                    }
                }
            }
        }
    }
    if let Some((offset, _)) = claimed.into_iter().next() {
        return Err(TerminalObjectError::InvalidUnitStackEncoding {
            machine,
            owner: None,
            offset,
        });
    }
    Ok(())
}

fn is_x86_64_rsp_adjustment(instruction: &iced_x86::Instruction) -> bool {
    matches!(
        instruction.mnemonic(),
        iced_x86::Mnemonic::Add | iced_x86::Mnemonic::Sub
    ) && instruction.op0_register() == iced_x86::Register::RSP
}

fn is_expected_x86_64_linkage_instruction(
    instruction: &iced_x86::Instruction,
    offset: usize,
    function_byte_count: usize,
    call_starts: &std::collections::BTreeSet<usize>,
) -> bool {
    match instruction.mnemonic() {
        iced_x86::Mnemonic::Call => call_starts.contains(&offset),
        iced_x86::Mnemonic::Ret => {
            offset.checked_add(instruction.len()) == Some(function_byte_count)
        }
        _ => false,
    }
}

fn aarch64_stack_adjustment_at(bytes: &[u8], offset: usize) -> bool {
    let Some(encoded) = bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_le_bytes)
    else {
        return false;
    };
    // ADD/SUB (immediate), 64-bit, without flags, whose destination register
    // is SP. This also catches the shifted-immediate form, which the Unit
    // emitter does not currently produce and therefore cannot claim.
    matches!(encoded & 0xff00_001f, 0xd100_001f | 0x9100_001f)
}

fn x86_64_stack_adjustment(byte_size: u32, add: bool) -> Vec<u8> {
    if byte_size <= i8::MAX as u32 {
        vec![0x48, 0x83, if add { 0xc4 } else { 0xec }, byte_size as u8]
    } else {
        let mut bytes = vec![0x48, 0x81, if add { 0xc4 } else { 0xec }];
        bytes.extend_from_slice(&byte_size.to_le_bytes());
        bytes
    }
}

fn x86_64_stack_release_preserving_flags(byte_size: u32) -> Vec<u8> {
    if byte_size <= i8::MAX as u32 {
        vec![0x48, 0x8d, 0x64, 0x24, byte_size as u8]
    } else {
        let mut bytes = vec![0x48, 0x8d, 0xa4, 0x24];
        bytes.extend_from_slice(&byte_size.to_le_bytes());
        bytes
    }
}

fn aarch64_unit_link_instruction(load: bool, byte_offset: u32) -> u32 {
    let base = if load { 0xf940_0000 } else { 0xf900_0000 };
    base | ((byte_offset / 8) << 10) | (31 << 5) | 30
}

/// Compose the exact caller-owned peaks retained by the target emitter for a
/// selected Unit entry. Sequential calls take a maximum; one active caller
/// prefix adds to the selected callee's peak. Cycles and any reachable
/// non-Unit function fail closed.
pub fn derive_terminal_unit_stack_demand(
    artifact: &TerminalObjectArtifact,
    entry: MachineId,
) -> Result<TerminalUnitStackDemand, TerminalObjectError> {
    derive_terminal_stack_demand(artifact, entry)
}

/// Compose byte-validated stack evidence for the currently admitted terminal
/// function slices. Unit and branch-free scalar functions retain the acyclic
/// internal-call closure.
pub fn derive_terminal_stack_demand(
    artifact: &TerminalObjectArtifact,
    entry: MachineId,
) -> Result<TerminalStackDemand, TerminalObjectError> {
    let functions = artifact
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    if !functions.contains_key(&entry) {
        return Err(TerminalObjectError::EntryFunctionMissing(entry));
    }
    let mut active = std::collections::BTreeSet::new();
    let mut memoized = std::collections::BTreeMap::new();
    let mut contributing_machines = std::collections::BTreeSet::new();
    let ceiling_bytes = derive_terminal_stack_peak(
        entry,
        &functions,
        &mut active,
        &mut memoized,
        &mut contributing_machines,
    )?;
    Ok(TerminalStackDemand {
        terminal_psi: artifact.terminal_psi,
        target: artifact.target,
        entry,
        ceiling_bytes,
        stack_alignment: 16,
        contributing_machines,
    })
}

fn derive_terminal_stack_peak(
    machine: MachineId,
    functions: &std::collections::BTreeMap<MachineId, &TerminalObjectFunction>,
    active: &mut std::collections::BTreeSet<MachineId>,
    memoized: &mut std::collections::BTreeMap<MachineId, u64>,
    contributing_machines: &mut std::collections::BTreeSet<MachineId>,
) -> Result<u64, TerminalObjectError> {
    if let Some(peak) = memoized.get(&machine) {
        contributing_machines.insert(machine);
        return Ok(*peak);
    }
    if !active.insert(machine) {
        return Err(TerminalObjectError::TerminalStackCycle(machine));
    }
    contributing_machines.insert(machine);
    let function =
        functions
            .get(&machine)
            .copied()
            .ok_or(TerminalObjectError::UnknownInternalCallTarget {
                caller: machine,
                target: machine,
            })?;
    let mut peak = match (function.unit_stack, function.scalar_stack) {
        (Some(_), Some(_)) => {
            return Err(TerminalObjectError::ConflictingTerminalStackEvidence(
                machine,
            ));
        }
        (Some(stack), None) => u64::from(stack.local_peak_bytes),
        (None, Some(stack)) => u64::from(stack.local_peak_bytes),
        (None, None) => return Err(TerminalObjectError::UnaccountedTerminalStack(machine)),
    };
    for call in &function.unit_call_stacks {
        let callee_peak = derive_terminal_stack_peak(
            call.target,
            functions,
            active,
            memoized,
            contributing_machines,
        )?;
        let caller_live = u64::from(call.caller_live_bytes);
        let composed = caller_live.checked_add(callee_peak).ok_or(
            TerminalObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            },
        )?;
        peak = peak.max(composed);
    }
    for call in &function.scalar_call_stacks {
        let callee_peak = derive_terminal_stack_peak(
            call.target,
            functions,
            active,
            memoized,
            contributing_machines,
        )?;
        let caller_live = u64::from(call.caller_live_bytes);
        let composed = caller_live.checked_add(callee_peak).ok_or(
            TerminalObjectError::TerminalStackCompositionOverflow {
                caller: machine,
                owner: call.owner,
            },
        )?;
        peak = peak.max(composed);
    }
    active.remove(&machine);
    memoized.insert(machine, peak);
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

pub fn emit_terminal_object_container(
    artifact: &TerminalObjectArtifact,
) -> TerminalObjectContainer {
    TerminalObjectContainer {
        terminal_psi: artifact.terminal_psi,
        output: emit_omega_object_container(ObjectContainerInput {
            target: artifact.target,
            object: &artifact.object,
            relocations: &artifact.relocations,
            text_bytes: &artifact.text_bytes,
            data_bytes: &[],
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObjectContainer {
    pub terminal_psi: TerminalPsiIdentity,
    pub output: ObjectContainerOutput,
}

pub fn can_emit_terminal_executable_image(target: NativeTarget) -> bool {
    target.pointer_size == 8
        && target.pointer_alignment == 8
        && matches!(
            (target.object_format, target.architecture),
            (ObjectFormat::Elf, Architecture::Aarch64)
                | (ObjectFormat::Elf, Architecture::X86_64)
                | (ObjectFormat::MachO, Architecture::Aarch64)
                | (ObjectFormat::Coff, Architecture::X86_64)
        )
}

/// Emit and validate one direct executable image.
///
/// The clean lane admits only typed internal-call relocations. Final-text
/// mutation outside their architecture-specific immediate bits, imports,
/// appended thunks, overlapping/missing function spans, and unclassified
/// executable bytes are hard failures.
pub fn emit_terminal_executable_image(
    artifact: &TerminalObjectArtifact,
    subsystem: u16,
) -> Result<TerminalExecutableImage, Diagnostic> {
    if !can_emit_terminal_executable_image(artifact.target) {
        return Err(Diagnostic::error(format!(
            "cannot emit terminal-Psi executable image for {:?}",
            artifact.target
        )));
    }
    let image = omega_image::build_final_image(FinalImageInput {
        target: artifact.target,
        object: &artifact.object,
        relocations: &artifact.relocations,
        text_bytes: &artifact.text_bytes,
        data_bytes: &[],
    });
    let output = match (artifact.target.object_format, artifact.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_image_elf::emit_elf_aarch64_executable(image)
        }
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_image_elf::emit_elf_x86_64_executable(image)
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            omega_image_macho::emit_macho_aarch64_executable(image)
        }
        (ObjectFormat::Coff, Architecture::X86_64) => {
            omega_image_pe::emit_pe_x86_64_executable(image, subsystem)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "cannot emit terminal-Psi executable image for {:?}",
                artifact.target
            )));
        }
    }?;
    let mut output = emitted_direct_executable_output(output);
    output.compiler_text_validation = Some(validate_terminal_image(artifact, &output)?);
    Ok(TerminalExecutableImage {
        terminal_psi: artifact.terminal_psi,
        target: artifact.target,
        subsystem: matches!(artifact.target.object_format, ObjectFormat::Coff).then_some(subsystem),
        functions: artifact.functions.clone(),
        fuel_attribution: artifact.fuel_attribution.clone(),
        port_effects: artifact.port_effects.clone(),
        boundary_settlements: artifact.boundary_settlements.clone(),
        output,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalExecutableImage {
    terminal_psi: TerminalPsiIdentity,
    target: NativeTarget,
    subsystem: Option<u16>,
    functions: Vec<TerminalObjectFunction>,
    fuel_attribution: Vec<TerminalObjectFuelAttribution>,
    port_effects: Vec<TerminalObjectPortEffect>,
    boundary_settlements: Vec<TerminalObjectBoundarySettlement>,
    output: EmittedImageOutput,
}

impl TerminalExecutableImage {
    pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
        self.terminal_psi
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    /// PE/COFF subsystem selected by the writer. Other formats carry no
    /// subsystem fact because the argument is not interpreted by their writer.
    pub const fn subsystem(&self) -> Option<u16> {
        self.subsystem
    }

    pub const fn output(&self) -> &EmittedImageOutput {
        &self.output
    }

    pub fn boundary_settlements(&self) -> &[TerminalObjectBoundarySettlement] {
        &self.boundary_settlements
    }

    pub fn functions(&self) -> &[TerminalObjectFunction] {
        &self.functions
    }

    pub fn port_effects(&self) -> &[TerminalObjectPortEffect] {
        &self.port_effects
    }

    pub fn fuel_attribution(&self) -> &[TerminalObjectFuelAttribution] {
        &self.fuel_attribution
    }
}

fn validate_terminal_image(
    artifact: &TerminalObjectArtifact,
    output: &EmittedImageOutput,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if output.final_image_imports != 0 {
        return Err(Diagnostic::error(
            "terminal-Psi internal-call image unexpectedly retained imports",
        ));
    }
    if output.final_image_relocations != artifact.relocations.record_count() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} relocation(s), expected {}",
            output.final_image_relocations,
            artifact.relocations.record_count()
        )));
    }
    if let Some(gap) = output.executable_regions.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi executable inventory left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    let compiler_regions = output
        .executable_regions
        .regions
        .iter()
        .filter(|region| region.origin == FinalExecutableRegionOrigin::CompilerFunction)
        .collect::<Vec<_>>();
    if compiler_regions.len() != artifact.functions.len() {
        return Err(Diagnostic::error(format!(
            "terminal-Psi image retained {} compiler function region(s), expected {}",
            compiler_regions.len(),
            artifact.functions.len()
        )));
    }
    for function in &artifact.functions {
        let symbol = omega_object_file::object_symbol_name(&artifact.object, function.symbol);
        let matching = compiler_regions
            .iter()
            .filter(|region| {
                region.symbol == symbol
                    && region.section_offset == function.text_offset
                    && region.byte_count == function.byte_count
            })
            .count();
        if matching != 1 {
            return Err(Diagnostic::error(format!(
                "terminal-Psi function {} must bind exactly one final executable region; found {matching}",
                function.machine
            )));
        }
    }
    validate_final_text_relocation_envelope(
        &artifact.text_bytes,
        &output.final_text_bytes,
        &artifact.relocations,
    )
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
