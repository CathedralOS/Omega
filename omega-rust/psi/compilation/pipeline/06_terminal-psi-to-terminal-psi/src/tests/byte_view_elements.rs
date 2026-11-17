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

#[test]
fn a_representation_view_count_must_measure_its_captured_descriptor() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    let typed = frontier_of(
        "machine product_entry() {}",
        r#"
machine build_entry(words: &[u32], peer: &[u32]) requires words.len <= 2 {
    let earlier: u64 = peer.len;
    let bytes: &[u8] = words as &[u8];
    consume(bytes);
}
machine consume(bytes: &[u8]) {}
"#,
    );
    let mut elaboration = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        roster(
            &[symbol(&typed, "build_entry")],
            &[Some(package())],
            source::DependencyScope::Build,
        ),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let machine = elaboration
        .module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::EstablishElementView {
                            region: terminal_psi::ElementViewRegion::WholeRepresentation { .. },
                            ..
                        }
                    )
                })
        })
        .unwrap();
    let earlier = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ElementViewLength { .. }))
        .unwrap()
        .result
        .expect_scalar()
        .id;
    let operation = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishElementView {
                    region: terminal_psi::ElementViewRegion::WholeRepresentation { .. },
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::EstablishElementView {
        region: terminal_psi::ElementViewRegion::WholeRepresentation { source_length, .. },
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    assert_ne!(*source_length, earlier);
    *source_length = earlier;
    let refusal = match judge(Ok(elaboration)) {
        Err(refusal) | Ok(RouteVerdict::FallBack(refusal)) => refusal,
        other => panic!("a substituted measurement must refuse: {other:?}"),
    };
    assert!(
        refusal
            .detail
            .contains("source length is not its view's measurement directly before it"),
        "{refusal:?}"
    );
}

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
fn image_dimensions_establish_the_representation_view_count() {
    let result = judge_build(
        r#"
machine build_entry(width: u32, height: u32, pixels: &[i32])
requires width > 0 && width <= 8192
requires height > 0 && height <= 8192
requires pixels.len == (width as u64) * (height as u64)
{
    let bytes: &[u8] = pixels as &[u8];
    consume(bytes);
}
machine consume(bytes: &[u8]) {}
"#,
    )
    .unwrap();
    assert!(matches!(result, RouteVerdict::Accepted(_)), "{result:#?}");
}

#[test]
fn representation_views_do_not_gain_writable_access_or_boolean_validity() {
    let mutable = judge_build(
        r#"
machine build_entry(words: &[u32]) requires words.len <= 2 {
    let bytes: &[u8] = words as &[u8];
    mutate(bytes);
}
machine mutate(bytes: &mut [u8]) {}
"#,
    );
    match mutable {
        Ok(RouteVerdict::Rejected { violations, .. }) => assert!(
            violations.iter().any(|violation| violation
                .statement
                .contains("caller lends only immutable access")),
            "{violations:?}"
        ),
        other => panic!("shared representation bytes cannot grant writable access: {other:?}"),
    }
    let boolean = verdict(
        r#"
machine build_entry(values: &[bool]) requires values.len <= 2 {
    let bytes: &[u8] = values as &[u8];
    consume(bytes);
}
machine consume(bytes: &[u8]) {}
"#,
    );
    assert!(
        boolean.contains("target elements do not exactly tile its source elements"),
        "{boolean}"
    );
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

#[test]
fn a_typed_slice_read_needs_a_bound_on_its_own_descriptor() {
    let source = |view: &str, index: u64| {
        format!(
            "machine build_entry(numbers: &[u32], peer: &[u32]) requires {view}.len > 0 {{ use(numbers[{index}]); }}
             machine use(number: u32) {{ }}"
        )
    };
    assert_eq!(verdict(&source("numbers", 0)), "accepted");
    assert_eq!(verdict(&source("numbers", 1)), "rejected");
    assert_eq!(verdict(&source("peer", 0)), "rejected");
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

/// Byte indices use the length carrier's `u64`; another width still refuses
/// before any indexing obligation is published.
#[test]
fn a_byte_view_index_of_another_width_is_refused_by_name() {
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8], index: u8) requires path.len > 255 { use(path[index]); }
machine use(byte: u8) { }"
        ),
        "typing byte view index that is not a u64 while reading the body of `build_entry`"
    );
}

/// A record keeps the descriptor's own extent and backing. A sibling view's
/// bound cannot authorize its bytes, even under the same enclosing record.
#[test]
fn stored_byte_reads_keep_their_field_extent_and_bounds() {
    // A literal's exact extent survives storing its descriptor. The sibling
    // literal has another extent and cannot justify reading past this field.
    let literal = r#"
data Holder<'held> { known: &'held [u8]; path: &'held [u8]; }
machine build_entry() {
    let holder: Holder = Holder { known: "AB", path: "%" };
    use(holder.path[0]);
}
machine use(byte: u8) {}
"#;
    assert_eq!(verdict(literal), "accepted");
    assert_eq!(verdict(&literal.replace("path[0]", "path[1]")), "rejected");
    assert_eq!(
        verdict(&literal.replace("path: \"%\"", "path: \"\"")),
        "rejected"
    );

    let source = r#"
data Holder<'held> { known: &'held [u8]; path: &'held [u8]; }
data Outer<'held> { holder: Holder<'held>; count: u32; }
machine build_entry<'held>(outer: &Outer<'held>)
requires outer.holder.path.len > 0
{ use(outer.holder.path[0]); }
machine use(byte: u8) {}
"#;
    assert_eq!(verdict(source), "accepted");
    for changed in [
        source.replace("requires outer.holder.path.len > 0", ""),
        source.replace(
            "requires outer.holder.path.len > 0",
            "requires outer.holder.known.len > 0",
        ),
        source.replace("path[0]", "path[1]"),
    ] {
        assert_eq!(verdict(&changed), "rejected", "{changed}");
    }
    let accepted = match judge_build(source).unwrap() {
        RouteVerdict::Accepted(accepted) => *accepted,
        other => panic!("{other:?}"),
    };
    let pairs = accepted
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| block.operations.windows(2))
        .filter_map(|pair| match (&pair[0].kind, &pair[1].kind) {
            (
                OperationKind::StructuralByteSequenceFieldLength {
                    source: measured,
                    path: measured_path,
                    field: measured_field,
                },
                OperationKind::StructuralByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    length,
                    ..
                },
            ) => Some(
                measured == source
                    && measured_path == path
                    && measured_field == field
                    && !path.is_empty()
                    && pair[0]
                        .result
                        .scalar_ref()
                        .is_some_and(|result| result.id == *length),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs, [true]);
    accepted.publish_invocation().unwrap();
}

/// Receiving an edited program must rejoin its exact field and length;
/// a valid bound on the original field cannot authenticate a sibling read.
#[test]
fn a_stored_byte_read_cannot_substitute_a_sibling_field() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    let typed = frontier_of(
        "machine product_entry() {}",
        r#"
data Holder<'held> { left: &'held [u8]; right: &'held [u8]; }
machine build_entry<'held>(holder: &Holder<'held>)
requires holder.left.len > 0 && holder.right.len > 0
{ use(holder.left[0], holder.right[0]); }
machine use(left: u8, right: u8) {}
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
    let mut changed = elaborate();
    let fields = changed
        .module
        .machines
        .iter()
        .flat_map(|machine| machine.operations())
        .filter_map(|operation| match operation.kind {
            OperationKind::StructuralByteSequenceFieldRead { field, .. } => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert_ne!(fields[0], fields[1]);
    let read = changed
        .module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralByteSequenceFieldRead { .. }
            )
        })
        .unwrap();
    let OperationKind::StructuralByteSequenceFieldRead { field, .. } = &mut read.kind else {
        unreachable!()
    };
    *field = fields[1];
    assert!(!matches!(judge(Ok(changed)), Ok(RouteVerdict::Accepted(_))));
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
fn mutable_windows_reborrow_the_same_backing_without_moving_the_parent() {
    let source = r#"
        machine write(bytes: &mut [u8]) requires bytes.len > 0 { bytes[0] = 7; }
        machine window(bytes: &mut [u8], end: u64)
            requires end > 0 && end <= bytes.len
        { write(&mut bytes[0..end]); }
        machine build_entry() {
            let mut bytes: [u8; 3] = [17, 42, 99];
            window(&mut bytes[..], 2);
            use_byte(bytes[0]);
        }
        machine use_byte(byte: u8) {}
    "#;
    assert_eq!(verdict(source), "accepted");
    let shared = source.replace("window(bytes: &mut [u8]", "window(bytes: &[u8]");
    let refusal = verdict(&shared);
    assert!(
        refusal.contains("a subslice without authenticated backing and access"),
        "{refusal}"
    );
    let unbounded = source.replace("end > 0 && end <= bytes.len", "end > 0");
    assert!(verdict(&unbounded).starts_with("rejected"));

    let direct = source.replace("window(&mut bytes[..], 2);", "write(&mut bytes[1..2]);");
    assert_eq!(verdict(&direct), "accepted");

    let shared = r#"
        machine read(bytes: &[u8]) -> u8 requires bytes.len > 0 { bytes[0] }
        machine window(bytes: &[u8]) -> u8 requires bytes.len > 0 {
            read(&bytes[0..1])
        }
        machine build_entry(bytes: &[u8]) requires bytes.len > 0 { use_byte(window(bytes)); }
        machine use_byte(byte: u8) {}
    "#;
    assert_eq!(verdict(shared), "accepted");

    let cohort = r#"
        machine build_entry(bytes: &mut [u8]) requires bytes.len > 0 {
            let selected: &[u8] = &bytes[0..1];
            use_byte(selected[0]);
            bytes[0] = 9;
        }
        machine use_byte(byte: u8) {}
    "#;
    assert_eq!(verdict(cohort), "accepted");
    let conflict = cohort.replace(
        "use_byte(selected[0]);\n            bytes[0] = 9;",
        "bytes[0] = 9;\n            use_byte(selected[0]);",
    );
    let rejected = judge_build(&conflict);
    let Ok(RouteVerdict::Rejected { conflicts, .. }) = rejected else {
        panic!("the parent write must conflict with its live shared child: {rejected:?}");
    };
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.conflict == crate::LoanConflict::ChangeWhileBorrowed),
        "{conflicts:?}"
    );
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
fn a_qualified_byte_range_keeps_its_typed_builtin_formation() {
    let program = r#"
        domain [u8]::Path requires no_nul(self);
        machine prefix_length(path: &[u8] in Path, position: u64) -> u64
        requires position <= path.len {
            let prefix: &[u8] = &path[0..position];
            prefix.len
        }
        machine build_entry() {
            let length: u64 = prefix_length("foo/bar" as +Path, 3);
        }
    "#;
    match judge_build(program).unwrap() {
        RouteVerdict::Accepted(accepted) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("qualified byte range must be accepted: {other:?}"),
    }

    let outside = program.replace(r#""foo/bar" as +Path, 3"#, r#""foo/bar" as +Path, 8"#);
    assert_eq!(verdict(&outside), "rejected");

    let authored = format!(
        r#"machine [..] alternative(path: &[u8] in Path, start: u64, end: u64) -> &[u8] {{
               "alternative"
           }}
           {program}"#
    );
    let actual = verdict(&authored);
    assert!(
        actual.contains("range of qualified bytes without its selected operand type"),
        "{actual}"
    );
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
fn an_exclusive_array_view_lends_shared_access_after_its_cohort_ends() {
    let source = |uses: &str| {
        format!(
            r#"
            machine demand(bytes: &[u8]) requires bytes.len == 3 {{}}
            machine use_byte(byte: u8) {{}}
            machine build_entry() {{
                let mut bytes: [u8; 3] = [17, 42, 99];
                let view: &[u8] = &bytes[..];
                {uses}
                use_byte(bytes[0]);
            }}
        "#
        )
    };
    for uses in [
        "demand(view); demand(&mut bytes[..]);",
        "demand(&bytes[..]); demand(view);",
        "demand(view); let writable: &mut [u8] = &mut bytes[..]; demand(writable);",
    ] {
        assert_eq!(verdict(&source(uses)), "accepted", "{uses}");
    }
    let rejected = judge_build(&source("demand(&mut bytes[..]); demand(view);")).unwrap();
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref conflicts, .. }
            if conflicts.iter().any(|row| row.conflict == crate::LoanConflict::ExclusiveAndShared)),
        "the written exclusive array formation overlaps the live shared view: {rejected:?}"
    );
    let immutable =
        source("demand(view); demand(&mut bytes[..]);").replace("let mut bytes", "let bytes");
    assert_eq!(
        verdict(&immutable),
        "expression mutable array view without exclusive backing access while reading the body of \
         `build_entry`"
    );
    let shared_source = judge_build(
        r#"
        machine demand(bytes: &mut [u8]) requires bytes.len == 3 {}
        machine build_entry() {
            let mut bytes: [u8; 3] = [17, 42, 99];
            demand(&bytes[..]);
        }
    "#,
    )
    .unwrap();
    assert!(
        matches!(shared_source, RouteVerdict::Rejected { ref violations, .. }
            if violations.iter().any(|row| row.message().contains(
                "the explicit array view lends only immutable access"))),
        "a shared formation cannot grant exclusive access: {shared_source:?}"
    );
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

/// `&entries[..]` of noncopy records forms the same whole-collection
/// element view a byte array does, so its length is the array's extent:
/// `require_four` holds for four entries and is refused for three.
#[test]
fn typed_whole_array_views_keep_exact_length_and_their_backing() {
    let program = r#"
        data Entry { value: i32; }
        machine require_four(length: u64) -> u64 requires length == 4 { length }
        machine extent(entries: &[Entry; 4]) -> u64 {
            let view: &[Entry] = &entries[..];
            require_four(view.len)
        }
        machine build_entry() {
            let entries: [Entry; 4] = [Entry { value: 7 }, Entry { value: 8 },
                Entry { value: 9 }, Entry { value: 10 }];
            use(extent(&entries));
            use(extent(&entries));
        }
        machine use(extent: u64) {}
    "#;
    assert_eq!(verdict(program), "accepted", "{program}");
    let short = program
        .replace("[Entry; 4]", "[Entry; 3]")
        .replace(", Entry { value: 10 }", "");
    assert!(
        verdict(&short).starts_with("rejected"),
        "a three-entry view cannot satisfy require_four: {short}"
    );
    let partial = program.replace("&entries[..]", "&entries[1..]");
    let refused = judge_build(&partial).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if !unproved.is_empty()
                && unproved.iter().all(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "the three-entry window cannot meet the four-entry contract: {refused:?}"
    );
    let partial_with_its_extent = partial
        .replace("require_four", "require_three")
        .replace("length == 4", "length == 3");
    assert_eq!(
        verdict(&partial_with_its_extent),
        "accepted",
        "{partial_with_its_extent}"
    );
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
        "typing argument types while reading the body of `build_entry`"
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
        "rejected"
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
        "call argument for a borrowed formal that is not a borrow while reading the body of \
         `build_entry`"
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
        "typing argument types while reading the body of `build_entry`"
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
        "statement store through a byte view without mutable access while reading the body of \
         `build_entry`"
    );
    assert_eq!(
        verdict(
            "machine build_entry(bytes: & &mut [u8]) requires bytes.len > 0 { bytes[0] = 42; }"
        ),
        "statement store through shared descriptor storage while reading the body of `build_entry`"
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

#[test]
fn writes_through_array_views_retire_old_backing_facts_not_saved_values() {
    for write in [
        "mutate(&mut bytes[..]);",
        "let view: &mut [u8] = &mut bytes[..]; view[0] = 0; count(view.len);",
    ] {
        let source = |observed| {
            format!(
                r#"
            machine mutate(bytes: &mut [u8]) requires bytes.len > 0 {{ bytes[0] = 0; }}
            machine demand(value: u8) requires value == 17 {{}}
            machine count(extent: u64) requires extent == 3 {{}}
            machine build_entry() {{
                let mut bytes: [u8; 3] = [17, 42, 99];
                let saved: u8 = bytes[0];
                {write}
                demand({observed});
            }}
        "#
            )
        };
        // The old scalar remains exactly what was read before mutation.
        let saved = source("saved");
        assert_eq!(verdict(&saved), "accepted", "{saved}");
        // A later read of changed storage cannot reuse that old fact.
        let changed = source("bytes[0]");
        let refused = judge_build(&changed).unwrap();
        let call_requirement = matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. })));
        assert!(
            call_requirement,
            "{write}: expected an unproved CallRequirement; verdict: {}",
            verdict(&changed)
        );
    }
}

#[test]
fn a_view_write_keeps_facts_about_sibling_fields() {
    let source = r#"
        data Packet { before: u8; bytes: [u8; 3]; after: u8; }
        machine mutate(bytes: &mut [u8]) requires bytes.len > 0 { bytes[0] = 0; }
        machine before(value: u8) requires value == 17 {}
        machine after(value: u8) requires value == 99 {}
        machine build_entry() {
            let mut packet: Packet = Packet { before: 17, bytes: [17, 42, 99], after: 99 };
            mutate(&mut packet.bytes[..]);
            before(packet.before);
            after(packet.after);
        }
    "#;
    assert_eq!(verdict(source), "accepted");
    let stale = source.replace("before(packet.before)", "before(packet.bytes[0])");
    assert!(
        verdict(&stale).starts_with("rejected"),
        "{}",
        verdict(&stale)
    );
}

#[test]
fn a_shared_array_view_cannot_observe_write_only_backing() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    let typed = frontier_of(
        "machine product_entry() {}",
        "machine build_entry(bytes: &mut [u8; 3]) { let view: &[u8] = &bytes[..]; }",
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
    let mut changed = elaborate();
    let machine = changed
        .module
        .machines
        .iter_mut()
        .find(|machine| {
            machine.operations().any(|operation| {
                matches!(operation.kind, OperationKind::EstablishElementView { .. })
            })
        })
        .unwrap();
    machine.structural_parameters[0].access = terminal_psi::StructuralAccess::WriteOnlyBorrow;
    let refused = judge(Ok(changed)).unwrap();
    let category = match &refused {
        RouteVerdict::Accepted(_) => "accepted".to_owned(),
        RouteVerdict::Incomplete(_) => "incomplete".to_owned(),
        RouteVerdict::FallBack(refusal) => format!("fallback: {}", refusal.detail),
        RouteVerdict::Rejected { conflicts, .. } => format!(
            "loan conflicts: {:?}",
            conflicts.iter().map(|row| row.conflict).collect::<Vec<_>>()
        ),
    };
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref conflicts, .. }
        if conflicts.iter().any(|row| row.conflict == crate::LoanConflict::AccessExceedsParent)),
        "expected the source loan authority check to refuse write-only observation: {category}"
    );
}

/// conformances.md, "Core equality acquisition": text is "compared by byte
/// content". The comparison is a Boolean like any other: a guard, an
/// operand of `&&`, an argument, a result.
#[test]
fn two_byte_views_are_compared_by_their_bytes() {
    for program in [
        "machine build_entry(path: &[u8]) { transition path == \"main.omg\" { true -> yes() false -> no() } }
machine yes() { }
machine no() { }",
        "machine build_entry(left: &[u8], right: &[u8], count: u64) { use(left == right && count == 2, left != \"x\"); }
machine use(same: bool, other: bool) { }",
        "machine same(left: &[u8], right: &[u8]) -> bool { left == right }
machine build_entry(path: &[u8]) { use(same(path, \"main.omg\")); }
machine use(same: bool) { }",
    ] {
        assert_eq!(verdict(program), "accepted", "{program}");
    }
}

/// The lowered form of a comparison is the left view's measurement, the
/// right view's, and the comparison naming both, with nothing between. A
/// comparison that names another view's measurement, an operand it does not
/// read whole and shared, or a result that is no plain `bool` is not
/// accepted.
#[test]
fn a_byte_comparison_names_each_views_own_measurement() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    let typed = frontier_of(
        "machine product_entry() {}",
        "machine build_entry(left: &[u8], right: &[u8]) { use(left == right); }
machine use(same: bool) { }",
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
    let compared = |elaboration: &typed_trees_to_terminal_psi::Elaboration| {
        elaboration
            .module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| block.operations.windows(3))
            .filter_map(|run| match (&run[0].kind, &run[1].kind, &run[2].kind) {
                (
                    OperationKind::ByteSequenceLength { source: first },
                    OperationKind::ByteSequenceLength { source: second },
                    OperationKind::ByteSequenceEqual { left, right },
                ) => Some(
                    first.place == left.source()
                        && second.place == right.source()
                        && first.place != second.place
                        && run[0].result.scalar_ref().map(|result| result.id)
                            == Some(left.length())
                        && run[1].result.scalar_ref().map(|result| result.id)
                            == Some(right.length()),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let lowered = elaborate();
    assert_eq!(compared(&lowered), [true]);
    assert!(matches!(
        judge(Ok(lowered)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
    for fault in 0..4 {
        let mut changed = elaborate();
        let operation = changed
            .module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceEqual { .. }))
            .unwrap();
        let OperationKind::ByteSequenceEqual { left, right } = &mut operation.kind else {
            unreachable!()
        };
        use terminal_psi::control_flow::ByteSequenceOperand;
        let (
            ByteSequenceOperand::Whole {
                source: left_view,
                length: left_length,
            },
            ByteSequenceOperand::Whole {
                source: right_view,
                length: right_length,
            },
        ) = (left, right)
        else {
            unreachable!()
        };
        match fault {
            // Each view under the other one's measurement.
            0 => std::mem::swap(left_length, right_length),
            // One view compared with itself under the other's length.
            1 => right_view.place = left_view.place,
            2 => left_view.access = terminal_psi::StructuralAccess::MutableBorrow,
            3 => *right_length = *left_length,
            _ => unreachable!(),
        }
        assert!(
            !matches!(judge(Ok(changed)), Ok(RouteVerdict::Accepted(_))),
            "fault {fault} accepted"
        );
    }
}

/// A stored descriptor is compared through its exact field. Other operand
/// kinds still require the ordinary scalar/type rules.
#[test]
fn byte_field_comparisons_use_the_existing_readable_descriptor() {
    assert_eq!(
        verdict(
            "data Holder<'held> { path: &'held [u8]; }
machine build_entry<'held>(holder: &Holder<'held>) { use(holder.path == \"main.omg\"); }
machine use(same: bool) { }"
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            "machine build_entry(path: &[u8], count: u64) { use(path == count); }
machine use(same: bool) { }"
        ),
        "expression operator on an operand that is not a plain or policy-qualified scalar while \
         reading the body of `build_entry`"
    );
}

/// A match whose arms name the window's length, or the endpoints the window
/// was formed from, still indexes the window's last byte after the join. The
/// arm that read `window.len` crosses as `length == count - 0`; the read after
/// the join measures the same window again as `count - start`. Only
/// transport through that arrival equality relates the two.
fn last_byte_through_a_match(range: &str, requires: &str, arms: &str) -> RouteVerdict {
    let source = format!(
        "machine last_byte(items: &[u8], count: u64, flag: bool) -> u8
requires {requires} && count <= items.len
{{
    let window: &[u8] = &items[{range}];
    let length: u64 = match flag {{ {arms} }};
    window[length - 1]
}}
machine build_entry(path: &[u8]) {{ transition path.len > 2 {{ true -> use(last_byte(path, 2, true)) false -> none() }} }}
machine use(byte: u8) {{ }}
machine none() {{ }}"
    );
    judge_build(&source).unwrap()
}

#[test]
fn a_match_of_a_window_length_and_its_endpoint_keeps_the_last_byte_in_range() {
    for (range, requires, arms) in [
        (
            "0..count",
            "count > 0",
            "true -> window.len, false -> count",
        ),
        (
            "0..count",
            "count > 0",
            "true -> window.len, false -> window.len",
        ),
        (
            "1..count",
            "count > 1",
            "true -> window.len, false -> count - 1",
        ),
    ] {
        let verdict = last_byte_through_a_match(range, requires, arms);
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{range} / {arms}: {verdict:?}"
        );
    }
}

#[test]
fn a_match_arm_naming_the_backing_length_leaves_the_last_byte_unproved() {
    for (range, requires, arms) in [
        (
            "0..count",
            "count > 0",
            "true -> window.len, false -> items.len",
        ),
        (
            "1..count",
            "count > 1",
            "true -> window.len, false -> count",
        ),
    ] {
        let RouteVerdict::Rejected { unproved, .. } =
            last_byte_through_a_match(range, requires, arms)
        else {
            panic!("{range} / {arms} is accepted");
        };
        assert!(
            unproved.iter().any(|row| {
                matches!(row.kind, ObligationKind::OperationSafety { .. })
                    && row.cause == UnprovedCause::NoDerivation
                    && row.statement.contains("machine `last_byte`")
            }),
            "{range} / {arms}: {unproved:?}"
        );
    }
}

#[test]
fn a_call_guarantee_keeps_the_length_of_the_same_byte_window() {
    // The call's result and a later measurement denote the same descriptor
    // extent. A lower bound on the result alone cannot establish the index.
    for (start, minimum, requirement) in [
        (0, 0, ""),
        (0, 0, "requires window.len > 0"),
        (1, 1, "requires window.len > 0"),
    ] {
        for (comparison, expected) in [("==", "accepted"), (">=", "rejected")] {
            let source = format!(
                r#"
machine measure(window: &[u8]) -> u64
ensures result {comparison} window.len
{requirement}
{{ window.len }}

machine build_entry(items: &[u8], count: u64) -> u8
requires count > {minimum} && count <= items.len
{{
    let window: &[u8] = &items[{start}..count];
    let length: u64 = measure(window);
    window[length - 1]
}}
"#
            );
            assert_eq!(verdict(&source), expected, "{source}");
        }
    }
}

#[test]
fn loop_arrivals_prove_the_selected_views_own_extent() {
    const SOURCE: &str = r#"
machine build_entry(bytes: &[u8], other: &[u8], position: u64)
requires position < bytes.len {
    let byte: u8 = scan(bytes, other, position);
}
machine scan(bytes: &[u8], other: &[u8], position: u64) -> u8
requires position < bytes.len {
    transition { _ -> read(bytes, other, position) }
    machine read(bytes: &[u8], other: &[u8], position: u64) -> u8
    requires position < bytes.len {
        transition position > 0 {
            true -> scan(bytes, other, position - 1)
            false -> bytes[position]
        }
    }
}
"#;
    assert_eq!(verdict(SOURCE), "accepted");
    let sibling = SOURCE.replacen(
        "scan(bytes, other, position)",
        "scan(other, bytes, position)",
        1,
    );
    let refused = judge_build(&sibling).expect("the sibling-view program reaches checking");
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. } | ObligationKind::ArrivalInvariant { .. }))),
        "the first view's bound cannot establish a bound on its sibling"
    );
}

#[test]
fn field_byte_comparisons_retain_their_own_measurement_and_access() {
    use super::build_purpose::{frontier_of, package, roster, symbol};
    use terminal_psi::control_flow::ByteSequenceOperand;
    let typed = frontier_of(
        "machine product_entry() {}",
        "data Holder<'held> { left: &'held [u8]; right: &'held [u8]; }
machine build_entry<'held>(holder: &Holder<'held>) { use(holder.left == holder.right); }
machine use(same: bool) { }",
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
    assert_eq!(verdict("data Token {}
data Holder<'held> { token: Token; path: &'held [u8]; }
machine take(token: Token) {}
machine build_entry<'held>(holder: Holder<'held>) { take(holder.token); use(holder.path == \"main.omg\"); }
machine use(same: bool) {}"), "accepted");
    for fault in 0..4 {
        let mut changed = elaborate();
        let operation = changed
            .module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceEqual { .. }))
            .unwrap();
        let OperationKind::ByteSequenceEqual { left, right } = &mut operation.kind else {
            unreachable!()
        };
        let (
            ByteSequenceOperand::Field {
                source: left_source,
                field: left_field,
                length: left_length,
            },
            ByteSequenceOperand::Field {
                field: right_field,
                length: right_length,
                ..
            },
        ) = (left, right)
        else {
            panic!("both operands retain their fields")
        };
        match fault {
            0 => std::mem::swap(left_field, right_field),
            1 => std::mem::swap(left_length, right_length),
            2 => left_source.access = terminal_psi::StructuralAccess::MutableBorrow,
            3 => *left_field = semantic_vocabulary::StructuralFieldId::new(9999).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            !matches!(judge(Ok(changed)), Ok(RouteVerdict::Accepted(_))),
            "fault {fault} accepted"
        );
    }
}

/// Adding a primitive byte domain is checked even when its result is unused.
/// Later destination facts cannot excuse a literal that fails introduction.
#[test]
fn a_qualified_byte_literal_still_proves_its_domain_at_introduction() {
    let program = r#"
        domain [u8]::Path requires no_nul(self);
        machine build_entry() {
            let other: &[u8] in Path = "trusted" as +Path;
            let path: &[u8] in Path = "foo/bar" as +Path;
        }
    "#;
    assert_eq!(verdict(program), "accepted");
    let invalid = program.replace("foo/bar", r"foo\x00bar");
    let rejected = judge_build(&invalid).unwrap();
    assert!(
        matches!(&rejected, RouteVerdict::Rejected { unestablished, .. }
        if unestablished.iter().any(|row| {
            matches!(row.at, Construct::Edge(_))
                && row.detail == "a byte transfer's source does not establish its primitive byte-domain qualifications"
        })),
        "the literal itself must establish no_nul: {rejected:?}"
    );
}

/// Both arrivals retain their own descriptor extent. A long arrival cannot
/// supply the short arrival's length, while both retain their byte domain.
#[test]
fn qualified_byte_arrivals_keep_each_backings_length_and_predicate() {
    let program = r#"
        domain [u8]::Path requires no_nul(self);
        machine require_path(path: &[u8] in Path, position: u64)
        requires position <= path.len { }
        machine build_entry(flag: bool) {
            let selected: &[u8] in Path = match flag {
                true -> "foo/bar" as +Path,
                _ -> "other" as +Path
            };
            require_path(selected, 3);
        }
    "#;
    match judge_build(program).unwrap() {
        RouteVerdict::Accepted(accepted) => {
            accepted.publish_invocation().unwrap();
        }
        other => panic!("both qualified arrivals have at least three bytes: {other:?}"),
    }
    let short = program.replace(r#""other" as +Path"#, r#""xy" as +Path"#);
    let rejected = judge_build(&short).unwrap();
    assert!(
        matches!(&rejected, RouteVerdict::Rejected {
            formations, violations, unproved, unestablished, conflicts, exceeded
        } if formations.is_empty() && violations.is_empty() && unestablished.is_empty()
            && conflicts.is_empty() && exceeded.is_empty() && !unproved.is_empty()
            && unproved.iter().all(|row| {
                matches!(row.kind, ObligationKind::CallRequirement { .. })
                    && row.statement.contains("for call require_path from build_entry")
                    && row.statement.contains("needs fact `position <= path.len`")
            })),
        "the short arrival must fail only its own length requirement: {rejected:?}"
    );
}

#[test]
fn whole_records_from_a_view_keep_their_index_bound_and_access() {
    let source = r#"
data Cell [copy] { before: u32; value: u32; after: u32; }
machine pick(cells: &[Cell], index: u64) -> u32 requires index < cells.len {
    let cell: Cell = cells[index];
    cell.value
}
machine entry() -> u32 {
    let cells: [Cell; 2] = [Cell { before: 11, value: 12, after: 13 }, Cell { before: 21, value: 22, after: 23 }];
    pick(&cells[..], 1u64)
}
"#;
    let check = |text: &str| {
        super::entryless::judged_typed_result(&super::boundary_calls::typed(text), false)
    };
    let verdict = check(source);
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "{verdict:?}"
    );
    let writing = source
        .replace("cells: &[Cell]", "cells: &mut [Cell]")
        .replace("let cells: [Cell; 2]", "let mut cells: [Cell; 2]")
        .replace("pick(&cells[..]", "pick(&mut cells[..]")
        .replace("let cell: Cell = cells[index];", "let view: &mut [Cell] = &mut cells[..];\n    view[index].value = 9u32;\n    let cell: Cell = view[index];");
    let verdict = check(&writing);
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "the indexed write uses the actual retained descriptor permission: {verdict:?}"
    );
    let bypass = writing.replace("view[index].value = 9u32;", "cells[index].value = 9u32;");
    let verdict = check(&bypass);
    assert!(
        matches!(verdict, Ok(RouteVerdict::Rejected { ref conflicts, .. })
        if conflicts.iter().any(|conflict| conflict.conflict == crate::LoanConflict::ChangeWhileBorrowed)),
        "the original owner cannot bypass its still-live view: {verdict:?}"
    );
    let missing = source.replace("index < cells.len", "index <= cells.len");
    let verdict = check(&missing);
    assert!(
        matches!(verdict, Ok(RouteVerdict::Rejected { ref unproved, .. })
        if unproved.iter().any(|failure| matches!(failure.kind, ObligationKind::OperationSafety { .. }))),
        "the record copy still owes its exact index bound: {verdict:?}"
    );
    let moving = source.replace("Cell [copy]", "Cell");
    let verdict = check(&moving);
    assert!(
        match &verdict {
            Err(crate::CompilationRefusal::Elaboration(Refusal { detail, .. }))
            | Ok(RouteVerdict::FallBack(Refusal { detail, .. })) =>
                detail.contains("copying a slice element requires a copyable element type"),
            _ => false,
        },
        "a shared view cannot give away its affine element: {verdict:?}"
    );
}
