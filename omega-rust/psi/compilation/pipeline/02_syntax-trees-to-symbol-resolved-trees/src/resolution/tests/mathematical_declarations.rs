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
use symbol_resolved_trees::proposition::PropositionBody;
use symbol_resolved_trees::types::TypeReference;
use symbols::{FixedCoreDeclaration, SymbolKind};
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

fn resolve(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    crate::resolve(ResolutionRequest::new(syntax)).expect("resolve")
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
        syntax,
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

/// Proposition bodies share the proof-surface expression walk but keep
/// their authored call kinds at `[Proposition]`: qualified `core::*`
/// spellings still bind their compiler-installed declarations there —
/// the qualified-call pass owns them because `core` is the vocabulary's
/// fixed namespace, never a `Module` — while authored proposition calls
/// keep resolving through the ordinary chain.
#[test]
fn proposition_call_targets_bind_the_fixed_vocabulary() {
    let program = resolve(
        "proposition base(x: u64) = x == x; proposition squashed(x: u64) = core::Squash(x); proposition via(x: u64) = base(x);",
    );
    let squash = program
        .symbols
        .fixed_core_declaration_symbol(FixedCoreDeclaration::Squash)
        .expect("fixed core::Squash installed");
    let propositions = program
        .propositions
        .iter()
        .map(|proposition| (proposition.name.as_str(), proposition))
        .collect::<std::collections::BTreeMap<_, _>>();

    let PropositionBody::Transparent { proposition: body } = propositions["squashed"].body else {
        panic!("transparent proposition body")
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(body) else {
        panic!("squashed body is a call")
    };
    assert_eq!(call.target.as_str(), "core::Squash");
    assert_eq!(call.target_symbol, squash);
    assert!(
        !call.receiver.is_valid(),
        "the fixed binding leaves no unresolved qualifier receiver"
    );

    // An authored proposition call resolves through the unchanged kind set.
    let PropositionBody::Transparent { proposition: body } = propositions["via"].body else {
        panic!("transparent proposition body")
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(body) else {
        panic!("via body is a call")
    };
    assert_eq!(call.target_symbol, propositions["base"].symbol);
}

/// A `core` module authored beside the vocabulary still cannot claim a
/// fixed name at a call site: `core::Squash(x)` binds the installed
/// declaration, never the module's same-spelled proposition — the same
/// non-shadowing law type references follow.
#[test]
fn an_authored_core_module_cannot_steal_a_fixed_call_target() {
    let source = "module core; pub proposition Squash(x: u64) = x == x; proposition caller(x: u64) = core::Squash(x);";
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("core.omg"), source.to_owned())
        .source_id;
    let syntax =
        parse_syntax_trees_with_id(source_id, &Lexer::new(source).tokenize().unwrap()).unwrap();
    let program = crate::resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve module-scoped same-spelled call");

    let squash = program
        .symbols
        .fixed_core_declaration_symbol(FixedCoreDeclaration::Squash)
        .expect("fixed core::Squash installed");
    let authored_twin = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "Squash")
        .expect("authored Squash proposition exists")
        .symbol;
    assert_ne!(authored_twin, squash);

    let caller = program
        .propositions
        .iter()
        .find(|proposition| proposition.name.as_str() == "caller")
        .expect("caller proposition");
    let PropositionBody::Transparent { proposition: body } = caller.body else {
        panic!("transparent proposition body")
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(body) else {
        panic!("caller body is a call")
    };
    assert_eq!(
        call.target_symbol, squash,
        "the fixed `core::Squash` owns its exact spelling even inside an authored `core` module"
    );
}

/// A bare `core::*` name path in a proposition body carries no call: it
/// binds the installed declaration by exact spelling, so no symbol-less
/// reference reaches the transitional spelling classifier — and the
/// namespace head itself stays unbound because `core` is not a
/// declaration.
#[test]
fn proposition_name_paths_bind_the_fixed_vocabulary() {
    let program = resolve(
        "proposition leveled(x: u64) = core::Level; proposition squashed(x: u64) = base(core::Squash); proposition base(x: u64) = x == x;",
    );
    let level = program
        .symbols
        .fixed_core_declaration_symbol(FixedCoreDeclaration::Level)
        .expect("fixed core::Level installed");
    let squash = program
        .symbols
        .fixed_core_declaration_symbol(FixedCoreDeclaration::Squash)
        .expect("fixed core::Squash installed");
    let propositions = program
        .propositions
        .iter()
        .map(|proposition| (proposition.name.as_str(), proposition))
        .collect::<std::collections::BTreeMap<_, _>>();

    let PropositionBody::Transparent { proposition: body } = propositions["leveled"].body else {
        panic!("transparent proposition body")
    };
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(body) else {
        panic!("leveled body is a name path")
    };
    assert_eq!(path.symbol, level);
    assert!(
        !path.head_symbol.is_valid(),
        "`core` is the fixed namespace, not a resolvable declaration"
    );

    // The same law applies inside a call argument: the `core::Squash`
    // operand is a name path, not a call target.
    let PropositionBody::Transparent { proposition: body } = propositions["squashed"].body else {
        panic!("transparent proposition body")
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(body) else {
        panic!("squashed body is a call")
    };
    let [argument] = program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
    else {
        panic!("one call argument")
    };
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(*argument) else {
        panic!("argument is a name path")
    };
    assert_eq!(path.symbol, squash);
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
        syntax: base_syntax,
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
        syntax: extension_syntax,
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
