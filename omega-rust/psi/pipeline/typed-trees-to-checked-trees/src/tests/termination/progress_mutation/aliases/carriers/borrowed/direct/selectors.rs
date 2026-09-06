use super::*;
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

fn assert_nested_identity(program: &typed_trees::TypedTrees, known: bool) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(borrowed) = &statements[0] else {
        panic!("the caller's selected reference")
    };
    let resolver = validation::CallFrameResolver::new(program).unwrap();
    let frame = resolver.inferred_state_write_frame(machine, state);
    let origin = resolver.local_reference_origin_before_statement(
        machine,
        statements.last().unwrap(),
        borrowed.symbol,
    );
    assert_eq!(resolver.inferred_state_write_frame(machine, state), frame);
    if known {
        let (root, segments) = origin.expect("the helper's exact owned field projection");
        assert_eq!(root, program.state_parameters(state)[0].symbol);
        let [
            facts::PlaceSegment::Field { symbol: inner },
            facts::PlaceSegment::Field { symbol: context },
        ] = segments.as_slice()
        else {
            panic!("two exact owned fields: {segments:?}")
        };
        assert_eq!(program.symbols.display_path(*inner, "::"), "Carrier::inner");
        assert_eq!(
            program.symbols.display_path(*context, "::"),
            "Inner::context"
        );
    } else {
        assert_eq!(
            origin, None,
            "a corrupted nested selector cannot identify an input"
        );
    }
}

fn assert_selector_corruptions(body: &str, receiver: &str) {
    for missing in [false, true] {
        let source = format!(
            "{}
             data Inner {{ context: Context; }}
             data Foreign {{ context: Context; }}
             machine Carrier::project(&self) -> &Context {{ {body} }}",
            direct_source("", "carrier.project()")
                .replace(
                    "data Carrier { context: &Context; }",
                    "data Carrier { inner: Inner; }"
                )
                .replace(
                    "carrier.context.scheduler",
                    "carrier.inner.context.scheduler"
                )
        );
        let mut program = typed_source(&source);
        assert_nested_identity(&program, true);
        let foreign = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Foreign")
            .unwrap()
            .symbol;
        let replacement = if missing {
            SymbolHandle::invalid()
        } else {
            program
                .symbols
                .find_child_by_name(foreign, "context")
                .unwrap()
        };
        let selected = program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, node)| {
                let ExpressionNode::Member(member) = node else {
                    return None;
                };
                (member.member.as_str() == "context"
                    && program.expression_table.display_name(member.receiver) == receiver)
                    .then_some(handle)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected.len(),
            1,
            "only the helper's {receiver}.context selection"
        );
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(selected[0])
        else {
            unreachable!()
        };
        // Missing ordinary member symbols are legal at this stage; exact
        // nominal projection resolves them. A conflicting retained selector
        // must not be repaired by that same normalization.
        member.member_symbol = replacement;
        assert_nested_identity(&program, missing);
    }
}

#[test]
fn nested_self_results_require_the_retained_outer_field_identity() {
    assert_selector_corruptions("&self.inner.context", "self.inner");
}

#[test]
fn an_alias_to_self_inner_cannot_repair_a_foreign_field() {
    assert_selector_corruptions(
        "let selected: &Inner = &self.inner; &selected.context",
        "selected",
    );
}
