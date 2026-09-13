use super::*;

fn array(bytes: &[u8]) -> Value {
    Value::Array(
        bytes
            .iter()
            .map(|byte| Value::Int(i64::from(*byte)).cell())
            .collect(),
    )
}

fn assert_equal(evaluator: &Evaluator<'_>, left: &Value, right: &Value, expected: bool) {
    assert!(matches!(evaluator.values_equal(left, right), Ok(actual) if actual == expected));
    assert!(matches!(evaluator.values_equal(right, left), Ok(actual) if actual == expected));
}

#[test]
fn byte_equality_observes_exact_live_lengths_and_raw_contents() {
    let program = TypedTrees::default();
    let evaluator = Evaluator::new(&program, &[]);
    for (left, right, expected) in [
        (b"".as_slice(), b"".as_slice(), true),
        (b"", b"x", false),
        (b"look", b"look", true),
        (b"look", b"quit", false),
        (b"look\r", b"look", false),
        (b"look\0", b"look", false),
        (b"look\0", b"look\0", true),
        (b"look\r", b"look\r", true),
        (b"\xff\0\rX", b"\xff\0\rY", false),
    ] {
        assert_equal(&evaluator, &array(left), &Value::bytes(right), expected);
        assert_equal(&evaluator, &array(left), &array(right), expected);
        assert_equal(
            &evaluator,
            &Value::bytes(left),
            &Value::bytes(right),
            expected,
        );
    }
    let all_bytes = (u8::MIN..=u8::MAX).collect::<Vec<_>>();
    assert_equal(
        &evaluator,
        &array(&all_bytes),
        &Value::bytes(all_bytes.clone()),
        true,
    );
    assert_equal(&evaluator, &array(&all_bytes), &array(&all_bytes), true);
}

#[test]
fn byte_equality_ignores_backing_capacity_but_observes_live_cell_changes() {
    let program = TypedTrees::default();
    let evaluator = Evaluator::new(&program, &[]);
    let text = Value::bytes(b"!look\0unused");
    let Value::Str(backing) = text else {
        panic!("packed text")
    };
    let packed_view = Value::Str(backing.subslice(1, 5).unwrap());
    let Value::Array(elements) = array(b"!look\rignored") else {
        panic!("cell array")
    };
    let view = Value::Array(elements[1..5].to_vec());
    assert_equal(&evaluator, &view, &packed_view, true);
    *elements[4].borrow_mut() = Value::Int(i64::from(b'p'));
    assert_equal(&evaluator, &view, &packed_view, false);
}

#[test]
fn scalar_equality_preserves_numeric_meaning() {
    let program = TypedTrees::default();
    let evaluator = Evaluator::new(&program, &[]);
    assert_equal(
        &evaluator,
        &Value::Float(f64::NAN),
        &Value::Float(f64::NAN),
        false,
    );
    assert_equal(&evaluator, &Value::Float(-0.0), &Value::Float(0.0), true);
    assert_equal(&evaluator, &Value::Int(7), &Value::Int(8), false);
}

#[test]
fn unsupported_equality_never_succeeds_from_two_missing_integer_values() {
    let program = TypedTrees::default();
    let evaluator = Evaluator::new(&program, &[]);
    for element in [
        Value::Unit,
        Value::Bool(false),
        Value::Int(-1),
        Value::Int(256),
        array(b"x"),
        Value::Enum {
            type_symbol: SymbolHandle::invalid(),
            variant_name: "Payload".to_owned(),
            payload: vec![("value".to_owned(), Value::Int(1).cell())],
        },
    ] {
        let nonbytes = Value::Array(vec![element.cell()]);
        let text = Value::bytes(b"x");
        assert!(matches!(
            evaluator.values_equal(&nonbytes, &text),
            Err(Halt::Unsupported(_))
        ));
        assert!(matches!(
            evaluator.values_equal(&text, &nonbytes),
            Err(Halt::Unsupported(_))
        ));
        assert!(matches!(
            evaluator.values_equal(&nonbytes, &nonbytes),
            Err(Halt::Unsupported(_))
        ));
        assert!(matches!(
            evaluator.values_equal(&array(b"x"), &nonbytes),
            Err(Halt::Unsupported(_))
        ));
        assert!(matches!(
            evaluator.values_equal(&nonbytes, &array(b"x")),
            Err(Halt::Unsupported(_))
        ));
    }
    for (left, right) in [
        (Value::Unit, Value::Unit),
        (array(b"x"), Value::Int(1)),
        (Value::Float(1.0), Value::Unit),
        (
            Value::Array(vec![Value::Unit.cell()]),
            Value::Array(vec![Value::Unit.cell()]),
        ),
    ] {
        assert!(matches!(
            evaluator.values_equal(&left, &right),
            Err(Halt::Unsupported(_))
        ));
    }
}

#[test]
fn byte_inequality_is_the_complement_of_equality() {
    let program = TypedTrees::default();
    let evaluator = Evaluator::new(&program, &[]);
    for (operator, expected) in [
        (BinaryOperator::Equal, false),
        (BinaryOperator::NotEqual, true),
    ] {
        assert!(matches!(
            evaluator.eval_binary(operator, array(b"look\r"), Value::bytes(b"look"), false, None, None),
            Ok(Value::Bool(actual)) if actual == expected
        ));
    }
}
