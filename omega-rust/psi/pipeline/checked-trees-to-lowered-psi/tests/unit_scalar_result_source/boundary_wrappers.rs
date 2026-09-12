use super::*;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};

fn source() -> String {
    SOURCE
        .replace("Host::measure(70)", "Scalar::measure()")
        .replace(
            "data Main {}",
            r#"
        data Scalar {}
        machine Scalar::measure() -> i32 reaches Host {
            let result: i32 = Host::measure(70);
            result
        }
        data Main {}
        "#,
        )
}

fn artifact(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("Unit closure retains scalar boundary body");
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = decode_module(&semantic).unwrap();
    let proof = decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let published = terminal_production::TerminalProductionRequest::new(checked, "Main::main")
        .produce_artifact()
        .expect("source-owned shared closure publishes");
    assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
    (semantic, evidence)
}

fn integer(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Signed,
            32,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Signed(value),
    }
}

#[derive(Default)]
struct Observe {
    arguments: Vec<Vec<TerminalScalarValue>>,
}

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("scalar boundaries require the result-bearing handler")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            result,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("expected boundary call")
        };
        assert!(structural_arguments.is_empty());
        self.arguments.push(arguments.clone());
        Ok(match result {
            terminal_psi::BoundaryMachineResult::Scalar(_) => {
                TerminalEffectResult::Scalar(arguments[0])
            }
            terminal_psi::BoundaryMachineResult::Unit => TerminalEffectResult::Unit,
            _ => panic!("no structural return in this scalar call lane"),
        })
    }
}

fn execute(artifact: &(Vec<u8>, Vec<u8>)) -> (TerminalExecutionStatus, Observe) {
    let module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameters = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 100 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        &parameters,
    )
    .unwrap();
    let mut observer = Observe::default();
    let status = execution
        .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
        .unwrap();
    (status, observer)
}

#[test]
fn scalar_wrapper_result_executes_and_publishes_with_exact_services() {
    let artifact = artifact(&checked_from_source(&source()));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(module.machines.len(), 2);
    assert_eq!(module.boundary_machines.len(), 2);
    assert_eq!(module.services.len(), 1);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn direct_boundary_return_composes_with_ordered_calls_and_locals() {
    for (body, expected) in [
        ("Host::measure(70)", &[70, 70][..]),
        ("let input: i32 = 70; Host::measure(input)", &[70, 70][..]),
        ("Host::finish(11); Host::measure(70)", &[11, 70, 70][..]),
        (
            "let first: i32 = Host::measure(11); Host::finish(first); Host::measure(70)",
            &[11, 11, 70, 70][..],
        ),
        ("Host::measure(identity(70))", &[70, 70][..]),
    ] {
        let source = format!(
            "machine identity(value: i32) -> i32 {{ value }}\n{}",
            source().replace(
                "let result: i32 = Host::measure(70);\n            result",
                body
            )
        );
        let checked = checked_from_source(&source);
        let artifact = artifact(&checked);
        let (status, observed) = execute(&artifact);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        let expected = expected
            .iter()
            .map(|value| vec![integer(*value)])
            .collect::<Vec<_>>();
        assert_eq!(observed.arguments, expected, "{body}");
    }
}

#[test]
fn direct_boundary_return_rejects_result_and_occurrence_substitution() {
    let source = source().replace(
        "let result: i32 = Host::measure(70);\n            result",
        "let prior: i32 = Host::measure(11); Host::measure(70)",
    );
    let original = checked_from_source(&source);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    artifact(&original);
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == target)
            .unwrap();
        let first = plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } => Some(*result),
                _ => None,
            })
            .unwrap();
        match mutation {
            0 => plan.scalar_result = Some(first),
            1 => plan.operations.insert(1, plan.operations[0].clone()),
            2 => {
                let earlier_site = match &plan.operations[0] {
                    CheckedUnitEffectOperationPlan::BoundaryScalarCall { source_site, .. } => {
                        *source_site
                    }
                    _ => unreachable!(),
                };
                let CheckedUnitEffectOperationPlan::BoundaryScalarCall { source_site, .. } =
                    &mut plan.operations[1]
                else {
                    unreachable!()
                };
                *source_site = earlier_site;
            }
            _ => {
                let CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } =
                    &mut plan.operations[1]
                else {
                    unreachable!()
                };
                result.binding_ordinal = first.binding_ordinal;
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation} must not substitute another same-typed result"
        );
    }
}

#[test]
fn scalar_wrapper_parameters_forward_through_named_and_unit_entries() {
    let source = source()
        .replace("Scalar::measure() ->", "Scalar::measure(value: i32) ->")
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let checked = checked_from_source(&source);
    let named = checked_trees_to_lowered_psi::lower_machine(&checked, "Scalar::measure")
        .expect("a boundary-returning body retains its scalar entry parameter");
    assert_eq!(named.semantic_module.machines[0].parameters.len(), 1);
    let artifact = artifact(&checked);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn scalar_wrapper_parameter_ranges_survive_call_proofs_and_publication() {
    let source = source()
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(value: i32 [1..=100]) ->",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let checked = checked_from_source(&source);
    let artifact = artifact(&checked);
    let module = decode_module(&artifact.0).unwrap();
    let wrapper = module
        .machines
        .iter()
        .find(|machine| !machine.parameters.is_empty())
        .unwrap();
    assert_eq!(wrapper.contract.requires.len(), 1);
    assert!(!matches!(
        wrapper.contract.requires[0],
        semantic_vocabulary::Proposition::Truth
    ));
    let call_obligation = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::Call {
                callee,
                requirement_obligations,
                ..
            } if *callee == wrapper.id => {
                assert_eq!(requirement_obligations.len(), 1);
                Some(requirement_obligations[0])
            }
            _ => None,
        })
        .unwrap();
    let mut missing_call_proof = decode_proof_bundle(&artifact.1).unwrap();
    let original_count = missing_call_proof.evidence.len();
    missing_call_proof
        .evidence
        .retain(|evidence| evidence.obligation != call_obligation);
    assert_eq!(missing_call_proof.evidence.len(), original_count - 1);
    assert!(
        terminal_verifier::verify_module(
            &module,
            &missing_call_proof,
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

fn normal_guarantee_source(body: &str) -> String {
    format!("machine identity(value: i32) -> i32 ensures result == value {{ value }}\n{}", source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32) -> i32\nrequires value >= 1\nensures result == value\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", body)
        .replace("Scalar::measure();", "Scalar::measure(70);"))
}

#[test]
fn ordered_scalar_completion_proves_normal_result_guarantees() {
    for body in [
        "Host::finish(11); value",
        "let observed: i32 = Host::measure(11); value",
        "Host::finish(11); identity(value)",
    ] {
        let source = normal_guarantee_source(body);
        let artifact = artifact(&checked_from_source(&source));
        let module = decode_module(&artifact.0).unwrap();
        let wrapper = module
            .machines
            .iter()
            .find(|machine| {
                !machine.contract.ensures.is_empty() && !machine.contract.requires.is_empty()
            })
            .expect("normal guarantee published");
        assert_eq!(wrapper.contract.ensures.len(), 1);
        let (status, observed) = execute(&artifact);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(observed.arguments, [vec![integer(11)], vec![integer(70)]]);
    }
}

fn boolean_guarantee_source(body: &str, input: bool) -> String {
    normal_guarantee_source(body)
        .replace("i32", "bool")
        .replace("requires value >= 1", "requires value == value")
        .replace("Scalar::measure(70)", &format!("Scalar::measure({input})"))
}

#[test]
fn ordered_boolean_completion_preserves_normal_result_guarantees() {
    for body in [
        "Host::finish(false); value",
        "Host::finish(false); identity(value)",
        "let observed: bool = Host::measure(false); value",
    ] {
        for input in [false, true] {
            let source = boolean_guarantee_source(body, input);
            let artifact = artifact(&checked_from_source(&source));
            let module = decode_module(&artifact.0).unwrap();
            assert!(
                module
                    .machines
                    .iter()
                    .any(|machine| !machine.contract.ensures.is_empty())
            );
            let (status, observed) = execute(&artifact);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_computed_boolean_result_proves_its_normal_guarantee() {
    for (body, guarantee) in [
        ("Host::finish(false); !value", "result == !value"),
        (
            "let observed: bool = Host::measure(false); !value",
            "!value == result",
        ),
    ] {
        for input in [false, true] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                &format!("ensures {guarantee}\nreaches Host"),
            );
            let artifact = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&artifact);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_call_produced_boolean_result_preserves_its_normal_guarantee() {
    for input in [false, true] {
        for body in [
            "let saved: bool = identity(!value); Host::finish(false); saved",
            "Host::finish(false); let saved: bool = identity(!value); saved",
            "let earlier: bool = !value; let saved: bool = identity(earlier); Host::finish(false); saved",
            "let first: bool = identity(value); Host::finish(false); let saved: bool = identity(!first); saved",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_boolean_call_computations_preserve_normal_guarantees() {
    for input in [false, true] {
        for body in [
            "Host::finish(false); identity(!identity(value))",
            "let saved: bool = identity(!identity(value)); Host::finish(false); saved",
            "Host::finish(false); !identity(value)",
            "Host::finish(false); identity(value) == false",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)],
                ],
                "{body}"
            );
        }
    }
}

#[test]
fn ordered_boolean_call_computations_preserve_nested_effect_order() {
    for input in [false, true] {
        let source = boolean_guarantee_source("Host::finish(false); echo(!echo(value))", input)
            .replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
        let source = format!(
            "machine echo(value: bool) -> bool ensures result == value\nreaches Host {{ Host::finish(value); value }}\n{source}"
        );
        let published = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [false, input, !input, !input].map(|value| vec![TerminalScalarValue::Boolean(value)])
        );
    }
}

#[test]
fn ordered_boolean_call_computations_reject_an_altered_operation() {
    let source = boolean_guarantee_source("Host::finish(false); !identity(value)", false).replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let operation = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanNot { .. }
            )
        })
        .expect("selected negation survives publication");
    operation.kind = terminal_psi::OperationKind::BooleanConstant { value: false };
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_boolean_call_computations_prove_branch_guarantees() {
    for input in [false, true] {
        for body in [
            "Host::finish(false); identity(false) || !identity(value)",
            "Host::finish(false); identity(!value) && identity(true)",
            "Host::finish(false); !(identity(value) || identity(false))",
            "Host::finish(false); identity(!(identity(value) && identity(true)))",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let checked = checked_from_source(&source);
            let published = artifact(&checked);
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ],
                "{body}"
            );
        }
    }
}

#[test]
fn ordered_call_produced_boolean_equations_compose_across_calls() {
    for bits in 0..16 {
        let inputs = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
        let source = nested_boolean_guarantee_source(
            "(value == spare) == (other == last)", "Host::finish(false);", inputs,
        ).replace(
            "Host::finish(false); (value == spare) == (other == last)",
            "let first: bool = equal(9u16, value, spare); Host::finish(false); let second: bool = equal(5u16, other, last); first == second",
        );
        let source = format!(
            "machine equal(marker: u16, left: bool, right: bool) -> bool ensures result == (left == right) {{ left == right }}\n{source}"
        );
        let published = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [
                vec![TerminalScalarValue::Boolean(false)],
                vec![TerminalScalarValue::Boolean(
                    (inputs[0] == inputs[1]) == (inputs[2] == inputs[3])
                )]
            ]
        );
    }
}

#[test]
fn ordered_call_produced_boolean_result_rejects_a_substituted_actual() {
    let source = boolean_guarantee_source(
        "let saved: bool = identity(!value); Host::finish(false); saved",
        true,
    )
    .replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let wrapper = module
        .machines
        .iter_mut()
        .find(|machine| {
            !machine.contract.ensures.is_empty()
                && machine.blocks.iter().any(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(operation.kind, terminal_psi::OperationKind::Call { .. })
                    })
                })
        })
        .unwrap();
    let input = wrapper.parameters[0].id;
    let arguments = wrapper
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::Call { arguments, .. } => Some(arguments),
            _ => None,
        })
        .unwrap();
    assert_ne!(
        arguments[0], input,
        "the call consumes the captured negation"
    );
    arguments[0] = input;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_saved_boolean_result_preserves_its_normal_guarantee() {
    for input in [false, true] {
        for body in [
            "let saved: bool = !value; Host::finish(false); saved",
            "Host::finish(false); let saved: bool = !value; saved",
            "let saved: bool = value; Host::finish(false); !saved",
            "let saved: bool = !value; Host::finish(false); let copied: bool = saved; copied",
            "let observed: bool = Host::measure(false); let saved: bool = !value; saved",
        ] {
            let source = boolean_guarantee_source(body, input).replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(!input)]
                ]
            );
        }
    }
}

#[test]
fn ordered_saved_boolean_computations_compose_across_boundary_effects() {
    for bits in 0..16 {
        let inputs = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
        let source = nested_boolean_guarantee_source(
            "(value == spare) == (other == last)", "Host::finish(false);", inputs,
        ).replace(
            "Host::finish(false); (value == spare) == (other == last)",
            "let first: bool = value == spare; Host::finish(false); let second: bool = other == last; first == second",
        );
        let published = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [
                vec![TerminalScalarValue::Boolean(false)],
                vec![TerminalScalarValue::Boolean(
                    (inputs[0] == inputs[1]) == (inputs[2] == inputs[3])
                )]
            ]
        );
    }
}

#[test]
fn ordered_saved_boolean_return_rejects_a_substituted_terminal_value() {
    let source =
        boolean_guarantee_source("let saved: bool = !value; Host::finish(false); saved", true)
            .replace(
                "ensures result == value\nreaches Host",
                "ensures result == !value\nreaches Host",
            );
    let published = artifact(&checked_from_source(&source));
    let mut module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let wrapper = module
        .machines
        .iter_mut()
        .find(|machine| !machine.contract.ensures.is_empty())
        .unwrap();
    let input = wrapper.parameters[0].id;
    let returned = wrapper
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::Return { value, .. } => Some(value),
            _ => None,
        })
        .expect("saved scalar return");
    assert_ne!(
        *returned, input,
        "execution returns the computed value, not an input replay"
    );
    *returned = input;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_nested_boolean_result_proves_its_normal_guarantee() {
    for bits in 0..16 {
        let inputs = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
        let expected = (inputs[0] == inputs[1]) == (inputs[2] == inputs[3]);
        for (expression, before, expected) in [
            (
                "(value == spare) == (other == last)",
                "Host::finish(false);",
                expected,
            ),
            (
                "!((value == spare) == (other == last))",
                "let observed: bool = Host::measure(false);",
                !expected,
            ),
        ] {
            let source = nested_boolean_guarantee_source(expression, before, inputs);
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(expected)]
                ]
            );
        }
    }
}

fn nested_boolean_guarantee_source(expression: &str, before: &str, inputs: [bool; 4]) -> String {
    let [value, spare, other, last] = inputs;
    boolean_guarantee_source(&format!("{before} {expression}"), value)
        .replace(
            "Scalar::measure(value: bool)",
            "Scalar::measure(marker: u16, spare: bool, value: bool, other: bool, last: bool)",
        )
        .replace(
            &format!("Scalar::measure({value})"),
            &format!("Scalar::measure(9u16, {spare}, {value}, {other}, {last})"),
        )
        .replace(
            "ensures result == value\nreaches Host",
            &format!("ensures result == ({expression})\nreaches Host"),
        )
}

#[test]
fn nested_boolean_return_requires_exact_operations_and_carried_equations() {
    let source = nested_boolean_guarantee_source(
        "(value == spare) == (other == last)",
        "Host::finish(false);",
        [true, false, false, true],
    );
    let published = artifact(&checked_from_source(&source));
    let module = decode_module(&published.0).unwrap();
    let proof = decode_proof_bundle(&published.1).unwrap();
    let mut changed_proof = proof.clone();
    let equalities = changed_proof
        .evidence
        .iter_mut()
        .find_map(|evidence| {
            let proof_admission::EvidenceRoute::CertificateDerived(certificate) =
                &mut evidence.route
            else {
                return None;
            };
            match &mut certificate.proof.rule {
                proof_admission::ProofRule::ValueEqualityTransport { equalities, .. } => {
                    Some(equalities)
                }
                _ => None,
            }
        })
        .expect("nested return carries explicit equation transport");
    assert!(!equalities.is_empty());
    assert!(equalities.iter().all(|equality| matches!(
        equality.rule,
        proof_admission::ProofRule::SemanticAxiom { .. }
    )));
    equalities.clear();
    assert!(
        terminal_verifier::verify_module(&module, &changed_proof, &AdmissionProfile::default())
            .is_err()
    );

    let mut changed_module = module.clone();
    let wrapper = changed_module
        .machines
        .iter_mut()
        .find(|machine| !machine.contract.ensures.is_empty())
        .unwrap();
    let changed_operation = wrapper
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanEqual { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationKind::BooleanEqual { left, right } = &mut changed_operation.kind
    else {
        unreachable!()
    };
    *right = *left;
    assert!(
        terminal_verifier::verify_module(&changed_module, &proof, &AdmissionProfile::default())
            .is_err()
    );
}

#[test]
fn boolean_result_equation_preserves_closed_entry_fact_proofs() {
    let source = boolean_guarantee_source("Host::finish(false); true", false)
        .replace("requires value == value", "requires !value")
        .replace(
            "ensures result == value\nreaches Host",
            "ensures result == !value\nreaches Host",
        );
    let published = artifact(&checked_from_source(&source));
    let (status, observed) = execute(&published);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [
            vec![TerminalScalarValue::Boolean(false)],
            vec![TerminalScalarValue::Boolean(true)]
        ]
    );
}

#[test]
fn ordered_computed_boolean_equality_keeps_mixed_parameter_identities() {
    for input in [false, true] {
        for spare in [false, true] {
            let source = boolean_guarantee_source("Host::finish(false); value == spare", input)
                .replace(
                    "Scalar::measure(value: bool)",
                    "Scalar::measure(marker: u16, spare: bool, value: bool)",
                )
                .replace(
                    &format!("Scalar::measure({input})"),
                    &format!("Scalar::measure(9u16, {spare}, {input})"),
                )
                .replace(
                    "ensures result == value\nreaches Host",
                    "ensures result == (value == spare)\nreaches Host",
                );
            let published = artifact(&checked_from_source(&source));
            let (status, observed) = execute(&published);
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(
                observed.arguments,
                [
                    vec![TerminalScalarValue::Boolean(false)],
                    vec![TerminalScalarValue::Boolean(input == spare)]
                ]
            );
        }
    }
}

#[test]
fn computed_boolean_guarantees_reject_changed_return_and_selected_evidence() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
    let source = boolean_guarantee_source("Host::finish(false); !value", false).replace(
        "ensures result == value\nreaches Host",
        "ensures result == !value\nreaches Host",
    );
    let original = checked_from_source(&source);
    let published = artifact(&original);
    let proof = decode_proof_bundle(&published.1).unwrap();
    let mut module = decode_module(&published.0).unwrap();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| !machine.contract.ensures.is_empty())
        .unwrap();
    let input = machine.parameters[0].id;
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, terminal_psi::Terminator::Return { .. }))
        .unwrap();
    let terminal_psi::Terminator::Return { value, .. } = &mut block.terminator else {
        unreachable!()
    };
    *value = input;
    assert!(
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).is_err()
    );

    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap();
    let state = original.machine_states(machine)[0].symbol;
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.values.scalar_expressions;
        let position = plans.expressions.iter().position(|plan| plan.state == state && matches!(&plan.expression, Scalar::Boolean(expression) if matches!(expression.as_ref(), Boolean::Not(_)))).unwrap();
        match mutation {
            0 => {
                plans.expressions[position].expression =
                    Scalar::Boolean(Box::new(Boolean::Parameter { position: 0 }))
            }
            1 => plans.expressions.push(plans.expressions[position].clone()),
            2 => {
                plans.expressions[position].role =
                    checked_trees::CheckedScalarExpressionRole::ContinuationReturn
            }
            3 => {
                let binding = plans
                    .source_bindings
                    .iter()
                    .find(|(_, binding)| {
                        binding.state == state
                            && binding.role == checked_trees::CheckedScalarExpressionRole::Return
                    })
                    .unwrap()
                    .1
                    .clone();
                plans.source_bindings.append(binding);
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordered_scalar_guarantees_require_return_evidence_and_exact_return_value() {
    assert_exact_normal_return_evidence(&normal_guarantee_source(
        "let observed: i32 = Host::measure(11); value",
    ));
    assert_exact_normal_return_evidence(&boolean_guarantee_source(
        "let observed: bool = Host::measure(false); value",
        true,
    ));
}

fn assert_exact_normal_return_evidence(source: &str) {
    let artifact = artifact(&checked_from_source(source));
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    let wrapper = module
        .machines
        .iter()
        .find(|machine| {
            !machine.contract.ensures.is_empty() && !machine.contract.requires.is_empty()
        })
        .unwrap();
    let obligation = wrapper.contract.ensures[0].obligation;
    let mut missing = proof.clone();
    missing
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert_eq!(missing.evidence.len() + 1, proof.evidence.len());
    assert!(
        terminal_verifier::verify_module(&module, &missing, &AdmissionProfile::default()).is_err()
    );

    let mut changed = module.clone();
    let wrapper = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == wrapper.id)
        .unwrap();
    let block = wrapper
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator, terminal_psi::Terminator::Return { .. }))
        .unwrap();
    let opaque_result = block
        .operations
        .iter()
        .find_map(|operation| match (&operation.kind, &operation.result) {
            (
                terminal_psi::OperationKind::BoundaryCall { .. },
                terminal_psi::OperationResult::Scalar(result),
            ) => Some(result.id),
            _ => None,
        })
        .unwrap();
    let terminal_psi::Terminator::Return { value, .. } = &mut block.terminator else {
        unreachable!()
    };
    assert_ne!(*value, opaque_result);
    *value = opaque_result;
    // The test handler returns its input, but an opaque boundary promises no
    // such relation. Neither its carrier nor a previous proof grants equality.
    assert!(
        terminal_verifier::verify_module(&changed, &proof, &AdmissionProfile::default()).is_err()
    );
}

#[test]
fn ordered_boolean_guarantees_keep_mixed_scalar_slots_and_source_identity() {
    use checked_trees::{CheckedBooleanExpression as Boolean, ClosedScalarContractValue as Clause};
    for input in [false, true] {
        let source = boolean_guarantee_source("Host::finish(false); value", input)
            .replace(
                "Scalar::measure(value: bool)",
                "Scalar::measure(marker: u16, value: bool, spare: bool)",
            )
            .replace(
                &format!("Scalar::measure({input})"),
                &format!("Scalar::measure(9u16, {input}, {})", !input),
            );
        let original = checked_from_source(&source);
        let published = artifact(&original);
        let (status, observed) = execute(&published);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments.last(),
            Some(&vec![TerminalScalarValue::Boolean(input)])
        );
        let target = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Scalar::measure")
            .unwrap()
            .symbol;
        for mutation in 0..6 {
            let mut changed = original.clone();
            let contract = changed
                .facts
                .contract_plans
                .machines
                .iter_mut()
                .find(|contract| contract.machine == target)
                .unwrap();
            let mut guarantees = contract.closed_scalar_values.ensures().to_vec();
            match mutation {
                0 => guarantees.clear(),
                1 => guarantees.push(guarantees[0].clone()),
                2 => guarantees[0] = Some(Clause::Boolean(true)),
                3..=5 => {
                    let Some(Clause::Predicate(Boolean::Equal { left, .. })) = &mut guarantees[0]
                    else {
                        panic!("Boolean equality")
                    };
                    assert_eq!(**left, Boolean::Parameter { position: 3 });
                    **left = match mutation {
                        3 => Boolean::Parameter { position: 2 },
                        4 => Boolean::Local { position: 3 },
                        5 => Boolean::Parameter { position: 0 },
                        _ => unreachable!(),
                    };
                }
                _ => unreachable!(),
            }
            contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
                contract.closed_scalar_values.requires().to_vec(),
                guarantees,
                contract.closed_scalar_values.has_crash_clauses(),
                contract.closed_scalar_values.has_outcome_specific_clauses(),
            );
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn ordered_scalar_guarantees_reject_changed_source_predicates() {
    use checked_trees::{CheckedBooleanExpression as Boolean, ClosedScalarContractValue as Clause};
    let original = checked_from_source(&normal_guarantee_source("Host::finish(11); value"));
    artifact(&original);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    for mutation in 0..7 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == target)
            .unwrap();
        let mut guarantees = contract.closed_scalar_values.ensures().to_vec();
        match mutation {
            0 => guarantees.clear(),
            1 => guarantees.push(guarantees[0].clone()),
            2 => guarantees[0] = None,
            3 => guarantees[0] = Some(Clause::Boolean(true)),
            4..=6 => {
                let Some(Clause::Predicate(Boolean::IntegerComparison { kind, left, .. })) =
                    &mut guarantees[0]
                else {
                    panic!("integer guarantee")
                };
                match mutation {
                    4 => {
                        **left = CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: typed_trees::types::PrimitiveType::I32,
                        }
                    }
                    5 => {
                        **left = CheckedScalarExpression::Local {
                            position: 1,
                            primitive_type: typed_trees::types::PrimitiveType::I32,
                        }
                    }
                    6 => *kind = checked_trees::CheckedIntegerComparisonKind::LessThan,
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
        contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
            contract.closed_scalar_values.requires().to_vec(),
            guarantees,
            contract.closed_scalar_values.has_crash_clauses(),
            contract.closed_scalar_values.has_outcome_specific_clauses(),
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordered_boundary_return_preserves_entry_predicates() {
    let source = source()
        .replace(
            "Scalar::measure() -> i32 reaches Host",
            "Scalar::measure(value: i32) -> i32\nrequires value >= 1\nreaches Host",
        )
        .replace(
            "let result: i32 = Host::measure(70);\n            result",
            "Host::finish(11); Host::measure(value)",
        )
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    let wrapper = module
        .machines
        .iter()
        .find(|machine| !machine.parameters.is_empty())
        .unwrap();
    assert_eq!(wrapper.contract.requires.len(), 1);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [vec![integer(11)], vec![integer(70)], vec![integer(70)]]
    );
}

fn ordered_contract_source() -> String {
    source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32 [1..=100], other: i32) -> i32\nrequires value >= 1\nrequires other >= 2\nrequires value >= 1\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", "Host::finish(other); Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70, 11);")
}

#[test]
fn ordered_boundary_return_preserves_boolean_entry_predicates() {
    let source = source()
        .replace("Scalar::measure() -> i32 reaches Host", "Scalar::measure(value: i32, allowed: bool, denied: bool) -> i32\nrequires allowed && !denied\nreaches Host")
        .replace("let result: i32 = Host::measure(70);\n            result", "Host::finish(11); Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70, true, false);");
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert!(
        !module
            .machines
            .iter()
            .find(|machine| machine.parameters.len() == 3)
            .unwrap()
            .contract
            .requires
            .is_empty()
    );
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [vec![integer(11)], vec![integer(70)], vec![integer(70)]]
    );
}

#[test]
fn ordered_boundary_requirements_keep_clause_slots_and_call_proofs() {
    for body in [
        "Host::finish(other); Host::measure(value)",
        "Host::finish(other); let result: i32 = Host::measure(value); result",
        "let saved: i32 = value; Host::finish(other); Host::measure(saved)",
    ] {
        let source =
            ordered_contract_source().replace("Host::finish(other); Host::measure(value)", body);
        let artifact = artifact(&checked_from_source(&source));
        let module = decode_module(&artifact.0).unwrap();
        let wrapper = module
            .machines
            .iter()
            .find(|machine| machine.parameters.len() == 2)
            .unwrap();
        // Source duplicates normalize before publication. Every distinct
        // published requirement still owns a separate call proof slot.
        assert_eq!(wrapper.contract.requires.len(), 3);
        let obligations = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallStructuralScalar {
                    callee,
                    requirement_obligations,
                    ..
                } if *callee == wrapper.id => Some(requirement_obligations),
                _ => None,
            })
            .unwrap();
        assert_eq!(obligations.len(), 3);
        for obligation in obligations {
            let mut proof = decode_proof_bundle(&artifact.1).unwrap();
            let count = proof.evidence.len();
            proof
                .evidence
                .retain(|evidence| evidence.obligation != *obligation);
            assert_eq!(proof.evidence.len() + 1, count);
            assert!(
                terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                    .is_err()
            );
        }
        let (status, observed) = execute(&artifact);
        assert_eq!(
            status,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            observed.arguments,
            [vec![integer(11)], vec![integer(70)], vec![integer(70)]]
        );
    }
}

#[test]
fn ordered_boundary_requirements_reject_source_predicate_substitution() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind};
    let original = checked_from_source(&ordered_contract_source());
    artifact(&original);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    for mutation in 0..10 {
        let mut changed = original.clone();
        let contract = changed
            .facts
            .contract_plans
            .machines
            .iter_mut()
            .find(|contract| contract.machine == target)
            .unwrap();
        let mut requirements = contract
            .crash
            .structural_runtime_requirements()
            .unwrap()
            .to_vec();
        match mutation {
            0 => {
                requirements.remove(0);
            }
            1 => requirements.push(requirements[0].clone()),
            2 => requirements.swap(0, 1),
            3 => requirements[0] = Boolean::Constant(true),
            4 => {
                let Boolean::IntegerComparison { kind, .. } = &mut requirements[0] else {
                    panic!("comparison");
                };
                *kind = CheckedIntegerComparisonKind::LessThan;
            }
            5 | 6 => {
                let Boolean::IntegerComparison { right, .. } = &mut requirements[0] else {
                    panic!("comparison");
                };
                **right = if mutation == 5 {
                    CheckedScalarExpression::Parameter {
                        position: 1,
                        primitive_type: checked_trees::types::PrimitiveType::I32,
                    }
                } else {
                    CheckedScalarExpression::Local {
                        position: 0,
                        primitive_type: checked_trees::types::PrimitiveType::I32,
                    }
                };
            }
            7 | 8 => {
                let Boolean::And { left, right } = &mut requirements[3] else {
                    panic!("range");
                };
                let Boolean::IntegerComparison {
                    left: minimum,
                    right: subject,
                    ..
                } = left.as_mut()
                else {
                    panic!("lower bound");
                };
                if mutation == 7 {
                    **subject = CheckedScalarExpression::Parameter {
                        position: 1,
                        primitive_type: checked_trees::types::PrimitiveType::I32,
                    };
                } else {
                    let Boolean::IntegerComparison { right: maximum, .. } = right.as_ref() else {
                        panic!("upper bound");
                    };
                    *minimum = maximum.clone();
                }
            }
            _ => {}
        }
        contract.crash = contract
            .crash
            .clone()
            .with_structural_runtime_requirements((mutation != 9).then_some(requirements));
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn scalar_wrapper_explicit_entry_predicate_survives_call_proofs() {
    let source = source()
        .replace(
            "Scalar::measure() -> i32 reaches Host",
            "Scalar::measure(value: i32) -> i32\nrequires value >= 1\nreaches Host",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let checked = checked_from_source(&source);
    let artifact = artifact(&checked);
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(
        module
            .machines
            .iter()
            .find(|machine| !machine.parameters.is_empty())
            .unwrap()
            .contract
            .requires
            .len(),
        1
    );
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
}

#[test]
fn parameterized_wrappers_nested_as_actuals_keep_one_body_and_ordered_effects() {
    let source = source()
        .replace("Scalar::measure() ->", "Scalar::measure(value: i32) ->")
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace(
            "Scalar::measure();",
            "Scalar::measure(Scalar::measure(70));",
        );
    let artifact = artifact(&checked_from_source(&source));
    assert_eq!(decode_module(&artifact.0).unwrap().machines.len(), 2);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(
        observed.arguments,
        [vec![integer(70)], vec![integer(70)], vec![integer(70)]]
    );
}

#[test]
fn scalar_wrapper_signature_and_parameter_range_custody_reject_mutations() {
    let source = source()
        .replace(
            "Scalar::measure() ->",
            "Scalar::measure(value: i32 [1..=100]) ->",
        )
        .replace("Host::measure(70)", "Host::measure(value)")
        .replace("Scalar::measure();", "Scalar::measure(70);");
    let original = checked_from_source(&source);
    for mutation in 0..4 {
        let mut checked = original.clone();
        let plan = &mut checked.facts.flow.terminal_boundary_scalar_returns.machines[0];
        match mutation {
            0 => plan.scalar_parameters.clear(),
            1 => plan.scalar_parameters[0].source_position = 1,
            2 => plan.scalar_parameters[0].primitive_type = typed_trees::types::PrimitiveType::Bool,
            3 => {
                let machine = plan.machine;
                let contract = checked
                    .facts
                    .contract_plans
                    .machines
                    .iter_mut()
                    .find(|contract| contract.machine == machine)
                    .unwrap();
                contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
                    Vec::new(),
                    contract.closed_scalar_values.ensures().to_vec(),
                    contract.closed_scalar_values.has_crash_clauses(),
                    contract.closed_scalar_values.has_outcome_specific_clauses(),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Scalar::measure").is_err(),
            "mutation {mutation}"
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn repeated_wrappers_share_boundaries_and_nested_helper_identities() {
    let source = format!(
        "machine identity(value: i32) -> i32\nrequires 0i32 == 0i32\nensures 0i32 == 0i32\n{{ value }}\n{}",
        source().replace("Host::measure(70)", "Host::measure(identity(identity(70)))")
            .replace("Host::finish(result);", "let second: i32 = Scalar::measure();\n Host::finish(result);\n Host::finish(second);")
    );
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(
        module.machines.len(),
        3,
        "one helper, one wrapper, one caller"
    );
    assert_eq!(module.boundary_machines.len(), 2);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, vec![vec![integer(70)]; 4]);
}

#[test]
fn wrapper_boundary_and_direct_boundary_share_one_exact_declaration() {
    let source = source().replace(
        "Host::finish(result);",
        "let direct: i32 = Host::measure(21);\n Host::finish(result);\n Host::finish(direct);",
    );
    let artifact = artifact(&checked_from_source(&source));
    assert_eq!(
        decode_module(&artifact.0).unwrap().boundary_machines.len(),
        2
    );
    let (_, observed) = execute(&artifact);
    assert_eq!(
        observed.arguments,
        [
            vec![integer(70)],
            vec![integer(21)],
            vec![integer(70)],
            vec![integer(21)]
        ]
    );
}

#[test]
fn scalar_wrapper_registration_and_result_drift_reject() {
    for mutation in 0..4 {
        let mut checked = checked_from_source(&source());
        let plans = &mut checked.facts.flow.terminal_boundary_scalar_returns;
        match mutation {
            0 => plans.machines.push(plans.machines[0].clone()),
            1 => plans.machines[0].result_type = typed_trees::types::PrimitiveType::Bool,
            2 => plans.boundary_machines.clear(),
            3 => plans.machines[0].attachment_type_identity = "named(name(Main))".into(),
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn wrappers_nested_as_operands_share_the_complete_unit_catalog() {
    let source = source()
        .replace("Host::measure(70)", "Host::measure(Other::measure())")
        .replace(
            "data Main {}",
            r#"
            data Other {}
            machine Other::measure() -> i32 reaches Host {
                let result: i32 = Host::measure(70);
                result
            }
            data Main {}
        "#,
        )
        .replace(
            "let result: i32 = Scalar::measure();\n    Host::finish(result);",
            "Host::finish(Scalar::measure());",
        );
    let artifact = artifact(&checked_from_source(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(module.machines.len(), 3);
    assert_eq!(module.boundary_machines.len(), 2);
    let (status, observed) = execute(&artifact);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, vec![vec![integer(70)]; 3]);
}

#[test]
fn selected_wrapper_type_duplicates_and_cross_owner_conflicts_reject() {
    for duplicate in [false, true] {
        let mut checked = checked_from_source(&source());
        let declaration = checked
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .structural_types
            .iter()
            .find(|plan| plan.identity == "named(name(Scalar))")
            .unwrap()
            .clone();
        if duplicate {
            checked
                .facts
                .flow
                .terminal_boundary_scalar_returns
                .structural_types
                .push(declaration);
        } else {
            let mut conflicting = declaration;
            conflicting.shape = checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                typed_trees::types::PrimitiveType::Bool,
            );
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(conflicting);
        }
        assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err());
    }
}

#[test]
fn equal_selected_type_declarations_coalesce_without_changing_module_bytes() {
    let mut checked = checked_from_source(&source());
    let original = artifact(&checked);
    let declaration = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .structural_types
        .iter()
        .find(|plan| plan.identity == "named(name(Scalar))")
        .unwrap()
        .clone();
    // Ordinary scalar bodies now retain this same selected type themselves.
    // Cross-catalog equality coalesces; duplicate/conflicting rows within a
    // catalog remain covered by the independent rejection test above.
    let ordinary = &mut checked.facts.flow.terminal_unit_effects.structural_types;
    assert_eq!(
        ordinary
            .iter()
            .filter(|plan| plan.identity == declaration.identity)
            .collect::<Vec<_>>(),
        vec![&declaration]
    );
    ordinary.retain(|plan| plan.identity != declaration.identity);
    assert_eq!(artifact(&checked), original);
    checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .push(declaration);
    assert_eq!(artifact(&checked), original);
}

#[test]
fn removing_wrapper_service_authority_rejects_independently() {
    let artifact = artifact(&checked_from_source(&source()));
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    for root in [false, true] {
        let mut module = decode_module(&artifact.0).unwrap();
        if root {
            module.root_service_reach.concrete.clear();
        } else {
            module
                .machines
                .iter_mut()
                .find(|machine| machine.id != module.entry)
                .unwrap()
                .published_service_ceiling
                .clear();
        }
        assert!(
            terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                .is_err()
        );
    }
}

#[test]
fn wrapper_operand_crash_preserves_call_ceiling_and_prevents_boundary_effects() {
    let source = format!(
        "machine abort() -> i32 crashes Abort {{ crash Abort; }}\n{}",
        source()
            .replace("Host::measure(70)", "Host::measure(abort())")
            .replace("reaches Host {", "reaches Host crashes Abort {")
            .replace("reaches Host\n{", "reaches Host\ncrashes Abort\n{")
    );
    for source in [
        source.clone(),
        source.replace(
            "let result: i32 = Host::measure(abort());\n            result",
            "Host::measure(abort())",
        ),
    ] {
        let artifact = artifact(&checked_from_source(&source));
        let (status, observed) = execute(&artifact);
        let TerminalExecutionStatus::Crashed(crash) = status else {
            panic!("the nested helper must crash before invoking the boundary")
        };
        assert_eq!(crash.cause, terminal_psi::CrashCause::Abort);
        assert!(observed.arguments.is_empty());
        let mut module = decode_module(&artifact.0).unwrap();
        let proof = decode_proof_bundle(&artifact.1).unwrap();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        for operation in entry
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if let terminal_psi::OperationKind::Call {
                crash_continuations,
                ..
            }
            | terminal_psi::OperationKind::CallStructuralScalar {
                crash_continuations,
                ..
            } = &mut operation.kind
            {
                crash_continuations.clear();
            }
        }
        assert!(
            terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                .is_err()
        );
    }
}

#[test]
fn ordinary_boundary_wrapper_replays_actual_body_and_call_custody() {
    let source = source().replace(
        "    machine finish",
        "    machine alternate(value: i32) -> i32 reaches Host;\n    machine finish",
    );
    // Retain a real same-signature alternative boundary through another body;
    // Main's reachable closure still calls only the authored measure target.
    let source = format!(
        "{source}\nmachine alternate_helper() -> i32 reaches Host {{ let value: i32 = Host::alternate(70); value }}"
    );
    let mut original = checked_from_source(&source);
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Scalar::measure")
        .unwrap()
        .symbol;
    let host = original
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Host")
        .unwrap();
    let alternate_symbol = original
        .trait_machine_signatures(host)
        .iter()
        .find(|signature| signature.name.as_str() == "alternate")
        .unwrap()
        .symbol;
    let alternate = original
        .facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .find(|boundary| boundary.machine == alternate_symbol)
        .unwrap();
    let alternate_state = alternate.state;
    let alternate_contract = alternate.contract_report_fingerprint;
    original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .retain(|plan| plan.machine != target);
    let body = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(target)
        .unwrap();
    let body_reach = body.service_reach;
    let caller = original.facts.flow.terminal_unit_effects.machines.iter()
        .find(|plan| plan.operations.iter().any(|operation| matches!(operation,
            CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } if *target_machine == target))).unwrap();
    let caller_symbol = caller.machine;
    let CheckedUnitEffectOperationPlan::ScalarCall { service_reach: call_reach, .. } = caller.operations.iter().find(|operation| matches!(operation,
        CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } if *target_machine == target)).unwrap() else { unreachable!(); };
    let empty_reach = call_reach.direct;
    assert_ne!(empty_reach, body_reach.direct);
    let state = body.state;
    let published = artifact(&original);
    let (status, observed) = execute(&published);
    assert_eq!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observed.arguments, [vec![integer(70)], vec![integer(70)]]);
    for mutation in [
        "missing body",
        "duplicate body",
        "missing boundary",
        "duplicate boundary",
        "redirect boundary",
        "completion",
        "order",
        "contract",
        "body direct",
        "body transitive",
        "published reach",
        "inferred reach",
        "call direct",
        "call transitive",
        "missing state",
        "duplicate state",
    ] {
        let mut changed = original.clone();
        if mutation == "missing state" || mutation == "duplicate state" {
            let (handle, retained) = changed
                .facts
                .flow
                .control
                .states
                .iter()
                .find(|(_, row)| row.machine_symbol == target && row.state_symbol == state)
                .unwrap();
            let retained = retained.clone();
            if mutation == "missing state" {
                changed
                    .facts
                    .flow
                    .control
                    .states
                    .get_mut(handle)
                    .state_symbol = symbols::SymbolHandle::invalid();
            } else {
                changed.facts.flow.control.states.insert(retained);
            }
        } else if mutation == "call direct" || mutation == "call transitive" {
            let caller = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == caller_symbol)
                .unwrap();
            let CheckedUnitEffectOperationPlan::ScalarCall { service_reach, .. } = caller.operations.iter_mut().find(|operation| matches!(operation,
                CheckedUnitEffectOperationPlan::ScalarCall { target_machine, .. } if *target_machine == target)).unwrap() else { unreachable!(); };
            if mutation == "call direct" {
                service_reach.direct = body_reach.direct;
            } else {
                service_reach.transitive = empty_reach;
            }
        }
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        let position = plans
            .iter()
            .position(|plan| plan.machine == target)
            .unwrap();
        if mutation == "missing body" {
            plans.remove(position);
        } else if mutation == "duplicate body" {
            plans.push(plans[position].clone());
        } else {
            let plan = &mut plans[position];
            let boundary = plan
                .operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::BoundaryScalarCall { .. }
                    )
                })
                .unwrap();
            match mutation {
                "missing boundary" => {
                    plan.operations.remove(boundary);
                }
                "duplicate boundary" => {
                    plan.operations
                        .insert(boundary, plan.operations[boundary].clone());
                }
                "redirect boundary" => {
                    let CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                        target_machine,
                        target_state,
                        target_contract_report_fingerprint,
                        ..
                    } = &mut plan.operations[boundary]
                    else {
                        unreachable!();
                    };
                    *target_machine = alternate_symbol;
                    *target_state = alternate_state;
                    *target_contract_report_fingerprint = alternate_contract;
                }
                "completion" => plan.scalar_result.as_mut().unwrap().binding_ordinal += 1,
                "order" => plan.operations.swap(boundary, boundary + 1),
                "body direct" => plan.service_reach.direct = empty_reach,
                "body transitive" => plan.service_reach.transitive = empty_reach,
                "published reach" => {
                    plan.contract_service_reach.interface =
                        language_semantics::ServiceReachInterface::PublishedCeiling(empty_reach)
                }
                "inferred reach" => plan.contract_service_reach.checked_inferred = empty_reach,
                "call direct" | "call transitive" | "missing state" | "duplicate state" => {}
                "contract" => {
                    plan.contract_commitment =
                        checked_trees::MachineContractCommitment::from_digest([0; 32])
                }
                _ => unreachable!(),
            }
        }
        // Do not rebuild source plans here: the receiving stage must reject
        // corrupted retained evidence despite an otherwise valid typed source.
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "Main::main")
                .produce_artifact()
                .is_err(),
            "{mutation}"
        );
    }
}
