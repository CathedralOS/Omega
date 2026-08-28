use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use omega_terminal_selected_instructions::{
    TerminalMachineEffectDeclaration, TerminalMachineSemanticKind, TerminalSelectedConstraintKeys,
    TerminalSelectedInstruction, TerminalSelectedInstructionKind, TerminalSelectedTerminator,
    ValidatedTerminalMachineEffectCatalog,
};

use crate::{
    TerminalBlockMachineEffects, TerminalFunctionMachineEffects, TerminalInstructionMachineEffects,
    TerminalMachineEffectError, TerminalPreAllocationMachineEffectPlan,
    TerminalStructuralUnitCallMachineEffects, TerminalStructuralUnitFunctionMachineEffects,
    terminal_pre_allocation_machine_effect_identity,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_pre_allocation_machine_effects<
    S: ValidatedTerminalSelectedAnalysis,
>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<TerminalPreAllocationMachineEffectPlan, TerminalMachineEffectError> {
    let source = selected.selected_plan();
    if source.fuel_schedule != selected.fuel_schedule_identity() {
        return Err(TerminalMachineEffectError::SelectedRootMismatch);
    }
    if target_register_environment_identity(
        source.target,
        physical,
        constraints,
        reservations,
        selected_keys,
    ) != register_environment
    {
        return Err(TerminalMachineEffectError::RegisterEnvironmentMismatch);
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
                TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
                | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
            };
            instructions.push(compute_instruction(terminator, constraints, catalog)?);
            blocks.push(TerminalBlockMachineEffects {
                block: block.id,
                instructions,
            });
        }
        functions.push(TerminalFunctionMachineEffects {
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
        structural_unit_functions.push(TerminalStructuralUnitFunctionMachineEffects {
            machine: function.machine,
            block: function.entry_block,
            call,
            return_instruction,
            return_effect: function.terminator.effect,
            return_ownership: function.terminator.ownership.clone(),
        });
    }
    let mut plan = TerminalPreAllocationMachineEffectPlan {
        identity: crate::TerminalPreAllocationMachineEffectIdentity::from_bytes([0; 32]),
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
    plan.identity = terminal_pre_allocation_machine_effect_identity(&plan);
    Ok(plan)
}

fn compute_structural_call(
    machine: psi_core::MachineId,
    call: &omega_terminal_selected_instructions::TerminalSelectedStructuralUnitCallInstruction,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<TerminalStructuralUnitCallMachineEffects, TerminalMachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == call.constraint)
        .ok_or(TerminalMachineEffectError::StructuralCallMismatch { machine })?;
    let declaration = catalog
        .catalog()
        .structural_unit_call
        .ok_or(TerminalMachineEffectError::StructuralCallMismatch { machine })?;
    if constraint.operands.is_empty()
        && call.implicit_uses == constraint.implicit_uses
        && call.implicit_defs == constraint.implicit_defs
        && call.clobbers == constraint.clobbers
        && declaration.constraint == call.constraint
    {
        Ok(TerminalStructuralUnitCallMachineEffects {
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
        Err(TerminalMachineEffectError::StructuralCallMismatch { machine })
    }
}

fn validate_catalog_roots(
    target: omega_target::NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<(), TerminalMachineEffectError> {
    if catalog.catalog().target != target {
        return Err(TerminalMachineEffectError::CatalogTargetMismatch);
    }
    if catalog.catalog().register_constraints != constraints.identity() {
        return Err(TerminalMachineEffectError::CatalogConstraintMismatch);
    }
    if catalog.catalog().selected_keys != terminal_selected_keys(selected_keys) {
        return Err(TerminalMachineEffectError::CatalogSelectedKeysMismatch);
    }
    Ok(())
}

fn terminal_selected_keys(
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> TerminalSelectedConstraintKeys {
    TerminalSelectedConstraintKeys {
        structural_unit_call: keys.structural_unit_call,
        materialize_i64: keys.materialize_i64,
        copy_i64: keys.copy_i64,
        add_i64: keys.add_i64,
        subtract_i64: keys.subtract_i64,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        conditional_branch: keys.conditional_branch,
        return_i64: keys.return_i64,
        return_unit: keys.return_unit,
    }
}

fn compute_instruction(
    instruction: &TerminalSelectedInstruction,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<TerminalInstructionMachineEffects, TerminalMachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == instruction.constraint)
        .ok_or(TerminalMachineEffectError::ConstraintEffectMismatch {
            instruction: instruction.id,
        })?;
    if instruction.implicit_uses != constraint.implicit_uses
        || instruction.implicit_defs != constraint.implicit_defs
        || instruction.clobbers != constraint.clobbers
    {
        return Err(TerminalMachineEffectError::ConstraintEffectMismatch {
            instruction: instruction.id,
        });
    }
    let declaration = exact_declaration(instruction, catalog)?;
    Ok(TerminalInstructionMachineEffects {
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
    instruction: &TerminalSelectedInstruction,
    catalog: &'a ValidatedTerminalMachineEffectCatalog,
) -> Result<&'a TerminalMachineEffectDeclaration, TerminalMachineEffectError> {
    let semantic = semantic(instruction.kind);
    let mut matches = catalog
        .catalog()
        .declarations
        .iter()
        .filter(|row| row.semantic == semantic && row.constraint == instruction.constraint);
    let Some(declaration) = matches.next() else {
        return Err(TerminalMachineEffectError::MissingDeclaration {
            instruction: instruction.id,
        });
    };
    if matches.next().is_some() {
        return Err(TerminalMachineEffectError::AmbiguousDeclaration {
            instruction: instruction.id,
        });
    }
    Ok(declaration)
}

fn semantic(kind: TerminalSelectedInstructionKind) -> TerminalMachineSemanticKind {
    match kind {
        TerminalSelectedInstructionKind::CompareI64Zero => {
            TerminalMachineSemanticKind::CompareI64Zero
        }
        TerminalSelectedInstructionKind::MaterializeI64 { .. } => {
            TerminalMachineSemanticKind::MaterializeI64
        }
        TerminalSelectedInstructionKind::CopyI64 => TerminalMachineSemanticKind::CopyI64,
        TerminalSelectedInstructionKind::ExactAddI64 { .. } => {
            TerminalMachineSemanticKind::ExactAddI64
        }
        TerminalSelectedInstructionKind::ExactAddI64Immediate { .. } => {
            TerminalMachineSemanticKind::ExactAddI64Immediate
        }
        TerminalSelectedInstructionKind::ExactSubtractI64 { .. } => {
            TerminalMachineSemanticKind::ExactSubtractI64
        }
        TerminalSelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            TerminalMachineSemanticKind::ExactSubtractI64Immediate
        }
        TerminalSelectedInstructionKind::ConditionalBranchNonZero => {
            TerminalMachineSemanticKind::ConditionalBranchNonZero
        }
        TerminalSelectedInstructionKind::ReturnI64 => TerminalMachineSemanticKind::ReturnI64,
        TerminalSelectedInstructionKind::ReturnUnit => TerminalMachineSemanticKind::ReturnUnit,
    }
}
