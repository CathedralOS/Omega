//! `X86SelectXorZeroI64MaterializationV1`: realize an exact i64 zero with
//! the three-byte dependency-breaking `xor r64, r64` idiom instead of the
//! ten-byte `mov r64, imm64`.
//!
//! The rule's own guard is the value: only an exact zero has this form. The
//! idiom writes RFLAGS where the baseline move does not; the shared
//! declared-form rewrite proves those units dead before committing.

use optimization_core::Optimization;
use selected_instructions::SelectedInstructionKind;
use semantic_vocabulary::IntegerValue;

use super::{RuleContext, select_declared_form};
use crate::optimization::{
    PostAllocationMachineOptimizationError, PostAllocationMachineRuleExecution,
};

pub(super) fn run(
    optimization: Optimization,
    context: &mut RuleContext<'_>,
) -> Result<PostAllocationMachineRuleExecution, PostAllocationMachineOptimizationError> {
    select_declared_form(optimization, context, materializes_exact_zero)
}

fn materializes_exact_zero(kind: SelectedInstructionKind) -> bool {
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => match value {
            IntegerValue::Signed(value) => value == 0,
            IntegerValue::Unsigned(value) => value == 0,
        },
        _ => false,
    }
}
