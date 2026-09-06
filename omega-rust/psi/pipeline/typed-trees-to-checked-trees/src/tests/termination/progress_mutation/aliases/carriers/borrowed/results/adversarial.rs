use super::*;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn result_origin(
    program: &typed_trees::TypedTrees,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let borrowed = statements
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "borrowed" => {
                Some(local.symbol)
            }
            _ => None,
        })
        .unwrap();
    let resolver = validation::CallFrameResolver::new(program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let origin = resolver.local_reference_origin_before_statement(
        machine,
        statements.last().unwrap(),
        borrowed,
    );
    assert_eq!(
        resolver.inferred_state_write_frame(machine, state),
        frame,
        "result identity discovery cannot change cached ordinary write frames"
    );
    let fresh = validation::CallFrameResolver::new(program).unwrap();
    assert_eq!(
        fresh.local_reference_origin_before_statement(
            machine,
            statements.last().unwrap(),
            borrowed,
        ),
        origin,
        "identity discovery cannot depend on a previously cached write frame"
    );
    assert_eq!(fresh.inferred_state_write_frame(machine, state), frame);
    origin
}

fn assert_original_input(program: &typed_trees::TypedTrees) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let (root, segments) = result_origin(program).expect("one unexposed helper input leaf");
    assert_eq!(
        root,
        program.state_parameters(&program.machine_states(machine)[0])[0].symbol
    );
    let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
        panic!("one declared carrier field: {segments:?}")
    };
    assert_eq!(
        program.symbols.display_path(*symbol, "::"),
        "Carrier::context"
    );
}

fn mutation_source(operation: &str) -> String {
    result_source(
        "mut ",
        &format!("{operation} Carrier {{ context: carrier.context }}"),
    )
    .replace(
        "carrier: &mut Carrier)",
        "carrier: &mut Carrier, replacement: &Context)",
    )
    .replace("rebuild(carrier)", "rebuild(carrier, replacement)")
}

// Query typed identity below borrow checking: these fixtures do not grant
// permission to replace or expose storage while a saved loan remains active.
#[test]
fn helper_carrier_replacement_cannot_export_the_original_input_leaf() {
    assert_original_input(&typed_source(&mutation_source("")));
    let program = typed_source(&mutation_source(
        "carrier = Carrier { context: replacement };",
    ));
    assert_eq!(result_origin(&program), None);
}

#[test]
fn helper_slot_replacement_cannot_export_the_original_input_leaf() {
    assert_original_input(&typed_source(&mutation_source("")));
    let program = typed_source(&mutation_source("carrier.context = replacement;"));
    assert_eq!(result_origin(&program), None);
}

#[test]
fn helper_alias_ancestor_mutation_or_exposure_retires_the_original_carrier_load() {
    for operation in [
        "",
        "selected = Carrier { context: replacement };",
        "selected.touch();",
        "let ignored: u64 = selected.touch();",
    ] {
        let source = format!(
            "{} machine Carrier::touch(&mut self) -> u64 {{ 0 }}",
            mutation_source(&format!(
                "let selected: &mut Carrier = carrier; {operation}"
            ))
        );
        let mut program = typed_source(&source);
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        // The returned literal reads carrier.context, so rejection must follow
        // selected's effects back to the original parameter's reference slot.
        if operation.is_empty() {
            assert_original_input(&program);
        } else {
            assert_eq!(result_origin(&program), None, "{operation}");
        }
    }
}

#[test]
fn helper_captured_result_cannot_bypass_later_source_carrier_exposure() {
    for operation in [
        "",
        "selected.touch();",
        "selected = Carrier { context: replacement };",
        "selected.context = replacement;",
    ] {
        let source = format!(
            "{}
             machine Carrier::touch(&mut self) -> u64 {{ 0 }}",
            result_source(
                "mut ",
                &format!(
                    "let selected: &mut Carrier = carrier;
                     let captured: &Context = carrier.context;
                     let result: Carrier = Carrier {{ context: captured }};
                     {operation}
                     result"
                ),
            )
            .replace(
                "carrier: &mut Carrier)",
                "carrier: &mut Carrier, replacement: &Context)",
            )
            .replace("rebuild(carrier)", "rebuild(carrier, replacement)")
        );
        let mut program = typed_source(&source);
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        // The local result already captured its leaf. Export still requires
        // the original input slot to survive the complete helper body.
        if operation.is_empty() {
            assert_original_input(&program);
        } else {
            assert_eq!(result_origin(&program), None, "{operation}");
        }
    }
}

#[test]
fn helper_terminal_operand_exposure_cannot_export_a_sibling_reference_leaf() {
    for operand in [
        "0",
        "carrier.touch()",
        "inspect_context(&mut carrier.context)",
    ] {
        let source = format!(
            "{}
             machine Carrier::touch(&mut self) -> u64 {{ 0 }}
             machine inspect_context(context: &mut Context) -> u64 {{ 0 }}",
            result_source(
                "mut ",
                &format!("Carrier {{ context: carrier.context, tag: {operand} }}"),
            )
            .replace(
                "data Carrier { context: &Context; }",
                "data Carrier { context: &Context; tag: u64; }",
            )
        );
        let mut program = typed_source(&source);
        crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
        // Both exposing helpers have empty bodies' write frames. The terminal
        // operand still participates in the frozen reference binding fence.
        if operand == "0" {
            assert_original_input(&program);
        } else {
            assert_eq!(result_origin(&program), None, "{operand}");
        }
    }
}

#[test]
fn an_unresolved_rebuild_target_cannot_recover_result_identity_from_its_name() {
    let mut program = typed_source(&result_source("", "Carrier { context: carrier.context }"));
    assert_original_input(&program);
    let calls = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            matches!(node, ExpressionNode::Call(call) if call.target.as_str() == "rebuild")
                .then_some(expression)
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let ExpressionNode::Call(call) = program.expression_table.expression_mut(calls[0]) else {
        unreachable!()
    };
    call.target_symbol = SymbolHandle::invalid();
    assert_eq!(result_origin(&program), None);
}

#[test]
fn erased_or_foreign_helper_input_symbols_cannot_recover_from_matching_names() {
    for erased in [false, true] {
        let mut program = typed_source(&format!(
            "{} machine unrelated(carrier: &Carrier) {{}}",
            result_source("", "Carrier { context: carrier.context }")
        ));
        assert_original_input(&program);
        let parameter_symbol = |name: &str| {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap();
            program.state_parameters(&program.machine_states(machine)[0])[0].symbol
        };
        let original = parameter_symbol("rebuild");
        let replacement = if erased {
            SymbolHandle::invalid()
        } else {
            parameter_symbol("unrelated")
        };
        let roots = program
            .expression_table
            .iter_expressions()
            .filter_map(|(expression, node)| {
                matches!(node, ExpressionNode::Name(name) if name.head_symbol == original)
                    .then_some(expression)
            })
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1, "only the helper's carrier.context root");
        for expression in roots {
            let ExpressionNode::Name(name) = program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            name.head_symbol = replacement;
        }
        assert_eq!(result_origin(&program), None, "erased: {erased}");
    }
}

#[test]
fn helper_reconstruction_cannot_select_a_possible_borrowed_input_case() {
    let source = format!(
        "{} data Choice {{ case Selected(context: &Context); case Empty; }}",
        result_source("", "Carrier { context: carrier.context }")
            .replace("carrier: &Carrier)", "carrier: &Choice)")
            .replace("requires carrier.context.scheduler in WeakFair", "")
    );
    assert_eq!(result_origin(&typed_source(&source)), None);
}

#[test]
fn helper_reconstruction_cannot_cross_an_additional_reference_boundary() {
    for body in [
        "Carrier { context: carrier.inner.context }",
        "let middle: &Carrier = carrier.inner; Carrier { context: middle.context }",
    ] {
        let source = format!(
            "{} data Outer {{ inner: &Carrier; }}",
            result_source("", body)
                .replace("carrier: &Carrier)", "carrier: &Outer)")
                .replace("requires carrier.context.scheduler in WeakFair", "")
        );
        assert_eq!(result_origin(&typed_source(&source)), None, "{body}");
    }
}
