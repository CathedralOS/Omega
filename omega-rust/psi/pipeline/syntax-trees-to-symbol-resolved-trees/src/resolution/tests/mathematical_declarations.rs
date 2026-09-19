//! Top-level `let`/`boundary let` mathematical declarations resolve into
//! `SymbolResolvedRoots::mathematical_definitions` with real symbols
//! (PROOF-CONTRACT-MIGRATION): the declaration, its binders and its
//! parameters, plus name resolution inside carriers, results and bodies.

use crate::resolution::{ExtensionRequest, ResolutionRequest, resolve_extension};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::mathematical::{MathematicalBody, MathematicalType};
use symbol_resolved_trees::types::TypeReference;
use symbols::SymbolKind;
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

fn resolve(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    crate::resolve(ResolutionRequest::new(&syntax)).expect("resolve")
}

#[test]
fn let_definition_resolves_onto_its_own_root_surface() {
    let program = resolve("let double(x: u64): u64 = x;");

    assert_eq!(program.mathematical_definitions.len(), 1);
    let definition = program.mathematical_definitions.first().unwrap();
    assert!(definition.symbol.is_valid());
    assert_eq!(
        program.symbols.get(definition.symbol).kind,
        SymbolKind::MathematicalDefinition
    );
    assert_eq!(definition.name.as_str(), "double");

    let [parameter] = program.mathematical_parameters(definition.parameters) else {
        panic!("one telescope parameter")
    };
    assert!(parameter.symbol.is_valid());
    assert_eq!(
        program.symbols.get(parameter.symbol).kind,
        SymbolKind::Parameter
    );
    assert!(
        program
            .symbols
            .child_handles(definition.symbol)
            .into_iter()
            .flatten()
            .any(|child| child == parameter.symbol)
    );

    // The `u64` result is an ordinary resolved type reference.
    let MathematicalType::Ordinary(result) = program.mathematical_type(definition.result) else {
        panic!("ordinary result type")
    };
    let TypeReference::Named { symbol, .. } = result else {
        panic!("builtin result resolves as a named reference")
    };
    assert_eq!(program.symbols.get(*symbol).kind, SymbolKind::BuiltinType);

    // The body `x` names the parameter child.
    let MathematicalBody::Definition(body) = definition.body else {
        panic!("transparent definition body")
    };
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(body) else {
        panic!("parameter reference body")
    };
    assert_eq!(path.symbol, parameter.symbol);
    assert_eq!(path.head_symbol, parameter.symbol);
}

#[test]
fn boundary_let_resolves_as_a_named_assumption() {
    let program = resolve("boundary let choose(inhabited: u64): u64;");

    let definition = program.mathematical_definitions.first().unwrap();
    assert!(definition.symbol.is_valid());
    assert_eq!(definition.body, MathematicalBody::Assumption);

    let [parameter] = program.mathematical_parameters(definition.parameters) else {
        panic!("one telescope parameter")
    };
    assert_eq!(parameter.name.as_str(), "inhabited");
    assert_eq!(
        program.symbols.get(parameter.symbol).kind,
        SymbolKind::Parameter
    );
}

#[test]
fn type_binders_scope_over_the_result_and_body() {
    let program = resolve("let id<A>(value: A): A = value;");

    let definition = program.mathematical_definitions.first().unwrap();
    let binders = program
        .tables
        .declarations
        .data_type_parameters
        .span_or_empty(definition.binders);
    let [binder] = binders else {
        panic!("one type binder")
    };
    assert_eq!(binder.name.as_str(), "A");
    assert!(binder.symbol.is_valid());
    assert_eq!(
        program.symbols.get(binder.symbol).kind,
        SymbolKind::TypeParameter
    );

    // The `A` result resolves to the binder's child symbol.
    let MathematicalType::Ordinary(result) = program.mathematical_type(definition.result) else {
        panic!("ordinary result type")
    };
    let TypeReference::Named { symbol, .. } = result else {
        panic!("binder result resolves as a named reference")
    };
    assert_eq!(*symbol, binder.symbol);

    let [parameter] = program.mathematical_parameters(definition.parameters) else {
        panic!("one telescope parameter")
    };
    let MathematicalBody::Definition(body) = definition.body else {
        panic!("transparent definition body")
    };
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(body) else {
        panic!("parameter reference body")
    };
    assert_eq!(path.symbol, parameter.symbol);
}

#[test]
fn calls_between_mathematical_declarations_resolve() {
    let program = resolve(
        "let plus(a: u64, b: u64): u64 = a;\n\
         let two(): u64 = plus(plus(1, 1), 1);",
    );

    assert_eq!(program.mathematical_definitions.len(), 2);
    let mut definitions = program.mathematical_definitions.iter();
    let plus = definitions.next().unwrap();
    let two = definitions.next().unwrap();
    assert_eq!(plus.name.as_str(), "plus");
    assert_eq!(two.name.as_str(), "two");

    let MathematicalBody::Definition(body) = two.body else {
        panic!("transparent definition body")
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(body) else {
        panic!("call body")
    };
    assert_eq!(call.target_symbol, plus.symbol);

    // Nested `plus(1, 1)` arguments resolve to the same declaration symbol.
    let arguments = program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments);
    let ExpressionNode::Call(nested) = program.tables.bodies.expressions.expression(arguments[0])
    else {
        panic!("nested call argument")
    };
    assert_eq!(nested.target_symbol, plus.symbol);
}

#[test]
fn dependent_arrows_and_type_applications_survive_resolution() {
    let program = resolve("let apply<A>(f: A -> A, value: A): F(value) = f;");

    let definition = program.mathematical_definitions.first().unwrap();
    let [f, value] = program.mathematical_parameters(definition.parameters) else {
        panic!("two telescope parameters")
    };
    assert_eq!(f.name.as_str(), "f");
    assert_eq!(value.name.as_str(), "value");

    // `f: A -> A` is an Arrow node over the binder `A` on both sides.
    let MathematicalType::Arrow {
        binder,
        domain,
        codomain,
    } = program.mathematical_type(f.ty)
    else {
        panic!("arrow parameter type")
    };
    assert!(binder.is_none());
    let [a] = program
        .tables
        .declarations
        .data_type_parameters
        .span_or_empty(definition.binders)
    else {
        panic!("one type binder")
    };
    for side in [*domain, *codomain] {
        let MathematicalType::Ordinary(side) = program.mathematical_type(side) else {
            panic!("ordinary arrow side")
        };
        let TypeReference::Named { symbol, .. } = side else {
            panic!("binder reference")
        };
        assert_eq!(*symbol, a.symbol);
    }

    // The `F(value)` result is a type-level application whose argument
    // expression names the `value` parameter.
    let MathematicalType::Application { callee, arguments } =
        program.mathematical_type(definition.result)
    else {
        panic!("type-level application result")
    };
    let MathematicalType::Ordinary(callee) = program.mathematical_type(*callee) else {
        panic!("ordinary application callee")
    };
    let TypeReference::Named { symbol, .. } = callee else {
        panic!("callee reference")
    };
    // `F` is not a declared binder here, so the callee stays unresolved at
    // this stage; the argument still resolves against the telescope.
    assert!(!symbol.is_valid());
    let [argument] = program
        .tables
        .bodies
        .expressions
        .expression_handles(*arguments)
    else {
        panic!("one application argument")
    };
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(*argument) else {
        panic!("argument name")
    };
    assert_eq!(path.symbol, value.symbol);
}

#[test]
fn public_let_marks_the_declaration_public() {
    let program = resolve("pub let answer(): u64 = 42;");
    let definition = program.mathematical_definitions.first().unwrap();
    assert!(definition.is_public);
    assert!(definition.symbol.is_valid());
}

#[test]
fn seeded_extension_keeps_mathematical_declarations_and_their_symbols() {
    let base_source = "let base_fn(x: u64): u64 = x;";
    let extension_source = "let extended_fn(y: u64): u64 = base_fn(y);";
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
    let base = crate::resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .unwrap();
    let retained = base.mathematical_definitions.first().unwrap().clone();
    let retained_source = base.symbols.symbol_source_span(retained.symbol);

    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source).tokenize().unwrap(),
    )
    .unwrap();
    let program = resolve_extension(ExtensionRequest {
        base,
        syntax: &extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .map(|seeded| seeded.into_unrebased_trees())
    .expect("extend existing mathematical symbols");

    let definitions = program.mathematical_definitions.iter().collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    // The retained declaration and its symbol identity survive untouched.
    assert_eq!(definitions[0], &retained);
    assert_eq!(
        program.symbols.symbol_source_span(retained.symbol),
        retained_source
    );
    // The extension's body resolves `base_fn` to the retained declaration.
    let MathematicalBody::Definition(body) = definitions[1].body else {
        panic!("transparent extension body")
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(body) else {
        panic!("extension call body")
    };
    assert_eq!(call.target_symbol, retained.symbol);
}
