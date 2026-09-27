//! A multi-state receiver machine that returns a primitive scalar lowers as a
//! state graph whose returning states complete through the ordinary scalar
//! exit roster, and an ordinary caller reaches it through the scalar call lane.
//! Each program executes the published artifact after independent checking.

use super::lower_machine;
use crate::TerminalMachineSelection;
use checked_trees::{CheckedComposedUnitControlTerminatorPlan, CheckedScalarReturnPlan};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, TerminalStructuralInputs,
};

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
        state leaf(&mut self, acc: i32 in Wrapping) -> i32 { (acc + 100) as i32 }
    }
    machine Main::main(&mut self) -> i32 {
        let n: i32 = self.count(5, 0);
        n
    }
"#;

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

/// Lower `Root::enter`, check the artifact independently, run it, and return
/// the integers its boundary observed.
fn observed(source: &str) -> Vec<u128> {
    let checked = crate::front_end::checked_program(source);
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
    observed.0
}

/// The published guarantees of the machine named `machine` once it lowers
/// as the selected root and verifies.
fn published_ensures(source: &str, machine: &str) -> Vec<terminal_psi::ContractClause> {
    let checked = crate::front_end::checked_program(source);
    state_graph(&checked, machine);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine))
        .unwrap_or_else(|error| panic!("{machine} lowers: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{machine} verifies: {error:?}"));
    lowered
        .semantic_module
        .machines
        .iter()
        .find(|candidate| candidate.id == lowered.semantic_module.entry)
        .expect("entry machine")
        .contract
        .ensures
        .clone()
}

/// A state graph with a primitive result publishes its authored `ensures`
/// and a closed result range as the machine's normal-return guarantees, and
/// the closure's proof finalization proves each from the facts every exit
/// shares: both edges forward `input` into the one returning state.
const GUARANTEED_FORWARD: &str = r#"
    boundary trait Output { machine observe(value: u64) reaches Output; }
    machine forward(line: &[u8], input: u64) -> u64
    ensures result == input
    {
        transition line.len == 0 {
            true -> last(line, input)
            _ -> last(line, input)
        }
        state last(line: &[u8], carried: u64) -> u64 { carried }
    }
    machine seven(line: &[u8]) -> u64 [7..=7] {
        transition line.len == 0 {
            true -> last(line, 7)
            _ -> last(line, 7)
        }
        state last(line: &[u8], carried: u64) -> u64 [7..=7] { carried }
    }
    machine measure(bytes: &[u8]) reaches Output {
        let forwarded: u64 = forward(bytes, 11);
        let fixed: u64 = seven(bytes);
        Output::observe(forwarded);
        Output::observe(fixed);
    }
    data Root {}
    machine Root::enter() reaches Output {
        measure("hi");
    }
"#;

#[test]
fn result_guarantees_publish_on_the_state_graph_contract_and_prove_at_its_exit() {
    assert_eq!(published_ensures(GUARANTEED_FORWARD, "forward").len(), 1);
    assert_eq!(published_ensures(GUARANTEED_FORWARD, "seven").len(), 1);
    assert_eq!(observed(GUARANTEED_FORWARD), vec![11, 7]);
}
