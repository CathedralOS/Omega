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

/// A destructure arm copies a plain payload out of the dispatch's saved
/// subject before its target evaluates any argument, so the copy may travel
/// beside a forwarded `&mut` of the subject's root.
#[test]
fn dispatch_copied_payload_travels_beside_a_mutable_root() {
    let declarations = "data Msg { case Ping(x: i32); case Pong(y: i32, z: i32); }
        data Packet { case Empty; case Samples(values: [i32; 2]); }
        data Main { never: Msg; packet: Packet; count: i32; }";
    for (arm, target) in [
        (
            "Msg::Ping { x } -> chk(main, x)",
            "chk(main: &mut Main, x: i32)",
        ),
        (
            "Msg::Ping { x } -> chk(main, x + 1)",
            "chk(main: &mut Main, x: i32)",
        ),
        (
            "Msg::Pong { y, z } -> pair(main, y, z)",
            "pair(main: &mut Main, y: i32, z: i32)",
        ),
    ] {
        let source = format!(
            "{declarations}
             machine Main::build(main: &mut Main) {{
                 transition main.never {{
                     {arm}
                     _ -> done(main)
                 }}
                 state {target} {{}}
                 state done(main: &mut Main) {{}}
             }}"
        );
        check_program(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
    let indexed = format!(
        "{declarations}
         machine Main::build(main: &mut Main) {{
             transition main.packet {{
                 Packet::Samples {{ values }} -> verify(main, values[0], values[1])
                 _ -> done(main)
             }}
             state verify(main: &mut Main, first: i32, second: i32) {{}}
             state done(main: &mut Main) {{}}
         }}"
    );
    check_program(&indexed).unwrap_or_else(|diagnostics| panic!("{indexed}: {diagnostics:#?}"));
}

/// A jump that forwards the caller's own data as the callee's subject is
/// judged by what that callee does through it, not by pairing the subject
/// against every sibling argument.
///
/// The subject is a reborrow of the caller's parent, so a sibling argument
/// derived from the same parent is its child:
/// [ownership](wiki/spec/language/ownership.md) says a shared child freezes the
/// parent's mutation. Freezing is admissible; writing the frozen branch is not.
/// So the callee's footprint decides, and a copy taken at the call site holds
/// no loan to freeze at all. Forwarding this exact parent carrier transfers
/// within the same activation; it does not stage a new ordinary-call loan.
/// Argument evaluation still follows authored order, and ordinary calls retain
/// their separate argument-loan checks.
///
/// Pairing instead admitted none of these. That strictness was an artifact of
/// the static spelling putting the parent among the authored arguments: a
/// receiver was never an argument, so the receiver spelling of every case here
/// was admitted, including the one this test now requires to reject.
#[test]
fn forwarded_subject_conflicts_only_where_the_callee_writes_the_frozen_branch() {
    let declarations = "data Msg { case Ping(x: i32); case Pong(y: i32, z: i32); }
        data Main { never: Msg; count: i32; other: i32; }";
    let program = |arm: &str, target: &str, body: &str| {
        format!(
            "{declarations}
             machine Main::build(main: &mut Main) {{
                 transition main.never {{
                     {arm}
                     _ -> done(main)
                 }}
                 state {target} {{ {body} }}
                 state done(main: &mut Main) {{}}
             }}"
        )
    };

    // A loan of the payload's storage freezes `main.never`; writing it through
    // the subject is the conflict.
    let conflicting = program(
        "Msg::Ping { x } -> peek(main, &x)",
        "peek(main: &mut Main, x: &i32)",
        "main.never = Msg::Pong { y: 1, z: 2 };",
    );
    let diagnostics = check_program(&conflicting)
        .expect_err("the subject writes the storage its sibling argument loans");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("while its mutable receiver writes")),
        "{conflicting}: {diagnostics:#?}"
    );

    // Everything else the pairing rule caught: the same loan beside a disjoint
    // write, and a copy taken at the call site, which holds no loan to freeze.
    for (arm, target, body) in [
        (
            "Msg::Ping { x } -> peek(main, &x)",
            "peek(main: &mut Main, x: &i32)",
            "main.other = 7;",
        ),
        (
            "Msg::Ping { x } -> chk(main, main.count)",
            "chk(main: &mut Main, x: i32)",
            "main.count = x;",
        ),
        (
            "Msg::Ping { x } -> chk(main, main.count)",
            "chk(main: &mut Main, x: i32)",
            "",
        ),
    ] {
        let source = program(arm, target, body);
        check_program(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn retained_payload_loan_freezes_its_variant_but_not_disjoint_parent_fields() {
    for (body, accepts) in [
        ("", true),
        ("main.count = x;", true),
        (
            "main.never = Msg::Pong { y: 1, z: 2 }; main.count = x;",
            false,
        ),
    ] {
        let source = format!(
            "data Msg {{ case Ping(x: i32); case Pong(y: i32, z: i32); }}
             data Main {{ never: Msg; count: i32; }}
             machine Main::build(main: &mut Main) {{
                 transition main.never {{
                     Msg::Ping {{ x }} -> peek(main, &x)
                     _ -> done(main)
                 }}
                 state peek(main: &mut Main, x: &i32) {{ {body} }}
                 state done(main: &mut Main) {{}}
             }}"
        );
        let result = check_program(&source);
        if accepts {
            result.unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        } else {
            let diagnostics = result.expect_err("a retained payload freezes enum replacement");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("while its mutable receiver writes")),
                "{source}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn dispatch_call_payload_uses_the_saved_subject_before_forwarding_mutable_storage() {
    let source = "data Message { case Captured(value: i32); case Empty; }
        data Counter { value: i32; }
        machine snapshot(counter: &Counter) -> Message {
            Message::Captured { value: counter.value }
        }
        machine run(counter: &mut Counter) {
            transition snapshot(counter) {
                Message::Captured { value } -> consume(counter, value)
                _ -> done(counter)
            }
            state consume(counter: &mut Counter, value: i32) { counter.value = value; }
            state done(counter: &mut Counter) {}
        }";
    check_program(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn dispatch_call_does_not_exempt_a_fresh_call_in_the_arm() {
    let source = "data Message { case Captured(value: i32); case Empty; }
        data Counter { value: i32; }
        machine snapshot(counter: &Counter) -> Message {
            Message::Captured { value: counter.value }
        }
        machine read(counter: &Counter) -> i32 { counter.value }
        machine run(counter: &mut Counter) {
            transition snapshot(counter) {
                Message::Captured { value } -> consume(counter, read(counter))
                _ -> done(counter)
            }
            state consume(counter: &mut Counter, value: i32) { counter.value = value; }
            state done(counter: &mut Counter) {}
        }";
    let diagnostics = check_program(source).expect_err("a fresh call observes the loaned root");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
        "{diagnostics:#?}"
    );
}

#[test]
fn dispatch_saved_payload_keeps_new_index_evaluation_in_the_arm() {
    let source = "data Cell [copy] { value: i32; }
        data Message { case Captured(cells: [Cell; 2]); case Empty; }
        data Counter { value: i32; }
        machine snapshot(counter: &Counter) -> Message {
            Message::Captured { cells: [Cell { value: counter.value }, Cell { value: 9 }] }
        }
        machine position(counter: &Counter) -> u64 { 0 }
        machine run(counter: &mut Counter) {
            transition snapshot(counter) {
                Message::Captured { cells } -> consume(counter, cells[position(counter)].value)
                _ -> done(counter)
            }
            state consume(counter: &mut Counter, value: i32) { counter.value = value; }
            state done(counter: &mut Counter) {}
        }";
    let diagnostics =
        check_program(source).expect_err("the selector is not part of the saved subject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("both mutable and read-only")),
        "{diagnostics:#?}"
    );
    let literal_index = source.replace("cells[position(counter)]", "cells[0]");
    check_program(&literal_index).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
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

#[test]
fn nested_affine_call_releases_operand_loans_before_later_arguments() {
    let source = "data Choice { case Value(value: u64); }
        data Counter { value: u64 in Wrapping; }
        machine forward(choice: Choice, counter: &mut Counter) -> Choice {
            counter.value = counter.value + 1;
            choice
        }
        machine stamp(counter: &mut Counter) -> u64 in Wrapping {
            counter.value = counter.value + 1;
            counter.value
        }
        machine consume(choice: Choice, value: u64 in Wrapping, counter: &mut Counter) {}
        machine exercise(choice: Choice, counter: &mut Counter) {
            consume(forward(choice, counter), stamp(counter), counter);
        }";
    check_program(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn nested_structural_result_retains_carried_loans_across_later_arguments() {
    for (access, diagnostic) in [
        ("&mut", "as mutable more than once"),
        ("&", "as both mutable and read-only"),
    ] {
        let source = format!(
            "data View {{ value: {access} i32; }}
             machine keep(value: {access} i32) -> View {{ View {{ value: value }} }}
             machine stamp(value: &mut i32) -> i32 {{ value = 1; 1 }}
             machine consume(view: View, value: i32, next: &mut i32) {{}}
             machine exercise() {{
                 let mut value: i32 = 0;
                 consume(keep({access} value), stamp(&mut value), &mut value);
             }}"
        );
        let diagnostics = check_program(&source)
            .expect_err("a returned structural carrier keeps its original backing loan");
        assert!(
            diagnostics
                .iter()
                .any(|entry| entry.message.contains(diagnostic)),
            "{source}: {diagnostics:#?}"
        );
    }
}
