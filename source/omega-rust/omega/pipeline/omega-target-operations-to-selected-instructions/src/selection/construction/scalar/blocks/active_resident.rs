//! Active-resident exact-add-chain block construction.

use crate::selection::constraints::instruction;
use crate::selection::shared::*;

use super::super::model::ScalarConstructionContext;

pub(in crate::selection::construction::scalar) fn active_resident_exact_add_chain(
    context: &ScalarConstructionContext<'_>,
    id: SelectedBlockId,
    source_block: psi_core::BlockId,
    source: &SourceLeaf,
) -> Result<SelectedBlock, SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &source.value else {
        return Err(SelectedInstructionError::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let keys = context.constraints.keys;
    let materialize = |id, register, immediate: &SourceImmediate| {
        instruction(
            SelectedInstructionId(id),
            SelectedInstructionKind::MaterializeI64 {
                value: immediate.value,
            },
            keys.materialize_i64,
            &[register],
            SelectedInstructionProvenance {
                operations: vec![immediate.constant_operation],
                values: vec![immediate.source_value],
                fuel: immediate.fuel.clone(),
                ..Default::default()
            },
            context.catalog,
        )
    };
    let exact_add = |id,
                     operands: [VirtualRegisterId; 3],
                     add: &omega_legalized_operations::LegalizedExactAdd,
                     values: Vec<psi_core::ValueId>| {
        instruction(
            SelectedInstructionId(id),
            SelectedInstructionKind::ExactAddI64 {
                obligation: add.obligation,
                accepted_fact: add.accepted_fact,
            },
            keys.add_i64,
            &operands,
            SelectedInstructionProvenance {
                operations: vec![add.operation],
                values,
                obligations: vec![add.obligation],
                fuel: add.fuel.clone(),
                ..Default::default()
            },
            context.catalog,
        )
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions: vec![
            materialize(2, VirtualRegisterId(1), &chain.resident)?,
            materialize(3, VirtualRegisterId(2), &chain.left)?,
            materialize(4, VirtualRegisterId(3), &chain.right)?,
            exact_add(
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
            )?,
            exact_add(
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
            )?,
            exact_add(
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
            )?,
        ],
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::ReturnI64,
                keys.return_i64,
                &[VirtualRegisterId(6)],
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
