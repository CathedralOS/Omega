use checked_trees::{CheckedScalarExpression, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[path = "unit_scalar_result_source/boundary_wrappers.rs"]
mod boundary_wrappers;

const SOURCE: &str = r#"
boundary trait Host {
    machine measure(value: i32) -> i32
    reaches Host;
    machine finish(value: i32)
    reaches Host;
}

data Main {}

machine Main::main(&mut self)
reaches Host
{
    let result: i32 = Host::measure(70);
    Host::finish(result);
}
"#;

const ORDINARY_SOURCE: &str = r#"
data Scalar {}

machine Scalar::identity(value: i32) -> i32
requires value == value
ensures result == value
{
    transition { _ -> value }
}

data Sink {}

machine Sink::finish(value: i32) {}

data Main {}

machine Main::main(&mut self) {
    let result: i32 = Scalar::identity(23);
    Sink::finish(result);
}
"#;

const RETURN_RESULT_SOURCE: &str = r#"
data Scalar {}

machine Scalar::identity(value: i32) -> i32
requires value == value
ensures result == value
{
    transition { _ -> value }
}

data Main {}

machine Main::main(&mut self) -> i32 {
    let result: i32 = Scalar::identity(23);
    result
}
"#;

fn checked() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(SOURCE)
}

fn ordinary_checked() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(ORDINARY_SOURCE)
}

fn main_symbol(checked: &checked_trees::CheckedTrees) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main")
        .symbol
}

fn main_operations_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut Vec<CheckedUnitEffectOperationPlan> {
    let main = main_symbol(checked);
    &mut checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == main)
        .expect("Main::main Unit plan")
        .operations
}

fn rejection_message(checked: &checked_trees::CheckedTrees) -> &'static str {
    match checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    ) {
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(message)) => message,
        result => panic!("tampered scalar result flow should reject, got {result:?}"),
    }
}

#[test]
fn attached_unit_ordinary_scalar_result_rejects_contract_and_argument_fact_drift() {
    let mut contract = ordinary_checked();
    let CheckedUnitEffectOperationPlan::ScalarCall {
        target_contract_commitment,
        ..
    } = &mut main_operations_mut(&mut contract)[0]
    else {
        panic!("first operation should bind the ordinary scalar result")
    };
    *target_contract_commitment = checked_trees::MachineContractCommitment::from_digest([0x5a; 32]);
    assert_eq!(
        rejection_message(&contract),
        "ordinary Unit scalar call disagrees with its checked target signature, contract, or reach"
    );

    let mut argument = ordinary_checked();
    let main = main_symbol(&argument);
    let state = argument
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main)
        .expect("Main::main Unit plan")
        .state;
    let (state, statement_ordinal, role) = {
        let CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. } =
            &main_operations_mut(&mut argument)[0]
        else {
            panic!("first operation should bind the ordinary scalar result")
        };
        (
            state,
            coordinate.statement_index,
            checked_trees::CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                argument_ordinal: 0,
            },
        )
    };
    let duplicate = argument
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .find(|candidate| {
            candidate.state == state
                && candidate.statement_ordinal == statement_ordinal
                && candidate.role == role
        })
        .expect("ordinary scalar argument fact")
        .clone();
    argument
        .facts
        .values
        .scalar_expressions
        .expressions
        .push(duplicate);
    assert_eq!(
        rejection_message(&argument),
        "call scalar operand has no unique source-bound checked plan"
    );
}

#[test]
fn attached_unit_scalar_result_rejects_coordinated_drift_from_original_flow_row() {
    let mut checked = checked();
    let operations = main_operations_mut(&mut checked);
    let CheckedUnitEffectOperationPlan::BoundaryScalarCall {
        coordinate, result, ..
    } = &mut operations[0]
    else {
        panic!("first operation should bind the scalar result")
    };
    coordinate.statement_index = 2;
    result.statement_index = 2;
    let CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } = &mut operations[1] else {
        panic!("second operation should consume the scalar result")
    };
    coordinate.statement_index = 3;
    let CheckedUnitEffectOperationPlan::Complete {
        statement_index, ..
    } = &mut operations[2]
    else {
        panic!("third operation should return Unit")
    };
    *statement_index = 4;

    // Coordinated row drift is rejected by the authored-statement existence
    // fence before the source roster compares call ownership.
    assert_eq!(
        rejection_message(&checked),
        "call result has no authored statement"
    );

    // Removing the call's ownership claim entirely still reaches the source
    // roster's missing-owner fence.
    let mut omitted = crate::front_end::checked_program(SOURCE);
    main_operations_mut(&mut omitted).remove(0);
    assert_eq!(
        rejection_message(&omitted),
        "Unit body omits or duplicates an authored call"
    );
}

#[test]
fn attached_unit_scalar_result_return_rejects_lost_argument_custody() {
    let mut checked = crate::front_end::checked_program(RETURN_RESULT_SOURCE);
    // The call's retained actual is the one UnitCallArgument row beside its
    // computation root: relabel it and the custody rejoin must refuse.
    let main = main_symbol(&checked);
    let state = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main)
        .expect("Main::main Unit plan")
        .state;
    let retained = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| {
            expression.state == state
                && expression.role
                    == checked_trees::CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: 0,
                        argument_ordinal: 0,
                    }
        })
        .expect("unit call argument row");
    retained.statement_ordinal = 1;
    let binding_handles = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .filter(|(_, binding)| {
            binding.state == state
                && binding.role
                    == checked_trees::CheckedScalarExpressionRole::UnitCallArgument {
                        call_ordinal: 0,
                        argument_ordinal: 0,
                    }
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in binding_handles {
        checked
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .get_mut(handle)
            .statement_ordinal = 1;
    }
    assert_eq!(
        rejection_message(&checked),
        "computed call operand lost its retained pure argument"
    );
}
