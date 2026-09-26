use numerics::literals::{FloatFormat, FloatLiteral};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[test]
fn empty_floating_aggregate_cannot_erase_its_authored_call() {
    let mut program = super::evaluate_fully(
        &[(
            "main.omg",
            "machine empty() -> [f32; 0] { [] }
            const EMPTY: [f32; 0] = empty();",
        )],
        &[],
    );
    let declaration = program.const_declarations()[0].clone();
    let replacement = program
        .expression_table
        .expression(declaration.materialized_initializer)
        .clone();
    assert!(matches!(replacement, ExpressionNode::ArrayLiteral(_)));
    *program
        .expression_table
        .expression_mut(declaration.authored_initializer) = replacement;
    assert!(
        super::super::replay::validate(&program, None).is_err(),
        "empty result cannot erase the computation retained by its selection receipt"
    );
}

fn first_float(program: &typed_trees::TypedTrees, root: ExpressionHandle) -> ExpressionHandle {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Float(_) => return expression,
            ExpressionNode::ArrayLiteral(elements) => pending.extend(
                program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .copied(),
            ),
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            _ => {}
        }
    }
    panic!("materialized aggregate contains a floating leaf");
}

#[test]
fn floating_aggregate_machine_results_replay_their_exact_bits() {
    for (carrier, format) in [("f32", FloatFormat::F32), ("f64", FloatFormat::F64)] {
        let source = format!(
            "data Cell [copy] {{ values: [{carrier}; 2]; }}
             data Choice [copy] {{ case Empty; case Value(value: {carrier}); }}
             machine table() -> [{carrier}; 2] {{ [1.5{carrier}, -0.0{carrier}] }}
             machine record() -> Cell {{ Cell {{ values: table() }} }}
             const ARRAY: [{carrier}; 2] = table();
             const RECORD: Cell = record();
             const CASE: Choice = Choice::Value {{ value: -0.0{carrier} }};"
        );
        let original = super::evaluate_fully(&[("main.omg", &source)], &[]);
        for declaration in original.const_declarations() {
            let leaf = first_float(&original, declaration.materialized_initializer);
            let mut changed = original.clone();
            *changed.expression_table.expression_mut(leaf) =
                ExpressionNode::Float(FloatLiteral::parse("0.0").unwrap().with_landing(format));
            assert!(
                super::super::replay::validate(&changed, None).is_err(),
                "computed aggregate or case cannot discard the sign of zero"
            );
        }
    }
}

#[test]
fn floating_aggregate_replay_reconstructs_exact_copied_bits() {
    for (carrier, format, other_format) in [
        ("f32", FloatFormat::F32, FloatFormat::F64),
        ("f64", FloatFormat::F64, FloatFormat::F32),
    ] {
        for copy in [false, true] {
            let source = format!(
                "data Cell<T [copy]> [copy] {{ value: T; }}
             const EMPTY: [Cell<{carrier}>; 0] = [];
             const TABLE: [Cell<{carrier}>; 2] = [
                 Cell {{ value: 1.5{carrier} }}, Cell {{ value: -0.0{carrier} }}
             ]; {}",
                if copy {
                    format!("const COPIED: [Cell<{carrier}>; 2] = TABLE;")
                } else {
                    String::new()
                },
            );
            let original = super::evaluate_fully(&[("main.omg", &source)], &[]);
            super::super::replay::validate(&original, None).unwrap();
            for name in ["TABLE", "COPIED"] {
                if name == "COPIED" && !copy {
                    continue;
                }
                let declaration = original
                    .const_declarations()
                    .iter()
                    .find(|declaration| original.symbols.name(declaration.symbol) == name)
                    .unwrap();
                let leaf = first_float(&original, declaration.materialized_initializer);
                for replacement in [
                    FloatLiteral::parse("0.0").unwrap().with_landing(format),
                    FloatLiteral::parse("1.5").unwrap().with_landing(format),
                    FloatLiteral::parse("-0.0")
                        .unwrap()
                        .with_landing(other_format),
                    FloatLiteral::parse("NaN").unwrap().with_landing(format),
                ] {
                    let mut changed = original.clone();
                    *changed.expression_table.expression_mut(leaf) =
                        ExpressionNode::Float(replacement);
                    assert!(
                        super::super::replay::validate(&changed, None).is_err(),
                        "changed {name} bits or declared format must reject"
                    );
                }
                let mut changed = original.clone();
                let declarations = changed.roots.const_declarations;
                let changed_declaration = changed
                    .tables
                    .const_declarations
                    .span_mut_or_empty(declarations)
                    .iter_mut()
                    .find(|candidate| candidate.symbol == declaration.symbol)
                    .unwrap();
                let encoding = changed_declaration
                    .canonical_value_encoding
                    .as_mut()
                    .unwrap();
                *encoding = encoding.replace(
                    if carrier == "f32" {
                        "float:f32:80000000"
                    } else {
                        "float:f64:8000000000000000"
                    },
                    if carrier == "f32" {
                        "float:f32:00000000"
                    } else {
                        "float:f64:0000000000000000"
                    },
                );
                assert!(
                    super::super::replay::validate(&changed, None).is_err(),
                    "changed {name} receipt must reject"
                );
            }
        }
    }
}
