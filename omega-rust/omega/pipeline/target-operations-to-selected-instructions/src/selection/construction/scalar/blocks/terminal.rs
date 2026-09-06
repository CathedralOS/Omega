//! Immediate and entry-parameter return blocks.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::super::model::ScalarConstructionContext;

pub(in crate::selection::construction::scalar) fn constant_return(
    context: &ScalarConstructionContext<'_>,
    id: SelectedBlockId,
    source_block: semantic_vocabulary::BlockId,
    materialize_id: u32,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        value,
        constant_operation,
        constant_fuel,
        ..
    } = &source.value
    else {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let keys = &context.constraints.keys;
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: vec![instruction(
            SelectedInstructionId(materialize_id),
            SelectedInstructionKind::MaterializeI64 { value: *value },
            keys.materialize_i64,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![*constant_operation],
                values: vec![source.source_value],
                fuel: constant_fuel.clone(),
                ..Default::default()
            },
            context.catalog,
        )?],
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(return_id),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[register],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                context.catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}

pub(in crate::selection::construction::scalar) fn parameter_return(
    context: &ScalarConstructionContext<'_>,
    id: SelectedBlockId,
    source_block: semantic_vocabulary::BlockId,
    return_id: u32,
    register: VirtualRegisterId,
    source: &SourceLeaf,
) -> Result<SelectedBlock, SelectedInstructionError> {
    if !matches!(source.value, SourceLeafValue::EntryParameter { .. }) {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: context.function,
        });
    }
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: Vec::new(),
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(return_id),
                SelectedInstructionKind::ReturnI64,
                context.constraints.keys.return_i64,
                &[register],
                SelectedInstructionProvenance {
                    values: vec![source.source_value],
                    edges: vec![source.return_edge],
                    fuel: source.return_fuel.clone(),
                    ..Default::default()
                },
                context.catalog,
            )?,
            psi_return_edge: source.return_edge,
        },
    })
}
