//! `view as &[T]` re-views the complete window of an existing shared view as
//! another element representation.
//!
//! byte_views.md, "Collection view establishment", `WholeRepresentation`:
//! source and target elements have total, padding-free representations, the
//! target elements exactly tile each source element, and the shared result
//! has `source_length * source_stride / target_stride` elements. A separate
//! obligation proves that count fits `u64`, and the region supplies no
//! mutable access. So a program may read an `&[i32]` as its bytes, learns
//! the byte view's exact length, and still owes the count bound; a target
//! that does not tile the source, a padded element, and a mutable re-view
//! are refused.

use super::*;

/// `accepted`, `rejected` with what the program could not establish, or
/// `not supported: ` and the refusal's text.
fn verdict(body: &str) -> String {
    match super::entryless::judged(body, false) {
        RouteVerdict::Accepted(_) => "accepted".to_owned(),
        RouteVerdict::Rejected { .. } => "rejected".to_owned(),
        RouteVerdict::FallBack(refusal) => format!("not supported: {}", refusal.detail),
        other => panic!("{other:?}"),
    }
}

/// The program is refused for `detail`, in whichever machine forms the view.
fn refused(body: &str, detail: &str) {
    let verdict = verdict(body);
    assert!(
        verdict.starts_with(&format!("not supported: {detail}")),
        "{verdict}"
    );
}

#[test]
fn a_shared_view_is_read_as_the_elements_that_tile_it() {
    // An `i32` holds four bytes, so sixteen words are sixty-four bytes.
    let bytes = |bound: u64| {
        format!(
            "machine bytes_of(words: &[i32]) requires words.len <= 16 {{
    let bytes: &[u8] = words as &[u8];
    take(bytes);
}}
machine take(bytes: &[u8]) requires bytes.len <= {bound} {{ }}"
        )
    };
    assert_eq!(verdict(&bytes(64)), "accepted");
    // The re-view passes straight to a slice parameter as well.
    assert_eq!(
        verdict(
            "machine bytes_of(words: &[i32]) requires words.len <= 16 { take(words as &[u8]); }
machine take(bytes: &[u8]) requires bytes.len <= 64 { }"
        ),
        "accepted"
    );
    // Sixty-three bytes would be a view shorter than its window.
    assert_eq!(verdict(&bytes(63)), "rejected");
    // A padding-free record is tiled by its leaves as well.
    assert_eq!(
        verdict(
            "data Pair { low: u16; high: u16; }
machine halves(pairs: &[Pair]) requires pairs.len <= 8 {
    let halves: &[u16] = pairs as &[u16];
    take(halves);
}
machine take(halves: &[u16]) requires halves.len <= 16 { }"
        ),
        "accepted"
    );
}

#[test]
fn the_converted_count_must_fit_u64() {
    let program = |requires: &str| {
        format!(
            "machine bytes_of(words: &[i32]) -> u64 {requires} {{
    let bytes: &[u8] = words as &[u8];
    bytes.len
}}"
        )
    };
    // A bounded word count has a byte count that fits.
    assert_eq!(verdict(&program("requires words.len <= 16")), "accepted");
    // Nothing bounds the word count, so four bytes per word may overflow.
    assert_eq!(verdict(&program("")), "rejected");
}

#[test]
fn a_re_view_that_does_not_tile_or_writes_is_refused() {
    const UNTILED: &str =
        "a whole-representation view whose target elements do not exactly tile its source elements";
    // One `u16` cannot hold a whole `u32`.
    refused(
        "machine widen(halves: &[u16]) requires halves.len <= 8 {
    let words: &[u32] = halves as &[u32];
}",
        UNTILED,
    );
    // Padding bytes have no valid byte value to read.
    refused(
        "data Padded { tag: u8; value: u32; }
machine bytes_of(rows: &[Padded]) requires rows.len <= 8 {
    let bytes: &[u8] = rows as &[u8];
}",
        UNTILED,
    );
    // A mutable re-view would also owe that every written element leaves
    // the source valid; nothing establishes that, so even bytes re-viewed
    // as bytes are refused when the view writes.
    refused(
        "machine scribble(bytes: &mut [u8]) {
    let same: &mut [u8] = bytes as &mut [u8];
}",
        "expression exclusive storage view without establishment and handback evidence",
    );
}

/// A window of a mutable byte parameter, handed to a shared parameter, reads
/// those bytes as the parameter's own measurement does (the standard
/// library's POSIX write path does this with `buffer[0..count]`).
#[test]
fn a_window_of_a_mutable_byte_parameter_is_lent_for_reading() {
    let program = |requires: &str| {
        format!(
            "machine send(bytes: &[u8]) -> u64 {{ bytes.len }}
machine write(buffer: &mut [u8], count: u64) -> u64 {requires} {{
    send(buffer[0..count])
}}"
        )
    };
    assert_eq!(
        verdict(&program("requires count <= buffer.len")),
        "accepted"
    );
    // Without the bound the window may end past the bytes it views.
    assert_eq!(verdict(&program("")), "rejected");
}

#[test]
fn a_borrowed_record_buffer_can_be_recast_as_a_whole() {
    let source = r#"
data Word [copy] { value: u64; }
data Receiver [copy] {
    prefix: u64;
    stat_buf: [u8; 8];
}

machine read_word(receiver: &Receiver) -> u64 {
    let word: &Word = &receiver.stat_buf as &Word;
    word.value
}

machine entry() crashes Trap {
    let receiver: Receiver = Receiver { prefix: 0, stat_buf: [42, 0, 0, 0, 0, 0, 0, 0] };
    let value: u64 = read_word(&receiver);
    transition value == 42 { true -> done() false -> fail() }
    machine done() {}
    machine fail() { crash Trap; }
}
"#;
    let judge_source = |program: &str| {
        let typed = super::boundary_calls::typed(program);
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            super::build_purpose::roster(&[entry], &[], source::DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap();
        judge(Ok(elaboration))
    };
    let accepted = match judge_source(source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        verdict => panic!("the field occupies the complete aligned region: {verdict:?}"),
    };
    accepted.publish_invocation().unwrap();

    // One borrowed byte does not grant access to its siblings.
    let single_byte = source.replace("&receiver.stat_buf as", "&receiver.stat_buf[0] as");
    // The root is still aligned to eight, but the buffer now begins at byte nine.
    let unaligned = source
        .replace("    prefix: u64;", "    prefix: u64;\n    tag: u8;")
        .replace("prefix: 0, stat_buf", "prefix: 0, tag: 0, stat_buf");
    // Stronger address alignment cannot make padding readable.
    let padded = source
        .replace(
            "data Receiver [copy]",
            "data Padded [copy] { tag: u8; value: u32; }\ndata Receiver [copy]",
        )
        .replace("stat_buf: [u8; 8]", "stat_buf: Padded")
        .replace("[42, 0, 0, 0, 0, 0, 0, 0]", "Padded { tag: 42, value: 0 }");
    for program in [single_byte, unaligned, padded] {
        let refused = judge_source(&program);
        assert!(
            format!("{refused:?}")
                .contains("reference formation changes its captured referent type"),
            "{refused:?}"
        );
    }
}

#[test]
fn representation_target_constraints_must_be_established() {
    let source = |predicate: &str| {
        format!(
            "data Target {predicate} {{ value: i32; }}
machine reinterpret(words: &[i32]) requires words.len <= 8 {{
    let values: &[Target] = words as &[Target];
    take(values);
}}
machine take(values: &[Target]) {{ }}"
        )
    };
    assert_eq!(verdict(&source("")), "accepted");
    let actual = verdict(&source("requires value > 0"));
    assert!(
        actual.starts_with(
            "not supported: an array view without plain element storage and unclaimed backing"
        ),
        "the source bits do not establish the target invariant: {actual}"
    );
}

#[test]
fn representation_byte_target_keeps_canonical_element_identity() {
    let source = "data ByteBox { value: u8; }
machine retain(value: ByteBox) -> u8 { value.value }
machine bytes_of(words: &[i32]) requires words.len <= 8 {
    let bytes: &[u8] = words as &[u8];
    take(bytes);
}
machine take(bytes: &[u8]) { }";
    let typed = super::boundary_calls::typed(source);
    let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) =
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            typed_trees_to_terminal_psi::Compilation {
                declaration_metadata: &[],
                entries: &[],
                subjects: &[None],
                purpose: source::DependencyScope::Product,
                selected: &[],
                calling: None,
                providers: None,
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program
    else {
        panic!("the original source must prepare its ordinary checking closure");
    };
    let mut changed = prepared.closure.expect("the original source elaborates");
    let RouteVerdict::Accepted(accepted) = judge(Ok(changed.clone())).unwrap() else {
        panic!("the canonical byte view must be accepted before the mutation");
    };
    let publication = accepted
        .publish_invocation()
        .expect("the original view publishes");
    terminal_codec::EvidenceSealValidator::new(
        publication.artifact.view().module(),
        &publication.seals.seals,
    )
    .validate_all()
    .expect("the original element owns its count evidence");
    let mut wrappers = changed.module.structural_types.iter().filter_map(|row| {
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &row.shape else {
            return None;
        };
        let [field] = fields.as_slice() else {
            return None;
        };
        let terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) =
            field.field_type
        else {
            return None;
        };
        (integer.bits() == Some(8) && integer.sign() == IntegerSign::Unsigned).then_some(row.id)
    });
    let wrapper = wrappers
        .next()
        .expect("the source retains its ByteBox declaration");
    assert!(
        wrappers.next().is_none(),
        "the wrapper declaration is unique"
    );
    let alter_element = |module: &mut terminal_psi::TerminalModule| {
        let mut formations = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishElementView {
                        region: terminal_psi::ElementViewRegion::WholeRepresentation { .. },
                        ..
                    }
                )
            });
        let formation = formations.next().expect("the original source has its view");
        assert!(
            formations.next().is_none(),
            "the source has exactly one view"
        );
        let terminal_psi::OperationKind::EstablishElementView { element, .. } = &mut formation.kind
        else {
            unreachable!()
        };
        *element = wrapper;
    };
    alter_element(&mut changed.module);
    let mut received = publication.artifact.view().module().clone();
    alter_element(&mut received);
    let changed_evidence =
        terminal_codec::EvidenceSealValidator::new(&received, &publication.seals.seals)
            .validate_all();
    let checked = judge(Ok(changed));
    assert!(
        matches!(
            changed_evidence,
            Err(terminal_codec::EvidenceSealValidationError::SealMismatch { .. })
        ),
        "the wrapper must not own the canonical byte element's evidence: {changed_evidence:?}"
    );
    match checked {
        Err(refusal) => {
            assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
            assert!(
                refusal
                    .detail
                    .contains("a byte view re-views elements that are not bytes"),
                "{}",
                refusal.detail
            );
        }
        Ok(RouteVerdict::Accepted(_)) => {
            panic!("a nominal byte wrapper must not authenticate the canonical u8 element")
        }
        Ok(other) => panic!("the altered element has a different verdict: {other:?}"),
    }
}
