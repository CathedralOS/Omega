use super::{
    Lexer, lower_syntax_extension_against_resolved_base, lower_syntax_trees,
    lower_syntax_trees_with_sources, parse_syntax_trees, parse_syntax_trees_with_id,
};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::measure::MeasureDefinition;
use symbols::{SymbolHandle, SymbolKind};

fn resolve(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize measure");
    let syntax = parse_syntax_trees(&tokens).expect("parse measure");
    lower_syntax_trees(&syntax).expect("resolve measure")
}

fn assert_parameter_forward(program: &SymbolResolvedTrees, measure: &MeasureDefinition) {
    let parameter = measure
        .parameter
        .as_ref()
        .expect("scalar measure parameter");
    assert!(measure.symbol.is_valid());
    assert!(parameter.symbol.is_valid());
    assert_eq!(
        program.symbols.get(measure.symbol).kind,
        SymbolKind::Measure
    );
    assert_eq!(
        program.symbols.get(parameter.symbol).kind,
        SymbolKind::Parameter
    );
    assert!(
        program
            .symbols
            .child_handles(measure.symbol)
            .into_iter()
            .flatten()
            .any(|child| child == parameter.symbol)
    );
    let expressions = &program.tables.bodies.expressions;
    let [body] = expressions.expression_handles(measure.body) else {
        panic!("one identity body expression")
    };
    let ExpressionNode::Name(path) = expressions.expression(*body) else {
        panic!("parameter reference")
    };
    assert_eq!(path.symbol, parameter.symbol);
    assert_eq!(path.head_symbol, parameter.symbol);
    assert_eq!(
        expressions.name_path_member_symbols(path.member_symbols),
        [parameter.symbol]
    );
}

#[test]
fn identity_measures_own_distinct_same_spelled_parameters() {
    let program = resolve(
        "measure First::Order(value: u64) -> u64 { value }
         measure Second::Order(value: u64) -> u64 { value }
         machine unrelated(value: u64) -> u64 { value }",
    );
    let measures = program.measures.iter().collect::<Vec<_>>();
    for measure in &measures {
        assert_parameter_forward(&program, measure);
        let parameter = measure.parameter.as_ref().unwrap();
        let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &parameter.type_reference
        else {
            panic!("named natural carrier")
        };
        assert!(symbol.is_valid(), "the parameter's carrier resolves too");
    }
    assert_ne!(measures[0].symbol, measures[1].symbol);
    assert_ne!(
        measures[0].parameter.as_ref().unwrap().symbol,
        measures[1].parameter.as_ref().unwrap().symbol
    );
    assert_eq!(program.symbols.name(measures[0].symbol), "First::Order");
    assert_eq!(program.symbols.name(measures[1].symbol), "Second::Order");
}

#[test]
fn other_names_and_self_cannot_impersonate_a_measure_parameter() {
    for body in [
        "nonexistent",
        "Other::value",
        "self",
        "value::value",
        "Count::Order::value",
    ] {
        let program = resolve(&format!(
            "data Other {{ value: u64; }}
             measure Count::Order(value: u64) -> u64 {{ {body} }}"
        ));
        let measure = program.measures.iter().next().unwrap();
        let parameter = measure.parameter.as_ref().unwrap();
        assert!(parameter.symbol.is_valid());
        let expressions = &program.tables.bodies.expressions;
        let expression = expressions.expression_handles(measure.body)[0];
        let ExpressionNode::Name(path) = expressions.expression(expression) else {
            panic!("expected name body for {body}")
        };
        assert_ne!(path.symbol, parameter.symbol, "{body}");
        if body == "Other::value" {
            assert!(
                path.symbol.is_valid(),
                "an actual foreign field retains its identity"
            );
            assert_eq!(program.symbols.get(path.symbol).kind, SymbolKind::Field);
        } else {
            assert_eq!(path.symbol, SymbolHandle::invalid(), "{body}");
        }
    }
}

#[test]
fn measure_body_traversal_binds_nested_parameter_uses() {
    let program = resolve("measure Count::Order(value: u64) -> u64 { value - value }");
    let measure = program.measures.iter().next().unwrap();
    let parameter = measure.parameter.as_ref().unwrap();
    let expressions = &program.tables.bodies.expressions;
    let body = expressions.expression_handles(measure.body)[0];
    let ExpressionNode::Binary(binary) = expressions.expression(body) else {
        panic!("binary measure body")
    };
    for operand in [binary.left, binary.right] {
        let ExpressionNode::Name(path) = expressions.expression(operand) else {
            panic!("nested parameter use")
        };
        assert_eq!(path.symbol, parameter.symbol);
        assert_eq!(path.head_symbol, parameter.symbol);
    }
}

#[test]
fn seeded_measure_extension_preserves_existing_declarations_and_binders() {
    let base_source = "measure First::Order(value: u64) -> u64 { value }";
    let extension_source = "measure Second::Order(value: u64) -> u64 { value }";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let base_syntax =
        parse_syntax_trees_with_id(base_id, &Lexer::new(base_source).tokenize().unwrap()).unwrap();
    let base = lower_syntax_trees_with_sources(&base_syntax, Arc::new(sources.clone())).unwrap();
    let retained = base.measures.iter().next().unwrap().clone();
    let retained_body = base
        .tables
        .bodies
        .expressions
        .expression_handles(retained.body)[0];
    let retained_expression = base
        .tables
        .bodies
        .expressions
        .expression(retained_body)
        .clone();
    let retained_source = base.symbols.symbol_source_span(retained.symbol);
    let retained_parameter_source = base
        .symbols
        .symbol_source_span(retained.parameter.as_ref().unwrap().symbol);
    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source).tokenize().unwrap(),
    )
    .unwrap();
    let program = lower_syntax_extension_against_resolved_base(
        base,
        &extension_syntax,
        Arc::new(sources),
        Vec::new(),
    )
    .expect("extend existing measure symbols");
    let measures = program.measures.iter().collect::<Vec<_>>();
    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0], &retained);
    assert_eq!(
        program.tables.bodies.expressions.expression(retained_body),
        &retained_expression
    );
    assert_eq!(
        program.symbols.symbol_source_span(retained.symbol),
        retained_source
    );
    assert_eq!(
        program
            .symbols
            .symbol_source_span(retained.parameter.as_ref().unwrap().symbol),
        retained_parameter_source
    );
    for measure in &measures {
        assert_parameter_forward(&program, measure);
    }
    assert_ne!(measures[0].symbol, measures[1].symbol);
    assert_ne!(
        measures[0].parameter.as_ref().unwrap().symbol,
        measures[1].parameter.as_ref().unwrap().symbol
    );
    assert_eq!(
        program
            .symbols
            .symbol_source_span(measures[1].symbol)
            .unwrap()
            .source_id,
        extension_id
    );
}

#[test]
fn free_constants_cannot_capture_measure_parameters_in_either_declaration_order() {
    let measure = "measure Count::Order(value: u64) -> u64 { value }";
    let constant = "const value: u64 = 5;";
    for source in [
        format!("{constant} {measure}"),
        format!("{measure} {constant}"),
    ] {
        let program = resolve(&source);
        let measure = program.measures.iter().next().expect("identity measure");
        assert_parameter_forward(&program, measure);
        let expressions = &program.tables.bodies.expressions;
        let [body] = expressions.expression_handles(measure.body) else {
            panic!("one parameter reference")
        };
        assert_eq!(
            expressions.authored_selection_occurrences(*body).count(),
            0,
            "a measure parameter must not acquire constant-selection custody"
        );
    }
    let program = resolve(&format!("const Other::value: u64 = 5; {measure}"));
    assert_parameter_forward(&program, program.measures.iter().next().unwrap());
}
