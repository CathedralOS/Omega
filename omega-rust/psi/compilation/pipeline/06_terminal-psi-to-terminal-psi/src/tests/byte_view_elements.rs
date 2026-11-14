//! One byte of a borrowed byte view, `view[index]`. Read from source, as a
//! Build program: a Build that is handed a path looks at its first byte.
//!
//! byte_views.md, `ByteSequenceRead`: "`u64` index and length; dominating
//! length observation of the identical view; checked `index < length`". The
//! lowered form measures the view and reads the byte directly after, and the
//! read owes `index < length` against that measurement. So a read is accepted
//! exactly where the program has shown the index lies inside this view: a
//! `requires` clause, or a guard, over this view's own `len`.

use super::build_purpose::judge_build;
use super::*;
use terminal_psi::OperationKind;

/// `accepted`, `rejected` with what the program could not establish, or the
/// refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { unestablished, .. }) => match &unestablished[..] {
            [first, ..] => format!("rejected: {}", first.detail),
            [] => "rejected".to_owned(),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_byte_is_read_where_the_view_is_known_to_reach_it() {
    // The caller owes the bound.
    assert_eq!(
        verdict(
            "machine first(path: &[u8]) -> u8 requires path.len > 0 { path[0] }
machine build_entry(path: &[u8]) { transition path.len > 0 { true -> use(first(path)) false -> none() } }
machine use(byte: u8) { }
machine none() { }"
        ),
        "accepted"
    );
    // A guard over the view's own length, with the read in the arm it
    // selects.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) { transition path.len > 0 { true -> use(path[0]) false -> none() } }
machine use(byte: u8) { }
machine none() { }"
        ),
        "accepted"
    );
    // An index chosen at run time, and two reads of one view.
    assert_eq!(
        verdict(
            "machine at(path: &[u8], index: u64) -> u8 requires index < path.len { path[index] }
machine both(path: &[u8]) -> u8 requires path.len > 1 { path[0] ^ path[1] }
machine build_entry(path: &[u8]) { transition path.len > 1 { true -> use(at(path, 1), both(path)) false -> none() } }
machine use(one: u8, two: u8) { }
machine none() { }"
        ),
        "accepted"
    );
    // The same view under another name.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) requires path.len > 0 { let view: &[u8] = path; use(view[0]); }
machine use(byte: u8) { }"
        ),
        "accepted"
    );
}

#[test]
fn a_byte_past_what_the_view_is_known_to_hold_is_rejected() {
    // Nothing says the view holds a byte.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) { use(path[0]); }
machine use(byte: u8) { }"
        ),
        "rejected"
    );
    // The bound is one short.
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8]) requires path.len > 0 { use(path[1]); }
machine use(byte: u8) { }"
        ),
        "rejected"
    );
    // The bound is on another view: "An arbitrary equal-valued length,
    // other view ... supplies no length custody."
    assert_eq!(
        verdict(
            "machine build_entry(known: &[u8], path: &[u8]) requires known.len > 0 { use(path[0]); }
machine use(byte: u8) { }"
        ),
        "rejected"
    );
}

/// The lowered form of an accepted read is the measurement of the view and
/// then the read that names it, with nothing between.
#[test]
fn a_byte_read_names_the_measurement_made_directly_before_it() {
    let accepted = match judge_build(
        "machine build_entry(path: &[u8]) requires path.len > 0 { use(path[0]); }
machine use(byte: u8) { }",
    )
    .unwrap()
    {
        RouteVerdict::Accepted(accepted) => *accepted,
        other => panic!("{other:?}"),
    };
    let found = accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| block.operations.windows(2))
        .filter_map(|pair| match (&pair[0].kind, &pair[1].kind) {
            (
                OperationKind::ByteSequenceLength { source: measured },
                OperationKind::ByteSequenceRead { source, length, .. },
            ) => Some(
                measured.place == *source
                    && measured.path.is_empty()
                    && pair[0].result.scalar_ref().map(|result| result.id) == Some(*length),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(found, [true]);
}

/// The forms this route does not lower yet are refused by name, with no
/// verdict: an index of another width than the read's `u64`, a byte of a
/// view that is itself stored in a field, and a store through a view.
#[test]
fn the_forms_not_lowered_are_refused_by_name() {
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8], index: u8) requires path.len > 255 { use(path[index]); }
machine use(byte: u8) { }"
        ),
        "typing byte view index that is not a u64"
    );
    assert_eq!(
        verdict(
            "data Holder<'held> { path: &'held [u8]; }
machine build_entry<'held>(holder: &Holder<'held>) requires holder.path.len > 0 { use(holder.path[0]); }
machine use(byte: u8) { }"
        ),
        "expression byte of a view stored in a field"
    );
}

#[test]
fn a_subslice_keeps_the_selected_bytes_and_exact_extent() {
    let program = r#"
        machine next(path: &[u8]) -> u8 requires path.len > 1 {
            let suffix: &[u8] = path[1..];
            suffix[0]
        }
        machine build_entry(path: &[u8]) {
            transition path.len > 1 { true -> use(next(path)) false -> none() }
        }
        machine use(value: u8) {}
        machine none() {}
    "#;
    assert_eq!(verdict(program), "accepted");
}

#[test]
fn a_length_can_be_used_directly_as_a_call_argument() {
    assert_eq!(
        verdict("machine build_entry(path: &[u8]) { use(path.len); } machine use(length: u64) {}"),
        "accepted"
    );
}

#[test]
fn shared_subslices_compose_with_literals_calls_and_stored_views() {
    for program in [
        r#"machine first(path: &[u8]) -> u8 requires path.len > 0 { path[0] }
            machine build_entry() { use(first("AXYZ"[1..3])); }
            machine use(byte: u8) {}"#,
        r#"machine build_entry() { let bytes: &[u8] = "AXYZ"; let selected: &[u8] = bytes[1..3]; use(selected[1]); }
            machine use(byte: u8) {}"#,
        r#"machine build_entry(path: &[u8]) requires path.len > 2 {
                let selected: &[u8] = path[1..=2]; let suffix: &[u8] = selected[1..]; use(suffix[0]);
            } machine use(byte: u8) {}"#,
        r#"machine select<'a>(path: &'a [u8]) -> &'a [u8] requires path.len > 1 { path[1..] }
            machine build_entry(path: &[u8]) requires path.len > 1 { let selected: &[u8] = select(path); use(selected.len); }
            machine use(length: u64) {}"#,
        r#"data Holder<'a> { bytes: &'a [u8]; }
            machine build_entry<'a>(holder: &Holder<'a>) requires holder.bytes.len > 1 {
                let selected: &[u8] = holder.bytes[1..]; use(selected[0]);
            } machine use(byte: u8) {}"#,
        r#"machine first(path: &[u8]) -> u8 requires path.len > 0 { path[0] }
            machine build_entry() { let path: &[u8] = "AXYZ"; use(first(path[1..3])); }
            machine use(byte: u8) {}"#,
        r#"machine build_entry(path: &[u8]) { let empty: &[u8] = path[path.len..]; use(empty.len); }
            machine use(length: u64) {}"#,
    ] {
        assert_eq!(verdict(program), "accepted", "{program}");
    }
}

#[test]
fn subslice_bounds_must_belong_to_the_same_view_and_be_ordered() {
    for program in [
        "machine build_entry(path: &[u8]) requires path.len > 2 { let selected: &[u8] = path[2..1]; }",
        "machine build_entry(path: &[u8]) requires path.len > 0 { let selected: &[u8] = path[0..2]; }",
        "machine build_entry(other: &[u8], path: &[u8]) requires other.len > 2 { let selected: &[u8] = path[1..2]; }",
        "machine build_entry(path: &[u8]) requires path.len > 1 { let selected: &[u8] = path[1..1]; use(selected[0]); } machine use(byte: u8) {}",
    ] {
        assert_eq!(verdict(program), "rejected", "{program}");
    }
}

#[test]
fn a_subslice_cannot_claim_another_measurement_or_mutable_access() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    let typed = frontier_of(
        "machine product_entry() {}",
        r#"
        machine build_entry(path: &[u8]) requires path.len > 1 {
            let selected: &[u8] = path[1..]; use(selected[0]);
        } machine use(byte: u8) {}
    "#,
    );
    let entry = symbol(&typed, "build_entry");
    let elaborate = || {
        typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            roster(&[entry], &[Some(package())], source::DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap()
    };
    assert!(matches!(
        judge(Ok(elaborate())).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    for fault in 0..5 {
        let mut changed = elaborate();
        let machine = changed
            .module
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                    })
            })
            .unwrap();
        let slice = machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
            .unwrap();
        let OperationKind::ByteSequenceSubslice {
            source,
            start,
            length,
            ..
        } = &mut slice.kind
        else {
            unreachable!()
        };
        match fault {
            0 => *length = *start,
            1 => source.access = terminal_psi::StructuralAccess::MutableBorrow,
            2 => {
                let terminal_psi::OperationResult::Structural(result) = &mut slice.result else {
                    unreachable!()
                };
                result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
            }
            3 => {
                machine.structural_parameters[0].access =
                    terminal_psi::StructuralAccess::WriteOnlyBorrow;
            }
            4 => {
                let input = &mut machine.structural_parameters[0];
                input.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
                let declaration = changed
                    .module
                    .structural_types
                    .iter_mut()
                    .find(|row| row.id == input.structural_type)
                    .unwrap();
                declaration.shape = terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access: None },
                );
            }
            _ => unreachable!(),
        }
        assert!(
            !matches!(judge(Ok(changed)), Ok(RouteVerdict::Accepted(_))),
            "fault {fault} accepted"
        );
    }
}

#[test]
fn fixed_byte_arrays_can_supply_borrowed_view_parameters() {
    for access in ["&", "&mut"] {
        let program = format!(
            r#"
            machine count(bytes: {access} [u8]) -> u64 {{ bytes.len }}
            machine middle(bytes: {access} [u8]) -> u8 {{
                transition bytes.len > 1 {{ true -> bytes[1] false -> 0 }}
            }}
            machine build_entry() {{
                let mut bytes: [u8; 3] = [17, 42, 99];
                use(count({access} bytes[..]), middle({access} bytes[..]));
                use_byte(bytes[0]);
            }}
            machine use(extent: u64, middle: u8) {{}}
            machine use_byte(byte: u8) {{}}
        "#
        );
        assert_eq!(verdict(&program), "accepted", "{program}");
    }
}

#[test]
fn explicit_array_views_keep_exact_length_and_release_the_original_backing() {
    for access in ["&", "&mut"] {
        let program = format!(
            r#"
            machine demand(bytes: {access} [u8]) requires bytes.len == 3 {{}}
            machine build_entry() {{
                let mut bytes: [u8; 3] = [17, 42, 99];
                let view: {access} [u8] = {access} bytes[..];
                demand(view);
                use_byte(bytes[0]);
            }}
            machine use_byte(byte: u8) {{}}
        "#
        );
        assert_eq!(verdict(&program), "accepted", "{program}");
        let wrong_length = program.replace("bytes.len == 3", "bytes.len == 4");
        assert!(verdict(&wrong_length).starts_with("rejected"));
    }
}

#[test]
fn an_array_loan_keeps_its_element_type_and_requested_access() {
    assert_eq!(
        verdict(
            r#"machine count(bytes: &mut &mut [u8]) -> u64 { bytes.len }
            machine build_entry() {
                let mut bytes: [u8; 3] = [17, 42, 99]; use(count(&mut bytes));
            }
            machine use(extent: u64) {}"#
        ),
        "typing argument types"
    );
    assert_eq!(
        verdict(
            r#"
        machine count(bytes: &mut [u8]) -> u64 { bytes.len }
        machine build_entry() {
            let mut bytes: [u8; 0] = [];
            use(count(&mut bytes));
        }
        machine use(extent: u64) {}
    "#
        ),
        "typing argument types"
    );
    assert_eq!(
        verdict(
            r#"
        machine count(bytes: &[u8]) -> u64 { bytes.len }
        machine build_entry() {
            let bytes: [u8; 3] = [17, 42, 99];
            use(count(bytes));
        }
        machine use(extent: u64) {}
    "#
        ),
        "call argument for a borrowed formal that is not a borrow"
    );
    assert_eq!(
        verdict(
            r#"
        machine count(bytes: &mut [u8]) -> u64 { bytes.len }
        machine build_entry() {
            let mut bytes: [u8; 3] = [17, 42, 99];
            use(count(&bytes));
        }
        machine use(extent: u64) {}
    "#
        ),
        "rejected"
    );
    assert_eq!(
        verdict(
            r#"
        machine count(bytes: &mut [u8]) -> u64 { bytes.len }
        machine build_entry() {
            let mut words: [u16; 3] = [17, 42, 99];
            use(count(&mut words));
        }
        machine use(extent: u64) {}
    "#
        ),
        "typing argument types"
    );
}

#[test]
fn an_empty_byte_window_requires_existing_array_backing() {
    let mut accepted = match judge_build(
        "machine build_entry() { let bytes: [u8; 3] = [17, 42, 99]; use(&bytes[..]); } \
         machine use(bytes: &[u8]) {}",
    )
    .unwrap()
    {
        RouteVerdict::Accepted(accepted) => accepted,
        other => panic!("{other:?}"),
    };
    let types = &mut accepted.elaboration.module.structural_types;
    let array = types
        .iter()
        .find(|row| {
            matches!(
                row.shape,
                terminal_psi::StructuralTypeShape::FixedArray { .. }
            )
        })
        .unwrap()
        .id;
    let empty = [terminal_psi::StructuralPathSegment::FixedRange { start: 3, end: 3 }];
    assert_eq!(
        terminal_semantics::fixed_byte_storage_window(types.iter(), array, &empty)
            .unwrap()
            .length,
        0
    );
    let declaration = types.iter_mut().find(|row| row.id == array).unwrap();
    let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = &mut declaration.shape
    else {
        unreachable!()
    };
    *length = terminal_psi::ArrayExtent::Literal(0);
    assert!(terminal_semantics::fixed_byte_storage_window(types.iter(), array, &[]).is_none());
}

#[test]
fn mutable_byte_stores_preserve_the_descriptor_and_its_bound() {
    for program in [
        "machine build_entry(bytes: &mut [u8]) requires bytes.len > 0 { bytes[0] = 42; use(bytes[0], bytes.len); } machine use(value: u8, length: u64) {}",
        "machine prefix(bytes: &mut [u8]) -> u8 requires bytes.len > 1 { bytes[0] = 7; 42 } machine build_entry(bytes: &mut [u8]) requires bytes.len > 1 { bytes[1] = prefix(bytes); use(bytes[1]); } machine use(value: u8) {}",
    ] {
        match judge_build(program) {
            Ok(RouteVerdict::Accepted(_)) => {}
            Ok(RouteVerdict::Rejected {
                unproved,
                unestablished,
                conflicts,
                ..
            }) => {
                panic!(
                    "{program}: unproved={:?}; storage={:?}; conflicts={conflicts:?}",
                    unproved
                        .iter()
                        .map(|row| (&row.kind, &row.detail, &row.statement))
                        .collect::<Vec<_>>(),
                    unestablished
                        .iter()
                        .map(|row| &row.detail)
                        .collect::<Vec<_>>()
                );
            }
            other => panic!("{program}: {other:?}"),
        }
    }
}

#[test]
fn a_byte_store_needs_mutable_access_and_its_own_index_bound() {
    for program in [
        "machine build_entry(bytes: &mut [u8]) { bytes[0] = 42; }",
        "machine build_entry(bytes: &mut [u8]) requires bytes.len > 0 { bytes[1] = 42; }",
        "machine build_entry(other: &[u8], bytes: &mut [u8]) requires other.len > 0 { bytes[0] = 42; }",
    ] {
        assert_eq!(verdict(program), "rejected", "{program}");
    }
    assert_eq!(
        verdict("machine build_entry(bytes: &[u8]) requires bytes.len > 0 { bytes[0] = 42; }"),
        "statement store through a byte view without mutable access"
    );
    assert_eq!(
        verdict(
            "machine build_entry(bytes: & &mut [u8]) requires bytes.len > 0 { bytes[0] = 42; }"
        ),
        "statement store through shared descriptor storage"
    );
}

#[test]
fn a_byte_store_cannot_claim_another_measurement_or_access() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    let typed = frontier_of(
        "machine product_entry() {}",
        "machine build_entry(bytes: &mut [u8], other: &[u8]) requires bytes.len > 0 && other.len > 0 { let spare: [u8; 1] = [5]; bytes[0] = 42; }",
    );
    let entry = symbol(&typed, "build_entry");
    let elaborate = || {
        typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            roster(&[entry], &[Some(package())], source::DependencyScope::Build),
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap()
    };
    assert!(matches!(
        judge(Ok(elaborate())).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    for fault in 0..4 {
        let mut changed = elaborate();
        let array_type = changed
            .module
            .structural_types
            .iter()
            .find(|row| {
                matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::FixedArray { .. }
                )
            })
            .unwrap()
            .id;
        let machine = changed
            .module
            .machines
            .iter_mut()
            .find(|machine| {
                machine.operations().any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceWrite { .. })
                })
            })
            .unwrap();
        match fault {
            0 => {
                let other = machine.structural_parameters[1].place;
                let measurement = machine
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| {
                        matches!(operation.kind, OperationKind::ByteSequenceLength { .. })
                    })
                    .unwrap();
                let OperationKind::ByteSequenceLength { source } = &mut measurement.kind else {
                    unreachable!()
                };
                source.place = other;
            }
            1 => {
                machine.structural_parameters[0].access =
                    terminal_psi::StructuralAccess::SharedBorrow
            }
            2 => {
                let store = machine
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| {
                        matches!(operation.kind, OperationKind::ByteSequenceWrite { .. })
                    })
                    .unwrap();
                let OperationKind::ByteSequenceWrite { value, length, .. } = &mut store.kind else {
                    unreachable!()
                };
                *value = *length;
            }
            3 => {
                let array = machine
                    .structural_places
                    .iter()
                    .find(|place| {
                        matches!(place.kind,
                    semantic_vocabulary::StructuralPlaceKind::OperationResult { structural_type, .. }
                        if structural_type == array_type)
                    })
                    .unwrap()
                    .id;
                let measurement = machine
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| {
                        matches!(operation.kind, OperationKind::ByteSequenceLength { .. })
                    })
                    .unwrap();
                let OperationKind::ByteSequenceLength { source } = &mut measurement.kind else {
                    unreachable!()
                };
                source.place = array;
            }
            _ => unreachable!(),
        }
        assert!(
            !matches!(judge(Ok(changed)), Ok(RouteVerdict::Accepted(_))),
            "fault {fault} accepted"
        );
    }
}

#[test]
fn a_shared_reference_kept_across_a_byte_store_prevents_the_write() {
    let before = "machine build_entry(bytes: &mut [u8]) requires bytes.len > 0 { let observed: & &mut [u8] = &bytes; observe(observed); bytes[0] = 42; } machine observe(value: & &mut [u8]) {}";
    assert_eq!(verdict(before), "accepted");
    let across = "machine build_entry(bytes: &mut [u8]) requires bytes.len > 0 { let observed: & &mut [u8] = &bytes; bytes[0] = 42; observe(observed); } machine observe(value: & &mut [u8]) {}";
    let actual = verdict(across);
    assert!(actual.starts_with("rejected"), "{actual}");
}

#[test]
fn byte_store_array_backing_program_is_checked_without_changing_its_operations() {
    let program = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/views/byte_view_store/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    assert_eq!(verdict(&program), "accepted");
    let out_of_bounds = program.replace("put(&mut bytes[..], 1, 73)", "put(&mut bytes[..], 3, 73)");
    assert_ne!(out_of_bounds, program);
    assert_eq!(verdict(&out_of_bounds), "rejected");
}

#[test]
fn replacing_a_stored_view_does_not_preserve_its_previous_extent() {
    let program = r#"
        data Holder<'a> { bytes: &'a [u8]; }
        machine replace<'a>(holder: &mut Holder<'a>) { holder.bytes = ""; }
        machine build_entry<'a>(holder: &mut Holder<'a>) requires holder.bytes.len > 0 {
            replace(holder);
            let selected: &[u8] = holder.bytes[0..1];
        }
    "#;
    assert_eq!(verdict(program), "rejected");
}
