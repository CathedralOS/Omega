//! Borrowed view parameters retain their exact source element declarations.
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

const RECORD_SLICE: &str = r#"
data Entry { value: i32 in Wrapping; }
machine sum(source: &[Entry], accumulator: i32 in Wrapping) -> i32
terminates by source -> Slice::Length;
{
    transition source.len > 0 {
        true -> sum(source[1..], accumulator + source[0].value)
        false -> (accumulator as i32)
    }
}
"#;

#[test]
fn a_record_slice_parameter_retains_its_element_declaration() {
    let nested = RECORD_SLICE
        .replace(
            "data Entry { value: i32 in Wrapping; }",
            "data Payload { value: i32 in Wrapping; } data Entry { tag: u64; payload: Payload; }",
        )
        .replace("source[0].value", "source[0].payload.value");
    for source in [RECORD_SLICE, nested.as_str()] {
        let checked = crate::front_end::checked_program(source);
        let _produced = terminal_production::TerminalProductionRequest::new(
            &checked,
            TerminalMachineSelection::Name("sum"),
        )
        .produce(
            terminal_production::TerminalProductionCustody::artifact_only(
                &mut terminal_production::TerminalProductionTimings::default(),
            ),
        )
        .expect("record slice accumulation must produce independently verified Terminal Psi");
    }
}

#[test]
fn substituted_record_slice_elements_and_fields_reject() {
    use checked_trees::{CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
    let original = crate::front_end::checked_program(RECORD_SLICE);
    for mutation in [
        "element",
        "duplicate",
        "field identity",
        "field type",
        "missing field",
    ] {
        let mut checked = original.clone();
        let flow = &mut checked.facts.flow;
        // Alter every retained copy. Agreement between producer catalogs must
        // not substitute for correspondence with the original declaration.
        for types in [
            &mut flow.terminal_scalar_graphs.structural_types,
            &mut flow.terminal_unit_effects.structural_types,
            &mut flow.terminal_structural_returns.structural_types,
            &mut flow.terminal_boundary_scalar_returns.structural_types,
            &mut flow.terminal_structural_scalar_returns.structural_types,
        ] {
            if types.is_empty() {
                continue;
            }
            if mutation == "element" {
                let Some(element) = types
                    .iter_mut()
                    .find_map(|declaration| match &mut declaration.shape {
                        CheckedUnitStructuralTypeShape::BorrowedSliceView {
                            element_type_identity,
                        } => Some(element_type_identity),
                        _ => None,
                    })
                else {
                    continue;
                };
                *element = "another element".to_owned();
            } else {
                let Some(position) = types.iter().position(|declaration| matches!(
                &declaration.shape,
                CheckedUnitStructuralTypeShape::Record { fields } if fields.len() == 1 && fields[0].identity == "value"
            )) else { continue; };
                if mutation == "duplicate" {
                    types.push(types[position].clone());
                } else {
                    let CheckedUnitStructuralTypeShape::Record { fields } =
                        &mut types[position].shape
                    else {
                        unreachable!()
                    };
                    match mutation {
                        "field identity" => fields[0].identity = "another field".to_owned(),
                        "field type" => {
                            fields[0].field_type = CheckedUnitStructuralFieldType::Scalar(
                                checked_trees::types::PrimitiveType::U64,
                            )
                        }
                        "missing field" => fields.clear(),
                        _ => unreachable!(),
                    }
                }
            }
        }
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("sum"),
        )
        .expect_err("forged element declarations must lose their source custody");
        if !matches!(mutation, "element" | "duplicate") {
            assert!(
                format!("{error:?}")
                    .contains("owned structural catalog differs from its typed declaration"),
                "{mutation}: {error:?}"
            );
        }
    }
}
