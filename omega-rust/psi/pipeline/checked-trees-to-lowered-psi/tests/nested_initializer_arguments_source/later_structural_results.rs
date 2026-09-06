use super::later_results::{SCALAR_HELPERS, encoded_locals};
use super::*;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{TerminalExecution, TerminalExecutionStatus};

#[test]
fn later_structural_boundary_initializer_uses_the_prior_scalar_namespace() {
    let source = structural_source()
        .replace(
            "let result: Token",
            "let prefix: u8 = left;\nlet result: Token",
        )
        .replace(
            "Scalar::identity(identity(left))",
            "Scalar::identity(identity(prefix))",
        );
    let artifact = encoded_locals(&checked(&source), &["prefix", "result"]);
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
        [vec![unsigned(16, 256), unsigned(16, 3), unsigned(16, 19)]]
    );
}

fn multiple_structural_source() -> String {
    format!(
        r#"
        {SCALAR_HELPERS}
        pub data Token {{ flag: bool; }}
        boundary trait Factory {{
            machine create(first: u16, second: u16, third: u16) -> Token reaches Factory;
        }}
        data Main {{}}
        machine Main::main() reaches Producer + Host + Factory {{
            let prefix: u16 = 5u16;
            let chosen: u16 = Producer::choose(identity16(prefix), 7u16, 11u16);
            Host::finish(chosen);
            let first: Token = Factory::create(identity16(chosen), identity16(prefix), 19u16);
            Host::finish(prefix);
            let between: u16 = chosen;
            let second: Token = Factory::create(identity16(between), identity16(chosen), 23u16);
            Host::finish(between);
        }}
    "#
    )
}

#[derive(Default)]
struct ObserveLaterStructuralResults {
    observed: ObserveResults,
    identities: Vec<u64>,
}

impl TerminalEffectHandler for ObserveLaterStructuralResults {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        self.observed.handle_effect(effect)
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let mut result = self.observed.handle_effect_result(effect)?;
        if let TerminalEffectResult::Structural(value) = &mut result {
            value.opaque_identity += self.identities.len() as u64;
            self.identities.push(value.opaque_identity);
        }
        Ok(result)
    }
}

#[test]
fn multiple_structural_boundaries_retain_scalar_values_and_reverse_result_cleanup() {
    let original = checked(&multiple_structural_source());
    let artifact = encoded_locals(
        &original,
        &["prefix", "chosen", "first", "between", "second"],
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
            terminal_psi::OperationResult::Structural(result) => Some(result.place),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert_ne!(results[0], results[1]);
    let cleanup = entry
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some(trivial_affine_discards),
            _ => None,
        })
        .unwrap();
    assert_eq!(cleanup, &[results[1], results[0]]);
    let expected = vec![
        vec![unsigned(16, 5), unsigned(16, 7), unsigned(16, 11)],
        vec![unsigned(16, 7)],
        vec![unsigned(16, 7), unsigned(16, 5), unsigned(16, 19)],
        vec![unsigned(16, 5)],
        vec![unsigned(16, 7), unsigned(16, 7), unsigned(16, 23)],
        vec![unsigned(16, 7)],
    ];
    let mut reference_effects = None;
    for incremental in [false, true] {
        let mut execution = TerminalExecution::start_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[],
        )
        .unwrap();
        let mut observer = ObserveLaterStructuralResults::default();
        let mut meter = if incremental {
            TerminalFuelMeter::with_allowance(0)
        } else {
            TerminalFuelMeter::unbounded()
        };
        let mut complete = false;
        for _ in 0..1024 {
            match execution
                .resume_with_effect_handler(&mut meter, &mut observer)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    assert!(incremental);
                    meter.replenish(1).unwrap();
                }
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    complete = true;
                    break;
                }
                status => panic!("unexpected result sequence status: {status:?}"),
            }
        }
        assert!(complete);
        assert_eq!(observer.observed.calls, expected);
        assert_eq!(observer.identities, [700, 701]);
        assert!(execution.live_affine_frontier().next().is_none());
        if let Some(reference) = &reference_effects {
            assert_eq!(execution.effects(), reference);
        } else {
            reference_effects = Some(execution.effects().to_vec());
        }
    }
}

#[test]
fn later_structural_operand_crash_keeps_earlier_results_without_cleanup() {
    let source = format!(
        "machine abort() -> u16 crashes Abort {{ crash Abort; }}\n{}",
        multiple_structural_source()
            .replace(
                "reaches Producer + Host + Factory {",
                "reaches Producer + Host + Factory crashes Abort {"
            )
            .replace(
                "Factory::create(identity16(between)",
                "Factory::create(abort()"
            )
    );
    let original = checked(&source);
    let artifact = encoded_locals(
        &original,
        &["prefix", "chosen", "first", "between", "second"],
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(&original, "Main::main").unwrap();
    let state = original.machine_states(main_machine(&original))[0].symbol;
    let crashing_operand = lowered
        .source_call_occurrences
        .iter()
        .find(|occurrence| {
            occurrence.source_state == state
                && occurrence.statement_index == 6
                && occurrence.call_ordinal == 1
        })
        .unwrap()
        .terminal_operation;
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let mut execution = TerminalExecution::start_artifact(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let mut observer = ObserveLaterStructuralResults::default();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut reached_operand = false;
    for _ in 0..1024 {
        let status = execution
            .resume_with_effect_handler(&mut meter, &mut observer)
            .unwrap();
        let TerminalExecutionStatus::SponsorExhausted(exhaustion) = status else {
            panic!("expected pause before entering crashing operand: {status:?}");
        };
        if exhaustion.site == terminal_fuel::FuelChargeSite::Operation(crashing_operand) {
            reached_operand = true;
            break;
        }
        meter.replenish(1).unwrap();
    }
    assert!(reached_operand);
    // The public frontier is frame-local; inspect before suspending this caller.
    assert_eq!(execution.live_affine_frontier().count(), 1);
    meter.replenish(1024).unwrap();
    let status = execution
        .resume_with_effect_handler(&mut meter, &mut observer)
        .unwrap();
    assert!(
        matches!(&status, TerminalExecutionStatus::Crashed(crash) if crash.cause == terminal_psi::CrashCause::Abort)
    );
    assert_eq!(
        observer.observed.calls,
        [
            vec![unsigned(16, 5), unsigned(16, 7), unsigned(16, 11)],
            vec![unsigned(16, 7)],
            vec![unsigned(16, 7), unsigned(16, 5), unsigned(16, 19)],
            vec![unsigned(16, 5)],
        ]
    );
    assert_eq!(observer.identities, [700]);
    for block in &entry.blocks {
        if let terminal_psi::Terminator::ReturnUnit { edge, .. } = block.terminator {
            assert!(
                meter
                    .usage()
                    .at(terminal_fuel::FuelChargeSite::Edge(edge))
                    .is_none(),
                "operand crash cannot run caller cleanup"
            );
        }
    }
    let effects = execution.effects().to_vec();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        status
    );
    assert_eq!(execution.effects(), effects);
}

#[test]
fn later_structural_boundary_rejoins_each_authored_local_and_result_ordinal() {
    let original = checked(&multiple_structural_source());
    let machine = main_machine(&original);
    let [state] = original.machine_states(machine) else {
        unreachable!()
    };
    let statements = original.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(first) = &statements[3] else {
        unreachable!()
    };
    let StatementNode::LocalData(second) = &statements[6] else {
        unreachable!()
    };
    let StatementNode::LocalData(prefix) = &statements[0] else {
        unreachable!()
    };
    for mutation in 0..8 {
        let mut changed = original.clone();
        if mutation < 4 {
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == machine.symbol)
                .unwrap();
            let operation = plan.operations.iter_mut().find(|operation| matches!(operation,
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } if result.binding_ordinal == 1)).unwrap();
            let CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                result,
                discard_result_on_return,
                source_site,
                ..
            } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => result.binding_ordinal = 0,
                1 => result.statement_index = 3,
                2 => *discard_result_on_return = false,
                3 => {
                    *source_site = Some(checked_trees::NominalMachineUseSite::Expression(
                        first.initial_value,
                    ))
                }
                _ => unreachable!(),
            }
        } else {
            let StatementNode::LocalData(local) = &mut changed
                .typed
                .statement_table
                .statements_mut(state.statement_nodes)[6]
            else {
                unreachable!()
            };
            assert_eq!(local.symbol, second.symbol);
            match mutation {
                4 => local.is_mutable = true,
                5 => local.symbol = symbols::SymbolHandle::invalid(),
                6 => local.type_reference = prefix.type_reference,
                7 => local.initial_value = first.initial_value,
                _ => unreachable!(),
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "later structural result mutation {mutation}"
        );
    }
    let artifact = encoded_locals(
        &original,
        &["prefix", "chosen", "first", "between", "second"],
    );
    let mut module = decode_module(&artifact.0).unwrap();
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let cleanup = entry
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some(trivial_affine_discards),
            _ => None,
        })
        .unwrap();
    cleanup.swap(0, 1);
    assert!(
        terminal_verifier::verify_module(
            &module,
            &decode_proof_bundle(&artifact.1).unwrap(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "result cleanup must remain in reverse production order"
    );
}

#[test]
fn later_structural_boundary_calls_retain_nominal_requirement_targets() {
    let source = multiple_structural_source()
        .replace("Factory::create(", "Create(")
        .replace(
            "machine Main::main()",
            "machine Main::main<machine Create>() where machine Create satisfies Factory::create;",
        );
    let artifact = encoded_locals(
        &checked(&source),
        &["prefix", "chosen", "first", "between", "second"],
    );
    let mut observer = ObserveResults::default();
    assert_eq!(
        execute(&artifact, &[], &mut observer).unwrap(),
        TerminalExecutionResult::Unit
    );
    assert_eq!(observer.calls.len(), 6);
    assert_eq!(
        observer.calls[2],
        [unsigned(16, 7), unsigned(16, 5), unsigned(16, 19)]
    );
    assert_eq!(
        observer.calls[4],
        [unsigned(16, 7), unsigned(16, 7), unsigned(16, 23)]
    );
}

#[test]
fn boundary_result_moves_into_a_later_ordinary_unit_call() {
    let source = format!(
        "{}\nmachine Main::consume(token: Token) {{}}",
        multiple_structural_source().replace(
            "Host::finish(between);",
            "Host::finish(between); Main::consume(first);"
        )
    );
    let artifact = encoded_locals(
        &checked(&source),
        &["prefix", "chosen", "first", "between", "second"],
    );
    let mut observer = ObserveResults::default();
    assert_eq!(
        execute(&artifact, &[], &mut observer).unwrap(),
        TerminalExecutionResult::Unit
    );
    assert_eq!(observer.calls.len(), 6);
}

#[test]
fn boundary_and_ordinary_results_share_ordinals_without_sharing_forwarding_custody() {
    let source = format!(
        "machine forward(token: Token, count: u16) -> Token {{ token }}\nmachine Main::consume(token: Token) {{}}\n{}",
        multiple_structural_source()
            .replace("Main::main()", "Main::main(token: Token)")
            .replace(
                "let between: u16",
                "let moved: Token = forward(token, identity16(chosen));\nlet between: u16"
            )
            .replace(
                "Host::finish(between);",
                "Host::finish(between); Main::consume(moved);"
            )
    );
    let artifact = encoded_locals(
        &checked(&source),
        &["prefix", "chosen", "first", "moved", "between", "second"],
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
            terminal_psi::OperationResult::Structural(result) => Some(result.place),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 3);
    let cleanup = entry
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some(trivial_affine_discards),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        cleanup,
        &[results[2], results[0]],
        "only the ordinary result transfers into consume"
    );
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("one input token")
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &artifact.0,
        &artifact.1,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 900,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .unwrap();
    let mut observer = ObserveLaterStructuralResults::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.identities, [700, 701]);
    assert_eq!(observer.observed.calls.len(), 6);
    assert!(execution.live_affine_frontier().next().is_none());
}
