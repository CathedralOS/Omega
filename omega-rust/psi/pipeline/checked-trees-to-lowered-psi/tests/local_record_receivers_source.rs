//! Actual local storage survives borrowed scalar-result helper calls.

use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/structural/local_record_receivers/main.omg"
));

#[test]
fn local_receiver_scalar_and_fresh_case_keep_argument_identity_and_once_only_effects() {
    let source = r#"
        boundary trait Sink { machine record(value: u64); }
        data Mode [copy] { case Narrow; case Wide; }
        data Region { base: u64; }
        machine Region::take(&self, value: u64, mode: Mode) -> u64 reaches Sink {
            Sink::record(self.base);
            value
        }
        data Main {}
        machine Main::main() reaches Sink {
            Sink::record(1);
            let region: Region = Region { base: 10 };
            let received: u64 = region.take(4, Mode::Wide);
            Sink::record(received);
            Sink::record(2);
        }
    "#;
    let payload = source
        .replace("case Wide;", "case Wide(payload: u64);")
        .replace("mode: Mode)", "mode: Mode, trailing: u64)")
        .replace("data Main {}", "machine stamp(value: u64) -> u64 reaches Sink { Sink::record(value); value } data Main {}")
        .replace("region.take(4, Mode::Wide)", "region.take(stamp(4), Mode::Wide { payload: stamp(3) }, stamp(5))");
    for (source, expected) in [
        (source.to_owned(), vec![1, 10, 4, 2]),
        (source.replace("Mode [copy]", "Mode"), vec![1, 10, 4, 2]),
        (payload, vec![1, 4, 3, 5, 10, 4, 2]),
    ] {
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
            .produce_artifact()
            .expect("mixed call establishes its fresh case before borrowing the local receiver");
        let owner = checked
            .data_definitions()
            .iter()
            .find(|owner| owner.name.as_str() == "Mode")
            .unwrap();
        let other_case = checked
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                checked_trees::data::DataMember::Variant(case)
                    if case.name.as_str() == "Narrow" =>
                {
                    Some(case.symbol)
                }
                _ => None,
            })
            .unwrap();
        let mut changed = checked.clone();
        let subject_handle = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .iter()
            .find_map(|(handle, argument)| match argument {
                checked_trees::CheckedScalarComputationStructuralArgument::Case(_) => Some(handle),
                _ => None,
            })
            .expect("retained fresh case argument");
        let checked_trees::CheckedScalarComputationStructuralArgument::Case(subject) = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(subject_handle)
        else {
            panic!("retained fresh case argument");
        };
        subject.case = other_case;
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "Main::main")
                .produce_artifact()
                .is_err(),
            "a same-typed case cannot replace the authored argument"
        );
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        assert_eq!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishScalarCase { .. }
                ))
                .count(),
            1,
            "fresh case retains a real structural establishment"
        );
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
        let mut completed = false;
        for _ in 0..256 {
            let status = execution.resume(&mut fuel).unwrap();
            match status {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert_eq!(execution.resume(&mut fuel).unwrap(), status);
                    fuel.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
                    completed = true;
                    break;
                }
                other => panic!("mixed argument execution: {other:?}"),
            }
        }
        assert!(completed);
        let observed = execution
            .effects()
            .iter()
            .map(|effect| {
                let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect
                else {
                    panic!("only Sink recording effects");
                };
                let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
                    panic!("one u64 recording argument");
                };
                *value
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed,
            expected
                .into_iter()
                .map(semantic_vocabulary::IntegerValue::Unsigned)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn scalar_return_helper_reads_its_established_local_record_across_fuel() {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let helper = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "sum_local")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(helper.symbol)
            .is_some(),
        "local helper needs an ordered checked body; machines: {:?}; retained ordinary bodies: {:?}; scalar graphs: {:?}; structural scalar returns: {:?}",
        checked
            .machines()
            .iter()
            .map(|machine| (machine.symbol, machine.name.as_str()))
            .collect::<Vec<_>>(),
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>(),
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>(),
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>()
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "sum_local")
        .produce_artifact()
        .expect("scalar-result helper retains its local receiver storage");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::validate_module(&module).unwrap();
    for initial_fuel in 0..32 {
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(initial_fuel);
        let mut completed = false;
        for _ in 0..256 {
            match execution.resume(&mut fuel).unwrap() {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::Integer { value, .. },
                )) => {
                    assert_eq!(value, semantic_vocabulary::IntegerValue::Unsigned(42));
                    completed = true;
                    break;
                }
                TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
                other => panic!("local scalar helper execution: {other:?}"),
            }
        }
        assert!(
            completed,
            "local helper did not complete after fuel replenishment"
        );
    }
}

#[test]
fn nested_record_constructor_and_mutable_receiver_publish_verified_terminal() {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "observe")
        .produce_artifact()
        .expect("nested record observer retains its checked transitive body");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn owned_record_children_reuse_parameter_and_local_places() {
    for body in [
        "machine wrap(child: Inner) -> Outer { Outer { child: child } }",
        "machine wrap() -> Outer { let child: Inner = Inner { value: 7 }; Outer { child: child } }",
        "machine make_child() -> Inner { Inner { value: 7 } } machine wrap() -> Outer { let mut child: Inner = make_child(); Outer { child: child } }",
    ] {
        let source = format!("data Inner {{ value: u64; }} data Outer {{ child: Inner; }} {body}");
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "wrap")
            .produce_artifact()
            .unwrap_or_else(|error| panic!("{body}: {error:?}"));
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
        assert!(module.machines.iter().flat_map(|machine| &machine.blocks).flat_map(|block| &block.operations).any(|operation|
            matches!(&operation.kind, terminal_psi::OperationKind::EstablishRecord { fields } if fields.iter().any(|field| matches!(field.value, terminal_psi::RecordFieldValue::Structural(_))))));
    }
}
