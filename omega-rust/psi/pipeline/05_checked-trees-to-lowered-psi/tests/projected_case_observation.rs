//! Case tests observe the selected sum below its original borrowed owner.

use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn fixed_index_case_observation_retains_the_selected_element() {
    let checked = crate::front_end::checked_program(
        r#"
        data Color [copy] { case Red; case Blue; }
        data Palette { colors: [Color; 3]; }
        machine Palette::is_blue(&self) -> bool {
            self.colors[1] in Color::Blue
        }
    "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Palette::is_blue"),
    )
    .expect("observe the indexed sum rather than the array or its first element");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification accepts the exact root, index and case");
    let observations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::StructuralCaseMembership { path, .. } => Some(path),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observations,
        vec![&vec![
            terminal_psi::StructuralPathSegment::Field("colors".into()),
            terminal_psi::StructuralPathSegment::FixedIndex(1),
        ]]
    );
}

#[test]
fn projected_case_observation_rejects_foreign_cases_and_write_only_access() {
    for (receiver, selected_case) in [("&self", "Other::Blue"), ("&write self", "Color::Blue")] {
        let source = format!(
            r#"
            data Color [copy] {{ case Red; case Blue; }}
            data Other [copy] {{ case Red; case Blue; }}
            data Palette {{ colors: [Color; 3]; }}
            machine Palette::is_blue({receiver}) -> bool {{
                self.colors[1] in {selected_case}
            }}
        "#
        );
        assert!(
            crate::front_end::checked_program_result(&source).is_err(),
            "{receiver}: {selected_case} cannot authorize reading the selected discriminator"
        );
    }
}
