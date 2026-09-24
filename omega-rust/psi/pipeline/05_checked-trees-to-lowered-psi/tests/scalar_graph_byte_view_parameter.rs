//! A borrowed BYTE view is a scalar-graph parameter like any other view.
//!
//! The scalar graph's borrowed-parameter custody joined a `&[T]` referee to
//! its retained shape by requiring `BorrowedSliceView { element_type_identity }`
//! and then matching that identity's own catalog row against the referee's
//! element. A byte view carries no element identity -- the
//! `ByteSequence(BorrowedView)` carrier already fixes `u8` -- so every byte
//! slice was refused with `scalar graph slice view shape differs from its
//! source`, the exact mirror of the Unit graph's old byte-only admission.
//!
//! `providers/lifetime_boundary_requirement_dispatch_exit` is the corpus
//! case: its `boundary requirement CellSource::read<'a>(source: &'a [u8])`
//! lends `&self.bytes[0..41]`.

use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn program(element: &str) -> String {
    format!(
        r#"
machine read_len(source: &[{element}]) -> i32 {{
    transition {{ _ -> ((source.len as u16 in Wrapping) as i32) }}
}}
data Main {{ bytes: [{element}; 64]; out: i32; }}
machine Main::main(&mut self) {{
    self.out = read_len(&self.bytes[0..41]);
}}
"#
    )
}

fn outcome(element: &str, machine: &str) -> String {
    let checked = crate::front_end::checked_program(&program(element));
    match checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(machine),
    ) {
        Ok(_) => "lowered".to_owned(),
        Err(error) => format!("{error:?}"),
    }
}

#[test]
fn a_borrowed_view_parameter_reaches_the_scalar_graph() {
    // A byte view and a non-byte element view take the same scalar-graph
    // route: the verifier establishes a shared element-view input at entry
    // (cf7b8baa2b), so neither needs its own admission arm.
    for element in ["u8", "i32"] {
        assert_eq!(
            outcome(element, "read_len"),
            "lowered",
            "read_len should admit a borrowed [{element}] view parameter"
        );
    }
    assert_eq!(outcome("u8", "Main::main"), "lowered");
}

#[test]
fn a_non_byte_subslice_argument_is_a_checked_omission() {
    // The caller half differs only upstream of this stage: checking does not
    // yet admit `&self.bytes[0..41]` over an `[i32; 64]` field as a call's
    // structural argument, so the Unit closure has no body to lower.
    let outcome = outcome("i32", "Main::main");
    assert!(
        outcome.contains("structural arguments: parameter path"),
        "expected the checked subslice-argument omission, got: {outcome}"
    );
}

#[test]
fn the_byte_view_shape_refusal_is_gone() {
    // The negative half of the same fact: whatever else may stop a program,
    // it must not be this shape check, for either element type.
    for element in ["u8", "i32"] {
        for machine in ["read_len", "Main::main"] {
            let outcome = outcome(element, machine);
            assert!(
                !outcome.contains("slice view shape differs from its source"),
                "{element} {machine} still fails the view-shape check: {outcome}"
            );
        }
    }
}
