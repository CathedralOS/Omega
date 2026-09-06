use super::boundary_result_moves::{ObserveMoves, source as result_source};
use super::later_results::encoded_locals;
use super::*;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{OperationResult, Terminator};

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
