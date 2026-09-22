//! Case tests observe the selected sum below its original borrowed owner.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check")
}

#[test]
fn fixed_index_case_observation_retains_the_selected_element() {
    let checked = checked(
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
fn nested_record_case_observation_retains_every_carrier() {
    let checked = checked(
        r#"
        data Color [copy] { case Red; case Blue; }
        data Swatch { color: Color; }
        data Palette { swatches: [Swatch; 3]; }
        data Canvas { palette: Palette; }
        machine Canvas::is_blue(&self) -> bool {
            self.palette.swatches[2].color in Color::Blue
        }
    "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Canvas::is_blue"),
    )
    .expect("record and fixed-index projections compose");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification accepts the nested case observation");
    let paths = lowered
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
        paths,
        vec![&vec![
            terminal_psi::StructuralPathSegment::Field("palette".into()),
            terminal_psi::StructuralPathSegment::Field("swatches".into()),
            terminal_psi::StructuralPathSegment::FixedIndex(2),
            terminal_psi::StructuralPathSegment::Field("color".into()),
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
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        assert!(
            typed_trees_to_checked_trees::lower_typed_trees(
                typed,
                &typed_trees_to_checked_trees::CheckingRequest::settled()
            )
            .is_err(),
            "{receiver}: {selected_case} cannot authorize reading the selected discriminator"
        );
    }
}
