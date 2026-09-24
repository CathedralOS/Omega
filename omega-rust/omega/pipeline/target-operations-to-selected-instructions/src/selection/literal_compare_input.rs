//! Zero keeps flag-only comparison; U12 literals use the immediate comparison
//! form. A literal folds only when the comparison is its sole use, so the
//! literal needs no register. Construction and validation read the same
//! legalized source through this one decision.
use std::ops::RangeInclusive;

use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

fn folded_literal<'a>(
    function: &LegalizedScalarFunction,
    block: &'a LegalizedScalarBlock,
    comparison_index: usize,
    immediates: RangeInclusive<u64>,
) -> Option<&'a LegalizedScalarInstruction> {
    let comparison = block.instructions.get(comparison_index)?;
    let literal = block.instructions.get(comparison_index.checked_sub(1)?)?;
    let definition = literal.result?;
    let LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(value)) = literal.kind
    else {
        return None;
    };
    let value = u64::try_from(value).ok()?;
    if !matches!(
        definition.scalar_type,
        ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64
    ) || !immediates.contains(&value)
    {
        return None;
    }
    let LegalizedScalarInstructionKind::Compare {
        predicate: LegalizedScalarComparison::Equal,
        left,
        right,
        ..
    } = comparison.kind
    else {
        return None;
    };
    if (left == definition.value) == (right == definition.value) {
        return None;
    }
    for source_block in &function.blocks {
        if source_block.terminator.references_value(definition.value) {
            return None;
        }
        for instruction in &source_block.instructions {
            if instruction.operation == comparison.operation {
                continue;
            }
            if instruction.references_value(definition.value) {
                return None;
            }
        }
    }
    Some(literal)
}

pub(super) fn folded_zero<'a>(
    function: &LegalizedScalarFunction,
    block: &'a LegalizedScalarBlock,
    comparison_index: usize,
) -> Option<&'a LegalizedScalarInstruction> {
    folded_literal(function, block, comparison_index, 0..=0)
}

pub(super) fn folded_immediate<'a>(
    function: &LegalizedScalarFunction,
    block: &'a LegalizedScalarBlock,
    comparison_index: usize,
) -> Option<&'a LegalizedScalarInstruction> {
    folded_literal(function, block, comparison_index, 1..=4095)
}
