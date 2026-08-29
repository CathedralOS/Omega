//! Active-resident exact-add chain paired with one false-arm immediate.

use crate::selection::constraints::row;
use crate::selection::shared::*;

use super::blocks;
use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::registers;

pub(super) fn is_candidate(source: &SourceFunction) -> bool {
    matches!(
        source.when_true.value,
        SourceLeafValue::ActiveResidentExactAddChain(..)
    ) && matches!(source.when_false.value, SourceLeafValue::Immediate { .. })
}

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let SourceLeafValue::ActiveResidentExactAddChain(chain) = &context.source.when_true.value
    else {
        unreachable!("catalog selected the active-resident family")
    };
    let SourceLeafValue::Immediate {
        definition_site: false_site,
        ..
    } = &context.source.when_false.value
    else {
        unreachable!("catalog selected the active-resident family")
    };
    let result_class = row(context.catalog, context.constraints.keys.add_i64)?.operands[2].class;
    let mut virtual_registers = vec![registers::condition(context)];
    for (id, instruction, source_value, definition_site) in [
        (
            1,
            2,
            chain.resident.source_value,
            chain.resident.definition_site,
        ),
        (2, 3, chain.left.source_value, chain.left.definition_site),
        (3, 4, chain.right.source_value, chain.right.definition_site),
        (4, 5, chain.inner.source_value, chain.inner.definition_site),
        (
            5,
            6,
            chain.middle.source_value,
            chain.middle.definition_site,
        ),
        (
            6,
            7,
            chain.result.source_value,
            chain.result.definition_site,
        ),
        (7, 9, context.source.when_false.source_value, *false_site),
    ] {
        virtual_registers.push(registers::instruction_result(
            context,
            id,
            instruction,
            source_value,
            definition_site,
            result_class,
        ));
    }
    Ok(ConstructedScalarBody {
        virtual_registers,
        blocks: vec![
            blocks::condition(context)?,
            blocks::active_resident_exact_add_chain(
                context,
                SelectedBlockId(1),
                context.source.true_block,
                &context.source.when_true,
            )?,
            blocks::constant_return(
                context,
                SelectedBlockId(2),
                context.source.false_block,
                9,
                10,
                VirtualRegisterId(7),
                &context.source.when_false,
            )?,
        ],
    })
}
