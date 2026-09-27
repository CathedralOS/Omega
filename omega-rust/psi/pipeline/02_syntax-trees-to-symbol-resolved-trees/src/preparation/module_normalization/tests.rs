//! Module normalization tests.

use super::{Item, SyntaxTrees};
use source::SourceId;
use source_files_to_tokens::Lexer;

fn parse(sources: &[&str]) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::default();
    for (source_ordinal, text) in sources.iter().enumerate() {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize normalization control");
        tokens_to_syntax_trees::parse(&mut syntax, SourceId(source_ordinal), &tokens)
            .expect("parse normalization control");
    }
    syntax
}

#[test]
fn module_array_declarations_are_validated_even_without_uses() {
    for visibility in ["", "pub "] {
        for (carrier, value) in [
            ("[u8; 2]", "[1]"),
            ("[u8; 2]", "[1, 2, 3]"),
            ("[u8; 2]", "[1, 256]"),
            ("[u8; 2]", "[1, true]"),
            ("[u8; 2]", "[1u64, 2]"),
            ("[[u8; 2]; 1]", "[[1]]"),
        ] {
            let source = format!("module settings; {visibility}const SIZE: {carrier} = {value};");
            let diagnostics = crate::preparation::generic_data::normalize_generic_data(
                crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source])),
            )
            .expect_err("an unused array declaration still owes component conformance");
            assert!(
                diagnostics[0].message.contains("module array constant"),
                "{diagnostics:?}"
            );
        }
        for (carrier, value) in [
            ("[u8; 2]", "[1u8, 2u8]"),
            ("[[bool; 2]; 1]", "[[true, false]]"),
            ("[u64; 0]", "[]"),
        ] {
            let source = format!("module settings; {visibility}const SIZE: {carrier} = {value};");
            crate::preparation::generic_data::normalize_generic_data(
                crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source])),
            )
            .expect("valid canonical array declaration");
        }
    }
}

#[test]
fn module_float_arrays_are_declarations_but_text_arrays_remain_fenced() {
    for source in [
        "module settings; const SIZE: [f32; 0] = [];",
        "module settings; const VALUES: [f64; 2] = [1.5f64, -0.0f64];",
    ] {
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(parse(&[source])),
        )
        .expect("floating declarations have determined representation bits");
    }
    assert!(
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(parse(&[
                "module settings; const SIZE: [string; 0] = [];"
            ]))
        )
        .expect_err("array shape cannot bypass missing text value ownership")[0]
            .message
            .contains("runtime floating/text identity")
    );
}

#[test]
fn module_aggregate_constants_check_unused_initializers() {
    for source in [
        "module first; data Value { value: u64; } const VALUE: Value = Value { value: 1 };",
        "module first; data Value { value: u64; } const VALUE: [Value; 0] = [];",
    ] {
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(parse(&[source])),
        )
        .expect("selected nominal initializer");
    }
    for initializer in [
        "Value { value: 256 }",
        "Value {}",
        "Value { value: 1, extra: 2 }",
    ] {
        let source = format!(
            "module first; data Value {{ value: u8; }} const VALUE: Value = {initializer};"
        );
        crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source])),
        )
        .expect_err("unused nominal initializer must still check");
    }
}

#[test]
fn closed_record_arguments_select_current_domains_by_exact_carrier() {
    let syntax = parse(&[
        "data Token {} data Other {} domain Token::Issued; domain Other::Issued; data Cell<T> { value: T; } data Holder { first: Cell<Token in Issued>; second: Cell<Other in Issued>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("current declared domain arguments normalize");
    let instances = normalized
        .root_items()
        .filter_map(|item| {
            let Item::Data(data) = item else {
                return None;
            };
            data.generic_instance.map(|_| data.name.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        instances.len(),
        2,
        "both exact carrier-domain applications must synthesize: {instances:?}"
    );
    assert!(instances.contains(&"Cell<Token in Issued>"));
    assert!(instances.contains(&"Cell<Other in Issued>"));
}

#[test]
fn ambiguous_imported_traits_synthesize_no_default() {
    let mut syntax = parse(&[
        "module first; pub trait Service { machine run(&mut self) { } }",
        "module second; pub trait Service { machine stop(&mut self) { } }",
        "use first::Service; use second::Service; data Worker {} membership: Worker satisfies Service;",
    ]);
    crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax, None, Vec::new())
        .expect("an ambiguous authored trait defers to resolution diagnostics");
    assert!(
        !syntax
            .root_items()
            .any(|item| matches!(item, Item::Machine(_))),
        "no default is synthesized from an ambiguous trait name"
    );
}

#[test]
fn same_leaf_generic_family_contests_the_non_generic_sibling() {
    use std::sync::Arc;
    // A leaf spelling reaches both a non-generic sibling and a generic
    // family. Resolution pools both owners and declines the ambiguous
    // selection, so const-fact evaluation must retain the obligation
    // instead of discharging it against the non-generic sibling.
    let text = "domain u64::Tagged requires self > 5; domain<T> T::Tagged; data Bound<const N: u64> where N in Tagged, { value: u64; } data Use { value: Bound<7>; }";
    let mut map = source::SourceMap::default();
    let source_id = map
        .add(std::path::PathBuf::from("tagged.omg"), text.to_owned())
        .source_id;
    let mut syntax = SyntaxTrees::default();
    let tokens = Lexer::new(text).tokenize().expect("tokenize");
    tokens_to_syntax_trees::parse(&mut syntax, source_id, &tokens).expect("parse");
    let sources = Arc::new(map);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest {
            syntax,
            sources: Some(sources.clone()),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .expect("a contested membership is retained, not discharged");
    let instance = normalized
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.generic_instance.is_some() => Some(data),
            _ => None,
        })
        .expect("one synthesized instance");
    assert_eq!(
        normalized
            .tables
            .items
            .proof_facts(instance.where_facts)
            .len(),
        1,
        "the contested membership stays a checked obligation"
    );
    let program = crate::resolve(crate::ResolutionRequest {
        syntax: &normalized,
        sources: Some(sources),
        top_level_bindings: Vec::new(),
    })
    .expect("the retained obligation still resolves");
    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Bound<7>")
        .expect("the concrete instance");
    let [symbol_resolved_trees::domain::ProofFact::Membership(membership)] =
        program.proof_facts(instance.where_facts)
    else {
        panic!("the retained fact is a membership")
    };
    assert!(
        !membership.domain_symbol.is_valid(),
        "resolution pools the generic family and declines the ambiguous owner"
    );
}

#[test]
fn module_generic_carrier_arrays_share_the_deferral() {
    // `[Box<u64>; 2]` peels to the same generic-leaf gate: the declaration's
    // canonical identity is the closed instance encoding, and the base still
    // lands on the declaring module's template.
    let syntax = parse(&[
        "module mine; data Box<T> { value: T; } const BS: [Box<u64>; 2] = [Box { value: 1 }, Box { value: 2 }];",
    ]);
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("an array of generic-carrier values resolves");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::BS")
        .expect("the module constant resolved");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "an array of generic carriers publishes the closed instance identity too"
    );
    let symbol_resolved_trees::types::TypeReference::FixedArray(array) = &declaration.declared_type
    else {
        panic!("the declared carrier remains a fixed array");
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        program.child_type_reference(array.element_type)
    else {
        panic!("the array element remains a generic application");
    };
    assert_eq!(
        program.symbols.display_path(application.base_symbol, "::"),
        "mine::Box"
    );
}

#[test]
fn module_generic_carrier_constants_reject_unselected_or_ineligible_bases() {
    for (sources, fragment) in [
        // `Box` exists only inside the private sibling `other`; the bare leaf
        // cannot reach it from `mine`.
        (
            &[
                "module other; data Box<T> { value: T; }",
                "module mine; const B: Box<u64> = Box { value: 3 };",
            ][..],
            "does not select one declared canonical data type",
        ),
        // The selected base is not a generic template at all.
        (
            &["module mine; data Box { value: u64; } const B: Box<u64> = Box { value: 3 };"][..],
            "does not select a generic data template",
        ),
    ] {
        let syntax = parse(sources);
        let diagnostics = crate::resolve(crate::ResolutionRequest::new(&syntax))
            .expect_err("an unselected or ineligible carrier still rejects");
        assert!(
            diagnostics[0].message.contains(fragment),
            "{sources:?}: {diagnostics:?}"
        );
    }
}

#[test]
fn module_constrained_consts_discharge_domain_facts_at_declaration_site() {
    // `X`'s constrained carrier selects the module-local `Pos` — never the
    // same-leaf foreign or root sibling — and replays its facts with `self`
    // bound to the canonical value before publishing identity.
    let syntax = parse(&[
        "domain u64::Pos requires self > 5;",
        "module other; domain u64::Pos requires self > 9;",
        "module mine; domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 3;",
    ]);
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("the module-local domain owns and discharges the fact");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::X")
        .expect("the module constant resolved");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "a discharged constrained const publishes canonical identity"
    );
}
