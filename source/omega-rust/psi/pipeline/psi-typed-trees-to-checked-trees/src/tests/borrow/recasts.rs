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
fn literal_indexed_nested_fixed_array_recast_retains_the_exact_range_for_both_polarities() {
    let source = r#"
        data Cell {
            bytes: [u8; 24];
        }

        machine observe(value: &[[u16; 2]; 2]) {
        }

        machine Cell::exercise(&mut self) {
            let shared: &[[u16; 2]; 2] = &self.bytes[2] as &[[u16; 2]; 2];
            observe(shared);
            let mutable: &mut [[u16; 2]; 2] =
                &mut self.bytes[12] as &mut [[u16; 2]; 2];
            mutable[1][1] = 1;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed).expect("both nested-array recasts should validate");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 2, "one loan per validated nested-array recast");
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
fn literal_indexed_nested_fixed_array_recast_keeps_immediate_siblings_writable() {
    check_program(&indexed_mutable_nested_array_recast_source(
        "self.bytes[2] = 1; self.bytes[11] = 1;",
        "view[1][1] = 2;",
    ))
    .expect("the bytes immediately before and after [3, 11) are disjoint");
}

#[test]
fn literal_indexed_nested_record_array_recast_retains_padded_ranges_for_both_polarities() {
    let source = r#"
        data Header {
            code: u8;
        }

        data Desc {
            head: Header;
            tail: u32;
        }

        data Cell {
            bytes: [u8; 80];
        }

        machine observe(value: &[[Desc; 2]; 2]) {
        }

        machine Cell::exercise(&mut self) {
            let shared: &[[Desc; 2]; 2] = &self.bytes[2] as &[[Desc; 2]; 2];
            observe(shared);
            let mutable: &mut [[Desc; 2]; 2] =
                &mut self.bytes[40] as &mut [[Desc; 2]; 2];
            mutable[1][1].head.code = 1;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed)
        .expect("both nested record-array recasts should validate");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 2, "one loan per nested record-array recast");
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Read
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 2, end: 34 }]
    }));
    assert!(loans.iter().any(|loan| {
        loan.kind == psi_checked_trees::BorrowAccessKind::Mutable
            && facts.loan_segments(loan)
                == [psi_facts::PlaceSegment::FixedRange { start: 40, end: 72 }]
    }));
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
fn literal_indexed_nested_record_array_recast_keeps_immediate_siblings_writable() {
    check_program(&indexed_mutable_nested_record_array_recast_source(
        "self.bytes[2] = 1; self.bytes[35] = 1;",
        "view[1][1].tail = 2;",
    ))
    .expect("the bytes immediately before and after [3, 35) are disjoint");
}

#[test]
fn records_with_nested_array_fields_retain_exact_ranges_for_direct_and_array_targets() {
    let source = r#"
        data Payload {
            prefix: u8;
            code: u32;
        }

        data Packet {
            marker: u8;
            blocks: [[Payload; 2]; 2];
            trailer: u16;
        }

        data Cell {
            bytes: [u8; 256];
        }

        machine observe_packet(value: &Packet) {
        }

        machine observe_packets(value: &[Packet; 2]) {
        }

        machine Cell::exercise(&mut self) {
            let shared_record: &Packet = &self.bytes[2] as &Packet;
            observe_packet(shared_record);
            let mutable_record: &mut Packet = &mut self.bytes[48] as &mut Packet;
            mutable_record.blocks[1][1].code = 1;

            let shared_array: &[Packet; 2] = &self.bytes[96] as &[Packet; 2];
            observe_packets(shared_array);
            let mutable_array: &mut [Packet; 2] =
                &mut self.bytes[176] as &mut [Packet; 2];
            mutable_array[1].blocks[1][1].code = 2;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed)
        .expect("record and record-array recasts with nested array fields should validate");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 4, "one loan per validated recast");
    for (kind, start, end) in [
        (psi_checked_trees::BorrowAccessKind::Read, 2, 42),
        (psi_checked_trees::BorrowAccessKind::Mutable, 48, 88),
        (psi_checked_trees::BorrowAccessKind::Read, 96, 176),
        (psi_checked_trees::BorrowAccessKind::Mutable, 176, 256),
    ] {
        assert!(loans.iter().any(|loan| {
            loan.kind == kind
                && facts.loan_segments(loan) == [psi_facts::PlaceSegment::FixedRange { start, end }]
        }));
    }
}

#[test]
fn record_array_with_nested_array_fields_rejects_padding_and_every_boundary() {
    for mutation in [
        "self.bytes[3] = 1;",
        "self.bytes[4] = 1;",
        "self.bytes[8] = 1;",
        "self.bytes[14] = 1;",
        "self.bytes[15] = 1;",
        "self.bytes[22] = 1;",
        "self.bytes[23] = 1;",
        "self.bytes[42] = 1;",
        "self.bytes[43] = 1;",
        "self.bytes[82] = 1;",
    ] {
        let diagnostics = check_program(&indexed_mutable_array_field_record_recast_source(
            mutation,
            "view[1].blocks[1][1].code = 2;",
        ))
        .expect_err("record, nested-array, and repeated-record padding remain borrowed");

        assert_conflict(&diagnostics, mutation.trim_end_matches(" = 1;"), "view");
    }
}

#[test]
fn record_array_with_nested_array_fields_keeps_immediate_siblings_writable() {
    check_program(&indexed_mutable_array_field_record_recast_source(
        "self.bytes[2] = 1; self.bytes[83] = 1;",
        "view[1].blocks[1][1].code = 2;",
    ))
    .expect("the bytes immediately before and after [3, 83) are disjoint");
}

#[test]
fn nonzero_records_with_zero_array_fields_retain_direct_and_array_ranges() {
    let source = r#"
        data Leaf {
            value: u16;
        }

        data ZeroFieldRecord {
            marker: u8;
            empty_primitives: [[u32; 0]; 2];
            empty_records: [[Leaf; 0]; 2];
            tail: u32;
        }

        data Cell {
            bytes: [u8; 56];
        }

        machine observe_record(value: &ZeroFieldRecord) {
        }

        machine observe_records(value: &[ZeroFieldRecord; 2]) {
        }

        machine Cell::exercise(&mut self) {
            let shared_record: &ZeroFieldRecord =
                &self.bytes[2] as &ZeroFieldRecord;
            observe_record(shared_record);
            let mutable_record: &mut ZeroFieldRecord =
                &mut self.bytes[12] as &mut ZeroFieldRecord;
            mutable_record.tail = 1;

            let shared_array: &[ZeroFieldRecord; 2] =
                &self.bytes[24] as &[ZeroFieldRecord; 2];
            observe_records(shared_array);
            let mutable_array: &mut [ZeroFieldRecord; 2] =
                &mut self.bytes[40] as &mut [ZeroFieldRecord; 2];
            mutable_array[1].tail = 2;
        }
    "#;

    let typed = typed_program(source);
    psi_validation::validate_program(&typed)
        .expect("otherwise-nonzero records may retain validated zero array fields");
    let facts = build_borrow_facts(&typed);
    let loans = facts.loans.iter().map(|(_, loan)| loan).collect::<Vec<_>>();

    assert_eq!(loans.len(), 4, "one loan per validated zero-field recast");
    for (kind, start, end) in [
        (psi_checked_trees::BorrowAccessKind::Read, 2, 10),
        (psi_checked_trees::BorrowAccessKind::Mutable, 12, 20),
        (psi_checked_trees::BorrowAccessKind::Read, 24, 40),
        (psi_checked_trees::BorrowAccessKind::Mutable, 40, 56),
    ] {
        assert!(loans.iter().any(|loan| {
            loan.kind == kind
                && facts.loan_segments(loan) == [psi_facts::PlaceSegment::FixedRange { start, end }]
        }));
    }
}

#[test]
fn zero_field_record_array_recast_rejects_padding_and_record_boundaries() {
    for mutation in [
        "self.bytes[3] = 1;",
        "self.bytes[4] = 1;",
        "self.bytes[6] = 1;",
        "self.bytes[7] = 1;",
        "self.bytes[10] = 1;",
        "self.bytes[11] = 1;",
        "self.bytes[12] = 1;",
        "self.bytes[18] = 1;",
    ] {
        let diagnostics = check_program(&indexed_mutable_zero_field_record_recast_source(
            mutation,
            "view[1].tail = 2;",
        ))
        .expect_err("zero fields add no bytes but the enclosing padded records stay borrowed");

        assert_conflict(&diagnostics, mutation.trim_end_matches(" = 1;"), "view");
    }
}

#[test]
fn zero_field_record_array_recast_keeps_immediate_siblings_writable() {
    check_program(&indexed_mutable_zero_field_record_recast_source(
        "self.bytes[2] = 1; self.bytes[19] = 1;",
        "view[1].tail = 2;",
    ))
    .expect("the bytes immediately before and after [3, 19) are disjoint");
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
fn atomic_nested_array_fields_publish_no_precise_loan() {
    let source = r#"
        data AtomicArrayRecord {
            values: [[AtomicU32; 1]; 2];
        }

        data Cell {
            bytes: [u8; 32];
        }

        machine Cell::exercise(&mut self) {
            let atomic_direct: &AtomicArrayRecord =
                &self.bytes[8] as &AtomicArrayRecord;
            let atomic_array: &[AtomicArrayRecord; 2] =
                &self.bytes[16] as &[AtomicArrayRecord; 2];
        }
    "#;

    assert_valid_recast_has_no_loan(source, "atomic array leaves stay outside exact custody");
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
fn zero_size_records_and_record_arrays_publish_no_precise_loan() {
    let source = r#"
        data ZeroSize {
            absent: [[u32; 0]; 2];
        }
        data Cell { bytes: [u8; 4]; }
        machine Cell::exercise(&mut self) {
            let direct: &ZeroSize = &self.bytes[0] as &ZeroSize;
            let array: &[ZeroSize; 2] = &self.bytes[0] as &[ZeroSize; 2];
        }
    "#;

    assert_valid_recast_has_no_loan(
        source,
        "canonical zero-size targets cannot publish an empty loan authority",
    );
}

#[test]
fn bool_and_constrained_nested_array_fields_publish_no_loan_and_keep_diagnostics() {
    let bool_source = r#"
        data BoolArrayRecord {
            values: [[bool; 1]; 2];
        }
        data Cell { bytes: [u8; 4]; }
        machine Cell::exercise(&mut self) {
            let view: &BoolArrayRecord = &self.bytes[0] as &BoolArrayRecord;
        }
    "#;
    assert_invalid_recast_has_no_loan(bool_source, "must be recursively fact-free");

    let constrained_source = r#"
        domain u16::Small
        requires
            self <= 10;

        data ConstrainedArrayRecord {
            values: [[u16 in Small; 1]; 2];
        }
        data Cell { bytes: [u8; 4]; }
        machine Cell::exercise(&mut self) {
            let view: &ConstrainedArrayRecord =
                &self.bytes[0] as &ConstrainedArrayRecord;
        }
    "#;
    assert_invalid_recast_has_no_loan(constrained_source, "must be recursively fact-free");
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
fn zero_inner_and_outer_nested_fixed_arrays_publish_no_precise_loan() {
    let source = r#"
        data Word {
            value: u16;
        }

        data Cell {
            bytes: [u8; 8];
        }

        machine Cell::exercise(&mut self) {
            let empty_outer: &[[u16; 2]; 0] = &self.bytes[0] as &[[u16; 2]; 0];
            let empty_inner: &[[u16; 0]; 2] = &self.bytes[0] as &[[u16; 0]; 2];
            let empty_record_outer: &[Word; 0] = &self.bytes[0] as &[Word; 0];
            let empty_record_inner: &[[Word; 0]; 2] =
                &self.bytes[0] as &[[Word; 0]; 2];
        }
    "#;

    assert_valid_recast_has_no_loan(
        source,
        "every literal fixed-array level must be nonzero before publishing a range",
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
fn nested_bool_and_constrained_fixed_array_targets_publish_no_loan_and_keep_diagnostics() {
    let bool_source = r#"
        data Cell {
            bytes: [u8; 8];
        }

        machine Cell::exercise(&mut self) {
            let view: &[[bool; 2]; 2] = &self.bytes[0] as &[[bool; 2]; 2];
        }
    "#;
    assert_invalid_recast_has_no_loan(bool_source, "must be recursively fact-free");

    let constrained_source = r#"
        domain u16::Small
        requires
            self <= 10;

        data Cell {
            bytes: [u8; 8];
        }

        machine Cell::exercise(&mut self) {
            let view: &[[u16 in Small; 2]; 2] =
                &self.bytes[0] as &[[u16 in Small; 2]; 2];
        }
    "#;
    assert_invalid_recast_has_no_loan(constrained_source, "must be recursively fact-free");
}

#[test]
fn atomic_scalar_and_fixed_array_targets_remain_valid_but_publish_no_precise_loan() {
    let source = r#"
        data Cell {
            bytes: [u8; 64];
        }

        machine Cell::exercise(&mut self) {
            let top_u32: &[AtomicU32; 2] = &self.bytes[0] as &[AtomicU32; 2];
            let nested_u32: &[[AtomicU32; 2]; 2] =
                &self.bytes[8] as &[[AtomicU32; 2]; 2];
            let top_u64: &[AtomicU64; 1] = &self.bytes[24] as &[AtomicU64; 1];
            let nested_u64: &[[AtomicU64; 1]; 2] =
                &self.bytes[32] as &[[AtomicU64; 1]; 2];
            let direct_u32: &AtomicU32 = &self.bytes[48] as &AtomicU32;
            let direct_u64: &AtomicU64 = &self.bytes[52] as &AtomicU64;
        }
    "#;

    assert_valid_recast_has_no_loan(
        source,
        "atomic names retain ordinary recast behavior without borrowing primitive authority",
    );
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

fn indexed_mutable_array_field_record_recast_source(mutation: &str, final_use: &str) -> String {
    format!(
        r#"
            data Payload {{
                prefix: u8;
                code: u32;
            }}

            data Packet {{
                marker: u8;
                blocks: [[Payload; 2]; 2];
                trailer: u16;
            }}

            data Cell {{
                bytes: [u8; 88];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut [Packet; 2] =
                    &mut self.bytes[3] as &mut [Packet; 2];
                {mutation}
                {final_use}
            }}
        "#
    )
}

fn indexed_mutable_zero_field_record_recast_source(mutation: &str, final_use: &str) -> String {
    format!(
        r#"
            data Leaf {{
                value: u16;
            }}

            data ZeroFieldRecord {{
                marker: u8;
                empty_primitives: [[u32; 0]; 2];
                empty_records: [[Leaf; 0]; 2];
                tail: u32;
            }}

            data Cell {{
                bytes: [u8; 24];
            }}

            machine Cell::exercise(&mut self) {{
                let view: &mut [ZeroFieldRecord; 2] =
                    &mut self.bytes[3] as &mut [ZeroFieldRecord; 2];
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

fn assert_typed_recast_has_no_loan(source: &str, context: &str) {
    let typed = typed_program(source);
    assert!(
        psi_validation::validate_program(&typed).is_err(),
        "{context} unexpectedly entered the ordinary raw recast subset"
    );
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
