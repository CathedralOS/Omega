//! Shared projected actuals preserve both authored roots through nested calls.

use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

#[test]
fn nested_equality_borrows_distinct_projected_receiver_and_explicit_actual() {
    for (right_base, right_size, expected) in [(7, 11, true), (9, 11, false), (7, 13, false)] {
        let source = format!(
            r#"
            data Region {{ base: u64; size: u64; }}
            machine Region::equals(&self, other: &Region) -> bool {{
                self.base == other.base && self.size == other.size
            }}
            data Filter {{ leading: u64; range: Region; sibling: Region; }}
            machine Filter::equals(&self, other: &Filter) -> bool {{
                self.range.equals(&other.range)
            }}
            machine value() -> bool {{
                let left: Filter = Filter {{ leading: 1, range: Region {{ base: 7, size: 11 }}, sibling: Region {{ base: 99, size: 98 }} }};
                let right: Filter = Filter {{ leading: 2, range: Region {{ base: {right_base}, size: {right_size} }}, sibling: Region {{ base: 7, size: 11 }} }};
                left.equals(&right)
            }}
        "#
        );
        let checked = checked(&source);
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "value")
            .produce_artifact()
            .expect("nested equality retains both projected shared actuals");
        if expected {
            reject_projected_argument_substitutions(&checked);
        }
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
            )
            .expect("encoded nested equality executes without source"),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected))
        );
    }
}

fn reject_projected_argument_substitutions(checked: &checked_trees::CheckedTrees) {
    use checked_trees::{
        CheckedScalarComputationStructuralArgument, CheckedUnitStructuralArgumentSourcePlan,
    };
    let argument = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .find_map(|(handle, argument)| match argument {
            CheckedScalarComputationStructuralArgument::Place(argument)
                if !argument.path.is_empty()
                    && argument.source
                        == CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: 1,
                        } =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("explicit projected other parameter");
    for mutation in 0..4 {
        let mut forged = checked.clone();
        let CheckedScalarComputationStructuralArgument::Place(argument) = forged
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(argument)
        else {
            panic!("place argument");
        };
        match mutation {
            0 => {
                argument.path[0] =
                    checked_trees::CheckedUnitStructuralPathSegment::Field("sibling".into())
            }
            1 => {
                argument.source =
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
            }
            2 => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
            3 => argument.path.clear(),
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&forged, "value")
                .produce_artifact()
                .is_err(),
            "projected source/path/access substitution {mutation} must reject"
        );
    }
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Filter::equals")
        .expect("wrapper machine");
    let state = &checked.machine_states(machine)[0];
    let root = checked.state_parameters(state)[0].symbol;
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|borrow| {
            borrow.machine_symbol == machine.symbol && borrow.state_symbol == state.symbol
        })
        .expect("exact wrapper borrow state");
    let call = checked
        .facts
        .borrow
        .calls
        .span(borrow_state.calls)
        .unwrap()
        .first()
        .unwrap();
    for mutation in 0..3 {
        let mut forged = checked.clone();
        let rows = forged
            .facts
            .borrow
            .argument_accesses
            .span_mut(call.accesses)
            .unwrap();
        assert_eq!(
            rows.len(),
            1,
            "implicit receiver is not an explicit observation row"
        );
        match mutation {
            0 => rows[0].root_symbol = root,
            1 => rows[0].segments = arena::HandleSpan::empty(),
            2 => rows[0].kind = checked_trees::BorrowAccessKind::Mutable,
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&forged, "value")
                .produce_artifact()
                .is_err(),
            "captured source/path/access substitution {mutation} must reject"
        );
    }
}
