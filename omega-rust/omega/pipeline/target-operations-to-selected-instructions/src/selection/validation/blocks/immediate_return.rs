use crate::selection::shared::*;

use super::instruction_projection;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    function_index: usize,
    block: &SelectedBlock,
    materialize_id: u32,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        value,
        constant_operation,
        constant_fuel,
        ..
    } = &source.value
    else {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        });
    };
    if block.instructions.len() != 1 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    instruction_projection::validate(
        function_index,
        &block.instructions[0],
        SelectedInstructionId(materialize_id),
        SelectedInstructionKind::MaterializeI64 { value: *value },
        keys.materialize_i64,
        &[register],
        &SelectedInstructionProvenance {
            operations: vec![*constant_operation],
            values: vec![source.source_value],
            fuel: constant_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
    instruction_projection::validate(
        function_index,
        instruction,
        SelectedInstructionId(return_id),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[register],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}
