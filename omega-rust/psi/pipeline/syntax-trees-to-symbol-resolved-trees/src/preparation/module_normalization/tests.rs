//! Module normalization tests.

use super::{Item, SyntaxTrees};
use source::SourceId;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn parse(sources: &[&str]) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::default();
    for (source_ordinal, text) in sources.iter().enumerate() {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize normalization control");
        parse_syntax_trees_into_with_id(&mut syntax, SourceId(source_ordinal), &tokens)
            .expect("parse normalization control");
    }
    syntax
}

#[test]
fn module_scalar_constants_survive_pre_symbol_normalization() {
    let syntax = parse(&[
        "module first; const VALUE: u64 = 1;",
        "module second; const VALUE: u64 = 2;",
    ]);
    crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax.clone()),
    )
    .expect("body substitution follows symbols");
    crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("exact module constant identities");
}

#[test]
fn unused_module_scalar_initializers_obey_their_declared_carriers() {
    for (carrier, value) in [
        ("u8", "256"),
        ("u8", "1u64"),
        ("bool", "1"),
        ("u64", "true"),
        ("f32", "1.0f64"),
    ] {
        let source = format!("module settings; const VALUE: {carrier} = {value};");
        assert!(
            crate::preparation::generic_data::normalize_generic_data(
                crate::preparation::generic_data::GenericDataRequest::new(parse(&[&source]))
            )
            .is_err(),
            "invalid unused declaration accepted: {source}"
        );
    }
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
fn structural_integer_encoding_retains_exact_landing_domain() {
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::literals::{IntegerLanding, LandedIntegerType};
    use syntax_trees::expression::ExpressionNode;
    for domain in [
        ArithmeticDomain::Exact,
        ArithmeticDomain::Wrapping,
        ArithmeticDomain::Saturating,
        ArithmeticDomain::Trapping,
    ] {
        let mut syntax = parse(&["module settings; const SIZE: [u8; 1] = [1u8];"]);
        let (handle, literal) = syntax
            .expressions
            .iter_expressions()
            .find_map(|(handle, node)| {
                if let ExpressionNode::Integer(literal) = node {
                    Some((handle, literal.clone()))
                } else {
                    None
                }
            })
            .expect("integer array leaf");
        syntax.expressions.replace_expression(
            handle,
            ExpressionNode::Integer(literal.with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain,
            })),
        );
        let result = crate::preparation::generic_data::normalize_generic_data(
            crate::preparation::generic_data::GenericDataRequest::new(syntax),
        );
        if domain == ArithmeticDomain::Exact {
            result.expect("matching exact component landing");
        } else {
            assert!(
                result.expect_err("canonical component cannot erase arithmetic policy")[0]
                    .message
                    .contains("integer literal landing conflicts")
            );
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
fn module_generic_records_do_not_overwrite_same_name_templates() {
    let syntax = parse(&[
        "module first; data Box<T> { value: T; }",
        "module second; data Box<T> { other: T; }",
    ]);
    crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("distinct module record templates remain available");
}

#[test]
fn module_attached_methods_do_not_join_unrelated_generic_carriers() {
    let syntax = parse(&[
        "data Box<T> { value: T; }",
        "module other; data Box {} machine Box::act(&self) {}",
    ]);
    let syntax = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("same carrier spelling has distinct owners");
    assert_eq!(
        syntax
            .root_items()
            .filter(|item| matches!(item, Item::Machine(_)))
            .count(),
        1
    );
}

#[test]
fn same_spelled_module_arguments_select_their_own_declarations() {
    let syntax = parse(&[
        "data Box<T> { value: T; }",
        "module first; data Point {} data Container { value: Box<Point>; }",
        "module second; data Point {}",
    ]);
    crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("the argument is selected in its declaring module");
}

#[test]
fn unrelated_primitive_generic_instances_remain_available_with_modules() {
    let syntax = parse(&[
        "data Box<T> { value: T; }",
        "module other; data Container { value: Box<u64>; }",
    ]);
    crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("module presence does not disable unrelated templates");
}

#[test]
fn module_trait_defaults_join_the_exact_selected_template() {
    let mut syntax = parse(&[
        "module first; trait Service { machine run(&mut self) { } } data Worker {} first_membership: Worker satisfies Service;",
        "module second; trait Service { machine stop(&mut self) { } } data Worker {}",
    ]);
    crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax, None, Vec::new())
        .expect("same-spelled module traits keep their own default templates");
    let mut machines = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) => Some((
                machine.name.as_str().to_string(),
                machine
                    .attached_data
                    .as_ref()
                    .map(|attached| attached.source_span().source_id),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    machines.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        machines,
        [("first::Worker::run".to_string(), Some(SourceId(0)))],
        "only the declaring module's template attaches to its own carrier: {machines:?}"
    );
}

#[test]
fn module_closed_conformance_rows_keep_the_exact_declaring_path() {
    use syntax_trees::item::{ConformanceBody, ConformanceMember};
    let mut syntax = parse(&[
        "module first; trait Service { machine run(&mut self) { } } data Worker {} membership: Worker satisfies Service {}",
        "module second; trait Service { machine stop(&mut self) { } }",
    ]);
    crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax, None, Vec::new())
        .expect("a module conformance selects the same-module trait");
    let conformance = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Conformance(conformance) => Some(conformance),
            _ => None,
        })
        .expect("one conformance");
    let ConformanceBody::Closed { members } = &conformance.body else {
        panic!("closed conformance retained");
    };
    let [
        ConformanceMember::TraitDefault {
            declaring_trait,
            requirement_ordinal,
            machine,
        },
    ] = syntax.items.conformance_members(*members)
    else {
        panic!("exactly one synthesized default row");
    };
    assert_eq!(declaring_trait.as_str(), "first::Service");
    assert_eq!(*requirement_ordinal, 0);
    assert_eq!(machine.name.as_str(), "run");
    assert_eq!(
        machine
            .attached_data
            .as_ref()
            .map(|attached| attached.as_str()),
        Some("Worker")
    );
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
fn module_attached_machines_override_only_their_own_carrier() {
    let mut syntax = parse(&[
        "trait Service { machine run(&mut self) { } }",
        "module first; data Worker {} machine Worker::run(&mut self) { } first_membership: Worker satisfies Service;",
        "module second; data Worker {} second_membership: Worker satisfies Service;",
    ]);
    crate::preparation::trait_defaults::synthesize_trait_defaults(&mut syntax, None, Vec::new())
        .expect("carrier identity is exact across same-spelled modules");
    let mut names = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) => Some(machine.name.as_str().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        ["Worker::run", "second::Worker::run"],
        "the authored override suppresses only its own carrier's default: {names:?}"
    );
}

#[test]
fn module_domains_and_operators_lower_under_their_namespace() {
    for source in [
        "module units; domain u64::Distance;",
        "module units; operator add(left: u64, right: u64) -> u64;",
        "module units; domain u64::Distance requires self > 0; operator u64::Distance::add(left: u64 in u64::Distance, right: u64) -> u64;",
    ] {
        let syntax = parse(&[source]);
        crate::resolve(crate::ResolutionRequest::new(&syntax))
            .expect("module-owned domains and operators lower under exact namespaces");
    }
}

/// Module-owned `IntervalSet`/`CountedQuantity` declarations are ordinary
/// user templates: the content-algebra exemption covers only the unmoduled
/// declarations a bare algebra spelling can select, so a qualified or
/// imported module application synthesizes its closed instance under the
/// declaring module's logical path.
#[test]
fn module_owned_algebra_leaf_names_normalize_as_templates() {
    let syntax = parse(&[
        "module units; pub data IntervalSet<T> { value: T; } pub data CountedQuantity<U> { magnitude: U; }",
        "data Holder { set: units::IntervalSet<u64>; count: units::CountedQuantity<u64>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("module-owned same-leaf templates normalize");
    let mut instances = normalized
        .root_items()
        .filter_map(|item| match item {
            Item::Data(data) if data.generic_instance.is_some() => {
                Some(data.name.as_str().to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    instances.sort();
    assert_eq!(
        instances,
        [
            "CountedQuantity<u64>".to_string(),
            "IntervalSet<u64>".to_string()
        ],
        "each module template synthesizes its closed instance: {instances:?}"
    );
    let program = crate::resolve(crate::ResolutionRequest::new(&normalized))
        .expect("the synthesized instances resolve");
    for expected in ["units::IntervalSet<u64>", "units::CountedQuantity<u64>"] {
        let instance = program
            .data_definitions
            .iter()
            .find(|definition| program.symbols.display_path(definition.symbol, "::") == expected)
            .unwrap_or_else(|| panic!("{expected} instance"));
        assert!(
            instance.generic_instance.is_some(),
            "{expected} retains its authored application origin"
        );
    }
}

/// The exemption still pins the unmoduled algebra carriers: a root-scope
/// `IntervalSet` application keeps its authored generic spelling instead of
/// synthesizing a record, and an imported module leaf cannot capture it.
#[test]
fn unmoduled_algebra_carriers_keep_their_generic_spelling() {
    let syntax = parse(&[
        "module units; pub data IntervalSet<T> { value: T; }",
        "data IntervalSet<Space> { start: u64; end: u64; } data Holder { field: IntervalSet<u64>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("the unmoduled algebra carrier is exempt from synthesis");
    assert!(
        normalized
            .root_items()
            .all(|item| !matches!(item, Item::Data(data) if data.generic_instance.is_some())),
        "no closed instance may stand in for the unmoduled algebra"
    );
    use syntax_trees::types::TypeReferenceNode;
    let holder = normalized
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == "Holder" => Some(data),
            _ => None,
        })
        .expect("Holder");
    let [syntax_trees::item::DataMember::Field(field)] =
        normalized.tables.items.data_members(holder.members)
    else {
        panic!("one field")
    };
    assert!(
        matches!(
            normalized
                .tables
                .type_references
                .type_reference(field.type_reference),
            TypeReferenceNode::Generic { .. }
        ),
        "the root application retains its structural generic argument"
    );
}

/// A narrow import selects the exact module template for a leaf application
/// in a source that does not declare the leaf itself.
#[test]
fn imported_module_algebra_leaf_selects_the_module_template() {
    let syntax = parse(&[
        "module units; pub data IntervalSet<T> { value: T; }",
        "use units::IntervalSet; data Holder { field: IntervalSet<u64>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("the imported module template synthesizes");
    let instances = normalized
        .root_items()
        .filter(|item| matches!(item, Item::Data(data) if data.generic_instance.is_some()))
        .count();
    assert_eq!(
        instances, 1,
        "only the selected module template materializes"
    );
    let program = crate::resolve(crate::ResolutionRequest::new(&normalized))
        .expect("the imported instance resolves");
    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.generic_instance.is_some())
        .expect("one instance");
    assert_eq!(
        program.symbols.display_path(instance.symbol, "::"),
        "units::IntervalSet<u64>",
        "the leaf application selects the imported module owner"
    );
}

/// Local declarations precede imported names: a source that declares its own
/// `IntervalSet` keeps the leaf on that unmoduled declaration even when it
/// also imports the module's same-leaf template, so the exempt root carrier
/// still keeps its structural generic spelling and no instance is born.
#[test]
fn same_source_algebra_leaf_outranks_the_module_import() {
    let syntax = parse(&[
        "module units; pub data IntervalSet<T> { value: T; }",
        "use units::IntervalSet; data IntervalSet<Space> { start: u64; end: u64; } data Holder { field: IntervalSet<u64>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("the local declaration keeps the exempt leaf");
    assert!(
        normalized
            .root_items()
            .all(|item| !matches!(item, Item::Data(data) if data.generic_instance.is_some())),
        "the imported template cannot capture a same-source leaf"
    );
    use syntax_trees::types::TypeReferenceNode;
    let holder = normalized
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.name.as_str() == "Holder" => Some(data),
            _ => None,
        })
        .expect("Holder");
    let [syntax_trees::item::DataMember::Field(field)] =
        normalized.tables.items.data_members(holder.members)
    else {
        panic!("one field")
    };
    assert!(
        matches!(
            normalized
                .tables
                .type_references
                .type_reference(field.type_reference),
            TypeReferenceNode::Generic { .. }
        ),
        "the leaf application retains its structural generic argument"
    );
    let program = crate::resolve(crate::ResolutionRequest::new(&normalized))
        .expect("the local algebra carrier resolves");
    let holder = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Holder")
        .expect("Holder");
    let [symbol_resolved_trees::data::DataMember::Field(field)] =
        program.data_members(holder.members)
    else {
        panic!("one field")
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) = &field.type_reference
    else {
        panic!("the leaf application remains a generic carrier")
    };
    assert_eq!(
        program.symbols.display_path(application.base_symbol, "::"),
        "IntervalSet",
        "the leaf selects the same-source unmoduled declaration"
    );
}

#[test]
fn module_generic_operators_lower_under_their_namespace() {
    for source in [
        "module units; pub operator copy<T>(value: T) -> T;",
        "module units; pub operator at<T, const N: u64>(items: [T; N], index: u64) -> T;",
    ] {
        let syntax = parse(&[source]);
        crate::resolve(crate::ResolutionRequest::new(&syntax))
            .expect("module-owned generic operators lower under exact namespaces");
    }
}

#[test]
fn same_named_domain_siblings_discharge_const_facts_against_their_exact_owner() {
    // The module leaf reaches its own domain; the root sibling neither
    // competes nor supplies its requirement. `3 > 0` holds for the
    // module-local `Distance` while the root `3 > 5` would refute.
    let syntax = parse(&[
        "domain u64::Distance requires self > 5;",
        "module units; domain u64::Distance requires self > 0; data Bound<const N: u64> where N in Distance, { value: u64; } data Use { value: Bound<3>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("the module-local domain owns the membership fact");
    crate::resolve(crate::ResolutionRequest::new(&normalized))
        .expect("the discharged instance resolves");

    // The same generic at the root selects the root domain, whose
    // requirement `3 > 5` is false.
    let syntax = parse(&[
        "domain u64::Distance requires self > 5;",
        "module units; domain u64::Distance requires self > 0;",
        "data Bound<const N: u64> where N in Distance, { value: u64; } data Use { value: Bound<3>; }",
    ]);
    let diagnostics = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect_err("the root domain owns and refutes the membership fact");
    assert!(
        diagnostics[0].message.contains("is false"),
        "{diagnostics:?}"
    );
}

#[test]
fn constrained_generic_arguments_select_their_module_domain_owner() {
    use syntax_trees::item::DataMember;
    use syntax_trees::types::TypeConstraintNode;
    use syntax_trees::types::TypeReferenceNode;
    let syntax = parse(&[
        "data Cell<T> { value: T; } domain u64::Distance requires self > 5; data Root { value: Cell<u64 in Distance>; }",
        "module units; domain u64::Distance requires self > 0; data Holder { value: Cell<u64 in Distance>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("same-named domain siblings select their exact owners");
    let mut owners = normalized
        .root_items()
        .filter_map(|item| {
            let Item::Data(data) = item else {
                return None;
            };
            data.generic_instance?;
            let [DataMember::Field(field)] = normalized.tables.items.data_members(data.members)
            else {
                return None;
            };
            let TypeReferenceNode::Constrained { constraints, .. } = normalized
                .tables
                .type_references
                .type_reference(field.type_reference)
            else {
                return None;
            };
            let [TypeConstraintNode::Domain(domain)] =
                normalized.tables.type_references.constraints(*constraints)
            else {
                return None;
            };
            Some(domain.name.source_span().source_id)
        })
        .collect::<Vec<_>>();
    owners.sort_by_key(|source| source.0);
    assert_eq!(
        owners,
        [SourceId(0), SourceId(1)],
        "each instance retains the constraint authored in its own module"
    );
    crate::resolve(crate::ResolutionRequest::new(&normalized))
        .expect("each instance's retained spelling resolves its own domain");
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
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse");
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
fn module_local_domain_outranks_a_same_leaf_root_generic_family() {
    // Module-local precedence pools generic families with every other
    // candidate: the module's own non-generic domain still owns the leaf.
    let syntax = parse(&[
        "domain<T> T::Tagged requires self > 5;",
        "module units; domain u64::Tagged requires self > 0; data Bound<const N: u64> where N in Tagged, { value: u64; } data Use { value: Bound<7>; }",
    ]);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest::new(syntax),
    )
    .expect("the module-local owner discharges the fact");
    let instance = normalized
        .root_items()
        .find_map(|item| match item {
            Item::Data(data) if data.generic_instance.is_some() => Some(data),
            _ => None,
        })
        .expect("one synthesized instance");
    assert!(
        normalized
            .tables
            .items
            .proof_facts(instance.where_facts)
            .is_empty(),
        "7 > 0 discharges under the module-local owner"
    );
    crate::resolve(crate::ResolutionRequest::new(&normalized))
        .expect("the discharged instance resolves");
}

#[test]
fn same_leaf_generic_family_declines_constrained_argument_identity() {
    use std::sync::Arc;
    // A constrained argument cannot record the non-generic sibling as its
    // domain owner while a generic family contests the same leaf: the
    // identity declines and the open application stays for resolution.
    let text = "domain u64::Tagged; domain<T> T::Tagged; data Cell<T> { value: T; } data Root { value: Cell<u64 in Tagged>; }";
    let mut map = source::SourceMap::default();
    let source_id = map
        .add(std::path::PathBuf::from("tagged.omg"), text.to_owned())
        .source_id;
    let mut syntax = SyntaxTrees::default();
    let tokens = Lexer::new(text).tokenize().expect("tokenize");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse");
    let sources = Arc::new(map);
    let normalized = crate::preparation::generic_data::normalize_generic_data(
        crate::preparation::generic_data::GenericDataRequest {
            syntax,
            sources: Some(sources.clone()),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .expect("a contested constraint still normalizes");
    assert!(
        normalized
            .root_items()
            .all(|item| !matches!(item, Item::Data(data) if data.generic_instance.is_some())),
        "no instance may claim the contested domain owner"
    );
    crate::resolve(crate::ResolutionRequest {
        syntax: &normalized,
        sources: Some(sources),
        top_level_bindings: Vec::new(),
    })
    .expect("the open application resolves downstream");
}

#[test]
fn module_generic_carrier_constants_defer_value_admission_with_exact_base_selection() {
    // A generic carrier admits its value through the closed instance's
    // evaluation at lowering as at root — while the base template still
    // selects exactly in the declaring source, preferring the module-local
    // `Box` over the imported same-leaf one. The declaration's canonical
    // identity is the closed `Box<u64>` instance's encoding.
    let syntax = parse(&[
        "module other; pub data Box<T> { value: T; }",
        "module mine; use other::Box; data Box<T> { value: T; } const B: Box<u64> = Box { value: 3 };",
    ]);
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("generic-carrier module constant resolves under its own template");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::B")
        .expect("the module constant resolved");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "a closed generic carrier publishes the instance's canonical identity"
    );
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &declaration.declared_type
    else {
        panic!("the declared carrier remains a generic application");
    };
    assert_eq!(
        program.symbols.display_path(application.base_symbol, "::"),
        "mine::Box",
        "the base template selects owner-locally in the declaring source"
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
fn module_generic_carrier_constants_select_qualified_foreign_bases() {
    // `geom::Box` is the complete logical path selected in the declaring
    // source; the constant never falls back to a same-leaf sibling.
    let syntax = parse(&[
        "module geom; pub data Box<T> { value: T; }",
        "module mine; const B: geom::Box<u64> = geom::Box { value: 3 };",
    ]);
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("a qualified foreign generic carrier resolves");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::B")
        .expect("the module constant resolved");
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &declaration.declared_type
    else {
        panic!("the declared carrier remains a generic application");
    };
    assert_eq!(
        program.symbols.display_path(application.base_symbol, "::"),
        "geom::Box"
    );
}

#[test]
fn module_generic_carrier_constants_remain_usable_as_const_arguments() {
    // Deferred value admission keeps the declaration's own identity for each
    // use: the constant still serves as a generic const argument.
    let syntax = parse(&[
        "module mine; data Box<T> { value: T; } const B: Box<u64> = Box { value: 3 }; data Holder<const V: Box<u64>> { x: u64; } data Use { h: Holder<B>; }",
    ]);
    crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("the generic-carrier constant still serves as a const argument");
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
fn module_generic_carrier_constants_publish_public_identity() {
    // Public constants owe canonical declaration identity: a public
    // generic-carrier const publishes the closed instance's encoding rather
    // than deferring value admission the way a private one may.
    let syntax = parse(&[
        "module mine; data Box<T> { value: T; } pub const B: Box<u64> = Box { value: 3 };",
    ]);
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("a public generic-carrier const publishes canonical identity");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::B")
        .expect("the module constant resolved");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "the public declaration identity is the closed instance's encoding"
    );
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

#[test]
fn module_constrained_consts_discharge_closed_indexed_domains() {
    // The closed index application `u64::Window<8>` selects `mine`'s family
    // under module-local precedence — never the imported same-leaf sibling,
    // whose `self > N` fact would refute `3` — binds `N` to `8`, replays
    // `self < N`, and publishes identity.
    let syntax = parse(&[
        "module other; pub domain<const N: u64> u64::Window<N> requires self > N;",
        "module mine; use other; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window<8> = 3;",
    ]);
    let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("the module-local family owns and discharges the indexed fact");
    let declaration = program
        .const_declarations
        .iter()
        .find(|declaration| program.symbols.display_path(declaration.symbol, "::") == "mine::X")
        .expect("the module constant resolved");
    assert!(
        declaration.canonical_value_encoding.is_some(),
        "a discharged indexed constrained const publishes canonical identity"
    );
}

#[test]
fn module_constrained_consts_reject_refuted_or_unproved_domains() {
    for (sources, fragment) in [
        // The module-local owner refutes `0 > 0`.
        (
            &["module mine; domain u64::Pos requires self > 0; const X: u64 in u64::Pos = 0;"][..],
            "is false",
        ),
        // `Pos` exists only inside the unimported sibling `other`; the leaf
        // cannot reach it from `mine`, so no owner discharges the constraint.
        (
            &[
                "module other; domain u64::Pos requires self > 0;",
                "module mine; const X: u64 in u64::Pos = 3;",
            ][..],
            "constrained const declarations require declaration-site proof checking",
        ),
        // The selected family replays `self < N` with `N` bound to `8` at
        // `9` — a precise refutation, not the generic fence.
        (
            &[
                "module mine; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window<8> = 9;",
            ][..],
            "is false",
        ),
        // A closed index application whose argument cannot bind a concrete
        // value at declaration site — `K` names no scoped or selected const —
        // still owes its open-template membership proof.
        (
            &[
                "module mine; domain<const N: u64> u64::Window<N> requires self < N; const X: u64 in u64::Window<K> = 3;",
            ][..],
            "constrained const declarations require declaration-site proof checking",
        ),
        // Two narrow-imported foreign families contest the bare leaf; the
        // pool declines rather than bind the index against a guessed owner.
        (
            &[
                "module a; pub domain<const N: u64> u64::Window<N> requires self < N;",
                "module b; pub domain<const N: u64> u64::Window<N> requires self < N;",
                "module mine; use a::Window; use b::Window; const X: u64 in Window<8> = 3;",
            ][..],
            "constrained const declarations require declaration-site proof checking",
        ),
    ] {
        let syntax = parse(sources);
        let diagnostics = crate::resolve(crate::ResolutionRequest::new(&syntax))
            .expect_err("a refuted or unproved constrained const still rejects");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(fragment)),
            "{sources:?}: {diagnostics:?}"
        );
    }
}
