use super::*;
use source::SourceId;
use source_files_to_tokens::Lexer;
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
        let syntax = normalize_generic_data(parse_sources(root, module, module_first))
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

            let syntax =
                normalize_generic_data(syntax).expect("generic templates defer binder selection");

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
    let syntax = normalize_generic_data(parse_sources(source, "", false))
        .expect("direct structural values normalize");
    crate::lowerer::lower_syntax_trees(&syntax).expect("actual direct arguments lower");
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
    let errors =
        crate::lowerer::lower_syntax_trees(&changed).expect_err("same-span value drift rejects");
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
