//! A byte carrier compared with a byte-sequence literal is content equality:
//! the carrier's live length against the literal's length, then each literal
//! byte against the carrier's byte at that index, folded leftmost-first so
//! the length comparison dominates every indexed read. Borrowed byte views
//! take the same decomposition as bounded owned byte fields, wherever the
//! view's storage is rooted.
use super::checked;
use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStorageRoot, CheckedStructuralPredicatePathSegment,
};

/// The observations one decomposed literal comparison makes, in evaluation
/// order: the measured length, then every `(index, byte)` read.
#[derive(Debug, PartialEq, Eq)]
struct LiteralComparison {
    root: CheckedStorageRoot,
    path: Vec<CheckedStructuralPredicatePathSegment>,
    length: u64,
    bytes: Vec<(u64, u64)>,
}

fn literal_value(expression: &CheckedScalarExpression) -> u64 {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        panic!("a literal operand: {expression:?}");
    };
    literal.value_u64().expect("a non-negative literal")
}

fn literal_comparison(guard: &CheckedBooleanExpression) -> LiteralComparison {
    match guard {
        CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left,
            right,
        } => {
            let CheckedScalarExpression::StructuralParameterByteLength { root, path } =
                left.as_ref()
            else {
                panic!("the leftmost conjunct measures the carrier: {left:?}");
            };
            LiteralComparison {
                root: *root,
                path: path.clone(),
                length: literal_value(right),
                bytes: Vec::new(),
            }
        }
        CheckedBooleanExpression::And { left, right } => {
            let mut comparison = literal_comparison(left);
            let CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: read,
                right: byte,
            } = right.as_ref()
            else {
                panic!("each later conjunct compares one byte: {right:?}");
            };
            let CheckedScalarExpression::StructuralParameterIndexedRead {
                root,
                path,
                index,
                inner_indexes,
                element_path,
                primitive_type,
            } = read.as_ref()
            else {
                panic!("a byte conjunct reads the carrier: {read:?}");
            };
            assert_eq!(
                (*root, path, *primitive_type),
                (
                    comparison.root,
                    &comparison.path,
                    typed_trees::types::PrimitiveType::U8
                ),
                "every read observes the measured carrier",
            );
            assert!(inner_indexes.is_empty() && element_path.is_empty());
            comparison
                .bytes
                .push((literal_value(index), literal_value(byte)));
            comparison
        }
        other => panic!("a decomposed literal comparison: {other:?}"),
    }
}

/// The checked guard at `statement_ordinal` of `machine`'s first state.
fn guard(
    checked: &checked_trees::CheckedTrees,
    machine: &str,
    statement_ordinal: u32,
) -> Option<CheckedBooleanExpression> {
    let machine = checked
        .machines()
        .iter()
        .find(|candidate| candidate.name.as_str().ends_with(machine))
        .expect("the authored machine");
    let state = &checked.machine_states(machine)[0];
    match checked.facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_ordinal,
        CheckedScalarExpressionRole::Guard,
    )? {
        CheckedScalarExpression::Boolean(guard) => Some(guard.as_ref().clone()),
        other => panic!("a Boolean guard: {other:?}"),
    }
}

fn quit_bytes() -> Vec<(u64, u64)> {
    b"quit"
        .iter()
        .enumerate()
        .map(|(index, byte)| (index as u64, u64::from(*byte)))
        .collect()
}

#[test]
fn a_borrowed_byte_view_parameter_compares_its_length_then_each_literal_byte() {
    let checked = checked(
        r#"
        data Main { }
        machine Main::classify(&self, text: &[u8]) {
            transition text == "quit" {
                true -> finished()
                _ -> other()
            }
            state finished(&self) {}
            state other(&self) {}
        }
        machine Main::reversed(&self, text: &[u8]) {
            transition "quit" != text {
                true -> finished()
                _ -> other()
            }
            state finished(&self) {}
            state other(&self) {}
        }
        "#,
    );
    // The view is the second authored formal after the receiver; the whole
    // view is observed, so the path is empty.
    let expected = LiteralComparison {
        root: CheckedStorageRoot::Parameter { index: 1 },
        path: Vec::new(),
        length: 4,
        bytes: quit_bytes(),
    };
    let classify = guard(&checked, "::classify", 0).expect("the view guard is retained");
    assert_eq!(literal_comparison(&classify), expected);
    // Either operand order spells the same comparison; `!=` negates it.
    let reversed = guard(&checked, "::reversed", 0).expect("the reversed guard is retained");
    let CheckedBooleanExpression::Not(reversed) = reversed else {
        panic!("`!=` negates the content comparison: {reversed:?}");
    };
    assert_eq!(literal_comparison(&reversed), expected);
}

#[test]
fn a_borrowed_byte_view_field_compares_through_its_field_path() {
    let checked = checked(
        r#"
        data Main { name: &[u8]; }
        machine Main::classify(&self) {
            transition self.name == "quit" {
                true -> finished()
                _ -> other()
            }
            state finished(&self) {}
            state other(&self) {}
        }
        "#,
    );
    let classify = guard(&checked, "::classify", 0).expect("the field guard is retained");
    let comparison = literal_comparison(&classify);
    assert_eq!(comparison.root, CheckedStorageRoot::Parameter { index: 0 });
    assert!(
        matches!(
            comparison.path.as_slice(),
            [CheckedStructuralPredicatePathSegment::Field(_)]
        ),
        "the view field is one field below the receiver: {:?}",
        comparison.path,
    );
    assert_eq!((comparison.length, comparison.bytes), (4, quit_bytes()));
}

#[test]
fn a_byte_view_local_compares_as_its_whole_established_view() {
    let checked = checked(
        r#"
        data Main { }
        machine Main::classify(&self, text: &[u8]) requires 2 <= text.len {
            let prefix: &[u8] = text[0..2];
            transition prefix == "qu" {
                true -> finished()
                _ -> other()
            }
            state finished(&self) {}
            state other(&self) {}
        }
        "#,
    );
    let classify = guard(&checked, "::classify", 1).expect("the view-local guard is retained");
    let comparison = literal_comparison(&classify);
    assert!(
        matches!(comparison.root, CheckedStorageRoot::ViewLocal { .. }),
        "the local's established view is the root: {:?}",
        comparison.root,
    );
    assert!(comparison.path.is_empty(), "a view local is observed whole");
    assert_eq!(
        (comparison.length, comparison.bytes),
        (2, vec![(0, u64::from(b'q')), (1, u64::from(b'u'))])
    );
}

/// A borrowed fixed byte array declares its extent in its type and keeps no
/// live length, so a literal comparison has no length to measure: it keeps no
/// content form, while the same machine over a borrowed view does.
#[test]
fn a_borrowed_fixed_byte_array_keeps_no_content_comparison() {
    let checked = checked(
        r#"
        data Main { }
        machine Main::fixed(&self, text: &[u8; 4]) {
            transition text == "quit" {
                true -> finished()
                _ -> other()
            }
            state finished(&self) {}
            state other(&self) {}
        }
        machine Main::viewed(&mut self, text: &mut [u8]) {
            transition text == "quit" {
                true -> finished()
                _ -> other()
            }
            state finished(&mut self) {}
            state other(&mut self) {}
        }
        "#,
    );
    assert_eq!(guard(&checked, "::fixed", 0), None);
    // A mutable view is readable: it measures and reads like a shared one.
    let viewed = guard(&checked, "::viewed", 0).expect("the mutable view guard is retained");
    assert_eq!(
        literal_comparison(&viewed),
        LiteralComparison {
            root: CheckedStorageRoot::Parameter { index: 1 },
            path: Vec::new(),
            length: 4,
            bytes: quit_bytes(),
        }
    );
}
