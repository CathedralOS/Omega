//! A call whose write frame is unknown retires no more than its declared
//! signature ceiling: the exact storage of its exclusive actuals and a
//! `&mut self` receiver. Unrelated live facts survive, and the ceiling never
//! narrows below what the declaration can reach -- an exclusive actual with
//! no representable storage origin keeps the conservative retirement of every
//! live fact (wiki/spec/language/dependent_values.md, "Mutation frames").
//!
//! The observed fact is a bounded byte field's live extent:
//! `self.out.bytes = "XXX"` establishes three live bytes, `self.out.bytes[2]`
//! reads inside them only while that evidence survives, and a call that
//! retires it turns the read into a bounds rejection.
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

const DEFINITIONS: &str = r#"
    domain [u8; 3]::Utf8 requires valid_utf8(self);
    data Slot { bytes: [u8; 3] in Utf8; }
    data Record { out: Slot; other: Slot; others: [Slot; 2]; count: u64; }
    machine clear(slot: &mut Slot) { slot.bytes = ""; }
"#;

fn check(source: &str, extent_survives: bool) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => assert!(
            extent_survives,
            "the call must retire the read extent:\n{source}"
        ),
        Err(diagnostics) => {
            assert!(!extent_survives, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(
                        "cannot prove index `2` is within unknown slice length of `self.out.bytes`"
                    )),
                "expected the retired-extent bounds rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

/// A builtin function has no state parameters and no body summary, so its
/// frame resolves to nothing; its ceiling is empty because it takes scalar
/// values and owns no caller storage.
#[test]
fn builtin_call_keeps_unrelated_live_facts() {
    for builtin in ["min", "max"] {
        let source = format!(
            r#"{DEFINITIONS}
            machine Record::probe(&mut self) {{
                self.out.bytes = "XXX";
                let smaller: u64 = {builtin}(self.count, 4);
                self.count = smaller;
                let observed: u8 = self.out.bytes[2];
            }}
        "#
        );
        check(&source, true);
    }
}

/// An exclusive actual with an exact storage origin retires only that
/// storage: `self.other` is written, `self.out` keeps its extent.
#[test]
fn exact_exclusive_actual_retires_only_its_storage() {
    for (target, extent_survives) in [("self.other", true), ("self.out", false)] {
        let source = format!(
            r#"{DEFINITIONS}
            machine Record::probe(&mut self) {{
                self.out.bytes = "XXX";
                clear(&mut {target});
                let observed: u8 = self.out.bytes[2];
            }}
        "#
        );
        check(&source, extent_survives);
    }
}

/// A reference local bound from a reference-returning call with two
/// candidate origins has no single representable storage origin, so an
/// exclusive actual spelled through it leaves the ceiling unrepresentable:
/// the call still retires every live fact rather than surviving on a
/// guessed narrower frame.
#[test]
fn unrepresentable_exclusive_actual_keeps_the_conservative_retirement() {
    let source = format!(
        r#"{DEFINITIONS}
        machine pick(record: &mut Record, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut record.others[0]
                false -> &mut record.others[1]
            }}
        }}
        machine Record::probe(&mut self, spare: &mut Record, first: bool) {{
            self.out.bytes = "XXX";
            let chosen: &mut Slot = pick(spare, first);
            clear(chosen);
            let observed: u8 = self.out.bytes[2];
        }}
    "#
    );
    check(&source, false);
}

/// The retirement observed through scalar facts a call never hands back:
/// `count == 7` survives a call exactly where the call could not write.
fn check_scalar_survivals(source: &str, retired: &[&str], surviving: &[&str]) {
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(source)) else {
        panic!("the retired scalar facts must reject their contract calls:\n{source}");
    };
    let unproved = |place: &str| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!(
                "cannot prove requires contract for call needs_seven from probe: {place} == 7"
            ))
        })
    };
    for place in retired {
        assert!(
            unproved(place),
            "{place} must be retired: {diagnostics:#?}\n{source}"
        );
    }
    for place in surviving {
        assert!(
            !unproved(place),
            "{place} must survive: {diagnostics:#?}\n{source}"
        );
    }
}

const CANDIDATE_DEFINITIONS: &str = r#"
    domain [u8; 3]::Utf8 requires valid_utf8(self);
    data Slot { bytes: [u8; 3] in Utf8; count: u64; }
    data Record { out: Slot; others: [Slot; 2]; }
    machine needs_seven(count: u64) requires count == 7 { }
    machine clear(slot: &mut Slot) { slot.bytes = ""; slot.count = 0; }
"#;

/// A `&mut Slot` local bound from a checked reference result whose exits
/// return `&mut record.others[0]` or `&mut record.others[1]` names one of
/// those two candidates: the callee's `slot.count` write lands on exactly
/// both candidates' `count`, and the unrelated `mine.out` / `spare.out`
/// facts survive.
#[test]
fn two_candidate_reference_result_retires_exactly_its_candidates() {
    let source = format!(
        r#"{CANDIDATE_DEFINITIONS}
        machine pick(record: &mut Record, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut record.others[0]
                false -> &mut record.others[1]
            }}
        }}
        machine probe(mine: &mut Record, spare: &mut Record, first: bool) {{
            mine.out.count = 7;
            spare.out.count = 7;
            spare.others[0].count = 7;
            spare.others[1].count = 7;
            let chosen: &mut Slot = pick(spare, first);
            clear(chosen);
            needs_seven(mine.out.count);
            needs_seven(spare.out.count);
            needs_seven(spare.others[0].count);
            needs_seven(spare.others[1].count);
        }}
    "#
    );
    check_scalar_survivals(
        &source,
        &["spare.others[0].count", "spare.others[1].count"],
        &["mine.out.count", "spare.out.count"],
    );
}

/// A result routed through another state has no finite candidate set, so
/// the call keeps retiring every live fact.
#[test]
fn state_routed_reference_result_keeps_the_conservative_retirement() {
    let source = format!(
        r#"{CANDIDATE_DEFINITIONS}
        machine pick(record: &mut Record, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut record.others[0]
                false -> second(record)
            }}
            state second(record: &mut Record) -> &mut Slot {{ &mut record.others[1] }}
        }}
        machine probe(mine: &mut Record, spare: &mut Record, first: bool) {{
            mine.out.count = 7;
            spare.out.count = 7;
            let chosen: &mut Slot = pick(spare, first);
            clear(chosen);
            needs_seven(mine.out.count);
            needs_seven(spare.out.count);
        }}
    "#
    );
    check_scalar_survivals(&source, &["mine.out.count", "spare.out.count"], &[]);
}

/// A result selected by a runtime index into a fixed array already resolves
/// through the frame resolver's own origin to the element family
/// (`spare.others[*]`): both elements retire, nothing else does.
#[test]
fn runtime_indexed_reference_result_retires_the_element_family_only() {
    let source = format!(
        r#"{CANDIDATE_DEFINITIONS}
        machine pick(record: &mut Record, index: u64 [0..=1]) -> &mut Slot {{
            &mut record.others[index]
        }}
        machine probe(mine: &mut Record, spare: &mut Record, index: u64 [0..=1]) {{
            mine.out.count = 7;
            spare.out.count = 7;
            spare.others[0].count = 7;
            spare.others[1].count = 7;
            let chosen: &mut Slot = pick(spare, index);
            clear(chosen);
            needs_seven(mine.out.count);
            needs_seven(spare.out.count);
            needs_seven(spare.others[0].count);
            needs_seven(spare.others[1].count);
        }}
    "#
    );
    check_scalar_survivals(
        &source,
        &["spare.others[0].count", "spare.others[1].count"],
        &["mine.out.count", "spare.out.count"],
    );
}

/// A call through a `&mut` local bound from a two-candidate reference result
/// is valid only while every candidate storage origin carries the declared
/// field domain the callee's `self` parameter requires. The nested operand
/// call `builder.label()` writes only disjoint `self` storage, so it preserves
/// the candidate set; corrupting one candidate's field keeps the call
/// rejected.
#[test]
fn candidate_reference_result_call_requires_every_candidate_domain() {
    let source = format!(
        r#"{CANDIDATE_DEFINITIONS}
        data Builder {{ tag: u64; }}
        machine Builder::label(&mut self) -> u64 {{ self.tag }}
        machine Record::pick(&mut self, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut self.others[0]
                false -> &mut self.others[1]
            }}
        }}
        machine Slot::touch(&mut self, tag: u64) {{ self.count = tag; }}
        machine probe(builder: &mut Builder, spare: &mut Record, first: bool) {{
            let chosen: &mut Slot = spare.pick(first);
            chosen.touch(builder.label());
        }}
    "#
    );
    lower_typed_trees(parse_typed_trees(&source)).expect(
        "both candidates carry the declared domain and the disjoint operand preserves them",
    );
    let corrupted = format!(
        r#"{CANDIDATE_DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 3]) {{ bytes[0] = 255; }}
        machine Record::pick(&mut self, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut self.others[0]
                false -> &mut self.others[1]
            }}
        }}
        machine Slot::touch(&mut self) {{ }}
        machine probe(spare: &mut Record, first: bool) {{
            corrupt(&mut spare.others[1].bytes);
            let chosen: &mut Slot = spare.pick(first);
            chosen.touch();
        }}
    "#
    );
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(&corrupted)) else {
        panic!("a corrupted candidate must keep the call rejected");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove default-domain field requirement")),
        "the corrupted candidate must surface as an unproven declared-field requirement: {diagnostics:#?}"
    );
}

/// A write through a two-candidate reference local rewrites exactly one
/// candidate with a value the write checker proved in the field's declared
/// domain and leaves the other untouched, so both candidates keep their
/// `Utf8` coverage: the machine's return re-proves it for every room.
#[test]
fn write_through_two_candidate_alias_keeps_both_candidates_domains() {
    let source = format!(
        r#"{CANDIDATE_DEFINITIONS}
        machine Record::pick(&mut self, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut self.others[0]
                false -> &mut self.others[1]
            }}
        }}
        machine label() -> [u8; 3] in Utf8 {{ "abc" }}
        machine carve(spare: &mut Record, first: bool) {{
            let chosen: &mut Slot = spare.pick(first);
            chosen.count = 3;
            chosen.bytes = label();
        }}
    "#
    );
    lower_typed_trees(parse_typed_trees(&source))
        .expect("both candidates keep their declared domain across the alias write");
    let corrupted = format!(
        r#"{CANDIDATE_DEFINITIONS}
        machine Record::pick(&mut self, first: bool) -> &mut Slot {{
            transition first {{
                true -> &mut self.others[0]
                false -> &mut self.others[1]
            }}
        }}
        machine carve(spare: &mut Record, first: bool) {{
            let chosen: &mut Slot = spare.pick(first);
            let raw: &mut [u8; 3] = &mut chosen.bytes;
            raw[0] = 255;
        }}
    "#
    );
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(&corrupted)) else {
        panic!("a corrupting alias write through the candidates must not be handed back");
    };
    for candidate in ["spare.others[0].bytes", "spare.others[1].bytes"] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!(
                    "for return from carve at statement 3: {candidate} requires"
                ))),
            "{candidate} must stay retired: {diagnostics:#?}"
        );
    }
}
