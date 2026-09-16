use super::constant_selection;
use crate::pre_resolution::GenericDataRequest;
use crate::pre_resolution::normalize_generic_data;
use crate::preparation::generic_data::EvaluatedConst;
use crate::preparation::generic_data::desugar_generic_data_instances;
use crate::preparation::generic_data::evaluate_const_argument_expression;
use crate::preparation::generic_data::validate_direct_const_arguments;
use source::SourceId;
use source_files_to_tokens::Lexer;
use std::collections::HashMap;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::DataMember;
use syntax_trees::item::Item;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn parse_sources(root: &str, module: &str, module_first: bool) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::new(SourceId::default());
    let mut sources = [(SourceId(1), root), (SourceId(2), module)];
    if module_first {
        sources.reverse();
    }
    for (source_id, source) in sources {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize module constants");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse module constants");
    }
    syntax
}

#[test]
fn module_constants_cannot_overwrite_or_fall_back_to_root_generic_indices() {
    for module_first in [false, true] {
        let root = "const SIZE: u64 = 1; data Buffer<const N: u64> { value: u64; }";
        let module = "module combat; const SIZE: u64 = 2; data Use { value: Buffer<SIZE>; }";
        let syntax = normalize_generic_data(GenericDataRequest::new(parse_sources(
            root,
            module,
            module_first,
        )))
        .expect("bare module constant selects its exact value");
        assert!(syntax.root_items().any(|item| matches!(item,
            Item::Data(definition) if definition.name.as_str() == "Buffer<2>"
        )));
        assert!(!syntax.root_items().any(|item| matches!(item,
            Item::Data(definition) if definition.name.as_str() == "Buffer<1>"
        )));
        {
            let argument = "SIZE + 0";
            let module = format!(
                "module combat; const SIZE: u64 = 2; data Use {{ value: Buffer<{argument}>; }}"
            );
            let mut syntax = parse_sources(root, &module, module_first);
            let result = desugar_generic_data_instances(&mut syntax, &mut Vec::new());
            assert!(
                result.is_err(),
                "generic {argument}, module_first={module_first}"
            );
            let errors = result.expect_err("module generic selection rejects");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("module constant `SIZE`")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn generic_binders_keep_exact_template_arguments_while_concrete_constants_fold() {
    for module_first in [false, true] {
        for binder in ["const SIZE: u64", "SIZE"] {
            let root = format!(
                "const SIZE: u64 = 1;
                 data Buffer<const N: u64> {{ value: u64; }}
                 data Generic<{binder}> {{ value: Buffer<SIZE>; }}
                 data Root {{ value: Buffer<SIZE>; }}"
            );
            let module = "module combat; const SIZE: u64 = 2; data Use { value: Buffer<SIZE>; }";
            let syntax = parse_sources(&root, module, module_first);
            let template = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Data(definition) if definition.name.as_str() == "Generic" => {
                        Some(definition)
                    }
                    _ => None,
                })
                .expect("generic template");
            let [DataMember::Field(field)] = syntax.items.data_members(template.members) else {
                panic!("one template field");
            };
            let field_type = field.type_reference;
            let original_type = syntax.type_references.type_reference(field_type).clone();
            let TypeReferenceNode::Generic { arguments, .. } = &original_type else {
                panic!("generic field application");
            };
            let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
                panic!("one template argument");
            };
            let argument = *argument;
            let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(argument)
            else {
                panic!("authored binder argument");
            };
            let original_span = name.source_span();

            let syntax = normalize_generic_data(GenericDataRequest::new(syntax))
                .expect("generic templates defer binder selection");

            assert_eq!(
                syntax.type_references.type_reference(field_type),
                &original_type
            );
            let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(argument)
            else {
                panic!("binder remains named");
            };
            assert_eq!(name.as_str(), "SIZE");
            assert_eq!(name.source_span(), original_span);
            for expected in ["Buffer<1>", "Buffer<2>"] {
                assert!(
                    syntax.root_items().any(|item| matches!(item,
                        Item::Data(definition) if definition.name.as_str() == expected
                    )),
                    "missing concrete {expected}"
                );
            }
            assert!(!syntax.root_items().any(|item| matches!(item,
                Item::Data(definition) if definition.name.as_str() == "Buffer<SIZE>"
            )));
        }
    }
}

#[test]
fn module_constant_domain_expressions_retain_unresolved_selection() {
    for module_first in [false, true] {
        for argument in ["SIZE + 0", "SIZE + 1"] {
            let root = "const SIZE: u64 = 1; domain<T, const N: u64> T::Indexed<N>;";
            let module = format!(
                "module combat; const SIZE: u64 = 2; data Use {{ value: u64 in Indexed<{argument}>; }}"
            );
            let mut syntax = parse_sources(root, &module, module_first);
            let index = domain_index_argument(&syntax);
            let original = syntax.tables.type_references.type_reference(index).clone();
            let TypeReferenceNode::ConstExpression(expression) = original else {
                panic!("authored domain argument remains an expression");
            };
            let name_expression = match syntax.expressions.expression(expression) {
                ExpressionNode::Name(_) => expression,
                ExpressionNode::Binary(binary) => binary.left,
                _ => panic!("name or addition domain argument"),
            };
            let original_name = syntax.expressions.expression(name_expression).clone();
            let ExpressionNode::Name(path) = original_name else {
                panic!("domain argument contains the authored name");
            };
            let original_span = syntax.expressions.identifier_path_members(path)[0].source_span();
            assert_eq!(original_span.source_id, SourceId(2));

            desugar_generic_data_instances(&mut syntax, &mut Vec::new())
                .expect("named domain expressions await resolved selection");

            assert_eq!(domain_index_argument(&syntax), index);
            assert_eq!(
                syntax.tables.type_references.type_reference(index),
                &original
            );
            assert_eq!(
                syntax.expressions.expression(name_expression),
                &original_name
            );
            let member = &syntax.expressions.identifier_path_members(path)[0];
            assert_eq!(member.as_str(), "SIZE");
            assert_eq!(member.source_span(), original_span);
        }
    }
}

fn domain_index_argument(syntax: &SyntaxTrees) -> TypeReferenceHandle {
    let domains = syntax.tables.type_references.domain_constraints();
    let [domain] = domains.as_slice() else {
        panic!("one domain application");
    };
    let [argument] = syntax
        .tables
        .type_references
        .type_reference_handles(domain.arguments)
    else {
        panic!("one domain index");
    };
    *argument
}

#[test]
fn named_module_domain_indices_cannot_fold_through_root_names() {
    for module_first in [false, true] {
        let mut syntax = parse_sources(
            "const SIZE: u64 = 1; domain<T, const N: u64> T::Indexed<N>;",
            "module combat; const SIZE: u64 = 2; data Use { value: u64 in Indexed<SIZE>; }",
            module_first,
        );
        let index = domain_index_argument(&syntax);
        let TypeReferenceNode::Named(name) = syntax.tables.type_references.type_reference(index)
        else {
            panic!("authored bare domain argument is a named index");
        };
        assert_eq!(name.as_str(), "SIZE");
        assert_eq!(name.source_span().source_id, SourceId(2));
        desugar_generic_data_instances(&mut syntax, &mut Vec::new())
            .expect("named domain index selects the exact module constant");
        assert!(
            matches!(syntax.type_references.type_reference(index), TypeReferenceNode::Named(value) if value.as_str() == "2")
        );
        let normalization = syntax
            .type_references
            .const_argument_normalization(index)
            .expect("retained named index normalization");
        assert_eq!(normalization.canonical_result_encoding, "integer3:u641:2");
        let [origin] = syntax
            .type_references
            .const_argument_origins(normalization.selections)
        else {
            panic!("one exact selected declaration");
        };
        assert_eq!(origin.reference.source_id, SourceId(2));
        assert_eq!(origin.declaration.source_id, SourceId(2));
        assert_eq!(origin.initializer.source_id, SourceId(2));
        assert_eq!(origin.canonical_value_encoding, "integer3:u641:2");
    }
}

#[test]
fn imported_module_constants_cannot_discharge_generic_where_facts() {
    for module_first in [false, true] {
        let root = "use combat::SIZE; const SIZE: u64 = 1;
            data Buffer<const N: u64> where N == SIZE, { value: u64; }
            data Use { value: Buffer<1>; }";
        let mut syntax = parse_sources(root, "module combat; const SIZE: u64 = 2;", module_first);
        let result = desugar_generic_data_instances(&mut syntax, &mut Vec::new());
        assert!(result.is_err(), "where fact, module_first={module_first}");
        let errors = result.expect_err("module where-fact selection rejects");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("module constant `SIZE`")),
            "{errors:?}"
        );
    }
}

#[test]
fn unrelated_root_constants_and_literal_module_arguments_still_normalize() {
    for module_first in [false, true] {
        let root = "const SIZE: u64 = 1;
            data Buffer<const N: u64> where N == SIZE, { value: u64; }
            data Use { value: Buffer<SIZE>; }";
        let module = "module combat; const SIZE: u64 = 2; data Other { value: Buffer<1>; }";
        let mut syntax = parse_sources(root, module, module_first);
        desugar_generic_data_instances(&mut syntax, &mut Vec::new())
            .expect("unrelated module constants cannot alter root or literal arguments");
        assert!(
            syntax
                .root_items()
                .any(|item| matches!(item, Item::Data(data) if data.name.as_str() == "Buffer<1>"))
        );
        assert!(
            !syntax
                .root_items()
                .any(|item| matches!(item, Item::Data(data) if data.name.as_str() == "Buffer<2>"))
        );
    }
}

#[test]
fn symbolic_const_binders_precede_imported_module_constants() {
    let root = "use combat::SIZE; const SIZE: u64 = 1;
        data Buffer<const SIZE: u64> { value: Buffer<SIZE + 0>; }";
    let syntax = parse_sources(root, "module combat; const SIZE: u64 = 2;", false);
    let expression = syntax
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Binary(_)).then_some(handle))
        .expect("symbolic index expression");
    let value = evaluate_const_argument_expression(
        &syntax,
        expression,
        &HashMap::from([("SIZE".to_owned(), 1)]),
        &HashMap::new(),
        &HashSet::from(["SIZE".to_owned()]),
        None,
        &mut Vec::new(),
    )
    .expect("local symbolic binder masks constant candidates");
    assert!(matches!(value, EvaluatedConst::Symbolic(_)));
}

#[test]
fn qualified_module_constants_cannot_fold_through_same_spelled_type_scopes() {
    let root = "const combat::SIZE: u64 = 1;
        data Buffer<const N: u64> { value: u64; }
        data Use { value: Buffer<combat::SIZE>; }";
    for module_first in [false, true] {
        let mut syntax = parse_sources(root, "module combat; const SIZE: u64 = 2;", module_first);
        let applications = syntax.type_references.generic_nodes();
        let [application] = applications.as_slice() else {
            panic!("one authored generic application");
        };
        let original_application = syntax.type_references.type_reference(*application).clone();
        let TypeReferenceNode::Generic { arguments, .. } = &original_application else {
            panic!("generic application");
        };
        let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
            panic!("one constant argument");
        };
        let argument = *argument;
        let original_argument = syntax.type_references.type_reference(argument).clone();
        let TypeReferenceNode::Named(name) = &original_argument else {
            panic!("qualified named argument");
        };
        let original_span = name.source_span();
        let selection = constant_selection::ConstantSelection::new(&syntax, None, Vec::new())
            .expect("constant headers");
        // The shared resolver admits both complete paths as candidates. It
        // selects neither; normalization must leave rejection to validation.
        assert!(
            selection
                .select(&syntax, name)
                .expect("ambiguous selection")
                .is_none()
        );

        desugar_generic_data_instances(&mut syntax, &mut Vec::new())
            .expect("unresolved qualified argument is preserved for validation");

        assert_eq!(
            syntax.type_references.type_reference(*application),
            &original_application
        );
        assert_eq!(
            syntax.type_references.type_reference(argument),
            &original_argument
        );
        let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(argument) else {
            panic!("qualified argument remains named");
        };
        assert_eq!(name.source_span(), original_span);
        assert!(
            syntax
                .type_references
                .const_argument_normalization(argument)
                .is_none()
        );
        assert!(!syntax.root_items().any(|item| matches!(item,
            Item::Data(definition) if matches!(definition.name.as_str(), "Buffer<1>" | "Buffer<2>")
        )));
    }
}

#[test]
fn direct_structural_normalization_replays_live_expression_and_selected_parameter() {
    let source = "data Value [copy] { value: u64; }
        data Pick<const V: Value> { value: u64; }
        machine keep(first: Pick<(Value { value: 1 })>, second: Pick<(Value { value: 2 })>) -> u64 { 0 }";
    let syntax = normalize_generic_data(GenericDataRequest::new(parse_sources(source, "", false)))
        .expect("direct structural values normalize");
    crate::resolution::resolve(crate::resolution::ResolutionRequest::new(&syntax))
        .expect("actual direct arguments lower");
    let (application, argument, normalization) = syntax
        .type_references
        .generic_nodes()
        .into_iter()
        .find_map(|application| {
            let TypeReferenceNode::Generic { arguments, .. } =
                syntax.type_references.type_reference(application)
            else {
                return None;
            };
            syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| {
                    let normalization = syntax
                        .type_references
                        .const_argument_normalization(*argument)?;
                    normalization
                        .authored_expression
                        .is_valid()
                        .then(|| (application, *argument, normalization.clone()))
                })
        })
        .expect("retained direct generic application");
    for expression in [
        ExpressionHandle::invalid(),
        ExpressionHandle::from_parts(
            normalization.authored_expression.arena_index(),
            normalization.authored_expression.generation() + 1,
        ),
    ] {
        let mut changed = normalization.clone();
        changed.authored_expression = expression;
        assert!(crate::constant::validate_normalized_expression(&syntax, &changed).is_err());
    }
    let mut swapped = normalization.clone();
    swapped.authored_expression = syntax
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| {
            (matches!(node, ExpressionNode::StructLiteral(_))
                && syntax.expressions.source_span(handle) != normalization.reference)
                .then_some(handle)
        })
        .expect("another real authored constructor");
    assert!(crate::constant::validate_normalized_expression(&syntax, &swapped).is_err());

    // The source coordinate alone cannot certify the normalized value: replay
    // the current AST under the actual template parameter before lowering it.
    let mut changed = syntax.clone();
    let ExpressionNode::StructLiteral(literal) = changed
        .expressions
        .expression(normalization.authored_expression)
    else {
        panic!("record constructor");
    };
    let field = changed.expressions.struct_fields(literal.fields)[0].value;
    let replacement = changed
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| {
            (handle != field && matches!(node, ExpressionNode::Integer(_))).then(|| node.clone())
        })
        .expect("different source integer");
    changed.expressions.replace_expression(field, replacement);
    let errors = crate::resolution::resolve(crate::resolution::ResolutionRequest::new(&changed))
        .expect_err("same-span value drift rejects");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("differs from its normalized value")),
        "{errors:?}"
    );

    let TypeReferenceNode::Generic {
        base_name,
        arguments,
        ..
    } = syntax.type_references.type_reference(application)
    else {
        panic!("actual application");
    };
    assert!(validate_direct_const_arguments(&syntax, base_name, *arguments, None).is_err());
    crate::constant::validate_normalized_const_argument(
        syntax.type_references.type_reference(argument),
        &normalization,
    )
    .expect("atom itself remains unchanged throughout the hostile replay");
}

fn parse_multiple_sources(sources: &[&str]) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::new(SourceId::default());
    for (index, source) in sources.iter().enumerate() {
        let tokens = Lexer::new(source).tokenize().expect("tokenize sources");
        parse_syntax_trees_into_with_id(&mut syntax, SourceId(index + 1), &tokens)
            .expect("parse sources");
    }
    syntax
}

/// The named argument of the only domain constraint matching `expected`,
/// for tests that fold one application among several.
fn named_domain_argument<'a>(
    syntax: &'a SyntaxTrees,
    expected: &str,
) -> (TypeReferenceHandle, &'a Identifier) {
    let constraint = syntax
        .tables
        .type_references
        .domain_constraints()
        .into_iter()
        .find(|constraint| constraint.name.as_str() == expected)
        .expect("domain application");
    let [argument] = syntax
        .tables
        .type_references
        .type_reference_handles(constraint.arguments)
    else {
        panic!("one index argument");
    };
    let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(*argument) else {
        panic!("named index argument");
    };
    (*argument, name)
}

#[test]
fn module_owned_indexed_domain_selects_its_own_telescope() {
    // `buffers::Counted` was fence-rejected as a module-owned generic domain;
    // the family now owns its closed applications under module name law, and
    // named indices fold through the declaration's own module constants.
    let module = "module buffers;
        domain<const N: u64> u64::Counted<N> requires self < N;
        const CAP: u64 = 3;
        data Main { direct: u64 in Counted<2>; named: u64 in Counted<CAP>; }";
    let syntax = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[module])))
        .expect("module-owned indexed domain admits closed applications");
    let arguments: Vec<TypeReferenceHandle> = syntax
        .tables
        .type_references
        .domain_constraints()
        .into_iter()
        .filter(|constraint| constraint.name.as_str() == "Counted")
        .flat_map(|constraint| {
            syntax
                .tables
                .type_references
                .type_reference_handles(constraint.arguments)
                .to_vec()
        })
        .collect();
    assert_eq!(arguments.len(), 2);
    let atoms: Vec<String> = arguments
        .iter()
        .map(
            |&argument| match syntax.type_references.type_reference(argument) {
                TypeReferenceNode::Named(name) => name.as_str().to_owned(),
                other => panic!("closed index stays named: {other:?}"),
            },
        )
        .collect();
    assert!(atoms.iter().any(|atom| atom == "2"));
    // `CAP` folded against `buffers::Counted`'s own module constant, with the
    // selected declaration's normalization record retained.
    let folded = arguments
        .iter()
        .zip(&atoms)
        .find_map(|(argument, atom)| (atom == "3").then_some(*argument))
        .expect("the module constant folded against `buffers::Counted`");
    let normalization = syntax
        .type_references
        .const_argument_normalization(folded)
        .expect("the selected module const leaves its normalization record");
    let [origin] = syntax
        .type_references
        .const_argument_origins(normalization.selections)
    else {
        panic!("one exact selected declaration");
    };
    assert_eq!(origin.declaration.source_id, SourceId(1));
}

#[test]
fn same_leaf_indexed_domain_families_follow_exact_namespace_selection() {
    let a = "module a; pub domain<const N: u64, const M: u64> u64::Counted<N, M>;";
    let b = "module b; pub domain<const N: u64> u64::Counted<N>;";
    // A narrow import exposes exactly one same-leaf owner. `a`'s two-index
    // telescope rejects the arity outright — the witness that the leaf
    // selected `a::Counted` rather than declining or borrowing `b`'s.
    let imported_a = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[
        a,
        b,
        "use a::Counted; data Hold { value: u64 in Counted<3>; }",
    ])))
    .expect_err("the import selects `a::Counted`'s telescope");
    assert!(
        imported_a.iter().any(|error| error
            .message
            .contains("requires 2 closed index argument(s)")),
        "{imported_a:?}"
    );
    // Under `use b::Counted` the leaf folds a named index through the
    // requester's own constant — proof `b::Counted`'s telescope owns it.
    let syntax = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[
        a,
        b,
        "use b::Counted; const CAP: u64 = 4; data Hold { value: u64 in Counted<CAP>; }",
    ])))
    .expect("the import selects `b::Counted`'s telescope");
    let (argument, name) = named_domain_argument(&syntax, "Counted");
    assert_eq!(name.as_str(), "4");
    assert!(
        syntax
            .type_references
            .const_argument_normalization(argument)
            .is_some(),
        "the selected module const leaves its normalization record"
    );
    // Unimported, the leaf reaches neither foreign owner: `CAP` stays
    // authored rather than folding against a telescope the source cannot
    // name (selecting `a` would fail arity; `b` would fold to `7`).
    let syntax = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[
        a,
        b,
        "module c; const CAP: u64 = 7; data Hold { value: u64 in Counted<CAP>; }",
    ])))
    .expect("unreachable foreign families leave the application open");
    let (argument, name) = named_domain_argument(&syntax, "Counted");
    assert_eq!(name.as_str(), "CAP");
    assert!(
        syntax
            .type_references
            .const_argument_normalization(argument)
            .is_none()
    );
}

#[test]
fn module_local_indexed_family_outranks_a_same_leaf_root_family() {
    let root_family = "domain<const N: u64> u64::Counted<N>;
        data Root { value: u64 in Counted<7>; }";
    // Inside `c`, `Counted` names `c`'s own two-index family — never the
    // same-leaf root family. The arity error is the witness: against the root
    // telescope `Counted<7>` would fold cleanly.
    let errors = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[
        root_family,
        "module c;
            domain<const N: u64, const M: u64> u64::Counted<N, M>;
            data Hold { value: u64 in Counted<7>; }",
    ])))
    .expect_err("the module's own family owns its application");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("requires 2 closed index argument(s)")),
        "{errors:?}"
    );
    // Each source folds against its own family when the spellings line up.
    normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[
        root_family,
        "module c;
            domain<const N: u64, const M: u64> u64::Counted<N, M>;
            data Hold { value: u64 in Counted<7, 9>; }",
    ])))
    .expect("root and module applications select their own telescopes");
}

#[test]
fn probe_open_template_domain_index_downstream() {
    let source = "domain<const N: u64> u64::Counted<N> requires self < N;
         data Buffer<const N: u64> { value: u64 in Counted<N>; }
         data Main { field: Buffer<3>; }";
    let syntax =
        normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[&source])))
            .expect("open domain index defers binder selection");
    let outcome = crate::resolve(crate::ResolutionRequest::new(&syntax));
    match &outcome {
        Ok(program) => {
            eprintln!("PROBE: resolve OK");
            for definition in &program.data_definitions {
                eprintln!("PROBE: data {}", definition.name.as_str());
            }
            // Find the resolved field type of Buffer<3>.value
            let instance = program
                .data_definitions
                .iter()
                .find(|d| d.name.as_str() == "Buffer<3>")
                .expect("instance");
            let [symbol_resolved_trees::data::DataMember::Field(field)] =
                program.data_members(instance.members)
            else {
                panic!("one field");
            };
            eprintln!("PROBE: field type = {:?}", field.type_reference);
        }
        Err(errors) => {
            for error in errors {
                eprintln!("PROBE: resolve error: {}", error.message);
            }
        }
    }
    // Also inspect the syntax-side domain constraint.
    for constraint in syntax.type_references.domain_constraints() {
        let args: Vec<String> = syntax
            .type_references
            .type_reference_handles(constraint.arguments)
            .iter()
            .map(|a| format!("{:?}", syntax.type_references.type_reference(*a)))
            .collect();
        eprintln!("PROBE: domain constraint {} args {:?}", constraint.name.as_str(), args);
    }

    // Probe: range constraint endpoints naming a const binder.
    let ranged = "data Buffer<const N: u64> { value: u64 [0..N]; }
         data Main { field: Buffer<3>; }";
    match normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[ranged]))) {
        Ok(syntax) => {
            eprintln!("PROBE: ranged normalize OK");
            let outcome = crate::resolve(crate::ResolutionRequest::new(&syntax));
            match &outcome {
                Ok(_) => eprintln!("PROBE: ranged resolve OK"),
                Err(errors) => {
                    for error in errors {
                        eprintln!("PROBE: ranged resolve error: {}", error.message);
                    }
                }
            }
        }
        Err(errors) => {
            for error in &errors {
                eprintln!("PROBE: ranged normalize error: {}", error.message);
            }
        }
    }

    // Probe: ConstExpression domain index argument on a binder.
    let expression_index = "domain<const N: u64> u64::Counted<N> requires self < N;
         data Buffer<const N: u64> { value: u64 in Counted<N + 1>; }
         data Main { field: Buffer<3>; }";
    match normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[expression_index]))) {
        Ok(syntax) => {
            eprintln!("PROBE: expr-index normalize OK");
            for constraint in syntax.type_references.domain_constraints() {
                let args: Vec<String> = syntax
                    .type_references
                    .type_reference_handles(constraint.arguments)
                    .iter()
                    .map(|a| format!("{:?}", syntax.type_references.type_reference(*a)))
                    .collect();
                eprintln!("PROBE: expr-index constraint {} args {:?}", constraint.name.as_str(), args);
            }
            let outcome = crate::resolve(crate::ResolutionRequest::new(&syntax));
            match &outcome {
                Ok(_) => eprintln!("PROBE: expr-index resolve OK"),
                Err(errors) => {
                    for error in errors {
                        eprintln!("PROBE: expr-index resolve error: {}", error.message);
                    }
                }
            }
        }
        Err(errors) => {
            for error in &errors {
                eprintln!("PROBE: expr-index normalize error: {}", error.message);
            }
        }
    }
}

#[test]
fn open_template_domain_indices_keep_their_binder_at_root_and_in_modules() {
    // `Counted<N>` inside an open template defers binder selection; the
    // synthesized instance inherits the authored constraint verbatim at both
    // scopes. Instance-side domain-constraint replay is a later stage's job.
    for prefix in ["", "module buffers; "] {
        let source = format!(
            "{prefix}domain<const N: u64> u64::Counted<N> requires self < N;
             data Buffer<const N: u64> {{ value: u64 in Counted<N>; }}
             data Main {{ field: Buffer<3>; }}"
        );
        let syntax =
            normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[&source])))
                .expect("open domain index defers binder selection");
        // The instance shares the template's constrained type node verbatim:
        // both fields carry the same `Counted<N>` constraint handle.
        let field_types: Vec<TypeReferenceHandle> = ["Buffer", "Buffer<3>"]
            .into_iter()
            .map(|expected| {
                let definition = syntax
                    .root_items()
                    .find_map(|item| match item {
                        Item::Data(definition) if definition.name.as_str() == expected => {
                            Some(definition)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("missing {expected}"));
                let [DataMember::Field(field)] = syntax.items.data_members(definition.members)
                else {
                    panic!("one field on {expected}");
                };
                field.type_reference
            })
            .collect();
        assert_eq!(
            field_types[0], field_types[1],
            "domain-constrained field type is shared verbatim: prefix {prefix:?}"
        );
        for constraint in syntax.tables.type_references.domain_constraints() {
            assert_eq!(constraint.name.as_str(), "Counted");
            let [argument] = syntax
                .tables
                .type_references
                .type_reference_handles(constraint.arguments)
            else {
                panic!("one index argument");
            };
            assert!(
                matches!(
                    syntax.type_references.type_reference(*argument),
                    TypeReferenceNode::Named(name) if name.as_str() == "N"
                ),
                "the authored binder rides verbatim: prefix {prefix:?}"
            );
        }
    }
}
