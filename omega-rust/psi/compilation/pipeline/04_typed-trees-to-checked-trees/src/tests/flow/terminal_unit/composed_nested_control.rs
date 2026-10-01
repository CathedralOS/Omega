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

#[test]
fn closed_case_dispatch_forwards_a_scalar_payload_beside_an_unread_structural_payload() {
    // The std `IoResult { Error(kind: ErrorKind), Ok(count: u64) }` shape:
    // the other case's payload is itself a sum. The arm that binds it leaves
    // it unread, and the dispatch's one disposal of the plain-owned subject
    // covers it, so the case roster still closes over a local subject.
    let checked = checked(
        r#"
        data Fault {
            case Other;
            case Short;
        }
        data Reading {
            case Failed(fault: Fault);
            case Counted(count: u64);
        }
        data Outcome {
            case Missing;
            case Total(total: u64);
        }
        machine measure(value: u64) -> Reading {
            transition value < 10 {
                true -> (Reading::Counted { count: value })
                false -> (Reading::Failed { fault: Fault::Short })
            }
        }
        machine tally(value: u64) -> Outcome {
            transition { _ -> step(value) }
            state step(value: u64) -> Outcome {
                let reading: Reading = measure(value);
                transition reading {
                    Reading::Counted { count } -> counted(count)
                    Reading::Failed { fault } -> (Outcome::Missing)
                }
            }
            state counted(count: u64) -> Outcome {
                transition { _ -> (Outcome::Total { total: count }) }
            }
        }
        "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "tally"))
        .expect("an unread structural payload leaves the closed-sum dispatch admissible");
    let [_entry, step, counted] = plan.states.as_slice() else {
        panic!("the dispatch state and its payload destination remain")
    };
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
        &step.terminator
    else {
        panic!("the case pair dispatches on the local subject's closed roster")
    };
    let [counted_case, failed_case] = cases.as_slice() else {
        panic!("the dispatch keeps both authored case arms in source order")
    };
    assert!(
        failed_case.payloads.is_empty(),
        "the structural payload is bound but never transferred"
    );
    let checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) =
        &counted_case.outcome
    else {
        panic!("the scalar payload arm transfers to the named successor")
    };
    assert_eq!(successor.target_state, counted.state);
    let [payload] = counted_case.payloads.as_slice() else {
        panic!("the destructured count binds as the case payload")
    };
    assert_eq!(payload.target_scalar_parameter_index, 0);
}

#[test]
fn closed_case_dispatch_never_copies_a_structural_payload_out_of_its_consumed_subject() {
    // Forwarding the structural payload itself would read storage the
    // dispatch consumes on that edge. The closed roster refuses the copy; an
    // admitted plan keeps the arm on another route instead.
    let checked = checked(
        r#"
        data Fault [copy] {
            case Other;
            case Short;
        }
        data Reading {
            case Failed(fault: Fault);
            case Counted(count: u64);
        }
        data Outcome {
            case Missing;
            case Total(total: u64);
        }
        machine measure(value: u64) -> Reading {
            transition value < 10 {
                true -> (Reading::Counted { count: value })
                false -> (Reading::Failed { fault: Fault::Short })
            }
        }
        machine tally(value: u64) -> Outcome {
            transition { _ -> step(value) }
            state step(value: u64) -> Outcome {
                let reading: Reading = measure(value);
                transition reading {
                    Reading::Counted { count } -> counted(count)
                    Reading::Failed { fault } -> failed(fault)
                }
            }
            state counted(count: u64) -> Outcome {
                transition { _ -> (Outcome::Total { total: count }) }
            }
            state failed(fault: Fault) -> Outcome {
                transition { _ -> (Outcome::Missing) }
            }
        }
        "#,
    );
    let Some(plan) = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "tally"))
    else {
        return;
    };
    assert!(
        plan.states.iter().all(|state| !matches!(
            state.terminator,
            checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
        )),
        "no closed-sum dispatch copies a payload out of the subject it consumes"
    );
}
