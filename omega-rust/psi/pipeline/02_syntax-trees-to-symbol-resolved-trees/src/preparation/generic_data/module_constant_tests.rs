use crate::pre_resolution::GenericDataRequest;
use crate::pre_resolution::normalize_generic_data;
use crate::preparation::generic_data::desugar_generic_data_instances;
use crate::preparation::generic_data::validate_direct_const_arguments;
use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::DataMember;
use syntax_trees::item::Item;
use syntax_trees::types::TypeConstraintNode;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

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
        tokens_to_syntax_trees::parse(&mut syntax, source_id, &tokens)
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
        tokens_to_syntax_trees::parse(&mut syntax, SourceId(index + 1), &tokens)
            .expect("parse sources");
    }
    syntax
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

fn field_type_reference(syntax: &SyntaxTrees, data_name: &str) -> TypeReferenceHandle {
    let definition = syntax
        .root_items()
        .find_map(|item| match item {
            Item::Data(definition) if definition.name.as_str() == data_name => Some(definition),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {data_name}"));
    let [DataMember::Field(field)] = syntax.items.data_members(definition.members) else {
        panic!("one field on {data_name}");
    };
    field.type_reference
}

fn domain_index_arguments(
    syntax: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
) -> &[TypeReferenceHandle] {
    let TypeReferenceNode::Constrained { constraints, .. } =
        syntax.type_references.type_reference(type_reference)
    else {
        panic!("a constrained field type");
    };
    let [TypeConstraintNode::Domain(domain)] = syntax.type_references.constraints(*constraints)
    else {
        panic!("one domain constraint");
    };
    syntax
        .type_references
        .type_reference_handles(domain.arguments)
}

#[test]
fn open_template_domain_index_substitutes_on_the_closed_instance() {
    // `Counted<N>` keeps its binder on the open template; the synthesized
    // `Buffer<3>` carries `Counted<3>` -- the index leaves the template's
    // telescope with the field, at root and inside a module alike. Resolution
    // then sees only closed indices on the instance.
    for prefix in ["", "module buffers; "] {
        let source = format!(
            "{prefix}domain<const N: u64> u64::Counted<N> requires self < N;
             data Buffer<const N: u64> {{ value: u64 in Counted<N>; }}
             data Main {{ field: Buffer<3>; }}"
        );
        let syntax =
            normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[&source])))
                .expect("open domain index defers binder selection");
        let template = field_type_reference(&syntax, "Buffer");
        let instance = field_type_reference(&syntax, "Buffer<3>");
        assert_ne!(
            template, instance,
            "the instance rebuilds the constrained node: prefix {prefix:?}"
        );
        let [template_argument] = domain_index_arguments(&syntax, template) else {
            panic!("one index argument on the template: prefix {prefix:?}");
        };
        assert!(
            matches!(
                syntax.type_references.type_reference(*template_argument),
                TypeReferenceNode::Named(name) if name.as_str() == "N"
            ),
            "the template keeps the authored binder: prefix {prefix:?}"
        );
        let [instance_argument] = domain_index_arguments(&syntax, instance) else {
            panic!("one index argument on the instance: prefix {prefix:?}");
        };
        assert!(
            matches!(
                syntax.type_references.type_reference(*instance_argument),
                TypeReferenceNode::Named(name) if name.as_str() == "3"
            ),
            "the instance carries the closed index: prefix {prefix:?}"
        );
        let program = crate::resolve(crate::ResolutionRequest::new(&syntax))
            .expect("the closed instance resolves");
        let resolved_instance = program
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == "Buffer<3>")
            .expect("resolved instance");
        let [symbol_resolved_trees::data::DataMember::Field(field)] =
            program.data_members(resolved_instance.members)
        else {
            panic!("one resolved field: prefix {prefix:?}");
        };
        let symbol_resolved_trees::types::TypeReference::Constrained(field_type) =
            &field.type_reference
        else {
            panic!("resolved constrained field: prefix {prefix:?}");
        };
        let [symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = program
            .tables
            .types
            .constraints
            .span_or_empty(field_type.constraints)
        else {
            panic!("resolved domain constraint: prefix {prefix:?}");
        };
        assert_eq!(domain.name.as_str(), "Counted");
        let [argument] = program
            .tables
            .declarations
            .child_type_references
            .span_or_empty(domain.arguments)
        else {
            panic!("one resolved index argument: prefix {prefix:?}");
        };
        assert!(
            matches!(
                argument,
                symbol_resolved_trees::types::TypeReference::Named { name, .. }
                    if name.as_str() == "3"
            ),
            "the resolved instance carries the closed index: prefix {prefix:?}"
        );
    }
}

#[test]
fn open_template_domain_expression_index_keeps_the_authored_operator_on_the_instance() {
    // `Counted<N + 1>` cannot fold during synthesis: `+` is an authored
    // operator whose exact overload is selected later. The instance receives a
    // copied `3 + 1` subtree -- the binder leaf rewritten to the closed
    // argument, the operator spelling intact -- while the template keeps
    // `N + 1` untouched.
    let source = "domain<const N: u64> u64::Counted<N> requires self < N;
         data Buffer<const N: u64> { value: u64 in Counted<N + 1>; }
         data Main { field: Buffer<3>; }";
    let syntax = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect("open expression index defers binder selection");
    let template = field_type_reference(&syntax, "Buffer");
    let instance = field_type_reference(&syntax, "Buffer<3>");
    let [template_argument] = domain_index_arguments(&syntax, template) else {
        panic!("one index argument on the template");
    };
    let [instance_argument] = domain_index_arguments(&syntax, instance) else {
        panic!("one index argument on the instance");
    };
    assert_ne!(
        *template_argument, *instance_argument,
        "the open index copies into the instance rather than sharing the node"
    );
    let TypeReferenceNode::ConstExpression(template_expression) =
        syntax.type_references.type_reference(*template_argument)
    else {
        panic!("the template keeps its authored const expression");
    };
    let ExpressionNode::Binary(template_binary) =
        syntax.expressions.expression(*template_expression)
    else {
        panic!("the template index stays `N + 1`");
    };
    assert!(
        matches!(
            syntax.expressions.expression(template_binary.left),
            ExpressionNode::Name(_)
        ),
        "the template's binder leaf is untouched"
    );
    let TypeReferenceNode::ConstExpression(instance_expression) =
        syntax.type_references.type_reference(*instance_argument)
    else {
        panic!("the instance keeps the authored operator spelling");
    };
    let ExpressionNode::Binary(instance_binary) =
        syntax.expressions.expression(*instance_expression)
    else {
        panic!("the instance index stays an authored `+` application");
    };
    assert_eq!(
        instance_binary.operator,
        syntax_trees::expression::BinaryOperator::Add,
        "the copied subtree preserves the authored operator"
    );
    assert!(
        matches!(
            syntax.expressions.expression(instance_binary.left),
            ExpressionNode::Integer(literal) if literal.value_u64() == Some(3)
        ),
        "the instance's binder leaf rewrote to the closed argument"
    );
    assert!(
        matches!(
            syntax.expressions.expression(instance_binary.right),
            ExpressionNode::Integer(literal) if literal.value_u64() == Some(1)
        ),
        "the authored right operand is preserved"
    );
}

#[test]
fn open_template_range_endpoint_substitutes_on_the_closed_instance() {
    // `[0..N]` copies its binder endpoint onto `Buffer<3>` so the instance's
    // bound is the closed `0..3`; the template keeps `0..N` (the non-constant
    // template bound is still rejected downstream, deliberately).
    let source = "data Buffer<const N: u64> { value: u64 [0..N]; }
         data Main { field: Buffer<3>; }";
    let syntax = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect("open range endpoint defers binder selection");
    let template = field_type_reference(&syntax, "Buffer");
    let instance = field_type_reference(&syntax, "Buffer<3>");
    let range_maximum = |syntax: &SyntaxTrees, type_reference: TypeReferenceHandle| {
        let TypeReferenceNode::Constrained { constraints, .. } =
            syntax.type_references.type_reference(type_reference)
        else {
            panic!("a constrained field type");
        };
        let [TypeConstraintNode::Range { maximum, .. }] =
            syntax.type_references.constraints(*constraints)
        else {
            panic!("one range constraint");
        };
        *maximum
    };
    assert!(
        matches!(
            syntax
                .expressions
                .expression(range_maximum(&syntax, template)),
            ExpressionNode::Name(_)
        ),
        "the template keeps the open `N` bound"
    );
    assert!(
        matches!(
            syntax
                .expressions
                .expression(range_maximum(&syntax, instance)),
            ExpressionNode::Integer(literal) if literal.value_u64() == Some(3)
        ),
        "the instance's bound rewrote to the closed `3`"
    );
    crate::resolve(crate::ResolutionRequest::new(&syntax))
        .expect("the closed range endpoint resolves");
}

#[test]
fn bounded_carrier_domain_discharges_carrier_independent_facts() {
    // The bound checks against the concrete carrier while `self` stays
    // unbound; a closed fact that never reads `self` still replays.
    let source =
        "module mine; domain<T [copy]> T::Small requires 7 > 0; const X: u64 in u64::Small = 7;";
    normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect("satisfied bound plus carrier-independent fact discharges");
}

#[test]
fn bounded_carrier_domain_discharges_satisfied_carry_bound() {
    // A builtin scalar rides the permissive carry policy, so any authored
    // `carry(...)` bound holds of it.
    let source = "module mine; domain<T [carry(suspension: allowed, cpu: any, thread: any, address: movable)]> T::Anywhere; const X: u64 in u64::Anywhere = 7;";
    normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect("a satisfied carry bound discharges the marker domain");
}

#[test]
fn bounded_carrier_domain_keeps_an_unsatisfied_linear_bound_fenced() {
    // `u64` is not linear, so the bound's application evidence cannot be
    // proven and the constraint keeps the declaration-site fence.
    let source = "module mine; domain<T [linear]> T::Marked; const X: u64 in u64::Marked = 7;";
    let errors = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect_err("an unsatisfiable carrier bound keeps the fence");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("declaration-site proof checking")),
        "{errors:?}"
    );
}

#[test]
fn compiler_carry_atom_in_alias_names_its_evidence_class() {
    // `Carry::*` atoms are grant evidence, not domains: instead of bouncing
    // off domain selection the constraint names what a const cannot supply.
    let source = "module mine; domain u64::Pos requires self > 0; domain u64::Alias = u64::Pos & Carry::Portable; const X: u64 in u64::Alias = 3;";
    let errors = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect_err("a const cannot supply carry-grant evidence");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("compiler carry permission")),
        "{errors:?}"
    );
}

#[test]
fn compiler_carry_atom_direct_constraint_names_its_evidence_class() {
    let source = "module mine; const X: u64 in Carry::AnyCpu = 3;";
    let errors = normalize_generic_data(GenericDataRequest::new(parse_multiple_sources(&[source])))
        .expect_err("a const cannot supply carry-grant evidence");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("compiler carry permission")),
        "{errors:?}"
    );
}
