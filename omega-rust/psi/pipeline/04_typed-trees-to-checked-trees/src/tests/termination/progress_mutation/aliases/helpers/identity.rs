use crate::tests::front_end::typed_program;

use crate::tests::termination::progress_mutation::CONTEXT_FIXTURE;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn a_slice_view_spelling_does_not_replace_a_declared_helper_body() {
    for loaded in [false, true] {
        let field_type = if loaded { "&Context" } else { "Context" };
        let result = if loaded {
            "self.selected"
        } else {
            "&self.selected"
        };
        let mut program = typed_program(&format!(
            "{CONTEXT_FIXTURE}
             data Holder {{ selected: {field_type}; }}
             machine Holder::as_mut_slice(&self) -> &Context {{ {result} }}
             machine forward(holder: &Holder) -> &Context {{ holder.as_mut_slice() }}
             machine replace(holder: &Holder) -> u64 {{
                 let borrowed: &Context = forward(holder);
                 transition {{ _ -> 0 }}
             }}"
        ));
        if loaded {
            assert_eq!(origin(&program), None);
        } else {
            let (_, segments) = origin(&program).expect("body-proven selected field");
            let [facts::PlaceSegment::Field { symbol }] = segments.as_slice() else {
                panic!("body selection must survive the helper name: {segments:?}")
            };
            assert_eq!(
                program.symbols.display_path(*symbol, "::"),
                "Holder::selected"
            );
        }
        let calls: Vec<_> = program.expression_table.iter_expressions().filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "as_mut_slice").then_some(handle)
        }).collect();
        assert!(!calls.is_empty());
        for expression in calls {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            call.target_symbol = SymbolHandle::invalid();
        }
        assert_eq!(
            origin(&program),
            None,
            "an unresolved name cannot supply exact identity"
        );
    }
}

fn origin(program: &typed_trees::TypedTrees) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("reference local")
    };
    validation::CallFrameResolver::new(program)
        .unwrap()
        .local_reference_origin_before_statement(machine, statements.last().unwrap(), local.symbol)
}
