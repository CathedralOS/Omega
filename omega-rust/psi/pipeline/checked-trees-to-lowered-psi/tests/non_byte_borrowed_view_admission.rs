//! TERMINAL-SLICE-VIEW-VOCABULARY: regression pins for the borrowed-view
//! admission generalized at `1739bf1441a`, which landed without crate-level
//! tests. That change stopped the Unit graph's borrowed-parameter admission
//! from reading the element type -- it published `BorrowedSliceView` for a
//! non-byte `Slice` where it had returned `Unit graph borrowed slice is not
//! bytes` -- and peeled re-borrow wrappers off edge arguments.
//!
//! The site is reached only when a borrowed view parameter is FORWARDED to a
//! successor state; an empty callee body never gets there, which is why these
//! use the forwarding shape. Two things are worth holding still: the element
//! refusal is gone, and the byte shape now lowers end to end. The non-byte
//! shape does not, and its residual is named here so the next worker inherits
//! an address rather than a guess.

fn forwarding_program(element: &str) -> String {
    format!(
        r#"
data Worker {{ }}
machine Worker::drain(&mut self, buf: &mut [{element}], index: u64) {{
    transition {{
        _ -> probe(&mut buf, index)
    }}
    state probe(&mut self, buf: &mut [{element}], index: u64) {{
        transition {{
            _ -> done()
        }}
    }}
    state done(&mut self) {{ }}
}}
data Main {{ worker: Worker; items: [{element}; 3]; }}
machine Main::main(&mut self) {{
    self.worker.drain(&mut self.items, 0);
}}
"#
    )
}

fn outcome(element: &str, machine: &str) -> String {
    let checked = crate::front_end::checked_program_result(&forwarding_program(element))
        .expect("the borrowed-view program should reach checked trees");
    match checked_trees_to_lowered_psi::lower_machine(
        &checked,
        checked_trees_to_lowered_psi::TerminalMachineSelection::Name(machine),
    ) {
        Ok(_) => "lowered".to_owned(),
        Err(error) => format!("{error:?}"),
    }
}

#[test]
fn the_element_type_refusal_is_gone_from_the_forwarding_shape() {
    // The admission no longer reads the element type at all, so no part of a
    // non-byte program may be refused for it -- not the callee that owns the
    // parameter, and not the caller that lends the field.
    for machine in ["Main::main", "Worker::drain"] {
        let outcome = outcome("u64", machine);
        assert!(
            !outcome.contains("borrowed slice is not bytes"),
            "{machine} refuses a non-byte borrowed view for its element type: {outcome}"
        );
        assert!(
            !outcome.contains("is not a primitive or byte slice"),
            "{machine} refuses a non-byte borrowed referent for its shape: {outcome}"
        );
    }
}

#[test]
fn a_forwarded_byte_view_lowers_through_the_re_borrow_peel() {
    // `&mut buf` on the successor edge is a re-borrow of a parameter binding.
    // Before the peel this stopped at `Unit graph successor is not the
    // retained parameter binding` even for bytes; both machines now lower.
    for machine in ["Main::main", "Worker::drain"] {
        assert_eq!(
            outcome("u8", machine),
            "lowered",
            "{machine} should forward a re-borrowed byte view to its successor"
        );
    }
}

#[test]
fn the_non_byte_residual_is_the_successor_custody_verifier() {
    // What still separates a non-byte view from the byte one above, stated by
    // name. A forwarded `&mut` argument emits a fresh re-borrow place that the
    // successor frontier cannot name, so this is custody accounting and not
    // element views -- the admission and the argument presentation are both
    // past. When this stops being true the assertion fails and says so, which
    // is the point: it should not move silently.
    for machine in ["Main::main", "Worker::drain"] {
        let outcome = outcome("u64", machine);
        assert!(
            outcome.contains("InvalidStructuralSuccessorArgument"),
            "expected the non-byte forwarding shape to stop at successor custody, \
             got: {outcome}"
        );
    }
}
