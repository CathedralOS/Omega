//! A multi-state receiver machine that returns a primitive scalar lowers as a
//! state graph whose returning states complete through the ordinary scalar
//! exit roster, and an ordinary caller reaches it through the scalar call lane.
//! Each program executes the published artifact after independent checking.

use super::lower_machine;
use crate::TerminalMachineSelection;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedControlResultPlan, CheckedScalarReturnPlan,
};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

/// Publish `entry`, independently check the artifact, run it with one opaque
/// input per structural parameter, and return the scalar it completes with.
fn run_entry(source: &str, entry: &str) -> TerminalScalarValue {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name(entry),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("{entry} publishes one terminal artifact: {error:?}"))
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode module");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let inputs = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: u64::try_from(index).expect("small index") + 1,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &inputs,
            ..Default::default()
        },
    )
    .expect("the published artifact independently checks");
    match execution.resume(
        &mut terminal_fuel::TerminalFuelMeter::unbounded(),
        &mut terminal_interpreter::AcceptTerminalEffects,
    ) {
        Ok(TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(value))) => value,
        other => panic!("{entry} completes with one scalar: {other:?}"),
    }
}

/// The published state graph of `machine`, or the omission that stopped it.
fn state_graph<'checked>(
    checked: &'checked checked_trees::CheckedTrees,
    machine: &str,
) -> &'checked checked_trees::CheckedComposedUnitControlMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| checked.symbols.display_path(plan.machine, "::") == machine)
        .unwrap_or_else(|| {
            let symbol = checked
                .machines()
                .iter()
                .find(|candidate| checked.symbols.display_path(candidate.symbol, "::") == machine)
                .expect("the machine exists")
                .symbol;
            panic!(
                "{machine} is a state graph: {:?}",
                crate::unit::unit_plan_omission_explanation(checked, symbol)
            )
        })
}

fn signed_32(value: TerminalScalarValue) -> i128 {
    let TerminalScalarValue::Integer {
        value: semantic_vocabulary::IntegerValue::Signed(value),
        ..
    } = value
    else {
        panic!("an i32 result: {value:?}");
    };
    value
}

/// A counting loop re-enters the entry state; the leaf returns a binary
/// expression over its parameter.
const LOOP_TO_EXPRESSION_LEAF: &str = r#"
    data Main { last: i32 in Wrapping; }
    machine Main::count(&mut self, n: u32, acc: i32 in Wrapping)
    terminates by n -> Nat::Descending;
    -> i32
    {
        self.last = acc;
        transition n > 0 {
            true -> count(n - 1, acc + 1)
            false -> leaf(acc)
        }
        state leaf(&mut self, acc: i32 in Wrapping) -> i32 { acc + 100 }
    }
    machine Main::main(&mut self) -> i32 {
        let n: i32 = self.count(5, 0);
        n
    }
"#;

#[test]
fn counting_loop_returns_its_leaf_expression_to_an_ordinary_caller() {
    let checked = crate::front_end::checked_program(LOOP_TO_EXPRESSION_LEAF);
    let graph = state_graph(&checked, "Main::count");
    assert_eq!(
        graph.result,
        CheckedControlResultPlan::Scalar {
            primitive_type: checked_trees::types::PrimitiveType::I32,
        }
    );
    assert!(matches!(
        graph.states[1].terminator,
        CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
    ));
    assert_eq!(
        signed_32(run_entry(LOOP_TO_EXPRESSION_LEAF, "Main::main")),
        105
    );
}

/// `transition true { true -> done(v) }` always takes its one arm: it plans
/// and lowers as the same unconditional jump an `Always` guard does.
const CONSTANT_TRUE_TRANSITION: &str = r#"
    data Tally { count: i32 in Wrapping; }
    machine Tally::get(&mut self) -> i32 in Wrapping {
        let v: i32 in Wrapping = 41;
        transition true { true -> done(v + 1) }
        state done(&mut self, value: i32 in Wrapping) -> i32 in Wrapping { value }
    }
    machine Tally::main(&mut self) -> i32 in Wrapping {
        let a: i32 in Wrapping = self.get();
        a
    }
"#;

#[test]
fn a_constant_true_transition_is_an_unconditional_jump() {
    // Typing lowers the run-closing constant-true arm to `Always`, so every
    // scalar owner plans it as the ordinary unconditional jump.
    let checked = crate::front_end::checked_program(CONSTANT_TRUE_TRANSITION);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == "Tally::get")
        .expect("Tally::get exists");
    let entry = &checked.machine_states(machine)[0];
    assert!(matches!(
        checked.statement_table.statements(entry.statement_nodes).last(),
        Some(checked_trees::statement::StatementNode::Transition(transition))
            if transition.guard == checked_trees::statement::TransitionGuardNode::Always
    ));
    assert_eq!(
        signed_32(run_entry(CONSTANT_TRUE_TRANSITION, "Tally::main")),
        42
    );
}

/// A receiver field read carried by an unconditional jump, and value-only
/// guarded arms that each return.
const JUMP_AND_GUARDED_RETURNS: &str = r#"
    data Tally { count: i32 in Wrapping; }
    machine Tally::pick(&mut self, idx: i32) -> i32 in Wrapping {
        transition idx == 0 {
            true -> arm_x()
            _ -> arm_y(idx)
        }
        state arm_x(&mut self) -> i32 in Wrapping {
            let b: i32 in Wrapping = self.count;
            transition { _ -> (b) }
        }
        state arm_y(&mut self, idx: i32) -> i32 in Wrapping {
            transition idx == 1 {
                true -> (20)
                _ -> (30)
            }
        }
    }
    machine Tally::main(&mut self) -> i32 in Wrapping {
        let a: i32 in Wrapping = self.pick(1);
        a
    }
"#;

#[test]
fn guarded_value_arms_return_through_the_selected_state() {
    assert_eq!(
        signed_32(run_entry(JUMP_AND_GUARDED_RETURNS, "Tally::main")),
        20
    );
}

/// A state graph's reach summary covers every state, not only its entry, so a
/// boundary effect in a later state keeps the caller's scalar call admitted.
const LATER_STATE_EFFECT: &str = r#"
    boundary trait Host { machine note(code: i32) reaches Host; }
    data Tally { count: i32 in Wrapping; }
    machine Tally::pick(&mut self, idx: i32) -> i32 reaches Host {
        transition idx == 0 {
            true -> quiet()
            _ -> loud()
        }
        state quiet(&mut self) -> i32 { transition { _ -> (1) } }
        state loud(&mut self) -> i32 {
            Host::note(7);
            transition { _ -> (2) }
        }
    }
    machine Tally::main(&mut self) -> i32 reaches Host {
        let a: i32 = self.pick(1);
        a
    }
"#;

#[test]
fn a_later_state_effect_keeps_the_callers_scalar_call() {
    assert_eq!(signed_32(run_entry(LATER_STATE_EFFECT, "Tally::main")), 2);
}

/// The returned binding and the exit roster are each rejoined with the
/// authored tail: a forged ordinal or carrier must not reach Terminal.
#[test]
fn forged_scalar_completions_reject_lowering() {
    let forge = |source: &str, machine: &str, edit: &dyn Fn(&mut CheckedScalarReturnPlan)| {
        let mut checked = crate::front_end::checked_program(source);
        let graph = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| {
                plan.states.iter().any(|state| {
                    matches!(
                        state.terminator,
                        CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
                    )
                })
            })
            .expect("one scalar-result state graph");
        let CheckedComposedUnitControlTerminatorPlan::ReturnScalar { completion } = &mut graph
            .states
            .iter_mut()
            .find(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ReturnScalar { .. }
                )
            })
            .expect("one returning state")
            .terminator
        else {
            unreachable!()
        };
        edit(completion);
        lower_machine(&checked, TerminalMachineSelection::Name(machine)).is_err()
    };
    assert!(
        forge(LOOP_TO_EXPRESSION_LEAF, "Main::main", &|completion| {
            let CheckedScalarReturnPlan::Binding(binding) = completion else {
                panic!("a final expression completes through its binding");
            };
            binding.binding_ordinal += 1;
        }),
        "a returned binding at another ordinal must reject"
    );
    assert!(
        forge(LOOP_TO_EXPRESSION_LEAF, "Main::main", &|completion| {
            let CheckedScalarReturnPlan::Binding(binding) = completion else {
                unreachable!()
            };
            binding.primitive_type = checked_trees::types::PrimitiveType::I64;
        }),
        "a returned binding with another carrier must reject"
    );
    assert!(
        forge(JUMP_AND_GUARDED_RETURNS, "Tally::main", &|completion| {
            let CheckedScalarReturnPlan::Exits(exits) = completion else {
                panic!("value-only transitions complete through their exit roster");
            };
            let checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal } =
                &mut exits.terminator
            else {
                return;
            };
            *statement_ordinal += 1;
        }),
        "an exit roster that moved off its authored tail must reject"
    );
}

/// Only a single-state scalar graph retains a receiver, so a multi-state body
/// that reads its borrowed `self` is a state graph: the receiver is an
/// ordinary structural parameter with a machine-scope `self` place, and no
/// edge forwards it.
#[test]
fn borrowed_receiver_reads_lower_through_the_state_graph() {
    let checked = crate::front_end::checked_program(
        "data Filter { width: u64; }
         machine Filter::count(&self, alignment: u64) -> u64
         crashes Abort
         {
             transition alignment > 0 {
                 true -> divide(alignment)
                 false -> violated(alignment)
             }
             state violated(&self, alignment: u64) -> u64 {
                 crash Abort;
             }
             state divide(&self, alignment: u64) -> u64 {
                 transition {
                     _ -> (self.width / alignment)
                 }
             }
         }",
    );
    let graph = state_graph(&checked, "Filter::count");
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(graph.machine)
            .is_none()
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Filter::count"))
        .expect("a receiver-reading state graph lowers");
    let machine = &lowered.semantic_module.machines[0];
    assert!(
        machine.structural_places.iter().any(|place| matches!(
            place.kind,
            semantic_vocabulary::StructuralPlaceKind::Parameter { is_self: true, .. }
        )),
        "the receiver keeps a machine-scope self place"
    );
}

/// A promised result guarantee has no Terminal publication on this route, so
/// the body stays unadmitted instead of silently dropping the promise.
#[test]
fn result_guarantees_keep_the_body_out_of_the_state_graph() {
    let checked = crate::front_end::checked_program(
        r#"
        data Tally { count: i32 in Wrapping; }
        machine Tally::pick(&mut self, idx: i32) -> i32
        ensures result >= 20
        {
            transition idx == 0 {
                true -> arm_x()
                _ -> arm_y()
            }
            state arm_x(&mut self) -> i32 { transition { _ -> (20) } }
            state arm_y(&mut self) -> i32 { transition { _ -> (30) } }
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .all(|plan| checked.symbols.display_path(plan.machine, "::") != "Tally::pick")
    );
}

/// Observed boundary integers, in effect order.
#[derive(Default)]
struct Observed(Vec<u128>);

impl terminal_interpreter::TerminalEffectHandler for Observed {
    fn handle_effect(
        &mut self,
        effect: &terminal_interpreter::TerminalEffect,
    ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
        let terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected an observation boundary: {effect:?}");
        };
        let [
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            },
        ] = arguments.as_slice()
        else {
            panic!("expected one unsigned observation: {arguments:?}");
        };
        self.0.push(*value);
        Ok(())
    }
}

/// A receiver-free multi-state machine whose states forward a borrowed view
/// formal. The scalar graph keeps structural formals to one body, so the
/// state graph owns it, and a Unit caller reaches it through the scalar call
/// lane: `scan(bytes, 4, 9)` forwards the view through `step` into `finish`,
/// which returns its length, and `scan(bytes, 0, 9)` returns the first
/// position.
const FORWARDED_VIEW_CALLEE: &str = r#"
    boundary trait Output { machine observe(value: u64) reaches Output; }
    machine scan(line: &[u8], start: u64, end: u64) -> u64 {
        transition { _ -> step(line, start, end, start) }
        state step(line: &[u8], start: u64, end: u64, position: u64) -> u64 {
            transition position < end {
                true -> finish(line, position)
                false -> (start)
            }
        }
        state finish(line: &[u8], position: u64) -> u64 {
            transition position < line.len {
                true -> (position)
                _ -> (line.len)
            }
        }
    }
    machine measure(bytes: &[u8]) reaches Output {
        let beyond: u64 = scan(bytes, 4, 9);
        let first: u64 = scan(bytes, 0, 9);
        Output::observe(beyond);
        Output::observe(first);
    }
    data Root {}
    machine Root::enter() reaches Output {
        measure("hi");
    }
"#;

#[test]
fn a_unit_caller_reaches_a_receiver_free_multi_state_view_callee() {
    let checked = crate::front_end::checked_program(FORWARDED_VIEW_CALLEE);
    let graph = state_graph(&checked, "scan");
    assert_eq!(
        graph.result,
        CheckedControlResultPlan::Scalar {
            primitive_type: checked_trees::types::PrimitiveType::U64,
        }
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(graph.machine)
            .is_none(),
        "structural formals stay with one scalar-graph body"
    );
    for state in &graph.states {
        let [line] = state.structural_parameters.as_slice() else {
            panic!("each state carries its own borrowed view formal");
        };
        assert_eq!(
            line.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
        );
        assert!(!line.is_self);
    }
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .unwrap_or_else(|error| panic!("the Unit closure lowers: {error:?}"));
    let mut observed = Observed::default();
    let result = terminal_interpreter::interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).expect("encode module"),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof"),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut observed,
    )
    .expect("the published artifact independently checks and runs");
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    assert_eq!(observed.0, vec![2, 0]);
}

/// Lower `machine` as the selected root and verify the published module.
fn lowers_and_verifies(source: &str, machine: &str) {
    let checked = crate::front_end::checked_program(source);
    state_graph(&checked, machine);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine))
        .unwrap_or_else(|error| panic!("{machine} lowers through the state graph: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{machine} verifies: {error:?}"));
}

/// Each state's shared view formal arrives on its incoming edge as a
/// whole-parameter `SharedBorrow` transfer: the target re-borrows the source
/// place instead of taking custody it never had.
#[test]
fn forwarded_shared_view_chain_lowers_and_verifies() {
    lowers_and_verifies(FORWARDED_VIEW_CALLEE, "scan");
}

/// The `seek -> check -> seek` cycle never re-enters the entry. The scalar
/// graph's loop plan owned only entry-backed cycles and declined it; the state
/// graph carries any cycle of states, so the machine lowers and verifies.
#[test]
fn forwarded_shared_view_cycle_lowers_and_verifies() {
    lowers_and_verifies(
        r#"
            machine find_equals(line: &[u8], start: u64, end: u64) -> u64 {
                transition { _ -> seek(line, start, end, start) }
                state seek(line: &[u8], start: u64, end: u64, position: u64) -> u64 {
                    transition position < end {
                        true -> check(line, start, end, position)
                        false -> (end)
                    }
                }
                state check(line: &[u8], start: u64, end: u64, position: u64) -> u64 {
                    transition position < line.len && line[position] == 61 {
                        true -> (position)
                        _ -> seek(line, start, end, position + 1)
                    }
                }
            }
        "#,
        "find_equals",
    );
}
