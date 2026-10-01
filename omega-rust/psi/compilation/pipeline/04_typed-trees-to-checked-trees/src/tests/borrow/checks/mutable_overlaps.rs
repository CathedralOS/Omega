use super::check_program;

#[test]
fn nested_copy_receiver_keeps_returned_reference_but_releases_scalar_result() {
    let source = "data Counter [copy] { count: i32; }
        machine Counter::view(&self) -> &i32 { &self.count }
        machine Counter::read(&self) -> i32 { self.count }
        machine consume_view(value: &i32, counter: &mut Counter) {}
        machine consume_value(value: i32, counter: &mut Counter) {}
        machine advance() {
            let mut counter: Counter;
            consume_view(counter.view(), &mut counter);
        }";
    let diagnostics =
        check_program(source).expect_err("returned reference still loans receiver storage");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
        "{diagnostics:#?}"
    );
    let scalar = source.replace(
        "consume_view(counter.view()",
        "consume_value(counter.read()",
    );
    check_program(&scalar).unwrap_or_else(|diagnostics| panic!("{scalar}: {diagnostics:#?}"));
}

#[test]
fn nested_scalar_call_loans_end_on_return_but_outer_loans_remain() {
    let declarations = "machine stamp(value: &mut i32, replacement: i32) -> i32 {
            value = replacement; replacement
        }
        machine consume(first: i32, second: i32) {}
        machine consume_borrow(first: &mut i32, second: i32) {}";
    let accepted = format!(
        "{declarations}
        machine advance() {{
            let mut scratch: i32 = 0;
            consume(stamp(&mut scratch, 7), scratch);
        }}"
    );
    check_program(&accepted).unwrap_or_else(|diagnostics| panic!("{accepted}: {diagnostics:#?}"));
    let rejected = accepted.replace(
        "consume(stamp(&mut scratch, 7), scratch)",
        "consume_borrow(&mut scratch, stamp(&mut scratch, 7))",
    );
    let diagnostics =
        check_program(&rejected).expect_err("the outer loan remains across its later operand");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("as mutable more than once")),
        "{diagnostics:#?}"
    );
}

/// A nested borrow records its loan, then the reads its place selector
/// performs. The selector read still meets an earlier retained `&mut` of the
/// selector's own storage, while a selector on other storage does not.
#[test]
fn nested_borrow_selector_reads_conflict_with_an_outer_mutable_loan() {
    let accepted = "data Cell { value: i32; }
        machine read_cell(cell: &Cell) -> i32 { cell.value }
        machine consume(position: &mut u64, value: i32) {}
        machine advance() {
            let mut cells: [Cell; 4];
            let mut position: u64 = 0;
            let slot: u64 = 1;
            consume(&mut position, read_cell(&cells[slot]));
        }";
    check_program(accepted).unwrap_or_else(|diagnostics| panic!("{accepted}: {diagnostics:#?}"));
    let rejected = accepted.replace("&cells[slot]", "&cells[position]");
    let diagnostics = check_program(&rejected)
        .expect_err("the nested selector reads the mutably loaned position");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
        "{diagnostics:#?}"
    );
}

#[test]
fn evaluated_scalar_argument_ends_before_a_later_mutable_borrow() {
    for argument in ["counter.count", "counter.count + 1"] {
        let source = format!(
            "data Counter {{ count: i32; }}
             machine advance() {{
                 let mut counter: Counter;
                 transition {{ _ -> consume({argument}, &mut counter) }}
                 state consume(value: i32, counter: &mut Counter) {{}}
             }}"
        );
        check_program(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn scalar_argument_still_conflicts_with_an_earlier_mutable_borrow() {
    let source = "data Counter { count: i32; }
        machine advance() {
            let mut counter: Counter;
            transition { _ -> consume(&mut counter, counter.count) }
            state consume(counter: &mut Counter, value: i32) {}
        }";
    let diagnostics = check_program(source).expect_err("the first argument retains its loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
        "{diagnostics:#?}"
    );
}

#[test]
fn shared_reference_argument_is_not_a_transient_scalar_read() {
    for argument in ["&counter.count", "borrow_count(&counter.count)"] {
        let source = format!(
            "data Counter {{ count: i32; }}
             machine borrow_count(value: &i32) -> &i32 {{ value }}
             machine advance() {{
                 let mut counter: Counter;
                 transition {{ _ -> consume({argument}, &mut counter) }}
                 state consume(value: &i32, counter: &mut Counter) {{}}
             }}"
        );
        let diagnostics =
            check_program(&source).expect_err("shared loan survives argument evaluation");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn transient_scalar_reads_still_conflict_with_active_local_loans() {
    let source = "data Counter { count: i32; }
        machine consume(value: i32) {}
        machine use_loan(counter: &mut Counter) {}
        machine advance() {
            let mut counter: Counter;
            let loan: &mut Counter = &mut counter;
            consume(counter.count);
            use_loan(loan);
        }";
    let diagnostics = check_program(source).expect_err("the active local loan remains exclusive");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("still active")),
        "{diagnostics:#?}"
    );
}

/// Bodyless provider requirements create caller-side loans too, so declaration
/// validation must reject their ambiguous view sources before any call is
/// attributed.
#[test]
fn rejects_ambiguous_view_return_from_boundary_trait_signature() {
    let source = r#"
        boundary trait Storage {
            machine view(first: &u8, second: &u8) -> &u8;
        }

        data Main {}

        machine Main::main(&mut self) {}
    "#;

    let diagnostics = check_program(source)
        .expect_err("a bodyless view requirement with two ref inputs should be ambiguous");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot infer which input the returned view borrows"),
        "expected the bodyless-signature elision rejection, got:\n{combined}"
    );
}
