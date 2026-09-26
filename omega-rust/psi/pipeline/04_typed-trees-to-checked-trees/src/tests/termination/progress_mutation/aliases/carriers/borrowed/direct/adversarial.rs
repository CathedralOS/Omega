use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::assert_input_premise;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::direct::direct_source;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::source;
use crate::tests::termination::progress_mutation::check_source;
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
            let mut program = typed_program(&source);
            crate::lookup::resolve_projected_receiver_calls(&mut program).unwrap();
            assert_identity(&program, "borrowed", operand == "0");
            if operand == "0" {
                assert_input_premise(&check_source(&source));
            }
        }
    }
}

#[test]
fn a_conditional_direct_result_and_its_copy_stay_exact_beside_a_known_query() {
    let source = direct_source(
        "",
        "transition carrier.context.counter == 0 {
        true -> carrier.context false -> carrier.context }",
    )
    .replace(
        "let borrowed: &Context = select(carrier);",
        "let conditional: &Context = select(carrier);
             let copied: &Context = conditional;
             let borrowed: &Context = carrier.context;",
    );
    let program = typed_program(&source);
    for local in ["conditional", "copied", "borrowed"] {
        assert_identity(&program, local, true);
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
        assert_identity(&typed_program(&source), "borrowed", false);
    }
    for (body, nested_argument) in [
        ("forward(carrier.context)", false),
        ("forward(forward(carrier.context))", true),
    ] {
        let source = format!(
            "{} machine forward(context: &Context) -> &Context {{ context }}",
            direct_source("", body)
        );
        assert_identity(&typed_program(&source), "borrowed", true);
        if nested_argument {
            // Exact provenance does not remove the existing realization fence
            // on a machine call used directly as another call's argument.
            let diagnostics =
                lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
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
fn an_owned_self_result_cannot_repair_a_foreign_field_from_its_spelling() {
    let source = format!(
        "{} data Foreign {{ context: Context; }}
         machine Carrier::project(&self) -> &Context {{ &self.context }}",
        direct_source("", "carrier.project()").replace("context: &Context;", "context: Context;")
    );
    let mut program = typed_program(&source);
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
        assert_identity(&typed_program(&source), "borrowed", tag == "0");
    }
}
