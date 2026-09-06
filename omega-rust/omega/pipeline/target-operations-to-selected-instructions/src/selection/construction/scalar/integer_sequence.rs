//! A direct condition followed by an ordered exact scalar arm.

use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::{blocks, registers};
use crate::selection::constraints::{instruction, row};
use crate::selection::shared::*;

pub(super) fn is_candidate(source: &SourceFunction) -> bool {
    matches!(source.condition, LegalizedCondition::DirectParameter { .. })
        && matches!(
            source.when_true.value,
            SourceLeafValue::ExactIntegerSequence(_)
        )
        && matches!(source.when_false.value, SourceLeafValue::Immediate { .. })
}

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape {
        function: context.function,
    };
    let SourceLeafValue::ExactIntegerSequence(sequence) = &context.source.when_true.value else {
        return Err(invalid());
    };
    let SourceLeafValue::Immediate {
        definition_site, ..
    } = &context.source.when_false.value
    else {
        return Err(invalid());
    };
    let mut virtual_registers = vec![registers::condition_input(context, 0, 0)];
    let (instructions, result) = super::super::integer_sequence::build(
        context.function,
        sequence,
        context.source.when_true.source_value,
        &[],
        2,
        &mut virtual_registers,
        context.constraints.keys,
        context.catalog,
    )?;
    let return_id = 2 + u32::try_from(instructions.len()).map_err(|_| invalid())?;
    let leaf = &context.source.when_true;
    let when_true = SelectedBlock {
        id: SelectedBlockId(1),
        source_block: context.source.true_block,
        instructions,
        terminator: SelectedTerminator::Return {
            instruction: instruction(
                SelectedInstructionId(return_id),
                SelectedInstructionKind::ReturnI64,
                context.constraints.keys.return_i64,
                &[result],
                SelectedInstructionProvenance {
                    values: vec![leaf.source_value],
                    edges: vec![leaf.return_edge],
                    fuel: leaf.return_fuel.clone(),
                    ..Default::default()
                },
                context.catalog,
            )?,
            psi_return_edge: leaf.return_edge,
        },
    };
    let false_register =
        VirtualRegisterId(u32::try_from(virtual_registers.len()).map_err(|_| invalid())?);
    virtual_registers.push(registers::instruction_result(
        context,
        false_register.0,
        return_id + 1,
        context.source.when_false.source_value,
        *definition_site,
        row(context.catalog, context.constraints.keys.materialize_i64)?.operands[0].class,
    ));
    Ok(ConstructedScalarBody {
        virtual_registers,
        blocks: vec![
            blocks::condition(context)?,
            when_true,
            blocks::constant_return(
                context,
                SelectedBlockId(2),
                context.source.false_block,
                return_id + 1,
                return_id + 2,
                false_register,
                &context.source.when_false,
            )?,
        ],
    })
}
