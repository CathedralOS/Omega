use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{DataDefinition, DataMember, DataVariant};
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use syntax_trees_to_symbol_resolved_trees::{
    lower_syntax_trees_with_sources, normalize_generic_data,
};
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

/// A case `where` fact over only the case's own payload names rides each
/// synthesized generic instance: `Window<i32>::Range` carries `lo <= hi` with
/// `lo`/`hi` resolved to the INSTANCE's payload symbols, not the template's.
#[test]
fn generic_instance_case_where_facts_ride_the_instance() {
    let program = resolve(
        r#"
        data Window<T> {
            marker: T;
            case Empty;
            case Range(lo: u64, hi: u64) where lo <= hi;
        }
        data Main { window: Window<i32>; }
        "#,
    );

    let template = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Window")
        .expect("Window template");
    assert!(template.generic_instance.is_none());
    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Window<i32>")
        .expect("synthesized Window<i32>");
    assert!(instance.generic_instance.is_some());

    let template_range = find_variant(&program, template, "Range");
    let instance_range = find_variant(&program, instance, "Range");
    let template_lo = payload_symbol(&program, template_range, "lo");
    let template_hi = payload_symbol(&program, template_range, "hi");
    let instance_lo = payload_symbol(&program, instance_range, "lo");
    let instance_hi = payload_symbol(&program, instance_range, "hi");
    assert_ne!(template_lo, instance_lo);
    assert_ne!(template_hi, instance_hi);

    let [ProofFact::Expression(expression)] = program.proof_facts(instance_range.where_facts)
    else {
        panic!("instance Range carries one expression fact")
    };
    let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("binary case fact")
    };
    assert_eq!(binary.operator, BinaryOperator::LessOrEqual);
    assert_name_identity(&program, binary.left, instance_lo);
    assert_name_identity(&program, binary.right, instance_hi);

    // The template's own copy still resolves to the template payload.
    let [ProofFact::Expression(expression)] = program.proof_facts(template_range.where_facts)
    else {
        panic!("template Range keeps one expression fact")
    };
    let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("binary template fact")
    };
    assert_name_identity(&program, binary.left, template_lo);
    assert_name_identity(&program, binary.right, template_hi);
}

/// A `const` binder inside a case `where` fact arrives on the instance as its
/// literal argument: `index < N` on `Window<8>::At` is `index < 8`.
#[test]
fn generic_instance_case_where_const_binder_substitutes_its_argument() {
    let program = resolve(
        r#"
        data Window<const N: u64> {
            case At(index: u64) where index < N;
        }
        data Main { window: Window<8>; }
        "#,
    );

    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Window<8>")
        .expect("synthesized Window<8>");
    let at = find_variant(&program, instance, "At");
    let index = payload_symbol(&program, at, "index");

    let [ProofFact::Expression(expression)] = program.proof_facts(at.where_facts) else {
        panic!("instance At carries one expression fact")
    };
    let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("binary case fact")
    };
    assert_eq!(binary.operator, BinaryOperator::Less);
    assert_name_identity(&program, binary.left, index);
    let ExpressionNode::Integer(literal) =
        program.tables.bodies.expressions.expression(binary.right)
    else {
        panic!("const binder substituted to a literal")
    };
    assert_eq!(literal.text(), "8");
}

/// A case `where` fact that names a TYPE parameter still refuses: there is no
/// fact-position substitution for `T` on `Value<i32>::Integer`.
#[test]
fn generic_case_where_naming_a_type_parameter_still_refuses() {
    let source = r#"
        data Value<T> {
            case Integer(value: T) where T == i32;
        }
        data Main { value: Value<i32>; }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize parameter-mentioning case fact");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse parameter-mentioning case fact");
    let syntax = normalize_generic_data(syntax).expect("synthesize Value<i32>");
    let errors = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect_err("parameter-mentioning case fact refuses");
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("case constraints on generic data may not mention generic parameters")),
        "expected the generic case-fact fence, got {errors:?}"
    );
}

fn resolve(source: &str) -> SymbolResolvedTrees {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    let syntax = normalize_generic_data(syntax).expect("synthesize instances");
    lower_syntax_trees_with_sources(&syntax, Arc::new(sources)).expect("resolve")
}

fn find_variant<'a>(
    program: &'a SymbolResolvedTrees,
    definition: &'a DataDefinition,
    name: &str,
) -> &'a DataVariant {
    program
        .data_members(definition.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.name.as_str() == name => Some(variant),
            _ => None,
        })
        .expect("variant")
}

fn payload_symbol(
    program: &SymbolResolvedTrees,
    variant: &DataVariant,
    name: &str,
) -> symbols::SymbolHandle {
    program
        .data_payload_fields(variant.payload)
        .iter()
        .find(|field| field.name.as_str() == name)
        .map(|field| field.symbol)
        .expect("payload field")
}

fn assert_name_identity(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    expected: symbols::SymbolHandle,
) {
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
    else {
        panic!("name expression")
    };
    assert_eq!(path.head_symbol, expected);
    assert_eq!(path.symbol, expected);
    assert_eq!(
        program
            .tables
            .bodies
            .expressions
            .name_path_member_symbols(path.member_symbols),
        &[expected]
    );
}
