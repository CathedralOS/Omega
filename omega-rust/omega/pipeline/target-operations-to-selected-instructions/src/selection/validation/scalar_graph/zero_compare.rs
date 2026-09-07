//! Local zero-equality selection; executable source rows and fuel remain accounted for.
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

pub(super) fn folded_zero<'a>(
    function: &LegalizedScalarFunction,
    block: &'a LegalizedScalarBlock,
    comparison_index: usize,
) -> Option<&'a LegalizedScalarInstruction> {
    let comparison = block.instructions.get(comparison_index)?;
    let zero = block.instructions.get(comparison_index.checked_sub(1)?)?;
    if !matches!(
        zero.kind,
        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(0))
    ) || !matches!(zero.scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
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
    if (left == zero.result) == (right == zero.result) {
        return None;
    }
    for source_block in &function.blocks {
        if source_block.terminator.references_value(zero.result) {
            return None;
        }
        for instruction in &source_block.instructions {
            if instruction.operation == comparison.operation {
                continue;
            }
            let uses = match &instruction.kind {
                LegalizedScalarInstructionKind::Constant(_) => false,
                LegalizedScalarInstructionKind::Call(call) => call
                    .arguments
                    .iter()
                    .any(|argument| argument.source == zero.result),
                LegalizedScalarInstructionKind::ExactBinary { left, right, .. }
                | LegalizedScalarInstructionKind::Compare { left, right, .. } => {
                    *left == zero.result || *right == zero.result
                }
            };
            if uses {
                return None;
            }
        }
    }
    Some(zero)
}
