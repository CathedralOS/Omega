//! Local record field stores on floating fields admit the same already-typed
//! sources the parameter lane accepts — authored IEEE literals and
//! dense-namespace scalars — while a selected floating computation still
//! refuses locally because it must retain its own operation and call
//! correspondence.
use checked_trees::{
    CheckedScalarExpression, CheckedStructuralScalarFieldStoreDestination,
    CheckedStructuralScalarFieldStoreValue, CheckedUnitEffectOperationPlan,
};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).unwrap()
}

fn enter_store(source: &str) -> checked_trees::CheckedStructuralScalarFieldStorePlan {
    let checked = checked(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .expect("local float field store admits a scalar graph");
    for operation in &graph.states[0].unit_operations {
        if let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = operation {
            return store.clone();
        }
    }
    panic!("no structural scalar field store planned");
}

#[test]
fn local_float_field_store_admits_literal_parameter_and_local_sources() {
    for (field_type, body, expected_primitive) in [
        (
            "f32",
            "let mut r: Rec = Rec { x: 1.0 }; r.x = 2.5; r.x",
            typed_trees::types::PrimitiveType::F32,
        ),
        (
            "f64",
            "let mut r: Rec = Rec { x: 1.0 }; r.x = 2.5; r.x",
            typed_trees::types::PrimitiveType::F64,
        ),
        (
            "f64",
            "let mut r: Rec = Rec { x: 1.0 }; r.x = v; r.x",
            typed_trees::types::PrimitiveType::F64,
        ),
        (
            "f64",
            "let mut r: Rec = Rec { x: 1.0 }; let y: f64 = v; r.x = y; r.x",
            typed_trees::types::PrimitiveType::F64,
        ),
    ] {
        let has_parameter = body.contains('v');
        let source = if has_parameter {
            format!(
                "data Rec [copy] {{ x: {field_type}; }}
                 machine enter(v: f64) -> f64 {{ {body} }}"
            )
        } else {
            format!(
                "data Rec [copy] {{ x: {field_type}; }}
                 machine enter() -> {field_type} {{ {body} }}"
            )
        };
        let store = enter_store(&source);
        assert_eq!(store.field_identity, "x");
        assert_eq!(store.primitive_type, expected_primitive);
        assert!(matches!(
            store.destination,
            CheckedStructuralScalarFieldStoreDestination::Local { .. }
        ));
        match &store.value {
            CheckedStructuralScalarFieldStoreValue::Pure(
                CheckedScalarExpression::IeeeFloatLiteral { .. },
            )
            | CheckedStructuralScalarFieldStoreValue::Pure(CheckedScalarExpression::Parameter {
                ..
            })
            | CheckedStructuralScalarFieldStoreValue::Pure(CheckedScalarExpression::Local {
                ..
            }) => {}
            other => panic!("unexpected float source: {other:?}"),
        }
    }
}

#[test]
fn local_float_field_store_refuses_selected_computations() {
    for body in [
        "let mut r: Rec = Rec { x: 1.0 }; r.x = r.x + 1.0; r.x",
        "let mut r: Rec = Rec { x: 1.0 }; r.x = ident(2.0); r.x",
    ] {
        let source = format!(
            "data Rec [copy] {{ x: f64; }}
             machine ident(v: f64) -> f64 {{ v }}
             machine enter() -> f64 {{ {body} }}"
        );
        let checked = checked(&source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "enter")
            .unwrap();
        assert!(
            checked
                .facts
                .flow
                .terminal_scalar_graphs
                .for_machine(machine.symbol)
                .is_none(),
            "{body}"
        );
    }
}
