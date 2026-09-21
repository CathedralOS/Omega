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
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
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

#[test]
fn signature_free_route_rejects_unimported_module_trait() {
    let error = lower_multi(&[
        (
            "issuer.omg",
            "module issuer; pub trait Issuer { machine issue(value: u64) -> Token in Token::Issued; }",
        ),
        (
            "main.omg",
            "pub data Token { value: u64; } pub domain Token::Issued established by Issuer::issue;",
        ),
    ])
    .map(|_| ()).expect_err("loading a module must not expose its trait to a bare route");
    assert!(error.contains("does not resolve to one exact"), "{error}");
}

#[test]
fn signature_free_routes_keep_imports_file_local_and_nontransitive() {
    for (relay, imports) in [
        ("use issuer::Issuer;", ""),
        ("module relay; use issuer::Issuer;", "use relay;"),
    ] {
        let source = format!(
            "{imports} pub data Token {{ value: u64; }} pub domain Token::Issued established by Issuer::issue;"
        );
        let error = lower_multi(&[
            ("issuer.omg", "module issuer; pub trait Issuer { machine issue(value: u64) -> Token in Token::Issued; }"),
            ("relay.omg", relay),
            ("main.omg", &source),
        ]).map(|_| ()).expect_err("another file's import cannot authorize the route");
        assert!(error.contains("does not resolve to one exact"), "{error}");
    }
}

#[test]
fn signature_free_route_rejects_unimported_module_machine() {
    let error = lower_multi(&[
        ("issuer.omg", "module issuer; pub data Issuer {} pub machine Issuer::issue(value: u64) -> Token in Token::Issued { Token { value: value } }"),
        ("main.omg", "pub data Token { value: u64; } pub domain Token::Issued established by Issuer::issue;"),
    ]).map(|_| ()).expect_err("loading a module must not expose its machine to a bare route");
    assert!(error.contains("does not resolve to one exact"), "{error}");
}

#[test]
fn signature_free_route_selects_the_imported_attached_machine() {
    for (imports, route) in [
        ("use issuer::Issuer;", "Issuer::issue"),
        ("", "issuer::Issuer::issue"),
    ] {
        let source = format!(
            "{imports} pub data Token {{ value: u64; }} pub domain Token::Issued established by {route};"
        );
        let program = lower_multi(&[
            ("issuer.omg", "module issuer; pub data Issuer {} pub machine Issuer::issue(value: u64) -> Token in Token::Issued { Token { value: value } }"),
            ("decoy.omg", "module decoy; pub data Issuer {} pub machine Issuer::issue(value: u64) -> Token in Token::Issued { Token { value: value } }"),
            ("main.omg", &source),
        ]).expect("the selected carrier owns the exact-machine route");
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == "Token::Issued")
            .expect("issued domain");
        let [language_semantics::DomainEstablishmentRoute::ExactMachine { machine }] =
            domain.establishment_routes.as_slice()
        else {
            panic!("exact-machine route: {:?}", domain.establishment_routes);
        };
        assert_eq!(
            program.symbols.display_path(*machine, "::"),
            "issuer::Issuer::issue"
        );
    }
}

#[test]
fn signature_free_route_rejects_applicable_flat_trait_competitors() {
    let error = lower_multi(&[
        (
            "first.omg",
            "pub trait Issuer { machine issue(value: u64) -> Token in Token::Issued; }",
        ),
        (
            "second.omg",
            "pub trait Issuer { machine issue(value: u64) -> Token in Token::Issued; }",
        ),
        (
            "main.omg",
            "pub data Token { value: u64; } pub domain Token::Issued established by Issuer::issue;",
        ),
    ])
    .map(|_| ())
    .expect_err("no first-candidate fallback for signature-free selection");
    assert!(error.contains("does not resolve to one exact"), "{error}");
}

#[test]
fn signature_free_route_keeps_the_imported_trait_with_an_unimported_competitor() {
    for (imports, route) in [
        ("use issuer::Issuer;", "Issuer::issue"),
        ("", "issuer::Issuer::issue"),
        ("use issuer;", "issuer::Issuer::issue"),
    ] {
        let source = format!(
            "{imports} pub data Token {{ value: u64; }} pub domain Token::Issued established by {route};
             machine register<machine Selected>() where machine Selected satisfies {route}; {{}}"
        );
        let program = lower_multi(&[
        (
            "issuer.omg",
            "module issuer; pub trait Issuer { machine issue(value: u64) -> Token in Token::Issued; }",
        ),
        (
            "decoy.omg",
            "module decoy; pub trait Issuer { machine issue(value: u64) -> Token in Token::Issued; }",
        ),
        (
            "main.omg",
            &source,
        ),
    ])
    .expect("the exact imported trait owns the route");
        let domain = program
            .domain_definitions
            .iter()
            .find(|domain| domain.name.as_str() == "Token::Issued")
            .expect("issued domain");
        let [
            language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                trait_definition,
                requirement,
            },
        ] = domain.establishment_routes.as_slice()
        else {
            panic!("trait route: {:?}", domain.establishment_routes);
        };
        assert_eq!(
            program.symbols.display_path(*trait_definition, "::"),
            "issuer::Issuer"
        );
        assert_eq!(
            program.symbols.display_path(*requirement, "::"),
            "issuer::Issuer::issue"
        );
        let parameter = program
            .tables
            .declarations
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .find(|parameter| parameter.name.as_str() == "Selected")
            .expect("nominal machine binder");
        assert!(matches!(
            parameter.kind,
            symbol_resolved_trees::data::TypeParameterKind::Machine {
                contract: symbol_resolved_trees::data::MachineParameterContract::Nominal {
                    trait_definition: selected_trait, requirement: selected_requirement, ..
                },
            } if selected_trait == *trait_definition && selected_requirement == *requirement
        ));
    }
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

/// The selected declarations on a `x in Type::Case` fact after the fact has
/// normalized to a case-membership expression: (case owner, case).
fn selected_case_membership(
    program: &SymbolResolvedTrees,
    facts: arena::HandleSpan<ProofFact>,
) -> (String, String) {
    let [ProofFact::Expression(expression)] = program.proof_facts(facts) else {
        panic!("a case-membership fact normalizes to one expression")
    };
    let ExpressionNode::Membership(membership) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("the normalized fact is a case-membership expression")
    };
    assert!(
        !membership.domain_symbol.is_valid(),
        "a case selection never fills the declared-domain slot"
    );
    (
        program
            .symbols
            .display_path(membership.case_type_symbol, "::"),
        program.symbols.display_path(membership.case_symbol, "::"),
    )
}

fn data_where_facts(program: &SymbolResolvedTrees, name: &str) -> arena::HandleSpan<ProofFact> {
    let definition = program
        .data_definitions
        .iter()
        .find(|definition| {
            program
                .symbols
                .display_path(definition.symbol, "::")
                .ends_with(name)
        })
        .unwrap_or_else(|| panic!("data `{name}`"));
    definition.storage.where_facts
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
    // The `K in Window<8>` fact selects the FOREIGN family exactly: its
    // recorded domain symbol is `units::Window` with a live authored-selection
    // receipt, and the index argument is the closed `8`.
    let [ProofFact::Membership(membership)] =
        program.proof_facts(data_where_facts(&program, "Holder"))
    else {
        panic!("`K in Window<8>` stays a declared-domain membership fact")
    };
    assert_eq!(
        program.symbols.display_path(membership.domain_symbol, "::"),
        "units::Window"
    );
    assert!(
        membership.authored_domain_selection.is_some(),
        "the selected domain records an authored-selection occurrence"
    );
    assert_eq!(membership.domain_arguments.count(), 1);
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
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| {
            program.symbols.display_path(definition.symbol, "::") == "policy::Choice::NonEmpty"
        })
        .expect("policy::Choice::NonEmpty");
    // Resolution success alone says nothing about WHICH case the fact
    // selected: assert the foreign owner and exact case, not just a hit.
    assert_eq!(
        selected_case_membership(&program, domain.facts),
        (
            "shapes::Choice".to_string(),
            "shapes::Choice::Some".to_string()
        )
    );
    // The fact's selected declaration is the exact foreign case symbol, not
    // merely "resolution succeeded": `self in shapes::Choice::Some` is a
    // case-membership expression whose `case_symbol`/`case_type_symbol` bind
    // `shapes::Choice::Some` on `shapes::Choice`.
    let fact_expression = program
        .proof_facts(domain.facts)
        .iter()
        .find_map(|fact| match fact {
            ProofFact::Expression(handle) => Some(*handle),
            ProofFact::Membership(_) => None,
        })
        .expect("the domain's fact is an expression");
    let symbol_resolved_trees::expression::ExpressionNode::Membership(membership) = program
        .tables
        .bodies
        .expressions
        .expression(fact_expression)
    else {
        panic!("the domain fact is a case membership expression")
    };
    assert_eq!(
        program
            .symbols
            .display_path(membership.case_type_symbol, "::"),
        "shapes::Choice"
    );
    assert_eq!(
        program.symbols.display_path(membership.case_symbol, "::"),
        "shapes::Choice::Some"
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
    let program = lower_multi(&[
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
    // The `where` fact retains its selected declaration rather than only
    // resolving: the case target keeps `domain_symbol` invalid and records a
    // checked-domain-membership selection binding for the checking stage.
    let holder = program
        .data_definitions
        .iter()
        .find(|definition| program.symbols.display_path(definition.symbol, "::") == "Holder")
        .expect("Holder");
    let membership = program
        .proof_facts(holder.storage.where_facts)
        .iter()
        .find_map(|fact| match fact {
            ProofFact::Membership(membership) => Some(membership),
            ProofFact::Expression(_) => None,
        })
        .expect("the where clause is a membership fact");
    let authored = program
        .domain_path_members(membership.domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    assert_eq!(authored, "Choice::Some");
    assert!(
        !membership.domain_symbol.is_valid(),
        "a case membership target carries no declared-domain symbol"
    );
    let occurrence = membership
        .authored_domain_selection
        .expect("the where fact retains its authored selection");
    let selection = program
        .authored_declaration_selections()
        .get(occurrence)
        .expect("the occurrence's selection row");
    match selection.target() {
        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(
            resolved,
        ) => assert_eq!(
            program
                .symbols
                .display_path(resolved.selected_symbol(), "::"),
            "shapes::Choice::Some"
        ),
        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::LateBound(
            binding,
        ) => assert_eq!(
            binding,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership
        ),
        language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Intrinsic(
            _,
        ) => panic!("the case selection is not an intrinsic"),
    }
}

#[test]
fn unimported_qualified_domain_fact_selects_package_internal() {
    // The one selection law for domain facts: a complete logical path selects
    // exactly — exposure governs imports and leaf spellings, not the spelled
    // `a::u64::Private` itself. The fact records that exact selected
    // declaration even though `check.omg` never imports `a`.
    let program = lower_multi(&[
        ("a.omg", "module a; domain u64::Private requires self > 0;"),
        (
            "check.omg",
            "data H where v in a::u64::Private, { v: u64; }",
        ),
    ])
    .expect("complete-path domain fact resolves");
    let [ProofFact::Membership(membership)] = program.proof_facts(data_where_facts(&program, "H"))
    else {
        panic!("`v in a::u64::Private` stays a membership fact")
    };
    assert_eq!(
        program.symbols.display_path(membership.domain_symbol, "::"),
        "a::u64::Private"
    );
    assert!(membership.authored_domain_selection.is_some());
}

#[test]
fn transitive_case_import_does_not_expose_membership() {
    // `use` is file-local: relay's import of `shapes::Choice::Some` does not
    // re-expose the case, so check's fact has nothing to select. Resolution
    // must fail closed — no domain symbol, no case pair — leaving a
    // late-bound authored-selection occurrence for the checked stage to
    // report, rather than guessing through another file's import.
    let program = lower_multi(&[
        (
            "shapes.omg",
            "module shapes; pub data Choice { case Empty; case Some(v: u32); }",
        ),
        ("relay.omg", "module relay; use shapes::Choice::Some;"),
        (
            "check.omg",
            "use relay; data H where c in Choice::Some, { c: u32; }",
        ),
    ])
    .expect("the unresolved fact lowers and defers selection");
    let [ProofFact::Membership(membership)] = program.proof_facts(data_where_facts(&program, "H"))
    else {
        panic!("`c in Choice::Some` stays a membership fact")
    };
    assert!(
        !membership.domain_symbol.is_valid(),
        "transitive exposure never fabricates a selected domain"
    );
    assert!(membership.authored_domain_selection.is_some());
}

#[test]
fn qualified_indexed_domain_constraint() {
    // Qualified spelling of a module's indexed domain family as a field
    // constraint.
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain<const N: u64> u64::Window<N> requires self < N;",
        ),
        ("main.omg", "data S { v: u64 in units::u64::Window<8>; }"),
    ])
    .expect("qualified indexed domain constraint resolves");
    // The selected declaration is the foreign family's own symbol and the
    // constraint retains the closed index argument it was selected under.
    assert_eq!(domain_symbol_path(&program, "Window"), "units::Window");
    let data = program
        .data_definitions
        .iter()
        .find(|definition| program.symbols.display_path(definition.symbol, "::") == "S")
        .expect("data S");
    let member = program
        .data_members(data.storage.members)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .expect("S carries a field member");
    let TypeReference::Constrained(constrained) = &member.type_reference else {
        panic!("the field's type reference is constrained")
    };
    let domain = program
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints)
        .iter()
        .find_map(|constraint| match constraint {
            TypeConstraint::Domain(domain) => Some(domain),
            _ => None,
        })
        .expect("the field constraint is a domain constraint");
    assert_eq!(domain.name.as_str(), "units::u64::Window");
    assert_eq!(domain.arguments.len(), 1, "the closed index `8` is bound");
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
fn constrained_const_discharges_against_module_local_domain() {
    // Both `units` and `mine` declare a `u64::Pos`; `use units` makes the
    // foreign spelling reachable, so the leaf is contested and module-local
    // precedence must own it. The foreign fact (`self > 9`) would refute
    // `3`, while the module-local fact (`self > 0`) discharges — resolving
    // proves the constraint evaluated against the exact local owner.
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Pos requires self > 9;",
        ),
        (
            "mine.omg",
            "module mine; use units; domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 3;",
        ),
    ])
    .expect("module-local domain discharges the constrained const");
    assert_eq!(const_named(&program, "X"), "mine::X");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::X")
        .expect("mine::X");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "a discharged constrained const publishes compatibility identity"
    );
}

#[test]
fn constrained_const_reaches_foreign_domain_through_import_and_qualified_spelling() {
    // `units` owns the only `Pos` requiring `self > 0`; `decoys` owns a
    // same-leaf sibling whose fact would refute `3` — but nothing imports it,
    // so it never enters the pool. Both the carrier-qualified leaf spelling
    // (through `use units`) and the complete qualified spelling must select
    // `units`' exact owner and discharge.
    for (tag, declaring) in [
        (
            "imported",
            "module mine; use units; const X: u64 in u64::Pos = 3;",
        ),
        (
            "qualified",
            "module mine; const X: u64 in units::u64::Pos = 3;",
        ),
    ] {
        let program = lower_multi(&[
            (
                "units.omg",
                "module units; pub domain u64::Pos requires self > 0;",
            ),
            (
                "decoys.omg",
                "module decoys; pub domain u64::Pos requires self > 9;",
            ),
            ("mine.omg", declaring),
        ])
        .unwrap_or_else(|e| panic!("{tag} foreign domain selection: {e}"));
        assert_eq!(const_named(&program, "X"), "mine::X", "{tag}");
    }
}

#[test]
fn constrained_const_rejects_refuted_domain_fact() {
    // The selected module-local owner evaluates `self > 0` at `0` and refutes
    // the declaration — a precise rejection, not the generic fence.
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 0;",
    )])
    .expect_err("a refuted constrained const rejects");
    assert!(
        error.contains("domain constraint `u64::Pos` for const `X` is false")
            || error.contains("is false"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn constrained_const_discharges_closed_indexed_domain() {
    // `mine`'s local `Window` family outranks the imported same-leaf sibling
    // under module-local precedence; its `self < N` fact replays with `N`
    // bound to the closed index `8` and `self` to the canonical value, so the
    // declaration publishes identity. The foreign sibling's `self <= N` would
    // also hold for `3` — make it refuting (`self > N`) so a wrong owner
    // would reject instead of discharge.
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain<const N: u64> u64::Window<N> requires self > N;",
        ),
        (
            "mine.omg",
            "module mine; use units; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window<8> = 3;",
        ),
    ])
    .expect("closed indexed domain discharges the constrained const");
    assert_eq!(const_named(&program, "X"), "mine::X");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::X")
        .expect("mine::X");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "a discharged indexed constrained const publishes compatibility identity"
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
fn constrained_const_rejects_refuted_indexed_domain_fact() {
    // The selected family replays `self < N` with `N` bound to `8` and `self`
    // to `9` — a precise refutation, not the generic fence.
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window<8> = 9;",
    )])
    .expect_err("a refuted indexed constrained const rejects");
    assert!(error.contains("is false"), "unexpected diagnostic: {error}");
}

#[test]
fn constrained_bool_const_discharges_against_module_local_domain() {
    // A constrained const over the `bool` carrier binds `self` to the
    // canonical Boolean and replays the selected domain's facts exactly as an
    // integer-carrier const does. The foreign sibling's `!self`-style refuting
    // fact (`self == false`) would reject `true`, so discharging proves the
    // contested leaf resolved to the module-local owner.
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain bool::Flag requires self == false;",
        ),
        (
            "mine.omg",
            "module mine; use units; domain bool::Flag requires self; const F: bool in bool::Flag = true;",
        ),
    ])
    .expect("module-local bool domain discharges the constrained const");
    assert_eq!(const_named(&program, "F"), "mine::F");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::F")
        .expect("mine::F");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "a discharged bool constrained const publishes compatibility identity"
    );
}

#[test]
fn constrained_bool_const_rejects_refuted_domain_fact() {
    let error = lower_multi(&[(
        "mine.omg",
        "module mine; domain bool::Flag requires self; const F: bool in bool::Flag = false;",
    )])
    .expect_err("a refuted bool constrained const rejects");
    assert!(error.contains("is false"), "unexpected diagnostic: {error}");
}

#[test]
fn constrained_bool_const_reaches_foreign_domain_through_import_and_qualified_spelling() {
    // `units` owns the only reachable `bool::Flag`; the decoy sibling's
    // refuting fact is unreachable and never enters the pool.
    for (tag, declaring) in [
        (
            "imported",
            "module mine; use units; const F: bool in bool::Flag = true;",
        ),
        (
            "qualified",
            "module mine; const F: bool in units::bool::Flag = true;",
        ),
    ] {
        let program = lower_multi(&[
            (
                "units.omg",
                "module units; pub domain bool::Flag requires self;",
            ),
            (
                "decoys.omg",
                "module decoys; pub domain bool::Flag requires self == false;",
            ),
            ("mine.omg", declaring),
        ])
        .unwrap_or_else(|e| panic!("{tag} foreign bool domain selection: {e}"));
        assert_eq!(const_named(&program, "F"), "mine::F", "{tag}");
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
            declaration.canonical_value_encoding.is_some(),
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
        declaration.canonical_value_encoding.is_some(),
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
        declaration.canonical_value_encoding.is_some(),
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
        declaration.canonical_value_encoding.is_some(),
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
