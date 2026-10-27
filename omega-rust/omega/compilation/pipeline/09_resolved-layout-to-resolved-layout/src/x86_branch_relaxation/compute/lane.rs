//! The one predicate lane a resolved conditional branch reads.
//!
//! A resolved conditional branch names one predicate, and the predicate names
//! exactly one x86-64 family realized at exactly two widths: the two-byte
//! `Jcc rel8` short form and the six-byte `Jcc rel32` near form. The lane is
//! that mapping read once - the width a byte length names, the alternative the
//! family selects, and the family's encoder and independent validator - so the
//! sweep, the inspection, the reflow and their fixture ask the lane instead of
//! re-listing each predicate, its family and the entrypoint that carries it.
//! The predicate's own `rank` names the lane row; the width names the column.

use isa_x86_64::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};

use machine_code::ResolvedConditionalBranchPredicate;

use super::super::error::OptimizedX86BranchRelaxationError;

/// The two widths every selected conditional branch realizes.
pub(super) const SHORT_WIDTH: usize = 2;
pub(super) const NEAR_WIDTH: usize = 6;

type Encoding = Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError>;
type Encode = fn(&ValidatedPhysicalRegisterModel, MachineAlternativeKey, i64) -> Encoding;
type Validate = fn(&ValidatedPhysicalRegisterModel, MachineAlternativeKey, i64, &[u8]) -> Encoding;

/// The encoders and independent validators in predicate-rank order, short form
/// before near form. The nonzero family owns two entrypoints; the less-than
/// families carry the width on the alternative's variant and name one.
const ENCODE: [[Encode; 2]; 3] = [
    [
        encode_x86_64_selected_short_nonzero_branch_form,
        encode_x86_64_selected_nonzero_branch_form,
    ],
    [encode_x86_64_selected_u64_less_than_branch_form; 2],
    [encode_x86_64_selected_i64_less_than_branch_form; 2],
];
const VALIDATE: [[Validate; 2]; 3] = [
    [
        validate_x86_64_selected_short_nonzero_branch_form,
        validate_x86_64_selected_nonzero_branch_form,
    ],
    [validate_x86_64_selected_u64_less_than_branch_form; 2],
    [validate_x86_64_selected_i64_less_than_branch_form; 2],
];

/// Whether a row's byte length names the near form. `None` is neither width:
/// the row is not a selected conditional branch form.
pub(super) fn near_width(bytes: usize) -> Option<bool> {
    match bytes {
        SHORT_WIDTH => Some(false),
        NEAR_WIDTH => Some(true),
        _ => None,
    }
}

/// The alternative a predicate's family selects: the nonzero family keeps the
/// caller's alternative, and the less-than families carry the width on the
/// variant, so the near form is variant zero and the short form variant one.
pub(super) fn selected_alternative(
    predicate: ResolvedConditionalBranchPredicate,
    alternative: MachineAlternativeKey,
    near: bool,
) -> MachineAlternativeKey {
    let variant = if near { 0 } else { 1 };
    match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => alternative,
        ResolvedConditionalBranchPredicate::U64LessThanV1 => key(
            MachineAlternativeFamily::ConditionalBranchU64LessThan,
            variant,
        ),
        ResolvedConditionalBranchPredicate::I64LessThanV1 => key(
            MachineAlternativeFamily::ConditionalBranchI64LessThan,
            variant,
        ),
    }
}

const fn key(family: MachineAlternativeFamily, variant: u32) -> MachineAlternativeKey {
    MachineAlternativeKey { family, variant }
}

/// Encode the canonical form of `predicate` at the named width.
pub(super) fn encode(
    predicate: ResolvedConditionalBranchPredicate,
    near: bool,
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, OptimizedX86BranchRelaxationError> {
    (ENCODE[usize::from(predicate.rank())][usize::from(near)])(
        physical,
        alternative,
        byte_displacement,
    )
    .map_err(OptimizedX86BranchRelaxationError::X86_64)
}

/// Independently decode a row's recorded bytes as the form `predicate` and the
/// width name; another form's byte pattern is refused.
pub(super) fn validate(
    predicate: ResolvedConditionalBranchPredicate,
    near: bool,
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, OptimizedX86BranchRelaxationError> {
    (VALIDATE[usize::from(predicate.rank())][usize::from(near)])(
        physical,
        alternative,
        byte_displacement,
        bytes,
    )
    .map_err(OptimizedX86BranchRelaxationError::X86_64)
}
