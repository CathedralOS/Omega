//! Erased sequence storage can be shared across lifetimes, but each value
//! still carries the outer view region and every region of its elements.

use super::build_purpose::judge_build;
use super::*;

const SOURCE: &str = r#"
data Cell<'storage> { bytes: &'storage [u8]; }
machine echo<'storage>(items: &'storage [Cell<'storage>]) -> &'storage [Cell<'storage>] { items }
machine consume<'storage>(items: &'storage [Cell<'storage>]) { }
machine owned<'storage>(items: [Cell<'storage>]) -> [Cell<'storage>] { items }
machine build_entry<'left, 'right>(
    left: &'left [Cell<'left>], right: &'right [Cell<'right>], items: [Cell<'left>]
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
        .structural_types
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
        "right: &'right [Cell<'right>]",
        "right: &'left [Cell<'right>]",
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
            first: &'left mut [Cell];
            second: &'right mut [Cell];
        }
        machine update(pair: &mut Pair) {
            transition pair.first.len > 0u64 && pair.second.len > 0u64 {
                true -> fill(pair)
                _ -> done()
            }
            machine fill(pair: &mut Pair)
            requires pair.first.len > 0u64 && pair.second.len > 0u64
            {
                pair.first[0u64].value = 7u32;
                pair.second[0u64].value = 9u32;
            }
            machine done() {}
        }
        machine build_entry() -> u32 {
            let mut left: [Cell; 1] = [Cell { value: 1u32 }];
            let mut right: [Cell; 1] = [Cell { value: 2u32 }];
            let mut pair: Pair = Pair {
                first: &mut left[..], second: &mut right[..]
            };
            update(&mut pair);
            match left[0u64].value == 7u32 && right[0u64].value == 9u32 {
                true -> 16u32
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
    let overlapping = source.replace("second: &mut right[..]", "second: &mut left[..]");
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
            bytes: &'bytes mut [u8];
            fields: &'fields mut [Cell];
        }
        machine byte(output: &mut [u8]) requires output.len > 0u64 {
            output[0u64] = 11u8;
        }
        machine cells(fields: &mut [Cell]) requires fields.len > 0u64 {
            fields[0u64].value = 22u32;
        }
        machine touch_bytes(bytes: &mut [u8]) {}
        machine touch_fields(fields: &mut [Cell]) {}
        machine populate(bytes: &mut [u8], fields: &mut [Cell]) {
            let mut context: Context = Context { bytes: bytes, fields: fields };
            transition context.bytes.len > 0u64 && context.fields.len > 0u64 {
                true -> fill(&mut context)
                false -> done()
            }
            machine fill(context: &mut Context)
            requires context.bytes.len > 0u64 && context.fields.len > 0u64 {
                byte(context.bytes);
                context.fields[0u64].value = 22u32;
            }
            machine done() {}
        }
        machine build_entry() -> u32 {
            let mut bytes: [u8; 1] = [0u8];
            let mut fields: [Cell; 1] = [Cell { value: 0u32 }];
            populate(&mut bytes[..], &mut fields[..]);
            match bytes[0u64] == 11u8 && fields[0u64].value == 22u32 {
                true -> 1u32
                false -> 0u32
            }
        }
    "#;
    let ranged = source.replace("byte(context.bytes)", "byte(&mut context.bytes[..])");
    let typed_ranged = ranged.replace(
        "context.fields[0u64].value = 22u32;",
        "cells(&mut context.fields[..]);",
    );
    assert_ne!(source, ranged);
    assert_ne!(ranged, typed_ranged);
    for program in [source, ranged.as_str(), typed_ranged.as_str()] {
        let positive = judge_build(program).unwrap();
        let RouteVerdict::Accepted(accepted) = positive else {
            panic!("{positive:?}")
        };
        assert_integer_result(*accepted, 1);
    }
    for competing in ["touch_bytes(bytes);", "touch_fields(fields);"] {
        let constructor = "let mut context: Context = Context { bytes: bytes, fields: fields };";
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
        machine get(value: &u32) -> u32 { value }
        machine fetch(items: &[Cell]) -> u32 {
            transition items.len > 0u64 { true -> selected(items) false -> 0u32 }
            machine selected(items: &[Cell]) -> u32 requires items.len > 0u64 {
                get(&items[0u64].value)
            }
        }
        machine build_entry() -> u32 {
            let items: [Cell; 1] = [Cell { value: 17u32 }];
            fetch(&items[..])
        }
    "#;
    let positive = judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 17);

    let mutable = source
        .replace(
            "machine get(value: &u32) -> u32 { value }",
            "machine get(value: &mut u32) -> u32 { value = 17u32; value }",
        )
        .replace("items: &[Cell]", "items: &mut [Cell]")
        .replace("get(&items[", "get(&mut items[")
        .replace(
            "let items: [Cell; 1] = [Cell { value: 17u32 }]",
            "let mut items: [Cell; 1] = [Cell { value: 3u32 }]",
        )
        .replace("fetch(&items[..])", "fetch(&mut items[..])");
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

    let copied = source.replace("get(&items[0u64].value)", "items[0u64].value");
    let negative = judge_build(&copied).unwrap();
    assert!(
        format!("{negative:?}")
            .contains("copying a slice element requires a copyable element type"),
        "{negative:?}"
    );
}

#[test]
fn inferred_array_extent_is_retained_in_a_lexical_child() {
    let source = r#"
        machine extent<const N: u64>(items: &[u8; N]) -> u64 {
            finish(items)
            machine finish(items: &[u8; N]) -> u64 { N }
        }
        machine build_entry() -> u64 {
            let items: [u8; 3] = [1u8, 2u8, 3u8];
            extent(&items)
        }
    "#;
    let typed = super::build_purpose::frontier_of("machine product_entry() {}", source);
    let (typed, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        typed,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let entry = super::build_purpose::symbol(&typed, "build_entry");
    let mut verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source::DependencyScope::Build,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    let positive = verdicts.remove(0).unwrap();
    let RouteVerdict::Accepted(accepted) = positive else {
        panic!("{positive:?}")
    };
    assert_integer_result(*accepted, 3);
}

fn assert_integer_result(accepted: crate::AcceptedClosure, expected: u128) {
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
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
