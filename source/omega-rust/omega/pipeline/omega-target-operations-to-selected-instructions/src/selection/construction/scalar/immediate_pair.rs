//! Two immediate-return arms.

use crate::selection::constraints::row;
use crate::selection::shared::*;

use super::blocks;
use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::registers;

pub(super) fn is_candidate(source: &SourceFunction) -> bool {
    !matches!(
        source.recipe,
        LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1
    ) && matches!(source.when_true.value, SourceLeafValue::Immediate { .. })
        && matches!(source.when_false.value, SourceLeafValue::Immediate { .. })
}

pub(super) fn build(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let SourceLeafValue::Immediate {
        definition_site: true_site,
        ..
    } = &context.source.when_true.value
    else {
        unreachable!("catalog selected the immediate-pair family")
    };
    let SourceLeafValue::Immediate {
        definition_site: false_site,
        ..
    } = &context.source.when_false.value
    else {
        unreachable!("catalog selected the immediate-pair family")
    };
    let result_class =
        row(context.catalog, context.constraints.keys.materialize_i64)?.operands[0].class;
    Ok(ConstructedScalarBody {
        virtual_registers: vec![
            registers::condition_input(context, 0, 0),
            registers::instruction_result(
                context,
                1,
                2,
                context.source.when_true.source_value,
                *true_site,
                result_class,
            ),
            registers::instruction_result(
                context,
                2,
                4,
                context.source.when_false.source_value,
                *false_site,
                result_class,
            ),
        ],
        blocks: vec![
            blocks::condition(context)?,
            blocks::constant_return(
                context,
                SelectedBlockId(1),
                context.source.true_block,
                2,
                3,
                VirtualRegisterId(1),
                &context.source.when_true,
            )?,
            blocks::constant_return(
                context,
                SelectedBlockId(2),
                context.source.false_block,
                4,
                5,
                VirtualRegisterId(2),
                &context.source.when_false,
            )?,
        ],
    })
}
