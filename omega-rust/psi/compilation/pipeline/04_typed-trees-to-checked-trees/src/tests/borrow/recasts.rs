use super::checks::check_program;
use crate::borrow::build_borrow_facts;
use crate::tests::front_end::typed_program;

#[test]
fn literal_indexed_recast_keeps_immediate_sibling_bytes_writable() {
    check_program(&indexed_mutable_recast_source(
        "self.bytes[1] = 1; self.bytes[6] = 1;",
        "view = 2;",
    ))
    .expect("the bytes immediately before and after [2, 6) are disjoint");
}

#[test]
fn literal_indexed_nested_fixed_array_recast_rejects_complete_footprint_mutations() {
    for mutation in [
        "self.bytes[3] = 1;",
        "self.bytes[6] = 1;",
        "self.bytes[10] = 1;",
    ] {
        let diagnostics = check_program(&indexed_mutable_nested_array_recast_source(
            mutation,
            "view[1][1] = 2;",
        ))
        .expect_err("every byte across the nested-array footprint must remain borrowed");

        assert_conflict(&diagnostics, mutation.trim_end_matches(" = 1;"), "view");
    }
}

#[test]
fn literal_indexed_nested_record_array_recast_rejects_padding_and_element_boundaries() {
    for mutation in [
        "self.bytes[3] = 1;",
        "self.bytes[4] = 1;",
        "self.bytes[10] = 1;",
        "self.bytes[11] = 1;",
        "self.bytes[18] = 1;",
        "self.bytes[19] = 1;",
        "self.bytes[34] = 1;",
    ] {
        let diagnostics = check_program(&indexed_mutable_nested_record_array_recast_source(
            mutation,
            "view[1][1].tail = 2;",
        ))
        .expect_err("padding and both sides of each element boundary remain borrowed");

        assert_conflict(&diagnostics, mutation.trim_end_matches(" = 1;"), "view");
    }
}

#[test]
fn unsupported_literal_indexed_aggregate_targets_publish_no_precise_loan() {
    let source = r#"
        data Cell {
            bytes: [u8; 16];
        }

        machine Cell::exercise(&mut self) {
            let empty: &[u16; 0] = &self.bytes[0] as &[u16; 0];
            let slice: &[u16] = &self.bytes[0] as &[u16];
        }
    "#;

    assert_valid_recast_has_no_loan(source, "zero-length and slice targets stay fenced");
}

#[test]
fn erased_and_atomic_field_records_publish_no_precise_loan() {
    let source = r#"
        data ErasedRecord {
            value: u16;
            proof [erased]: u16;
        }

        data AtomicRecord {
            counter: AtomicU32;
        }

        data Cell {
            bytes: [u8; 32];
        }

        machine Cell::exercise(&mut self) {
            let erased_field: &[ErasedRecord; 2] =
                &self.bytes[0] as &[ErasedRecord; 2];
            let atomic_direct: &AtomicRecord = &self.bytes[12] as &AtomicRecord;
            let atomic_array: &[AtomicRecord; 2] =
                &self.bytes[16] as &[AtomicRecord; 2];
        }
    "#;

    assert_valid_recast_has_no_loan(
        source,
        "erased-field and atomic-field records stay outside exact custody",
    );
}

#[test]
fn zero_length_unsupported_terminals_do_not_skip_recursive_validation() {
    let bool_source = r#"
        data Holder { marker: u8; absent: [bool; 0]; }
        data Cell { bytes: [u8; 4]; }
        machine Cell::exercise(&mut self) {
            let view: &Holder = &self.bytes[0] as &Holder;
        }
    "#;
    assert_invalid_recast_has_no_loan(bool_source, "must be recursively fact-free");

    let constrained_source = r#"
        domain u16::Small
        requires
            self <= 10;
        data Holder { marker: u8; absent: [u16 in Small; 0]; }
        data Cell { bytes: [u8; 4]; }
        machine Cell::exercise(&mut self) {
            let view: &Holder = &self.bytes[0] as &Holder;
        }
    "#;
    assert_invalid_recast_has_no_loan(constrained_source, "must be recursively fact-free");

    let atomic_source = r#"
        data Holder { marker: u8; absent: [AtomicU32; 0]; }
        data Cell { bytes: [u8; 4]; }
        machine Cell::exercise(&mut self) {
            let view: &Holder = &self.bytes[0] as &Holder;
        }
    "#;
    assert_valid_recast_has_no_loan(
        atomic_source,
        "a zero count cannot erase an atomic terminal's authority fence",
    );

    for (context, source) in [
        (
            "zero-length generic terminal",
            r#"
                data Generic<T> { value: T; }
                data Holder { marker: u8; absent: [Generic<u16>; 0]; }
                data Cell { bytes: [u8; 4]; }
                machine Cell::exercise(&mut self) {
                    let view: &Holder = &self.bytes[0] as &Holder;
                }
            "#,
        ),
        (
            "zero-length array-mediated cycle",
            r#"
                data Node { marker: u8; absent: [Node; 0]; }
                data Cell { bytes: [u8; 4]; }
                machine Cell::exercise(&mut self) {
                    let view: &Node = &self.bytes[0] as &Node;
                }
            "#,
        ),
    ] {
        assert_typed_recast_has_no_loan(source, context);
    }
}

#[test]
fn generic_invariant_cased_and_cyclic_array_fields_publish_no_precise_loan() {
    for (context, source) in [
        (
            "generic record array field",
            r#"
                data Generic<T> { value: T; }
                data Holder { values: [Generic<u16>; 2]; }
                data Cell { bytes: [u8; 16]; }
                machine Cell::exercise(&mut self) {
                    let view: &Holder = &self.bytes[0] as &Holder;
                }
            "#,
        ),
        (
            "invariant-bearing record array field",
            r#"
                data Invariant
                where value <= limit,
                { value: u16; limit: u16; }
                data Holder { values: [Invariant; 2]; }
                data Cell { bytes: [u8; 16]; }
                machine Cell::exercise(&mut self) {
                    let view: &Holder = &self.bytes[0] as &Holder;
                }
            "#,
        ),
        (
            "cased record array field",
            r#"
                data Choice {
                    case First(value: u16);
                    case Second(value: u16);
                }
                data Holder { values: [Choice; 2]; }
                data Cell { bytes: [u8; 16]; }
                machine Cell::exercise(&mut self) {
                    let view: &Holder = &self.bytes[0] as &Holder;
                }
            "#,
        ),
        (
            "cyclic record array field",
            r#"
                data Node { children: [Node; 1]; }
                data Cell { bytes: [u8; 16]; }
                machine Cell::exercise(&mut self) {
                    let view: &Node = &self.bytes[0] as &Node;
                }
            "#,
        ),
    ] {
        assert_typed_recast_has_no_loan(source, context);
    }
}

#[test]
fn fact_establishing_literal_recast_gains_no_loan_and_keeps_its_validation_error() {
    let source = r#"
        data Cell {
            bytes: [u8; 12];
        }

        machine Cell::exercise(&mut self) {
            let view: &bool = &self.bytes[2] as &bool;
        }
    "#;

    assert_invalid_recast_has_no_loan(source, "cannot establish the target's representation facts");
}

fn indexed_mutable_recast_source(mutation: &str, final_use: &str) -> String {
    format!(
        r#"
            data Cell {{
                bytes: [u8; 12];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut u32 = &mut self.bytes[2] as &mut u32;
                {mutation}
                {final_use}
            }}
        "#
    )
}

fn indexed_mutable_nested_array_recast_source(mutation: &str, final_use: &str) -> String {
    format!(
        r#"
            data Cell {{
                bytes: [u8; 16];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut [[u16; 2]; 2] =
                    &mut self.bytes[3] as &mut [[u16; 2]; 2];
                {mutation}
                {final_use}
            }}
        "#
    )
}

fn indexed_mutable_nested_record_array_recast_source(mutation: &str, final_use: &str) -> String {
    format!(
        r#"
            data Header {{
                code: u8;
            }}

            data Desc {{
                head: Header;
                tail: u32;
            }}

            data Cell {{
                bytes: [u8; 40];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut [[Desc; 2]; 2] =
                    &mut self.bytes[3] as &mut [[Desc; 2]; 2];
                {mutation}
                {final_use}
            }}
        "#
    )
}

fn assert_invalid_recast_has_no_loan(source: &str, expected_diagnostic: &str) {
    let typed = typed_program(source);
    let diagnostics = validation::validate_program(&typed)
        .expect_err("the malformed indexed recast must remain rejected");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected_diagnostic),
        "expected `{expected_diagnostic}`, got:\n{combined}"
    );

    let facts = build_borrow_facts(&typed);
    assert_eq!(
        facts.loans.iter().count(),
        0,
        "an invalid recast cannot publish a loan footprint"
    );
}

fn assert_valid_recast_has_no_loan(source: &str, context: &str) {
    let typed = typed_program(source);
    validation::validate_program(&typed)
        .unwrap_or_else(|diagnostics| panic!("{context} should remain valid: {diagnostics:#?}"));
    let facts = build_borrow_facts(&typed);
    assert_eq!(facts.loans.iter().count(), 0, "{context}");
}

fn assert_typed_recast_has_no_loan(source: &str, context: &str) {
    let typed = typed_program(source);
    assert!(
        validation::validate_program(&typed).is_err(),
        "{context} unexpectedly entered the ordinary raw recast subset"
    );
    let facts = build_borrow_facts(&typed);
    assert_eq!(facts.loans.iter().count(), 0, "{context}");
}

fn assert_conflict(diagnostics: &[diagnostics::Diagnostic], source: &str, owner: &str) {
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(&format!(
            "mutates `{source}` while local borrow `{owner}` is still active"
        )),
        "expected recast loan conflict, got:\n{combined}"
    );
}
