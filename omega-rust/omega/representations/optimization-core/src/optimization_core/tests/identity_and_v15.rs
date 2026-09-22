use super::super::{Optimization, OptimizationSelections};

#[test]
fn identity_is_domain_stable_and_selection_sensitive() {
    let empty = OptimizationSelections::default().identity();
    let selected = OptimizationSelections::new([Optimization::ControlFlowCleanup])
        .expect("unique selection")
        .identity();
    assert_ne!(empty, selected);
    assert_eq!(empty, OptimizationSelections::default().identity());
}

#[test]
fn v15_selection_appends_the_exact_compare_right_operand_tag() {
    let selections = OptimizationSelections::new([
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareI64RightOperandV1,
    ])
    .unwrap();
    let encoded = selections.encode();
    assert_eq!(u32::from_le_bytes(encoded[8..12].try_into().unwrap()), 15);
    assert_eq!(&encoded[12..16], &1_u32.to_le_bytes());
    assert_eq!(encoded[16], 20);
    assert_eq!(OptimizationSelections::decode(&encoded), Ok(selections));
}
