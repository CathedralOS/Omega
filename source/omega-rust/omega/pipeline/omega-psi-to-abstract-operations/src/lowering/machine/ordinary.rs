//! Ordinary-machine roster construction, block traversal, and final assembly.

use super::*;

pub(super) fn lower_ordinary_machine(
    machine: &TerminalMachine,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    retain_payloadless_for_optimization: bool,
) -> Result<AbstractFunction, LoweringError> {
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
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter()),
        )
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
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
            } => Some((place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut lowered_unit_affine_locals = Vec::new();
    let mut lowered_byte_sequence_literals = 0_usize;

    for block in &machine.blocks {
        block_entries.push(AbstractBlockEntry {
            block: block.id,
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
                structural_types,
                retain_payloadless_for_optimization,
                &value_types,
                &byte_sequence_literals,
                &unit_affine_locals,
                &mut lowered_unit_affine_locals,
                &mut lowered_byte_sequence_literals,
                &mut operations,
            )?;
        }
        lower_terminator(
            block,
            machine,
            &blocks,
            result,
            &lowered_unit_affine_locals,
            retain_payloadless_for_optimization,
            &mut operations,
        )?;
    }

    Ok(AbstractFunction {
        machine: machine.id,
        attachment: machine.attachment,
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
            psi_terminal::TerminalMachineResult::Unit => AbstractFunctionResult::Unit,
            psi_terminal::TerminalMachineResult::Scalar(result) => {
                AbstractFunctionResult::Scalar(AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                })
            }
            psi_terminal::TerminalMachineResult::Structural(result) => {
                AbstractFunctionResult::Structural(result.clone())
            }
        },
        entry_claims: machine.entry_claims.clone(),
        published_service_ceiling: machine.published_service_ceiling.clone(),
        block_entries,
        operations,
    })
}
