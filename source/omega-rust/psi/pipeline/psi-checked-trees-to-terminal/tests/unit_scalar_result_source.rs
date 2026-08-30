use psi_checked_trees::{
    CheckedScalarExpression, CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

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

fn checked() -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    psi_typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

fn main_symbol(checked: &psi_checked_trees::CheckedTrees) -> psi_symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main")
        .symbol
}

fn main_operations_mut(
    checked: &mut psi_checked_trees::CheckedTrees,
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

fn rejection_message(checked: &psi_checked_trees::CheckedTrees) -> &'static str {
    match psi_checked_trees_to_terminal::lower_machine(checked, "Main::main") {
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(message)) => message,
        result => panic!("tampered scalar result flow should reject, got {result:?}"),
    }
}

#[test]
fn attached_unit_scalar_boundary_result_reaches_later_call_in_terminal_psi() {
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked(), "Main::main")
        .expect("complete scalar result flow should lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let boundary_calls = entry.blocks[0]
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                psi_terminal::OperationKind::BoundaryCall { .. }
            )
        })
        .collect::<Vec<_>>();
    let [producer, consumer] = boundary_calls.as_slice() else {
        panic!("entry should retain one scalar producer and one Unit consumer")
    };
    let psi_terminal::OperationResult::Scalar(result) = producer.result else {
        panic!("first boundary call should publish its scalar result")
    };
    let psi_terminal::OperationKind::BoundaryCall { arguments, .. } = &consumer.kind else {
        unreachable!()
    };
    assert_eq!(arguments, &[result.id]);
    assert!(matches!(
        consumer.result,
        psi_terminal::OperationResult::Unit
    ));
}

#[test]
fn attached_unit_scalar_result_coordinates_reject_drift() {
    let mut local_coordinate = checked();
    let CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } =
        &mut main_operations_mut(&mut local_coordinate)[0]
    else {
        panic!("first operation should bind the scalar result")
    };
    result.statement_index = 1;
    assert_eq!(
        rejection_message(&local_coordinate),
        "Unit scalar result local or call coordinate is not canonical"
    );

    let mut result_coordinate = checked();
    let CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } =
        &mut main_operations_mut(&mut result_coordinate)[0]
    else {
        panic!("first operation should bind the scalar result")
    };
    result.binding_ordinal = 1;
    assert_eq!(
        rejection_message(&result_coordinate),
        "Unit scalar result local or call coordinate is not canonical"
    );

    let mut call_coordinate = checked();
    let CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. } =
        &mut main_operations_mut(&mut call_coordinate)[0]
    else {
        panic!("first operation should bind the scalar result")
    };
    coordinate.call_ordinal = 1;
    assert_eq!(
        rejection_message(&call_coordinate),
        "Unit scalar result local or call coordinate is not canonical"
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
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index, ..
    } = &mut operations[2]
    else {
        panic!("third operation should return Unit")
    };
    *statement_index = 4;

    assert_eq!(
        rejection_message(&checked),
        "Unit scalar call coordinate and target do not rejoin its original checked flow call"
    );
}

#[test]
fn attached_unit_scalar_result_type_and_later_local_use_reject_drift() {
    let mut result_type = checked();
    let CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } =
        &mut main_operations_mut(&mut result_type)[0]
    else {
        panic!("first operation should bind the scalar result")
    };
    *result = CheckedUnitScalarResultBindingPlan {
        primitive_type: psi_typed_trees::types::PrimitiveType::U32,
        ..*result
    };
    assert_eq!(
        rejection_message(&result_type),
        "Unit boundary call does not match the exact checked target state, result, contract, and reach"
    );

    let mut local_use = checked();
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        scalar_arguments, ..
    } = &mut main_operations_mut(&mut local_use)[1]
    else {
        panic!("second operation should consume the scalar result")
    };
    let [CheckedScalarExpression::Local { position, .. }] = scalar_arguments.as_mut_slice() else {
        panic!("consumer should use the exact scalar local")
    };
    *position = 1;
    assert_eq!(
        rejection_message(&local_use),
        "scalar graph integer guard parameter type does not match"
    );
}
