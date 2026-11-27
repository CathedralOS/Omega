//! Erased sequence storage can be shared across lifetimes, but each value
//! still carries the outer view region and every region of its elements.

use super::build_purpose::judge_build;
use super::*;

const SOURCE: &str = r#"
data Cell<'storage> { bytes: @'storage [u8]; }
machine echo<'storage>(items: @'storage [Cell<'storage>]) -> @'storage [Cell<'storage>] { items }
machine consume<'storage>(items: @'storage [Cell<'storage>]) { }
machine owned<'storage>(items: [Cell<'storage>]) -> [Cell<'storage>] { items }
machine build_entry<'left, 'right>(
    left: @'left [Cell<'left>], right: @'right [Cell<'right>], items: [Cell<'left>]
) {
    consume(echo(left));
    consume(echo(right));
    let moved: [Cell<'left>] = owned(items);
}
"#;

#[test]
fn sequence_calls_keep_outer_and_element_regions_on_one_erased_row() {
    let result = judge_build(SOURCE).unwrap();
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let views = accepted
        .elaboration
        .module
        .types.structural_types
        .iter()
        .filter(|row| {
            matches!(
                row.shape,
                terminal_psi::StructuralTypeShape::ElementView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    ..
                }
            )
        })
        .count();
    assert_eq!(views, 1, "lifetimes do not select separate storage rows");
    accepted.publish_invocation().unwrap();

    // The second helper call cannot give its one lifetime both 'left and
    // 'right. Dropping either the outer or element tuple loses this refusal.
    let changed = SOURCE.replace(
        "right: @'right [Cell<'right>]",
        "right: @'left [Cell<'right>]",
    );
    let refusal = match judge_build(&changed) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal,
        other => panic!("{other:?}"),
    };
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        refusal
            .detail
            .contains("call that gives one lifetime two regions"),
        "{refusal:?}"
    );
}

/// Moving two exclusive view carriers into a record retains their actual
/// source loans. Distinct fields do not make overlapping backing disjoint.
#[test]
fn mutable_sequence_fields_preserve_each_backing_loan() {
    let source = r#"
        data Cell [copy] { value: u32; }
        data Pair<'left, 'right> {
            first: @'left mutable [Cell];
            second: @'right mutable [Cell];
        }
        machine update(pair: @mutable Pair) {
            transition pair.first.length > 0u64 && pair.second.length > 0u64 {
                true -> fill(pair),
                _ -> done()
            }
            machine fill(pair: @mutable Pair)
            requires pair.first.length > 0u64 && pair.second.length > 0u64
            {
                pair.first[0u64].value = 7u32;
                pair.second[0u64].value = 9u32;
            }
            machine done() {}
        }
        machine build_entry() -> u32 {
            let mutable left: [Cell; 1] = [Cell { value: 1u32 }];
            let mutable right: [Cell; 1] = [Cell { value: 2u32 }];
            let mutable pair: Pair = Pair {
                first: @mutable left[..], second: @mutable right[..]
            };
            update(@mutable pair);
            match left[0u64].value == 7u32 && right[0u64].value == 9u32 {
                true -> 16u32,
                false -> 0u32
            }
        }
    "#;
    let whole = source
        .replace(
            "pair.first[0u64].value = 7u32;",
            "pair.first[0u64] = Cell { value: 7u32 };",
        )
        .replace(
            "pair.second[0u64].value = 9u32;",
            "pair.second[0u64] = Cell { value: 9u32 };",
        );
    for program in [source, whole.as_str()] {
        let positive = judge_build(program).unwrap();
        let RouteVerdict::Accepted(accepted) = positive else {
            panic!("{positive:?}")
        };
        // Equal lengths do not let a foreign artifact substitute a different
        // descriptor for the one whose selected element the edge captures.
        let mut forged = accepted.elaboration.module.clone();
        let body = forged
            .machines
            .iter_mut()
            .find(|machine| {
                machine.blocks.iter().any(|block| {
                    block.terminator.storage_uses().iter().any(|projection| {
                        projection
                            .path
                            .iter()
                            .any(|segment| segment.runtime_index().is_some())
                    })
                })
            })
            .unwrap();
        let wrong = body
            .operations()
            .filter_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::ElementViewLength { source } => Some(source.clone()),
                _ => None,
            })
            .nth(1)
            .unwrap();
        let first = body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::ElementViewLength { source } => Some(source),
                _ => None,
            })
            .unwrap();
        *first = wrong;
        assert!(
            terminal_codec::EvidenceSealValidator::new(&forged, &accepted.seals)
                .validate_all()
                .is_err(),
            "the receiver must reject a count from another descriptor"
        );
        assert_integer_result(*accepted, 16);
    }
    let outside = source.replace("pair.first[0u64].value =", "pair.first[1u64].value =");
    let negative = judge_build(&outside).unwrap();
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref unproved, .. } if unproved.iter().any(|failure| failure.statement.contains("storage index bounds"))),
        "{negative:?}"
    );
    let overlapping = source.replace("second: @mutable right[..]", "second: @mutable left[..]");
    let negative = judge_build(&overlapping).unwrap();
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref conflicts, .. } if !conflicts.is_empty()),
        "{negative:?}"
    );
}

#[test]
fn mutable_view_inputs_can_be_retained_in_a_local_context() {
    let source = r#"
        data Cell [copy] { value: u32; }
        data Context<'bytes, 'fields> {
            bytes: @'bytes mutable [u8];
            fields: @'fields mutable [Cell];
        }
        machine byte(output: @mutable [u8]) requires output.length > 0u64 {
            output[0u64] = 11u8;
        }
        machine cells(fields: @mutable [Cell]) requires fields.length > 0u64 {
            fields[0u64].value = 22u32;
        }
        machine touch_bytes(bytes: @mutable [u8]) {}
        machine touch_fields(fields: @mutable [Cell]) {}
        machine populate(bytes: @mutable [u8], fields: @mutable [Cell]) {
            let mutable context: Context = Context { bytes: bytes, fields: fields };
            transition context.bytes.length > 0u64 && context.fields.length > 0u64 {
                true -> fill(@mutable context),
                false -> done()
            }
            machine fill(context: @mutable Context)
            requires context.bytes.length > 0u64 && context.fields.length > 0u64 {
                byte(context.bytes);
                context.fields[0u64].value = 22u32;
            }
            machine done() {}
        }
        machine build_entry() -> u32 {
            let mutable bytes: [u8; 1] = [0u8];
            let mutable fields: [Cell; 1] = [Cell { value: 0u32 }];
            populate(@mutable bytes[..], @mutable fields[..]);
            match bytes[0u64] == 11u8 && fields[0u64].value == 22u32 {
                true -> 1u32,
                false -> 0u32
            }
        }
    "#;
    let ranged = source.replace("byte(context.bytes)", "byte(@mutable context.bytes[..])");
    let typed_ranged = ranged.replace(
        "context.fields[0u64].value = 22u32;",
        "cells(@mutable context.fields[..]);",
    );
    assert_ne!(source, ranged);
    assert_ne!(ranged, typed_ranged);
    let typed = super::boundary_calls::typed(source);
    let context = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Context")
        .unwrap();
    for member in typed.data_members(context) {
        if let crate::source_state::data::DataMember::Field(field) = member {
            assert_eq!(
                typed
                    .type_reference_table
                    .operand_lifetime_owner(field.type_reference),
                context.symbol,
                "a stored view keeps the declaration that owns its lifetime"
            );
        }
    }
    for program in [source, ranged.as_str(), typed_ranged.as_str()] {
        let positive = judge_build(program).unwrap();
        let RouteVerdict::Accepted(accepted) = positive else {
            panic!("{positive:?}")
        };
        assert_integer_result(*accepted, 1);
    }
    for competing in ["touch_bytes(bytes);", "touch_fields(fields);"] {
        let constructor = "let mutable context: Context = Context { bytes: bytes, fields: fields };";
        let conflicting = source.replace(constructor, &format!("{constructor} {competing}"));
        let negative = judge_build(&conflicting).unwrap();
        assert!(
            matches!(negative, RouteVerdict::Rejected { ref conflicts, .. } if !conflicts.is_empty()),
            "{competing}: {negative:?}"
        );
    }
}

#[test]
fn indexed_view_loans_capture_noncopy_elements_with_their_own_bounds() {
    let source = r#"
        data Cell { value: u32; }
        machine get(value: @u32) -> u32 { $value }
        machine fetch(items: @[Cell]) -> u32 {
            transition items.length > 0u64 { true -> selected(items), false -> 0u32 }
            machine selected(items: @[Cell]) -> u32 requires items.length > 0u64 {
                get(@items[0u64].value)
            }
        }
        machine build_entry() -> u32 {
            let items: [Cell; 1] = [Cell { value: 17u32 }];
            fetch(@items[..])
        }
    "#;
    let positive = judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 17);

    let mutable = source
        .replace(
            "machine get(value: @u32) -> u32 { $value }",
            "machine get(value: @mutable u32) -> u32 { $value = 17u32; $value }",
        )
        .replace("items: @[Cell]", "items: @mutable [Cell]")
        .replace("get(@items[", "get(@mutable items[")
        .replace(
            "let items: [Cell; 1] = [Cell { value: 17u32 }]",
            "let mutable items: [Cell; 1] = [Cell { value: 3u32 }]",
        )
        .replace("fetch(@items[..])", "fetch(@mutable items[..])");
    let positive = judge_build(&mutable).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 17);

    let outside = source.replace("items[0u64]", "items[1u64]");
    let negative = judge_build(&outside).unwrap();
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|proof| proof.statement.contains("storage index bounds"))),
        "{negative:?}"
    );

    // Reading one field of a non-copy element copies only that field.
    let field = source.replace("get(@items[0u64].value)", "items[0u64].value");
    let positive = judge_build(&field).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 17);

    // Reading the whole non-copy element would copy it out of the view.
    let whole = source
        .replace(
            "machine get(value: @u32) -> u32 { $value }",
            "machine take(cell: Cell) -> u32 { cell.value }",
        )
        .replace("get(@items[0u64].value)", "take(items[0u64])");
    let negative = judge_build(&whole).unwrap();
    assert!(
        format!("{negative:?}")
            .contains("copying a slice element requires a copyable element type"),
        "{negative:?}"
    );
}

#[test]
fn a_returned_slice_element_keeps_the_region_of_its_backing_array() {
    let source = r#"
        data Cell { value: u32; }
        data Row { cells: [Cell; 1]; }
        machine first(row: @mutable Row) -> @mutable Cell {
            let cells: @mutable [Cell] = @mutable row.cells[..];
            @mutable cells[0]
        }
        machine build_entry() -> u32 {
            let mutable row: Row = Row { cells: [Cell { value: 3u32 }] };
            let child: @mutable Cell = first(@mutable row);
            child.value = 17u32;
            row.cells[0].value
        }
    "#;
    let positive = judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 17);

    let out_of_range = source.replace("@mutable cells[0]", "@mutable cells[1]");
    let refusal = match judge_build(&out_of_range) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal,
        other => panic!("{other:?}"),
    };
    assert!(
        refusal.detail.contains("outside its fixed array bounds"),
        "{refusal:?}"
    );

    let local = source.replace(
        "let cells: @mutable [Cell] = @mutable row.cells[..]",
        "let mutable local: [Cell; 1] = [Cell { value: 3u32 }]; let cells: @mutable [Cell] = @mutable local[..]",
    );
    let negative = judge_build(&local).unwrap();
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref violations, .. }
            if violations.iter().any(|failure| failure.statement.contains("local storage cannot establish"))),
        "{negative:?}"
    );
}

#[test]
fn each_selector_returns_only_the_child_created_on_its_own_arm() {
    let source = include_str!(
        "../../../../../../../../samples/cli/probes/case_selected_slice_reference/main.omg"
    )
    .replace("machine main()", "machine build_entry()");
    let wrong = source.replace("alternate[0].value == 11", "alternate[0].value == 12");
    for (program, expect_trap) in [(&source, false), (&wrong, true)] {
        let verdict = judge_build(program).unwrap();
        let RouteVerdict::Accepted(accepted) = verdict else {
            panic!("{verdict:?}")
        };
        let status = execute(*accepted, 10_000);
        if expect_trap {
            assert!(
                matches!(status, terminal_interpreter::TerminalExecutionStatus::Crashed(ref crash)
                    if crash.cause == semantic_vocabulary::CrashCause::Trap),
                "{status:?}"
            );
        } else {
            assert!(
                matches!(
                    status,
                    terminal_interpreter::TerminalExecutionStatus::Complete(
                        terminal_interpreter::TerminalExecutionResult::Unit
                    )
                ),
                "{status:?}"
            );
        }
    }
}

#[test]
fn a_later_argument_cannot_retarget_a_borrowed_slice_element() {
    let source = r#"
        data Cell { value: u64; }
        data Row { cells: [Cell; 2]; }
        data Cursor { index: u64; }
        machine advance(cursor: @mutable Cursor) -> u64 {
            cursor.index = 1u64;
            19u64
        }
        machine write(cell: @mutable Cell, value: u64) { cell.value = value; }
        machine fill(row: @mutable Row, cursor: @mutable Cursor) {
            let cells: @mutable [Cell] = @mutable row.cells[..];
            write(@mutable cells[cursor.index], advance(@mutable cursor));
        }
        machine build_entry() -> u32 {
            let mutable row: Row = Row { cells: [Cell { value: 3u64 }, Cell { value: 7u64 }] };
            let mutable cursor: Cursor = Cursor { index: 0u64 };
            fill(@mutable row, @mutable cursor);
            transition row.cells[0].value == 19u64 && row.cells[1].value == 7u64
                && cursor.index == 1u64 { true -> 47u32, false -> 0u32 }
        }
    "#;
    let result = judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    assert_integer_result(*accepted, 47);
}

#[test]
fn bare_ranges_lend_shared_element_views_of_noncopy_records() {
    // expressions.md, "Borrowed range access": a bare range forms a shared
    // view when a call receives it as a reference, exactly as `@items[..]`.
    let source = r#"
        data Cell { value: u32; }
        machine first(items: @[Cell]) -> u32 {
            transition items.length > 0u64 { true -> read(items), false -> 0u32 }
            machine read(items: @[Cell]) -> u32 requires items.length > 0u64 {
                items[0u64].value
            }
        }
        machine build_entry() -> u32 {
            let items: [Cell; 3] = [Cell { value: 1u32 }, Cell { value: 7u32 }, Cell { value: 9u32 }];
            let suffix: u32 = first(items[1u64..]);
            let whole: u32 = first(items[..]);
            transition whole == 1u32 { true -> suffix, false -> 0u32 }
        }
    "#;
    let positive = judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 7);

    // An expected `@mutable [T]` never inserts the mutable borrow.
    let mutable = source
        .replace("items: @[Cell]", "items: @mutable [Cell]")
        .replace("let items: [Cell; 3]", "let mutable items: [Cell; 3]");
    let negative = judge_build(&mutable);
    assert!(
        !matches!(negative, Ok(RouteVerdict::Accepted(_))),
        "{negative:?}"
    );
}

#[test]
fn inferred_array_extent_is_retained_in_a_lexical_child() {
    let source = r#"
        machine extent<const N: u64>(items: @[u8; N]) -> u64 {
            finish(items)
            machine finish(items: @[u8; N]) -> u64 { N }
        }
        machine build_entry() -> u64 {
            let items: [u8; 3] = [1u8, 2u8, 3u8];
            extent(@items)
        }
    "#;
    let typed = super::build_purpose::frontier_of("machine product_entry() {}", source);
    let typed = crate::preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
    let entry = super::build_purpose::symbol(typed.trees(), "build_entry");
    let mut verdicts = crate::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source_provenance::DependencyScope::Build,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
        .program,
    );
    let positive = verdicts.remove(0).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 3);
}

fn execute(
    accepted: crate::AcceptedClosure,
    budget: u64,
) -> terminal_interpreter::TerminalExecutionStatus {
    let publication = accepted.publish_invocation().unwrap();
    let program =
        terminal_interpreter::TerminalProgram::borrow(publication.artifact.view()).unwrap();
    let mut instance = program.instantiate(Default::default()).unwrap();
    instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(budget))
        .unwrap()
}

fn assert_integer_result(accepted: crate::AcceptedClosure, expected: u128) {
    let status = execute(accepted, 1_000);
    assert!(
        matches!(status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Integer {
                        value: semantic_vocabulary::IntegerValue::Unsigned(actual), ..
                    }
                )
            ) if actual == expected
        ),
        "{status:?}"
    );
}
