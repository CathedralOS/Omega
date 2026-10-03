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

fn lower_multi(sources: &[(&str, &str)]) -> Result<SymbolResolvedTrees, String> {
    let mut map = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (name, text) in sources {
        let id = map.add(PathBuf::from(name), (*text).to_owned()).source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        tokens_to_syntax_trees::parse(&mut syntax, id, &tokens).expect("parse");
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

fn const_named(program: &SymbolResolvedTrees, name: &str) -> String {
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
fn scoped_constant_retains_template_owner_and_closed_value_carrier() {
    let owner_source = "module values; pub data Holder<T [copy]> [copy] { value: T; } pub const Holder::VALUE: Holder<u32> = Holder { value: 7u32 };";
    let scope_start = owner_source.find("const Holder::VALUE").expect("scope") + "const ".len();
    let program = lower_multi(&[
        ("values.omg", owner_source),
        (
            "reader.omg",
            "use values::Holder; machine read() -> u32 { values::Holder::VALUE.value }",
        ),
    ])
    .expect("generic owner and closed constant value resolve separately");
    let owner = program
        .data_definitions
        .iter()
        .find(|definition| !definition.type_parameters.is_empty())
        .expect("retained template");
    assert!(program.authored_declaration_selections().iter().any(|selection| {
        selection.source_span().source_id == source::SourceId(0)
            && selection.source_span().span.start == scope_start
            && matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(selected) if selected.selected_symbol() == owner.symbol)
    }), "the original scope occurrence must select the template");
    let declaration = program.const_declarations.iter().next().expect("constant");
    let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
        declaration.declared_type
    else {
        panic!("the value carrier must be closed before resolution");
    };
    assert_ne!(symbol, owner.symbol);
    assert!(
        program.data_definitions.iter().any(|definition| {
            definition.symbol == symbol && definition.generic_instance.is_some()
        }),
        "the value retains the specialized carrier independently"
    );
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

#[test]
fn constrained_const_indexed_domain_reaches_foreign_family_and_named_index() {
    // `units` owns the only reachable `Window` family. A generic family
    // declares a leaf-named domain, so module name law reaches it through the
    // narrow import's bare leaf or the complete qualified spelling — never a
    // carrier-qualified `u64::Window` from outside its own module. An exactly
    // selected module-constant index argument discharges against `units`'
    // owner as well.
    for (tag, declaring) in [
        (
            "narrow import",
            "module mine; use units::Window; const X: u64 in Window<8> = 3;",
        ),
        (
            "qualified",
            "module mine; const X: u64 in units::Window<8> = 3;",
        ),
        (
            "selected module const index",
            "module mine; use units::Window; const W: u64 = 8; const X: u64 in Window<W> = 3;",
        ),
    ] {
        let program = lower_multi(&[
            (
                "units.omg",
                "module units; pub domain<const N: u64> u64::Window<N> requires self < N;",
            ),
            (
                "decoys.omg",
                "module decoys; pub domain<const N: u64> u64::Window<N> requires self > N;",
            ),
            ("mine.omg", declaring),
        ])
        .unwrap_or_else(|e| panic!("{tag} foreign indexed domain selection: {e}"));
        assert_eq!(const_named(&program, "X"), "mine::X", "{tag}");
    }
}

#[test]
fn constrained_bool_const_discharges_logical_and_indexed_family_facts() {
    // `!self` and `self == <bool expression>` replay through the same fact
    // evaluator; the indexed family's integer binder still closes its
    // arguments exactly while `self` carries the canonical Boolean.
    let program = lower_multi(&[(
        "mine.omg",
        "module mine; domain bool::Off requires !self; const F: bool in bool::Off = false;
         domain<const N: u64> bool::Saw<N> requires self == (N > 0); const G: bool in bool::Saw<8> = true;",
    )])
    .expect("bool-carrier facts discharge");
    assert_eq!(const_named(&program, "F"), "mine::F");
    assert_eq!(const_named(&program, "G"), "mine::G");
    for name in ["F", "G"] {
        let declaration = program
            .const_declarations
            .iter()
            .find(|declaration| {
                program.symbols.display_path(declaration.symbol, "::") == format!("mine::{name}")
            })
            .unwrap_or_else(|| panic!("mine::{name}"));
        assert!(
            declaration.evaluated_value.is_some(),
            "{name} publishes compatibility identity"
        );
    }
    for (tag, source) in [
        (
            "refuted logical-not fact",
            "module mine; domain bool::Off requires !self; const F: bool in bool::Off = true;",
        ),
        (
            "refuted indexed bool fact",
            "module mine; domain<const N: u64> bool::Saw<N> requires self == (N > 0); const G: bool in bool::Saw<8> = false;",
        ),
        (
            // The selected family's integer carrier cannot hold a Boolean
            // const: the carrier check rejects before any index binding.
            "bool const against an integer-carrier family",
            "module mine; domain<const N: u64> u64::Saw<N> requires self < N; const G: bool in u64::Saw<8> = true;",
        ),
        (
            "bool const against an integer-carrier domain",
            "module mine; domain u64::Pos requires self > 0; const F: bool in u64::Pos = true;",
        ),
    ] {
        let error = lower_multi(&[("mine.omg", source)]).expect_err("{tag} must reject");
        assert!(
            error.contains("is false") || error.contains("carrier"),
            "{tag}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn constrained_bool_const_discharges_nested_membership() {
    // `self in bool::Inner` inside `bool::Outer` carries the Boolean operand
    // into the nested domain with its `bool` carrier.
    let program = lower_multi(&[(
        "mine.omg",
        "module mine; domain bool::Inner requires self; domain bool::Outer requires self in bool::Inner; const F: bool in bool::Outer = true;",
    )])
    .expect("nested bool membership discharges");
    assert_eq!(const_named(&program, "F"), "mine::F");
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; domain bool::Inner requires self; domain bool::Outer requires self in bool::Inner; const F: bool in bool::Outer = false;",
    )])
    .expect_err("a nested bool membership refutes");
    assert!(error.contains("is false"), "unexpected diagnostic: {error}");
}

#[test]
fn constrained_boolean_index_publishes_the_selected_constant() {
    let program = lower_multi(&[(
        "mine.omg",
        "module mine; domain<const F: bool> u64::Tag<F> requires self > 0; const X: u64 in u64::Tag<true> = 3;",
    )])
    .expect("a closed Boolean index discharges its declaration facts");
    assert_eq!(const_named(&program, "X"), "mine::X");
}

#[test]
fn constrained_record_const_discharges_field_domain_facts() {
    // A record constant binds `self` through its scalar-decodable fields, so
    // `self.x > 0` evaluates against the canonical `x` leaf and the
    // declaration publishes compatibility identity.
    let program = lower_multi(&[(
        "mine.omg",
        "module mine; data Pair [copy] { x: u64; } domain Pair::NonEmpty requires self.x > 0; const P: Pair in Pair::NonEmpty = Pair { x: 1 };",
    )])
    .expect("record field fact discharges the constrained const");
    assert_eq!(const_named(&program, "P"), "mine::P");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::P")
        .expect("mine::P");
    assert!(
        declaration.evaluated_value.is_some(),
        "a discharged record constrained const publishes compatibility identity"
    );

    // A refuting field value rejects with the precise refutation diagnostic,
    // not the generic fence.
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; data Pair [copy] { x: u64; } domain Pair::NonEmpty requires self.x > 0; const P: Pair in Pair::NonEmpty = Pair { x: 0 };",
    )])
    .expect_err("a refuted record field fact rejects");
    assert!(error.contains("is false"), "unexpected diagnostic: {error}");

    for (tag, sources) in [
        // A field that never decodes to a scalar leaf cannot bind `self`'s
        // projection, so the fact stays fenced for checked downstream
        // evidence.
        (
            "unknown field projection",
            "module mine; data Pair [copy] { x: u64; } domain Pair::Counted requires self.count > 0; const P: Pair in Pair::Counted = Pair { x: 1 };",
        ),
        // A nested record field has no scalar leaf; `self.inner.v` does not
        // project through the record binding.
        (
            "nested record field",
            "module mine; data Inner [copy] { v: u64; } data Outer [copy] { inner: Inner; } domain Outer::Deep requires self.inner.v > 0; const O: Outer in Outer::Deep = Outer { inner: Inner { v: 1 } };",
        ),
        // A whole-aggregate `self` operand has no scalar identity to compare.
        (
            "whole-aggregate self operand",
            "module mine; data Pair [copy] { x: u64; } domain Pair::Any requires self == self; const P: Pair in Pair::Any = Pair { x: 1 };",
        ),
    ] {
        let error = lower_multi(&[("mine.omg", sources)]).expect_err("{tag} must stay fenced");
        assert!(
            error
                .contains("constrained const declarations require declaration-site proof checking"),
            "{tag}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn constrained_array_const_discharges_element_domain_facts() {
    // An array constant binds `self` through its scalar-decodable elements, so
    // `self[0] in u8::NonZero` evaluates against the canonical element leaf
    // and the declaration publishes compatibility identity.
    let program = lower_multi(&[(
        "mine.omg",
        "module mine; domain u8::NonZero requires self > 0; domain [u8; 8]::Full requires self[0] in u8::NonZero; const A: [u8; 8] in Full = [1, 2, 3, 4, 5, 6, 7, 8];",
    )])
    .expect("array element membership discharges the constrained const");
    assert_eq!(const_named(&program, "A"), "mine::A");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::A")
        .expect("mine::A");
    assert!(
        declaration.evaluated_value.is_some(),
        "a discharged array constrained const publishes compatibility identity"
    );

    // A refuting element value rejects with the precise refutation
    // diagnostic, not the generic fence.
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; domain u8::NonZero requires self > 0; domain [u8; 8]::Full requires self[0] in u8::NonZero; const A: [u8; 8] in Full = [0, 2, 3, 4, 5, 6, 7, 8];",
    )])
    .expect_err("a refuted array element fact rejects");
    assert!(error.contains("is false"), "unexpected diagnostic: {error}");

    for (tag, sources) in [
        // An index beyond the canonical element count binds no element, so
        // the fact stays fenced for checked downstream evidence.
        (
            "out-of-bounds element index",
            "module mine; domain u8::NonZero requires self > 0; domain [u8; 8]::Saturated requires self[8] in u8::NonZero; const A: [u8; 8] in Saturated = [1, 2, 3, 4, 5, 6, 7, 8];",
        ),
        // An element that is itself an aggregate has no scalar leaf;
        // `self[0]` does not project through the element binding.
        (
            "nested array element",
            "module mine; domain u8::NonZero requires self > 0; domain [[u8; 2]; 2]::Deep requires self[0] in u8::NonZero; const A: [[u8; 2]; 2] in Deep = [[1, 2], [3, 4]];",
        ),
        // A whole-aggregate `self` operand has no scalar identity to compare.
        (
            "whole-aggregate self operand",
            "module mine; domain [u8; 8]::Any requires self == self; const A: [u8; 8] in Any = [1, 2, 3, 4, 5, 6, 7, 8];",
        ),
    ] {
        let error = lower_multi(&[("mine.omg", sources)]).expect_err("{tag} must stay fenced");
        assert!(
            error
                .contains("constrained const declarations require declaration-site proof checking"),
            "{tag}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn constrained_variant_const_discharges_case_payload_domain_facts() {
    // A variant constant binds `self` through its selected case's
    // scalar-decodable payload fields, so `self.volume > 0` evaluates
    // against the canonical `Say` payload leaf and the declaration
    // publishes compatibility identity.
    let program = lower_multi(&[(
        "mine.omg",
        "module mine; data Command { case None; case Say(volume: u64); } domain Command::Audible requires self.volume > 0; const C: Command in Command::Audible = Command::Say { volume: 1 };",
    )])
    .expect("case payload fact discharges the constrained const");
    assert_eq!(const_named(&program, "C"), "mine::C");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::C")
        .expect("mine::C");
    assert!(
        declaration.evaluated_value.is_some(),
        "a discharged variant constrained const publishes compatibility identity"
    );

    // A refuting payload value rejects with the precise refutation
    // diagnostic, not the generic fence.
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; data Command { case None; case Say(volume: u64); } domain Command::Audible requires self.volume > 0; const C: Command in Command::Audible = Command::Say { volume: 0 };",
    )])
    .expect_err("a refuted case payload fact rejects");
    assert!(error.contains("is false"), "unexpected diagnostic: {error}");

    for (tag, sources) in [
        // A case without the queried payload field binds no projection, so
        // the fact stays fenced for checked downstream evidence.
        (
            "different case's payload field",
            "module mine; data Command { case None; case Say(volume: u64); } domain Command::Audible requires self.volume > 0; const C: Command in Command::Audible = Command::None;",
        ),
        // A payload field without a scalar leaf cannot bind `self`'s
        // projection; `self.v[0]` does not project through the field
        // binding.
        (
            "non-scalar payload field",
            "module mine; data Signal { case Pair(v: [u8; 2]); } domain Signal::Deep requires self.v[0] > 0; const S: Signal in Signal::Deep = Signal::Pair { v: [1, 2] };",
        ),
        // A whole-aggregate `self` operand has no scalar identity to compare.
        (
            "whole-aggregate self operand",
            "module mine; data Command { case None; case Say(volume: u64); } domain Command::Any requires self == self; const C: Command in Command::Any = Command::Say { volume: 1 };",
        ),
    ] {
        let error = lower_multi(&[("mine.omg", sources)]).expect_err("{tag} must stay fenced");
        assert!(
            error
                .contains("constrained const declarations require declaration-site proof checking"),
            "{tag}: unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn constrained_const_keeps_fence_for_unselected_or_indexed_domains() {
    for (tag, sources) in [
        // `Pos` exists only inside the unimported sibling `units`; the
        // carrier-qualified leaf cannot reach it from `mine`.
        (
            "unreachable foreign leaf",
            &[
                (
                    "units.omg",
                    "module units; pub domain u64::Pos requires self > 0;",
                ),
                ("mine.omg", "module mine; const X: u64 in u64::Pos = 3;"),
            ][..],
        ),
        // Two imported foreign owners contest the leaf with no module-local
        // candidate: the pool declines rather than guess.
        (
            "contested foreign owners",
            &[
                ("a.omg", "module a; pub domain u64::Pos requires self > 0;"),
                ("b.omg", "module b; pub domain u64::Pos requires self > 0;"),
                (
                    "mine.omg",
                    "module mine; use a; use b; const X: u64 in u64::Pos = 3;",
                ),
            ][..],
        ),
        // Two narrow-imported foreign families contest the bare leaf: the
        // pool declines rather than bind the index against a guessed owner.
        (
            "contested indexed foreign owners",
            &[
                (
                    "a.omg",
                    "module a; pub domain<const N: u64> u64::Window<N> requires self < N;",
                ),
                (
                    "b.omg",
                    "module b; pub domain<const N: u64> u64::Window<N> requires self < N;",
                ),
                (
                    "mine.omg",
                    "module mine; use a::Window; use b::Window; const X: u64 in Window<8> = 3;",
                ),
            ][..],
        ),
        // The unimported sibling's family is unreachable from `mine`.
        (
            "unreachable indexed foreign family",
            &[
                (
                    "units.omg",
                    "module units; pub domain<const N: u64> u64::Window<N> requires self < N;",
                ),
                (
                    "mine.omg",
                    "module mine; const X: u64 in u64::Window<8> = 3;",
                ),
            ][..],
        ),
        // An unresolved index argument cannot bind a concrete value at
        // declaration site; the application keeps its authored shape for
        // ordinary resolution.
        (
            "open index argument",
            &[(
                "mine.omg",
                "module mine; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window<K> = 3;",
            )][..],
        ),
    ] {
        let error = lower_multi(sources).expect_err("{tag} must stay fenced");
        assert!(
            error
                .contains("constrained const declarations require declaration-site proof checking"),
            "{tag}: unexpected diagnostic: {error}"
        );
    }
    // Exact family selection diagnoses its missing index even though no fact
    // can be evaluated: unused binders remain part of the complete application.
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window = 3;",
    )]).expect_err("incomplete family application must reject");
    assert!(
        error.contains("requires 1 closed index argument(s), but 0 were supplied"),
        "{error}"
    );
}
