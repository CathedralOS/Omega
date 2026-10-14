//! Ordinary-machine roster construction, block traversal, and final assembly.
use crate::lowering::LoweringError;
use semantic_vocabulary::OperationId;
mod operation;
mod terminator;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractParameter,
};
use operation::lower_operation;
use semantic_vocabulary::StructuralPlaceKind;
use std::collections::BTreeMap;
use terminal_psi::TerminalMachine;
use terminator::lower_terminator;

type StructuralLiteral<'a> = (
    &'a terminal_psi::StructuralPlaceDeclaration,
    u32,
    semantic_vocabulary::StructuralTypeId,
);
type LoweredAffineLocal = (
    OperationId,
    terminal_psi::StructuralPlaceDeclaration,
    terminal_psi::StructuralTypeDeclaration,
);

pub(super) fn lower_machine(
    module: &terminal_psi::TerminalModule,
    machine: &TerminalMachine,
) -> Result<AbstractFunction, LoweringError> {
    let structural_types = &module.structural_types;
    let result = machine.result.scalar();
    let blocks = machine
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut operations = Vec::new();
    let mut block_entries = Vec::with_capacity(machine.blocks.len());
    let value_types = machine
        .parameters
        .iter()
        .chain(result.iter())
        .chain(machine.block_parameters())
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|operation| operation.result.scalar_ref())
        }))
        .map(|value| (value.id, value.scalar_type))
        .collect::<BTreeMap<_, _>>();
    let byte_sequence_literals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let unit_affine_locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                ..
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut lowered_unit_affine_locals = Vec::new();

    for block in &machine.blocks {
        block_entries.push(AbstractBlockEntry {
            block: block.id,
            structural_parameters: block.structural_parameters.clone(),
            parameters: block
                .parameters
                .iter()
                .map(|parameter| AbstractParameter {
                    value: parameter.id,
                    scalar_type: parameter.scalar_type,
                })
                .collect(),
            operation_offset: operations.len(),
        });
        for operation in &block.operations {
            lower_operation(
                operation,
                block,
                machine,
                module,
                structural_types,
                &module.placed_view_plans,
                &value_types,
                &byte_sequence_literals,
                &unit_affine_locals,
                &mut lowered_unit_affine_locals,
                &mut operations,
            )?;
        }
        lower_terminator(module, block, machine, &blocks, result, &mut operations)?;
    }

    Ok(AbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
        nominal_cleanup_subject: machine.nominal_cleanup_subject,
        entry: machine.entry,
        parameters: machine
            .parameters
            .iter()
            .map(|parameter| AbstractParameter {
                value: parameter.id,
                scalar_type: parameter.scalar_type,
            })
            .collect(),
        structural_parameters: machine.structural_parameters.clone(),
        result: match &machine.result {
            terminal_psi::TerminalMachineResult::Unit => AbstractFunctionResult::Unit,
            terminal_psi::TerminalMachineResult::Scalar(result) => {
                AbstractFunctionResult::Scalar((*result).into())
            }
            terminal_psi::TerminalMachineResult::Structural(result) => {
                AbstractFunctionResult::Structural(result.clone())
            }
        },
        entry_claims: machine.entry_claims.clone(),
        block_entry_claims: machine.block_entry_claims.clone(),
        published_service_ceiling: machine.published_service_ceiling.clone(),
        block_entries,
        operations,
    })
}
