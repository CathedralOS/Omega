use super::checks::check_program;
use crate::build_borrow_facts;

#[test]
fn mutable_whole_place_recast_retains_source_loan() {
    let source = r#"
        data Cell {
            value: u64;
        }

        machine Cell::exercise(&mut self) {
            let view: &mut f64 = &mut self.value as &mut f64;
            self.value = 1;
            view = 2.0;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a mutable whole-place recast must retain its source");
    assert_conflict(&diagnostics, "self.value", "view");
}

#[test]
fn shared_whole_place_recast_retains_source_loan() {
    let source = r#"
        data Cell {
            value: u64;
        }

        machine observe(value: &f64) {
        }

        machine Cell::exercise(&mut self) {
            let view: &f64 = &self.value as &f64;
            self.value = 1;
            observe(view);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a shared whole-place recast must retain its source");
    assert_conflict(&diagnostics, "self.value", "view");
}

#[test]
fn whole_member_recast_keeps_disjoint_sibling_writable() {
    let source = r#"
        data Pair {
            left: u64;
            right: u64;
        }

        machine Pair::exercise(&mut self) {
            let view: &mut f64 = &mut self.left as &mut f64;
            self.right = 1;
            view = 2.0;
        }
    "#;

    check_program(source).expect("a whole-member recast must not capture a disjoint sibling");
}

#[test]
fn whole_member_recast_rejects_overlapping_write() {
    let source = r#"
        data Pair {
            left: u64;
            right: u64;
        }

        machine Pair::exercise(&mut self) {
            let view: &mut f64 = &mut self.left as &mut f64;
            self.left = 1;
            view = 2.0;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a whole-member recast must retain the exact field loan");
    assert_conflict(&diagnostics, "self.left", "view");
}

#[test]
fn literal_indexed_recast_retains_the_complete_range_for_both_polarities() {
    let source = r#"
        data Cell {
            bytes: [u8; 12];
        }

        machine observe(value: &u32) {
        }

        machine Cell::exercise(&mut self) {
            let shared: &u32 = &self.bytes[2] as &u32;
            observe(shared);
            let mutable: &mut u16 = &mut self.bytes[8] as &mut u16;
            mutable = 1;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed).expect("both indexed recasts should validate");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 2, "one loan per validated indexed recast");
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Read
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 2, end: 6 }]
    }));
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Mutable
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 8, end: 10 }]
    }));
}

#[test]
fn literal_indexed_recast_rejects_mutation_of_its_first_footprint_byte() {
    let diagnostics = check_program(&indexed_mutable_recast_source(
        "self.bytes[2] = 1;",
        "view = 2;",
    ))
    .expect_err("the first byte is inside the retained u32 footprint");

    assert_conflict(&diagnostics, "self.bytes[2]", "view");
}

#[test]
fn literal_indexed_recast_rejects_mutation_of_its_last_footprint_byte() {
    let diagnostics = check_program(&indexed_mutable_recast_source(
        "self.bytes[5] = 1;",
        "view = 2;",
    ))
    .expect_err("the last byte is inside the retained u32 footprint");

    assert_conflict(&diagnostics, "self.bytes[5]", "view");
}

#[test]
fn literal_indexed_recast_keeps_immediate_sibling_bytes_writable() {
    check_program(&indexed_mutable_recast_source(
        "self.bytes[1] = 1; self.bytes[6] = 1;",
        "view = 2;",
    ))
    .expect("the bytes immediately before and after [2, 6) are disjoint");
}

#[test]
fn literal_indexed_record_recast_retains_its_exact_padded_range() {
    let source = r#"
        data Header {
            code: u8;
        }

        data Desc {
            head: Header;
            tail: u32;
        }

        data Cell {
            bytes: [u8; 24];
        }

        machine observe(value: &Desc) {
        }

        machine Cell::exercise(&mut self) {
            let shared: &Desc = &self.bytes[2] as &Desc;
            observe(shared);
            let mutable: &mut Desc = &mut self.bytes[12] as &mut Desc;
            mutable.head.code = 1;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed).expect("both record recasts should validate");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 2, "one loan per validated record recast");
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Read
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 2, end: 10 }]
    }));
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Mutable
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 12, end: 20 }]
    }));
}

#[test]
fn literal_indexed_record_recast_rejects_mutation_of_its_first_footprint_byte() {
    let diagnostics = check_program(&indexed_mutable_record_recast_source(
        "self.bytes[2] = 1;",
        "view.head.code = 2;",
    ))
    .expect_err("the first byte is inside the retained record footprint");

    assert_conflict(&diagnostics, "self.bytes[2]", "view");
}

#[test]
fn literal_indexed_record_recast_rejects_mutation_of_its_last_footprint_byte() {
    let diagnostics = check_program(&indexed_mutable_record_recast_source(
        "self.bytes[9] = 1;",
        "view.head.code = 2;",
    ))
    .expect_err("the last byte is inside the retained record footprint");

    assert_conflict(&diagnostics, "self.bytes[9]", "view");
}

#[test]
fn literal_indexed_record_recast_rejects_mutation_of_interior_padding() {
    let diagnostics = check_program(&indexed_mutable_record_recast_source(
        "self.bytes[3] = 1;",
        "view.head.code = 2;",
    ))
    .expect_err("canonical record padding remains part of the retained footprint");

    assert_conflict(&diagnostics, "self.bytes[3]", "view");
}

#[test]
fn literal_indexed_record_recast_keeps_immediate_sibling_bytes_writable() {
    check_program(&indexed_mutable_record_recast_source(
        "self.bytes[1] = 1; self.bytes[10] = 1;",
        "view.head.code = 2;",
    ))
    .expect("the bytes immediately before and after [2, 10) are disjoint");
}

#[test]
fn literal_indexed_fixed_array_recast_retains_the_exact_range_for_both_polarities() {
    let source = r#"
        data Cell {
            bytes: [u8; 20];
        }

        machine observe(value: &[u16; 3]) {
        }

        machine Cell::exercise(&mut self) {
            let shared: &[u16; 3] = &self.bytes[2] as &[u16; 3];
            observe(shared);
            let mutable: &mut [u16; 3] = &mut self.bytes[10] as &mut [u16; 3];
            mutable[0] = 1;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed).expect("both fixed-array recasts should validate");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 2, "one loan per validated fixed-array recast");
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Read
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 2, end: 8 }]
    }));
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Mutable
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 10, end: 16 }]
    }));
}

#[test]
fn literal_indexed_fixed_array_recast_rejects_first_and_last_byte_mutations() {
    for mutation in ["self.bytes[3] = 1;", "self.bytes[8] = 1;"] {
        let diagnostics = check_program(&indexed_mutable_array_recast_source(
            mutation,
            "view[0] = 2;",
        ))
        .expect_err("the first and last bytes are inside the retained array footprint");

        assert_conflict(&diagnostics, mutation.trim_end_matches(" = 1;"), "view");
    }
}

#[test]
fn literal_indexed_fixed_array_recast_keeps_immediate_sibling_bytes_writable() {
    check_program(&indexed_mutable_array_recast_source(
        "self.bytes[2] = 1; self.bytes[9] = 1;",
        "view[0] = 2;",
    ))
    .expect("the bytes immediately before and after [3, 9) are disjoint");
}

#[test]
fn unsupported_literal_indexed_aggregate_targets_publish_no_precise_loan() {
    let source = r#"
        data Word {
            value: u16;
        }

        data Cell {
            bytes: [u8; 16];
        }

        machine Cell::exercise(&mut self) {
            let empty: &[u16; 0] = &self.bytes[0] as &[u16; 0];
            let nested: &[[u16; 2]; 2] = &self.bytes[0] as &[[u16; 2]; 2];
            let records: &[Word; 2] = &self.bytes[8] as &[Word; 2];
            let slice: &[u16] = &self.bytes[0] as &[u16];
        }
    "#;

    assert_valid_recast_has_no_loan(
        source,
        "zero-length, nested-array, record-element, and slice targets stay fenced",
    );
}

#[test]
fn bool_and_constrained_fixed_array_targets_publish_no_loan_and_keep_diagnostics() {
    let bool_source = r#"
        data Cell {
            bytes: [u8; 4];
        }

        machine Cell::exercise(&mut self) {
            let view: &[bool; 2] = &self.bytes[0] as &[bool; 2];
        }
    "#;
    assert_invalid_recast_has_no_loan(bool_source, "must be recursively fact-free");

    let constrained_source = r#"
        domain u16::Small
        requires
            self <= 10;

        data Cell {
            bytes: [u8; 4];
        }

        machine Cell::exercise(&mut self) {
            let view: &[u16 in Small; 2] = &self.bytes[0] as &[u16 in Small; 2];
        }
    "#;
    assert_invalid_recast_has_no_loan(constrained_source, "must be recursively fact-free");
}

#[test]
fn bounded_runtime_indexed_recast_remains_outside_precise_loan_publication() {
    let source = r#"
        data Cell {
            bytes: [u8; 12];
        }

        machine Cell::exercise(&mut self, offset: u32 [0..=8]) {
            let view: &mut u32 = &mut self.bytes[offset] as &mut u32;
            view = 1;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed).expect("the bounded runtime recast remains valid");
    let facts = build_borrow_facts(&typed);
    assert_eq!(
        facts.loans.iter().count(),
        0,
        "a bounded runtime offset must not be mistaken for one exact byte range"
    );
}

#[test]
fn out_of_bounds_literal_recast_gains_no_loan_and_keeps_its_validation_error() {
    let source = r#"
        data Cell {
            bytes: [u8; 12];
        }

        machine Cell::exercise(&mut self) {
            let view: &mut u32 = &mut self.bytes[9] as &mut u32;
            view = 1;
        }
    "#;

    assert_invalid_recast_has_no_loan(source, "view would read past the buffer");
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

#[test]
fn constrained_record_literal_recast_gains_no_loan_and_keeps_its_validation_error() {
    let source = r#"
        domain u16::Small
        requires
            self <= 10;

        data Facted {
            value: u16 in Small;
        }

        data Cell {
            bytes: [u8; 4];
        }

        machine Cell::exercise(&mut self) {
            let view: &mut Facted = &mut self.bytes[1] as &mut Facted;
            view.value = 1;
        }
    "#;

    assert_invalid_recast_has_no_loan(source, "must be recursively fact-free");
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

fn indexed_mutable_record_recast_source(mutation: &str, final_use: &str) -> String {
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
                bytes: [u8; 12];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut Desc = &mut self.bytes[2] as &mut Desc;
                {mutation}
                {final_use}
            }}
        "#
    )
}

fn indexed_mutable_array_recast_source(mutation: &str, final_use: &str) -> String {
    format!(
        r#"
            data Cell {{
                bytes: [u8; 12];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut [u16; 3] = &mut self.bytes[3] as &mut [u16; 3];
                {mutation}
                {final_use}
            }}
        "#
    )
}

fn typed_program(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize recast fixture");
    let syntax =
        psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse recast fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve recast fixture");
    psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type recast fixture")
}

fn assert_invalid_recast_has_no_loan(source: &str, expected_diagnostic: &str) {
    let typed = typed_program(source);
    let diagnostics = psi_validation::validate_program(&typed)
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
    psi_validation::validate_program(&typed)
        .unwrap_or_else(|diagnostics| panic!("{context} should remain valid: {diagnostics:#?}"));
    let facts = build_borrow_facts(&typed);
    assert_eq!(facts.loans.iter().count(), 0, "{context}");
}

fn assert_conflict(diagnostics: &[psi_diagnostics::Diagnostic], source: &str, owner: &str) {
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
