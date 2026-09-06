use super::boundary_result_moves::{ObserveMoves, source};
use super::later_results::encoded_locals;
use super::*;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};
use terminal_psi::{BoundaryMachineResult, OperationKind, OperationResult, Terminator};

#[test]
fn a_boundary_result_moves_directly_into_a_later_boundary_call() {
    let artifact = encoded_locals(
        &checked(&source("Sink::consume(first, identity16(prefix));")),
        &["prefix", "first", "spare"],
    );
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut observer = ObserveMoves::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.produced, [700, 701]);
    assert_eq!(observer.consumed, [700]);
    assert!(execution.live_affine_frontier().next().is_none());
}

#[test]
fn an_ordinary_result_moves_into_a_bodyless_boundary_with_its_identity_intact() {
    let source = format!(
        "{} boundary machine Main::take(token: Token, count: u16) reaches Sink ensures true;",
        source(
            "let moved: Token = forward(first, identity16(prefix)); Main::take(moved, identity16(prefix));"
        ).replace("data Main {}", "pub data Main {}")
    );
    let artifact = encoded_locals(&checked(&source), &["prefix", "first", "spare", "moved"]);
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut observer = ObserveMoves::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.produced, [700, 701]);
    assert_eq!(observer.consumed, [700]);
    assert!(execution.live_affine_frontier().next().is_none());
}

fn replacement_source() -> String {
    source("let replacement: Token = Sink::replace(first, identity16(prefix), 17u16); Sink::consume(replacement, identity16(prefix));")
        .replace("boundary trait Sink {", "boundary trait Sink { machine replace(token: Token, first: u16, second: u16) -> Token reaches Sink;")
}

#[test]
fn consuming_boundaries_establish_replacements_only_after_successful_completion() {
    for nominal in [false, true] {
        let source = if nominal {
            replacement_source()
                .replace("machine Main::main()", "machine Main::main<machine Replace>() where machine Replace satisfies Sink::replace;")
                .replace("Sink::replace(first", "Replace(first")
        } else {
            replacement_source()
        };
        let artifact = encoded_locals(
            &checked(&source),
            &["prefix", "first", "spare", "replacement"],
        );
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
            .filter_map(|operation| match &operation.result {
                OperationResult::Structural(result) => Some(result.place),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 3);
        let cleanup = entry
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::ReturnUnit {
                    trivial_affine_discards,
                    ..
                } => Some(trivial_affine_discards),
                _ => None,
            })
            .unwrap();
        assert_eq!(cleanup, &[results[1]]);
        let mut execution = TerminalExecution::start_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut observer = ObserveMoves::default();
        let mut fuel = TerminalFuelMeter::with_allowance(0);
        let mut complete = false;
        for _ in 0..1024 {
            match execution
                .resume_with_effect_handler(&mut fuel, &mut observer)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    fuel.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(observer.produced, [700, 701, 702]);
        assert_eq!(observer.consumed, [700, 702]);
        assert!(execution.live_affine_frontier().next().is_none());
    }
}

#[derive(Default)]
struct RejectReplacement {
    observed: ObserveMoves,
    reject: bool,
    wrong_result: bool,
}

impl TerminalEffectHandler for RejectReplacement {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result handler required")
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if matches!(effect, TerminalEffect::BoundaryCall { result: BoundaryMachineResult::Structural(_), structural_arguments, .. } if !structural_arguments.is_empty())
        {
            if self.reject {
                return Err(TerminalEffectRejection {
                    reason: "replacement refused".into(),
                });
            }
            if self.wrong_result {
                return Ok(TerminalEffectResult::Unit);
            }
        }
        self.observed.handle_effect_result(effect)
    }
}

#[test]
fn rejected_or_mistyped_replacements_retain_the_direct_input_for_exact_retry() {
    let artifact = encoded_locals(
        &checked(&replacement_source()),
        &["prefix", "first", "spare", "replacement"],
    );
    for wrong_result in [false, true] {
        let mut execution = TerminalExecution::start_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut observer = RejectReplacement {
            reject: !wrong_result,
            wrong_result,
            ..Default::default()
        };
        let mut fuel = TerminalFuelMeter::unbounded();
        let error = execution
            .resume_with_effect_handler(&mut fuel, &mut observer)
            .unwrap_err();
        if wrong_result {
            assert!(matches!(
                error,
                TerminalInterpretError::VerifiedOperationMalformed
            ));
        } else {
            assert!(matches!(
                error,
                TerminalInterpretError::EffectRejected { .. }
            ));
        }
        assert_eq!(execution.live_affine_frontier().count(), 2);
        assert_eq!(observer.observed.produced, [700, 701]);
        assert!(observer.observed.consumed.is_empty());
        observer.reject = false;
        observer.wrong_result = false;
        assert_eq!(
            execution
                .resume_with_effect_handler(&mut fuel, &mut observer)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(observer.observed.produced, [700, 701, 702]);
        assert_eq!(observer.observed.consumed, [700, 702]);
        assert!(execution.live_affine_frontier().next().is_none());
    }
}

#[test]
fn direct_boundary_result_custody_rejects_substitution_and_cleanup_after_transfer() {
    let original = checked(&source("Sink::consume(first, identity16(prefix));"));
    let artifact = encoded_locals(&original, &["prefix", "first", "spare"]);
    let machine = main_machine(&original);
    let mut changed = original.clone();
    let plan = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine.symbol)
        .unwrap();
    for operation in &mut plan.operations {
        match operation {
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return,
                ..
            } => {
                *discard_result_on_return = result.binding_ordinal == 0;
            }
            CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            } if !structural_arguments.is_empty() => {
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: 1,
                    };
            }
            _ => {}
        }
    }
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
        "same-typed spare cannot replace the authored first result"
    );
    let mut module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let consumed = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } if !structural_arguments.is_empty() => Some(structural_arguments[0].place),
            _ => None,
        })
        .unwrap();
    for block in &mut entry.blocks {
        if let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut block.terminator
        {
            trivial_affine_discards.push(consumed);
        }
    }
    assert!(
        terminal_verifier::verify_module(
            &module,
            &decode_proof_bundle(&artifact.1).unwrap(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}
