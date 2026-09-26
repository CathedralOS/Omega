//! Top-level `let`/`boundary let` parsing (PROOF-CONTRACT-MIGRATION).

use crate::parser::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{
    MathematicalDefinitionBody, MathematicalTypeHandle, MathematicalTypeNode, TypeParameterKind,
};
use syntax_trees::types::TypeReferenceNode;

fn parse(source: &str) -> SyntaxTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    parse_syntax_trees(&tokens).expect("parse")
}

fn single_definition(parsed: &SyntaxTrees) -> &syntax_trees::item::MathematicalDefinition {
    let handles = parsed.root_mathematical_definition_handles();
    assert_eq!(handles.len(), 1, "exactly one mathematical definition");
    parsed.root_mathematical_definition(handles[0])
}

/// Render the authored type identity for assertions: names, generic
/// applications and unit, with arrows spelled `d -> c` / `(x: d) -> c`.
fn render_type(parsed: &SyntaxTrees, handle: MathematicalTypeHandle) -> String {
    match parsed.items.mathematical_type(handle) {
        MathematicalTypeNode::Ordinary(reference) => render_type_reference(parsed, *reference),
        MathematicalTypeNode::Arrow {
            binder,
            domain,
            codomain,
        } => {
            let domain = render_type(parsed, *domain);
            let codomain = render_type(parsed, *codomain);
            match binder {
                Some(binder) => format!("({}: {domain}) -> {codomain}", binder.as_str()),
                None => format!("{domain} -> {codomain}"),
            }
        }
        MathematicalTypeNode::Application { callee, arguments } => {
            let callee = render_type(parsed, *callee);
            let arguments = parsed
                .expressions
                .expression_handles(*arguments)
                .iter()
                .map(|argument| render_expression(parsed, *argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{callee}({arguments})")
        }
    }
}

fn render_expression(
    parsed: &SyntaxTrees,
    handle: syntax_trees::expression::ExpressionHandle,
) -> String {
    match parsed.expressions.expression(handle) {
        ExpressionNode::Name(path) => parsed
            .expressions
            .identifier_path_members(*path)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::"),
        other => panic!("unexpected expression in test: {other:?}"),
    }
}

fn render_type_reference(
    parsed: &SyntaxTrees,
    handle: syntax_trees::types::TypeReferenceHandle,
) -> String {
    match parsed.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => name.as_str().to_owned(),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = parsed
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| render_type_reference(parsed, *argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", base_name.as_str())
        }
        TypeReferenceNode::Unit => "()".to_owned(),
        other => panic!("unexpected type reference in test: {other:?}"),
    }
}

#[test]
fn parses_plain_mathematical_let() {
    let parsed =
        parse("let greater_than(limit: i32, value: i32): core::Strict<0> =\n    value > limit;");
    assert_eq!(parsed.root_item_handles().len(), 0);
    let definition = single_definition(&parsed);
    assert_eq!(definition.name.as_str(), "greater_than");
    assert!(!definition.is_public);
    assert!(definition.type_parameters.is_empty());

    let parameters = parsed.items.mathematical_parameters(definition.parameters);
    assert_eq!(parameters.len(), 2);
    let limit = parsed.items.mathematical_parameter(parameters[0]);
    assert_eq!(limit.name.as_str(), "limit");
    assert_eq!(limit.relevance, language_core::BindingRelevance::Relevant);
    assert_eq!(render_type(&parsed, limit.ty), "i32");
    let value = parsed.items.mathematical_parameter(parameters[1]);
    assert_eq!(value.name.as_str(), "value");

    assert_eq!(render_type(&parsed, definition.result), "core::Strict<0>");
    assert!(matches!(
        definition.body,
        MathematicalDefinitionBody::Definition(_)
    ));
}

#[test]
fn parses_universe_and_type_binder_telescope() {
    let parsed = parse(
        "let compose<u: core::Level, v: core::Level, w: core::Level,
             A: core::Type<u>, B: core::Type<v>, C: core::Type<w>>(
    f: B -> C, g: A -> B, value: A
): C = f(g(value));",
    );
    let definition = single_definition(&parsed);
    assert_eq!(definition.name.as_str(), "compose");

    let binders = parsed.items.type_parameters(definition.type_parameters);
    assert_eq!(binders.len(), 6);
    assert_eq!(binders[0].name.as_str(), "u");
    assert!(matches!(binders[0].kind, TypeParameterKind::Value { .. }));
    let TypeParameterKind::Value { type_reference } = &binders[3].kind else {
        panic!("`A: core::Type<u>` parses as a value binder");
    };
    assert_eq!(
        render_type_reference(&parsed, *type_reference),
        "core::Type<u>"
    );

    let parameters = parsed.items.mathematical_parameters(definition.parameters);
    assert_eq!(parameters.len(), 3);
    let f = parsed.items.mathematical_parameter(parameters[0]);
    assert_eq!(f.name.as_str(), "f");
    assert_eq!(render_type(&parsed, f.ty), "B -> C");
    let g = parsed.items.mathematical_parameter(parameters[1]);
    assert_eq!(render_type(&parsed, g.ty), "A -> B");
    assert_eq!(render_type(&parsed, definition.result), "C");
}

#[test]
fn parses_boundary_let_as_bodyless_assumption() {
    let parsed = parse(
        "boundary let choose<u: core::Level, A: core::Type<u>>(
    inhabited: core::Squash<A>
): A;",
    );
    let definition = single_definition(&parsed);
    assert_eq!(definition.name.as_str(), "choose");
    assert_eq!(render_type(&parsed, definition.result), "A");
    assert_eq!(definition.body, MathematicalDefinitionBody::Assumption);

    let parameters = parsed.items.mathematical_parameters(definition.parameters);
    assert_eq!(parameters.len(), 1);
    let inhabited = parsed.items.mathematical_parameter(parameters[0]);
    assert_eq!(render_type(&parsed, inhabited.ty), "core::Squash<A>");
}

#[test]
fn arrow_types_associate_right_and_bind_domains() {
    let parsed = parse("let fam(u: core::Level): A -> B -> C =\n    term;");
    let definition = single_definition(&parsed);
    assert_eq!(render_type(&parsed, definition.result), "A -> B -> C");

    let parsed = parse("let dependent(u: core::Level): (value: A) -> F(value) =\n    term;");
    let definition = single_definition(&parsed);
    assert_eq!(
        render_type(&parsed, definition.result),
        "(value: A) -> F(value)"
    );

    // Parentheses make a function type the domain, defeating right
    // associativity for that step.
    let parsed = parse("let higher(u: core::Level): (A -> B) -> C =\n    term;");
    let definition = single_definition(&parsed);
    assert_eq!(render_type(&parsed, definition.result), "A -> B -> C");
    assert!(matches!(
        parsed.items.mathematical_type(definition.result),
        MathematicalTypeNode::Arrow { domain, .. }
            if matches!(
                parsed.items.mathematical_type(*domain),
                MathematicalTypeNode::Arrow { .. }
            )
    ));
}

#[test]
fn erased_marker_parses_on_mathematical_parameters() {
    let parsed = parse("let keep(proof [erased]: P): Q = term;");
    let definition = single_definition(&parsed);
    let parameters = parsed.items.mathematical_parameters(definition.parameters);
    let proof = parsed.items.mathematical_parameter(parameters[0]);
    assert_eq!(proof.relevance, language_core::BindingRelevance::Erased);
}

#[test]
fn empty_parameter_list_names_a_nullary_definition() {
    let parsed = parse("let pi(): core::Real = term;");
    let definition = single_definition(&parsed);
    assert_eq!(definition.name.as_str(), "pi");
    assert!(definition.parameters.is_empty());
    assert_eq!(render_type(&parsed, definition.result), "core::Real");
}

#[test]
fn pub_marks_mathematical_declarations() {
    let parsed = parse("pub let open(): A = term;\npub boundary let ax(): B;");
    let handles = parsed.root_mathematical_definition_handles();
    assert_eq!(handles.len(), 2);
    assert!(parsed.root_mathematical_definition(handles[0]).is_public);
    let boundary = parsed.root_mathematical_definition(handles[1]);
    assert!(boundary.is_public);
    assert_eq!(boundary.body, MathematicalDefinitionBody::Assumption);
}

#[test]
fn top_level_let_requires_a_body_or_boundary() {
    let tokens = Lexer::new("let f(): A;").tokenize().expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
    assert!(
        error.message.contains("boundary let"),
        "directed error mentions the assumption spelling: {}",
        error.message
    );
}

#[test]
fn boundary_let_rejects_a_body() {
    let tokens = Lexer::new("boundary let f(): A = term;")
        .tokenize()
        .expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
    assert!(
        error.message.contains("`;`"),
        "bodyless form rejects a body: {}",
        error.message
    );
}

#[test]
fn let_declaration_rejects_lifetime_and_machine_binders() {
    let tokens = Lexer::new("let f<'a>(): A = term;")
        .tokenize()
        .expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
    assert!(error.message.contains("lifetime"), "{}", error.message);

    let tokens = Lexer::new("let f<machine M>(): A = term;")
        .tokenize()
        .expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
    assert!(error.message.contains("machine"), "{}", error.message);
}

#[test]
fn dependent_binder_requires_an_arrow() {
    let tokens = Lexer::new("let f(): (value: A) = term;")
        .tokenize()
        .expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("parse must reject");
    assert!(error.message.contains("->"), "{}", error.message);
}
