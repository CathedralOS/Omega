//! Align a call's authored operands with the formals those operands answer.
//!
//! A selector call's first operand is the selector. Dispatch consumes it, and
//! the requirement's formals — the only names a published crash route or
//! contract predicate may use — begin at the next operand. A list that already
//! has one operand per formal is left unchanged, including a slot roster whose
//! own parameter 0 is the selector.

use symbols::SymbolHandle;

pub(crate) fn arguments_aligned_to_formals<'a, T>(
    program: &dyn ::validation::CheckingInput,
    call_target: SymbolHandle,
    formal_count: usize,
    arguments: &'a [T],
) -> &'a [T] {
    let leading = usize::from(program.dynamic_slot_by_symbol(call_target).is_some());
    arguments
        .get(leading..)
        .filter(|aligned| aligned.len() == formal_count)
        .unwrap_or(arguments)
}
