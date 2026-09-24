use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::{DataDefinition, DataMember, DataVariant};
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::types::TypeReference;
use syntax_trees_to_symbol_resolved_trees::ResolutionRequest;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
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

/// A `type` binder in a `T == name` conjunct is decided at synthesis against
/// the closed argument identity: `Value<i32>` proves `T == i32`, so the
/// discharged fact does not ride the instance, while `Value<bool>` refutes it
/// and the instance's `Integer` case carries the literal `0` witness that
/// reads FALSE for construction, zero gating, and coverage.
#[test]
fn generic_instance_case_where_type_equality_decides_per_instance() {
    let program = resolve(
        r#"
        data Value<T> {
            case Integer(value: T) where T == i32;
            case Boolean(value: T) where T == bool;
        }
        data Main { yes: Value<i32>; no: Value<bool>; }
        "#,
    );

    let yes = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Value<i32>")
        .expect("synthesized Value<i32>");
    let yes_integer = find_variant(&program, yes, "Integer");
    assert!(
        program.proof_facts(yes_integer.where_facts).is_empty(),
        "proved `T == i32` discharges off `Value<i32>::Integer`"
    );
    let yes_boolean = find_variant(&program, yes, "Boolean");
    let [ProofFact::Expression(expression)] = program.proof_facts(yes_boolean.where_facts) else {
        panic!("refuted `T == bool` on `Value<i32>` leaves the `0` witness")
    };
    let ExpressionNode::Integer(literal) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("refuted case fact is the literal `0` witness")
    };
    assert_eq!(literal.text(), "0");
    // `Value<i32>`'s zero tag is `Integer`, whose fact discharged: the zeroed
    // representation can still be born established.
    assert!(!yes.zero_gated);

    let no = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Value<bool>")
        .expect("synthesized Value<bool>");
    let no_integer = find_variant(&program, no, "Integer");
    let [ProofFact::Expression(expression)] = program.proof_facts(no_integer.where_facts) else {
        panic!("refuted `T == i32` leaves the literal `0` witness")
    };
    let ExpressionNode::Integer(literal) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("refuted case fact is the literal `0` witness")
    };
    assert_eq!(literal.text(), "0");
    let no_boolean = find_variant(&program, no, "Boolean");
    assert!(
        program.proof_facts(no_boolean.where_facts).is_empty(),
        "proved `T == bool` discharges off `Value<bool>::Boolean`"
    );
    // `Value<bool>`'s zero tag is `Integer`, which can never hold here: the
    // zeroed representation cannot be born established.
    assert!(no.zero_gated);
}

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
        syntax: &syntax,
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

/// The same lift covers a `const` binder: `Window<8>::At` carries
/// `index in u64::Bounded<8>`.
#[test]
fn generic_instance_case_where_membership_const_argument_substitutes() {
    let program = resolve(
        r#"
        domain<const B: u64> u64::Bounded<B>;
        data Window<const N: u64> {
            case At(index: u64) where index in u64::Bounded<N>;
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
    let [TypeReference::Named { name, .. }] =
        program.child_type_references(membership.domain_arguments)
    else {
        panic!("a `const` binder argument lands as its literal name")
    };
    assert_eq!(name.as_str(), "8");
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

/// A binder in the membership fact's VALUE still refuses: `value in i32::Held<i32>`
/// names no binder honestly only if `value` is a place/name -- `T` there is a
/// type-expression, not a value.
#[test]
fn generic_instance_case_where_membership_binder_value_still_refuses() {
    let source = r#"
        domain<S> i32::Held<S>;
        data Box<T> {
            case Full(value: T) where T in i32::Held<T>;
        }
        data Main { b: Box<i32>; }
    "#;
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize binder-value membership fact");
    let syntax =
        parse_syntax_trees_with_id(source_id, &tokens).expect("parse binder-value membership fact");
    let syntax =
        normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize Box<i32>");
    let errors = syntax_trees_to_symbol_resolved_trees::resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect_err("binder-valued case membership refuses");
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
    let syntax =
        normalize_generic_data(GenericDataRequest::new(syntax)).expect("synthesize instances");
    syntax_trees_to_symbol_resolved_trees::resolve(ResolutionRequest {
        syntax: &syntax,
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
