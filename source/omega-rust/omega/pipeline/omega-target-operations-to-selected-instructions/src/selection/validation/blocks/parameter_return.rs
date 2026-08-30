use crate::selection::shared::*;

use super::instruction_projection;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    function_index: usize,
    block: &SelectedBlock,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    if !matches!(source.value, SourceLeafValue::EntryParameter { .. })
        || !block.instructions.is_empty()
    {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: block.id.0,
        });
    }
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
