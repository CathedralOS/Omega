//! Token bindings survive resolution; bindings without a semantic home in their
//! operand tuple and duplicate owner-local shapes reject.

use crate::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve_source(source: &str) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    resolve(ResolutionRequest::new(&syntax))
}

#[test]
fn free_binding_over_bare_primitives_rejects_at_the_declaration() {
    let diagnostics = resolve_source("machine + add(left: u8, right: u8) -> u64 { 0u64 }")
        .expect_err("a primitive family has no declaration-owned home");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`add` binds the fixed operator token `+` over operands that name no declared \
             type or domain"
        ),
        "{}",
        diagnostic.message
    );
}

#[test]
fn a_home_anywhere_in_the_operand_tuple_participates() {
    let program = resolve_source(
        "data Wrapped { value: u8; }
         data Pair<T> { first: T; second: T; }
         domain u8::Level;
         machine + Wrapped::scale(factor: u64, value: &Wrapped) -> u64 { factor }
         machine * Wrapped::spread(items: [Wrapped], factor: u64) -> u64 { factor }
         machine - Wrapped::inside(pair: Pair<Wrapped>, factor: u64) -> u64 { factor }
         machine + add(left: u8 in Level, right: u8) -> u64 { 0u64 }
         machine - subtract(left: &Wrapped, right: u64) -> u64 { right }",
    )
    .expect("a reference, element, generic argument, or domain constraint names the home");
    assert_eq!(program.machines.len(), 5);
}

#[test]
fn attached_self_bindings_with_one_shape_reject_as_duplicates() {
    // Two `&self` bindings on one owner spell `Self` as their own machine
    // symbols; both carry the attached data's operand shape, so they collide.
    let diagnostics = resolve_source(
        "data Buffer { value: i32; }
         data Index {}
         machine [] Buffer::index(&self, index: Index) -> i32 { self.value }
         machine [] Buffer::at(&self, index: Index) -> i32 { self.value }",
    )
    .expect_err("two `&self` bindings on one owner share the operand shape");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic per duplicate: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`Buffer::at` binds the fixed operator token `[]` already bound by `Buffer::index`"
        ),
        "{}",
        diagnostic.message
    );
}

#[test]
fn distinct_operand_shapes_may_share_a_token() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec2::scale(left: Vec2, factor: u64) -> Vec2 { left }
         machine + Vec2::add_borrowed(left: &Vec2, right: &Vec2) -> u64 { 0u64 }
         machine + Vec2::add_mutable(left: &mut Vec2, right: &Vec2) -> u64 { 0u64 }",
    )
    .expect("overloads by operand shape are not duplicates");
    assert_eq!(program.machines.len(), 4);
}

#[test]
fn domain_operator_repeating_a_domain_machine_binding_rejects() {
    let diagnostics = resolve_source(
        "data Quantity { value: i32; }
         domain Quantity::Additive requires self.value >= 0;
         operator + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;
         machine + Quantity::Additive::plus(left: Quantity, right: Quantity) -> Quantity { left }",
    )
    .expect_err("the domain operator repeats the domain machine's token, owner, and shape");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already bound by `Quantity::Additive::plus`")),
        "{diagnostics:?}"
    );
}

#[test]
fn generic_domain_binding_homes_through_qualified_operands_and_index_arguments_shape() {
    // `domain<T, const U: Unit> T::Quantity<U>` classifies an open carrier, so
    // an operand participates by carrying the domain; distinct index
    // arguments are distinct operand shapes under the one owner.
    let source = "data Unit [copy] { scale: i32; }
         data Units {}
         const Units::METER: Unit = Unit { scale: 1 };
         const Units::KILOMETER: Unit = Unit { scale: 1000 };
         domain<T, const U: Unit> T::Quantity<U>;
         machine + Quantity::add_meters(
             left: f64 in Quantity<Units::METER>,
             right: f64 in Quantity<Units::METER>
         ) -> f64 in Quantity<Units::METER> { left }
         machine + Quantity::add_kilometers(
             left: f64 in Quantity<Units::KILOMETER>,
             right: f64 in Quantity<Units::KILOMETER>
         ) -> f64 in Quantity<Units::KILOMETER> { left }";
    resolve_source(source).expect("qualified operands home in the generic domain");

    let diagnostics = resolve_source(
        "data Unit [copy] { scale: i32; }
         domain<T, const U: Unit> T::Quantity<U>;
         machine + Quantity::add(left: f64, right: f64) -> f64 { left }",
    )
    .expect_err("bare carrier operands name no home of a generic domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no operand names the carrier of its home domain `Quantity`")),
        "{diagnostics:?}"
    );

    let diagnostics = resolve_source(&source.replace(
        "Quantity<Units::KILOMETER>,\n             right: f64 in Quantity<Units::KILOMETER>",
        "Quantity<Units::METER>,\n             right: f64 in Quantity<Units::METER>",
    ))
    .expect_err("the same index arguments repeat the first binding's shape");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Quantity::add_kilometers` binds the fixed operator token `+` already bound by `Quantity::add_meters`"
        )),
        "{diagnostics:?}"
    );
}

fn resolve_sealed_core_source(
    source: &str,
) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    resolve_sealed_core_file(
        numerics::float_projection::FLOAT_PROJECTION_CORE_SOURCE,
        source,
    )
}

fn resolve_sealed_core_file(
    relative: &str,
    source: &str,
) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    use std::{path::PathBuf, sync::Arc};
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("/toolchain/core").join(relative),
            source.to_owned(),
            PathBuf::from("/toolchain/core"),
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("syntax");
    let mut request = ResolutionRequest::new(&syntax);
    request.sources = Some(Arc::new(sources));
    resolve(request)
}

const SEALED_FLOAT_PRELUDE: &str = "data FloatMeaning { value: u64; }
     data FloatFormat { width: u8; }
     data FloatClass { kind: u8; }";

#[test]
fn sealed_float_semantics_signature_that_drifts_from_its_row_rejects() {
    let cases: &[(&str, &str)] = &[
        (
            "machine FloatSemantics::add(left: FloatMeaning, right: FloatMeaning) -> FloatMeaning;",
            "its signature (FloatMeaning, FloatMeaning) -> FloatMeaning matches no catalog row",
        ),
        (
            "machine FloatSemantics::from_integer(format: FloatFormat, value: i128) -> FloatMeaning;",
            "parameter `value` type `i128` is not a catalog value kind",
        ),
        (
            "machine FloatSemantics::less(left: FloatMeaning, right: FloatMeaning) -> FloatMeaning;",
            "its signature (FloatMeaning, FloatMeaning) -> FloatMeaning matches no catalog row",
        ),
        (
            "machine FloatSemantics::negate<T>(format: FloatFormat, value: FloatMeaning) -> FloatMeaning;",
            "catalog rows declare no type or lifetime parameters",
        ),
        (
            "machine FloatSemantics::is_nan(value: &FloatMeaning) -> bool;",
            "parameter `value` type is not a named catalog value kind",
        ),
    ];
    for (declaration, expected) in cases {
        let diagnostics =
            resolve_sealed_core_source(&format!("{SEALED_FLOAT_PRELUDE}\n{declaration}"))
                .expect_err(declaration);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("names the compiler float-semantics catalog, but")
                && diagnostic.message.contains(expected)
                && diagnostic
                    .message
                    .contains("the sealed declaration must match one catalog row exactly")),
            "{declaration}: {diagnostics:?}"
        );
    }
}

#[test]
fn bodiless_token_bearing_nonboundary_machine_rejects_missing_supply() {
    // `machine + Owner::name(...);` outside `boundary` and the sealed catalog
    // has no supply mechanism: it is neither a checked-body declaration nor a
    // requirement slot, so the declaration itself rejects
    // ([expressions: executable supply](../../../../../../wiki/spec/language/expressions.md#executable-supply)).
    let diagnostics = resolve_source(
        "data Wrapped { value: u8; }
         machine + Wrapped::add(left: Wrapped, right: Wrapped) -> Wrapped;",
    )
    .expect_err("a token binding without a body or a requirement context has no supply");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`Wrapped::add` has no body and is neither a boundary signature nor a \
             compiler-catalog primitive; a nonboundary direct machine must own a checked body \
             (`machine + Wrapped::add(...) { ... }`), a boundary contract is spelled `boundary \
             machine + ...;`, and an external leaf `satisfies Requirement via <Binding>;`"
        ),
        "{}",
        diagnostic.message
    );
}

#[test]
fn binders_reordered_across_introducers_reject_as_duplicates() {
    // The machine and operator forms share one binding space, so the
    // alpha-equivalence rule crosses introducers too.
    let diagnostics = resolve_source(
        "data Pair<T, U> { first: T; second: U; }
         machine + Pair::combine<T, U>(left: Pair<T, U>, right: U) -> Pair<T, U> { left }
         operator + Pair::join<A, B>(left: Pair<B, A>, right: A) -> Pair<B, A>;",
    )
    .expect_err("an operator repeating a machine's alpha-equivalent shape duplicates");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message.contains(
            "`Pair::join` binds the fixed operator token `+` already bound by `Pair::combine`"
        ),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn sealed_ranking_view_signature_that_drifts_from_its_row_rejects() {
    for (declaration, expected) in [
        (
            "machine Nat::Descending(value: u32) -> u64;",
            "parameter `value` is not the row's `u64` subject",
        ),
        (
            "machine Nat::Descending(value: u64) -> bool;",
            "its result is not the row's `u64` rank",
        ),
        (
            "machine Nat::Descending(lower: u64, upper: u64) -> u64;",
            "the row ranks exactly one subject",
        ),
        (
            "machine Nat::Descending<T>(value: u64) -> u64;",
            "catalog rows declare no type or lifetime parameters",
        ),
    ] {
        let diagnostics =
            resolve_sealed_core_file(language_semantics::RANKING_VIEW_CORE_SOURCE, declaration)
                .expect_err(declaration);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("`Nat::Descending` names the compiler ranking-view catalog, but")
                && diagnostic.message.contains(expected)
                && diagnostic
                    .message
                    .contains("must be exactly `machine Nat::Descending(u64) -> u64;`")),
            "{declaration}: {diagnostics:?}"
        );
    }
}

mod package_ownership {
    //! A closed direct family is published only by its semantic-home owner's
    //! package: a `machine <token>` binding whose attached home, home domain,
    //! or any declared operand type belongs to another package rejects at its
    //! own declaration.

    use super::{Lexer, SymbolResolvedTrees};
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{DependencyScope, SourceMap, SourceOrigin, SourceResolutionStratum};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    /// `(root, filename, text, package marker)`; one marker is one package.
    fn resolve_packaged_sources(
        members: &[(&str, &str, &str, u8)],
    ) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
        let mut sources = SourceMap::default();
        for (root, filename, text, marker) in members {
            sources.add_checked_instance(
                PathBuf::from(root).join(filename),
                (*text).to_owned(),
                PathBuf::from(root),
                PackageKeyIdentity::from_digest([*marker; 32]),
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                DependencyScope::Product,
            );
        }
        let mut syntax = syntax_trees::SyntaxTrees::default();
        for file in sources.files() {
            let tokens = Lexer::new(&file.source).tokenize().expect("tokens");
            syntax
                .extend_from(&parse_syntax_trees_with_id(file.source_id, &tokens).expect("syntax"));
        }
        crate::resolve(crate::ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        })
    }

    #[test]
    fn same_package_members_across_files_share_the_family() {
        resolve_packaged_sources(&[
            (
                "pkg",
                "wrap.omg",
                "module wrap; pub data Wrap { value: u64; }",
                1,
            ),
            (
                "pkg",
                "ops.omg",
                "module ops; use wrap;
                 machine + wrap::Wrap::add(left: wrap::Wrap, right: wrap::Wrap) -> wrap::Wrap { left }
                 machine - combine(left: wrap::Wrap, right: u64) -> u64 { right }",
                1,
            ),
        ])
        .expect("one package may publish both attached and free bindings over its own types");
    }

    #[test]
    fn free_binding_over_foreign_domain_rejects() {
        let diagnostics = resolve_packaged_sources(&[
            ("owner", "wrap.omg", "module wrap; pub domain u8::Level;", 1),
            (
                "intruder",
                "intruder.omg",
                "module intruder; use wrap;
                 machine + add(left: u8 in Level, right: u8) -> u64 { 0u64 }",
                2,
            ),
        ])
        .expect_err("a foreign domain's constraint names its owning family");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "into a closed family through the operand declaration `wrap::u8::Level`, which \
                 another package owns"
                )),
            "{diagnostics:?}"
        );
    }
}
