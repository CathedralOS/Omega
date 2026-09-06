use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedScalarExpressionBindings,
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::statement::StatementNode;

const BOUNDARY_SOURCE: &str = r#"
    boundary trait Host {
        machine measure(first: i32, second: i32) -> i32 reaches Host;
        machine finish(first: i32, second: i32) reaches Host;
    }
    data Main {}
    machine Main::main(left: i32, right: i32)
    reaches Host
    {
        let measured: i32 = Host::measure(left, right);
        Host::finish(measured, left);
    }
"#;

const UNIT_SOURCE: &str = r#"
    boundary trait Host {
        machine finish(first: i32, second: i32) reaches Host;
    }
    data Scalar {}
    machine Scalar::first(left: i32, right: i32) -> i32
    requires 0i32 == 0i32
    ensures result == left
    { left }
    data Sink {}
    machine Sink::finish(first: i32, second: i32)
    reaches Host
    { Host::finish(first, second); }
    data Main {}
    machine Main::main(left: i32, right: i32)
    reaches Host
    {
        let measured: i32 = Scalar::first(left, right);
        Sink::finish(measured, right);
    }
"#;

const BOUNDARY_RETURN_SOURCE: &str = r#"
    boundary trait Host {
        machine measure(first: i32, second: i32) -> i32 reaches Host;
    }
    data Main {}
    machine Main::main() -> i32
    reaches Host
    {
        let measured: i32 = Host::measure(23i32, 70i32);
        measured
    }
"#;

const CALLABLE_BOUNDARY_SOURCE: &str = r#"
    boundary trait Host {
        machine finish(first: i32, second: i32) reaches Host;
    }
    data Main {}
    machine Main::main<machine Finish>(left: i32, right: i32)
    where machine Finish satisfies Host::finish;
    reaches Host
    {
        Finish(left, right);
    }
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn encoded(checked: &checked_trees::CheckedTrees) -> (Vec<u8>, Vec<u8>) {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("supported source lowers");
    let artifact = (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    );
    let module = decode_module(&artifact.0).unwrap();
    let proof = decode_proof_bundle(&artifact.1).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("fresh decoded verification");
    artifact
}

fn rows(
    checked: &checked_trees::CheckedTrees,
) -> Vec<(
    arena::Handle<CheckedScalarExpressionBindings>,
    CheckedScalarExpressionBindings,
)> {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .filter(|(_, row)| {
            states.iter().any(|state| state.symbol == row.state)
                && matches!(
                    row.role,
                    CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                        | CheckedScalarExpressionRole::UnitCallArgument { .. }
                )
        })
        .map(|(handle, row)| (handle, row.clone()))
        .collect()
}

fn signed(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(value),
    }
}

#[derive(Default)]
struct ObserveArguments {
    calls: Vec<Vec<TerminalScalarValue>>,
}

impl TerminalEffectHandler for ObserveArguments {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("boundary effect");
        };
        self.calls.push(arguments.clone());
        Ok(())
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<Option<TerminalScalarValue>, TerminalEffectRejection> {
        self.handle_effect(effect)?;
        let TerminalEffect::BoundaryCall {
            arguments, result, ..
        } = effect
        else {
            unreachable!();
        };
        Ok(match result {
            terminal_psi::BoundaryMachineResult::Scalar(_) => Some(arguments[1]),
            terminal_psi::BoundaryMachineResult::Unit => None,
            _ => panic!("scalar or Unit boundary result"),
        })
    }
}

#[test]
fn boundary_and_unit_scalar_arguments_keep_their_authored_values_after_roundtrip() {
    for (source, parameters, expected_result, expected_calls) in [
        (
            BOUNDARY_SOURCE,
            vec![signed(23), signed(70)],
            TerminalExecutionResult::Unit,
            vec![vec![signed(23), signed(70)], vec![signed(70), signed(23)]],
        ),
        (
            UNIT_SOURCE,
            vec![signed(23), signed(70)],
            TerminalExecutionResult::Unit,
            vec![vec![signed(23), signed(70)]],
        ),
        (
            BOUNDARY_RETURN_SOURCE,
            Vec::new(),
            TerminalExecutionResult::Scalar(signed(70)),
            vec![vec![signed(23), signed(70)]],
        ),
        (
            CALLABLE_BOUNDARY_SOURCE,
            vec![signed(23), signed(70)],
            TerminalExecutionResult::Unit,
            vec![vec![signed(23), signed(70)]],
        ),
    ] {
        let checked = checked(source);
        assert!(
            !rows(&checked).is_empty(),
            "call operands have authored bindings"
        );
        let artifact = encoded(&checked);
        let mut handler = ObserveArguments::default();
        let execution = interpret_terminal_artifact_with_effect_handler_measured(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &parameters,
            &[],
            &mut handler,
        )
        .unwrap();
        assert_eq!(execution.value(), expected_result);
        assert_eq!(handler.calls, expected_calls);
    }
}

fn replace_source_argument(
    checked: &mut checked_trees::CheckedTrees,
    row: &CheckedScalarExpressionBindings,
    replacement: typed_trees::expression::ExpressionHandle,
) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let state = checked
        .typed
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == row.state)
        .unwrap();
    let statement = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)[row.statement_ordinal as usize]
        .clone();
    match statement {
        StatementNode::Call(call) => {
            let arguments = checked
                .typed
                .statement_table
                .expression_handles(call.arguments);
            let index = arguments
                .iter()
                .position(|argument| *argument == row.expression)
                .expect("authored argument in statement call");
            checked
                .typed
                .statement_table
                .set_expression_handle_at_offset(call.arguments, index as u32, replacement);
        }
        StatementNode::LocalData(local) => replace_expression_call_argument(
            checked,
            local.initial_value,
            row.expression,
            replacement,
        ),
        StatementNode::Expression(expression) => {
            replace_expression_call_argument(checked, expression, row.expression, replacement)
        }
        _ => panic!("supported top-level call spelling"),
    }
}

fn replace_expression_call_argument(
    checked: &mut checked_trees::CheckedTrees,
    call_expression: typed_trees::expression::ExpressionHandle,
    original: typed_trees::expression::ExpressionHandle,
    replacement: typed_trees::expression::ExpressionHandle,
) {
    let typed_trees::expression::ExpressionNode::Call(call) =
        checked.typed.expression_table.expression(call_expression)
    else {
        panic!("bare expression call");
    };
    let arguments = call.arguments;
    let index = checked
        .typed
        .expression_table
        .expression_handles(arguments)
        .iter()
        .position(|argument| *argument == original)
        .unwrap();
    checked
        .typed
        .expression_table
        .set_expression_handle_at_offset(arguments, index as u32, replacement);
}

fn arguments_mut(
    operation: &mut CheckedUnitEffectOperationPlan,
) -> Option<(u32, &mut Vec<CheckedCallScalarArgument>)> {
    match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            scalar_arguments,
            ..
        } => Some((coordinate.statement_index, scalar_arguments)),
        _ => None,
    }
}

fn replace_operation_argument(
    checked: &mut checked_trees::CheckedTrees,
    row: &CheckedScalarExpressionBindings,
    value: CheckedScalarExpression,
) {
    let ordinal = match row.role {
        CheckedScalarExpressionRole::BoundaryCallArgument {
            argument_ordinal, ..
        }
        | CheckedScalarExpressionRole::UnitCallArgument {
            argument_ordinal, ..
        } => argument_ordinal as usize,
        _ => unreachable!(),
    };
    let mut replaced = 0;
    for plan in &mut checked.facts.flow.terminal_unit_effects.machines {
        if plan.state != row.state {
            continue;
        }
        for operation in &mut plan.operations {
            if let Some((statement, arguments)) = arguments_mut(operation)
                && statement == row.statement_ordinal
            {
                arguments[ordinal] = CheckedCallScalarArgument::Pure(value.clone());
                replaced += 1;
            }
        }
    }
    for plan in &mut checked.facts.flow.terminal_boundary_scalar_returns.machines {
        if plan.state == row.state {
            let (statement, arguments) = arguments_mut(&mut plan.boundary_call).unwrap();
            if statement == row.statement_ordinal {
                arguments[ordinal] = CheckedCallScalarArgument::Pure(value.clone());
                replaced += 1;
            }
        }
    }
    assert!(replaced > 0, "operation carries the argument copy");
}

fn mutate_operation_custody(operation: &mut CheckedUnitEffectOperationPlan, mutation: u8) -> bool {
    let (coordinate, target_state, source_site) = match operation {
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            target_state,
            source_site,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            target_state,
            source_site,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            target_state,
            source_site,
            ..
        } => (coordinate, target_state, Some(source_site)),
        CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            target_state,
            ..
        }
        | CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            target_state,
            ..
        } => (coordinate, target_state, None),
        _ => return false,
    };
    match mutation {
        0 => coordinate.call_ordinal += 1,
        1 => *target_state = symbols::SymbolHandle::invalid(),
        2 | 3 => {
            let Some(source_site) = source_site else {
                return false;
            };
            *source_site = if mutation == 2 {
                None
            } else {
                Some(
                    match source_site.expect("baseline retains the authored boundary site") {
                        checked_trees::NominalMachineUseSite::Statement(handle) => {
                            checked_trees::NominalMachineUseSite::Statement(
                                arena::Handle::from_parts(
                                    handle.arena_index(),
                                    handle.generation() + 1,
                                ),
                            )
                        }
                        checked_trees::NominalMachineUseSite::Expression(handle) => {
                            checked_trees::NominalMachineUseSite::Expression(
                                arena::Handle::from_parts(
                                    handle.arena_index(),
                                    handle.generation() + 1,
                                ),
                            )
                        }
                    },
                )
            };
        }
        _ => unreachable!(),
    }
    true
}

fn assert_operation_custody(checked: &checked_trees::CheckedTrees) {
    let main = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap()
        .symbol;
    for (plan_index, plan) in checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .enumerate()
    {
        if plan.machine != main {
            continue;
        }
        for operation_index in 0..plan.operations.len() {
            for mutation in 0..4 {
                let mut changed = checked.clone();
                let operation = &mut changed.facts.flow.terminal_unit_effects.machines[plan_index]
                    .operations[operation_index];
                if mutate_operation_custody(operation, mutation) {
                    assert!(
                        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main")
                            .is_err(),
                        "operation={operation_index}, outer custody mutation={mutation}"
                    );
                }
            }
        }
    }
    for (plan_index, plan) in checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .iter()
        .enumerate()
    {
        if plan.machine != main {
            continue;
        }
        for mutation in 0..4 {
            let mut changed = checked.clone();
            let operation = &mut changed.facts.flow.terminal_boundary_scalar_returns.machines
                [plan_index]
                .boundary_call;
            assert!(mutate_operation_custody(operation, mutation));
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "boundary return outer custody mutation={mutation}"
            );
        }
    }
}

#[test]
fn checked_call_operations_reject_changed_ordinals_targets_and_boundary_sites() {
    for source in [
        BOUNDARY_SOURCE,
        UNIT_SOURCE,
        BOUNDARY_RETURN_SOURCE,
        CALLABLE_BOUNDARY_SOURCE,
    ] {
        let checked = checked(source);
        encoded(&checked);
        assert_operation_custody(&checked);
    }
}

#[test]
fn call_scalar_binding_stamps_reject_missing_duplicate_stale_and_reordered_sources() {
    for source in [
        BOUNDARY_SOURCE,
        UNIT_SOURCE,
        BOUNDARY_RETURN_SOURCE,
        CALLABLE_BOUNDARY_SOURCE,
    ] {
        let checked = checked(source);
        encoded(&checked);
        let rows = rows(&checked);
        assert!(rows.len() >= 2);
        for (handle, row) in &rows {
            for mutation in 0..5 {
                let mut changed = checked.clone();
                let plans = &mut changed.facts.values.scalar_expressions;
                match mutation {
                    0 => {
                        let mut retained = arena::Arena::new();
                        for (other, row) in plans.source_bindings.iter() {
                            if other != *handle {
                                retained.append(row.clone());
                            }
                        }
                        plans.source_bindings = retained;
                    }
                    1 => {
                        plans.source_bindings.append(row.clone());
                    }
                    2 => {
                        plans.source_bindings.get_mut(*handle).expression = arena::Handle::invalid()
                    }
                    3 => plans.source_bindings.get_mut(*handle).statement_ordinal += 100,
                    4 => plans.source_bindings.get_mut(*handle).destination = row.state,
                    _ => unreachable!(),
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "mutation={mutation}, role={:?}",
                    row.role
                );
            }
            let symbols = checked
                .facts
                .values
                .scalar_expressions
                .binding_symbols
                .span_or_empty(row.symbols);
            if symbols.len() >= 2 {
                let mut changed = checked.clone();
                let mut reordered = symbols.to_vec();
                reordered.swap(0, 1);
                let plans = &mut changed.facts.values.scalar_expressions;
                let replacement = plans.binding_symbols.insert_many(reordered);
                plans.source_bindings.get_mut(*handle).symbols = replacement;
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                    "caller namespace order is source custody"
                );
            }
        }
    }
}

#[test]
fn same_carrier_call_operands_cannot_swap_source_handles_or_coordinate_copies() {
    for source in [
        BOUNDARY_SOURCE,
        UNIT_SOURCE,
        BOUNDARY_RETURN_SOURCE,
        CALLABLE_BOUNDARY_SOURCE,
    ] {
        let checked = checked(source);
        encoded(&checked);
        let rows = rows(&checked);
        for (handle, row) in &rows {
            let (_, other) = rows
                .iter()
                .find(|(other, candidate)| {
                    other != handle && candidate.statement_ordinal == row.statement_ordinal
                })
                .unwrap();
            assert_ne!(row.expression, other.expression);
            let mut changed = checked.clone();
            replace_source_argument(&mut changed, row, other.expression);
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "authored argument handle swapped, role={:?}",
                row.role
            );

            let replacement = checked
                .facts
                .values
                .scalar_expressions
                .expressions
                .iter()
                .find(|fact| {
                    fact.state == other.state
                        && fact.statement_ordinal == other.statement_ordinal
                        && fact.role == other.role
                })
                .unwrap()
                .expression
                .clone();
            let mut changed = checked.clone();
            let facts = &mut changed.facts.values.scalar_expressions;
            facts
                .expressions
                .iter_mut()
                .find(|fact| {
                    fact.state == row.state
                        && fact.statement_ordinal == row.statement_ordinal
                        && fact.role == row.role
                })
                .unwrap()
                .expression = replacement.clone();
            facts.source_bindings.get_mut(*handle).expression = other.expression;
            replace_operation_argument(&mut changed, row, replacement);
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "coordinated checked copies do not change the authored operand"
            );
        }
    }
}

#[test]
fn mixed_callee_parameters_keep_dense_scalar_roles_at_authored_argument_positions() {
    for boundary in [false, true] {
        let declaration = if boundary {
            "boundary trait Sink { machine finish(first: i32, token: Token, second: i32) reaches Sink; }"
        } else {
            "data Sink {} machine Sink::finish(first: i32, token: Token, second: i32) {}"
        };
        let reach = if boundary { "reaches Sink" } else { "" };
        let source = format!(
            r#"
            pub data Token {{}}
            {declaration}
            data Main {{}}
            machine Main::main(token: Token, left: i32, right: i32)
            {reach}
            {{ Sink::finish(left, token, right); }}
        "#
        );
        let checked = checked(&source);
        encoded(&checked);
        let rows = rows(&checked);
        assert_eq!(
            rows.len(),
            2,
            "structural argument has no scalar operand row"
        );
        for (ordinal, (_, row)) in rows.iter().enumerate() {
            let scalar_ordinal = match row.role {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    argument_ordinal, ..
                }
                | CheckedScalarExpressionRole::UnitCallArgument {
                    argument_ordinal, ..
                } => argument_ordinal,
                _ => unreachable!(),
            };
            assert_eq!(
                scalar_ordinal as usize, ordinal,
                "dense scalar callee positions"
            );
            let symbols = checked
                .facts
                .values
                .scalar_expressions
                .binding_symbols
                .span_or_empty(row.symbols);
            assert_eq!(
                symbols.len(),
                2,
                "caller namespace excludes the structural token"
            );
            let other = &rows[1 - ordinal].1;
            let mut changed = checked.clone();
            replace_source_argument(&mut changed, row, other.expression);
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
                "same-carrier source argument swap across structural slot, boundary={boundary}"
            );
        }
    }
}

#[test]
fn zero_scalar_call_roots_reject_same_signature_authored_target_substitution() {
    for boundary in [false, true] {
        let declaration = if boundary {
            "boundary trait Sink { machine first() reaches Sink; machine second() reaches Sink; }"
        } else {
            "data Sink {} machine Sink::first() {} machine Sink::second() {}"
        };
        let reach = if boundary { "reaches Sink" } else { "" };
        let source = format!(
            "{declaration} data Main {{}} \
             machine Main::main() {reach} {{ Sink::first(); }} \
             machine Main::alternative() {reach} {{ Sink::second(); }}"
        );
        let mut checked = checked(&source);
        encoded(&checked);
        assert_operation_custody(&checked);
        assert!(
            rows(&checked).is_empty(),
            "outer custody cannot depend on operand iteration"
        );
        let alternative_machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::alternative")
            .unwrap();
        let alternative_statements =
            checked.typed.machine_states(alternative_machine)[0].statement_nodes;
        let StatementNode::Call(alternative_call) = &checked
            .typed
            .statement_table
            .statements(alternative_statements)[0]
        else {
            panic!("alternate source root retains its resolved zero-argument call");
        };
        let alternative = alternative_call.target_symbol;
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .unwrap();
        let statements = checked.typed.machine_states(machine)[0].statement_nodes;
        let main_symbol = machine.symbol;
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main_symbol)
            .unwrap();
        let operation = plan.operations.first_mut().unwrap();
        match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { target_state, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { target_state, .. } => {
                *target_state = alternative;
            }
            _ => panic!("zero-argument call operation"),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "same-signature operation target state substitution, boundary={boundary}"
        );
        let StatementNode::Call(call) =
            &mut checked.typed.statement_table.statements_mut(statements)[0]
        else {
            panic!("top-level zero-argument call");
        };
        call.target_symbol = alternative;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").is_err(),
            "zero-operand outer target custody, boundary={boundary}"
        );
    }
}

#[test]
fn structural_boundary_results_retain_exact_scalar_operand_source_custody() {
    let source = r#"
        pub data ByteRead {
            case Eof;
            case Byte(value: i32);
        }
        boundary trait Host {
            machine read(first: i32, second: i32) -> ByteRead reaches Host;
        }
        data Main {}
        machine Main::main(left: i32, right: i32)
        reaches Host
        { let result: ByteRead = Host::read(left, right); }
    "#;
    let checked = checked(source);
    encoded(&checked);
    assert_operation_custody(&checked);
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .flat_map(|machine| &machine.operations)
            .any(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
            )),
        "test exercises structural-result admission"
    );
    let rows = rows(&checked);
    assert_eq!(rows.len(), 2);
    for (handle, row) in &rows {
        let other = &rows.iter().find(|(other, _)| other != handle).unwrap().1;
        let mut changed = checked.clone();
        replace_source_argument(&mut changed, row, other.expression);
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "structural result call source operand swapped"
        );
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .get_mut(*handle)
            .expression = arena::Handle::invalid();
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "structural result call binding lost"
        );
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .append(row.clone());
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "structural result duplicate source binding"
        );
    }
}

#[test]
fn composed_unit_boundary_leaves_verify_and_reject_changed_operand_custody() {
    let source = r#"
        boundary trait Host { machine exit(first: i32, second: i32); }
        data Main {}
        machine Main::main(first: bool, second: bool) {
            transition first { true -> dispatch(second) _ -> outer_no() }
            state dispatch(second: bool) {
                transition second { true -> inner_yes() _ -> inner_no() }
            }
            state inner_yes() { Host::exit(1i32, 11i32); }
            state inner_no() { Host::exit(2i32, 22i32); }
            state outer_no() { Host::exit(3i32, 33i32); }
        }
    "#;
    let checked = checked(source);
    let artifact = encoded(&checked);
    assert!(
        !checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .is_empty(),
        "composed admission path"
    );
    for first in [false, true] {
        for second in [false, true] {
            let mut handler = ObserveArguments::default();
            let execution = interpret_terminal_artifact_with_effect_handler_measured(
                &artifact.0,
                &artifact.1,
                &AdmissionProfile::default(),
                &[
                    TerminalScalarValue::Boolean(first),
                    TerminalScalarValue::Boolean(second),
                ],
                &[],
                &mut handler,
            )
            .unwrap();
            assert_eq!(execution.value(), TerminalExecutionResult::Unit);
            let result = if !first {
                3
            } else if second {
                1
            } else {
                2
            };
            assert_eq!(
                handler.calls,
                vec![vec![signed(result), signed(result * 11)]]
            );
        }
    }
    let rows = rows(&checked);
    assert_eq!(
        rows.len(),
        6,
        "two exact scalar operands at each of three leaves"
    );
    for (handle, row) in &rows {
        let other = &rows
            .iter()
            .find(|(other, candidate)| other != handle && candidate.state == row.state)
            .unwrap()
            .1;
        let mut changed = checked.clone();
        replace_source_argument(&mut changed, row, other.expression);
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "composed leaf source operand swapped"
        );
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .get_mut(*handle)
            .expression = arena::Handle::invalid();
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "composed leaf binding lost"
        );
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_expressions
            .source_bindings
            .append(row.clone());
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "composed leaf duplicate binding"
        );
    }
}
