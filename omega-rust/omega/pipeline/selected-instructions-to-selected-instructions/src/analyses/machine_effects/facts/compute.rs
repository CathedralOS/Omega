use crate::ValidatedSelectedAnalysis;
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedConstraintKeys, SelectedInstruction,
    SelectedInstructionKind, SelectedTerminator, ValidatedMachineEffectCatalog,
};

use crate::MachineEffectError;
use selected_instructions::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectPlan, pre_allocation_machine_effect_identity,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_terminal_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<PreAllocationMachineEffectPlan, MachineEffectError> {
    let source = selected.selected_plan();
    if !source.projected_structural_call_returns.is_empty() {
        return Err(MachineEffectError::ProjectedStructuralCallReturnUnsupported);
    }
    if source.fuel_schedule != selected.fuel_schedule_identity() {
        return Err(MachineEffectError::SelectedRootMismatch);
    }
    if target_register_environment_identity(
        source.target,
        physical,
        constraints,
        reservations,
        selected_keys,
    ) != register_environment
    {
        return Err(MachineEffectError::RegisterEnvironmentMismatch);
    }
    validate_catalog_roots(source.target, constraints, selected_keys, catalog)?;
    let mut functions = Vec::with_capacity(source.functions.len());
    for function in &source.functions {
        let mut blocks = Vec::with_capacity(function.blocks.len());
        for block in &function.blocks {
            let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
            for instruction in &block.instructions {
                instructions.push(compute_instruction(instruction, constraints, catalog)?);
            }
            let terminator = match &block.terminator {
                SelectedTerminator::ConditionalBranch { instruction, .. }
                | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                | SelectedTerminator::Jump { instruction, .. }
                | SelectedTerminator::Return { instruction, .. } => instruction,
            };
            instructions.push(compute_instruction(terminator, constraints, catalog)?);
            blocks.push(BlockMachineEffects {
                block: block.id,
                instructions,
            });
        }
        functions.push(FunctionMachineEffects {
            machine: function.machine,
            blocks,
        });
    }
    let mut plan = PreAllocationMachineEffectPlan {
        identity: selected_instructions::PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: source.target,
        register_environment,
        register_constraints: constraints.identity(),
        machine_effect_catalog: catalog.identity(),
        functions,
    };
    plan.identity = pre_allocation_machine_effect_identity(&plan);
    Ok(plan)
}

fn validate_catalog_roots(
    target: target::NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    if catalog.catalog().target != target {
        return Err(MachineEffectError::CatalogTargetMismatch);
    }
    if catalog.catalog().register_constraints != constraints.identity() {
        return Err(MachineEffectError::CatalogConstraintMismatch);
    }
    if catalog.catalog().selected_keys != terminal_selected_keys(selected_keys) {
        return Err(MachineEffectError::CatalogSelectedKeysMismatch);
    }
    Ok(())
}

fn terminal_selected_keys(
    keys: &TargetRegisterEnvironmentConstraintKeys,
) -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        load64: keys.load64,
        store64: keys.store64,
        frame_address: keys.frame_address,
        call_unit: keys.call_unit,
        call_i64: keys.call_i64.clone(),
        materialize_i64: keys.materialize_i64,
        copy_i64: keys.copy_i64,
        add_i64: keys.add_i64,
        subtract_i64: keys.subtract_i64,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        compare_i64: keys.compare_i64,
        conditional_branch: keys.conditional_branch,
        jump: keys.jump,
        return_i64: keys.return_i64,
        return_unit: keys.return_unit,
    }
}

fn compute_instruction(
    instruction: &SelectedInstruction,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<InstructionMachineEffects, MachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == instruction.constraint)
        .ok_or(MachineEffectError::ConstraintEffectMismatch {
            instruction: instruction.id,
        })?;
    if instruction.implicit_uses != constraint.implicit_uses
        || instruction.implicit_defs != constraint.implicit_defs
        || instruction.clobbers != constraint.clobbers
    {
        return Err(MachineEffectError::ConstraintEffectMismatch {
            instruction: instruction.id,
        });
    }
    let declaration = exact_declaration(instruction, catalog)?;
    Ok(InstructionMachineEffects {
        instruction: instruction.id,
        kind: instruction.kind,
        constraint: instruction.constraint,
        unit_uses: instruction.implicit_uses.clone(),
        unit_defs: instruction.implicit_defs.clone(),
        unit_clobbers: instruction.clobbers.clone(),
        memory: declaration.memory,
        trap: declaration.trap,
        barrier: declaration.barrier,
        call: declaration.call,
        cleanup: declaration.cleanup,
        provenance: instruction.provenance.clone(),
        alternatives: declaration.alternatives.clone(),
    })
}

fn exact_declaration<'a>(
    instruction: &SelectedInstruction,
    catalog: &'a ValidatedMachineEffectCatalog,
) -> Result<&'a MachineEffectDeclaration, MachineEffectError> {
    let semantic = semantic(instruction.kind);
    let mut matches = catalog
        .catalog()
        .declarations
        .iter()
        .filter(|row| row.semantic == semantic && row.constraint == instruction.constraint);
    let Some(declaration) = matches.next() else {
        return Err(MachineEffectError::MissingDeclaration {
            instruction: instruction.id,
        });
    };
    if matches.next().is_some() {
        return Err(MachineEffectError::AmbiguousDeclaration {
            instruction: instruction.id,
        });
    }
    Ok(declaration)
}

fn semantic(kind: SelectedInstructionKind) -> MachineSemanticKind {
    match kind {
        SelectedInstructionKind::CompareI64Zero => MachineSemanticKind::CompareI64Zero,
        SelectedInstructionKind::CompareI64 => MachineSemanticKind::CompareI64,
        SelectedInstructionKind::MaterializeI64 { .. } => MachineSemanticKind::MaterializeI64,
        SelectedInstructionKind::CopyI64 => MachineSemanticKind::CopyI64,
        SelectedInstructionKind::ZeroExtendU8 => MachineSemanticKind::ZeroExtendU8,
        SelectedInstructionKind::ZeroExtendU32 => MachineSemanticKind::ZeroExtendU32,
        SelectedInstructionKind::ExactAddI64 { .. } => MachineSemanticKind::ExactAddI64,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            MachineSemanticKind::ExactAddI64Immediate
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => MachineSemanticKind::ExactSubtractI64,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            MachineSemanticKind::ExactSubtractI64Immediate
        }
        SelectedInstructionKind::ConditionalBranchNonZero => {
            MachineSemanticKind::ConditionalBranchNonZero
        }
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            MachineSemanticKind::ConditionalBranchU64LessThan
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            MachineSemanticKind::ConditionalBranchI64LessThan
        }
        SelectedInstructionKind::Jump => MachineSemanticKind::Jump,
        SelectedInstructionKind::ReturnI64 => MachineSemanticKind::ReturnI64,
        SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
        SelectedInstructionKind::CallI64 { .. } => MachineSemanticKind::CallI64,
        SelectedInstructionKind::Load64 { .. } => MachineSemanticKind::Load64,
        SelectedInstructionKind::Store64 { .. } => MachineSemanticKind::Store64,
        SelectedInstructionKind::FrameAddress { .. } => MachineSemanticKind::FrameAddress,
        SelectedInstructionKind::CallUnit { .. } => MachineSemanticKind::CallUnit,
    }
}
