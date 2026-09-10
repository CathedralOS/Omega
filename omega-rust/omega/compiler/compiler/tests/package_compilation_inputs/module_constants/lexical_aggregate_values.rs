use super::*;
use compiler::CheckedCompilation;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

#[test]
fn root_aggregate_values_substitute_only_after_lexical_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for (declarations, carrier, initializer, expected) in [
        (
            "data Pair [copy] { value: u64; }",
            "Pair",
            "Pair { value: 11 }",
            "Pair{11}",
        ),
        ("", "[u64; 2]", "[11, 12]", "[11,12]"),
        (
            "data Choice [copy] { case Empty; case Some(value: u64); }",
            "Choice",
            "Choice::Empty",
            "Choice::Empty{}",
        ),
        (
            "data Choice [copy] { case Empty; case Some(value: u64); }",
            "Choice",
            "Choice::Some { value: 11 }",
            "Choice::Some{11}",
        ),
    ] {
        TempTree::write(root.join("main.omg"), &format!(
            "{declarations} const AGG: {carrier} = {initializer};
             machine parameter(AGG: {carrier}) -> {carrier} {{ let observed: {carrier} = AGG; observed }}
             machine prior(input: {carrier}) -> {carrier} {{ let AGG: {carrier} = input; let observed: {carrier} = AGG; observed }}
             machine later(input: {carrier}) -> {carrier} {{ let before: {carrier} = AGG; let AGG: {carrier} = input; let after: {carrier} = AGG; before }}"
        ));
        let checked =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect("root aggregate permits legal lexical shadows");
        assert_eq!(
            value_shape(&checked, local_value(&checked, "later", "before")),
            expected
        );
        for (machine, local, expected_kind) in [
            ("parameter", "observed", symbols::SymbolKind::Parameter),
            ("prior", "observed", symbols::SymbolKind::Local),
            ("later", "after", symbols::SymbolKind::Local),
        ] {
            let ExpressionNode::Name(path) = checked
                .typed
                .expression_table
                .expression(local_value(&checked, machine, local))
            else {
                panic!("runtime aggregate reference must remain a lexical name")
            };
            assert_eq!(checked.symbols.get(path.symbol).kind, expected_kind);
        }
        let selections = aggregate_selections(&checked);
        assert_eq!(
            selections.len(),
            1,
            "only the occurrence before the local declaration selects AGG"
        );
        assert_eq!(
            checked.symbols.symbol_package_identity(selections[0]),
            Some(identity(1))
        );
    }
}

#[test]
fn root_aggregate_constructor_owner_survives_a_module_consumer_shadow() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("main.omg"),
        "use consumer; data Pair [copy] { value: u64; } const AGG: Pair = Pair { value: 11 };",
    );
    TempTree::write(
        root.join("consumer.omg"),
        "module consumer; machine read(Pair: u64) -> Pair { let observed: Pair = AGG; observed }",
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("module consumer selects a root-owned aggregate constant");
    let value = local_value(&checked, "consumer::read", "observed");
    assert_eq!(value_shape(&checked, value), "Pair{11}");
    let ExpressionNode::StructLiteral(literal) = checked.typed.expression_table.expression(value)
    else {
        panic!("materialized record")
    };
    assert_eq!(
        checked.symbols.symbol_package_identity(literal.type_symbol),
        Some(identity(1))
    );
    assert_eq!(
        checked.symbols.get(literal.type_symbol).kind,
        symbols::SymbolKind::Data
    );
    assert_eq!(aggregate_selections(&checked).len(), 1);
}

fn local_value(
    checked: &CheckedCompilation,
    machine_name: &str,
    local_name: &str,
) -> ExpressionHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == machine_name)
        .expect("machine declaration");
    checked
        .typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| {
            checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
        })
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("local initializer")
}

fn aggregate_selections(checked: &CheckedCompilation) -> Vec<symbols::SymbolHandle> {
    checked
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| {
            let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
                return None;
            };
            let symbol = target.selected_symbol();
            (checked.symbols.get(symbol).kind == symbols::SymbolKind::Const
                && checked.symbols.name(symbol) == "AGG")
                .then_some(symbol)
        })
        .collect()
}

fn value_shape(checked: &CheckedCompilation, expression: ExpressionHandle) -> String {
    match checked.typed.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_u64()
            .expect("nonnegative fixture integer")
            .to_string(),
        ExpressionNode::ArrayLiteral(elements) => format!(
            "[{}]",
            checked
                .typed
                .expression_table
                .expression_handles(*elements)
                .iter()
                .map(|element| value_shape(checked, *element))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ExpressionNode::StructLiteral(literal) => {
            let symbol = literal.case_symbol.unwrap_or(literal.type_symbol);
            assert!(symbol.is_valid());
            let fields = checked.typed.expression_table.struct_fields(literal.fields);
            for field in fields {
                assert_eq!(
                    checked.symbols.get(field.field_symbol).kind,
                    symbols::SymbolKind::Field
                );
            }
            format!(
                "{}{{{}}}",
                checked.symbols.display_path(symbol, "::"),
                fields
                    .iter()
                    .map(|field| value_shape(checked, field.value))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        ExpressionNode::Name(path) => {
            assert_eq!(
                checked.symbols.get(path.symbol).kind,
                symbols::SymbolKind::Variant
            );
            checked.symbols.display_path(path.symbol, "::")
        }
        other => panic!("expected a fully materialized literal aggregate, found {other:?}"),
    }
}
