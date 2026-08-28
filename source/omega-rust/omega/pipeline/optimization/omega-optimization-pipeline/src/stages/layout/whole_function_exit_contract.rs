use std::collections::{BTreeMap, BTreeSet};

use omega_isa_aarch64::aarch64_preservation_convention_for_target;
use omega_isa_x86_64::x86_64_preservation_convention_for_target;
use omega_isa_x86_64::{
    X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
    X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET, X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
    X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT, X86_64StructuralUnitInternalControlFixup,
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_machine_optimizer::{
    Aarch64CbnzFusionIdentity, Aarch64CbnzInstructionDisposition, PhysicalOperandFootprint,
    PostAllocationMachineInstruction,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{
    PreservationConvention, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use omega_selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlanIdentity, SelectedTerminator,
    VirtualRegisterId,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{EdgeId, MachineId};
use sha2::{Digest, Sha256};

use crate::{
    OptimizedResolvedSelectedFormLayoutError, OptimizedX86BranchRelaxationError,
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, SelectedFormEncodingState,
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    StagedOptimizedX86BranchRelaxation, X86BranchRelaxationIdentity,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_x86_branch_relaxation,
};

const CONTRACT_SCHEMA: &[u8] = b"omega.terminal.whole-function-exit-contract.v6\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WholeFunctionExitContractIdentity([u8; 32]);

impl WholeFunctionExitContractIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionExitPolicy {
    SystemVAMD64FramelessLeafV1,
    MicrosoftX64FramelessLeafV1,
    Aapcs64FramelessLeafV1,
    DarwinAapcs64FramelessLeafV1,
    /// Exact Microsoft-x64 custody for one balanced structural Unit caller
    /// and its Unit leaf. This is deliberately not a frameless-leaf policy:
    /// the caller owns a canonical 72-byte outgoing frame around its call.
    MicrosoftX64BalancedStructuralUnitCallV1,
    /// Exact Microsoft-x64 custody for one structural-signature Unit leaf.
    /// The function owns no call frame and consists solely of its validated
    /// `ReturnUnit` encoding.
    MicrosoftX64FramelessStructuralUnitLeafV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionHardeningPolicy {
    NoAdditionalEntryExitHardeningV1,
}

/// Exact authority under which the final function-relative layout entered
/// whole-function exit validation. The baseline variant never admits a
/// transformed layout; the relaxation variant is available only through the
/// dedicated independently replayed x86 relaxation API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionExitLayoutCustody {
    BaselineNearLayoutV1,
    X86RelaxConditionalBranchesToRel8V1 {
        relaxation: X86BranchRelaxationIdentity,
    },
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
        fusion: Aarch64CbnzFusionIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WholeFunctionEntryAssumption {
    CallerReturnAddressAtStackPointerV1,
    CallerLinkRegisterV1 { link_register: RegisterViewId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionReturnMechanism {
    X86ActivationStackReturnV1 {
        stack_pointer: RegisterViewId,
        read_bytes: u16,
        pop_bytes: u16,
    },
    Aarch64LinkRegisterReturnV1 {
        stack_pointer: RegisterViewId,
        link_register: RegisterViewId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionReturnValueEvidence {
    UnitV1,
    ScalarI64V1 {
        virtual_register: VirtualRegisterId,
        view: RegisterViewId,
        units: Vec<RegisterUnitId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionReturnEvidence {
    pub block: SelectedBlockId,
    pub psi_return_edge: EdgeId,
    pub instruction: SelectedInstructionId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub value: WholeFunctionReturnValueEvidence,
    pub trap: MachineEncodedTrapBehavior,
    pub mechanism: WholeFunctionReturnMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionExitEvidence {
    pub machine: MachineId,
    pub entry_block: SelectedBlockId,
    pub body_stack_delta: i64,
    pub modified_callee_saved_units: Vec<RegisterUnitId>,
    pub returns: Vec<WholeFunctionReturnEvidence>,
}

/// Whole-function evidence for the one atomic structural Unit call bundle.
/// The rel32 remains a typed unresolved fixup until whole-text placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionStructuralUnitCallEvidence {
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub operation: psi_core::OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub frame_byte_count: u32,
    pub shadow_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub frame_is_balanced: bool,
}

/// Parallel custody for the bounded zero-VReg structural Unit roster. Keeping
/// this distinct prevents its function-local instruction IDs from colliding
/// with ordinary rows or with the other structural function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionStructuralUnitExitEvidence {
    pub machine: MachineId,
    pub entry_block: SelectedBlockId,
    pub body_stack_delta: i64,
    pub modified_callee_saved_units: Vec<RegisterUnitId>,
    pub call: Option<WholeFunctionStructuralUnitCallEvidence>,
    pub returned: WholeFunctionReturnEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeFunctionExitContract {
    pub identity: WholeFunctionExitContractIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub post_allocation_manifest:
        omega_optimization_core::PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: omega_register_model::PhysicalRegisterModelIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub layout_custody: WholeFunctionExitLayoutCustody,
    pub target: NativeTarget,
    pub policy: WholeFunctionExitPolicy,
    pub hardening: WholeFunctionHardeningPolicy,
    pub entry_assumption: WholeFunctionEntryAssumption,
    pub stack_pointer: RegisterViewId,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
    pub result_view: RegisterViewId,
    pub callee_saved_units: Vec<RegisterUnitId>,
    /// These rosters stay heap-owned because the validated contract is nested
    /// in several owning pipeline carriers; adding structural evidence must
    /// not inflate every ordinary carrier's stack frame.
    pub functions: Box<Vec<WholeFunctionExitEvidence>>,
    /// Parallel to `functions`; never merged by function-local instruction ID.
    pub structural_unit_functions: Box<Vec<WholeFunctionStructuralUnitExitEvidence>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWholeFunctionExitContract {
    contract: WholeFunctionExitContract,
}

impl ValidatedWholeFunctionExitContract {
    pub const fn contract(&self) -> &WholeFunctionExitContract {
        &self.contract
    }

    pub const fn identity(&self) -> WholeFunctionExitContractIdentity {
        self.contract.identity
    }

    #[cfg(test)]
    pub(crate) fn contract_mut(&mut self) -> &mut WholeFunctionExitContract {
        &mut self.contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionExitContractError {
    Layout(OptimizedResolvedSelectedFormLayoutError),
    Relaxation(OptimizedX86BranchRelaxationError),
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    FunctionRosterMismatch(MachineId),
    StructuralFunctionRosterMismatch(MachineId),
    StructuralCallRosterMismatch(SelectedInstructionId),
    StructuralCallTopologyMismatch,
    StructuralCallLayoutMismatch(SelectedInstructionId),
    BlockRosterMismatch(SelectedBlockId),
    InstructionRosterMismatch(SelectedInstructionId),
    CalleeSavedWrite {
        instruction: SelectedInstructionId,
        unit: RegisterUnitId,
    },
    LinkRegisterWrite(SelectedInstructionId),
    NonReturnStackEffect(SelectedInstructionId),
    NonReturnMemoryEffect(SelectedInstructionId),
    NonReturnControlEffect(SelectedInstructionId),
    MissingReturn(MachineId),
    ReturnOperandMismatch(SelectedInstructionId),
    ReturnEncodingMismatch(SelectedInstructionId),
    ReturnEffectsMismatch(SelectedInstructionId),
    ReturnPlacementMismatch(SelectedInstructionId),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for WholeFunctionExitContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal whole-function exit contract: {self:?}"
        )
    }
}

impl std::error::Error for WholeFunctionExitContractError {}

pub fn stage_whole_function_exit_contract<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    )?;
    let validated = ValidatedWholeFunctionExitContract { contract };
    validate_whole_function_exit_contract(
        selected, machine, physical, encoding, layout, &validated,
    )?;
    Ok(validated)
}

pub fn validate_whole_function_exit_contract<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout(selected, machine, physical, encoding, layout)
        .map_err(WholeFunctionExitContractError::Layout)?;
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

/// Stage an exit contract over an independently validated x86 branch-relaxed
/// layout. This path retains the relaxation receipt in the contract rather
/// than treating the transformed layout as baseline layout authority.
pub fn stage_whole_function_exit_contract_after_x86_branch_relaxation<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    source_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let layout_custody = WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
        relaxation: relaxation.identity(),
    };
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        relaxation.layout(),
        layout_custody,
    )?;
    let validated = ValidatedWholeFunctionExitContract { contract };
    validate_whole_function_exit_contract_after_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        source_layout,
        relaxation,
        &validated,
    )?;
    Ok(validated)
}

/// Independently validate the source near layout and replay the x86 branch
/// relaxation before admitting its transformed layout to exit validation.
pub fn validate_whole_function_exit_contract_after_x86_branch_relaxation<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    source_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        source_layout,
        relaxation,
    )
    .map_err(WholeFunctionExitContractError::Relaxation)?;
    let layout_custody = WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
        relaxation: relaxation.identity(),
    };
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        relaxation.layout(),
        layout_custody,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

/// Stage an exit contract over the independently replayed final CBNZ layout.
/// The symbolic fusion receipt remains explicit authority for the zero-byte
/// compare and fused branch; neither is admitted as an ordinary baseline row.
pub fn stage_whole_function_exit_contract_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion: fusion.fusion().receipt().identity(),
        };
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
    )?;
    let validated = ValidatedWholeFunctionExitContract { contract };
    validate_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout, &validated,
    )?;
    Ok(validated)
}

/// Independently reconstruct the CBNZ encoding and final layout before
/// accepting its whole-function exit contract.
pub fn validate_whole_function_exit_contract_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout,
    )
    .map_err(WholeFunctionExitContractError::Layout)?;
    let layout_custody =
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion: fusion.fusion().receipt().identity(),
        };
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged_machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_custody: WholeFunctionExitLayoutCustody,
) -> Result<WholeFunctionExitContract, WholeFunctionExitContractError> {
    let selected_plan = selected.selected_plan();
    let machine = staged_machine.machine().plan();
    if selected.selected_identity() != machine.selected
        || encoding.selected() != machine.selected
        || layout.selected() != machine.selected
        || encoding.machine() != machine.identity
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity()
        || selected_plan.target != machine.target
        || layout.target() != machine.target
        || machine.physical_register_model != physical.identity()
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }

    let target = machine.target;
    let (ordinary_policy, convention, stack_name, link_name, entry_assumption) =
        target_contract_inputs(physical, target)?;
    if convention.result_views.len() != 1 || convention.stack_alignment == 0 {
        return Err(WholeFunctionExitContractError::InvalidConvention);
    }
    let stack_pointer = view(physical, stack_name)?.id;
    let result_view = convention.result_views[0];
    let link_register = link_name
        .map(|name| view(physical, name).map(|view| view.id))
        .transpose()?;
    let entry_assumption = match (entry_assumption, link_register) {
        (EntryAssumptionKind::ActivationStack, None) => {
            WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
        }
        (EntryAssumptionKind::LinkRegister, Some(link_register)) => {
            WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register }
        }
        _ => return Err(WholeFunctionExitContractError::InvalidConvention),
    };

    let encoding_rows = unique_encoding_rows(encoding)?;
    let layout_rows = unique_layout_rows(layout)?;
    let machine_functions = machine
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let layout_functions = layout
        .functions()
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    if machine_functions.len() != selected_plan.functions.len()
        || layout_functions.len() != selected_plan.functions.len()
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }

    let callee_saved = convention
        .callee_saved
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let stack_units = view(physical, stack_name)?
        .units
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let link_units = link_name
        .map(|name| view(physical, name).map(|view| view.units.iter().copied().collect()))
        .transpose()?
        .unwrap_or_default();

    if !selected_plan.structural_unit_functions.is_empty() {
        if ordinary_policy != WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
            || !selected_plan.functions.is_empty()
            || !machine.functions.is_empty()
            || !encoding.rows().is_empty()
            || !layout.functions().is_empty()
            || layout.policy()
                != crate::SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1
            || layout_custody != WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        {
            return Err(WholeFunctionExitContractError::UnsupportedTargetPolicy);
        }
        let structural_unit_functions = validate_structural_unit_functions(
            selected_plan,
            machine,
            encoding,
            layout,
            target,
            stack_pointer,
            result_view,
            &callee_saved,
            &link_units,
        )?;
        let policy = if structural_unit_functions
            .iter()
            .any(|function| function.call.is_some())
        {
            WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1
        } else {
            WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1
        };
        let mut contract = WholeFunctionExitContract {
            identity: WholeFunctionExitContractIdentity([0; 32]),
            selected: machine.selected,
            post_allocation_manifest: machine.post_allocation_manifest,
            post_allocation_machine: machine.identity,
            register_environment: machine.register_environment,
            physical_register_model: machine.physical_register_model,
            pre_layout: encoding.identity(),
            resolved_layout: layout.identity(),
            layout_custody,
            target,
            policy,
            hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
            entry_assumption,
            stack_pointer,
            stack_alignment: convention.stack_alignment,
            red_zone_bytes: convention.red_zone_bytes,
            result_view,
            callee_saved_units: convention.callee_saved.clone(),
            functions: Box::new(Vec::new()),
            structural_unit_functions: Box::new(structural_unit_functions),
        };
        contract.identity = contract_identity(&contract);
        return Ok(contract);
    }
    if !machine.structural_unit_functions.is_empty()
        || !encoding.structural_unit_functions().is_empty()
        || !layout.structural_unit_functions().is_empty()
    {
        return Err(WholeFunctionExitContractError::RootMismatch);
    }
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for function in &selected_plan.functions {
        let machine_function = machine_functions.get(&function.machine).ok_or(
            WholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
        )?;
        let layout_function = layout_functions.get(&function.machine).ok_or(
            WholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
        )?;
        let machine_blocks = machine_function
            .blocks
            .iter()
            .map(|block| (block.block, block))
            .collect::<BTreeMap<_, _>>();
        let layout_blocks = layout_function
            .blocks
            .iter()
            .map(|block| (block.block, block))
            .collect::<BTreeMap<_, _>>();
        if machine_blocks.len() != function.blocks.len()
            || layout_blocks.len() != function.blocks.len()
        {
            return Err(WholeFunctionExitContractError::FunctionRosterMismatch(
                function.machine,
            ));
        }

        let mut returns = Vec::new();
        for block in &function.blocks {
            let machine_block = machine_blocks.get(&block.id).ok_or(
                WholeFunctionExitContractError::BlockRosterMismatch(block.id),
            )?;
            let _layout_block = layout_blocks.get(&block.id).ok_or(
                WholeFunctionExitContractError::BlockRosterMismatch(block.id),
            )?;
            if machine_block.instructions.len() != block.instructions.len() + 1 {
                return Err(WholeFunctionExitContractError::BlockRosterMismatch(
                    block.id,
                ));
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let (instruction, return_edge, conditional_terminator) =
                    if index < block.instructions.len() {
                        (&block.instructions[index], None, false)
                    } else {
                        match &block.terminator {
                            SelectedTerminator::ConditionalBranch { instruction, .. } => {
                                (instruction, None, true)
                            }
                            SelectedTerminator::Return {
                                instruction,
                                psi_return_edge,
                            } => (instruction, Some(*psi_return_edge), false),
                        }
                    };
                if machine_instruction.instruction != instruction.id {
                    return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                        instruction.id,
                    ));
                }
                let encoding_row = encoding_rows.get(&instruction.id).ok_or(
                    WholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                let (resolved_block, resolved_row) = layout_rows.get(&instruction.id).ok_or(
                    WholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                if resolved_block.block != block.id
                    || encoding_row.alternative != machine_instruction.alternative.key
                    || resolved_row.alternative != machine_instruction.alternative.key
                {
                    return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                        instruction.id,
                    ));
                }
                reject_preservation_writes(
                    machine_instruction,
                    &callee_saved,
                    &link_units,
                    instruction.id,
                )?;
                if let Some(psi_return_edge) = return_edge {
                    let layout_block_end = resolved_block
                        .offset
                        .checked_add(resolved_block.byte_count)
                        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
                    returns.push(validate_return(
                        target,
                        stack_pointer,
                        link_register,
                        Some(result_view),
                        block.id,
                        psi_return_edge,
                        instruction,
                        machine_instruction,
                        encoding_row,
                        resolved_row,
                        layout_block_end,
                    )?);
                } else {
                    if machine_instruction
                        .unit_defs
                        .iter()
                        .chain(&machine_instruction.unit_clobbers)
                        .any(|unit| stack_units.contains(unit))
                    {
                        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
                            instruction.id,
                        ));
                    }
                    validate_non_return(
                        instruction.id,
                        conditional_terminator,
                        encoding_row,
                        resolved_row,
                    )?;
                }
            }
        }
        if returns.is_empty() {
            return Err(WholeFunctionExitContractError::MissingReturn(
                function.machine,
            ));
        }
        functions.push(WholeFunctionExitEvidence {
            machine: function.machine,
            entry_block: function.entry_block,
            body_stack_delta: 0,
            modified_callee_saved_units: Vec::new(),
            returns,
        });
    }

    let mut contract = WholeFunctionExitContract {
        identity: WholeFunctionExitContractIdentity([0; 32]),
        selected: machine.selected,
        post_allocation_manifest: machine.post_allocation_manifest,
        post_allocation_machine: machine.identity,
        register_environment: machine.register_environment,
        physical_register_model: machine.physical_register_model,
        pre_layout: encoding.identity(),
        resolved_layout: layout.identity(),
        layout_custody,
        target,
        policy: ordinary_policy,
        hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
        entry_assumption,
        stack_pointer,
        stack_alignment: convention.stack_alignment,
        red_zone_bytes: convention.red_zone_bytes,
        result_view,
        callee_saved_units: convention.callee_saved.clone(),
        functions: Box::new(functions),
        structural_unit_functions: Box::new(Vec::new()),
    };
    contract.identity = contract_identity(&contract);
    Ok(contract)
}

#[allow(clippy::too_many_arguments)]
fn validate_structural_unit_functions(
    selected: &omega_selected_instructions::SelectedInstructionPlan,
    machine: &omega_machine_optimizer::PostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    result_view: RegisterViewId,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
) -> Result<Vec<WholeFunctionStructuralUnitExitEvidence>, WholeFunctionExitContractError> {
    let structural_function_count = selected.structural_unit_functions.len();
    if !matches!(structural_function_count, 1 | 2)
        || machine.structural_unit_functions.len() != structural_function_count
        || encoding.structural_unit_functions().len() != structural_function_count
        || layout.structural_unit_functions().len() != structural_function_count
    {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    }

    let mut machine_functions = BTreeMap::new();
    for function in &machine.structural_unit_functions {
        if machine_functions
            .insert(function.machine, function)
            .is_some()
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(function.machine),
            );
        }
    }
    let mut encoding_functions = BTreeMap::new();
    for function in encoding.structural_unit_functions() {
        if encoding_functions
            .insert(function.machine, function)
            .is_some()
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(function.machine),
            );
        }
    }
    let mut layout_functions = BTreeMap::new();
    for function in layout.structural_unit_functions() {
        if layout_functions
            .insert(function.machine, function)
            .is_some()
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(function.machine),
            );
        }
    }

    let mut selected_machines = BTreeSet::new();
    let mut caller = None;
    let mut leaf = None;
    let mut evidence = Vec::with_capacity(structural_function_count);
    for selected_function in &selected.structural_unit_functions {
        if !selected_machines.insert(selected_function.machine) {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        let machine_function = machine_functions.get(&selected_function.machine).ok_or(
            WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                selected_function.machine,
            ),
        )?;
        let encoding_function = encoding_functions.get(&selected_function.machine).ok_or(
            WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                selected_function.machine,
            ),
        )?;
        let layout_function = layout_functions.get(&selected_function.machine).ok_or(
            WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                selected_function.machine,
            ),
        )?;
        if selected_function.entry_block != machine_function.block
            || selected_function.entry_block != encoding_function.block
            || selected_function.entry_block != layout_function.block
            || layout_function.offset != 0
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }

        let selected_return = &selected_function.terminator.instruction;
        let expected_return_id = SelectedInstructionId(if selected_function.call.is_some() {
            1
        } else {
            0
        });
        if selected_return.id != expected_return_id
            || selected_return.id != machine_function.return_instruction.instruction
            || selected_return.id != encoding_function.return_instruction.instruction
            || selected_return.id != layout_function.return_instruction.instruction
            || selected_return.kind != SelectedInstructionKind::ReturnUnit
            || selected_return.provenance != machine_function.return_provenance
            || selected_function.terminator.effect != machine_function.return_effect
            || selected_function.terminator.ownership != machine_function.return_ownership
            || machine_function.return_instruction.alternative.key
                != encoding_function.return_instruction.alternative
            || machine_function.return_instruction.alternative.key
                != layout_function.return_instruction.alternative
        {
            return Err(
                WholeFunctionExitContractError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        reject_preservation_writes(
            &machine_function.return_instruction,
            callee_saved,
            link_units,
            selected_return.id,
        )?;

        let call = match (
            &selected_function.call,
            &machine_function.call,
            &encoding_function.call,
            &layout_function.call,
        ) {
            (None, None, None, None) => {
                if leaf.replace(selected_function.machine).is_some()
                    || layout_function.byte_count != 1
                    || layout_function.return_instruction.offset != 0
                {
                    return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
                }
                None
            }
            (Some(selected_call), Some(machine_call), Some(encoding_call), Some(layout_call)) => {
                if caller.replace(selected_function.machine).is_some()
                    || selected_function.machine != selected.entry
                    || selected_call.id != SelectedInstructionId(0)
                    || selected_call.id != machine_call.instruction
                    || selected_call.id != encoding_call.instruction
                    || selected_call.id != layout_call.instruction
                    || selected_call.operation != machine_call.operation
                    || selected_call.operation != encoding_call.operation
                    || selected_call.operation != layout_call.operation
                    || selected_call.callee != machine_call.callee
                    || selected_call.callee != encoding_call.callee
                    || selected_call.callee != layout_call.callee
                    || selected_call.constraint != machine_call.constraint
                    || selected_call.implicit_uses != machine_call.unit_uses
                    || selected_call.implicit_defs != machine_call.unit_defs
                    || selected_call.clobbers != machine_call.unit_clobbers
                    || selected_call.layout != machine_call.layout
                    || selected_call.effect != machine_call.effect
                    || selected_call.ownership != machine_call.ownership
                    || selected_call.claim_transfers != machine_call.claim_transfers
                    || selected_call.provenance != machine_call.provenance
                    || encoding_call.bytes != layout_call.bytes
                    || encoding_call.footprint != layout_call.footprint
                    || encoding_call.fixup != layout_call.fixup
                {
                    return Err(
                        WholeFunctionExitContractError::StructuralCallRosterMismatch(
                            selected_call.id,
                        ),
                    );
                }
                validate_structural_call_layout(
                    selected_call.id,
                    selected_call.callee,
                    machine_call,
                    layout_call,
                    callee_saved,
                )?;
                if layout_function.byte_count
                    != u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT + 1)
                        .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?
                    || layout_function.return_instruction.offset
                        != u64::try_from(X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT)
                            .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?
                {
                    return Err(
                        WholeFunctionExitContractError::StructuralCallLayoutMismatch(
                            selected_call.id,
                        ),
                    );
                }
                Some(WholeFunctionStructuralUnitCallEvidence {
                    block: selected_function.entry_block,
                    instruction: selected_call.id,
                    operation: selected_call.operation,
                    callee: selected_call.callee,
                    offset: layout_call.offset,
                    bytes: layout_call.bytes.clone(),
                    fixup: layout_call.fixup,
                    unit_uses: machine_call.unit_uses.clone(),
                    unit_defs: machine_call.unit_defs.clone(),
                    unit_clobbers: machine_call.unit_clobbers.clone(),
                    frame_byte_count: layout_call.footprint.frame_byte_count,
                    shadow_byte_count: layout_call.footprint.shadow_byte_count,
                    pre_call_stack_alignment: layout_call.footprint.pre_call_stack_alignment,
                    frame_is_balanced: layout_call.footprint.frame_is_balanced,
                })
            }
            (Some(selected_call), _, _, _) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(selected_call.id),
                );
            }
            (None, Some(machine_call), _, _) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(
                        machine_call.instruction,
                    ),
                );
            }
            (None, None, Some(encoding_call), _) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(
                        encoding_call.instruction,
                    ),
                );
            }
            (None, None, None, Some(layout_call)) => {
                return Err(
                    WholeFunctionExitContractError::StructuralCallRosterMismatch(
                        layout_call.instruction,
                    ),
                );
            }
        };

        let function_end = layout_function
            .offset
            .checked_add(layout_function.byte_count)
            .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
        let returned = validate_return(
            target,
            stack_pointer,
            None,
            Some(result_view),
            selected_function.entry_block,
            selected_function.terminator.psi_return_edge,
            selected_return,
            &machine_function.return_instruction,
            &encoding_function.return_instruction,
            &layout_function.return_instruction,
            function_end,
        )?;
        evidence.push(WholeFunctionStructuralUnitExitEvidence {
            machine: selected_function.machine,
            entry_block: selected_function.entry_block,
            body_stack_delta: 0,
            modified_callee_saved_units: Vec::new(),
            call,
            returned,
        });
    }

    if structural_function_count == 1 {
        if caller.is_some() || leaf != Some(selected.entry) {
            return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
        }
        return Ok(evidence);
    }
    let (Some(caller), Some(leaf)) = (caller, leaf) else {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    };
    let caller_evidence = evidence
        .iter()
        .find(|function| function.machine == caller)
        .and_then(|function| function.call.as_ref())
        .ok_or(WholeFunctionExitContractError::StructuralCallTopologyMismatch)?;
    if caller != selected.entry || caller_evidence.callee != leaf || caller == leaf {
        return Err(WholeFunctionExitContractError::StructuralCallTopologyMismatch);
    }
    Ok(evidence)
}

fn validate_structural_call_layout(
    instruction: SelectedInstructionId,
    callee: MachineId,
    machine: &omega_machine_optimizer::StructuralUnitCallMachineEffects,
    layout: &crate::ResolvedStructuralUnitCallLayout,
    callee_saved: &BTreeSet<RegisterUnitId>,
) -> Result<(), WholeFunctionExitContractError> {
    let footprint = &layout.footprint;
    let fixup = layout.fixup;
    let rel32_start = usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET);
    let rel32_end = rel32_start + usize::from(X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH);
    if layout.offset != 0
        || layout.bytes.len() != X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT
        || layout
            .bytes
            .get(usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET))
            != Some(&0xe8)
        || layout.bytes.get(rel32_start..rel32_end) != Some(&[0, 0, 0, 0][..])
        || footprint.implicit_unit_uses != machine.unit_uses
        || footprint.implicit_unit_defs != machine.unit_defs
        || footprint.implicit_unit_clobbers != machine.unit_clobbers
        || footprint.frame_byte_count != 72
        || footprint.shadow_byte_count != 32
        || footprint.pre_call_stack_alignment != 16
        || !footprint.frame_is_balanced
        || machine.layout.outgoing_frame_byte_count != 72
        || machine.layout.shadow_byte_count != 32
        || machine.layout.pre_call_stack_alignment != 16
        || fixup.kind
            != X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        || fixup.state != X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || fixup.opcode_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET
        || fixup.field_byte_offset != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET
        || fixup.next_instruction_byte_offset
            != X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET
        || fixup.field_byte_width != X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH
        || fixup.addend != 0
    {
        return Err(
            WholeFunctionExitContractError::StructuralCallLayoutMismatch(instruction),
        );
    }
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        if callee_saved.contains(unit) {
            return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                instruction,
                unit: *unit,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum EntryAssumptionKind {
    ActivationStack,
    LinkRegister,
}

fn target_contract_inputs(
    physical: &ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Result<
    (
        WholeFunctionExitPolicy,
        &PreservationConvention,
        &'static str,
        Option<&'static str>,
        EntryAssumptionKind,
    ),
    WholeFunctionExitContractError,
> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Ok((
            WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::X86_64, ObjectFormat::Coff) => Ok((
            WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::Aarch64, ObjectFormat::Elf) => Ok((
            WholeFunctionExitPolicy::Aapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        (Architecture::Aarch64, ObjectFormat::MachO) => Ok((
            WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(WholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        _ => Err(WholeFunctionExitContractError::UnsupportedTargetPolicy),
    }
}

fn view<'model>(
    physical: &'model ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<&'model omega_register_model::RegisterView, WholeFunctionExitContractError> {
    physical.model().view_named(name).ok_or(
        WholeFunctionExitContractError::MissingArchitecturalView(name),
    )
}

fn unique_encoding_rows(
    encoding: &StagedOptimizedSelectedFormEncoding,
) -> Result<
    BTreeMap<SelectedInstructionId, &crate::SelectedFormEncodingRow>,
    WholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for row in encoding.rows() {
        if rows.insert(row.instruction, row).is_some() {
            return Err(WholeFunctionExitContractError::DuplicateInstruction(
                row.instruction,
            ));
        }
    }
    Ok(rows)
}

fn unique_layout_rows(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    BTreeMap<
        SelectedInstructionId,
        (
            &crate::ResolvedSelectedBlockLayout,
            &crate::ResolvedSelectedFormRow,
        ),
    >,
    WholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for function in layout.functions() {
        for block in &function.blocks {
            for row in &block.instructions {
                if rows.insert(row.instruction, (block, row)).is_some() {
                    return Err(WholeFunctionExitContractError::DuplicateInstruction(
                        row.instruction,
                    ));
                }
            }
        }
    }
    Ok(rows)
}

fn reject_preservation_writes(
    machine: &PostAllocationMachineInstruction,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    instruction: SelectedInstructionId,
) -> Result<(), WholeFunctionExitContractError> {
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        if callee_saved.contains(unit) {
            return Err(WholeFunctionExitContractError::CalleeSavedWrite {
                instruction,
                unit: *unit,
            });
        }
        if link_units.contains(unit) {
            return Err(WholeFunctionExitContractError::LinkRegisterWrite(
                instruction,
            ));
        }
    }
    Ok(())
}

fn validate_non_return(
    instruction: SelectedInstructionId,
    conditional_terminator: bool,
    encoding: &crate::SelectedFormEncodingRow,
    layout: &crate::ResolvedSelectedFormRow,
) -> Result<(), WholeFunctionExitContractError> {
    let effects = match &encoding.state {
        SelectedFormEncodingState::Encoded { footprint, bytes } => {
            let disposition_matches = match encoding.machine_disposition {
                Aarch64CbnzInstructionDisposition::RetainedV1 => bytes == &layout.bytes,
                Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { .. } => {
                    layout.bytes.is_empty()
                }
                Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 { .. } => false,
            };
            if conditional_terminator || !disposition_matches || layout.branch.is_some() {
                return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ));
            }
            &footprint.encoded
        }
        SelectedFormEncodingState::DeferredControl { .. } => {
            if !conditional_terminator {
                return Err(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ));
            }
            layout
                .branch
                .as_ref()
                .map(|branch| &branch.decoded_effects)
                .ok_or(WholeFunctionExitContractError::InstructionRosterMismatch(
                    instruction,
                ))?
        }
    };
    if effects.stack != MachineEncodedStackEffect::UnchangedV1 {
        return Err(WholeFunctionExitContractError::NonReturnStackEffect(
            instruction,
        ));
    }
    if effects.memory != MachineEncodedMemoryEffect::NoneV1 {
        return Err(WholeFunctionExitContractError::NonReturnMemoryEffect(
            instruction,
        ));
    }
    let expected_control = if conditional_terminator {
        MachineEncodedControlEffect::ConditionalRelativeBranchV1
    } else {
        MachineEncodedControlEffect::FallThroughV1
    };
    if effects.control != expected_control {
        return Err(WholeFunctionExitContractError::NonReturnControlEffect(
            instruction,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_return(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    link_register: Option<RegisterViewId>,
    result_view: Option<RegisterViewId>,
    block: SelectedBlockId,
    psi_return_edge: EdgeId,
    selected: &omega_selected_instructions::SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    encoding: &crate::SelectedFormEncodingRow,
    layout: &crate::ResolvedSelectedFormRow,
    layout_block_end: u64,
) -> Result<WholeFunctionReturnEvidence, WholeFunctionExitContractError> {
    let value = match selected.kind {
        SelectedInstructionKind::ReturnI64 => {
            let Some(result_view) = result_view else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            let [operand]: &[PhysicalOperandFootprint] = machine.operands.as_slice() else {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            };
            if selected.operands.len() != 1
                || operand.operand != 0
                || operand.access != RegisterOperandAccess::Use
                || operand.view != result_view
                || operand.read_units != operand.storage_units
                || !operand.write_units.is_empty()
            {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnValueEvidence::ScalarI64V1 {
                virtual_register: operand.virtual_register,
                view: operand.view,
                units: operand.storage_units.clone(),
            }
        }
        SelectedInstructionKind::ReturnUnit => {
            if !selected.operands.is_empty() || !machine.operands.is_empty() {
                return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnValueEvidence::UnitV1
        }
        _ => {
            return Err(WholeFunctionExitContractError::ReturnOperandMismatch(
                selected.id,
            ));
        }
    };
    let (bytes, effects): (&[u8], &MachineEncodedEffects) = match &encoding.state {
        SelectedFormEncodingState::Encoded { bytes, footprint } => (bytes, &footprint.encoded),
        SelectedFormEncodingState::DeferredControl { .. } => {
            return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
                selected.id,
            ));
        }
    };
    if bytes != layout.bytes || layout.branch.is_some() || effects != &machine.alternative.encoded {
        return Err(WholeFunctionExitContractError::ReturnEncodingMismatch(
            selected.id,
        ));
    }
    let end = layout
        .offset
        .checked_add(
            u64::try_from(layout.bytes.len())
                .map_err(|_| WholeFunctionExitContractError::OffsetOverflow)?,
        )
        .ok_or(WholeFunctionExitContractError::OffsetOverflow)?;
    if end != layout_block_end {
        return Err(WholeFunctionExitContractError::ReturnPlacementMismatch(
            selected.id,
        ));
    }
    let mechanism = match target.architecture {
        Architecture::X86_64 => {
            if effects.memory
                != (MachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer,
                    byte_count: 8,
                })
                || effects.stack
                    != (MachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    })
                || effects.control != MachineEncodedControlEffect::ReturnFromActivationStackV1
                || bytes != [0xc3]
            {
                return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            }
        }
        Architecture::Aarch64 => {
            let link_register = link_register.ok_or(
                WholeFunctionExitContractError::ReturnEffectsMismatch(selected.id),
            )?;
            if effects.memory != MachineEncodedMemoryEffect::NoneV1
                || effects.stack != MachineEncodedStackEffect::UnchangedV1
                || effects.control
                    != (MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: link_register,
                    })
                || bytes != [0xc0, 0x03, 0x5f, 0xd6]
            {
                return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
                    selected.id,
                ));
            }
            WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                stack_pointer,
                link_register,
            }
        }
    };
    if effects.trap != MachineEncodedTrapBehavior::MayArchitecturalFaultV1 {
        return Err(WholeFunctionExitContractError::ReturnEffectsMismatch(
            selected.id,
        ));
    }
    Ok(WholeFunctionReturnEvidence {
        block,
        psi_return_edge,
        instruction: selected.id,
        offset: layout.offset,
        bytes: layout.bytes.clone(),
        value,
        trap: effects.trap,
        mechanism,
    })
}

fn contract_identity(contract: &WholeFunctionExitContract) -> WholeFunctionExitContractIdentity {
    let mut hasher = Sha256::new();
    hasher.update(CONTRACT_SCHEMA);
    hasher.update(contract.selected.bytes());
    hasher.update(contract.post_allocation_manifest.bytes());
    hasher.update(contract.post_allocation_machine.bytes());
    hasher.update(contract.register_environment.bytes());
    hasher.update(contract.physical_register_model.bytes());
    hasher.update(contract.pre_layout.bytes());
    hasher.update(contract.resolved_layout.bytes());
    match contract.layout_custody {
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1 => hasher.update([1]),
        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { relaxation } => {
            hasher.update([2]);
            hasher.update(relaxation.bytes());
        }
        WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion,
        } => {
            hasher.update([3]);
            hasher.update(fusion.bytes());
        }
    }
    encode_target(&mut hasher, contract.target);
    hasher.update([policy_tag(contract.policy)]);
    hasher.update([1]);
    match contract.entry_assumption {
        WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => {
            hasher.update([1]);
        }
        WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
            hasher.update([2]);
            hasher.update(link_register.0.to_le_bytes());
        }
    }
    hasher.update(contract.stack_pointer.0.to_le_bytes());
    hasher.update(contract.stack_alignment.to_le_bytes());
    hasher.update(contract.red_zone_bytes.to_le_bytes());
    hasher.update(contract.result_view.0.to_le_bytes());
    encode_units(&mut hasher, &contract.callee_saved_units);
    hasher.update((contract.functions.len() as u64).to_le_bytes());
    for function in contract.functions.iter() {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.entry_block.0.to_le_bytes());
        hasher.update(function.body_stack_delta.to_le_bytes());
        encode_units(&mut hasher, &function.modified_callee_saved_units);
        hasher.update((function.returns.len() as u64).to_le_bytes());
        for returned in &function.returns {
            hasher.update(returned.block.0.to_le_bytes());
            hasher.update(returned.psi_return_edge.get().to_le_bytes());
            hasher.update(returned.instruction.0.to_le_bytes());
            hasher.update(returned.offset.to_le_bytes());
            hasher.update((returned.bytes.len() as u64).to_le_bytes());
            hasher.update(&returned.bytes);
            match &returned.value {
                WholeFunctionReturnValueEvidence::UnitV1 => hasher.update([1]),
                WholeFunctionReturnValueEvidence::ScalarI64V1 {
                    virtual_register,
                    view,
                    units,
                } => {
                    hasher.update([2]);
                    hasher.update(virtual_register.0.to_le_bytes());
                    hasher.update(view.0.to_le_bytes());
                    encode_units(&mut hasher, units);
                }
            }
            hasher.update([1]);
            match returned.mechanism {
                WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                    stack_pointer,
                    read_bytes,
                    pop_bytes,
                } => {
                    hasher.update([1]);
                    hasher.update(stack_pointer.0.to_le_bytes());
                    hasher.update(read_bytes.to_le_bytes());
                    hasher.update(pop_bytes.to_le_bytes());
                }
                WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                    stack_pointer,
                    link_register,
                } => {
                    hasher.update([2]);
                    hasher.update(stack_pointer.0.to_le_bytes());
                    hasher.update(link_register.0.to_le_bytes());
                }
            }
        }
    }
    hasher.update((contract.structural_unit_functions.len() as u64).to_le_bytes());
    for function in contract.structural_unit_functions.iter() {
        hasher.update(function.machine.get().to_le_bytes());
        hasher.update(function.entry_block.0.to_le_bytes());
        hasher.update(function.body_stack_delta.to_le_bytes());
        encode_units(&mut hasher, &function.modified_callee_saved_units);
        match &function.call {
            None => hasher.update([0]),
            Some(call) => {
                hasher.update([1]);
                hasher.update(call.block.0.to_le_bytes());
                hasher.update(call.instruction.0.to_le_bytes());
                hasher.update(call.operation.get().to_le_bytes());
                hasher.update(call.callee.get().to_le_bytes());
                hasher.update(call.offset.to_le_bytes());
                hasher.update((call.bytes.len() as u64).to_le_bytes());
                hasher.update(&call.bytes);
                encode_structural_fixup(&mut hasher, call.fixup);
                encode_units(&mut hasher, &call.unit_uses);
                encode_units(&mut hasher, &call.unit_defs);
                encode_units(&mut hasher, &call.unit_clobbers);
                hasher.update(call.frame_byte_count.to_le_bytes());
                hasher.update(call.shadow_byte_count.to_le_bytes());
                hasher.update(call.pre_call_stack_alignment.to_le_bytes());
                hasher.update([u8::from(call.frame_is_balanced)]);
            }
        }
        encode_return(&mut hasher, &function.returned);
    }
    WholeFunctionExitContractIdentity(hasher.finalize().into())
}

fn encode_return(hasher: &mut Sha256, returned: &WholeFunctionReturnEvidence) {
    hasher.update(returned.block.0.to_le_bytes());
    hasher.update(returned.psi_return_edge.get().to_le_bytes());
    hasher.update(returned.instruction.0.to_le_bytes());
    hasher.update(returned.offset.to_le_bytes());
    hasher.update((returned.bytes.len() as u64).to_le_bytes());
    hasher.update(&returned.bytes);
    match &returned.value {
        WholeFunctionReturnValueEvidence::UnitV1 => hasher.update([1]),
        WholeFunctionReturnValueEvidence::ScalarI64V1 {
            virtual_register,
            view,
            units,
        } => {
            hasher.update([2]);
            hasher.update(virtual_register.0.to_le_bytes());
            hasher.update(view.0.to_le_bytes());
            encode_units(hasher, units);
        }
    }
    hasher.update([1]);
    match returned.mechanism {
        WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
            stack_pointer,
            read_bytes,
            pop_bytes,
        } => {
            hasher.update([1]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(read_bytes.to_le_bytes());
            hasher.update(pop_bytes.to_le_bytes());
        }
        WholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
            stack_pointer,
            link_register,
        } => {
            hasher.update([2]);
            hasher.update(stack_pointer.0.to_le_bytes());
            hasher.update(link_register.0.to_le_bytes());
        }
    }
}

fn encode_structural_fixup(hasher: &mut Sha256, fixup: X86_64StructuralUnitInternalControlFixup) {
    hasher.update([match fixup.kind {
        X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1 => 1,
    }]);
    hasher.update([match fixup.state {
        X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1 => 1,
    }]);
    hasher.update(fixup.callee.get().to_le_bytes());
    hasher.update(fixup.opcode_byte_offset.to_le_bytes());
    hasher.update(fixup.field_byte_offset.to_le_bytes());
    hasher.update(fixup.next_instruction_byte_offset.to_le_bytes());
    hasher.update([fixup.field_byte_width]);
    hasher.update(fixup.addend.to_le_bytes());
}

fn encode_target(hasher: &mut Sha256, target: NativeTarget) {
    hasher.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    hasher.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    hasher.update((target.pointer_size as u64).to_le_bytes());
    hasher.update((target.pointer_alignment as u64).to_le_bytes());
}

fn policy_tag(policy: WholeFunctionExitPolicy) -> u8 {
    match policy {
        WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
        WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1 => 5,
        WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1 => 6,
    }
}

fn encode_units(hasher: &mut Sha256, units: &[RegisterUnitId]) {
    hasher.update((units.len() as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.0.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract_with_custody(
        layout_custody: WholeFunctionExitLayoutCustody,
    ) -> WholeFunctionExitContract {
        let mut contract = WholeFunctionExitContract {
            identity: WholeFunctionExitContractIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            post_allocation_manifest:
                omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                    [2; 32],
                ),
            post_allocation_machine:
                omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes([3; 32]),
            register_environment:
                omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            physical_register_model:
                omega_register_model::PhysicalRegisterModelIdentity::from_bytes([5; 32]),
            pre_layout: SelectedFormEncodingIdentity::from_bytes([6; 32]),
            resolved_layout: ResolvedSelectedFormLayoutIdentity::from_bytes([7; 32]),
            layout_custody,
            target: NativeTarget::linux_x64(),
            policy: WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
            entry_assumption: WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1,
            stack_pointer: RegisterViewId(0),
            stack_alignment: 16,
            red_zone_bytes: 128,
            result_view: RegisterViewId(1),
            callee_saved_units: Vec::new(),
            functions: Box::new(Vec::new()),
            structural_unit_functions: Box::new(Vec::new()),
        };
        contract.identity = contract_identity(&contract);
        contract
    }

    #[test]
    fn layout_custody_and_relaxation_receipt_are_identity_bound() {
        let baseline = contract_with_custody(WholeFunctionExitLayoutCustody::BaselineNearLayoutV1);
        let relaxed = contract_with_custody(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: X86BranchRelaxationIdentity::from_bytes([8; 32]),
            },
        );
        let another_relaxation = contract_with_custody(
            WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: X86BranchRelaxationIdentity::from_bytes([9; 32]),
            },
        );

        assert_ne!(baseline.identity, relaxed.identity);
        assert_ne!(relaxed.identity, another_relaxation.identity);
    }

    #[test]
    fn structural_call_frame_fixup_and_returns_are_identity_bound() {
        let caller = MachineId::new(1).unwrap();
        let leaf = MachineId::new(2).unwrap();
        let mut contract =
            contract_with_custody(WholeFunctionExitLayoutCustody::BaselineNearLayoutV1);
        contract.target = NativeTarget::uefi_x64();
        contract.policy = WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1;
        contract.red_zone_bytes = 0;
        let mut call_bytes = vec![0; X86_64_STRUCTURAL_UNIT_CALL_TEMPLATE_BYTE_COUNT];
        call_bytes[usize::from(X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET)] = 0xe8;
        let returned = |instruction, offset, edge| WholeFunctionReturnEvidence {
            block: SelectedBlockId(0),
            psi_return_edge: EdgeId::new(edge).unwrap(),
            instruction: SelectedInstructionId(instruction),
            offset,
            bytes: vec![0xc3],
            value: WholeFunctionReturnValueEvidence::UnitV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            mechanism: WholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer: contract.stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            },
        };
        *contract.structural_unit_functions = vec![
            WholeFunctionStructuralUnitExitEvidence {
                machine: caller,
                entry_block: SelectedBlockId(0),
                body_stack_delta: 0,
                modified_callee_saved_units: Vec::new(),
                call: Some(WholeFunctionStructuralUnitCallEvidence {
                    block: SelectedBlockId(0),
                    instruction: SelectedInstructionId(0),
                    operation: psi_core::OperationId::new(3).unwrap(),
                    callee: leaf,
                    offset: 0,
                    bytes: call_bytes,
                    fixup: X86_64StructuralUnitInternalControlFixup {
                        kind: X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1,
                        state: X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1,
                        callee: leaf,
                        opcode_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_OPCODE_OFFSET,
                        field_byte_offset: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_OFFSET,
                        next_instruction_byte_offset:
                            X86_64_STRUCTURAL_UNIT_CALL_NEXT_INSTRUCTION_OFFSET,
                        field_byte_width: X86_64_STRUCTURAL_UNIT_CALL_REL32_FIELD_WIDTH,
                        addend: 0,
                    },
                    unit_uses: Vec::new(),
                    unit_defs: Vec::new(),
                    unit_clobbers: Vec::new(),
                    frame_byte_count: 72,
                    shadow_byte_count: 32,
                    pre_call_stack_alignment: 16,
                    frame_is_balanced: true,
                }),
                returned: returned(1, 89, 4),
            },
            WholeFunctionStructuralUnitExitEvidence {
                machine: leaf,
                entry_block: SelectedBlockId(0),
                body_stack_delta: 0,
                modified_callee_saved_units: Vec::new(),
                call: None,
                returned: returned(0, 0, 5),
            },
        ];
        contract.identity = contract_identity(&contract);

        let mut changed_frame = contract.clone();
        changed_frame.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .frame_byte_count = 71;
        changed_frame.identity = contract_identity(&changed_frame);
        let mut changed_fixup = contract.clone();
        changed_fixup.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .fixup
            .field_byte_offset += 1;
        changed_fixup.identity = contract_identity(&changed_fixup);
        let mut changed_return = contract.clone();
        changed_return.structural_unit_functions[0].returned.offset -= 1;
        changed_return.identity = contract_identity(&changed_return);
        let mut structural_leaf = contract.clone();
        structural_leaf.policy = WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1;
        *structural_leaf.structural_unit_functions =
            vec![structural_leaf.structural_unit_functions[1].clone()];
        structural_leaf.identity = contract_identity(&structural_leaf);

        assert_ne!(contract.identity, changed_frame.identity);
        assert_ne!(contract.identity, changed_fixup.identity);
        assert_ne!(contract.identity, changed_return.identity);
        assert_ne!(contract.identity, structural_leaf.identity);
    }
}
