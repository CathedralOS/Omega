use super::blocks::validate_selected_blocks;
use super::integrity::{
    validate_block_constraints, validate_def_use, validate_dense, validate_provenance_partition,
};
use super::virtual_registers::validate_virtual_registers;
use crate::selection::constraints::row;
use crate::selection::construction::{structural_call_row, structural_unit_layout};
use crate::selection::shared::*;

pub(super) fn validate_function(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if function.machine != source.machine
        || function.attachment != source.attachment
        || function.provenance != source.provenance
        || function.entry_block != SelectedBlockId(0)
    {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    validate_dense(function_index, source, function)?;
    validate_virtual_registers(
        function_index,
        source,
        function,
        constraints,
        physical,
        catalog,
    )?;
    validate_selected_blocks(function_index, source, function, constraints.keys, catalog)?;
    for block in &function.blocks {
        validate_block_constraints(function_index, block, function, catalog)?;
    }
    validate_def_use(function_index, function, catalog)?;
    validate_provenance_partition(function_index, source, function)?;
    Ok(())
}

pub(super) fn validate_unit_function(
    function_index: usize,
    source: &SourceUnitFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let expected_provenance = SelectedInstructionProvenance {
        edges: vec![source.return_edge],
        fuel: source.return_fuel.clone(),
        ..Default::default()
    };
    let valid_shape = function.machine == source.machine
        && function.attachment == source.attachment
        && function.provenance == source.provenance
        && function.entry_block == SelectedBlockId(0)
        && function.virtual_registers.is_empty()
        && function.blocks.len() == 1
        && function.blocks[0].id == SelectedBlockId(0)
        && function.blocks[0].source_block == source.entry_block
        && function.blocks[0].instructions.is_empty();
    if !valid_shape {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    let block = &function.blocks[0];
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    };
    if instruction.id != SelectedInstructionId(0)
        || instruction.kind != SelectedInstructionKind::ReturnUnit
        || instruction.constraint != keys.return_unit
        || !instruction.operands.is_empty()
        || instruction.provenance != expected_provenance
        || *psi_return_edge != source.return_edge
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function: function_index,
            instruction: instruction.id.0,
        });
    }
    validate_block_constraints(function_index, block, function, catalog)?;
    validate_def_use(function_index, function, catalog)
}

pub(super) fn validate_structural_unit_function(
    function_index: usize,
    source: &SourceStructuralUnitFunction,
    selected: &SelectedStructuralUnitFunction,
    plan: &LegalizedOperationPlan,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if plan.target != omega_target::NativeTarget::uefi_x64() {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        });
    }
    let layout = structural_unit_layout(function_index, source)?;
    let parameters_match = selected.abi.parameters.len() == source.parameters.len()
        && selected
            .abi
            .parameters
            .iter()
            .zip(&source.parameters)
            .all(|(selected, source)| {
                selected.semantic == source.semantic && selected.target == source.target
            });
    if selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.structural_types != source.structural_types
        || selected.abi.recipe != SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1
        || selected.abi.call_plan != source.call_plan
        || !parameters_match
        || selected.abi.layout != layout
        || selected.structural_places != source.structural_places
        || selected.entry_claims != source.entry_claims
        || selected.published_service_ceiling != source.published_service_ceiling
        || selected.entry_block != SelectedBlockId(0)
        || selected.source_entry_block != source.entry_block
        || selected.boundary_settlements != source.boundary_settlements
    {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }

    match (&source.call, &selected.call) {
        (None, None) => {}
        (Some(source_call), Some(selected_call)) => {
            let Some(callee) = plan
                .structural_unit_functions
                .iter()
                .find(|candidate| candidate.machine == source_call.callee)
            else {
                return Err(SelectedInstructionError::SourceCustodyMismatch);
            };
            let callee_layout = structural_unit_layout(function_index, callee)?;
            let row = structural_call_row(function_index, keys, catalog)?;
            let arguments_match = selected_call.arguments.len() == source_call.arguments.len()
                && selected_call
                    .arguments
                    .iter()
                    .zip(&source_call.arguments)
                    .all(|(selected, source)| {
                        selected.semantic == source.semantic && selected.target == source.target
                    });
            let call_shape_valid = source_call.arguments.len() == 2
                && source_call
                    .arguments
                    .iter()
                    .enumerate()
                    .all(|(index, argument)| {
                        argument.semantic.access == StructuralAccess::Owned
                            && argument.semantic.path.is_empty()
                            && argument.target.place == argument.semantic.place
                            && argument.target.access == argument.semantic.access
                            && argument.target.path == argument.semantic.path
                            && argument.target.root_structural_type
                                == source.parameters[index].semantic.structural_type
                            && argument.target.structural_type
                                == callee.parameters[index].semantic.structural_type
                            && argument.target.source_byte_offset == 0
                            && argument.target.fixed_array_length.is_none()
                            && argument.target.element_stride.is_none()
                            && argument.target.shape == source.parameters[index].target.shape
                            && argument.target.source == source.parameters[index].target.placement
                            && argument.target.destination
                                == callee.parameters[index].target.placement
                    });
            if callee.call_plan != source.call_plan
                || callee_layout != layout
                || !call_shape_valid
                || selected_call.id != SelectedInstructionId(0)
                || selected_call.source != source_call.source
                || selected_call.operation != source_call.operation
                || selected_call.callee != source_call.callee
                || selected_call.caller_call_plan != source.call_plan
                || selected_call.callee_call_plan != callee.call_plan
                || !arguments_match
                || selected_call.claim_transfers != source_call.claim_transfers
                || selected_call.layout != layout
                || selected_call.constraint != row.key
                || selected_call.implicit_uses != row.implicit_uses
                || selected_call.implicit_defs != row.implicit_defs
                || selected_call.clobbers != row.clobbers
                || selected_call.provenance
                    != (SelectedInstructionProvenance {
                        operations: vec![source_call.operation],
                        fuel: source_call.fuel.clone(),
                        ..Default::default()
                    })
                || selected_call.effect != source_call.effect
                || selected_call.ownership != source_call.ownership
            {
                return Err(SelectedInstructionError::InstructionProjectionMismatch {
                    function: function_index,
                    instruction: selected_call.id.0,
                });
            }
        }
        _ => {
            return Err(SelectedInstructionError::FunctionProjectionMismatch {
                function: function_index,
            });
        }
    }

    let return_id = SelectedInstructionId(u32::from(source.call.is_some()));
    let instruction = &selected.terminator.instruction;
    let return_row = row(catalog, keys.return_unit)?;
    if instruction.id != return_id
        || instruction.kind != SelectedInstructionKind::ReturnUnit
        || instruction.constraint != keys.return_unit
        || !instruction.operands.is_empty()
        || instruction.implicit_uses != return_row.implicit_uses
        || instruction.implicit_defs != return_row.implicit_defs
        || instruction.clobbers != return_row.clobbers
        || instruction.provenance
            != (SelectedInstructionProvenance {
                edges: vec![source.return_edge],
                fuel: source.return_fuel.clone(),
                ..Default::default()
            })
        || selected.terminator.psi_return_edge != source.return_edge
        || selected.terminator.effect != source.return_effect
        || selected.terminator.ownership != source.return_ownership
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function: function_index,
            instruction: instruction.id.0,
        });
    }
    Ok(())
}
