use std::collections::{BTreeMap, BTreeSet};

use omega_machine_optimizer::{
    TerminalAarch64CbnzFusionIdentity, TerminalAarch64CbnzInstructionDisposition,
    TerminalPhysicalOperandFootprint, TerminalPostAllocationMachineInstruction,
};
use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::{
    PreservationConvention, RegisterOperandAccess, RegisterUnitId, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_isa_aarch64::aarch64_preservation_convention_for_target;
use omega_terminal_isa_x86_64::x86_64_preservation_convention_for_target;
use omega_terminal_selected_instructions::{
    TerminalMachineEncodedControlEffect, TerminalMachineEncodedEffects,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalSelectedBlockId, TerminalSelectedInstructionId,
    TerminalSelectedInstructionKind, TerminalSelectedInstructionPlanIdentity,
    TerminalSelectedTerminator, TerminalVirtualRegisterId,
};
use psi_core::{EdgeId, MachineId};
use sha2::{Digest, Sha256};

use crate::{
    OptimizedResolvedSelectedFormLayoutError, OptimizedX86BranchRelaxationError,
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    StagedOptimizedX86BranchRelaxation, TerminalResolvedSelectedFormLayoutIdentity,
    TerminalSelectedFormEncodingIdentity, TerminalSelectedFormEncodingState,
    TerminalX86BranchRelaxationIdentity, validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_x86_branch_relaxation,
};

const CONTRACT_SCHEMA: &[u8] = b"omega.terminal.whole-function-exit-contract.v3\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalWholeFunctionExitContractIdentity([u8; 32]);

impl TerminalWholeFunctionExitContractIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalWholeFunctionExitPolicy {
    SystemVAMD64FramelessLeafV1,
    MicrosoftX64FramelessLeafV1,
    Aapcs64FramelessLeafV1,
    DarwinAapcs64FramelessLeafV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalWholeFunctionHardeningPolicy {
    NoAdditionalEntryExitHardeningV1,
}

/// Exact authority under which the final function-relative layout entered
/// whole-function exit validation. The baseline variant never admits a
/// transformed layout; the relaxation variant is available only through the
/// dedicated independently replayed x86 relaxation API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalWholeFunctionExitLayoutCustody {
    BaselineNearLayoutV1,
    X86RelaxConditionalBranchesToRel8V1 {
        relaxation: TerminalX86BranchRelaxationIdentity,
    },
    Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
        fusion: TerminalAarch64CbnzFusionIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalWholeFunctionEntryAssumption {
    CallerReturnAddressAtStackPointerV1,
    CallerLinkRegisterV1 { link_register: RegisterViewId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalWholeFunctionReturnMechanism {
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
pub struct TerminalWholeFunctionReturnEvidence {
    pub block: TerminalSelectedBlockId,
    pub psi_return_edge: EdgeId,
    pub instruction: TerminalSelectedInstructionId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub result_virtual_register: TerminalVirtualRegisterId,
    pub result_view: RegisterViewId,
    pub result_units: Vec<RegisterUnitId>,
    pub trap: TerminalMachineEncodedTrapBehavior,
    pub mechanism: TerminalWholeFunctionReturnMechanism,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalWholeFunctionExitEvidence {
    pub machine: MachineId,
    pub entry_block: TerminalSelectedBlockId,
    pub body_stack_delta: i64,
    pub modified_callee_saved_units: Vec<RegisterUnitId>,
    pub returns: Vec<TerminalWholeFunctionReturnEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalWholeFunctionExitContract {
    pub identity: TerminalWholeFunctionExitContractIdentity,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub post_allocation_manifest:
        omega_optimization_core::PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_machine_optimizer::TerminalPostAllocationMachineIdentity,
    pub register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub physical_register_model: omega_register_model::PhysicalRegisterModelIdentity,
    pub pre_layout: TerminalSelectedFormEncodingIdentity,
    pub resolved_layout: TerminalResolvedSelectedFormLayoutIdentity,
    pub layout_custody: TerminalWholeFunctionExitLayoutCustody,
    pub target: NativeTarget,
    pub policy: TerminalWholeFunctionExitPolicy,
    pub hardening: TerminalWholeFunctionHardeningPolicy,
    pub entry_assumption: TerminalWholeFunctionEntryAssumption,
    pub stack_pointer: RegisterViewId,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
    pub result_view: RegisterViewId,
    pub callee_saved_units: Vec<RegisterUnitId>,
    pub functions: Vec<TerminalWholeFunctionExitEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalWholeFunctionExitContract {
    contract: TerminalWholeFunctionExitContract,
}

impl ValidatedTerminalWholeFunctionExitContract {
    pub const fn contract(&self) -> &TerminalWholeFunctionExitContract {
        &self.contract
    }

    pub const fn identity(&self) -> TerminalWholeFunctionExitContractIdentity {
        self.contract.identity
    }

    #[cfg(test)]
    pub(crate) fn contract_mut(&mut self) -> &mut TerminalWholeFunctionExitContract {
        &mut self.contract
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalWholeFunctionExitContractError {
    Layout(OptimizedResolvedSelectedFormLayoutError),
    Relaxation(OptimizedX86BranchRelaxationError),
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(TerminalSelectedInstructionId),
    MissingInstruction(TerminalSelectedInstructionId),
    FunctionRosterMismatch(MachineId),
    BlockRosterMismatch(TerminalSelectedBlockId),
    InstructionRosterMismatch(TerminalSelectedInstructionId),
    CalleeSavedWrite {
        instruction: TerminalSelectedInstructionId,
        unit: RegisterUnitId,
    },
    LinkRegisterWrite(TerminalSelectedInstructionId),
    NonReturnStackEffect(TerminalSelectedInstructionId),
    NonReturnMemoryEffect(TerminalSelectedInstructionId),
    NonReturnControlEffect(TerminalSelectedInstructionId),
    MissingReturn(MachineId),
    ReturnOperandMismatch(TerminalSelectedInstructionId),
    ReturnEncodingMismatch(TerminalSelectedInstructionId),
    ReturnEffectsMismatch(TerminalSelectedInstructionId),
    ReturnPlacementMismatch(TerminalSelectedInstructionId),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for TerminalWholeFunctionExitContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal whole-function exit contract: {self:?}"
        )
    }
}

impl std::error::Error for TerminalWholeFunctionExitContractError {}

pub fn stage_terminal_whole_function_exit_contract<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedTerminalWholeFunctionExitContract, TerminalWholeFunctionExitContractError> {
    let contract = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    )?;
    let validated = ValidatedTerminalWholeFunctionExitContract { contract };
    validate_terminal_whole_function_exit_contract(
        selected, machine, physical, encoding, layout, &validated,
    )?;
    Ok(validated)
}

pub fn validate_terminal_whole_function_exit_contract<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<(), TerminalWholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout(selected, machine, physical, encoding, layout)
        .map_err(TerminalWholeFunctionExitContractError::Layout)?;
    let replayed = compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    )?;
    if replayed != contract.contract {
        return Err(TerminalWholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

/// Stage an exit contract over an independently validated x86 branch-relaxed
/// layout. This path retains the relaxation receipt in the contract rather
/// than treating the transformed layout as baseline layout authority.
pub fn stage_terminal_whole_function_exit_contract_after_x86_branch_relaxation<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    source_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
) -> Result<ValidatedTerminalWholeFunctionExitContract, TerminalWholeFunctionExitContractError> {
    let layout_custody =
        TerminalWholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
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
    let validated = ValidatedTerminalWholeFunctionExitContract { contract };
    validate_terminal_whole_function_exit_contract_after_x86_branch_relaxation(
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
pub fn validate_terminal_whole_function_exit_contract_after_x86_branch_relaxation<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    source_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
    contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<(), TerminalWholeFunctionExitContractError> {
    validate_optimized_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        source_layout,
        relaxation,
    )
    .map_err(TerminalWholeFunctionExitContractError::Relaxation)?;
    let layout_custody =
        TerminalWholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
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
        return Err(TerminalWholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

/// Stage an exit contract over the independently replayed final CBNZ layout.
/// The symbolic fusion receipt remains explicit authority for the zero-byte
/// compare and fused branch; neither is admitted as an ordinary baseline row.
pub fn stage_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedTerminalWholeFunctionExitContract, TerminalWholeFunctionExitContractError> {
    let layout_custody =
        TerminalWholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
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
    let validated = ValidatedTerminalWholeFunctionExitContract { contract };
    validate_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout, &validated,
    )?;
    Ok(validated)
}

/// Independently reconstruct the CBNZ encoding and final layout before
/// accepting its whole-function exit contract.
pub fn validate_terminal_whole_function_exit_contract_after_aarch64_cbnz_fusion<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedTerminalWholeFunctionExitContract,
) -> Result<(), TerminalWholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion(
        selected, machine, physical, encoding, fusion, layout,
    )
    .map_err(TerminalWholeFunctionExitContractError::Layout)?;
    let layout_custody =
        TerminalWholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
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
        return Err(TerminalWholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}

fn compute<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    staged_machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_custody: TerminalWholeFunctionExitLayoutCustody,
) -> Result<TerminalWholeFunctionExitContract, TerminalWholeFunctionExitContractError> {
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
        return Err(TerminalWholeFunctionExitContractError::RootMismatch);
    }

    let target = machine.target;
    let (policy, convention, stack_name, link_name, entry_assumption) =
        target_contract_inputs(physical, target)?;
    if convention.result_views.len() != 1 || convention.stack_alignment == 0 {
        return Err(TerminalWholeFunctionExitContractError::InvalidConvention);
    }
    let stack_pointer = view(physical, stack_name)?.id;
    let result_view = convention.result_views[0];
    let link_register = link_name
        .map(|name| view(physical, name).map(|view| view.id))
        .transpose()?;
    let entry_assumption = match (entry_assumption, link_register) {
        (EntryAssumptionKind::ActivationStack, None) => {
            TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1
        }
        (EntryAssumptionKind::LinkRegister, Some(link_register)) => {
            TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register }
        }
        _ => return Err(TerminalWholeFunctionExitContractError::InvalidConvention),
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
        return Err(TerminalWholeFunctionExitContractError::RootMismatch);
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
    let mut functions = Vec::with_capacity(selected_plan.functions.len());
    for function in &selected_plan.functions {
        let machine_function = machine_functions.get(&function.machine).ok_or(
            TerminalWholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
        )?;
        let layout_function = layout_functions.get(&function.machine).ok_or(
            TerminalWholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
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
            return Err(
                TerminalWholeFunctionExitContractError::FunctionRosterMismatch(function.machine),
            );
        }

        let mut returns = Vec::new();
        for block in &function.blocks {
            let machine_block = machine_blocks.get(&block.id).ok_or(
                TerminalWholeFunctionExitContractError::BlockRosterMismatch(block.id),
            )?;
            let _layout_block = layout_blocks.get(&block.id).ok_or(
                TerminalWholeFunctionExitContractError::BlockRosterMismatch(block.id),
            )?;
            if machine_block.instructions.len() != block.instructions.len() + 1 {
                return Err(TerminalWholeFunctionExitContractError::BlockRosterMismatch(
                    block.id,
                ));
            }
            for (index, machine_instruction) in machine_block.instructions.iter().enumerate() {
                let (instruction, return_edge, conditional_terminator) =
                    if index < block.instructions.len() {
                        (&block.instructions[index], None, false)
                    } else {
                        match &block.terminator {
                            TerminalSelectedTerminator::ConditionalBranch {
                                instruction, ..
                            } => (instruction, None, true),
                            TerminalSelectedTerminator::Return {
                                instruction,
                                psi_return_edge,
                            } => (instruction, Some(*psi_return_edge), false),
                        }
                    };
                if machine_instruction.instruction != instruction.id {
                    return Err(
                        TerminalWholeFunctionExitContractError::InstructionRosterMismatch(
                            instruction.id,
                        ),
                    );
                }
                let encoding_row = encoding_rows.get(&instruction.id).ok_or(
                    TerminalWholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                let (resolved_block, resolved_row) = layout_rows.get(&instruction.id).ok_or(
                    TerminalWholeFunctionExitContractError::MissingInstruction(instruction.id),
                )?;
                if resolved_block.block != block.id
                    || encoding_row.alternative != machine_instruction.alternative.key
                    || resolved_row.alternative != machine_instruction.alternative.key
                {
                    return Err(
                        TerminalWholeFunctionExitContractError::InstructionRosterMismatch(
                            instruction.id,
                        ),
                    );
                }
                reject_preservation_writes(
                    machine_instruction,
                    &callee_saved,
                    &link_units,
                    instruction.id,
                )?;
                if let Some(psi_return_edge) = return_edge {
                    returns.push(validate_return(
                        target,
                        stack_pointer,
                        link_register,
                        result_view,
                        block.id,
                        psi_return_edge,
                        instruction,
                        machine_instruction,
                        encoding_row,
                        resolved_block,
                        resolved_row,
                    )?);
                } else {
                    if machine_instruction
                        .unit_defs
                        .iter()
                        .chain(&machine_instruction.unit_clobbers)
                        .any(|unit| stack_units.contains(unit))
                    {
                        return Err(
                            TerminalWholeFunctionExitContractError::NonReturnStackEffect(
                                instruction.id,
                            ),
                        );
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
            return Err(TerminalWholeFunctionExitContractError::MissingReturn(
                function.machine,
            ));
        }
        functions.push(TerminalWholeFunctionExitEvidence {
            machine: function.machine,
            entry_block: function.entry_block,
            body_stack_delta: 0,
            modified_callee_saved_units: Vec::new(),
            returns,
        });
    }

    let mut contract = TerminalWholeFunctionExitContract {
        identity: TerminalWholeFunctionExitContractIdentity([0; 32]),
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
        hardening: TerminalWholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
        entry_assumption,
        stack_pointer,
        stack_alignment: convention.stack_alignment,
        red_zone_bytes: convention.red_zone_bytes,
        result_view,
        callee_saved_units: convention.callee_saved.clone(),
        functions,
    };
    contract.identity = contract_identity(&contract);
    Ok(contract)
}

#[derive(Clone, Copy)]
enum EntryAssumptionKind {
    ActivationStack,
    LinkRegister,
}

fn target_contract_inputs<'model>(
    physical: &'model ValidatedPhysicalRegisterModel,
    target: NativeTarget,
) -> Result<
    (
        TerminalWholeFunctionExitPolicy,
        &'model PreservationConvention,
        &'static str,
        Option<&'static str>,
        EntryAssumptionKind,
    ),
    TerminalWholeFunctionExitContractError,
> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf) => Ok((
            TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(TerminalWholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::X86_64, ObjectFormat::Coff) => Ok((
            TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1,
            x86_64_preservation_convention_for_target(physical, target)
                .ok_or(TerminalWholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "rsp",
            None,
            EntryAssumptionKind::ActivationStack,
        )),
        (Architecture::Aarch64, ObjectFormat::Elf) => Ok((
            TerminalWholeFunctionExitPolicy::Aapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(TerminalWholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        (Architecture::Aarch64, ObjectFormat::MachO) => Ok((
            TerminalWholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1,
            aarch64_preservation_convention_for_target(physical, target)
                .ok_or(TerminalWholeFunctionExitContractError::UnsupportedTargetPolicy)?,
            "sp",
            Some("x30"),
            EntryAssumptionKind::LinkRegister,
        )),
        _ => Err(TerminalWholeFunctionExitContractError::UnsupportedTargetPolicy),
    }
}

fn view<'model>(
    physical: &'model ValidatedPhysicalRegisterModel,
    name: &'static str,
) -> Result<&'model omega_register_model::RegisterView, TerminalWholeFunctionExitContractError> {
    physical
        .model()
        .view_named(name)
        .ok_or(TerminalWholeFunctionExitContractError::MissingArchitecturalView(name))
}

fn unique_encoding_rows(
    encoding: &StagedOptimizedSelectedFormEncoding,
) -> Result<
    BTreeMap<TerminalSelectedInstructionId, &crate::TerminalSelectedFormEncodingRow>,
    TerminalWholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for row in encoding.rows() {
        if rows.insert(row.instruction, row).is_some() {
            return Err(
                TerminalWholeFunctionExitContractError::DuplicateInstruction(row.instruction),
            );
        }
    }
    Ok(rows)
}

fn unique_layout_rows(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<
    BTreeMap<
        TerminalSelectedInstructionId,
        (
            &crate::TerminalResolvedSelectedBlockLayout,
            &crate::TerminalResolvedSelectedFormRow,
        ),
    >,
    TerminalWholeFunctionExitContractError,
> {
    let mut rows = BTreeMap::new();
    for function in layout.functions() {
        for block in &function.blocks {
            for row in &block.instructions {
                if rows.insert(row.instruction, (block, row)).is_some() {
                    return Err(
                        TerminalWholeFunctionExitContractError::DuplicateInstruction(
                            row.instruction,
                        ),
                    );
                }
            }
        }
    }
    Ok(rows)
}

fn reject_preservation_writes(
    machine: &TerminalPostAllocationMachineInstruction,
    callee_saved: &BTreeSet<RegisterUnitId>,
    link_units: &BTreeSet<RegisterUnitId>,
    instruction: TerminalSelectedInstructionId,
) -> Result<(), TerminalWholeFunctionExitContractError> {
    for unit in machine.unit_defs.iter().chain(&machine.unit_clobbers) {
        if callee_saved.contains(unit) {
            return Err(TerminalWholeFunctionExitContractError::CalleeSavedWrite {
                instruction,
                unit: *unit,
            });
        }
        if link_units.contains(unit) {
            return Err(TerminalWholeFunctionExitContractError::LinkRegisterWrite(
                instruction,
            ));
        }
    }
    Ok(())
}

fn validate_non_return(
    instruction: TerminalSelectedInstructionId,
    conditional_terminator: bool,
    encoding: &crate::TerminalSelectedFormEncodingRow,
    layout: &crate::TerminalResolvedSelectedFormRow,
) -> Result<(), TerminalWholeFunctionExitContractError> {
    let effects = match &encoding.state {
        TerminalSelectedFormEncodingState::Encoded { footprint, bytes } => {
            let disposition_matches = match encoding.machine_disposition {
                TerminalAarch64CbnzInstructionDisposition::RetainedV1 => bytes == &layout.bytes,
                TerminalAarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { .. } => {
                    layout.bytes.is_empty()
                }
                TerminalAarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                    ..
                } => false,
            };
            if conditional_terminator || !disposition_matches || layout.branch.is_some() {
                return Err(
                    TerminalWholeFunctionExitContractError::InstructionRosterMismatch(instruction),
                );
            }
            &footprint.encoded
        }
        TerminalSelectedFormEncodingState::DeferredControl { .. } => {
            if !conditional_terminator {
                return Err(
                    TerminalWholeFunctionExitContractError::InstructionRosterMismatch(instruction),
                );
            }
            layout
                .branch
                .as_ref()
                .map(|branch| &branch.decoded_effects)
                .ok_or(
                    TerminalWholeFunctionExitContractError::InstructionRosterMismatch(instruction),
                )?
        }
    };
    if effects.stack != TerminalMachineEncodedStackEffect::UnchangedV1 {
        return Err(TerminalWholeFunctionExitContractError::NonReturnStackEffect(instruction));
    }
    if effects.memory != TerminalMachineEncodedMemoryEffect::NoneV1 {
        return Err(TerminalWholeFunctionExitContractError::NonReturnMemoryEffect(instruction));
    }
    let expected_control = if conditional_terminator {
        TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1
    } else {
        TerminalMachineEncodedControlEffect::FallThroughV1
    };
    if effects.control != expected_control {
        return Err(TerminalWholeFunctionExitContractError::NonReturnControlEffect(instruction));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_return(
    target: NativeTarget,
    stack_pointer: RegisterViewId,
    link_register: Option<RegisterViewId>,
    result_view: RegisterViewId,
    block: TerminalSelectedBlockId,
    psi_return_edge: EdgeId,
    selected: &omega_terminal_selected_instructions::TerminalSelectedInstruction,
    machine: &TerminalPostAllocationMachineInstruction,
    encoding: &crate::TerminalSelectedFormEncodingRow,
    layout_block: &crate::TerminalResolvedSelectedBlockLayout,
    layout: &crate::TerminalResolvedSelectedFormRow,
) -> Result<TerminalWholeFunctionReturnEvidence, TerminalWholeFunctionExitContractError> {
    if !matches!(selected.kind, TerminalSelectedInstructionKind::ReturnI64)
        || selected.operands.len() != 1
        || machine.operands.len() != 1
    {
        return Err(TerminalWholeFunctionExitContractError::ReturnOperandMismatch(selected.id));
    }
    let operand: &TerminalPhysicalOperandFootprint = &machine.operands[0];
    if operand.operand != 0
        || operand.access != RegisterOperandAccess::Use
        || operand.view != result_view
        || operand.read_units != operand.storage_units
        || !operand.write_units.is_empty()
    {
        return Err(TerminalWholeFunctionExitContractError::ReturnOperandMismatch(selected.id));
    }
    let (bytes, effects): (&[u8], &TerminalMachineEncodedEffects) = match &encoding.state {
        TerminalSelectedFormEncodingState::Encoded { bytes, footprint } => {
            (bytes, &footprint.encoded)
        }
        TerminalSelectedFormEncodingState::DeferredControl { .. } => {
            return Err(
                TerminalWholeFunctionExitContractError::ReturnEncodingMismatch(selected.id),
            );
        }
    };
    if bytes != layout.bytes || layout.branch.is_some() || effects != &machine.alternative.encoded {
        return Err(TerminalWholeFunctionExitContractError::ReturnEncodingMismatch(selected.id));
    }
    let end = layout
        .offset
        .checked_add(
            u64::try_from(layout.bytes.len())
                .map_err(|_| TerminalWholeFunctionExitContractError::OffsetOverflow)?,
        )
        .ok_or(TerminalWholeFunctionExitContractError::OffsetOverflow)?;
    let block_end = layout_block
        .offset
        .checked_add(layout_block.byte_count)
        .ok_or(TerminalWholeFunctionExitContractError::OffsetOverflow)?;
    if end != block_end {
        return Err(TerminalWholeFunctionExitContractError::ReturnPlacementMismatch(selected.id));
    }
    let mechanism = match target.architecture {
        Architecture::X86_64 => {
            if effects.memory
                != (TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
                    stack_pointer,
                    byte_count: 8,
                })
                || effects.stack
                    != (TerminalMachineEncodedStackEffect::PopBytesV1 {
                        stack_pointer,
                        byte_count: 8,
                    })
                || effects.control
                    != TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1
                || bytes != [0xc3]
            {
                return Err(
                    TerminalWholeFunctionExitContractError::ReturnEffectsMismatch(selected.id),
                );
            }
            TerminalWholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                stack_pointer,
                read_bytes: 8,
                pop_bytes: 8,
            }
        }
        Architecture::Aarch64 => {
            let link_register = link_register.ok_or(
                TerminalWholeFunctionExitContractError::ReturnEffectsMismatch(selected.id),
            )?;
            if effects.memory != TerminalMachineEncodedMemoryEffect::NoneV1
                || effects.stack != TerminalMachineEncodedStackEffect::UnchangedV1
                || effects.control
                    != (TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                        target: link_register,
                    })
                || bytes != [0xc0, 0x03, 0x5f, 0xd6]
            {
                return Err(
                    TerminalWholeFunctionExitContractError::ReturnEffectsMismatch(selected.id),
                );
            }
            TerminalWholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
                stack_pointer,
                link_register,
            }
        }
    };
    if effects.trap != TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1 {
        return Err(TerminalWholeFunctionExitContractError::ReturnEffectsMismatch(selected.id));
    }
    Ok(TerminalWholeFunctionReturnEvidence {
        block,
        psi_return_edge,
        instruction: selected.id,
        offset: layout.offset,
        bytes: layout.bytes.clone(),
        result_virtual_register: operand.virtual_register,
        result_view: operand.view,
        result_units: operand.storage_units.clone(),
        trap: effects.trap,
        mechanism,
    })
}

fn contract_identity(
    contract: &TerminalWholeFunctionExitContract,
) -> TerminalWholeFunctionExitContractIdentity {
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
        TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1 => hasher.update([1]),
        TerminalWholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
            relaxation,
        } => {
            hasher.update([2]);
            hasher.update(relaxation.bytes());
        }
        TerminalWholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
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
        TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => {
            hasher.update([1]);
        }
        TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
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
    for function in &contract.functions {
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
            hasher.update(returned.result_virtual_register.0.to_le_bytes());
            hasher.update(returned.result_view.0.to_le_bytes());
            encode_units(&mut hasher, &returned.result_units);
            hasher.update([1]);
            match returned.mechanism {
                TerminalWholeFunctionReturnMechanism::X86ActivationStackReturnV1 {
                    stack_pointer,
                    read_bytes,
                    pop_bytes,
                } => {
                    hasher.update([1]);
                    hasher.update(stack_pointer.0.to_le_bytes());
                    hasher.update(read_bytes.to_le_bytes());
                    hasher.update(pop_bytes.to_le_bytes());
                }
                TerminalWholeFunctionReturnMechanism::Aarch64LinkRegisterReturnV1 {
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
    TerminalWholeFunctionExitContractIdentity(hasher.finalize().into())
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

fn policy_tag(policy: TerminalWholeFunctionExitPolicy) -> u8 {
    match policy {
        TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        TerminalWholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        TerminalWholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
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
        layout_custody: TerminalWholeFunctionExitLayoutCustody,
    ) -> TerminalWholeFunctionExitContract {
        let mut contract = TerminalWholeFunctionExitContract {
            identity: TerminalWholeFunctionExitContractIdentity::from_bytes([0; 32]),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            post_allocation_manifest:
                omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                    [2; 32],
                ),
            post_allocation_machine:
                omega_machine_optimizer::TerminalPostAllocationMachineIdentity::from_bytes([3; 32]),
            register_environment:
                omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            physical_register_model:
                omega_register_model::PhysicalRegisterModelIdentity::from_bytes([5; 32]),
            pre_layout: TerminalSelectedFormEncodingIdentity::from_bytes([6; 32]),
            resolved_layout: TerminalResolvedSelectedFormLayoutIdentity::from_bytes([7; 32]),
            layout_custody,
            target: NativeTarget::linux_x64(),
            policy: TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1,
            hardening: TerminalWholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
            entry_assumption:
                TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1,
            stack_pointer: RegisterViewId(0),
            stack_alignment: 16,
            red_zone_bytes: 128,
            result_view: RegisterViewId(1),
            callee_saved_units: Vec::new(),
            functions: Vec::new(),
        };
        contract.identity = contract_identity(&contract);
        contract
    }

    #[test]
    fn layout_custody_and_relaxation_receipt_are_identity_bound() {
        let baseline =
            contract_with_custody(TerminalWholeFunctionExitLayoutCustody::BaselineNearLayoutV1);
        let relaxed = contract_with_custody(
            TerminalWholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: TerminalX86BranchRelaxationIdentity::from_bytes([8; 32]),
            },
        );
        let another_relaxation = contract_with_custody(
            TerminalWholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation: TerminalX86BranchRelaxationIdentity::from_bytes([9; 32]),
            },
        );

        assert_ne!(baseline.identity, relaxed.identity);
        assert_ne!(relaxed.identity, another_relaxation.identity);
    }
}
