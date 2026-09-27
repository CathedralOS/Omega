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
use symbols::{FixedCoreDeclaration, SymbolKind};
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

fn resolve(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    crate::resolve(ResolutionRequest::new(&syntax)).expect("resolve")
}

#[test]
fn boundary_let_resolves_as_a_named_assumption() {
    let program = resolve("pub boundary let choose(inhabited: u64): u64;");

    let definition = program.mathematical_definitions.first().unwrap();
    assert!(definition.is_public);
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
fn dependent_arrows_and_type_applications_survive_resolution() {
    let program = resolve("pub let apply<A>(f: A -> A, value: A): F(value) = f;");

    let definition = program.mathematical_definitions.first().unwrap();
    assert!(definition.is_public);
    assert!(definition.symbol.is_valid());
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

/// The fixed `core::*` mathematical declarations are compiler-installed
/// root children, not authored source: binder carriers, results and bodies
/// spelled `core::Level`/`core::Type<u>`/`core::Squash<A>` resolve to the
/// canonical fixed slots, while the authored declaration keeps a distinct
/// authored symbol after them.
#[test]
fn fixed_core_vocabulary_binds_canonical_symbols() {
    let program =
        resolve("let sorted<u: core::Level, A: core::Type<u>>(x: u64): core::Squash<A> = x;");

    let definition = program.mathematical_definitions.first().unwrap();
    let fixed = |member: FixedCoreDeclaration| {
        program
            .symbols
            .fixed_core_declaration_symbol(member)
            .expect("fixed core declaration installed")
    };

    // The authored declaration is none of the fixed slots — and every fixed
    // slot classifies back through the table's own identity check.
    for member in FixedCoreDeclaration::ALL {
        assert_eq!(
            program.symbols.fixed_core_declaration(fixed(member)),
            Some(member)
        );
    }
    assert!(definition.symbol.is_valid());
    assert_eq!(
        program.symbols.fixed_core_declaration(definition.symbol),
        None
    );

    let [u, a] = program
        .tables
        .declarations
        .data_type_parameters
        .span_or_empty(definition.binders)
    else {
        panic!("two binders")
    };
    // `u: core::Level` — a Named reference bound to the fixed `Level` slot.
    let symbol_resolved_trees::data::TypeParameterKind::Value {
        type_reference: u_carrier,
    } = &u.kind
    else {
        panic!("level binder carries a value type")
    };
    let TypeReference::Named { symbol, name } = u_carrier else {
        panic!("level carrier is a named reference")
    };
    assert_eq!(name.as_str(), "core::Level");
    assert_eq!(*symbol, fixed(FixedCoreDeclaration::Level));

    // `A: core::Type<u>` — a Generic base bound to the fixed `Type` slot.
    let symbol_resolved_trees::data::TypeParameterKind::Value {
        type_reference: a_carrier,
    } = &a.kind
    else {
        panic!("sort binder carries a value type")
    };
    let TypeReference::Generic(generic) = a_carrier else {
        panic!("sort carrier is a generic reference")
    };
    assert_eq!(generic.base_name.as_str(), "core::Type");
    assert_eq!(generic.base_symbol, fixed(FixedCoreDeclaration::Type));

    // `core::Squash<A>` result — Generic base bound to fixed `Squash`.
    let MathematicalType::Ordinary(result) = program.mathematical_type(definition.result) else {
        panic!("ordinary result type")
    };
    let TypeReference::Generic(generic) = result else {
        panic!("squash result is a generic reference")
    };
    assert_eq!(generic.base_symbol, fixed(FixedCoreDeclaration::Squash));
}

/// A module-scoped authored `Type` inside `core` is the false twin: the
/// canonical `core::Type` spelling still binds the fixed vocabulary slot,
/// and the authored data keeps its own ordinary symbol — never classified
/// as the sort.
#[test]
fn authored_core_spelling_cannot_shadow_fixed_vocabulary() {
    let source = "module core; pub data Type { marker: u64 } let uses(x: u64): core::Type = x;";
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("core.omg"), source.to_owned())
        .source_id;
    let syntax =
        parse_syntax_trees_with_id(source_id, &Lexer::new(source).tokenize().unwrap()).unwrap();
    let program = crate::resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve module-scoped same-spelled declaration");

    let fixed_type = program
        .symbols
        .fixed_core_declaration_symbol(FixedCoreDeclaration::Type)
        .expect("fixed core::Type installed");
    let authored_twin = program
        .data_definitions
        .first()
        .expect("authored Type data exists")
        .symbol;
    assert!(authored_twin.is_valid());
    assert_eq!(program.symbols.get(authored_twin).kind, SymbolKind::Data);
    assert_eq!(
        program.symbols.fixed_core_declaration(authored_twin),
        None,
        "the module-scoped twin never acquires fixed identity"
    );

    // `core::Type` in type position still binds the fixed declaration, not
    // the same-spelled authored data.
    let definition = program.mathematical_definitions.first().unwrap();
    let MathematicalType::Ordinary(result) = program.mathematical_type(definition.result) else {
        panic!("ordinary result type")
    };
    let TypeReference::Named { symbol, .. } = result else {
        panic!("named result reference")
    };
    assert_eq!(*symbol, fixed_type);
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
    .map(|seeded| seeded.trees().clone())
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
