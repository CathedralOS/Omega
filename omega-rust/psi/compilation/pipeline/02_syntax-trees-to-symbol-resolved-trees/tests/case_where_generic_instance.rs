use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{DataDefinition, DataMember, DataVariant};
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::types::TypeReference;
use syntax_trees_to_symbol_resolved_trees::ResolutionRequest;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

/// A case `where` fact that names a TYPE parameter still refuses outside a
/// decided `T == name` conjunct: `T <= i32` has no fact-position substitution
/// for `T` on `Value<i32>::Integer`.
#[test]
fn generic_case_where_naming_a_type_parameter_still_refuses() {
    let source = r#"
        data Value<T> {
            case Integer(value: T) where T <= i32;
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
    let syntax =
        normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize Value<i32>");
    let errors = syntax_trees_to_symbol_resolved_trees::resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("parameter-mentioning case fact refuses");
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("case constraints on generic data may not mention generic parameters")),
        "expected the generic case-fact fence, got {errors:?}"
    );
}

/// A case `where` membership fact may name a `type` binder inside the domain's
/// index arguments: `value in i32::Held<T>` on the `Box<T>` template carries
/// onto `Box<i32>::Full` as `value in i32::Held<i32>` -- the binder argument
/// substitutes to the closed type, while the carrier-qualified domain path and
/// the membership value stay verbatim.
#[test]
fn generic_instance_case_where_membership_type_argument_substitutes() {
    let program = resolve(
        r#"
        domain<S> i32::Held<S>;
        data Box<T> {
            case Full(value: T) where value in i32::Held<T>;
        }
        data Main { b: Box<i32>; }
        "#,
    );

    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Box<i32>")
        .expect("synthesized Box<i32>");
    let full = find_variant(&program, instance, "Full");
    let [ProofFact::Membership(membership)] = program.proof_facts(full.where_facts) else {
        panic!("case membership fact rides the instance")
    };
    let [TypeReference::Named { name, .. }] =
        program.child_type_references(membership.domain_arguments)
    else {
        panic!("a `type` binder argument lands as the closed named argument")
    };
    assert_eq!(name.as_str(), "i32");
}

/// A const-expression argument copies so its `const` binder leaves rewrite to
/// their literal argument: `Window<8>::At` carries `index in u64::Bounded<8+1>`.
#[test]
fn generic_instance_case_where_membership_const_expression_argument_substitutes() {
    let program = resolve(
        r#"
        domain<const B: u64> u64::Bounded<B>;
        data Window<const N: u64> {
            case At(index: u64) where index in u64::Bounded<N + 1>;
        }
        data Main { w: Window<8>; }
        "#,
    );

    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Window<8>")
        .expect("synthesized Window<8>");
    let at = find_variant(&program, instance, "At");
    let [ProofFact::Membership(membership)] = program.proof_facts(at.where_facts) else {
        panic!("case membership fact rides the instance")
    };
    let [TypeReference::ConstExpression(expression)] =
        program.child_type_references(membership.domain_arguments)
    else {
        panic!("an open const-expression argument copies as a const expression")
    };
    let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("`N + 1` stays a binary expression on the instance")
    };
    let ExpressionNode::Integer(left) = program.tables.bodies.expressions.expression(binary.left)
    else {
        panic!("the `N` leaf rewrote to its literal")
    };
    assert_eq!(left.text(), "8");
}

fn resolve(source: &str) -> SymbolResolvedTrees {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    let syntax =
        normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize instances");
    syntax_trees_to_symbol_resolved_trees::resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve")
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
