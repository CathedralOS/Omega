use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

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

const DIRECT_STORE_SOURCE: &str = r#"
data Scalar {}

machine Scalar::identity(value: i32) -> i32
requires value == value
ensures result == value
{
    transition { _ -> value }
}

data Main {}

machine Main::main(destination: &write i32) {
    let result: i32 = Scalar::identity(23);
    destination = result;
}
"#;

fn checked_from_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

fn checked() -> checked_trees::CheckedTrees {
    checked_from_source(SOURCE)
}

fn ordinary_checked() -> checked_trees::CheckedTrees {
    checked_from_source(ORDINARY_SOURCE)
}

fn direct_store_checked() -> checked_trees::CheckedTrees {
    checked_from_source(DIRECT_STORE_SOURCE)
}

fn checked_with_dependent_scalar_local() -> checked_trees::CheckedTrees {
    checked_from_source(&SOURCE.replace(
        "let result: i32 = Host::measure(70);\n    Host::finish(result);",
        "let measured: i32 = Host::measure(70);\n    let result: i32 = measured + 0i32;\n    Host::finish(result);",
    ))
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
    match checked_trees_to_lowered_psi::lower_machine(checked, "Main::main") {
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(message)) => message,
        result => panic!("tampered scalar result flow should reject, got {result:?}"),
    }
}

#[test]
fn attached_unit_scalar_boundary_result_reaches_later_call_in_terminal_psi() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked(), "Main::main")
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
                terminal_psi::OperationKind::BoundaryCall { .. }
            )
        })
        .collect::<Vec<_>>();
    let [producer, consumer] = boundary_calls.as_slice() else {
        panic!("entry should retain one scalar producer and one Unit consumer")
    };
    let terminal_psi::OperationResult::Scalar(result) = producer.result else {
        panic!("first boundary call should publish its scalar result")
    };
    let terminal_psi::OperationKind::BoundaryCall { arguments, .. } = &consumer.kind else {
        unreachable!()
    };
    assert_eq!(arguments, &[result.id]);
    assert!(matches!(
        consumer.result,
        terminal_psi::OperationResult::Unit
    ));
}

#[test]
fn attached_unit_ordinary_scalar_result_reaches_later_call_in_terminal_psi() {
    let checked = ordinary_checked();
    let operations = &checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main_symbol(&checked))
        .expect("Main::main Unit plan")
        .operations;
    assert!(matches!(
        operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::ScalarCall {
                result,
                scalar_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::CallUnit {
                scalar_arguments: consumer_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] if result.binding_ordinal == 0
            && matches!(
                scalar_arguments.as_slice(),
                [CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral { .. })]
            )
            && matches!(
                consumer_arguments.as_slice(),
                [CheckedCallScalarArgument::Pure(CheckedScalarExpression::Local { position: 0, .. })]
            )
    ));

    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("ordinary scalar result flow should lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let calls = entry.blocks[0]
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::Call { .. }
                    | terminal_psi::OperationKind::CallUnit { .. }
            )
        })
        .collect::<Vec<_>>();
    let [producer, consumer] = calls.as_slice() else {
        panic!("entry should retain the scalar producer and Unit consumer")
    };
    let terminal_psi::OperationResult::Scalar(result) = producer.result else {
        panic!("ordinary call should publish its scalar result")
    };
    assert!(matches!(
        producer.kind,
        terminal_psi::OperationKind::Call { .. }
    ));
    let terminal_psi::OperationKind::CallUnit { arguments, .. } = &consumer.kind else {
        panic!("consumer should remain an ordinary Unit call")
    };
    assert_eq!(arguments, &[result.id]);
    assert!(matches!(
        consumer.result,
        terminal_psi::OperationResult::Unit
    ));
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
fn attached_unit_ordinary_scalar_result_reaches_a_direct_write_only_store() {
    let checked = direct_store_checked();
    let operations = &checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main_symbol(&checked))
        .expect("Main::main Unit plan")
        .operations;
    assert!(matches!(
        operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::ScalarCall { result, .. },
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index: 1,
                value: CheckedScalarExpression::Local {
                    position: 0,
                    primitive_type: typed_trees::types::PrimitiveType::I32,
                },
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { statement_index: 2, .. },
        ] if result.binding_ordinal == 0
    ));

    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("direct scalar-result store should lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let producer = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, terminal_psi::OperationKind::Call { .. }))
        .expect("ordinary scalar producer");
    let terminal_psi::OperationResult::Scalar(result) = producer.result else {
        panic!("ordinary scalar producer should publish a result")
    };
    let store = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .expect("direct write-only store");
    let terminal_psi::OperationKind::WriteOnlyPrimitiveStore { value, .. } = store.kind else {
        unreachable!()
    };
    assert_eq!(value, result.id);
}

#[test]
fn attached_unit_scalar_expression_local_reaches_later_call_in_terminal_psi() {
    let checked = checked_with_dependent_scalar_local();
    let operations = &checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main_symbol(&checked))
        .expect("Main::main Unit plan")
        .operations;
    assert!(matches!(
        operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::BoundaryScalarCall { result: measured, .. },
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
            CheckedUnitEffectOperationPlan::BoundaryCall { scalar_arguments, .. },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] if measured.binding_ordinal == 0
            && result.binding_ordinal == 1
            && matches!(
                value,
                CheckedScalarExpression::IntegerBinary {
                    kind: checked_trees::CheckedIntegerBinaryKind::ExactAdd,
                    left,
                    ..
                } if matches!(
                    left.as_ref(),
                    CheckedScalarExpression::Local { position: 0, .. }
                )
            )
            && matches!(
                scalar_arguments.as_slice(),
                [CheckedCallScalarArgument::Pure(CheckedScalarExpression::Local { position: 1, .. })]
            )
    ));

    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("dependent scalar-local flow should lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let producer = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            ) && matches!(operation.result, terminal_psi::OperationResult::Scalar(_))
        })
        .expect("scalar boundary producer");
    let terminal_psi::OperationResult::Scalar(measured) = producer.result else {
        unreachable!()
    };
    let exact_add = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::ExactIntegerAdd { .. }
            )
        })
        .expect("dependent scalar expression");
    let terminal_psi::OperationKind::ExactIntegerAdd { left, .. } = exact_add.kind else {
        unreachable!()
    };
    assert_eq!(left, measured.id);
    let terminal_psi::OperationResult::Scalar(result) = exact_add.result else {
        panic!("dependent exact add should publish its scalar result")
    };
    let consumer = entry.blocks[0]
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            ) && matches!(operation.result, terminal_psi::OperationResult::Unit)
        })
        .expect("Unit boundary consumer");
    let terminal_psi::OperationKind::BoundaryCall { arguments, .. } = &consumer.kind else {
        unreachable!()
    };
    assert_eq!(arguments, &[result.id]);
}

#[test]
fn attached_unit_scalar_expression_local_rejects_checked_fact_drift() {
    let mut checked = checked_with_dependent_scalar_local();
    let CheckedUnitEffectOperationPlan::EstablishScalarLocal { value, .. } =
        &mut main_operations_mut(&mut checked)[1]
    else {
        panic!("second operation should establish the dependent scalar local")
    };
    *value = CheckedScalarExpression::Local {
        position: 0,
        primitive_type: typed_trees::types::PrimitiveType::I32,
    };

    assert_eq!(
        rejection_message(&checked),
        "Unit scalar expression local drifted from its checked value fact"
    );
}

#[test]
fn attached_unit_scalar_expression_local_rejects_source_coordinate_drift() {
    let mut checked = checked_with_dependent_scalar_local();
    let CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } =
        &mut main_operations_mut(&mut checked)[1]
    else {
        panic!("second operation should establish the dependent scalar local")
    };
    result.statement_index = 2;

    assert_eq!(
        rejection_message(&checked),
        "Unit machine operation order is not canonical source order"
    );
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
        "scalar source custody has no authored statement"
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
        primitive_type: typed_trees::types::PrimitiveType::U32,
        ..*result
    };
    assert_eq!(
        rejection_message(&result_type),
        "call result binding disagrees with its authored scalar local"
    );

    let mut local_use = checked();
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        scalar_arguments, ..
    } = &mut main_operations_mut(&mut local_use)[1]
    else {
        panic!("second operation should consume the scalar result")
    };
    let [CheckedCallScalarArgument::Pure(CheckedScalarExpression::Local { position, .. })] =
        scalar_arguments.as_mut_slice()
    else {
        panic!("consumer should use the exact scalar local")
    };
    *position = 1;
    assert_eq!(
        rejection_message(&local_use),
        "call scalar operand disagrees with its authored argument"
    );
}
