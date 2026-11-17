// Unit test verifying the v16 wire header and the dense tags of the
// vocabulary. Version 15 records, and a tag past the 54 selections, do not
// decode.

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
fn dense_vocabulary_ends_at_tag_54_and_rejects_the_previous_version() {
    let last = OptimizationSelections::new([Optimization::SelectedStoredLoadForwardingV1]).unwrap();
    let encoded = last.encode();
    assert_eq!(encoded[16], 54);
    let filled =
        OptimizationSelections::new([Optimization::SelectedIncomingLiteralExtensionElimination])
            .unwrap();
    assert_eq!(filled.encode()[16], 22);
    let mut prior = encoded.clone();
    prior[8..12].copy_from_slice(&15_u32.to_le_bytes());
    assert_eq!(
        OptimizationSelections::decode(&prior),
        Err(SelectionDecodeError::UnsupportedVersion(15))
    );
    let mut past_vocabulary = encoded;
    past_vocabulary[16] = 55;
    assert_eq!(
        OptimizationSelections::decode(&past_vocabulary),
        Err(SelectionDecodeError::UnknownOptimizationTag(55))
    );
}
