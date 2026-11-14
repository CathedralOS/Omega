// Unit test verifying the v16 wire header and tag `20` for
// `Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1`.
// Version 15 records, and tag 22 inside a version 16 payload, do not decode.

use super::super::{Optimization, OptimizationSelections, SelectionDecodeError};

#[test]
fn v16_selection_appends_the_exact_compare_right_operand_tag() {
    let selections = OptimizationSelections::new([
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
    ])
    .unwrap();
    let encoded = selections.encode();
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 16);
    assert_eq!(&encoded[12..16], &1_u32.to_le_bytes());
    assert_eq!(encoded[16], 20);
    assert_eq!(OptimizationSelections::decode(&encoded), Ok(selections));
}

#[test]
fn retired_selection_version_and_product_pruning_tag_do_not_decode() {
    let selections = OptimizationSelections::new([
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
    ])
    .unwrap();
    let mut prior = selections.encode();
    prior[8..12].copy_from_slice(&15_u32.to_le_bytes());
    assert_eq!(
        OptimizationSelections::decode(&prior),
        Err(SelectionDecodeError::UnsupportedVersion(15))
    );
    let mut retired_tag = selections.encode();
    retired_tag[16] = 22;
    assert_eq!(
        OptimizationSelections::decode(&retired_tag),
        Err(SelectionDecodeError::UnknownOptimizationTag(22))
    );
}
