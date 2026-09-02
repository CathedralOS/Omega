use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use omega_selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedConstraintKeys, SelectedInstruction,
    SelectedInstructionKind, SelectedTerminator, ValidatedMachineEffectCatalog,
};

use crate::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, MachineEffectError,
    PreAllocationMachineEffectPlan, StructuralUnitCallMachineEffects,
    StructuralUnitFunctionMachineEffects, pre_allocation_machine_effect_identity,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
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
    let mut structural_unit_functions = Vec::with_capacity(source.structural_unit_functions.len());
    for function in &source.structural_unit_functions {
        let call = function
            .call
            .as_ref()
            .map(|call| compute_structural_call(function.machine, call, constraints, catalog))
            .transpose()?;
        let return_instruction =
            compute_instruction(&function.terminator.instruction, constraints, catalog)?;
        structural_unit_functions.push(StructuralUnitFunctionMachineEffects {
            machine: function.machine,
            block: function.entry_block,
            call,
            return_instruction,
            return_effect: function.terminator.effect,
            return_ownership: function.terminator.ownership.clone(),
        });
    }
    let mut plan = PreAllocationMachineEffectPlan {
        identity: crate::PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: source.target,
        register_environment,
        register_constraints: constraints.identity(),
        machine_effect_catalog: catalog.identity(),
        functions,
        structural_unit_functions,
    };
    plan.identity = pre_allocation_machine_effect_identity(&plan);
    Ok(plan)
}

fn compute_structural_call(
    machine: psi_core::MachineId,
    call: &omega_selected_instructions::SelectedStructuralUnitCallInstruction,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<StructuralUnitCallMachineEffects, MachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == call.constraint)
        .ok_or(MachineEffectError::StructuralCallMismatch { machine })?;
    let declaration = catalog
        .catalog()
        .structural_unit_call
        .ok_or(MachineEffectError::StructuralCallMismatch { machine })?;
    if constraint.operands.is_empty()
        && call.implicit_uses == constraint.implicit_uses
        && call.implicit_defs == constraint.implicit_defs
        && call.clobbers == constraint.clobbers
        && declaration.constraint == call.constraint
    {
        Ok(StructuralUnitCallMachineEffects {
            instruction: call.id,
            operation: call.operation,
            callee: call.callee,
            constraint: call.constraint,
            unit_uses: call.implicit_uses.clone(),
            unit_defs: call.implicit_defs.clone(),
            unit_clobbers: call.clobbers.clone(),
            layout: call.layout,
            effect: call.effect,
            ownership: call.ownership.clone(),
            claim_transfers: call.claim_transfers.clone(),
            provenance: call.provenance.clone(),
            declaration,
        })
    } else {
        Err(MachineEffectError::StructuralCallMismatch { machine })
    }
}

fn validate_catalog_roots(
    target: omega_target::NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
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

fn terminal_selected_keys(keys: TargetRegisterEnvironmentConstraintKeys) -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        structural_unit_call: keys.structural_unit_call,
        materialize_i64: keys.materialize_i64,
        copy_i64: keys.copy_i64,
        add_i64: keys.add_i64,
        subtract_i64: keys.subtract_i64,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        compare_i64: keys.compare_i64,
        conditional_branch: keys.conditional_branch,
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
        SelectedInstructionKind::ReturnI64 => MachineSemanticKind::ReturnI64,
        SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
    }
}
