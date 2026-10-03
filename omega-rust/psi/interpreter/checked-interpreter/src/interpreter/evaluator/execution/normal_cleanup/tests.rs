use super::*;

const SOURCE: &str = "
    data Token { ready: bool; }
    machine Token::drop(token: &mut Token) requires token.ready {
        token.ready = false;
    }
    machine consume(first: Token, second: Token)
    requires first.ready; second.ready {}
    machine consume_mixed(keep: bool, first: Token, second: Token) -> bool
    requires first.ready; second.ready { keep }
    machine observed(keep: bool, token: Token) -> bool
    requires token.ready { keep }
";

fn ready_owner(
    evaluator: &mut Evaluator<'_>,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Cell {
    let value = evaluator
        .default_value_for_type(reference)
        .unwrap_or_else(|_| panic!("default owner value"));
    let Value::Struct { fields, .. } = &value else {
        panic!("record parameter")
    };
    *fields.get("ready").unwrap().borrow_mut() = Value::Bool(true);
    evaluator
        .allocate_cell(value)
        .unwrap_or_else(|_| panic!("owner cell"))
}

fn is_ready(cell: &Cell) -> bool {
    let value = cell.borrow();
    let Value::Struct { fields, .. } = &*value else {
        panic!("record owner")
    };
    matches!(*fields.get("ready").unwrap().borrow(), Value::Bool(true))
}

#[test]
fn checked_unit_and_scalar_returns_execute_cleanup_on_the_original_parameter_cells() {
    let checked = crate::front_end::checked_program(SOURCE);
    for name in ["consume", "consume_mixed", "observed"] {
        let mut evaluator = Evaluator::new_checked(&checked, &[]);
        let machine = evaluator.find_machine_by_name(name).unwrap();
        let state = evaluator.machine_entry_state(machine).unwrap();
        let mut owners = Vec::new();
        let mut arguments = Vec::new();
        for parameter in checked.typed.state_parameters(state) {
            let cell = if checked
                .typed
                .primitive_type_reference(parameter.type_reference)
                .is_some()
            {
                evaluator
                    .allocate_cell(Value::Bool(true))
                    .unwrap_or_else(|_| panic!("scalar argument cell"))
            } else {
                let owner = ready_owner(&mut evaluator, parameter.type_reference);
                owners.push(owner.clone());
                owner
            };
            arguments.push(EvaluatedArgument::plain(cell));
        }
        assert!(owners.iter().all(is_ready));
        let instance = evaluator
            .allocate_cell(Value::Unit)
            .unwrap_or_else(|_| panic!("instance cell"));
        let result = evaluator
            .run_state_collect(machine, state, instance, arguments)
            .unwrap_or_else(|halt| match halt {
                Halt::Exit(code) => panic!("{name} exited {code}"),
                Halt::Unsupported(message) | Halt::Trap(message) | Halt::Resource(message) => {
                    panic!("{name}: {message}")
                }
            });
        assert!(owners.iter().all(|owner| !is_ready(owner)), "{name}");
        if name != "consume" {
            assert!(
                matches!(result, Some(Value::Bool(true))),
                "scalar result precedes cleanup"
            );
        } else {
            assert!(result.is_none());
        }
    }
}

#[test]
fn checked_cleanup_requires_its_current_owned_place_and_an_executable_plan() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    {
        let mut evaluator = Evaluator::new_checked(&checked, &[]);
        let machine = evaluator.find_machine_by_name("consume").unwrap();
        let state = evaluator.machine_entry_state(machine).unwrap();
        let first = ready_owner(
            &mut evaluator,
            checked.typed.state_parameters(state)[0].type_reference,
        );
        let second = ready_owner(
            &mut evaluator,
            checked.typed.state_parameters(state)[1].type_reference,
        );
        let Value::Struct { fields, .. } = &*first.borrow() else {
            panic!("record owner")
        };
        *fields.get("ready").unwrap().borrow_mut() = Value::Bool(false);
        let instance = evaluator
            .allocate_cell(Value::Unit)
            .unwrap_or_else(|_| panic!("instance cell"));
        let error = evaluator
            .run_state_collect(
                machine,
                state,
                instance,
                vec![
                    EvaluatedArgument::plain(first.clone()),
                    EvaluatedArgument::plain(second.clone()),
                ],
            )
            .unwrap_err();
        assert!(matches!(error, Halt::Trap(message) if message.contains("prerequisite is false")));
        assert!(
            !is_ready(&second),
            "reverse order commits the second owner's hook before testing the first"
        );
    }
    checked
        .facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .clear();
    let evaluator = Evaluator::new_checked(&checked, &[]);
    let machine = evaluator.find_machine_by_name("consume").unwrap();
    let state = evaluator.machine_entry_state(machine).unwrap();
    assert!(matches!(
        evaluator.validate_normal_cleanup_selection(machine, state),
        Err(Halt::Unsupported(message)) if message.contains("no executable checked return plan")
    ));
}

#[test]
fn abrupt_result_evaluation_does_not_invoke_the_checked_normal_cleanup() {
    let checked = crate::front_end::checked_program(SOURCE);
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    let machine = evaluator.find_machine_by_name("observed").unwrap();
    let state = evaluator.machine_entry_state(machine).unwrap();
    assert_eq!(
        evaluator
            .normal_cleanup_actions(machine, state)
            .unwrap_or_else(|_| panic!("selected cleanup plan"))
            .len(),
        1,
        "this control requires a real normal-return action",
    );
    let owner = ready_owner(
        &mut evaluator,
        checked.typed.state_parameters(state)[1].type_reference,
    );
    let keep = evaluator
        .allocate_cell(Value::Bool(true))
        .unwrap_or_else(|_| panic!("scalar argument"));
    let instance = evaluator
        .allocate_cell(Value::Unit)
        .unwrap_or_else(|_| panic!("instance"));
    // The state-entry step succeeds; evaluating the scalar result halts. Any
    // attempted cleanup call would charge a third step before its hook body.
    evaluator.step_budget = 1;
    let result = evaluator.run_state_collect(
        machine,
        state,
        instance,
        vec![
            EvaluatedArgument::plain(keep),
            EvaluatedArgument::plain(owner.clone()),
        ],
    );
    assert!(matches!(result, Err(Halt::Trap(message)) if message == "step budget exceeded"));
    assert_eq!(evaluator.usage.fuel_units(), 2);
    assert!(
        is_ready(&owner),
        "abrupt evaluation leaves the owned value untouched"
    );
}
