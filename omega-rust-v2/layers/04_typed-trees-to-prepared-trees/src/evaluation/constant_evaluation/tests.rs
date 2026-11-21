use super::*;

use crate::test_source_package::source_package;

#[test]
fn internal_publication_guard_refuses_256_and_publishes_255_for_u8() {
    let source = "const BYTE:u8=255u8;";
    let package = source_package([("main.omg", source)]);
    let trees = omega_typer::type_source_package(&package).unwrap();
    let [byte] = trees.constants.as_slice() else { panic!("expected one typed constant") };
    let mut evaluator = ConstantEvaluation::new(&trees, &[]);

    // Vary the candidate at the private cache write, not the source or typed product.
    let error = evaluator.publish_constant(byte, PreparedValue::Integer(256u16.into())).unwrap_err();
    assert_eq!(evaluator.frozen_value(&byte.declaration), None);
    let limit = PreparedValue::Integer(255u8.into());
    evaluator.publish_constant(byte, limit.clone()).unwrap();
    assert_eq!(evaluator.frozen_value(&byte.declaration), Some(&limit));

    let start = source.find("255u8").unwrap();
    spec_corpus::assert_preparation_cause!(
        error,
        crate::RefusalCause::Preparation(PreparationReason::FrozenConstantOutsideCarrier),
        "app",
        "main.omg",
        (start, start + "255u8".len()),
        "static evaluation cannot admit this invocation: the frozen constant exceeds its carrier",
    );
}
