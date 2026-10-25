//! The ISA data branch widening reads.
//!
//! A conditional branch row holds either its short form, whose displacement
//! field reaches a bounded range, or its long form. Every long form is its
//! short form plus one four-byte field: x86 `Jcc rel8` (2 bytes) becomes
//! `Jcc rel32` (6), and AArch64 `B.cond imm19` (4) becomes
//! `B.<inverse> +8; B imm26` (8). One widening operation
//! (`reflow::widening`) and one closure check (`validation::widths`) therefore
//! serve both ISAs, and re-encoding picks a row's encoder by its width. This
//! table is the only place either reads which ISA it is serving.

use std::ops::Range;

use machine_code::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentConditionalBranchPredicate,
};
use selected_instructions::MachineAlternativeFamily;
use target::Architecture;

/// The machine alternative family one conditional-branch predicate names: the
/// family the target encoders and decoders are asked for and assert against
/// the recorded alternative.
pub(super) const fn conditional_branch_family(
    predicate: FunctionFragmentConditionalBranchPredicate,
) -> MachineAlternativeFamily {
    match predicate {
        FunctionFragmentConditionalBranchPredicate::NonZeroV1 => {
            MachineAlternativeFamily::ConditionalBranchNonZero
        }
        FunctionFragmentConditionalBranchPredicate::U64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchU64LessThan
        }
        FunctionFragmentConditionalBranchPredicate::I64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchI64LessThan
        }
    }
}

/// Bytes a widening adds to its row.
pub(super) const WIDENING_GROWTH: u64 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ShortConditionalForm {
    /// Row width of the short form.
    pub(super) width: u64,
    /// Displacements the short form encodes.
    pub(super) reach: Range<i64>,
    /// x86 measures a displacement from the row's end, AArch64 from its start.
    pub(super) measured_from_end: bool,
}

impl ShortConditionalForm {
    /// Whether this form, placed at `row_offset`, reaches `target`.
    pub(super) fn reaches(&self, row_offset: u64, target: u64) -> bool {
        let reference = if self.measured_from_end {
            i128::from(row_offset) + i128::from(self.width)
        } else {
            i128::from(row_offset)
        };
        i64::try_from(i128::from(target) - reference)
            .is_ok_and(|displacement| self.reach.contains(&displacement))
    }
}

/// The short form of one conditional branch, or `None` when frame
/// application has no in-place widening for it.
pub(super) fn short_conditional_form(
    architecture: Architecture,
    branch: &FunctionFragmentConditionalBranchEvidence,
) -> Option<ShortConditionalForm> {
    match architecture {
        // The nonzero `Jcc rel8` keeps its alternative when widened. The
        // less-than short forms are a separate alternative (variant 1), and
        // frame application preserves every row's alternative.
        Architecture::X86_64 => (branch.predicate
            == FunctionFragmentConditionalBranchPredicate::NonZeroV1)
            .then(|| ShortConditionalForm {
                width: 2,
                reach: i64::from(i8::MIN)..i64::from(i8::MAX) + 1,
                measured_from_end: true,
            }),
        // Every `B.cond` widens. The fused `CBNZ` reads its register operand
        // and has no long form.
        Architecture::Aarch64 => {
            branch
                .decoded_register_reads
                .is_empty()
                .then_some(ShortConditionalForm {
                    width: 4,
                    reach: isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH,
                    measured_from_end: false,
                })
        }
    }
}

/// Whether a row of `row_width` bytes holds the branch's long form.
pub(super) fn holds_long_form(
    architecture: Architecture,
    branch: &FunctionFragmentConditionalBranchEvidence,
    row_width: usize,
) -> bool {
    short_conditional_form(architecture, branch)
        .is_some_and(|form| u64::try_from(row_width).ok() != Some(form.width))
}
