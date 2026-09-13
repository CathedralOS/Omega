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
fn owned_record_calls_compose_without_ambiguous_body_catalogs() {
    for property in ["", "[copy]"] {
        for body in [
            "retain(record)",
            "let result: Record = retain(record); result",
            "_ = identity(mask); retain(record)",
            "let first: Record = retain(record); retain(first)",
        ] {
            for reverse in [false, true] {
                let retain = "machine retain(record: Record) -> Record { record }";
                let relay =
                    format!("machine relay(mask: u64, record: Record) -> Record {{ {body} }}");
                let machines = if reverse {
                    format!("{relay} {retain}")
                } else {
                    format!("{retain} {relay}")
                };
                let checked =
                    typed_trees_to_checked_trees::lower_typed_trees(typed_source(&format!(
                        "data Record {property} {{ first: u64; second: u64; third: u64; }}
                     machine identity(value: u64) -> u64 {{ value }} {machines}"
                    )))
                    .unwrap();
                let artifact =
                    terminal_production::TerminalProductionRequest::new(&checked, "relay")
                        .produce_artifact()
                        .unwrap_or_else(|error| {
                            panic!("{property} {body} reverse={reverse}: {error:?}")
                        });
                let artifact =
                    terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                        .unwrap();
                let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
                let entry = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == module.entry)
                    .unwrap();
                let input = TerminalStructuralValue {
                    opaque_identity: 73,
                    structural_type: entry.structural_parameters[0].structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                };
                let mut execution = TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
                    artifact.semantic_bytes(), artifact.proof_bytes(), &proof_admission::AdmissionProfile::default(),
                    &[unsigned(41)], std::slice::from_ref(&input), &[],
                ).unwrap();
                let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(0);
                let mut completed = false;
                for _ in 0..64 {
                    match execution.resume(&mut fuel).unwrap() {
                        TerminalExecutionStatus::SponsorExhausted(_) => fuel.replenish(1).unwrap(),
                        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
                            result,
                        )) => {
                            assert_eq!(result.value.structural_type, input.structural_type);
                            assert_eq!(result.value.path, input.path);
                            assert_eq!(result.value.qualifications, input.qualifications);
                            if property.is_empty() {
                                assert_eq!(result.value, input);
                            } else {
                                assert_ne!(result.value.opaque_identity, input.opaque_identity);
                            }
                            assert!(result.claims.is_empty());
                            completed = true;
                            break;
                        }
                        other => panic!("unexpected forwarded record result: {other:?}"),
                    }
                }
                assert!(
                    completed,
                    "forwarded ownership completes across fuel pauses"
                );
            }
        }
    }
}

#[test]
fn owned_record_call_replay_rejects_same_type_argument_and_access_substitution() {
    for property in ["", "[copy]"] {
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed_source(&format!(
            "data Record {property} {{ first: u64; second: u64; third: u64; }}
             machine retain(record: Record) -> Record {{ record }}
             machine relay(left: Record, right: Record) -> Record {{ retain(left) }}"
        )))
        .unwrap();
        let _ = terminal_production::TerminalProductionRequest::new(&checked, "relay")
            .produce_artifact()
            .unwrap();
        for mutation in 0..3 {
            let mut invalid = checked.clone();
            let operation = invalid
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter_mut()
                .flat_map(|machine| &mut machine.states)
                .flat_map(|state| &mut state.operations)
                .find(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::StructuralCall { .. }
                    )
                })
                .unwrap();
            let CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                coordinate,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    structural_arguments[0].source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: 1,
                        }
                }
                1 => {
                    structural_arguments[0].access =
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                }
                _ => coordinate.call_ordinal += 1,
            }
            assert!(
                terminal_production::TerminalProductionRequest::new(&invalid, "relay")
                    .produce_artifact()
                    .is_err(),
                "call mutation {mutation}"
            );
        }
    }
}

#[test]
fn structural_return_requires_remaining_affine_input_cleanup_evidence() {
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed_source(
        "data Record { first: u64; second: u64; third: u64; }
         machine combine(left: Record, right: Record) -> Record {
             Record { first: left.first, second: right.second, third: left.third }
         }",
    ))
    .unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "combine")
        .produce_artifact()
        .unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(module.machines.iter().flat_map(|machine| &machine.blocks).any(|block|
        matches!(&block.terminator, terminal_psi::Terminator::ReturnStructural { trivial_affine_discards, .. } if trivial_affine_discards.len() == 2)));
    let exits = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(handle, event)| {
            (event.source == language_semantics::PermissionEventSource::StateExit
                && event.kind == language_semantics::PermissionEventKind::AffineDrop)
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(exits.len(), 2);
    for exit in exits {
        let mut invalid = checked.clone();
        invalid
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(exit)
            .machine_symbol = Default::default();
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "combine")
                .produce_artifact()
                .is_err(),
            "missing source exit disposal must fail independent frontier verification"
        );
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
