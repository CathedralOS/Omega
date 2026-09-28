use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn reachable_state_write_borrow_custody_still_vetoes_the_route() {
    let live_veto: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    data Cell { filled: u8 }
    machine relay(bytes: &[u8], marker: u8, cell: &write Cell) reaches Output {
        Output::write(bytes, marker);
        transition { _ -> done(bytes) }
        state done(bytes: &[u8]) {
            Output::write(bytes, 1u8);
        }
    }
"#;
    let checked = crate::front_end::checked_program(live_veto);
    let error = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("relay"),
    )
    .expect_err("reachable-state custody veto still declines the route");
    assert!(format!("{error:?}").contains("custody"), "{error:?}");
}
