use omega_isa_x86_64::{
    validate_x86_64_register_constraint_catalog,
    validate_x86_64_selected_structural_unit_call_template, x86_64_register_constraint_catalog,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_target::Architecture;

use crate::StagedOptimizedPostAllocationMachinePlan;

use super::{
    super::{
        OptimizedSelectedFormEncodingError, SelectedFormEncodingState,
        SelectedFormMachineDisposition, SelectedStructuralUnitFunctionEncoding,
    },
    row,
};

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    candidate_functions: &[SelectedStructuralUnitFunctionEncoding],
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let selected_plan = selected.selected_plan();
    let machine = staged.machine().plan();
    let effects = staged.effects().plan();
    let count = selected_plan.structural_unit_functions.len();
    if count != machine.structural_unit_functions.len()
        || count != effects.structural_unit_functions.len()
        || count != candidate_functions.len()
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    if count == 0 {
        return Ok(());
    }
    if selected_plan.target.architecture != Architecture::X86_64 {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    let constraints = validate_x86_64_register_constraint_catalog(
        x86_64_register_constraint_catalog(physical),
        physical,
    )
    .map_err(|_| OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch)?;
    if constraints.identity() != machine.register_constraints
        || constraints.identity() != effects.register_constraints
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralConstraintCatalogMismatch);
    }

    for (((selected_function, machine_function), effect_function), candidate) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(&machine.structural_unit_functions)
        .zip(&effects.structural_unit_functions)
        .zip(candidate_functions)
    {
        if selected_function.machine != machine_function.machine
            || selected_function.machine != effect_function.machine
            || selected_function.entry_block != machine_function.block
            || selected_function.entry_block != effect_function.block
            || candidate.machine != selected_function.machine
            || candidate.block != selected_function.entry_block
            || machine_function.call != effect_function.call
            || machine_function.return_effect != effect_function.return_effect
            || machine_function.return_ownership != effect_function.return_ownership
        {
            return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
        }
        match (
            &selected_function.call,
            &machine_function.call,
            &candidate.call,
        ) {
            (None, None, None) => {}
            (Some(selected_call), Some(machine_call), Some(candidate_call)) => {
                if machine_call.instruction != selected_call.id
                    || machine_call.operation != selected_call.operation
                    || machine_call.callee != selected_call.callee
                    || machine_call.constraint != selected_call.constraint
                    || machine_call.unit_uses != selected_call.implicit_uses
                    || machine_call.unit_defs != selected_call.implicit_defs
                    || machine_call.unit_clobbers != selected_call.clobbers
                    || machine_call.layout != selected_call.layout
                    || machine_call.effect != selected_call.effect
                    || machine_call.ownership != selected_call.ownership
                    || machine_call.claim_transfers != selected_call.claim_transfers
                    || machine_call.provenance != selected_call.provenance
                    || selected_plan
                        .structural_unit_functions
                        .iter()
                        .filter(|function| function.machine == selected_call.callee)
                        .count()
                        != 1
                    || candidate_call.instruction != selected_call.id
                    || candidate_call.operation != selected_call.operation
                    || candidate_call.callee != selected_call.callee
                {
                    return Err(
                        OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(
                            selected_call.id,
                        ),
                    );
                }
                let decoded = validate_x86_64_selected_structural_unit_call_template(
                    selected_plan.target,
                    physical,
                    &constraints,
                    selected_call,
                    machine_call.declaration,
                    &candidate_call.bytes,
                )
                .map_err(|_| OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
                if candidate_call.bytes != decoded.bytes()
                    || candidate_call.footprint.as_ref() != decoded.footprint()
                    || candidate_call.fixup != decoded.fixup()
                {
                    return Err(OptimizedSelectedFormEncodingError::ArtifactMismatch);
                }
            }
            (Some(call), _, _) => {
                return Err(
                    OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(call.id),
                );
            }
            (_, Some(call), _) => {
                return Err(
                    OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(
                        call.instruction,
                    ),
                );
            }
            (_, _, Some(call)) => {
                return Err(
                    OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(
                        call.instruction,
                    ),
                );
            }
        }

        let selected_return = &selected_function.terminator.instruction;
        if selected_return.id != machine_function.return_instruction.instruction
            || selected_return.id != effect_function.return_instruction.instruction
            || selected_return.kind != effect_function.return_instruction.kind
            || selected_return.provenance != machine_function.return_provenance
            || selected_return.provenance != effect_function.return_instruction.provenance
            || selected_function.terminator.effect != machine_function.return_effect
            || selected_function.terminator.ownership != machine_function.return_ownership
            || selected_function.terminator.effect != effect_function.return_effect
            || selected_function.terminator.ownership != effect_function.return_ownership
            || !effect_function
                .return_instruction
                .alternatives
                .contains(&machine_function.return_instruction.alternative)
        {
            return Err(
                OptimizedSelectedFormEncodingError::StructuralReturnRosterMismatch(
                    selected_return.id,
                ),
            );
        }
        row::validate(
            selected_plan.target,
            selected_return,
            &machine_function.return_instruction,
            physical,
            &SelectedFormMachineDisposition::RetainedV1,
            None,
            &candidate.return_instruction,
        )?;
        if !matches!(
            candidate.return_instruction.state,
            SelectedFormEncodingState::Encoded { ref bytes, .. } if bytes.as_slice() == [0xc3]
        ) {
            return Err(
                OptimizedSelectedFormEncodingError::StructuralReturnRosterMismatch(
                    selected_return.id,
                ),
            );
        }
    }
    Ok(())
}
