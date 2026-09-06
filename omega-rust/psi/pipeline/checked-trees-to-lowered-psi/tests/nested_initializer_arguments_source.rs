use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarComputationKind, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult, TerminalExecutionResult, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[path = "nested_initializer_arguments_source/later_results.rs"]
mod later_results;

#[path = "nested_initializer_arguments_source/later_structural_results.rs"]
mod later_structural_results;

#[path = "nested_initializer_arguments_source/boundary_result_moves.rs"]
mod boundary_result_moves;

#[path = "nested_initializer_arguments_source/direct_boundary_results.rs"]
mod direct_boundary_results;

#[path = "nested_initializer_arguments_source/nested_boundary_arguments.rs"]
mod nested_boundary_arguments;

#[path = "nested_initializer_arguments_source/boundary_temporaries.rs"]
mod boundary_temporaries;

#[path = "nested_initializer_arguments_source/shared_result_borrows.rs"]
mod shared_result_borrows;

#[path = "nested_initializer_arguments_source/boundary_result_custody.rs"]
mod boundary_result_custody;

const IDENTITIES: &str = r#"
    machine identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
    data Scalar {}
    machine Scalar::identity(input: u8) -> u8
    requires 0u8 == 0u8
    ensures 0u8 == 0u8
    { input }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn main_machine(checked: &checked_trees::CheckedTrees) -> &typed_trees::machine::Machine {
    checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap()
}

fn encoded(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let machine = main_machine(checked);
    let states = checked.typed.machine_states(machine);
    assert_eq!(
        states.len(),
        1,
        "initializer remains in its authored source state"
    );
    let statements = checked
        .typed
        .statement_table
        .statements(states[0].statement_nodes);
    let locals = statements
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 1, "no hoisted source temporaries");
    assert_eq!(locals[0].name.as_str(), "result");
    assert!(!locals[0].is_mutable);
    assert!(matches!(statements[0], StatementNode::LocalData(_)));
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(locals[0].initial_value)
    else {
        panic!("the authored initializer is still one bare outer call");
    };
    let arguments = checked
        .typed
        .expression_table
        .expression_handles(call.arguments);
    let computations = &checked.facts.values.scalar_computations;
    let roots = computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.machine == machine.symbol
                && root.statement_ordinal == 0
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                        | CheckedScalarExpressionRole::UnitCallArgument { .. }
                )
        })
        .collect::<Vec<_>>();
    assert!(
        !roots.is_empty(),
        "initializer operands have checked computation roots"
    );
    for (_, root) in roots {
        assert!(arguments.contains(&computations.nodes.get(root.root).authored_root));
    }
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("computed result initializer lowers");
    let artifact = (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    );
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("independent verification after codec roundtrip");
    artifact
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[derive(Default)]
struct ObserveResults {
    calls: Vec<Vec<TerminalScalarValue>>,
    structural_response: StructuralResponse,
}

#[derive(Clone, Copy, Default)]
enum StructuralResponse {
    #[default]
    Correct,
    WrongType,
    WrongQualification,
    ProjectedPath,
    Unit,
    Rejected,
}

impl TerminalEffectHandler for ObserveResults {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("boundary effect");
        };
        self.calls.push(arguments.clone());
        Ok(())
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.handle_effect(effect)?;
        let TerminalEffect::BoundaryCall {
            arguments, result, ..
        } = effect
        else {
            unreachable!();
        };
        Ok(match result {
            terminal_psi::BoundaryMachineResult::Unit => TerminalEffectResult::Unit,
            terminal_psi::BoundaryMachineResult::Scalar(_) => {
                TerminalEffectResult::Scalar(arguments[1])
            }
            terminal_psi::BoundaryMachineResult::Structural(expected) => {
                let mut value = TerminalStructuralValue {
                    opaque_identity: 700,
                    structural_type: expected.structural_type,
                    qualifications: expected.qualifications.clone(),
                    path: Vec::new(),
                };
                match self.structural_response {
                    StructuralResponse::Correct => {}
                    StructuralResponse::WrongType => {
                        value.structural_type =
                            semantic_vocabulary::StructuralTypeId::new(99).unwrap();
                        assert_ne!(value.structural_type, expected.structural_type);
                    }
                    StructuralResponse::WrongQualification => {
                        value
                            .qualifications
                            .push(semantic_vocabulary::StructuralDomainId::new(99).unwrap());
                    }
                    StructuralResponse::ProjectedPath => value
                        .path
                        .push(terminal_psi::StructuralPathSegment::Field("flag".into())),
                    StructuralResponse::Unit => return Ok(TerminalEffectResult::Unit),
                    StructuralResponse::Rejected => {
                        return Err(TerminalEffectRejection {
                            reason: "provider refused structural result".into(),
                        });
                    }
                }
                TerminalEffectResult::Structural(value)
            }
        })
    }
}

fn execute(
    artifact: &(Vec<u8>, Vec<u8>),
    arguments: &[TerminalScalarValue],
    observer: &mut ObserveResults,
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_effect_handler_measured(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        arguments,
        &[],
        observer,
    )
    .map(|execution| execution.value())
}

fn scalar_source(boundary: bool) -> String {
    let producer = if boundary {
        "boundary trait Producer { machine choose(first: u16, second: u16, third: u16) -> u16 reaches Producer; }"
    } else {
        "data Producer {} machine Producer::choose(first: u16, second: u16, third: u16) -> u16\nrequires 0u16 == 0u16\nensures result == second\n{ second }"
    };
    let reach = if boundary { "Producer + Host" } else { "Host" };
    format!(
        r#"
        {IDENTITIES}
        {producer}
        boundary trait Host {{ machine finish(value: u16) reaches Host; }}
        data Main {{}}
        machine Main::main(left: u8, right: u8) reaches {reach} {{
            let result: u16 = Producer::choose((Scalar::identity(identity(left)) as u16) + 1u16,
                                               identity(right) as u16, 19u16);
            Host::finish(result);
        }}
    "#
    )
}

fn structural_source() -> String {
    format!(
        r#"
        {IDENTITIES}
        pub data Token {{ flag: bool; }}
        boundary trait Producer {{
            machine create(first: u16, second: u16, third: u16) -> Token reaches Producer;
        }}
        data Main {{}}
        machine Main::main(left: u8, right: u8) reaches Producer {{
            let result: Token = Producer::create((Scalar::identity(identity(left)) as u16) + 1u16,
                                                 identity(right) as u16, 19u16);
        }}
    "#
    )
}

#[test]
fn scalar_results_are_established_after_nested_operands_and_reach_later_calls() {
    for boundary in [false, true] {
        let checked = checked(&scalar_source(boundary));
        let artifact = encoded(&checked);
        for (left, right) in [(0, 7), (255, 3)] {
            let mut observer = ObserveResults::default();
            assert_eq!(
                execute(
                    &artifact,
                    &[unsigned(8, left), unsigned(8, right)],
                    &mut observer
                )
                .unwrap(),
                TerminalExecutionResult::Unit
            );
            let mut expected = Vec::new();
            if boundary {
                expected.push(vec![
                    unsigned(16, left + 1),
                    unsigned(16, right),
                    unsigned(16, 19),
                ]);
            }
            expected.push(vec![unsigned(16, right)]);
            assert_eq!(
                observer.calls, expected,
                "result reaches the consumer only after the complete outer call"
            );
        }
    }
}

#[test]
fn structural_initializer_retains_affine_result_and_normal_cleanup_after_verification() {
    let checked = checked(&structural_source());
    let artifact = encoded(&checked);
    let module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let results = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            if let terminal_psi::OperationResult::Structural(result) = &operation.result {
                Some(result)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].multiplicity,
        terminal_psi::StructuralMultiplicity::Affine
    );
    let discards = entry
        .blocks
        .iter()
        .filter_map(|block| {
            if let terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } = &block.terminator
            {
                Some(trivial_affine_discards)
            } else {
                None
            }
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(discards.len(), 1);
    assert_eq!(*discards[0], results[0].place);
    let mut observer = ObserveResults::default();
    assert_eq!(
        execute(
            &artifact,
            &[unsigned(8, 255), unsigned(8, 3)],
            &mut observer
        )
        .unwrap(),
        TerminalExecutionResult::Unit
    );
    assert_eq!(
        observer.calls,
        vec![vec![unsigned(16, 256), unsigned(16, 3), unsigned(16, 19)]],
        "returned affine value is installed and discarded on normal completion"
    );
}

#[test]
fn nominal_boundary_requirements_execute_computed_result_initializers() {
    for structural in [false, true] {
        for caller_self in [false, true] {
            let source = if structural {
                structural_source().replace("Producer::create(", "Create(")
            } else {
                scalar_source(true).replace("Producer::choose(", "Create(")
            };
            let requirement = if structural { "create" } else { "choose" };
            let parameters = if caller_self {
                "&mut self, left: u8, right: u8"
            } else {
                "left: u8, right: u8"
            };
            let source = source.replace(
                "machine Main::main(left: u8, right: u8)",
                &format!(
                    "machine Main::main<machine Create>({parameters})\nwhere machine Create satisfies Producer::{requirement};"
                ),
            );
            let checked = checked(&source);
            let artifact = encoded(&checked);
            let mut observer = ObserveResults::default();
            assert_eq!(
                execute(
                    &artifact,
                    &[unsigned(8, 255), unsigned(8, 3)],
                    &mut observer
                )
                .unwrap(),
                TerminalExecutionResult::Unit
            );
            let mut expected = vec![vec![unsigned(16, 256), unsigned(16, 3), unsigned(16, 19)]];
            if !structural {
                expected.push(vec![unsigned(16, 3)]);
            }
            assert_eq!(observer.calls, expected);
            let machine = main_machine(&checked);
            let (state_handle, state) = checked
                .facts
                .flow
                .control
                .states
                .iter()
                .find(|(_, state)| state.machine_symbol == machine.symbol)
                .unwrap();
            let (outer_handle, outer) = checked
                .facts
                .flow
                .control
                .calls
                .iter()
                .find(|(_, call)| {
                    call.statement_index == 0
                        && call.call_ordinal == 0
                        && checked
                            .typed
                            .machine_parameter_signature(call.target_symbol)
                            .is_some_and(|(owner, _)| owner.symbol == machine.symbol)
                })
                .unwrap();
            let (_, signature) = checked
                .typed
                .machine_parameter_signature(outer.target_symbol)
                .unwrap();
            assert_ne!(outer.target_symbol, signature.symbol);
            for duplicate in [false, true] {
                let mut changed = checked.clone();
                let control = &mut changed.facts.flow.control;
                if duplicate {
                    let mut copied = outer.clone();
                    copied.target_symbol = signature.symbol;
                    let mut calls = state.calls;
                    control.calls.append_to_span(&mut calls, copied);
                    control.states.get_mut(state_handle).calls = calls;
                } else {
                    control.calls.get_mut(outer_handle).target_symbol = signature.symbol;
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "normalized requirement cannot replace or duplicate the authored callable: duplicate={duplicate}"
                );
            }
        }
    }
}

#[test]
fn attached_unit_callers_execute_all_computed_initializer_result_kinds() {
    for source in [
        scalar_source(false),
        scalar_source(true),
        structural_source(),
    ] {
        let structural = source.contains("let result: Token");
        let boundary = source.contains("boundary trait Producer");
        let source = source.replace(
            "machine Main::main(left: u8, right: u8)",
            "machine Main::main(&mut self, left: u8, right: u8)",
        );
        let checked = checked(&source);
        let artifact = encoded(&checked);
        let mut observer = ObserveResults::default();
        assert_eq!(
            execute(
                &artifact,
                &[unsigned(8, 255), unsigned(8, 3)],
                &mut observer
            )
            .unwrap(),
            TerminalExecutionResult::Unit
        );
        let mut expected = Vec::new();
        if boundary {
            expected.push(vec![unsigned(16, 256), unsigned(16, 3), unsigned(16, 19)]);
        }
        if !structural {
            expected.push(vec![unsigned(16, 3)]);
        }
        assert_eq!(observer.calls, expected);
    }
}

#[test]
fn structural_result_installation_rejects_wrong_carriers_and_provider_refusal() {
    let checked = checked(&structural_source());
    let artifact = encoded(&checked);
    for structural_response in [
        StructuralResponse::WrongType,
        StructuralResponse::WrongQualification,
        StructuralResponse::ProjectedPath,
        StructuralResponse::Unit,
        StructuralResponse::Rejected,
    ] {
        let mut observer = ObserveResults {
            structural_response,
            ..ObserveResults::default()
        };
        let result = execute(
            &artifact,
            &[unsigned(8, 255), unsigned(8, 3)],
            &mut observer,
        );
        if matches!(structural_response, StructuralResponse::Rejected) {
            assert!(
                matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::EffectRejected { rejection, .. }))
                if rejection.reason == "provider refused structural result")
            );
        } else {
            assert!(matches!(
                result,
                Err(TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::VerifiedOperationMalformed
                ))
            ));
        }
        assert_eq!(
            observer.calls.len(),
            1,
            "provider is invoked once but invalid results are not installed"
        );
    }
}

#[test]
fn initializer_short_circuit_operands_skip_crashes_before_establishing_scalar_results() {
    for boundary in [false, true] {
        let producer = if boundary {
            "boundary trait Producer { machine choose(first: bool, second: bool) -> bool reaches Producer; }"
        } else {
            "data Producer {} machine Producer::choose(first: bool, second: bool) -> bool\nrequires true == true\nensures true == true\n{ second }"
        };
        let reach = if boundary { "Producer + Host" } else { "Host" };
        let source = format!(
            r#"
            machine abort() -> bool crashes Abort {{ crash Abort; }}
            machine trap() -> bool crashes Trap {{ crash Trap; }}
            {producer}
            boundary trait Host {{ machine finish(value: bool) reaches Host; }}
            data Main {{}}
            machine Main::main(first: bool, second: bool)
            reaches {reach}
            crashes Abort
            crashes Trap
            {{ let result: bool = Producer::choose(first && abort(), second || trap()); Host::finish(result); }}
        "#
        );
        let checked = checked(&source);
        let artifact = encoded(&checked);
        for (first, second, cause) in [
            (false, true, None),
            (true, false, Some(terminal_psi::CrashCause::Abort)),
            (false, false, Some(terminal_psi::CrashCause::Trap)),
        ] {
            let mut observer = ObserveResults::default();
            let result = execute(
                &artifact,
                &[
                    TerminalScalarValue::Boolean(first),
                    TerminalScalarValue::Boolean(second),
                ],
                &mut observer,
            );
            if let Some(cause) = cause {
                assert!(
                    matches!(result, Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == cause)
                );
                assert!(
                    observer.calls.is_empty(),
                    "no outer producer or result consumer after argument crash"
                );
            } else {
                assert_eq!(result.unwrap(), TerminalExecutionResult::Unit);
                let mut expected = Vec::new();
                if boundary {
                    expected.push(vec![
                        TerminalScalarValue::Boolean(false),
                        TerminalScalarValue::Boolean(true),
                    ]);
                }
                expected.push(vec![TerminalScalarValue::Boolean(true)]);
                assert_eq!(observer.calls, expected);
            }
        }
    }
}

#[test]
fn initializer_argument_crashes_precede_later_casts_and_all_outer_result_kinds() {
    for kind in 0..3 {
        for (first, second, expected) in [
            ("Abort", "Trap", terminal_psi::CrashCause::Abort),
            ("Trap", "Abort", terminal_psi::CrashCause::Trap),
        ] {
            let (producer, carrier, reach, consumer) = match kind {
                0 => (
                    "data Producer {} machine Producer::create(first: u16, second: u16) -> u16\nrequires 0u16 == 0u16\nensures 0u16 == 0u16\n{ second }",
                    "u16",
                    "Host",
                    "Host::finish(result);",
                ),
                1 => (
                    "boundary trait Producer { machine create(first: u16, second: u16) -> u16 reaches Producer; }",
                    "u16",
                    "Producer + Host",
                    "Host::finish(result);",
                ),
                2 => (
                    "pub data Token { flag: bool; } boundary trait Producer { machine create(first: u16, second: u16) -> Token reaches Producer; }",
                    "Token",
                    "Producer",
                    "",
                ),
                _ => unreachable!(),
            };
            let source = format!(
                r#"
                machine first() -> u16 crashes {first} {{ crash {first}; }}
                machine second() -> u8 crashes {second} {{ crash {second}; }}
                {producer}
                boundary trait Host {{ machine finish(value: u16) reaches Host; }}
                data Main {{}}
                machine Main::main()
                reaches {reach}
                crashes Abort
                crashes Trap
                {{ let result: {carrier} = Producer::create(first(), second() as u16); {consumer} }}
            "#
            );
            let checked = checked(&source);
            let artifact = encoded(&checked);
            let mut observer = ObserveResults::default();
            assert!(matches!(execute(&artifact, &[], &mut observer),
                Err(TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash))) if crash.cause == expected));
            assert!(
                observer.calls.is_empty(),
                "no result is established after the first operand crashes"
            );
        }
    }
}

#[test]
fn initializer_computations_and_outer_result_custody_reject_stale_source() {
    for source in [
        scalar_source(false),
        scalar_source(true),
        structural_source(),
    ] {
        let checked = checked(&source);
        encoded(&checked);
        let machine = main_machine(&checked);
        let statements = checked.typed.machine_states(machine)[0].statement_nodes;
        let StatementNode::LocalData(local) =
            &checked.typed.statement_table.statements(statements)[0]
        else {
            unreachable!();
        };
        let ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(local.initial_value)
        else {
            unreachable!();
        };
        let arguments = checked
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let computations = &checked.facts.values.scalar_computations;
        let (outer_flow, _) = checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .find(|(_, flow)| {
                flow.authored_expression == local.initial_value && flow.call_ordinal == 0
            })
            .expect("outer initializer retains its exact authored call occurrence");
        let (value, value_symbol) = computations
            .nodes
            .iter()
            .find_map(|(_, node)| {
                let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
                    return None;
                };
                let flow = checked.facts.flow.control.calls.get(source_call);
                let ExpressionNode::Call(inner) = checked
                    .typed
                    .expression_table
                    .expression(flow.authored_expression)
                else {
                    return None;
                };
                let [value] = checked
                    .typed
                    .expression_table
                    .expression_handles(inner.arguments)
                else {
                    return None;
                };
                let ExpressionNode::Name(path) = checked.typed.expression_table.expression(*value)
                else {
                    return None;
                };
                Some((*value, path.symbol))
            })
            .expect("nested operand contains a live scalar parameter value");
        assert!(
            matches!(
                checked.typed.expression_table.expression(call.receiver),
                ExpressionNode::Name(_)
            ),
            "outer path-qualified call has a live static type qualifier"
        );
        let roots = computations
            .roots
            .iter()
            .filter(|(_, root)| root.machine == machine.symbol && root.statement_ordinal == 0)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2);
        for (handle, root) in &roots {
            for mutation in 0..4 {
                let mut changed = checked.clone();
                let plans = &mut changed.facts.values.scalar_computations;
                match mutation {
                    0 => plans.roots.get_mut(*handle).root = arena::Handle::invalid(),
                    1 => plans.nodes.get_mut(root.root).authored_root = arguments[2],
                    2 => plans.roots.get_mut(*handle).statement_ordinal += 1,
                    3 => plans.roots.get_mut(*handle).role = CheckedScalarExpressionRole::Return,
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "initializer root mutation={mutation}"
                );
            }
        }
        for (handle, node) in computations.nodes.iter() {
            let CheckedScalarComputationKind::Call { source_call, .. } = node.kind else {
                continue;
            };
            for mutation in 0..3 {
                let mut changed = checked.clone();
                if mutation == 2 {
                    changed
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(source_call)
                        .authored_expression = arena::Handle::invalid();
                } else {
                    let CheckedScalarComputationKind::Call {
                        source_call,
                        call_ordinal,
                        ..
                    } = &mut changed
                        .facts
                        .values
                        .scalar_computations
                        .nodes
                        .get_mut(handle)
                        .kind
                    else {
                        unreachable!();
                    };
                    if mutation == 0 {
                        *source_call = arena::Handle::invalid();
                    } else {
                        *call_ordinal += 1;
                    }
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "nested occurrence mutation={mutation}"
                );
            }
        }
        for mutation in 0..14 {
            let mut changed = checked.clone();
            match mutation {
                0 => changed
                    .typed
                    .expression_table
                    .set_expression_handle_at_offset(call.arguments, 0, arguments[1]),
                1 => {
                    let ExpressionNode::Call(call) = changed
                        .typed
                        .expression_table
                        .expression_mut(local.initial_value)
                    else {
                        unreachable!();
                    };
                    call.target_symbol = symbols::SymbolHandle::invalid();
                }
                2 => {
                    let StatementNode::LocalData(local) =
                        &mut changed.typed.statement_table.statements_mut(statements)[0]
                    else {
                        unreachable!();
                    };
                    local.is_mutable = true;
                }
                3 | 4 => {
                    let plan = changed
                        .facts
                        .flow
                        .terminal_unit_effects
                        .machines
                        .iter_mut()
                        .find(|plan| plan.machine == machine.symbol)
                        .unwrap();
                    let scalar_arguments = match &mut plan.operations[0] {
                        CheckedUnitEffectOperationPlan::ScalarCall {
                            scalar_arguments, ..
                        }
                        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                            scalar_arguments,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            scalar_arguments,
                            ..
                        } => scalar_arguments,
                        _ => panic!("result-bearing outer call"),
                    };
                    if mutation == 3 {
                        scalar_arguments.swap(0, 1);
                    } else {
                        scalar_arguments[0] =
                            CheckedCallScalarArgument::Computation(arena::Handle::invalid());
                    }
                }
                5 | 11 => {
                    let ExpressionNode::Call(call) = changed
                        .typed
                        .expression_table
                        .expression_mut(local.initial_value)
                    else {
                        unreachable!();
                    };
                    call.receiver = value;
                    if mutation == 11 {
                        changed
                            .facts
                            .flow
                            .control
                            .calls
                            .get_mut(outer_flow)
                            .receiver_symbol = value_symbol;
                    }
                }
                6 => {
                    let ExpressionNode::Name(path) =
                        changed.typed.expression_table.expression_mut(call.receiver)
                    else {
                        unreachable!();
                    };
                    path.symbol = machine.attached_data_symbol;
                }
                7 | 8 => {
                    let StatementNode::LocalData(changed_local) =
                        &mut changed.typed.statement_table.statements_mut(statements)[0]
                    else {
                        unreachable!();
                    };
                    changed_local.initial_value = if mutation == 7 {
                        arena::Handle::invalid()
                    } else {
                        arena::Handle::from_parts(
                            local.initial_value.arena_index(),
                            local.initial_value.generation() + 1,
                        )
                    };
                }
                9 | 10 => {
                    changed
                        .facts
                        .flow
                        .control
                        .calls
                        .get_mut(outer_flow)
                        .authored_expression = if mutation == 9 {
                        arena::Handle::invalid()
                    } else {
                        arena::Handle::from_parts(
                            local.initial_value.arena_index(),
                            local.initial_value.generation() + 1,
                        )
                    };
                }
                12 | 13 => {
                    let ExpressionNode::Call(call) = changed
                        .typed
                        .expression_table
                        .expression_mut(local.initial_value)
                    else {
                        unreachable!();
                    };
                    let selected = typed_trees::expression::StaticMachineArgument {
                        path: Box::new([]),
                        application: None,
                        const_literal: None,
                        evidence_projection: None,
                        symbol: call.target_symbol,
                    };
                    if mutation == 12 {
                        call.quotient_operation =
                            Some(typed_trees::expression::QuotientOperationRequest {
                                kind: typed_trees::expression::QuotientOperationKind::Lift,
                                representative_operation: selected,
                                theorem_evidence: Box::new([]),
                            });
                    } else {
                        call.private_layout_operation =
                            Some(typed_trees::expression::PrivateLayoutOperationRequest {
                                selected_slot: selected,
                            });
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "outer initializer mutation={mutation}"
            );
        }
    }
}
