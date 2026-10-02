//! Call results do not choose the source identity rules for their operands.

use checked_trees::{CheckedTrees, CheckedUnitStructuralArgumentSourcePlan};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

const RANGE_SOURCE: &str = r#"
    boundary trait Output { machine size(length: u64) reaches Output; }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) -> u64 { bytes.len }
    machine Helper::relay(bytes: &[u8]) reaches Output {
        let observed: u64 = Helper::measure(bytes[..]);
        Output::size(observed);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::relay("abc");
        Helper::relay("");
    }
"#;

fn lower(
    checked: &CheckedTrees,
) -> Result<lowered_psi::LoweredPsi, checked_trees_to_lowered_psi::LoweringError> {
    lower_machine(checked, TerminalMachineSelection::Name("Root::enter"))
}

#[test]
fn scalar_call_consumes_the_staged_range_and_executes_its_authored_length() {
    let checked = crate::front_end::checked_program(RANGE_SOURCE);
    let lowered = lower(&checked).expect("scalar call consumes the shared staged operand");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("independently verified range call executes");
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    let lengths = result
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("authored size observation");
            };
            let [
                TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(length),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned length");
            };
            *length
        })
        .collect::<Vec<_>>();
    assert_eq!(lengths, [3, 0]);
}

#[test]
fn staged_scalar_argument_rejects_substituted_source_and_access() {
    let original = crate::front_end::checked_program(RANGE_SOURCE);
    lower(&original).expect("unaltered range lowers before tampering");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let argument = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .filter_map(|operation| operation.call_mut())
            .flat_map(|call| &mut call.structural_arguments)
            .find(|argument| {
                matches!(
                    argument.source,
                    CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
                )
            })
            .expect("retained range operand");
        let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root, expression, ..
        } = &mut argument.source
        else {
            unreachable!();
        };
        match mutation {
            0 => *root = checked_trees::CheckedStorageRoot::Parameter { index: 99 },
            1 => {
                *expression =
                    arena::Handle::from_parts(expression.arena_index(), expression.generation() + 1)
            }
            _ => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
        }
        assert!(
            lower(&changed).is_err(),
            "range custody mutation {mutation}"
        );
    }
}

#[test]
fn mutable_argument_may_attenuate_to_a_shared_formal() {
    for (result, body, invocation) in [
        (
            "",
            "output = record.count;",
            "Helper::inspect(&mut left, &mut output);",
        ),
        (
            "-> u64",
            "output = record.count; record.count",
            "let observed: u64 = Helper::inspect(&mut left, &mut output);",
        ),
    ] {
        let checked = crate::front_end::checked_program(&format!(
            "data Record {{ count: u64; }}
             data Helper {{}}
             machine Helper::inspect(record: &Record, output: &mut u64) {result} {{ {body} }}
             data Root {{}}
             machine Root::enter(left: &mut Record, output: &mut u64) {{ {invocation} }}"
        ));
        lower(&checked).expect("a mutable reborrow may be presented as shared");
    }
}

#[test]
fn parameter_source_identity_is_checked_for_unit_and_scalar_results() {
    for (result, completion, invocation) in [
        ("", "", "Helper::touch(&mut left);"),
        (
            "-> u64",
            "7",
            "let observed: u64 = Helper::touch(&mut left);",
        ),
    ] {
        let original = crate::front_end::checked_program(&format!(
            "data Record {{ count: u64; }}
             data Helper {{}}
             machine Helper::touch(record: &mut Record) {result} {{ record.count = 7; {completion} }}
             data Root {{}}
             machine Root::enter(left: &mut Record, right: &mut Record) {{ {invocation} }}"
        ));
        lower(&original).expect("authored parameter call lowers");
        let mut changed = original.clone();
        let argument = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .filter_map(|operation| operation.call_mut())
            .flat_map(|call| &mut call.structural_arguments)
            .next()
            .expect("call parameter");
        argument.source = CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
        let error = lower(&changed).unwrap_err();
        assert!(
            format!("{error:?}").contains("call argument changed its authored parameter place"),
            "{result}: {error:?}"
        );
    }
}

#[test]
fn reference_initializers_preserve_the_parameter_identity_after_reactivation() {
    let original = crate::front_end::checked_program(
        "data Helper {}
         machine Helper::mutate(value: &mut i32) { value = 2; }
         data Root {}
         machine Root::enter(left: &mut i32, right: &mut i32) {
             let parent: &mut i32 = &mut left;
             let child: &i32 = &parent;
             Helper::mutate(parent);
         }",
    );
    lower(&original).expect("restored reference keeps its authored backing");
    let mut changed = original.clone();
    let argument = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .filter_map(|operation| operation.call_mut())
        .flat_map(|call| &mut call.structural_arguments)
        .next()
        .expect("restored parent call argument");
    argument.source = CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    let error = lower(&changed).unwrap_err();
    assert!(
        format!("{error:?}").contains("call argument changed its authored parameter place"),
        "reactivation cannot substitute an unrelated backing: {error:?}"
    );

    let machine = original
        .machines()
        .iter()
        .find(|machine| original.symbols.name(machine.symbol) == "Root::enter")
        .expect("authored caller");
    let state = &original.machine_states(machine)[0];
    let other = original.state_parameters(state)[1].symbol;
    let checked_trees::statement::StatementNode::LocalData(parent) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("parent reference declaration");
    };
    let checked_trees::expression::ExpressionNode::Borrow(borrow) =
        original.expression_table.expression(parent.initial_value)
    else {
        panic!("parent initializer");
    };
    let mut changed = original.clone();
    let checked_trees::expression::ExpressionNode::Name(name) =
        changed.typed.expression_table.expression_mut(borrow.target)
    else {
        panic!("parent backing");
    };
    name.symbol = other;
    name.head_symbol = other;
    assert!(
        lower(&changed).is_err(),
        "changed initializer cannot keep the old backing"
    );
}

const CASE_RESULT_SOURCE: &str = r#"
    data Payload { value: u64; }
    machine Payload::clone(source: &Payload) -> Payload { Payload { value: source.value } }
    data Choice { case Found(payload: Payload); case Other(payload: Payload); case Empty; }
    machine Choice::clone(source: &Choice, alternate: &Choice) -> Choice {
        transition source {
            Choice::Found { payload } -> (Choice::Found { payload: Payload::clone(payload) })
            Choice::Other { payload } -> (Choice::Other { payload: Payload::clone(payload) })
            Choice::Empty -> (Choice::Empty)
        }
    }
"#;

#[test]
fn case_payload_operand_calls_preserve_authored_occurrence_and_source_custody() {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralValueCall,
        CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
    };
    let original = crate::front_end::checked_program(CASE_RESULT_SOURCE);
    let selection = TerminalMachineSelection::Name("Choice::clone");
    let lowered = lower_machine(&original, selection)
        .expect("case member operands retain their nested call occurrence");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("case payload calls and owned constructor results independently verify");
    for mutation in ["case", "field", "root"] {
        let mut changed = original.clone();
        let machine = changed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Choice::clone")
            .unwrap()
            .symbol;
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .expect("owned case result graph");
        let nested = plan
            .states
            .iter_mut()
            .find_map(|state| {
                let CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
                    &mut state.terminator
                else {
                    return None;
                };
                return_values.iter_mut().find_map(|producer| {
                    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } =
                        producer
                    else {
                        return None;
                    };
                    calls.iter_mut().find(|call| {
                        call.operation().call().is_some_and(|call| {
                            call.structural_arguments.iter().any(|argument| {
                                argument.path.iter().any(|segment| {
                                    matches!(segment, CheckedUnitStructuralPathSegment::Case(case) if case == "Found")
                                })
                            })
                        })
                    })
                })
            })
            .expect("nested payload clone retains its original Case path");
        let mut operation = nested.operation().clone();
        let argument = operation
            .call_mut()
            .unwrap()
            .structural_arguments
            .iter_mut()
            .find(|argument| {
                argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Case(_)))
            })
            .unwrap();
        match mutation {
            "case" => {
                *argument
                    .path
                    .iter_mut()
                    .find(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Case(_)))
                    .unwrap() = CheckedUnitStructuralPathSegment::Case("Other".into())
            }
            "field" => {
                *argument
                    .path
                    .iter_mut()
                    .find(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
                    .unwrap() = CheckedUnitStructuralPathSegment::Field("foreign_payload".into())
            }
            "root" => {
                argument.source =
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
            }
            _ => unreachable!(),
        }
        *nested = CheckedStructuralValueCall::with_operands(
            nested.value,
            nested.operands().clone(),
            operation,
        )
        .unwrap();
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("Choice::clone")).is_err(),
            "substituted {mutation} must fail source replay"
        );
    }
}
