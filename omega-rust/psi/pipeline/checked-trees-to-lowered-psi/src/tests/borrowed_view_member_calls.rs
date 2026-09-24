//! Borrowed-view member calls: reading `.len` or an element `self.view[i]` on a
//! `&'r [u8]` record field is a live-length observation on the field's view
//! descriptor — admissible in a scalar result, an entry `requires` clause, a
//! transition guard, and a guarding `&&` conjunct. The `&[u8]` param root
//! requirement only gates the whole-parameter case; a field-path read is
//! classified by the leaf's own byte-sequence carrier.
//!
//! Residual: a scalar contract `requires i < self.view.len` cannot be retained
//! — `ScalarTerm` has no byte-length term, so the clause has no proposition
//! spelling (and a byte store through a shared `&'r [u8]` view has none).
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

fn rejected(source: &str, machine: &'static str) {
    let Ok(checked) = crate::front_end::checked_program_result(source) else {
        return;
    };
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name(machine)).is_err(),
        "this borrowed-view member call must not lower"
    );
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

#[test]
fn borrowed_view_field_element_read_lowers_and_verifies() {
    verify(
        &format!(
            "{WV}
            boundary trait Output {{ machine flag(value: bool) reaches Output; }}
            machine Wv::probe(&self, i: u64) reaches Output {{
                Output::flag(i < self.view.len && self.view[i] == 7);
            }}"
        ),
        "Wv::probe",
    );
}

#[test]
fn borrowed_view_field_element_read_transition_lowers_and_verifies() {
    verify(
        &format!(
            "{WV}
            machine Wv::probe {{
                state probe(&mut self, i: u64) {{
                    transition i < self.view.len && self.view[i] == 7 {{
                        true -> hit()
                        _ -> miss()
                    }}
                }}
                state hit(&mut self) {{
                    self.out = 1;
                }}
                state miss(&mut self) {{
                    self.out = 2;
                }}
            }}"
        ),
        "Wv::probe",
    );
}

#[test]
fn borrowed_view_field_element_read_requires_term_rejects() {
    rejected(
        &format!(
            "{WV}
            machine Wv::read(&self, i: u64) -> u8 requires i < self.view.len {{
                self.view[i]
            }}"
        ),
        "Wv::read",
    );
}

#[test]
fn borrowed_view_field_element_read_mutable_requires_term_rejects() {
    rejected(
        &format!(
            "{WV}
            machine Wv::read_mut(&mut self, i: u64) -> u8 requires i < self.view.len {{
                self.view[i]
            }}"
        ),
        "Wv::read_mut",
    );
}

#[test]
fn borrowed_view_field_element_read_unguarded_rejects() {
    rejected(
        &format!(
            "{WV}
            machine Wv::read(&self, i: u64) -> u8 {{
                self.view[i]
            }}"
        ),
        "Wv::read",
    );
}

#[test]
fn borrowed_view_field_byte_store_rejects() {
    rejected(
        &format!(
            "{WV}
            machine Wv::poke(&mut self, i: u64) requires i < self.view.len {{
                self.view[i] = 1;
            }}"
        ),
        "Wv::poke",
    );
}

#[test]
fn bounded_owned_field_element_read_lowers_and_verifies() {
    verify(
        &format!(
            "{WV}
            boundary trait Output {{ machine flag(value: bool) reaches Output; }}
            domain [u8; 3]::Utf8 requires valid_utf8(self);
            data D {{ buf: [u8; 3] in Utf8; }}
            machine D::probe(&self, i: u64) reaches Output {{
                Output::flag(i < self.buf.len && self.buf[i] == 7);
            }}"
        ),
        "D::probe",
    );
}
