use crate::selection::shared::*;

use super::instruction_projection;

pub(super) fn validate(
    function: usize,
    block: &SelectedBlock,
    source: &SourceLeaf,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &source.value else {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    };
    if block.instructions.len() != 6 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function,
            block: block.id.0,
        });
    }
    for (position, (id, register, immediate)) in [
        (2, VirtualRegisterId(1), &chain.resident),
        (3, VirtualRegisterId(2), &chain.left),
        (4, VirtualRegisterId(3), &chain.right),
    ]
    .into_iter()
    .enumerate()
    {
        instruction_projection::validate(
            function,
            &block.instructions[position],
            SelectedInstructionId(id),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            &SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    for (position, (id, registers, add, values)) in [
        (
            5,
            [
                VirtualRegisterId(2),
                VirtualRegisterId(3),
                VirtualRegisterId(4),
            ],
            &chain.inner,
            vec![
                chain.left.source_value,
                chain.right.source_value,
                chain.inner.source_value,
            ],
        ),
        (
            6,
            [
                VirtualRegisterId(1),
                VirtualRegisterId(4),
                VirtualRegisterId(5),
            ],
            &chain.middle,
            vec![
                chain.resident.source_value,
                chain.inner.source_value,
                chain.middle.source_value,
            ],
        ),
        (
            7,
            [
                VirtualRegisterId(1),
                VirtualRegisterId(5),
                VirtualRegisterId(6),
            ],
            &chain.result,
            vec![
                chain.resident.source_value,
                chain.middle.source_value,
                chain.result.source_value,
            ],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        instruction_projection::validate(
            function,
            &block.instructions[position + 3],
            SelectedInstructionId(id),
            SelectedInstructionKind::ExactAddI64 {
                obligation: add.obligation,
                accepted_fact: add.accepted_fact,
            },
            keys.add_i64,
            &registers,
            &SelectedInstructionProvenance {
                operations: vec![add.operation],
                values,
                obligations: vec![add.obligation],
                fuel: add.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
    }
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function,
            block: block.id.0,
        });
    };
    if *psi_return_edge != source.return_edge {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function,
            block: block.id.0,
        });
    }
    instruction_projection::validate(
        function,
        instruction,
        SelectedInstructionId(8),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[VirtualRegisterId(6)],
        &SelectedInstructionProvenance {
            values: vec![source.source_value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}
