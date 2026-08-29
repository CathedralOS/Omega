//! Exact add/subtract arm pairs, including widened legalization recipes.

use crate::selection::constraints::row;
use crate::selection::shared::*;

use super::blocks;
use super::model::{ConstructedScalarBody, ScalarConstructionContext};
use super::registers;

pub(super) fn is_exact_add(source: &SourceFunction) -> bool {
    matches!(source.when_true.value, SourceLeafValue::ExactAdd { .. })
        && matches!(source.when_false.value, SourceLeafValue::ExactAdd { .. })
}

pub(super) fn is_exact_subtract(source: &SourceFunction) -> bool {
    matches!(
        source.when_true.value,
        SourceLeafValue::ExactSubtract { .. }
    ) && matches!(
        source.when_false.value,
        SourceLeafValue::ExactSubtract { .. }
    )
}

pub(super) fn is_widened_exact_add(source: &SourceFunction) -> bool {
    matches!(
        source.when_true.value,
        SourceLeafValue::WidenedExactAdd { .. }
    ) && matches!(
        source.when_false.value,
        SourceLeafValue::WidenedExactAdd { .. }
    )
}

pub(super) fn is_widened_exact_subtract(source: &SourceFunction) -> bool {
    matches!(
        source.when_true.value,
        SourceLeafValue::WidenedExactSubtract { .. }
    ) && matches!(
        source.when_false.value,
        SourceLeafValue::WidenedExactSubtract { .. }
    )
}

pub(super) fn build_exact_add(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    build_direct(context, true)
}

pub(super) fn build_exact_subtract(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    build_direct(context, false)
}

pub(super) fn build_widened_exact_add(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    build_widened(context, true)
}

pub(super) fn build_widened_exact_subtract(
    context: &ScalarConstructionContext<'_>,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    build_widened(context, false)
}

fn build_direct(
    context: &ScalarConstructionContext<'_>,
    add: bool,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let direct = |leaf: &SourceLeaf| match &leaf.value {
        SourceLeafValue::ExactAdd {
            definition_site,
            left,
            right,
            ..
        }
        | SourceLeafValue::ExactSubtract {
            definition_site,
            left,
            right,
            ..
        } => Some((*definition_site, left.clone(), right.clone())),
        _ => None,
    };
    let (true_site, true_left, true_right) =
        direct(&context.source.when_true).expect("catalog selected a direct exact-binary pair");
    let (false_site, false_left, false_right) =
        direct(&context.source.when_false).expect("catalog selected a direct exact-binary pair");
    let key = if add {
        context.constraints.keys.add_i64
    } else {
        context.constraints.keys.subtract_i64
    };
    let result_class = row(context.catalog, key)?.operands[2].class;
    let mut virtual_registers = vec![registers::condition(context)];
    for (id, instruction, source_value, definition_site) in [
        (1, 2, true_left.source_value, true_left.definition_site),
        (2, 3, true_right.source_value, true_right.definition_site),
        (3, 4, context.source.when_true.source_value, true_site),
        (4, 6, false_left.source_value, false_left.definition_site),
        (5, 7, false_right.source_value, false_right.definition_site),
        (6, 8, context.source.when_false.source_value, false_site),
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
        blocks: binary_blocks(context)?,
    })
}

fn build_widened(
    context: &ScalarConstructionContext<'_>,
    add: bool,
) -> Result<ConstructedScalarBody, SelectedInstructionError> {
    let widened = |leaf: &SourceLeaf| match &leaf.value {
        SourceLeafValue::WidenedExactAdd {
            widen_definition_site,
            left_temporary,
            right_temporary,
            left,
            right,
            ..
        }
        | SourceLeafValue::WidenedExactSubtract {
            widen_definition_site,
            left_temporary,
            right_temporary,
            left,
            right,
            ..
        } => Some((
            *widen_definition_site,
            *left_temporary,
            *right_temporary,
            left.clone(),
            right.clone(),
        )),
        _ => None,
    };
    let (true_site, true_left_temporary, true_right_temporary, true_left, true_right) =
        widened(&context.source.when_true).expect("catalog selected a widened exact-binary pair");
    let (false_site, false_left_temporary, false_right_temporary, false_left, false_right) =
        widened(&context.source.when_false).expect("catalog selected a widened exact-binary pair");
    let key = if add {
        context.constraints.keys.add_i64
    } else {
        context.constraints.keys.subtract_i64
    };
    let result_class = row(context.catalog, key)?.operands[2].class;
    let mut virtual_registers = vec![registers::condition(context)];
    for (id, instruction, temporary, immediate) in [
        (1, 2, true_left_temporary, true_left),
        (2, 3, true_right_temporary, true_right),
        (4, 6, false_left_temporary, false_left),
        (5, 7, false_right_temporary, false_right),
    ] {
        virtual_registers.push(registers::legalization_temporary(
            context,
            id,
            instruction,
            temporary,
            immediate.source_value,
            immediate.definition_site,
            result_class,
        ));
    }
    virtual_registers.insert(
        3,
        registers::instruction_result(
            context,
            3,
            4,
            context.source.when_true.source_value,
            true_site,
            result_class,
        ),
    );
    virtual_registers.push(registers::instruction_result(
        context,
        6,
        8,
        context.source.when_false.source_value,
        false_site,
        result_class,
    ));
    Ok(ConstructedScalarBody {
        virtual_registers,
        blocks: binary_blocks(context)?,
    })
}

fn binary_blocks(
    context: &ScalarConstructionContext<'_>,
) -> Result<Vec<SelectedBlock>, SelectedInstructionError> {
    Ok(vec![
        blocks::condition(context)?,
        blocks::exact_binary_return(
            context,
            SelectedBlockId(1),
            context.source.true_block,
            [2, 3, 4, 5],
            [
                VirtualRegisterId(1),
                VirtualRegisterId(2),
                VirtualRegisterId(3),
            ],
            &context.source.when_true,
        )?,
        blocks::exact_binary_return(
            context,
            SelectedBlockId(2),
            context.source.false_block,
            [6, 7, 8, 9],
            [
                VirtualRegisterId(4),
                VirtualRegisterId(5),
                VirtualRegisterId(6),
            ],
            &context.source.when_false,
        )?,
    ])
}
