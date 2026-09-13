//! Whole parameter returns use the same ordered body as preceding calls and stores.

use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn fixture(property: &str, parameters: &str, prefix: &str) -> CheckedTrees {
    let source = format!(
        "data Record {property} {{ first: u64; second: u64; third: u64; }}
         machine identity(value: u64) -> u64 {{ value }}
         machine stamp(output: &mut u64, value: u64) -> u64 {{ output = value; value }}
         machine retain(mask: u64, {parameters} record: Record) -> Record {{ {prefix} record }}"
    );
    typed_trees_to_checked_trees::lower_typed_trees(typed_source(&source)).unwrap()
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn discarded_scalar_invocation_precedes_whole_record_return() {
    for property in ["", "[copy]"] {
        let checked = fixture(property, "", "_ = identity(mask);");
        let source = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "retain")
            .unwrap();
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(source.symbol)
            .expect(
                "ordered body retains a structural result independently of preceding scalar calls",
            );
        assert!(plan.operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::ScalarCall { .. }
        )));
        assert!(plan.structural_result.is_some());
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "retain")
            .produce_artifact()
            .expect("discarded scalar result must not erase its call or record return");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        terminal_verifier::verify_module(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn effectful_discarded_call_writes_before_return_across_fuel() {
    for property in ["", "[copy]"] {
        let checked = fixture(
            property,
            "output: &mut u64,",
            "_ = stamp(&mut output, mask); let final_value: u64 = identity(73); _ = stamp(&mut output, final_value);",
        );
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "retain")
            .produce_artifact()
            .expect("effectful prefix shares structural completion");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let arguments: Vec<_> = entry
            .structural_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| TerminalStructuralValue {
                opaque_identity: 100 + index as u64,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            })
            .collect();
        assert_eq!(arguments.len(), 2);
        for allowance in 0..8 {
            let mut execution =
                TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    &proof_admission::AdmissionProfile::default(),
                    &[unsigned(41)],
                    &arguments,
                    &[TerminalStructuralPrimitiveValue {
                        argument_index: 0,
                        value: unsigned(9),
                    }],
                )
                .unwrap();
            let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(allowance);
            let mut completed = false;
            for _ in 0..64 {
                match execution.resume(&mut fuel).unwrap() {
                    TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
                        result,
                    )) => {
                        assert_eq!(result.value, arguments[1]);
                        assert!(result.claims.is_empty());
                        assert_eq!(
                            execution.structural_primitive_values()[0].value,
                            unsigned(73)
                        );
                        completed = true;
                        break;
                    }
                    other => panic!("unexpected record return status: {other:?}"),
                }
            }
            assert!(
                completed,
                "prefix and return must finish with replenished fuel"
            );
        }
    }
}

#[test]
fn source_replay_rejects_same_typed_return_parameter_substitution() {
    let mut checked = fixture("[copy]", "other: Record,", "_ = identity(mask);");
    let source = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retain")
        .unwrap()
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == source)
        .unwrap();
    plan.structural_result.as_mut().unwrap().source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 };
    assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "retain").is_err());
}

#[test]
fn consumed_affine_parameter_cannot_be_returned() {
    let source = "data Record { first: u64; second: u64; third: u64; }
        machine consume(record: Record) {}
        machine retain(record: Record) -> Record { consume(record); record }";
    let error = typed_trees_to_checked_trees::lower_typed_trees(typed_source(source)).unwrap_err();
    assert!(
        format!("{error:?}").contains("already transferred"),
        "{error:?}"
    );
}

#[test]
fn source_replay_requires_the_exact_affine_return_transfer() {
    let original = fixture("", "", "_ = identity(mask);");
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retain")
        .unwrap()
        .symbol;
    let handle = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.machine_symbol == machine
                && event.kind == language_semantics::PermissionEventKind::Transfer
                && event.source
                    == language_semantics::PermissionEventSource::Statement { statement_index: 1 })
            .then_some(handle)
        })
        .expect("authored affine return retains its statement transfer");
    for mutation in 0..4 {
        let mut changed = original.clone();
        let permissions = &mut changed.facts.flow.ownership.permissions;
        match mutation {
            0 => permissions.get_mut(handle).machine_symbol = Default::default(),
            1 => {
                permissions.get_mut(handle).multiplicity =
                    language_semantics::Multiplicity::Unrestricted
            }
            2 => permissions.get_mut(handle).obligation_live = true,
            _ => {
                permissions.append(permissions.get(handle).clone());
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "retain").is_err(),
            "changed return transfer {mutation}"
        );
    }
}
