use super::*;
use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};

fn resolved_local(program: &SymbolResolvedTrees, local_name: &str) -> ExpressionHandle {
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "generated")
        .expect("generated machine");
    let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
    program
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            symbol_resolved_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == local_name =>
            {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("generated local initializer")
}

#[test]
fn seeded_retained_scalar_and_array_constants_type_without_relowering_base() {
    use symbol_resolved_trees::expression::ExpressionNode;

    let (base, extension) = seeded_plain_data_inputs(
        "module settings; pub const COUNT: u64 = 7; pub const VALUES: [u64; 2] = [3, 5];",
        "use settings; machine generated() -> u64 {
            let count: u64 = settings::COUNT;
            let first: [u64; 2] = settings::VALUES;
            let second: [u64; 2] = settings::VALUES;
            count
        }",
    );
    let program = extension.trees();
    let expressions = &program.tables.bodies.expressions;
    let ExpressionNode::Integer(count) = expressions.expression(resolved_local(program, "count"))
    else {
        panic!("retained scalar substituted");
    };
    assert_eq!(count.value_u64(), Some(7));
    let ExpressionNode::ArrayLiteral(first) =
        expressions.expression(resolved_local(program, "first"))
    else {
        panic!("first retained array substituted");
    };
    let ExpressionNode::ArrayLiteral(second) =
        expressions.expression(resolved_local(program, "second"))
    else {
        panic!("second retained array substituted");
    };
    assert_ne!(first, second);
    for (first, second) in expressions
        .expression_handles(*first)
        .iter()
        .zip(expressions.expression_handles(*second))
    {
        assert_ne!(first, second);
        assert_eq!(
            expressions.expression(*first),
            expressions.expression(*second)
        );
    }
    let retained = base.typed().clone();
    let typed = lower_seeded_extension(extension, base)
        .expect("retained scalar and array uses enter seeded typing");
    assert!(retained_typed_base_is_exact_prefix(&retained, &typed));
    assert_eq!(typed.const_declarations(), retained.const_declarations());
    assert_eq!(typed.machines().len(), retained.machines().len() + 1);
}

#[test]
fn seeded_retained_nominal_constant_keeps_constructor_custody_under_shadowing() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };
    use symbol_resolved_trees::expression::ExpressionNode;

    let (base, extension) = seeded_plain_data_inputs(
        "module settings;
         pub data Value [copy] { count: u64; }
         pub data Holder [copy] { value: Value; }
         pub const VALUE: Value = Value { count: 7 };",
        "use settings; data Value [copy] { count: u64; }
         machine generated() -> settings::Value {
            let first: settings::Value = settings::VALUE;
            let second: settings::Value = settings::VALUE;
            second
         }",
    );
    assert!(
        base.typed().authored_declaration_selections().len()
            > base
                .resolved_base_for_extension()
                .authored_declaration_selections()
                .len(),
        "retained nominal field typing adds selection custody before suffix rebasing"
    );
    let program = extension.trees();
    let expressions = &program.tables.bodies.expressions;
    let first_handle = resolved_local(program, "first");
    let second_handle = resolved_local(program, "second");
    let ExpressionNode::StructLiteral(first) = expressions.expression(first_handle) else {
        panic!("first retained constructor substituted");
    };
    let ExpressionNode::StructLiteral(second) = expressions.expression(second_handle) else {
        panic!("second retained constructor substituted");
    };
    assert_eq!(
        program.symbols.display_path(first.type_symbol, "::"),
        "settings::Value"
    );
    assert_eq!(first.type_symbol, second.type_symbol);
    assert_ne!(first.fields, second.fields);
    for (first, second) in expressions
        .struct_fields(first.fields)
        .iter()
        .zip(expressions.struct_fields(second.fields))
    {
        assert_eq!(first.field_symbol, second.field_symbol);
        assert_ne!(first.value, second.value);
    }
    for handle in [first_handle, second_handle] {
        let selections = expressions
            .authored_selection_occurrences(handle)
            .map(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .expect("retained occurrence")
            })
            .collect::<Vec<_>>();
        let constructor = selections
            .iter()
            .find(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::StructLiteralType
            })
            .expect("declaration-side constructor occurrence survives rebasing");
        assert_eq!(constructor.source_span().source_id, source::SourceId(0));
        let AuthoredDeclarationSelectionTarget::Resolved(target) = constructor.target() else {
            panic!("resolved constructor selection");
        };
        assert_eq!(target.selected_symbol(), first.type_symbol);
        assert!(selections.iter().any(|selection| selection.kind()
            == AuthoredDeclarationSelectionKind::StaticPathSegment
            && selection.source_span().source_id == source::SourceId(1)));
    }
    let retained = base.typed().clone();
    let typed =
        lower_seeded_extension(extension, base).expect("retained nominal uses enter seeded typing");
    assert!(retained_typed_base_is_exact_prefix(&retained, &typed));
    assert_eq!(typed.const_declarations(), retained.const_declarations());
    assert_eq!(
        typed.data_definitions().len(),
        retained.data_definitions().len() + 1
    );
}
