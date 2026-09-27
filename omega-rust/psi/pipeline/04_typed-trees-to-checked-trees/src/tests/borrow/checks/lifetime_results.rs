use super::check_program;

#[test]
fn copied_shared_record_retains_its_local_backing_lifetime() {
    let source = r#"
        data View [copy] { value: &i32; }

        machine escape(seed: &i32) -> View {
            let local: i32 = 9;
            let original: View = View { value: &local };
            let copied: View = original;
            copied
        }
    "#;
    let typed = crate::tests::front_end::typed_program(source);
    validation::validate_program(&typed).expect("shared record copy declaration is valid");
    check_program(&source.replace("value: &local", "value: seed"))
        .expect("a copied record can retain its input-backed loan");
    let diagnostics = check_program(source).expect_err("copying cannot extend a local loan");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("returns a view borrowing the local")),
        "expected the local backing lifetime fence, got: {diagnostics:?}"
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
