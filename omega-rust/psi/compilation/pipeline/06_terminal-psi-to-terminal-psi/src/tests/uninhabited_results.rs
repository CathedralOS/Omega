//! An impossible declared result changes continuation, not the value returned
//! by its sibling arm. An impossible postcondition keeps its declared type.

use super::*;
use terminal_semantics::inhabitation::{Inhabitation, Status, TerminalContracts};

fn program(text: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = super::build_purpose::frontier_of(text, "machine build_entry() {}");
    program_from_typed(typed)
}

fn program_from_typed(
    typed: typed_trees::TypedTrees,
) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let entry = super::build_purpose::symbol(&typed, "main");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

fn accepted(text: &str) -> Box<crate::verdict::AcceptedClosure> {
    accepted_verdicts(program(text))
}

fn accepted_verdicts(
    mut verdicts: Vec<Result<RouteVerdict, crate::CompilationRefusal>>,
) -> Box<crate::verdict::AcceptedClosure> {
    let accepted = match verdicts.remove(0) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the source program must be judged: {other:?}"),
    };
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}",
    );
    accepted
}

fn check_no_normal(accepted: &crate::verdict::AcceptedClosure) -> MachineId {
    let module = &accepted.elaboration.module;
    let query =
        Inhabitation::prepare(&TerminalContracts::new(module.structural_types.iter())).unwrap();
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .filter_map(|block| match block.terminator {
            terminal_psi::Terminator::InvokeNoNormal { callee, .. }
                if module.machine(callee).is_some_and(|machine| {
                    machine.result.structural().is_some_and(|result| {
                        query.status(result.structural_type) == Status::ProvedEmpty
                    })
                }) =>
            {
                Some(callee)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [callee] = calls.as_slice() else {
        panic!("the actual empty-result call must remain a terminal invocation: {calls:?}");
    };
    let result = module
        .machine(*callee)
        .unwrap()
        .result
        .structural()
        .unwrap();
    assert_eq!(query.status(result.structural_type), Status::ProvedEmpty);
    // The call keeps its genuine Empty result declaration. No operation
    // materializes a receiving scalar or converts Empty into the sibling value.
    assert!(module.operations().all(|operation| {
        operation
            .declared_invocation()
            .is_none_or(|call| call.callee != (*callee).into())
    }));
    accepted
        .publish_invocation()
        .expect("actual accepted publication");
    let closed = terminal_codec::ClosedTerminalModule::new(module.clone())
        .expect("the exact invocation module encodes in its own scope");
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
            .expect("receiving admission retains the empty-result proof");
    assert_eq!(received.semantic_bytes(), closed.semantic_bytes());
    assert_eq!(received.identity(), closed.identity());
    assert_eq!(
        received
            .module()
            .machine(*callee)
            .unwrap()
            .result
            .structural(),
        Some(result),
    );
    *callee
}

fn river(declarations: &str, result: &str) -> String {
    format!(
        r#"
{declarations}
public machine halt() -> {result} crashes Trap {{ crash Trap; }}
public machine checked_value(value: u32, fatal: bool) -> u32
ensures result == value
crashes Trap {{
    transition fatal {{ true -> halt() false -> value }}
}}
machine main() crashes Trap {{
    let value: u32 = checked_value(7u32, false);
    transition value == 7u32 {{ true -> done() false -> wrong() }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}
"#
    )
}

#[test]
fn a_false_default_domain_preserves_the_actual_call_and_sibling_value() {
    let accepted = accepted(&river("public data Empty requires false {}", "Empty"));
    check_no_normal(&accepted);
}

#[test]
fn reachable_value_body_falloff_is_a_language_violation() {
    for (declarations, result) in [
        ("", "u32"),
        ("public data Empty requires false {}", "Empty"),
    ] {
        let source = format!(
            "{declarations} public machine produce() -> {result} {{}} \
             machine main() {{ let received: u32 = produce(); }}"
        );
        let verdicts = program(&source);
        let [Ok(RouteVerdict::Rejected { violations, .. })] = verdicts.as_slice() else {
            panic!("the reachable body end must be rejected: {verdicts:?}");
        };
        let [violation] = violations.as_slice() else {
            panic!("the body end must identify its language rule: {violations:?}");
        };
        assert_eq!(violation.statement, "value machine can fall off its end");
    }

    accepted(
        "public machine produce() -> u32 { 7u32 } \
         machine main() { let received: u32 = produce(); }",
    );
    let stopped = accepted(
        "public data Empty requires false {} \
         public machine produce() -> Empty crashes Trap { crash Trap; } \
         machine main() crashes Trap { let received: u32 = produce(); }",
    );
    check_no_normal(&stopped);
}

#[test]
fn canonical_process_exit_publishes_never_without_a_receiving_value() {
    let typed = super::build_purpose::frontier_with_core(
        "use omega::language::core::Never; \
         use omega::language::core::ProcessExit; \
         public machine finish(status: i32) -> Never \
         reaches ProcessExit crashes none terminates { \
             ProcessExit::exit_process(status); \
         } \
         machine main() reaches ProcessExit crashes none terminates { \
             let unreachable_value: u32 = finish(0); \
         }",
        "machine build_entry() {}",
        &[
            (
                "never.omg",
                "Never",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/never.omg"
                )),
            ),
            (
                "process_exit.omg",
                "ProcessExit",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../source/library/core/process_exit.omg"
                )),
            ),
        ],
    );
    let accepted = accepted_verdicts(program_from_typed(typed));
    check_no_normal(&accepted);
    let module = &accepted.elaboration.module;
    let query =
        Inhabitation::prepare(&TerminalContracts::new(module.structural_types.iter())).unwrap();
    let [boundary] = module.boundary_machines.as_slice() else {
        panic!("the exact canonical boundary is retained");
    };
    let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result else {
        panic!("the declared result is ordinary core Never, not Unit");
    };
    assert_eq!(query.status(result.structural_type), Status::ProvedEmpty);
    assert!(boundary.unconditional_termination);
    assert_eq!(
        boundary.completion,
        semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        ),
    );
    assert!(module.operations().all(|operation| !matches!(
        operation.kind,
        terminal_psi::OperationKind::BoundaryCall { .. },
    )));

    // Keeping terminal identity cannot authenticate a substituted inhabited
    // result. Receiving reconstructs the type proof rather than trusting it.
    let mut inhabited = module.clone();
    inhabited
        .structural_types
        .iter_mut()
        .find(|ty| ty.id == result.structural_type)
        .unwrap()
        .requires
        .clear();
    let encoded = terminal_codec::ClosedTerminalModule::new(inhabited)
        .expect("trusted encoding does not repeat receiving verification");
    let error =
        match terminal_codec::ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes()) {
            Err(error) => error,
            Ok(_) => panic!("an inhabited result cannot justify external completion"),
        };
    assert!(
        format!("{error:?}").contains("canonical process-exit signature"),
        "{error:?}",
    );
}

#[test]
fn an_effect_statement_discards_only_an_actual_normal_result() {
    // The inhabited declared type stays u32. Its published false postcondition
    // excludes a result just as an empty declared carrier does, without a
    // separate statement-only prediction of the call's outcome.
    let stopped = accepted(
        "public machine stop() -> u32 ensures false crashes Trap { crash Trap; } \
         public machine relay() -> u32 ensures false crashes Trap { stop(); } \
         machine main() crashes Trap { relay(); }",
    );
    assert!(
        stopped
            .elaboration
            .module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .any(|block| matches!(
                block.terminator,
                terminal_psi::Terminator::InvokeNoNormal { .. }
            )),
    );
    stopped.publish_invocation().unwrap();

    let normal = "public machine value() -> u32 crashes none { 1u32 } \
                  machine main() crashes none { value(); }";
    let verdicts = program(normal);
    assert!(
        format!("{verdicts:?}").contains("expression statement that discards its value"),
        "an inhabited normal result still requires the authored marker: {verdicts:?}",
    );
    accepted(&normal.replace("{ value(); }", "{ _ = value(); }"));
}

#[test]
fn a_first_empty_match_arm_does_not_choose_the_common_or_inferred_type() {
    for matched in [
        "match fatal { true -> halt() false -> value }",
        // Comparisons read their left operand without an expected carrier.
        // This exercises arm inference without a local type annotation.
        "let compared: bool = (match fatal { true -> halt() false -> value }) == value; value",
    ] {
        let original = river("public data Empty requires false {}", "Empty");
        let dispatch = "transition fatal { true -> halt() false -> value }";
        assert_eq!(original.matches(dispatch).count(), 1);
        let text = original.replace(dispatch, matched);
        check_no_normal(&accepted(&text));
    }
}

#[test]
fn a_required_empty_field_and_every_empty_sum_case_exclude_their_result() {
    for (declarations, result) in [
        (
            "public data Empty requires false {} public data Wrapped { value: Empty; }",
            "Wrapped",
        ),
        (
            "public data Empty requires false {} public data Choice { case First(value: Empty); case Second(value: Empty); }",
            "Choice",
        ),
    ] {
        check_no_normal(&accepted(&river(declarations, result)));
    }
}

#[test]
fn an_empty_input_can_be_syntactically_returned_without_creating_a_normal_value() {
    let accepted = accepted(
        "public data Empty requires false {} \
         public machine identity(value: Empty) -> Empty crashes none { value } \
         machine main(value: Empty) -> u32 crashes none { identity(value) }",
    );
    let module = &accepted.elaboration.module;
    assert_eq!(
        module.machines.len(),
        2,
        "the original source needs no helper"
    );
    let query =
        Inhabitation::prepare(&TerminalContracts::new(module.structural_types.iter())).unwrap();
    let callee = module
        .machines
        .iter()
        .find(|machine| machine.result.structural().is_some())
        .expect("the actual identity body remains present");
    assert_eq!(
        query.status(callee.result.structural().unwrap().structural_type),
        Status::ProvedEmpty
    );
    assert!(callee.blocks.iter().any(|block| matches!(
        block.terminator,
        terminal_psi::Terminator::ReturnStructural { .. }
    )));
    assert!(
        terminal_semantics::control_graph::normal_completion_possible(module, callee.id).unwrap(),
        "the declared-type proof must work independently of syntactic non-return",
    );
    let main = module
        .machines
        .iter()
        .find(|machine| machine.result.scalar_ref().is_some())
        .expect("the actual scalar-result caller remains present");
    let sources = terminal_semantics::normal_completion::ReturnSources::new(main).unwrap();
    assert!(
        main.blocks.iter().any(|block| sources
            .is_uninhabited_return(&block.terminator, &query)
            .unwrap()),
        "evaluating the actual Empty argument ends before entering identity"
    );
    assert!(
        module
            .machines
            .iter()
            .all(|machine| machine.module_invocations().count() == 0)
    );
    assert!(
        module
            .operations()
            .all(|operation| operation.result.scalar_ref().is_none()
                && !matches!(
                    operation.kind,
                    terminal_psi::OperationKind::IntegerConstant { .. }
                )),
        "the impossible operand cannot fabricate a scalar or a helper call"
    );
    accepted
        .publish_invocation()
        .expect("actual accepted publication");
    let closed = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
            .expect("receiving admission retains the real Empty return and operand elimination");
    assert_eq!(received.semantic_bytes(), closed.semantic_bytes());
    assert_eq!(received.identity(), closed.identity());
}

fn bare_empty_value(guarantee: &str) -> Box<crate::verdict::AcceptedClosure> {
    let accepted = accepted(&format!(
        "public data Empty requires false {{}} \
         machine main(value: Empty) -> u32 {guarantee} {{ value }}",
    ));
    let module = &accepted.elaboration.module;
    let [machine] = module.machines.as_slice() else {
        panic!("the actual body has no synthetic helper");
    };
    assert_eq!(machine.result.scalar().unwrap().scalar_type, u32_type());
    let [input] = machine.structural_parameters.as_slice() else {
        panic!("the actual Empty input must remain declared");
    };
    let query =
        Inhabitation::prepare(&TerminalContracts::new(module.structural_types.iter())).unwrap();
    assert_eq!(query.status(input.structural_type), Status::ProvedEmpty);
    assert_eq!(machine.module_invocations().count(), 0);
    assert!(module.operations().all(|operation| !matches!(
        operation.kind,
        terminal_psi::OperationKind::IntegerConstant { .. }
    )));
    accepted
        .publish_invocation()
        .expect("actual accepted publication");
    let closed = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
            .expect("the bare empty expression must survive receiving admission");
    assert_eq!(received.semantic_bytes(), closed.semantic_bytes());
    assert_eq!(received.identity(), closed.identity());
    accepted
}

#[test]
fn a_bare_empty_input_needs_no_call_to_occupy_a_scalar_result_position() {
    bare_empty_value("");
}

#[test]
fn a_bare_empty_input_still_establishes_its_scalar_result_guarantee() {
    bare_empty_value("ensures result == 7u32");
}

#[test]
fn receiving_rejects_the_same_structural_return_when_its_type_is_inhabited() {
    let accepted = bare_empty_value("");
    let mut forged = accepted.elaboration.module.clone();
    let [machine] = forged.machines.as_slice() else {
        panic!("the actual bare-value program has one machine");
    };
    let parameter = machine.structural_parameters[0].structural_type;
    let declaration = forged
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == parameter)
        .unwrap();
    let [predicate] = declaration.requires.as_mut_slice() else {
        panic!("the actual input type has one false default-domain clause");
    };
    assert_eq!(*predicate, Proposition::Falsehood);
    // Change only the actual type contract. The same owned structural return
    // can no longer use empty-value compatibility under the u32 header.
    *predicate = Proposition::Truth;
    let closed = terminal_codec::ClosedTerminalModule::new(forged)
        .expect("the single semantic mutation remains encodable");
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes());
    assert!(
        matches!(
            received,
            Err(terminal_codec::CodecError::MalformedStructuralFoundation(_))
        ),
        "the inhabited structural result must fail receiving type validation",
    );
}

fn type_refusal(text: &str) {
    let verdicts = program(text);
    match &verdicts[0] {
        Ok(RouteVerdict::FallBack(refusal)) => {
            assert!(refusal.detail.contains("type"), "{refusal:?}");
        }
        Ok(RouteVerdict::Rejected { violations, .. }) => {
            assert!(
                violations.iter().any(|violation| {
                    violation.statement.contains("scalar carrier `u32`")
                        && violation.statement.contains("requires `bool`")
                }),
                "the scalar result must keep its declared carrier: {verdicts:?}",
            );
        }
        _ => panic!("the declared types must remain incompatible: {verdicts:?}"),
    }
}

#[test]
fn an_ordinary_empty_record_is_inhabited_despite_a_private_crash_body() {
    type_refusal(&river("public data Ordinary {}", "Ordinary"));
}

#[test]
fn an_empty_element_in_a_zero_array_does_not_exclude_its_record() {
    type_refusal(&river(
        "public data Empty requires false {} public data Zero { values: [Empty; 0]; }",
        "Zero",
    ));
}

#[test]
fn an_impossible_postcondition_does_not_change_the_declared_result_type() {
    type_refusal(
        "public machine stop() -> u32 ensures false crashes Trap { crash Trap; } \
         machine main() crashes Trap { let value: bool = stop(); }",
    );
}

#[test]
fn a_nonreturning_initializer_does_not_replace_an_unreachable_locals_annotation() {
    check_no_normal(&accepted(
        "public data Empty requires false {} \
         public machine halt() -> Empty crashes Trap { crash Trap; } \
         machine main() crashes Trap { let n: u32 = halt(); }",
    ));
    type_refusal(
        "public data Empty requires false {} \
         public machine halt() -> Empty crashes Trap { crash Trap; } \
         machine main() crashes Trap { let n: u32 = halt(); let b: bool = n; }",
    );
}

#[test]
fn empty_results_keep_call_requirements_crash_coverage_and_progress() {
    let requires = program(
        "public data Empty requires false {} \
         public machine halt(value: u32) -> Empty requires value > 0u32 \
         crashes Trap { crash Trap; } \
         machine main() crashes Trap { let value: u32 = halt(0u32); }",
    );
    assert!(
        matches!(&requires[0], Ok(RouteVerdict::Rejected { unproved, .. })
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "an empty result does not discharge the call's entry condition: {requires:?}",
    );

    let crashes = program(
        "public data Empty requires false {} \
         public machine halt() -> Empty crashes Trap { crash Trap; } \
         machine main() crashes none { let value: u32 = halt(); }",
    );
    assert!(
        matches!(&crashes[0], Ok(RouteVerdict::Rejected { unproved, .. })
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CrashCoverage { .. }))),
        "an empty result does not grant permission to crash: {crashes:?}",
    );

    let termination = program(
        "public data Empty requires false {} \
         public machine recur() -> Empty terminates { recur() } \
         machine main() {}",
    );
    assert!(
        matches!(&termination[0], Ok(RouteVerdict::FallBack(refusal))
            if refusal.detail.contains("termination promise over a call cycle")),
        "an empty result does not prove an unranked call cycle terminates: {termination:?}",
    );
}

#[test]
fn an_empty_result_does_not_waive_the_bodys_construction_predicate() {
    let verdicts = program(
        "public data Empty requires false {} \
         public machine fabricate() -> Empty { Empty {} } \
         machine main() {}",
    );
    assert!(
        matches!(&verdicts[0], Ok(RouteVerdict::Rejected { unproved, .. })
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::DataDomain { .. }))),
        "a reachable construction still owes the actual false default domain: {verdicts:?}",
    );
}

#[test]
fn a_closed_sum_needs_no_arm_for_a_proved_empty_payload() {
    let text = r#"
public data Empty requires false {}
public data Outcome<T, E> {
    case Ok(value: T);
    case Err(error: E);
}
public machine take_value(outcome: Outcome<u32, Empty>) -> u32 {
    match outcome {
        Outcome::Ok { value } -> value
    }
}
machine main() crashes Trap {
    let outcome: Outcome<u32, Empty> = Outcome::Ok { value: 42u32 };
    let value: u32 = take_value(outcome);
    transition value == 42u32 {
        true -> done()
        false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;
    let borrowed = text
        .replace(
            "take_value(outcome: Outcome",
            "take_value(outcome: &Outcome",
        )
        .replace("take_value(outcome)", "take_value(&outcome)");
    for text in [text, &borrowed] {
        accepted(text)
            .publish_invocation()
            .expect("the actual closed sum keeps its excluded payload evidence");

        // Changing only the payload argument removes the exclusion evidence.
        let inhabited = text.replace("Outcome<u32, Empty>", "Outcome<u32, u32>");
        let verdicts = program(&inhabited);
        assert!(
            verdicts.iter().any(|verdict| matches!(
                verdict,
                Ok(RouteVerdict::Rejected { unestablished, .. })
                    if unestablished.iter().any(|refusal|
                        refusal.detail == "a dispatch selects no arm for a declared case")
            )),
            "an unknown payload still needs its own coverage: {verdicts:?}",
        );
    }
}

fn scalar_postcondition_call(
    accepted: &crate::verdict::AcceptedClosure,
    actual: Option<bool>,
    no_normal: bool,
) {
    use semantic_vocabulary::InvocationSite;
    let module = &accepted.elaboration.module;
    assert_eq!(
        module.machines.len(),
        if actual.is_some() { 3 } else { 2 },
        "only main, stop, and the conditional body's authored crash arm are lowered",
    );
    let stop = accepted
        .elaboration
        .names
        .iter()
        .find_map(|(id, names)| (names.machine == "stop").then_some(id))
        .unwrap();
    let callee = module.machine(stop).unwrap();
    assert_eq!(callee.result.scalar().unwrap().scalar_type, u32_type());
    assert!(callee.requires.is_empty(), "no new input precondition");
    assert_eq!(callee.ensures.len(), 1);
    assert!(!callee.crash_routes.is_empty(), "the Trap promise remains");
    let semantic_vocabulary::TargetMachineCallable::Machine(entry) = module.entry.unwrap() else {
        panic!("the actual main is a declared machine");
    };
    let caller = module.machine(entry).unwrap();
    let calls = caller.module_invocations().collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("main directly invokes the actual stop exactly once");
    };
    assert_eq!(call.inputs.callee, stop);
    assert_eq!(call.inputs.required_contract, None);
    assert!(call.inputs.structural_arguments.is_empty());
    assert!(call.inputs.requirement_obligations.is_empty());
    assert_eq!(
        matches!(call.site, InvocationSite::InvokeNoNormal(_)),
        no_normal,
    );
    match (actual, call.inputs.arguments) {
        (None, []) => {}
        (Some(actual), [argument]) => {
            assert!(caller.operations().any(|operation| {
                operation
                    .result
                    .as_scalar()
                    .is_some_and(|value| value.id == *argument)
                    && matches!(operation.kind,
                        terminal_psi::OperationKind::BooleanConstant { value } if value == actual)
            }));
        }
        _ => panic!("the actual arguments must remain unchanged"),
    }
    if no_normal {
        assert!(
            caller.operations().all(|operation| {
                operation
                    .module_invocation()
                    .is_none_or(|call| call.callee != stop)
                    && operation
                        .result
                        .as_scalar()
                        .is_none_or(|value| value.scalar_type != u32_type())
            }),
            "a no-normal scalar call creates no normal call or receiving value"
        );
    } else {
        let InvocationSite::Operation(id) = call.site else {
            panic!("the returning argument keeps its ordinary invocation");
        };
        let operation = caller
            .operations()
            .find(|operation| operation.id == id)
            .unwrap();
        assert_eq!(
            operation.result.as_scalar().unwrap().scalar_type,
            u32_type()
        );
    }
    accepted.publish_invocation().unwrap();
    let closed = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
            .expect("the actual published postcondition must justify receiving admission");
    assert_eq!(received.semantic_bytes(), closed.semantic_bytes());
    assert_eq!(received.identity(), closed.identity());
    assert_eq!(
        received.module().machine(stop).unwrap().result,
        callee.result
    );
}

#[test]
fn a_public_false_postcondition_keeps_u32_without_a_normal_receiver() {
    scalar_postcondition_call(
        &accepted(
            "public machine stop() -> u32 ensures false crashes Trap { crash Trap; } \
             machine main() crashes Trap { let value: u32 = stop(); }",
        ),
        None,
        true,
    );
}

#[test]
fn a_conditional_public_postcondition_uses_the_actual_boolean_argument() {
    for actual in [true, false] {
        let source = format!(
            "public machine stop(fatal: bool) -> u32 ensures !fatal crashes Trap {{ \
             transition fatal {{ true -> fail() false -> 7u32 }} \
             machine fail() -> u32 crashes Trap {{ crash Trap; }} }} \
             machine main() -> u32 crashes Trap {{ stop({actual}) }}"
        );
        scalar_postcondition_call(&accepted(&source), Some(actual), actual);
    }
}

#[test]
fn a_published_no_return_promise_still_owes_its_own_bodys_guarantee() {
    for text in [
        "public machine stop() -> u32 ensures false { 7u32 } machine main() {}",
        "public machine stop(fatal: bool) -> u32 ensures !fatal { 7u32 } machine main() {}",
    ] {
        let verdicts = program(text);
        assert!(
            matches!(&verdicts[0], Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
            "a public postcondition cannot waive its own returning body's proof: {verdicts:?}",
        );
    }
}

#[test]
fn an_empty_input_crosses_an_annotated_local_without_fabricating_its_carrier() {
    let accepted = accepted(
        "public data Empty requires false {} \
         machine main(value: Empty) -> u32 ensures result == 7u32 { \
             let n: u32 = value; n \
         }",
    );
    let module = &accepted.elaboration.module;
    let [machine] = module.machines.as_slice() else {
        panic!("the bare Empty expression needs no helper machine");
    };
    assert_eq!(
        machine.result.scalar_ref().unwrap().scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
    );
    assert_eq!(machine.module_invocations().count(), 0);
    assert!(
        module.operations().all(|operation| {
            !matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant { .. }
            ) && operation.result.scalar_ref().is_none()
        }),
        "an impossible initializer must not materialize a scalar local value"
    );
    let query =
        Inhabitation::prepare(&TerminalContracts::new(module.structural_types.iter())).unwrap();
    let sources = terminal_semantics::normal_completion::ReturnSources::new(machine).unwrap();
    assert!(
        machine.blocks.iter().any(|block| {
            sources
                .is_uninhabited_return(&block.terminator, &query)
                .unwrap()
        }),
        "the actual owned Empty expression must justify completion"
    );
    accepted
        .publish_invocation()
        .expect("actual accepted publication");
    let closed = terminal_codec::ClosedTerminalModule::new(module.clone()).unwrap();
    let received =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes())
            .expect("receiving admission retains the actual empty initializer");
    assert_eq!(received.semantic_bytes(), closed.semantic_bytes());
    assert_eq!(received.identity(), closed.identity());

    type_refusal(
        "public data Empty requires false {} \
         machine main(value: Empty) { let n: u32 = value; let b: bool = n; }",
    );
}
