//! Two conditional frontiers and three exact effect leaves.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn conditional_successors(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
) -> (
    &checked_trees::CheckedStructuralControlSuccessorPlan,
    &checked_trees::CheckedStructuralControlSuccessorPlan,
) {
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &state.terminator
    else {
        panic!("control state remains conditional")
    };
    (when_true, when_false)
}

#[test]
fn composes_a_reversed_boolean_label_pair_without_an_authored_fallback() {
    let checked = checked(
        r#"
        boundary trait Host { machine exit(code: i32); }
        data Root { flag: bool; }
        machine Root::enter(&mut self) reaches Host {
            self.flag = true;
            transition {
                self.flag == false -> good()
                self.flag == true -> bad()
            }
            state good(&mut self) { Host::exit(70); }
            state bad(&mut self) { Host::exit(71); }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "enter"))
        .expect("`x == false` / `x == true` over one subject is an exact false fallback");
    let [entry, good, bad] = plan.states.as_slice() else {
        panic!("the reversed Boolean pair keeps the entry conditional and both leaves")
    };
    let (when_true, when_false) = conditional_successors(entry);
    assert_eq!(when_true.target_state, good.state);
    assert_eq!(when_false.target_state, bad.state);
}

#[test]
fn composes_an_exact_case_pair_over_a_local_subject_without_an_authored_fallback() {
    let checked = checked(
        r#"
        data DigitParse {
            case Invalid;
            case Digit(digit: u64);
        }
        data RadixParse {
            case Failed;
            case Value(value: u64);
        }
        machine digit_value(value: u64) -> DigitParse {
            transition value < 10 {
                true -> (DigitParse::Digit { digit: value })
                false -> (DigitParse::Invalid)
            }
        }
        machine convert(value: u64) -> RadixParse {
            transition { _ -> step(value) }
            state step(value: u64) -> RadixParse {
                let parsed: DigitParse = digit_value(value);
                transition parsed {
                    DigitParse::Digit { digit } -> accumulate(digit)
                    DigitParse::Invalid -> (RadixParse::Failed)
                }
            }
            state accumulate(digit: u64) -> RadixParse {
                transition { _ -> (RadixParse::Value { value: digit }) }
            }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "convert"))
        .expect("an exact two-case pair over a local subject is a closed-sum dispatch");
    let [_entry, step, accumulate] = plan.states.as_slice() else {
        panic!("the local-subject case pair keeps the dispatch state and its destination")
    };
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
        &step.terminator
    else {
        panic!("the case pair dispatches on the local subject's closed roster")
    };
    let [digit_case, invalid_case] = cases.as_slice() else {
        panic!("the dispatch keeps both authored case arms")
    };
    let checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) = &digit_case.outcome
    else {
        panic!("the payload arm transfers to the named successor")
    };
    assert_eq!(successor.target_state, accumulate.state);
    let [payload] = digit_case.payloads.as_slice() else {
        panic!("the destructured digit binds as the case payload")
    };
    assert_eq!(payload.target_scalar_parameter_index, 0);
    let checked_trees::CheckedClosedSumCaseOutcomePlan::Return(
        checked_trees::CheckedConditionalReturnArm::Structural(
            checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. },
        ),
    ) = &invalid_case.outcome
    else {
        panic!("the value arm establishes the structural result on its own edge")
    };
}
