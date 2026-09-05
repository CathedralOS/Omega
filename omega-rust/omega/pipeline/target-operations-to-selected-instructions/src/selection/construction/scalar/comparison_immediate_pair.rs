//! Parameter-comparison condition controlling two immediate-return arms.

use crate::selection::constraints::row;
use crate::selection::shared::*;

use super::blocks;
use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::registers;

pub(super) fn is_candidate(source: &SourceFunction) -> bool {
    matches!(
        source.recipe,
        LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1
            | LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1
            | LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1
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
        unreachable!("catalog selected comparison immediate pair")
    };
    let SourceLeafValue::Immediate {
        definition_site: false_site,
        ..
    } = &context.source.when_false.value
    else {
        unreachable!("catalog selected comparison immediate pair")
    };
    let result_class =
        row(context.catalog, context.constraints.keys.materialize_i64)?.operands[0].class;
    Ok(ConstructedScalarBody {
        virtual_registers: vec![
            registers::condition_input(context, 0, 0),
            registers::condition_input(context, 1, 1),
            registers::instruction_result(
                context,
                2,
                2,
                context.source.when_true.source_value,
                *true_site,
                result_class,
            ),
            registers::instruction_result(
                context,
                3,
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
                VirtualRegisterId(2),
                &context.source.when_true,
            )?,
            blocks::constant_return(
                context,
                SelectedBlockId(2),
                context.source.false_block,
                4,
                5,
                VirtualRegisterId(3),
                &context.source.when_false,
            )?,
        ],
    })
}
