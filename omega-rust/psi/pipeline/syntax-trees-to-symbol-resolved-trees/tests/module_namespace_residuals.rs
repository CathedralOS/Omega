//! The MODULE-NAMESPACE-RESOLUTION residual legs on real source custody:
//! module-scoped and foreign const carriers, indexed domain families, trait
//! defaults, operator homes, and qualified case membership in declared-domain
//! proof facts, each lowered through the loader's `SourceMap` so package
//! visibility and module-local precedence are live. Const initializers which
//! need evaluation (calls, matches, const references, computed leaves) are
//! covered through the pre-resolution evaluator in build-time-evaluation's
//! `generic_application_carriers` tests, since this harness runs only
//! normalize+resolve.
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn lower_multi(sources: &[(&str, &str)]) -> Result<SymbolResolvedTrees, String> {
    let mut map = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (name, text) in sources {
        let id = map.add(PathBuf::from(name), (*text).to_owned()).source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        parse_syntax_trees_into_with_id(&mut syntax, id, &tokens).expect("parse");
    }
    let syntax = normalize_generic_data(GenericDataRequest {
        syntax,
        sources: Some(Arc::new(map.clone())),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .map_err(|errors| {
        errors
            .iter()
            .map(|error| format!("normalize: {}", error.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(map)),
        top_level_bindings: Vec::new(),
    })
    .map_err(|errors| {
        errors
            .iter()
            .map(|error| format!("resolve: {}", error.message))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn const_named<'program>(program: &'program SymbolResolvedTrees, name: &str) -> String {
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| {
            program
                .symbols
                .display_path(declaration.symbol, "::")
                .ends_with(name)
        })
        .unwrap_or_else(|| panic!("const `{name}`"));
    program.symbols.display_path(declaration.symbol, "::")
}

fn domain_symbol_path(program: &SymbolResolvedTrees, name: &str) -> String {
    let definition = program
        .domain_definitions
        .iter()
        .find(|definition| {
            program
                .symbols
                .display_path(definition.symbol, "::")
                .ends_with(name)
        })
        .unwrap_or_else(|| panic!("domain `{name}`"));
    program.symbols.display_path(definition.symbol, "::")
}

#[test]
fn foreign_nominal_const_attachment() {
    for (tag, declaring) in [
        (
            "qualified",
            "module mine; const P: geom::Point = geom::Point { x: 1 };",
        ),
        (
            "imported",
            "module mine; use geom::Point; const P: Point = Point { x: 1 };",
        ),
    ] {
        let program = lower_multi(&[
            ("geom.omg", "module geom; pub data Point { x: u64; }"),
            ("mine.omg", declaring),
        ])
        .unwrap_or_else(|e| panic!("{tag} foreign nominal module const: {e}"));
        assert_eq!(const_named(&program, "P"), "mine::P", "{tag}");
    }
}

#[test]
fn specialized_foreign_template_const_attachment() {
    // A module const attached to a specialized foreign template: the closed
    // instance is owned by geom's module path.
    let program = lower_multi(&[
        ("geom.omg", "module geom; pub data Box<T> { value: T; }"),
        (
            "mine.omg",
            "module mine; const B: geom::Box<u64> = geom::Box { value: 3 };",
        ),
    ])
    .expect("specialized foreign template const resolves");
    assert_eq!(const_named(&program, "B"), "mine::B");
    assert!(
        program
            .data_definitions
            .iter()
            .any(|definition| definition.generic_instance.is_some()),
        "the closed `Box<u64>` instance materializes"
    );
}

#[test]
fn open_template_index_membership_on_value_type() {
    // The proof-fact membership `K in Window<8>` carries the indexed domain
    // application to the resolver, which interns the instance identity.
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain<const N: u64> u64::Window<N> requires self < N;",
        ),
        (
            "mine.omg",
            "module mine; use units::Window; data Holder<const K: u64> where K in Window<8>, { v: u64; }",
        ),
    ])
    .expect("open-template indexed membership resolves");
    // Indexed/generic domain families name themselves `Window` (the carrier
    // prefix is only folded into unindexed domain names).
    assert_eq!(domain_symbol_path(&program, "Window"), "units::Window");
}

#[test]
fn module_indexed_domain_family() {
    let program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        domain<const N: u64> u64::Window<N> requires self < N;
        data Small { v: u64 in Window<8>; }
        "#,
    )])
    .expect("module indexed domain family resolves");
    assert_eq!(domain_symbol_path(&program, "Window"), "units::Window");
}

#[test]
fn trait_default_through_import() {
    let program = lower_multi(&[
        (
            "svc.omg",
            "module svc; pub trait Service { machine run(&mut self) -> u64 { 7 } }",
        ),
        (
            "mine.omg",
            "module mine; use svc::Service; data Worker {} membership: Worker satisfies Service;",
        ),
    ])
    .expect("module trait default resolves");
    assert!(
        program.data_definitions.iter().any(|definition| program
            .symbols
            .display_path(definition.symbol, "::")
            == "mine::Worker")
    );
}

#[test]
fn operator_home_qualified_parameter_domain() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Distance requires self > 0; pub operator + u64::Distance::add(left: u64 in u64::Distance, right: u64 in u64::Distance) -> u64 in u64::Distance;",
        ),
        (
            "main.omg",
            "use units; machine m(a: u64 in units::u64::Distance, b: u64 in units::u64::Distance) -> bool { true }",
        ),
    ])
    .expect("module operator home resolves");
    assert_eq!(
        domain_symbol_path(&program, "Distance"),
        "units::u64::Distance"
    );
}

#[test]
fn qualified_case_membership_in_foreign_domain_fact() {
    // A module-owned domain whose fact is membership in a FOREIGN qualified
    // case path resolves to the exact case owner and case symbol.
    let program = lower_multi(&[
        (
            "shapes.omg",
            "module shapes; pub data Choice { case Empty; case Some(v: u32); }",
        ),
        (
            "policy.omg",
            "module policy; use shapes::Choice; domain Choice::NonEmpty requires self in shapes::Choice::Some;",
        ),
    ])
    .expect("foreign qualified case in module domain fact resolves");
    assert_eq!(
        domain_symbol_path(&program, "NonEmpty"),
        "policy::Choice::NonEmpty"
    );
}

#[test]
fn module_const_used_cross_module_as_array_length() {
    let program = lower_multi(&[
        ("m.omg", "module m; pub const SIZE: u64 = 4;"),
        (
            "main.omg",
            "use m::SIZE; data Buffer { values: [u8; SIZE]; }",
        ),
    ])
    .expect("module const as foreign array length resolves");
    assert_eq!(const_named(&program, "SIZE"), "m::SIZE");
}

#[test]
fn contested_indexed_domain_family_prefers_module_local() {
    // An indexed domain family used as a value constraint inside a module
    // where both a local and foreign family contest the leaf.
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain<const N: u64> u64::Window<N> requires self < N;",
        ),
        (
            "mine.omg",
            "module mine; use units; domain<const N: u64> u64::Window<N> requires self <= N; data S { v: u64 in Window<8>; }",
        ),
    ])
    .expect("contested indexed domain family resolves");
    assert!(
        program.domain_definitions.iter().any(|definition| {
            program.symbols.display_path(definition.symbol, "::") == "mine::Window"
        }),
        "the module-local Window declares beside the imported family"
    );
}

#[test]
fn narrow_case_import_in_membership_fact() {
    // `use shapes::Choice::Some` exposes the exact case to `c in Some` ...
    lower_multi(&[
        (
            "shapes.omg",
            "module shapes; pub data Choice { case Empty; case Some(v: u32); }",
        ),
        (
            "check.omg",
            "use shapes::Choice::Some; data Holder where c in Choice::Some, { c: shapes::Choice; }",
        ),
    ])
    .expect("narrow case import in fact resolves");
}

#[test]
fn qualified_indexed_domain_constraint() {
    // Qualified spelling of a module's indexed domain family as a field
    // constraint.
    lower_multi(&[
        (
            "units.omg",
            "module units; pub domain<const N: u64> u64::Window<N> requires self < N;",
        ),
        ("main.omg", "data S { v: u64 in units::u64::Window<8>; }"),
    ])
    .expect("qualified indexed domain constraint resolves");
}

#[test]
fn domain_and_case_same_leaf_contest_rejects_ambiguous() {
    // `Choice::Some` is both a domain leaf and a case leaf on the same
    // carrier. Membership cannot decide between the two declared roles, so
    // the same-leaf contest must reject rather than guess.
    let error = lower_multi(&[
        (
            "a.omg",
            "module a; pub data Choice { case Empty; case Some(v: u32); } pub domain Choice::Some;",
        ),
        (
            "check.omg",
            "use a::Choice; machine m(c: Choice) -> bool { c in Choice::Some }",
        ),
    ])
    .expect_err("same-leaf domain/case contest is ambiguous");
    assert!(
        error.contains("ambiguous membership `Choice::Some`"),
        "unexpected diagnostic: {error}"
    );
}
