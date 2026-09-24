use super::check_program;

/// A single erased lifetime argument on borrow-carrying data has the same
/// linking role as a direct reference lifetime. The aggregate result borrows
/// only `first`, so mutating `second` while it remains live is sound.
#[test]
fn accepts_aggregate_return_disambiguated_by_explicit_lifetime_argument() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine select<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) -> View<'left> {
            let selected: View<'left> = View { body: first };
            transition {
                _ -> selected
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) {
            let selected: View<'left> = select(first, second);
            write(second);
            write(selected.body);
        }
    "#;

    check_program(source)
        .expect("the aggregate lifetime argument should retain only its named source");
}

/// The same aggregate result keeps its named source loan active at the call
/// site; mutating that source before the result's last use must reject.
#[test]
fn rejects_linked_source_mutation_for_explicit_aggregate_lifetime_argument() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine select<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) -> View<'left> {
            let selected: View<'left> = View { body: first };
            transition {
                _ -> selected
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) {
            let selected: View<'left> = select(first, second);
            write(first);
            write(selected.body);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the aggregate result must retain its named source");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `first` while local borrow `selected` is still active"),
        "expected the aggregate result's linked-source conflict, got:\n{combined}"
    );
}

/// Moving a borrow-carrying aggregate through another local transfers its
/// source loan rather than laundering it through ordinary data assignment.
#[test]
fn rejects_source_mutation_after_borrow_carrying_local_transfer() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(source: &'source mut i32) {
            let first: View<'source> = View { body: source };
            let second: View<'source> = first;
            write(source);
            write(second.body);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("transferring the aggregate must transfer its loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `source` while local borrow `second` is still active"),
        "expected the transferred aggregate's source conflict, got:\n{combined}"
    );
}

/// Selecting a borrow-carrying field from a larger aggregate strips the
/// selected owner-path prefix while retaining that field's source.
#[test]
fn rejects_source_mutation_after_borrow_carrying_field_transfer() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        data Pair<'left, 'right> {
            left: View<'left>;
            right: View<'right>;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {
            let pair: Pair<'left, 'right> = Pair {
                left: View { body: left },
                right: View { body: right },
            };
            let selected: View<'right> = pair.right;
            write(right);
            write(selected.body);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("projecting the aggregate field must retain its loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `right` while local borrow `selected` is still active"),
        "expected the projected aggregate's source conflict, got:\n{combined}"
    );
}

/// An explicitly multi-lifetime result derives one source mapping per field;
/// using `right` does not keep the unrelated `left` loan active.
#[test]
fn accepts_field_specific_sources_for_multi_lifetime_result() {
    let source = r#"
        data Pair<'left, 'right> {
            left: &'left mut i32;
            right: &'right mut i32;
        }

        machine pair<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) -> Pair<'left, 'right> {
            let result: Pair<'left, 'right> = Pair {
                left: left,
                right: right,
            };
            transition {
                _ -> result
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {
            let result: Pair<'left, 'right> = pair(left, right);
            write(left);
            write(result.right);
        }
    "#;

    check_program(source)
        .expect("a field-specific use should retain only that field's named source");
}

/// The field-specific mapping still rejects mutation of the source retained by
/// the field used later.
#[test]
fn rejects_linked_field_source_for_multi_lifetime_result() {
    let source = r#"
        data Pair<'left, 'right> {
            left: &'left mut i32;
            right: &'right mut i32;
        }

        machine pair<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) -> Pair<'left, 'right> {
            let result: Pair<'left, 'right> = Pair {
                left: left,
                right: right,
            };
            transition {
                _ -> result
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {
            let result: Pair<'left, 'right> = pair(left, right);
            write(right);
            write(result.right);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the right result field must retain the right input");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `right` while local borrow `result` is still active"),
        "expected the multi-lifetime field conflict, got:\n{combined}"
    );
}

/// Replacing a borrow-carrying field releases the source carried by the old
/// field and makes the replacement source the field's active loan.
#[test]
fn accepts_precise_borrow_carrying_field_reassignment() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(
            first: &'source mut i32,
            second: &'source mut i32
        ) {
            let mut selected: View<'source> = View { body: first };
            selected.body = second;
            write(first);
            selected.body = 2;
        }
    "#;

    check_program(source)
        .expect("field replacement should release the old source and retain the new source");
}

/// The replacement source cannot be mutated while the reassigned aggregate
/// field remains live.
#[test]
fn rejects_replacement_source_of_borrow_carrying_field_reassignment() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(
            first: &'source mut i32,
            second: &'source mut i32
        ) {
            let mut selected: View<'source> = View { body: first };
            selected.body = second;
            write(second);
            selected.body = 2;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the reassigned field must retain its replacement source");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `second` while local borrow `selected` is still active"),
        "expected the reassigned field's source conflict, got:\n{combined}"
    );
}
