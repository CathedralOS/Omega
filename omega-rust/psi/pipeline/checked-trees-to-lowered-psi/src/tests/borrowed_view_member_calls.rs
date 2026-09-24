<<<<<<< HEAD
//! Borrowed-view member calls: reading `.len` or an element `self.view[i]` on a
//! `&'r [u8]` record field is a live-length observation on the field's view
//! descriptor — admissible in a scalar result, an entry `requires` clause, a
//! transition guard, and a guarding `&&` conjunct. The `&[u8]` param root
//! requirement only gates the whole-parameter case; a field-path read is
//! classified by the leaf's own byte-sequence carrier.
//!
//! A byte store `self.view[i] = v` composes through the same read shape on an
//! `&'r mut [u8]` field: the checked-plan mint gate enforces `&mut` access
//! before the carrier collapses to a bare `BorrowedView`, and the verifier's
//! dominating-observation equation ties a successor's fresh `.len` to the
//! guard's bound. A store through a shared `&'r` field still declines.
//!
//! Residual: a scalar contract `requires i < self.view.len` cannot be retained
//! — `ScalarTerm` has no byte-length term, so the clause has no proposition
//! spelling; authored-bound subslice results have no floor spelling either
//! (`-> T` states die at the result-signature wall and tails cannot carry the
//! `self.view.len >= k` premise).
||||||| cc52d33521
//! Borrowed-view member calls: reading `.len` on a `&'r [u8]` record field is
//! a live-length observation on the field's view descriptor — admissible in a
//! scalar result, an entry `requires` clause, and a transition guard. The
//! `&[u8]` param root requirement only gates the whole-parameter case; a
//! field-path read is classified by the leaf's own byte-sequence carrier.
=======
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
>>>>>>> origin/leaf/borrowed-view-element-read
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
<<<<<<< HEAD

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

const MV: &str = r#"
    data Mv<'r> { view: &'r mut [u8]; out: u8; }

    machine Mv::build<'a>(x: &'a mut [u8]) -> Mv<'a> {
        Mv { view: x, out: 0 }
    }
"#;

/// Checking plans a guarded store through an `&'r mut [u8]` field, but the
/// Terminal verifier refuses it: the store reaches it on a `BorrowedView`
/// carrier that records no access, so it cannot tell this field from a shared
/// `&'r [u8]` one (TASKS.md BORROWED-VIEW-CARRIER-ACCESS). Once the carrier
/// carries its access, this becomes `..._lowers_and_verifies` through `verify`.
#[test]
fn mutable_view_field_element_store_waits_for_carrier_access() {
    let checked = crate::front_end::checked_program(&format!(
        "{MV}
            machine Mv::set(&mut self, i: u64, v: u8) {{
                transition i < self.view.len {{
                    true -> hit(i, v)
                    _ -> done()
                }}
                state hit(&mut self, i: u64, v: u8) {{
                    self.view[i] = v;
                }}
                state done(&mut self) {{
                    self.out = 1;
                }}
            }}"
    ));
    let error = lower_machine(&checked, TerminalMachineSelection::Name("Mv::set"))
        .expect_err("a borrowed-view byte store has no verifiable write authority yet");
    assert!(
        matches!(error, crate::LoweringError::InvalidTerminalModule(_)),
        "the refusal is the verifier's, not the planner's: {error:?}"
    );
}

#[test]
fn mutable_view_field_element_store_unguarded_rejects() {
    rejected(
        &format!(
            "{MV}
            machine Mv::set(&mut self, i: u64, v: u8) {{
                self.view[i] = v;
            }}"
        ),
        "Mv::set",
    );
}

#[test]
fn shared_view_field_element_store_transition_rejects() {
    rejected(
        &format!(
            "{WV}
            machine Wv::set(&mut self, i: u64, v: u8) {{
                transition i < self.view.len {{
                    true -> hit(i, v)
                    _ -> done()
                }}
                state hit(&mut self, i: u64, v: u8) {{
                    self.view[i] = v;
                }}
                state done(&mut self) {{
                    self.out = 1;
                }}
            }}"
        ),
        "Wv::set",
    );
}

#[test]
fn mutable_view_field_subslice_result_unguarded_rejects() {
    rejected(
        &format!(
            "{MV}
            machine Mv::head(&self) -> &'r mut [u8] {{
                self.view[0..3]
            }}"
        ),
        "Mv::head",
    );
}

#[test]
fn mutable_view_field_subslice_field_bound_result_rejects() {
    rejected(
        &format!(
            "{MV}
            data Nv<'r> {{ view: &'r mut [u8]; n: u64; }}

            machine Nv::build<'a>(x: &'a mut [u8], n: u64) -> Nv<'a> {{
                Nv {{ view: x, n: n }}
            }}

            machine Nv::head(&self) -> &'r mut [u8] {{
                self.view[0..self.n]
            }}"
        ),
        "Nv::head",
    );
}

#[test]
fn mutable_view_field_subslice_local_binding_rejects() {
    rejected(
        &format!(
            "{MV}
            machine Mv::head(&mut self) {{
                transition self.view.len >= 3 {{
                    true -> emit()
                    _ -> done()
                }}
                state emit(&mut self) {{
                    let w: &'r mut [u8] = self.view[0..3];
                    self.out = 1;
                }}
                state done(&mut self) {{
                    self.out = 0;
                }}
            }}"
        ),
        "Mv::head",
    );
}

#[test]
fn mutable_view_field_subslice_shared_result_rejects() {
    rejected(
        &format!(
            "{MV}
            machine Mv::head(&self) -> &'r [u8] {{
                transition self.view.len >= 3 {{
                    true -> emit()
                    _ -> done()
                }}
                state emit(&self) -> &'r [u8] {{
                    self.view[0..3]
                }}
                state done(&self) -> &'r [u8] {{
                    self.view
                }}
            }}"
        ),
        "Mv::head",
    );
}

#[test]
fn mutable_view_field_subslice_mut_call_argument_rejects() {
    rejected(
        &format!(
            "{MV}
            machine Mv::sink(&mut self, w: &'r mut [u8]) {{
                self.out = 1;
            }}

            machine Mv::head(&mut self) {{
                transition self.view.len >= 3 {{
                    true -> emit()
                    _ -> done()
                }}
                state emit(&mut self) {{
                    self.sink(&mut self.view[0..3]);
                }}
                state done(&mut self) {{
                    self.out = 1;
                }}
            }}"
        ),
        "Mv::head",
    );
}

#[test]
fn shared_view_field_subslice_mutable_result_rejects() {
    rejected(
        &format!(
            "{WV}
            machine Wv::head(&self) -> &'r mut [u8] {{
                transition self.view.len >= 3 {{
                    true -> emit()
                    _ -> done()
                }}
                state emit(&self) -> &'r mut [u8] {{
                    self.view[0..3]
                }}
                state done(&self) -> &'r mut [u8] {{
                    self.view
                }}
            }}"
        ),
        "Wv::head",
    );
}
||||||| cc52d33521
=======

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
>>>>>>> origin/leaf/borrowed-view-element-read
