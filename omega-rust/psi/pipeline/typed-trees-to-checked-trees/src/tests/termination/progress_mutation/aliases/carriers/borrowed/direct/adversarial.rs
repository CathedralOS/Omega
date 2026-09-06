use super::*;
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

pub(super) fn assert_identity(program: &typed_trees::TypedTrees, local_name: &str, known: bool) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let local = statements
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                Some(local.symbol)
            }
            _ => None,
        })
        .unwrap();
    let resolver = validation::CallFrameResolver::new(program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let query = |resolver: &validation::CallFrameResolver<'_>| {
        resolver.local_reference_origin_before_statement(machine, statements.last().unwrap(), local)
    };
    let origin = query(&resolver);
    assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
    let fresh = validation::CallFrameResolver::new(program).unwrap();
    assert_eq!(query(&fresh), origin, "query order cannot change identity");
    assert_eq!(fresh.inferred_state_write_frame(machine, state), frame);
    if known {
        let (root, segments) = origin.expect("the exact frozen input leaf");
        assert_eq!(root, program.state_parameters(state)[0].symbol);
        let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
            panic!("one declared reference boundary: {segments:?}")
        };
        assert_eq!(
            program.symbols.display_path(*symbol, "::"),
            "Carrier::context"
        );
    } else {
        assert_eq!(origin, None, "{local_name} cannot claim an exact input");
    }
}

// Query borrow-invalid mutations directly; saved loans cannot authorize slot exposure.
#[test]
fn direct_results_require_frozen_slots_even_after_capturing_a_local() {
    for result in ["carrier.context", "captured"] {
        for operation in [
            "",
            "carrier.context = replacement;",
            "carrier = Carrier { context: replacement };",
            "selected.context = replacement;",
            "selected = Carrier { context: replacement };",
            "selected.touch();",
            "let ignored: u64 = selected.touch();",
        ] {
            let source = format!(
                "{} machine Carrier::touch(&mut self) -> u64 {{ 0 }}",
                direct_source(
                    "mut ",
                    &format!(
                        "let selected: &mut Carrier = carrier;
                     let captured: &Context = carrier.context; {operation} {result}"
                    )
                )
                .replace(
                    "carrier: &mut Carrier)",
                    "carrier: &mut Carrier, replacement: &Context)"
                )
                .replace("select(carrier)", "select(carrier, replacement)")
            );
            let mut program = typed_source(&source);
            crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
            assert_identity(&program, "borrowed", operation.is_empty());
        }
    }
}

#[test]
fn a_terminal_call_checks_exposure_in_both_operand_orders() {
    for first in [false, true] {
        for operand in [
            "0",
            "carrier.touch()",
            "inspect_context(&mut carrier.context)",
        ] {
            let (parameters, arguments) = if first {
                (
                    "tag: u64, context: &Context",
                    format!("{operand}, carrier.context"),
                )
            } else {
                (
                    "context: &Context, tag: u64",
                    format!("carrier.context, {operand}"),
                )
            };
            let source = format!(
                "{}
                 machine forward({parameters}) -> &Context {{ context }}
                 machine Carrier::touch(&mut self) -> u64 {{ 0 }}
                 machine inspect_context(context: &mut Context) -> u64 {{ 0 }}",
                direct_source("mut ", &format!("forward({arguments})"))
            );
            let mut program = typed_source(&source);
            crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
            assert_identity(&program, "borrowed", operand == "0");
            if operand == "0" {
                assert_input_premise(&check_source(&source));
            }
        }
    }
}

#[test]
fn direct_results_require_selected_call_and_parameter_symbols() {
    for corrupt_call in [false, true] {
        for missing in [false, true] {
            let mut program = typed_source(&format!(
                "{} machine unrelated(carrier: &Carrier) {{}}",
                direct_source("", "carrier.context")
            ));
            assert_identity(&program, "borrowed", true);
            let parameter = |name: &str| {
                let machine = program
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == name)
                    .unwrap();
                program.state_parameters(&program.machine_states(machine)[0])[0].symbol
            };
            let original = parameter("select");
            // A real symbol from another scope is no more authoritative than
            // a missing one, even though both parameters are named carrier.
            let replacement = if missing {
                SymbolHandle::invalid()
            } else {
                parameter("unrelated")
            };
            let expressions = program
                .expression_table
                .iter_expressions()
                .filter_map(|(handle, node)| match node {
                    ExpressionNode::Call(call)
                        if corrupt_call && call.target.as_str() == "select" =>
                    {
                        Some(handle)
                    }
                    ExpressionNode::Name(name) if !corrupt_call && name.head_symbol == original => {
                        Some(handle)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(expressions.len(), 1);
            match program.expression_table.expression_mut(expressions[0]) {
                ExpressionNode::Call(call) => call.target_symbol = replacement,
                ExpressionNode::Name(name) => name.head_symbol = replacement,
                _ => unreachable!(),
            }
            assert_identity(&program, "borrowed", false);
        }
    }
}

#[test]
fn private_referents_cannot_escape_as_exact_caller_inputs() {
    let local = direct_source(
        "",
        "let local: Context = Context {
        scheduler: SchedulerHandle {}, counter: 0 }; &local",
    );
    let owned = direct_source("", "&carrier.private")
        .replace("carrier: &Carrier)", "carrier: Carrier)")
        .replace(
            "data Carrier { context: &Context; }",
            "data Carrier { context: &Context; private: Context; }",
        );
    // Invalid escapes cannot use the sibling reference field's input correspondence.
    for source in [local, owned] {
        assert_identity(&typed_source(&source), "borrowed", false);
    }
}

#[test]
fn an_unknown_direct_result_and_its_copy_leave_a_known_query_independent() {
    let source = direct_source(
        "",
        "transition carrier.context.counter == 0 {
        true -> carrier.context false -> carrier.context }",
    )
    .replace(
        "let borrowed: &Context = select(carrier);",
        "let unknown: &Context = select(carrier);
             let copied: &Context = unknown;
             let borrowed: &Context = carrier.context;",
    );
    let program = typed_source(&source);
    for (local, known) in [("unknown", false), ("copied", false), ("borrowed", true)] {
        assert_identity(&program, local, known);
    }
    assert_input_premise(&check_source(&source));
}

#[test]
fn recursive_result_bodies_are_opaque_but_finite_actual_nesting_is_exact() {
    for (body, extra) in [
        ("select(carrier)", ""),
        (
            "again(carrier)",
            "machine again(carrier: &Carrier) -> &Context { select(carrier) }",
        ),
    ] {
        let source = format!("{} {extra}", direct_source("", body));
        assert_identity(&typed_source(&source), "borrowed", false);
    }
    for (body, nested_argument) in [
        ("forward(carrier.context)", false),
        ("forward(forward(carrier.context))", true),
    ] {
        let source = format!(
            "{} machine forward(context: &Context) -> &Context {{ context }}",
            direct_source("", body)
        );
        assert_identity(&typed_source(&source), "borrowed", true);
        if nested_argument {
            // Exact provenance does not remove the existing realization fence
            // on a machine call used directly as another call's argument.
            let diagnostics = lower_typed_trees(typed_source(&source))
                .expect_err("nested call arguments still require realization support");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("value-call argument cannot itself be a machine call yet")),
                "{diagnostics:#?}"
            );
        } else {
            assert_input_premise(&check_source(&source));
        }
    }
}

#[test]
fn a_pure_terminal_exclusive_reborrow_keeps_its_direct_subject() {
    for body in [
        "&mut context",
        "let selected: &mut Context = context; &mut selected",
    ] {
        let source = source(
            "mut ",
            "mut ",
            "let borrowed: &mut Context = reborrow(carrier.context);
             transition { _ -> wait_context(borrowed) }",
            &format!("machine reborrow(context: &mut Context) -> &mut Context {{ {body} }}"),
        );
        assert_identity(&typed_source(&source), "borrowed", true);
        assert_input_premise(&check_source(&source));
    }
}

#[test]
fn an_owned_self_result_cannot_repair_a_foreign_field_from_its_spelling() {
    let source = format!(
        "{} data Foreign {{ context: Context; }}
         machine Carrier::project(&self) -> &Context {{ &self.context }}",
        direct_source("", "carrier.project()").replace("context: &Context;", "context: Context;")
    );
    let mut program = typed_source(&source);
    assert_identity(&program, "borrowed", true);
    let foreign = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Foreign")
        .unwrap()
        .symbol;
    let foreign_field = program
        .symbols
        .find_child_by_name(foreign, "context")
        .unwrap();
    let selected = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Member(member)
            if member.member.as_str() == "context"
                && program.expression_table.display_name(member.receiver) == "self")
            .then_some(handle)
        })
        .unwrap();
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(selected) else {
        unreachable!()
    };
    assert_ne!(member.member_symbol, foreign_field);
    member.member_symbol = foreign_field;
    // Fact-place normalization must not erase a conflicting retained selector.
    assert_identity(&program, "borrowed", false);
}

#[test]
fn a_local_carrier_capture_cannot_hide_binding_exposure_in_its_tag() {
    for tag in ["0", "touch(&mut context)"] {
        let source = source(
            "mut ",
            "mut ",
            "let borrowed: &mut Context = select(carrier.context);
             transition { _ -> wait_context(borrowed) }",
            &format!(
                "data CarrierWithTag {{ context: &mut Context; tag: u64; }}
                 machine touch(context: &mut Context) -> u64 {{ 0 }}
                 machine select(context: &mut Context) -> &mut Context {{
                     let saved: CarrierWithTag = CarrierWithTag {{
                         context: &mut context, tag: {tag}
                     }};
                     saved.context
                 }}"
            ),
        );
        // Typed identity isolates the binding fence from borrow and terminal-type checks.
        assert_identity(&typed_source(&source), "borrowed", tag == "0");
    }
}
