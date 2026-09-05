use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use omega_selected_instructions::{
    MachineEffectDeclaration, MachineSemanticKind, SelectedConstraintKeys, SelectedInstruction,
    SelectedInstructionKind, SelectedTerminator, ValidatedMachineEffectCatalog,
};
use omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::{
    FunctionMachineEffects, InstructionMachineEffects, MachineEffectError,
    PreAllocationMachineEffectPlan, StructuralUnitCallMachineEffects,
    ValidatedPreAllocationMachineEffects, pre_allocation_machine_effect_identity, receipt,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
    plan: PreAllocationMachineEffectPlan,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
    let source = selected.selected_plan();
    if !source.projected_structural_call_returns.is_empty() {
        return Err(MachineEffectError::ProjectedStructuralCallReturnUnsupported);
    }
    if target_register_environment_identity(
        source.target,
        physical,
        constraints,
        reservations,
        selected_keys,
    ) != register_environment
        || plan.register_environment != register_environment
    {
        return Err(MachineEffectError::RegisterEnvironmentMismatch);
    }
    if catalog.catalog().target != source.target || plan.target != source.target {
        return Err(MachineEffectError::CatalogTargetMismatch);
    }
    if catalog.catalog().register_constraints != constraints.identity()
        || plan.register_constraints != constraints.identity()
    {
        return Err(MachineEffectError::CatalogConstraintMismatch);
    }
    if catalog.catalog().selected_keys != copied_selected_keys(selected_keys) {
        return Err(MachineEffectError::CatalogSelectedKeysMismatch);
    }
    if plan.selected != selected.selected_identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || source.fuel_schedule != plan.fuel_schedule
        || plan.machine_effect_catalog != catalog.identity()
    {
        return Err(MachineEffectError::SelectedRootMismatch);
    }
    if source.functions.len() != plan.functions.len() {
        return Err(MachineEffectError::NonCanonicalFunction);
    }
    for (source_function, actual_function) in source.functions.iter().zip(&plan.functions) {
        if source_function.machine != actual_function.machine
            || source_function.blocks.len() != actual_function.blocks.len()
        {
            return Err(MachineEffectError::NonCanonicalFunction);
        }
        validate_function(source_function, actual_function, constraints, catalog)?;
    }
    validate_structural_functions(source, &plan, constraints, catalog)?;
    if plan.identity != pre_allocation_machine_effect_identity(&plan) {
        return Err(MachineEffectError::IdentityMismatch);
    }
    let receipt = receipt(&plan)?;
    Ok(ValidatedPreAllocationMachineEffects::new(plan, receipt))
}

fn validate_structural_functions(
    source: &omega_selected_instructions::SelectedInstructionPlan,
    plan: &PreAllocationMachineEffectPlan,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    if source.structural_unit_functions.len() != plan.structural_unit_functions.len() {
        return Err(MachineEffectError::NonCanonicalFunction);
    }
    let actual_machines = plan
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    if actual_machines.len() != plan.structural_unit_functions.len() {
        return Err(MachineEffectError::NonCanonicalFunction);
    }
    for source_function in &source.structural_unit_functions {
        let matches = plan
            .structural_unit_functions
            .iter()
            .filter(|function| function.machine == source_function.machine)
            .collect::<Vec<_>>();
        let [actual] = matches.as_slice() else {
            return Err(MachineEffectError::StructuralFunctionMismatch {
                machine: source_function.machine,
            });
        };
        if actual.block != source_function.entry_block
            || actual.return_effect != source_function.terminator.effect
            || actual.return_ownership != source_function.terminator.ownership
        {
            return Err(MachineEffectError::StructuralFunctionMismatch {
                machine: source_function.machine,
            });
        }
        validate_instruction(
            &source_function.terminator.instruction,
            &actual.return_instruction,
            constraints,
            catalog,
        )?;
        match (&source_function.call, &actual.call) {
            (Some(source_call), Some(actual_call)) => validate_structural_call(
                source_function.machine,
                source_call,
                actual_call,
                constraints,
                catalog,
            )?,
            (None, None) => {}
            _ => {
                return Err(MachineEffectError::StructuralCallMismatch {
                    machine: source_function.machine,
                });
            }
        }
    }
    Ok(())
}

fn validate_structural_call(
    machine: psi_core::MachineId,
    source: &omega_selected_instructions::SelectedStructuralUnitCallInstruction,
    actual: &StructuralUnitCallMachineEffects,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|row| row.key == source.constraint)
        .ok_or(MachineEffectError::StructuralCallMismatch { machine })?;
    let declaration = catalog
        .catalog()
        .structural_unit_call
        .ok_or(MachineEffectError::StructuralCallMismatch { machine })?;
    if !constraint.operands.is_empty()
        || source.implicit_uses != constraint.implicit_uses
        || source.implicit_defs != constraint.implicit_defs
        || source.clobbers != constraint.clobbers
        || declaration.constraint != source.constraint
        || actual.instruction != source.id
        || actual.operation != source.operation
        || actual.callee != source.callee
        || actual.constraint != source.constraint
        || actual.unit_uses != constraint.implicit_uses
        || actual.unit_defs != constraint.implicit_defs
        || actual.unit_clobbers != constraint.clobbers
        || actual.layout != source.layout
        || actual.effect != source.effect
        || actual.ownership != source.ownership
        || actual.claim_transfers != source.claim_transfers
        || actual.provenance != source.provenance
        || actual.declaration != declaration
    {
        return Err(MachineEffectError::StructuralCallMismatch { machine });
    }
    Ok(())
}

fn validate_function(
    source: &omega_selected_instructions::SelectedFunction,
    actual: &FunctionMachineEffects,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    for (source_block, actual_block) in source.blocks.iter().zip(&actual.blocks) {
        if source_block.id != actual_block.block
            || actual_block.instructions.len() != source_block.instructions.len() + 1
        {
            return Err(MachineEffectError::NonCanonicalBlock);
        }
        for (source_instruction, actual_instruction) in source_block
            .instructions
            .iter()
            .zip(&actual_block.instructions)
        {
            validate_instruction(source_instruction, actual_instruction, constraints, catalog)?;
        }
        let terminator = match &source_block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
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
    source: &SelectedInstruction,
    actual: &InstructionMachineEffects,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    let constraint = constraints
        .catalog()
        .constraints
        .iter()
        .find(|constraint| constraint.key == source.constraint)
        .ok_or(MachineEffectError::ConstraintEffectMismatch {
            instruction: source.id,
        })?;
    if source.implicit_uses != constraint.implicit_uses
        || source.implicit_defs != constraint.implicit_defs
        || source.clobbers != constraint.clobbers
    {
        return Err(MachineEffectError::ConstraintEffectMismatch {
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
        return Err(MachineEffectError::InstructionMismatch {
            instruction: source.id,
        });
    }
    Ok(())
}

fn replay_declaration<'a>(
    instruction: &SelectedInstruction,
    catalog: &'a ValidatedMachineEffectCatalog,
) -> Result<&'a MachineEffectDeclaration, MachineEffectError> {
    let semantic = match instruction.kind {
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
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            MachineSemanticKind::ConditionalBranchU64LessThan
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            MachineSemanticKind::ConditionalBranchI64LessThan
        }
        SelectedInstructionKind::ReturnI64 => MachineSemanticKind::ReturnI64,
        SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
        SelectedInstructionKind::CallI64 { .. } => MachineSemanticKind::CallI64,
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
        [] => Err(MachineEffectError::MissingDeclaration {
            instruction: instruction.id,
        }),
        _ => Err(MachineEffectError::AmbiguousDeclaration {
            instruction: instruction.id,
        }),
    }
}

fn copied_selected_keys(keys: TargetRegisterEnvironmentConstraintKeys) -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        structural_unit_call: keys.structural_unit_call,
        call_i64_2_u64_to_u64: keys.call_i64_2_u64_to_u64,
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
use std::collections::BTreeSet;
