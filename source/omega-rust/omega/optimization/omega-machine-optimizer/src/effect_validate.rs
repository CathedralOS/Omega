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
    TerminalFunctionMachineEffects, TerminalInstructionMachineEffects, TerminalMachineEffectError,
    TerminalPreAllocationMachineEffectPlan, ValidatedTerminalPreAllocationMachineEffects, receipt,
    terminal_pre_allocation_machine_effect_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_pre_allocation_machine_effects<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedTerminalMachineEffectCatalog,
    plan: TerminalPreAllocationMachineEffectPlan,
) -> Result<ValidatedTerminalPreAllocationMachineEffects, TerminalMachineEffectError> {
    let source = selected.selected_plan();
    if target_register_environment_identity(
        source.target,
        physical,
        constraints,
        reservations,
        selected_keys,
    ) != register_environment
        || plan.register_environment != register_environment
    {
        return Err(TerminalMachineEffectError::RegisterEnvironmentMismatch);
    }
    if catalog.catalog().target != source.target || plan.target != source.target {
        return Err(TerminalMachineEffectError::CatalogTargetMismatch);
    }
    if catalog.catalog().register_constraints != constraints.identity()
        || plan.register_constraints != constraints.identity()
    {
        return Err(TerminalMachineEffectError::CatalogConstraintMismatch);
    }
    if catalog.catalog().selected_keys != copied_selected_keys(selected_keys) {
        return Err(TerminalMachineEffectError::CatalogSelectedKeysMismatch);
    }
    if plan.selected != selected.selected_identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || source.fuel_schedule != plan.fuel_schedule
        || plan.machine_effect_catalog != catalog.identity()
    {
        return Err(TerminalMachineEffectError::SelectedRootMismatch);
    }
    if source.functions.len() != plan.functions.len() {
        return Err(TerminalMachineEffectError::NonCanonicalFunction);
    }
    for (source_function, actual_function) in source.functions.iter().zip(&plan.functions) {
        if source_function.machine != actual_function.machine
            || source_function.blocks.len() != actual_function.blocks.len()
        {
            return Err(TerminalMachineEffectError::NonCanonicalFunction);
        }
        validate_function(source_function, actual_function, constraints, catalog)?;
    }
    if plan.identity != terminal_pre_allocation_machine_effect_identity(&plan) {
        return Err(TerminalMachineEffectError::IdentityMismatch);
    }
    let receipt = receipt(&plan)?;
    Ok(ValidatedTerminalPreAllocationMachineEffects::new(
        plan, receipt,
    ))
}

fn validate_function(
    source: &omega_terminal_selected_instructions::TerminalSelectedFunction,
    actual: &TerminalFunctionMachineEffects,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<(), TerminalMachineEffectError> {
    for (source_block, actual_block) in source.blocks.iter().zip(&actual.blocks) {
        if source_block.id != actual_block.block
            || actual_block.instructions.len() != source_block.instructions.len() + 1
        {
            return Err(TerminalMachineEffectError::NonCanonicalBlock);
        }
        for (source_instruction, actual_instruction) in source_block
            .instructions
            .iter()
            .zip(&actual_block.instructions)
        {
            validate_instruction(source_instruction, actual_instruction, constraints, catalog)?;
        }
        let terminator = match &source_block.terminator {
            TerminalSelectedTerminator::ConditionalBranch { instruction, .. }
            | TerminalSelectedTerminator::Return { instruction, .. } => instruction,
        };
        validate_instruction(
            terminator,
            actual_block
                .instructions
                .last()
                .expect("validated sidecar block has a terminator row"),
            constraints,
            catalog,
        )?;
    }
    Ok(())
}

fn validate_instruction(
    source: &TerminalSelectedInstruction,
    actual: &TerminalInstructionMachineEffects,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<(), TerminalMachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|constraint| constraint.key == source.constraint)
        .ok_or(TerminalMachineEffectError::ConstraintEffectMismatch {
            instruction: source.id,
        })?;
    if source.implicit_uses != constraint.implicit_uses
        || source.implicit_defs != constraint.implicit_defs
        || source.clobbers != constraint.clobbers
    {
        return Err(TerminalMachineEffectError::ConstraintEffectMismatch {
            instruction: source.id,
        });
    }
    let declaration = replay_declaration(source, catalog)?;
    if actual.instruction != source.id
        || actual.kind != source.kind
        || actual.constraint != source.constraint
        || actual.unit_uses != constraint.implicit_uses
        || actual.unit_defs != constraint.implicit_defs
        || actual.unit_clobbers != constraint.clobbers
        || actual.memory != declaration.memory
        || actual.trap != declaration.trap
        || actual.barrier != declaration.barrier
        || actual.call != declaration.call
        || actual.cleanup != declaration.cleanup
        || actual.provenance != source.provenance
        || actual.alternatives != declaration.alternatives
    {
        return Err(TerminalMachineEffectError::InstructionMismatch {
            instruction: source.id,
        });
    }
    Ok(())
}

fn replay_declaration<'a>(
    instruction: &TerminalSelectedInstruction,
    catalog: &'a ValidatedTerminalMachineEffectCatalog,
) -> Result<&'a TerminalMachineEffectDeclaration, TerminalMachineEffectError> {
    let semantic = match instruction.kind {
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
    };
    let declarations = catalog
        .catalog()
        .declarations
        .iter()
        .filter(|declaration| {
            declaration.semantic == semantic && declaration.constraint == instruction.constraint
        })
        .collect::<Vec<_>>();
    match declarations.as_slice() {
        [declaration] => Ok(*declaration),
        [] => Err(TerminalMachineEffectError::MissingDeclaration {
            instruction: instruction.id,
        }),
        _ => Err(TerminalMachineEffectError::AmbiguousDeclaration {
            instruction: instruction.id,
        }),
    }
}

fn copied_selected_keys(
    keys: TargetRegisterEnvironmentConstraintKeys,
) -> TerminalSelectedConstraintKeys {
    TerminalSelectedConstraintKeys {
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
