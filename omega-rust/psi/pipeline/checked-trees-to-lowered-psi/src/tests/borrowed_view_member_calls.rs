//! Borrowed-view member calls: reading `.len` on a `&'r [u8]` record field is
//! a live-length observation on the field's view descriptor — admissible in a
//! scalar result, an entry `requires` clause, and a transition guard. The
//! `&[u8]` param root requirement only gates the whole-parameter case; a
//! field-path read is classified by the leaf's own byte-sequence carrier.
use crate::{TerminalMachineSelection, lower_machine};

fn verify(source: &str, machine: &'static str) {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine))
        .expect("borrowed-view field member call composes and lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("borrowed-view field member call verifies");
}

const WV: &str = r#"
    data Wv<'r> { view: &'r [u8]; out: u8; }

    machine Wv::build<'a>(x: &'a [u8]) -> Wv<'a> {
        Wv { view: x, out: 0 }
    }
"#;

#[test]
fn borrowed_view_field_length_result_lowers_and_verifies() {
    verify(
        &format!(
            "{WV}
            machine Wv::len(&self) -> u64 {{
                self.view.len
            }}"
        ),
        "Wv::len",
    );
}

#[test]
fn borrowed_view_field_length_entry_requirement_lowers_and_verifies() {
    verify(
        &format!(
            "{WV}
            machine Wv::use_len(&mut self, i: u64) requires i < self.view.len {{
                self.out = 7;
            }}"
        ),
        "Wv::use_len",
    );
}

#[test]
fn borrowed_view_field_length_guard_lowers_and_verifies() {
    verify(
        &format!(
            "{WV}
            machine Wv::guard_len {{
                state probe(&mut self, i: u64) {{
                    transition i < self.view.len {{
                        true -> bump()
                        _ -> probe(i)
                    }}
                }}
                state bump(&mut self) {{
                    self.out = 7;
                }}
            }}"
        ),
        "Wv::guard_len",
    );
}
