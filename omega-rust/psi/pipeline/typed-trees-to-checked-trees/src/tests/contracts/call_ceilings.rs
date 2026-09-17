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
