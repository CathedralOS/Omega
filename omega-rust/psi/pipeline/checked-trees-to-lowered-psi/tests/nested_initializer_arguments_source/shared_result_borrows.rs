use super::boundary_result_moves::{ObserveMoves, source as result_source};
use super::later_results::encoded_locals;
use super::*;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{OperationResult, Terminator};

#[test]
fn anonymous_shared_result_keeps_its_owner_until_call_completion() {
    let pure = checked(
        r#"
        data Token { value: u64; }
        machine forward(token: Token) -> Token { token }
        machine read(token: &Token) {}
        machine main(token: Token) { read(&forward(token)); }
    "#,
    );
    let _pure_artifact = terminal_production::produce_terminal_artifact(&pure, "main")
        .expect("anonymous shared call with an empty consumer publishes");
    let boundary = checked(
        r#"
        pub data Token { value: u64; }
        boundary trait Factory { machine create() -> Token reaches Factory; }
        machine read(token: &Token) {}
        machine main() reaches Factory { read(&Factory::create()); }
    "#,
    );
    let _boundary_artifact = terminal_production::produce_terminal_artifact(&boundary, "main")
        .expect("zero-parameter free caller retains a boundary-produced temporary");
    for boundary in [false, true] {
        for fields in ["value: u64;", "", "elements: [u16; 3];"] {
            assert_anonymous_shared(&anonymous_source(boundary, fields), boundary);
        }
    }
}

fn anonymous_source(boundary: bool, fields: &str) -> String {
    let mut source = format!(
        r#"
        pub data Token {{ {fields} }}
        machine forward(token: Token) -> Token {{ token }}
        machine read(token: &Token) reaches Sink {{ Sink::observe(token); }}
        boundary trait Sink {{ machine observe(token: &Token) reaches Sink; }}
        data Main {{}}
        machine Main::main(token: Token) reaches Sink {{ read(&forward(token)); }}
    "#
    );
    if boundary {
        source = source
            .replace(
                "Main::main(token: Token) reaches Sink",
                "Main::main() reaches Sink + Factory",
            )
            .replace("read(&forward(token))", "read(&Factory::create())");
        source.push_str("boundary trait Factory { machine create() -> Token reaches Factory; }");
    }
    source
}

fn assert_anonymous_shared(source: &str, boundary: bool) {
    use terminal_fuel::FuelChargeSite;
    use terminal_psi::{OperationKind, StructuralAccess};

    let checked = checked(source);
    let artifact = encoded_locals(&checked, &[]);
    let published = terminal_production::produce_terminal_artifact(&checked, "Main::main")
        .expect("anonymous shared argument retains and then cleans its owner");
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let result = operations
        .iter()
        .find_map(|operation| match &operation.result {
            OperationResult::Structural(result) => Some(result),
            _ => None,
        })
        .unwrap();
    let consumer = operations.iter().find(|operation| matches!(&operation.kind,
        OperationKind::CallUnit { structural_arguments, .. }
            if structural_arguments.len() == 1 && structural_arguments[0].access == StructuralAccess::SharedBorrow
    )).unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &consumer.kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments[0].place, result.place);
    let cleanup = caller
        .blocks
        .iter()
        .find(|block| {
            matches!(&block.terminator,
                Terminator::ReturnUnit { trivial_affine_discards, .. }
                    if trivial_affine_discards == &[result.place]
            )
        })
        .expect("only the actual temporary owner is cleaned");
    let arguments = if boundary {
        Vec::new()
    } else {
        vec![TerminalStructuralValue {
            opaque_identity: 700,
            structural_type: caller.structural_parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }]
    };
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
            &arguments,
        )
        .unwrap();
        let mut observer = ObserveMoves::default();
        let mut fuel = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        let mut observed_return = false;
        let mut observed_consumer = false;
        for _ in 0..256 {
            match execution
                .resume_with_effect_handler(&mut fuel, &mut observer)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                    assert!(incremental);
                    if exhaustion.site == FuelChargeSite::Operation(consumer.id) {
                        assert_eq!(
                            execution.live_affine_frontier().count(),
                            1,
                            "owner remains live at the shared call"
                        );
                        observed_consumer = true;
                    }
                    if exhaustion.site == FuelChargeSite::Edge(cleanup.terminator.edge()) {
                        assert_eq!(
                            execution.live_affine_frontier().count(),
                            1,
                            "read did not consume its owner"
                        );
                        assert_eq!(observer.consumed, [700]);
                        observed_return = true;
                    }
                    fuel.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected shared temporary status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(observed_return, incremental);
        assert_eq!(observed_consumer, incremental);
        assert_eq!(
            observer.produced,
            if boundary { vec![700] } else { Vec::new() }
        );
        assert_eq!(observer.consumed, [700]);
        assert!(execution.live_affine_frontier().next().is_none());
        if let Some(reference) = &reference {
            assert_eq!(execution.effects(), reference);
        } else {
            reference = Some(execution.effects().to_vec());
        }
    }
}

fn source(completion: &str) -> String {
    format!(
        "{}\n\
        machine Main::read(token: &Token, count: u16) reaches Sink {{ Sink::read(token, count); }}\n\
        machine Main::inspect(token: &Token, count: u16) -> u16 reaches Sink {{\
            let result: u16 = Sink::inspect(token, count, 17u16); result\
        }}",
        result_source(completion).replace(
            "boundary trait Sink {",
            "boundary trait Sink {\
                machine read(token: &Token, count: u16) reaches Sink;\
                machine inspect(token: &Token, first: u16, second: u16) -> u16 reaches Sink;"
        )
    )
}

#[test]
fn anonymous_shared_result_permissions_rejoin_exact_owner_and_continuation() {
    use language_semantics::{PermissionAccess, PermissionEventSource, PermissionProvenance};
    for boundary in [false, true] {
        let original = checked(&anonymous_source(boundary, "value: u64;"));
        encoded_locals(&original, &[]);
        let events = original
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| matches!(event.root, facts::PlaceRoot::Expression(_)))
            .map(|(handle, event)| (handle, event.clone()))
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 3, "owner, loan, then owner cleanup");
        for (handle, event) in &events {
            for mutation in 0..9 {
                let mut changed = original.clone();
                let mut altered = event.clone();
                match mutation {
                    0 => altered.root = facts::PlaceRoot::Unknown,
                    1 => altered.provenance = PermissionProvenance::Unknown,
                    2 => altered.source = PermissionEventSource::StateExit,
                    3 => altered.obligation_live = true,
                    4 => altered.access = PermissionAccess::Exclusive,
                    5 => altered.multiplicity = language_semantics::Multiplicity::Linear,
                    6 => {
                        changed
                            .facts
                            .flow
                            .ownership
                            .permissions
                            .insert(altered.clone());
                    }
                    7 => altered.kind = language_semantics::PermissionEventKind::Transfer,
                    8 => {
                        altered.segments = changed
                            .facts
                            .flow
                            .ownership
                            .segments
                            .insert_many([facts::PlaceSegment::FixedIndex { index: 0 }])
                    }
                    _ => unreachable!(),
                }
                *changed.facts.flow.ownership.permissions.get_mut(*handle) = altered;
                assert!(
                    terminal_production::produce_terminal_artifact(&changed, "Main::main").is_err(),
                    "boundary={boundary}, kind={:?}, mutation={mutation}",
                    event.kind
                );
            }
        }
        let mut changed = original.clone();
        *changed
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(events[1].0) = events[2].1.clone();
        *changed
            .facts
            .flow
            .ownership
            .permissions
            .get_mut(events[2].0) = events[1].1.clone();
        assert!(terminal_production::produce_terminal_artifact(&changed, "Main::main").is_err());
    }
}

#[test]
fn anonymous_shared_results_reject_missing_cleanup_and_later_statements() {
    for boundary in [false, true] {
        let source = anonymous_source(boundary, "value: u64;");
        let original = checked(&source);
        encoded_locals(&original, &[]);
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main_machine(&original).symbol)
            .unwrap();
        let (CheckedUnitEffectOperationPlan::StructuralCall {
            discard_result_on_return,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            discard_result_on_return,
            ..
        }) = &mut plan.operations[0]
        else {
            panic!("anonymous producer")
        };
        *discard_result_on_return = false;
        assert!(terminal_production::produce_terminal_artifact(&changed, "Main::main").is_err());
        let extended = source.replace(")); }", ")); done(); }") + "machine done() {}";
        assert!(
            terminal_production::produce_terminal_artifact(&checked(&extended), "Main::main")
                .is_err()
        );
    }
}

#[test]
fn named_results_share_their_identity_across_reads_and_final_disposition() {
    for ordinary_producer in [false, true] {
        for consumer in ["Sink::read", "Main::read", "Sink::inspect", "Main::inspect"] {
            for final_move in [false, true] {
                let mut names = vec!["prefix", "first", "spare"];
                let (prefix, value) = if ordinary_producer {
                    names.push("borrowed");
                    ("let borrowed: Token = forward(first, prefix);", "borrowed")
                } else {
                    ("", "first")
                };
                let calls = if consumer.ends_with("inspect") {
                    names.extend(["read_first", "read_second"]);
                    let extra = if consumer == "Sink::inspect" {
                        ", 17u16"
                    } else {
                        ""
                    };
                    format!(
                        "let read_first: u16 = {consumer}(&{value}, prefix{extra});\
                        let read_second: u16 = {consumer}(&{value}, prefix{extra});"
                    )
                } else {
                    format!("{consumer}(&{value}, prefix); {consumer}(&{value}, prefix);")
                };
                let completion = if final_move {
                    format!("Sink::consume({value}, prefix);")
                } else {
                    String::new()
                };
                let checked = checked(&source(&format!("{prefix} {calls} {completion}")));
                let artifact = encoded_locals(&checked, &names);
                let published =
                    terminal_production::produce_terminal_artifact(&checked, "Main::main").unwrap();
                let module = decode_module(&artifact.0).unwrap();
                assert_eq!(decode_module(published.semantic_bytes()).unwrap(), module);
                let entry = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == module.entry)
                    .unwrap();
                let results = entry
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match operation.result {
                        OperationResult::Structural(ref result) => Some(result.place),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let expected_cleanup = if final_move {
                    vec![results[1]]
                } else if ordinary_producer {
                    vec![results[2], results[1]]
                } else {
                    vec![results[1], results[0]]
                };
                assert!(entry.blocks.iter().any(|block| matches!(&block.terminator,
                    Terminator::ReturnUnit { trivial_affine_discards, .. }
                        if *trivial_affine_discards == expected_cleanup)));
                assert_execution(&artifact, if final_move { 3 } else { 2 });
            }
        }
    }
}

fn assert_execution(artifact: &(Vec<u8>, Vec<u8>), observations: usize) {
    let mut reference = None;
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut observer = ObserveMoves::default();
        let mut fuel = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        for _ in 0..1024 {
            match execution
                .resume_with_effect_handler(&mut fuel, &mut observer)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    fuel.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected shared result status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(observer.produced, [700, 701]);
        // The observer records every boundary argument, including shared reads.
        assert_eq!(observer.consumed, vec![700; observations]);
        assert!(execution.live_affine_frontier().next().is_none());
        if let Some(reference) = &reference {
            assert_eq!(execution.effects(), reference);
        } else {
            reference = Some(execution.effects().to_vec());
        }
    }
}

#[test]
fn shared_boundary_signatures_preserve_an_ordinary_callees_owned_parameter() {
    for final_move in [false, true] {
        let completion = if final_move {
            "Sink::consume(token, 5u16);"
        } else {
            ""
        };
        let source = format!(
            "{}\n\
            machine Main::own(token: Token) reaches Sink {{\
                Sink::read(&token, 5u16); Sink::read(&token, 5u16); {completion}\
            }}",
            source("Main::own(first);")
        );
        let artifact = encoded_locals(&checked(&source), &["prefix", "first", "spare"]);
        assert_execution(&artifact, if final_move { 3 } else { 2 });
    }
}

#[test]
fn shared_result_operands_keep_exact_authored_identity_and_final_cleanup() {
    let original = checked(&source(
        "Sink::read(&first, prefix); Sink::read(&first, prefix);",
    ));
    encoded_locals(&original, &["prefix", "first", "spare"]);
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main_machine(&original).symbol)
            .unwrap();
        if mutation == 0 {
            let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                discard_result_on_return,
                ..
            } = plan
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(operation,
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
                    if result.binding_ordinal == 0)
                })
                .unwrap()
            else {
                panic!("first producer")
            };
            *discard_result_on_return = false;
        } else {
            let CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            } = plan
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                    )
                })
                .unwrap()
            else {
                unreachable!()
            };
            match mutation {
                1 => {
                    structural_arguments[0].source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: 1,
                        }
                }
                2 => structural_arguments[0].access = checked_trees::CheckedStructuralAccess::Owned,
                3 => {
                    structural_arguments[0].access =
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                }
                _ => unreachable!(),
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "forged borrowed result mutation {mutation}"
        );
    }
}

#[test]
fn a_named_result_cannot_be_borrowed_after_its_owned_move() {
    let source = source("Sink::consume(first, prefix); Sink::read(&first, prefix);");
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    if let Ok(checked) = typed_trees_to_checked_trees::lower_typed_trees(typed) {
        assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err());
    }
}

#[derive(Default)]
struct RefuseSecondRead {
    observer: ObserveMoves,
    refused: bool,
}

impl TerminalEffectHandler for RefuseSecondRead {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        unreachable!("result-bearing handler")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if self.observer.consumed.len() == 1 && !self.refused {
            self.refused = true;
            return Err(TerminalEffectRejection {
                reason: "retry shared read".into(),
            });
        }
        self.observer.handle_effect_result(effect)
    }
}

#[test]
fn refused_shared_reads_leave_results_live_for_retry() {
    let artifact = encoded_locals(
        &checked(&source(
            "Sink::read(&first, prefix); Sink::read(&first, prefix); Sink::consume(first, prefix);",
        )),
        &["prefix", "first", "spare"],
    );
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut handler = RefuseSecondRead::default();
    let mut fuel = TerminalFuelMeter::unbounded();
    assert!(matches!(
        execution.resume_with_effect_handler(&mut fuel, &mut handler),
        Err(TerminalInterpretError::EffectRejected { .. })
    ));
    assert_eq!(handler.observer.produced, [700, 701]);
    assert_eq!(handler.observer.consumed, [700]);
    assert_eq!(execution.live_affine_frontier().count(), 2);
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut fuel, &mut handler)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(handler.observer.produced, [700, 701]);
    assert_eq!(handler.observer.consumed, [700, 700, 700]);
    assert!(execution.live_affine_frontier().next().is_none());
}

#[test]
fn shared_read_before_a_crashing_operand_has_no_cleanup_successor() {
    let source = format!(
        "machine abort() -> u16 crashes Abort {{ crash Abort; }}\n{}",
        source("Sink::read(&first, prefix); Sink::read(&first, abort());").replace(
            "reaches Factory + Sink {",
            "reaches Factory + Sink crashes Abort {"
        )
    );
    let artifact = encoded_locals(&checked(&source), &["prefix", "first", "spare"]);
    let module = decode_module(&artifact.0).unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut observer = ObserveMoves::default();
    let mut fuel = TerminalFuelMeter::unbounded();
    let status = execution
        .resume_with_effect_handler(&mut fuel, &mut observer)
        .unwrap();
    assert!(matches!(&status, TerminalExecutionStatus::Crashed(crash)
        if crash.cause == terminal_psi::CrashCause::Abort));
    assert_eq!(observer.produced, [700, 701]);
    assert_eq!(observer.consumed, [700]);
    for block in module.machines.iter().flat_map(|machine| &machine.blocks) {
        if let Terminator::ReturnUnit { edge, .. } = block.terminator {
            assert!(
                fuel.usage()
                    .at(terminal_fuel::FuelChargeSite::Edge(edge))
                    .is_none()
            );
        }
    }
    let effects = execution.effects().to_vec();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap(),
        status
    );
    assert_eq!(execution.effects(), effects);
}
