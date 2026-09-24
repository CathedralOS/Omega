use crate::lowerer::seeded_continuation::{
    lower_seeded_extension, plain_data_extension_shape_is_supported,
};
use crate::lowerer::tests::seeded_normalized_plain_data_inputs;
use crate::lowerer::{exact_field_symbol, exact_top_level_data_symbol};

#[test]
fn seeded_local_instance_gate_rejects_origin_and_declaration_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Cell<T> { value: T; } data Generated { first: Cell<u32>; second: Cell<u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));
    let instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].generic_instance.is_some())
        .expect("one instance index");
    let template_symbol = match resolved.data_definitions[instance_index]
        .generic_instance
        .as_ref()
    {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.base_symbol,
        _ => unreachable!(),
    };
    let template_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].symbol == template_symbol)
        .expect("one template index");
    let wrapper_index = (frontier..resolved.data_definitions.len())
        .find(|index| *index != instance_index && *index != template_index)
        .expect("one wrapper index");
    let template = &resolved.data_definitions[template_index];
    let instance = &resolved.data_definitions[instance_index];
    let wrapper = &resolved.data_definitions[wrapper_index];
    let instance_members = instance.members;
    let wrapper_members = wrapper.members;
    assert!(exact_top_level_data_symbol(&resolved, template));
    assert!(exact_top_level_data_symbol(&resolved, instance));
    assert!(exact_top_level_data_symbol(&resolved, wrapper));
    let [symbol_resolved_trees::data::DataMember::Field(template_field)] =
        resolved.data_members(template.members)
    else {
        panic!("one template field")
    };
    let [symbol_resolved_trees::data::DataMember::Field(instance_field)] =
        resolved.data_members(instance.members)
    else {
        panic!("one instance field")
    };
    let [
        symbol_resolved_trees::data::DataMember::Field(first_wrapper_field),
        symbol_resolved_trees::data::DataMember::Field(second_wrapper_field),
    ] = resolved.data_members(wrapper.members)
    else {
        panic!("two wrapper fields")
    };
    assert_ne!(template_field.symbol, instance_field.symbol);
    assert!(exact_field_symbol(
        &resolved,
        template.symbol,
        template_field
    ));
    assert!(exact_field_symbol(
        &resolved,
        instance.symbol,
        instance_field
    ));
    assert!(exact_field_symbol(
        &resolved,
        wrapper.symbol,
        first_wrapper_field
    ));
    assert!(exact_field_symbol(
        &resolved,
        wrapper.symbol,
        second_wrapper_field
    ));
    assert!(
        !exact_field_symbol(&resolved, wrapper.symbol, template_field),
        "a coordinated field-row retarget must not erase its exact parent"
    );

    let mut wrong_origin = resolved.clone();
    let Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) = wrong_origin
        .data_definitions[instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.base_name = symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_origin,
        frontier
    ));

    let mut missing_argument = resolved.clone();
    let Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) = missing_argument
        .data_definitions[instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.arguments = arena::HandleSpan::empty();
    assert!(!plain_data_extension_shape_is_supported(
        &missing_argument,
        frontier
    ));

    let mut wrong_instance_identity = resolved.clone();
    wrong_instance_identity.data_definitions[instance_index].name =
        symbol_resolved_trees::name::DiagnosticName::generated("Cell<u64>");
    assert!(!plain_data_extension_shape_is_supported(
        &wrong_instance_identity,
        frontier
    ));

    let mut wrong_retired_identity = resolved.clone();
    wrong_retired_identity.data_definitions[instance_index]
        .retired_identities
        .push(71);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_retired_identity, frontier),
        "the instance must retain the template's exact retired-identity set"
    );

    let mut wrong_substitution_name = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(instance_field) = wrong_substitution_name
        .tables
        .declarations
        .data_members
        .get_mut(instance_members.start())
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Named { name, .. } =
        &mut instance_field.type_reference
    else {
        unreachable!()
    };
    *name = symbol_resolved_trees::name::DiagnosticName::generated("u64");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_substitution_name, frontier),
        "the substituted builtin spelling must remain joined to its symbol"
    );

    let mut wrong_wrapper_type_name = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(wrapper_field) = wrong_wrapper_type_name
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Named { name, .. } =
        &mut wrapper_field.type_reference
    else {
        unreachable!()
    };
    *name = symbol_resolved_trees::name::DiagnosticName::generated("Cell<u64>");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_wrapper_type_name, frontier),
        "a wrapper use's diagnostic name cannot drift from the selected instance"
    );

    let authored_symbol = resolved.data_definitions[0].symbol;
    let mut wrong_parent = resolved.clone();
    let template_span = wrong_parent.data_definitions[template_index]
        .name
        .source_span();
    let instance_span = wrong_parent.data_definitions[instance_index]
        .name
        .source_span();
    wrong_parent.data_definitions[template_index].symbol = authored_symbol;
    wrong_parent.data_definitions[template_index].name =
        symbol_resolved_trees::name::DiagnosticName::new("Authored", template_span);
    wrong_parent.data_definitions[instance_index].name =
        symbol_resolved_trees::name::DiagnosticName::new("Authored<u32>", instance_span);
    let Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) = wrong_parent
        .data_definitions[instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.base_symbol = authored_symbol;
    origin.base_name = symbol_resolved_trees::name::DiagnosticName::generated("Authored");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_parent, frontier),
        "coordinated top-level identity retarget cannot detach parameter/field children"
    );

    let mut wrong_kind = resolved.clone();
    wrong_kind.data_definitions[wrapper_index].symbol = template_field.symbol;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_kind, frontier),
        "a Field symbol cannot impersonate the wrapper Data declaration"
    );

    let mut wrong_field_parent = resolved.clone();
    wrong_field_parent.data_definitions[wrapper_index].members =
        wrong_field_parent.data_definitions[template_index].members;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_field_parent, frontier),
        "a coordinated member-span retarget cannot reuse another owner's fields"
    );

    let mut lifetime_template = resolved.clone();
    lifetime_template.data_definitions[template_index]
        .lifetime_parameters
        .push(symbol_resolved_trees::name::DiagnosticName::generated(
            "scope",
        ));
    assert!(!plain_data_extension_shape_is_supported(
        &lifetime_template,
        frontier
    ));

    let mut fact_instance = resolved.clone();
    fact_instance.data_definitions[instance_index].where_facts =
        arena::HandleSpan::from_parts(arena::Handle::from_arena_index(1), 1);
    assert!(!plain_data_extension_shape_is_supported(
        &fact_instance,
        frontier
    ));

    let mut quotient_wrapper = resolved.clone();
    quotient_wrapper.data_definitions[wrapper_index].quotient =
        Some(symbol_resolved_trees::data::QuotientDefinition {
            carrier: symbol_resolved_trees::types::TypeReference::Unit,
            relation: Vec::new(),
            relation_symbol: symbols::SymbolHandle::invalid(),
            equivalence: None,
        });
    assert!(!plain_data_extension_shape_is_supported(
        &quotient_wrapper,
        frontier
    ));

    let mut zero_gated_instance = resolved;
    zero_gated_instance.data_definitions[instance_index].zero_gated = true;
    assert!(!plain_data_extension_shape_is_supported(
        &zero_gated_instance,
        frontier
    ));
}

/// CASE-CONSTRAINTS generic case-data synthesis: a case `where` fact that
/// names no unsubstituted parameter rides each synthesized instance. The
/// seeded-local replay re-derives the pairing independently: an instance whose
/// carried fact was rewritten or dropped must not validate.
#[test]
fn seeded_local_instance_replays_carried_case_where_facts() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        r#"
            data Window<T> {
                marker: T;
                case Empty;
                case Range(lo: u64, hi: u64) where lo <= hi;
            }
            data Generated { window: Window<i32>; }
        "#,
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(
        plain_data_extension_shape_is_supported(&resolved, frontier),
        "a carried case where-fact replays structurally against the template",
    );

    let instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| resolved.data_definitions[*index].generic_instance.is_some())
        .expect("one instance index");
    let instance_members = resolved.data_definitions[instance_index].members;
    let range = resolved
        .data_members(instance_members)
        .iter()
        .find_map(|member| match member {
            symbol_resolved_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Range" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("instance Range case");
    let [symbol_resolved_trees::domain::ProofFact::Expression(fact_expression)] =
        resolved.proof_facts(range.where_facts)
    else {
        panic!("the instance carries one expression case fact")
    };
    let symbol_resolved_trees::expression::ExpressionNode::Binary(binary) = resolved
        .tables
        .bodies
        .expressions
        .expression(*fact_expression)
    else {
        panic!("the carried fact is a binary bound")
    };
    assert_eq!(
        binary.operator,
        symbol_resolved_trees::expression::BinaryOperator::LessOrEqual
    );

    // A rewritten carried fact (`lo >= hi`) is not the template's fact: the
    // replay rejects the pair rather than trusting the producer's copy.
    let mut corrupted = resolved.clone();
    let symbol_resolved_trees::expression::ExpressionNode::Binary(binary) = corrupted
        .tables
        .bodies
        .expressions
        .expression_mut(*fact_expression)
    else {
        unreachable!()
    };
    binary.operator = symbol_resolved_trees::expression::BinaryOperator::GreaterOrEqual;
    assert!(
        !plain_data_extension_shape_is_supported(&corrupted, frontier),
        "a rewritten carried fact must fail the template/instance replay"
    );

    // A dropped carried fact is equally visible to the pairing. The instance's
    // member order is `marker`, `Empty`, `Range`, so `Range` is the third row.
    let mut dropped = resolved;
    let variant_handle = arena::Handle::from_parts(
        instance_members
            .start()
            .arena_index()
            .checked_add(2)
            .expect("member handle overflow"),
        instance_members.start().generation(),
    );
    let symbol_resolved_trees::data::DataMember::Variant(variant) = dropped
        .tables
        .declarations
        .data_members
        .get_mut(variant_handle)
    else {
        unreachable!()
    };
    assert_eq!(variant.name.as_str(), "Range");
    variant.where_facts = arena::HandleSpan::empty();
    assert!(
        !plain_data_extension_shape_is_supported(&dropped, frontier),
        "a dropped carried fact must fail the template/instance replay"
    );
}

/// CASE-CONSTRAINTS generic case-data synthesis: a `T == name` case fact is
/// decided against the closed argument identity at synthesis, so the seeded
/// replay must re-derive the same decision by symbol: `Value<i32>` discharges
/// `T == i32` (no carried fact), `Value<bool>` refutes it and carries the
/// literal `0` witness plus the zero gate it implies. A witness rewritten to
/// a non-false literal, a dropped gate flag, or a fabricated fact each fails
/// the independent check.
#[test]
fn seeded_local_instance_replays_decided_type_equality_facts() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        r#"
            data Value<T> {
                case Integer(value: T) where T == i32;
                case Boolean(value: T) where T == bool;
            }
            data Generated { yes: Value<i32>; no: Value<bool>; }
        "#,
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(
        plain_data_extension_shape_is_supported(&resolved, frontier),
        "decided type-equality facts replay against the template on both instances",
    );

    let instance_index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .expect("instance index")
    };
    let no_index = instance_index("Value<bool>");
    let yes_index = instance_index("Value<i32>");
    assert!(resolved.data_definitions[no_index].zero_gated);
    assert!(!resolved.data_definitions[yes_index].zero_gated);

    // The refuted instance's `Integer` fact must be the `0` witness exactly:
    // a rewritten literal is not the decided-false shape synthesis produces.
    let no_members = resolved.data_definitions[no_index].members;
    let witness = {
        let integer = resolved
            .data_members(no_members)
            .iter()
            .find_map(|member| match member {
                symbol_resolved_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == "Integer" =>
                {
                    Some(variant)
                }
                _ => None,
            })
            .expect("instance Integer case");
        let [symbol_resolved_trees::domain::ProofFact::Expression(fact)] =
            resolved.proof_facts(integer.where_facts)
        else {
            panic!("the refuted instance carries one expression case fact")
        };
        *fact
    };
    let mut corrupted = resolved.clone();
    let symbol_resolved_trees::expression::ExpressionNode::Integer(literal) =
        corrupted.tables.bodies.expressions.expression_mut(witness)
    else {
        unreachable!()
    };
    *literal = numerics::literals::IntegerLiteral::from_parts(
        false,
        numerics::literals::IntegerRadix::Decimal,
        "1",
    )
    .expect("literal `1` is a valid integer literal");
    assert!(
        !plain_data_extension_shape_is_supported(&corrupted, frontier),
        "a refuted witness rewritten to a non-false literal must fail the replay"
    );

    // Clearing the derived gate flag on a first-case-impossible instance is a
    // producer lie: the flag must equal what the replayed facts imply.
    let mut ungated = resolved.clone();
    ungated.data_definitions.for_each_mut(|definition| {
        if definition.name.as_str() == "Value<bool>" {
            definition.zero_gated = false;
        }
    });
    assert!(
        !plain_data_extension_shape_is_supported(&ungated, frontier),
        "a dropped zero gate on an impossible-first-case instance must fail the replay"
    );

    // The discharged instance carries no fact for `Integer`; fabricating one
    // is not a faithful copy of the template's discharged conjunct.
    let mut fabricated = resolved;
    let yes_members = fabricated.data_definitions[yes_index].members;
    let integer_handle = fabricated
        .data_members(yes_members)
        .iter()
        .enumerate()
        .find_map(|(offset, member)| match member {
            symbol_resolved_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Integer" =>
            {
                Some(offset)
            }
            _ => None,
        })
        .expect("instance Integer case offset");
    let variant_handle = arena::Handle::from_parts(
        yes_members
            .start()
            .arena_index()
            .checked_add(integer_handle as u32)
            .expect("member handle overflow"),
        yes_members.start().generation(),
    );
    let symbol_resolved_trees::data::DataMember::Variant(variant) = fabricated
        .tables
        .declarations
        .data_members
        .get_mut(variant_handle)
    else {
        unreachable!()
    };
    let mut facts = arena::HandleSpan::empty();
    facts.push_contiguous(fabricated.tables.declarations.proof_facts.insert(
        symbol_resolved_trees::domain::ProofFact::Expression(witness),
    ));
    variant.where_facts = facts;
    assert!(
        !plain_data_extension_shape_is_supported(&fabricated, frontier),
        "a fabricated fact on a discharged case must fail the replay"
    );
}

#[test]
fn seeded_nested_local_instance_gate_rejects_dependency_and_reachability_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Cell<T> { values: [T; 2]; } data Outer<T> { inner: Cell<T>; direct: T; } data Generated { value: Outer<u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let cell_instance_index = index("Cell<u32>");
    let outer_template_index = index("Outer");
    let outer_instance_index = index("Outer<u32>");
    let wrapper_index = index("Generated");

    let mut wrong_nested_member = resolved.clone();
    let outer_members = wrong_nested_member.data_definitions[outer_instance_index].members;
    let symbol_resolved_trees::data::DataMember::Field(inner) = wrong_nested_member
        .tables
        .declarations
        .data_members
        .get_mut(outer_members.start())
    else {
        unreachable!()
    };
    inner.type_reference = symbol_resolved_trees::types::TypeReference::Unit;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_nested_member, frontier),
        "a nested synthesized field must replay the exact inner instance"
    );

    let mut wrong_template_application = resolved.clone();
    let outer_template_members =
        wrong_template_application.data_definitions[outer_template_index].members;
    let symbol_resolved_trees::data::DataMember::Field(inner) = wrong_template_application
        .tables
        .declarations
        .data_members
        .get_mut(outer_template_members.start())
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut inner.type_reference
    else {
        unreachable!()
    };
    application.base_name = symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_template_application, frontier),
        "a local template application cannot drift from its exact base symbol"
    );

    let mut wrong_inner_origin = resolved.clone();
    let Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) = wrong_inner_origin
        .data_definitions[cell_instance_index]
        .generic_instance
        .as_mut()
    else {
        unreachable!()
    };
    origin.base_name = symbol_resolved_trees::name::DiagnosticName::generated("Other");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_inner_origin, frontier),
        "a transitive dependency must retain its exact synthesis origin"
    );

    let mut unreachable_instances = resolved;
    let wrapper_members = unreachable_instances.data_definitions[wrapper_index].members;
    let symbol_resolved_trees::data::DataMember::Field(value) = unreachable_instances
        .tables
        .declarations
        .data_members
        .get_mut(wrapper_members.start())
    else {
        unreachable!()
    };
    value.type_reference = symbol_resolved_trees::types::TypeReference::Unit;
    assert!(
        !plain_data_extension_shape_is_supported(&unreachable_instances, frontier),
        "an internally coherent but unreachable synthesized subgraph is not admitted"
    );
}

#[test]
fn seeded_local_sum_instance_gate_rejects_case_and_payload_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Maybe<T> { case #1 None; case #2 Some(#1 value: T, retired #3); retired #4; } data Generated { value: Maybe<u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let template_index = index("Maybe");
    let instance_index = index("Maybe<u32>");
    let template_members = resolved.data_definitions[template_index].members;
    let instance_members = resolved.data_definitions[instance_index].members;
    let template_some = match &resolved.data_members(template_members)[1] {
        symbol_resolved_trees::data::DataMember::Variant(variant) => variant,
        _ => panic!("Maybe::Some template case"),
    };
    let instance_some = match &resolved.data_members(instance_members)[1] {
        symbol_resolved_trees::data::DataMember::Variant(variant) => variant,
        _ => panic!("Maybe<u32>::Some instance case"),
    };
    assert_ne!(template_some.symbol, instance_some.symbol);
    assert_eq!(template_some.identity, instance_some.identity);
    assert_eq!(
        template_some.retired_payload_identities,
        instance_some.retired_payload_identities
    );

    let mut reordered_cases = resolved.clone();
    reordered_cases
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(instance_members)
        .swap(0, 1);
    assert!(
        !plain_data_extension_shape_is_supported(&reordered_cases, frontier),
        "case declaration order is part of the synthesized instance"
    );

    let mut wrong_case_parent = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_case_parent
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.symbol = template_some.symbol;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_case_parent, frontier),
        "a synthesized case cannot reuse the template case symbol"
    );

    let mut wrong_case_identity = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_case_identity
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.identity = Some(71);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_case_identity, frontier),
        "case identity must replay exactly"
    );

    let mut wrong_retired_payload = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_retired_payload
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.retired_payload_identities.push(72);
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_retired_payload, frontier),
        "retired payload identities must replay exactly"
    );

    let mut wrong_payload_substitution = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &wrong_payload_substitution.data_members(instance_members)[1]
    else {
        unreachable!()
    };
    let payload = instance_some_mut.payload;
    wrong_payload_substitution
        .tables
        .declarations
        .data_payload_fields
        .span_mut_or_empty(payload)[0]
        .type_reference = symbol_resolved_trees::types::TypeReference::Unit;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_payload_substitution, frontier),
        "payload substitution must replay the exact type argument"
    );

    let mut wrong_payload_parent = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Variant(instance_some_mut) =
        &mut wrong_payload_parent
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[1]
    else {
        unreachable!()
    };
    instance_some_mut.payload = template_some.payload;
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_payload_parent, frontier),
        "a synthesized case cannot reuse template-owned payload fields"
    );
}

#[test]
fn seeded_lifetime_instance_gate_rejects_binder_and_application_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'scope, T> { value: &'scope T; } data Generated<'scope> { value: Borrowed<'scope, u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let instance_index = index("Borrowed<u32>");
    let wrapper_index = index("Generated");
    let instance_members = resolved.data_definitions[instance_index].members;
    let wrapper_members = resolved.data_definitions[wrapper_index].members;

    let mut wrong_instance_binder = resolved.clone();
    wrong_instance_binder.data_definitions[instance_index].lifetime_parameters[0] =
        symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_instance_binder, frontier),
        "the instance must retain the template's exact erased lifetime binder"
    );

    let mut missing_application_lifetime = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(wrapper_field) =
        &mut missing_application_lifetime
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(wrapper_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut wrapper_field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments.clear();
    assert!(
        !plain_data_extension_shape_is_supported(&missing_application_lifetime, frontier),
        "the selected local instance requires its complete erased lifetime arity"
    );

    let mut unknown_application_lifetime = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(wrapper_field) =
        &mut unknown_application_lifetime
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(wrapper_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut wrapper_field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments[0] =
        symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !plain_data_extension_shape_is_supported(&unknown_application_lifetime, frontier),
        "a local instance application can name only an owning lifetime binder"
    );

    let mut wrong_reference_lifetime = resolved;
    let symbol_resolved_trees::data::DataMember::Field(instance_field) =
        &mut wrong_reference_lifetime
            .tables
            .declarations
            .data_members
            .span_mut_or_empty(instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Reference(reference) =
        &mut instance_field.type_reference
    else {
        unreachable!()
    };
    reference.lifetime = Some(symbol_resolved_trees::name::DiagnosticName::generated(
        "other",
    ));
    assert!(
        !plain_data_extension_shape_is_supported(&wrong_reference_lifetime, frontier),
        "the substituted reference must retain the template lifetime exactly"
    );
}

#[test]
fn seeded_nested_lifetime_instance_gate_rejects_internal_edge_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Nested<'outer, 'inner, T> { value: Borrowed<'inner, 'outer, T>; } data Generated<'one, 'two> { value: Nested<'one, 'two, u32>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let index = |name: &str| {
        (frontier..resolved.data_definitions.len())
            .find(|index| resolved.data_definitions[*index].name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let nested_template_index = index("Nested");
    let nested_instance_index = index("Nested<u32>");
    let borrowed_instance_index = index("Borrowed<u32>");
    let nested_template_members = resolved.data_definitions[nested_template_index].members;
    let nested_instance_members = resolved.data_definitions[nested_instance_index].members;
    let borrowed_instance_symbol = resolved.data_definitions[borrowed_instance_index].symbol;
    let nested_instance_symbol = resolved.data_definitions[nested_instance_index].symbol;
    let nested_instance_name = resolved.data_definitions[nested_instance_index]
        .name
        .clone();

    let mut unknown_template_lifetime = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut unknown_template_lifetime
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_template_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments[0] =
        symbol_resolved_trees::name::DiagnosticName::generated("other");
    assert!(
        !plain_data_extension_shape_is_supported(&unknown_template_lifetime, frontier),
        "a nested template application can name only an owning lifetime binder"
    );

    let mut missing_instance_lifetime = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut missing_instance_lifetime
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments.clear();
    assert!(
        !plain_data_extension_shape_is_supported(&missing_instance_lifetime, frontier),
        "the synthesized nested edge must retain its exact lifetime arity"
    );

    let mut reordered_instance_lifetimes = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut reordered_instance_lifetimes
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    application.lifetime_arguments.swap(0, 1);
    assert!(
        !plain_data_extension_shape_is_supported(&reordered_instance_lifetimes, frontier),
        "the synthesized nested edge must retain exact lifetime argument order"
    );

    let mut redirected_instance = resolved;
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut redirected_instance
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(nested_instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) =
        &mut field.type_reference
    else {
        unreachable!()
    };
    assert_eq!(application.base_symbol, borrowed_instance_symbol);
    application.base_symbol = nested_instance_symbol;
    application.base_name = nested_instance_name;
    assert!(
        !plain_data_extension_shape_is_supported(&redirected_instance, frontier),
        "a nested lifetime edge cannot be redirected to another validated instance"
    );
}

#[test]
fn seeded_lifetime_type_argument_gate_rejects_origin_routing_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Holder<'first, 'second, T> { value: T; } data Generated<'one, 'two> { value: Holder<'one, 'two, Borrowed<'one, 'two, u32>>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let holder_instance_index = (frontier..resolved.data_definitions.len())
        .find(|index| {
            matches!(
                resolved.data_definitions[*index].generic_instance.as_ref(),
                Some(symbol_resolved_trees::types::TypeReference::Generic(origin))
                    if origin.base_name.as_str() == "Holder"
            )
        })
        .expect("closed Holder instance");
    let holder_instance_symbol = resolved.data_definitions[holder_instance_index].symbol;
    let holder_instance_name = resolved.data_definitions[holder_instance_index]
        .name
        .clone();
    let origin_arguments = match resolved.data_definitions[holder_instance_index]
        .generic_instance
        .as_ref()
    {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };

    let mut reordered_lifetimes = resolved.clone();
    let symbol_resolved_trees::types::TypeReference::Generic(argument) = &mut reordered_lifetimes
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    argument.lifetime_arguments.swap(0, 1);
    assert!(
        !plain_data_extension_shape_is_supported(&reordered_lifetimes, frontier),
        "a lifetime-bearing Type argument must forward the owner's exact binder order"
    );

    let mut missing_lifetime = resolved.clone();
    let symbol_resolved_trees::types::TypeReference::Generic(argument) = &mut missing_lifetime
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    argument.lifetime_arguments.pop();
    assert!(
        !plain_data_extension_shape_is_supported(&missing_lifetime, frontier),
        "a lifetime-bearing Type argument must retain complete erased arity"
    );

    let mut redirected_argument = resolved;
    let symbol_resolved_trees::types::TypeReference::Generic(argument) = &mut redirected_argument
        .tables
        .declarations
        .child_type_references
        .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    argument.base_symbol = holder_instance_symbol;
    argument.base_name = holder_instance_name;
    assert!(
        !plain_data_extension_shape_is_supported(&redirected_argument, frontier),
        "the Type argument cannot redirect to another local lifetime instance"
    );

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Borrowed<'left, 'right, T> { left: &'left T; right: &'right T; } data Holder<'first, 'second, T> { value: T; } data Generated<'one, 'two> { value: Holder<'one, 'two, Borrowed<'two, 'one, u32>>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "a permuted nested lifetime route remains rejected until it has distinct identity"
    );
}

#[test]
fn seeded_arithmetic_domain_argument_gate_rejects_identity_mutations() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; }",
        "data Cell<T> { value: T; } data Generated { value: Cell<u32 in Wrapping>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let instance = resolved
        .data_definitions
        .iter()
        .skip(frontier)
        .find(|definition| definition.name.as_str() == "Cell<u32 in Wrapping>")
        .expect("closed constrained Cell instance");
    let origin_arguments = match instance.generic_instance.as_ref() {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };
    let instance_members = instance.members;

    let mut changed_origin_domain = resolved.clone();
    let symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &mut changed_origin_domain
            .tables
            .declarations
            .child_type_references
            .span_mut_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let [constraint] = changed_origin_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(argument.constraints)
    else {
        unreachable!()
    };
    *constraint = symbol_resolved_trees::types::TypeConstraint::ArithmeticDomain(
        numerics::arithmetic::ArithmeticDomain::Saturating,
    );
    assert!(
        !plain_data_extension_shape_is_supported(&changed_origin_domain, frontier),
        "the arithmetic-domain argument participates in canonical instance identity"
    );

    let mut changed_field_domain = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(field) = &mut changed_field_domain
        .tables
        .declarations
        .data_members
        .span_mut_or_empty(instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Constrained(field_type) =
        &field.type_reference
    else {
        unreachable!()
    };
    let field_constraints = field_type.constraints;
    let [constraint] = changed_field_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(field_constraints)
    else {
        unreachable!()
    };
    *constraint = symbol_resolved_trees::types::TypeConstraint::ArithmeticDomain(
        numerics::arithmetic::ArithmeticDomain::Saturating,
    );
    assert!(
        !plain_data_extension_shape_is_supported(&changed_field_domain, frontier),
        "the substituted field must retain the origin's exact arithmetic domain"
    );

    let mut changed_base_identity = resolved.clone();
    let symbol_resolved_trees::types::TypeReference::Constrained(argument) = &changed_base_identity
        .tables
        .declarations
        .child_type_references
        .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let base_type = argument.base_type;
    let symbol_resolved_trees::types::TypeReference::Named { name, .. } = changed_base_identity
        .tables
        .declarations
        .child_type_references
        .get_mut(base_type)
    else {
        unreachable!()
    };
    *name = symbol_resolved_trees::name::DiagnosticName::generated("u64");
    assert!(
        !plain_data_extension_shape_is_supported(&changed_base_identity, frontier),
        "a constrained argument cannot spoof its exact carrier identity"
    );

    let mut changed_constraint_kind = resolved;
    let symbol_resolved_trees::types::TypeReference::Constrained(argument) =
        &changed_constraint_kind
            .tables
            .declarations
            .child_type_references
            .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let constraints = argument.constraints;
    let [constraint] = changed_constraint_kind
        .tables
        .types
        .constraints
        .span_mut_or_empty(constraints)
    else {
        unreachable!()
    };
    *constraint = symbol_resolved_trees::types::TypeConstraint::Named(
        symbol_resolved_trees::name::DiagnosticName::generated("Wrapping"),
    );
    assert!(
        !plain_data_extension_shape_is_supported(&changed_constraint_kind, frontier),
        "a spelling-identical named constraint cannot replace the arithmetic-domain tag"
    );
}

#[test]
fn seeded_primitive_declared_domain_argument_rejoins_exact_identity() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; } domain u8::Issued;",
        "data Cell<T> { value: T; } data Generated { value: Cell<u8 in Issued>; }",
    );
    assert!(plain_data_extension_shape_is_supported(
        extension.trees(),
        base.typed().data_definitions().len()
    ));
    let typed = lower_seeded_extension(extension, base)
        .expect("the exact primitive domain instance keeps the existing continuation");
    assert!(
        typed
            .data_definitions()
            .iter()
            .any(|data| data.name.as_str() == "Cell<u8 in Issued>"
                && data.generic_instance.is_some())
    );
}

#[test]
fn seeded_unindexed_declared_domain_argument_rejoins_exact_identity() {
    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; } data Token { value: u8; } domain Token::Issued; domain Token::Other;",
        "data Cell<T> { value: T; } data Generated { value: Cell<Token in Issued>; }",
    );
    let frontier = base.typed().data_definitions().len();
    let resolved = extension.trees().clone();
    assert!(plain_data_extension_shape_is_supported(&resolved, frontier));

    let instance = resolved
        .data_definitions
        .iter()
        .skip(frontier)
        .find(|definition| definition.name.as_str() == "Cell<Token in Issued>")
        .expect("closed declared-domain Cell instance");
    let origin_arguments = match instance.generic_instance.as_ref() {
        Some(symbol_resolved_trees::types::TypeReference::Generic(origin)) => origin.arguments,
        _ => unreachable!(),
    };
    let instance_members = instance.members;
    let issued_symbol = resolved
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Issued")
        .expect("Issued domain declaration")
        .symbol;
    let other_name = resolved
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Token::Other")
        .expect("Other domain declaration")
        .name
        .clone();

    let mut changed_origin_domain = resolved.clone();
    let symbol_resolved_trees::types::TypeReference::Constrained(argument) = &changed_origin_domain
        .tables
        .declarations
        .child_type_references
        .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let constraints = argument.constraints;
    let [symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = changed_origin_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(constraints)
    else {
        unreachable!()
    };
    domain.name = other_name.clone();
    assert!(
        !plain_data_extension_shape_is_supported(&changed_origin_domain, frontier),
        "the declared-domain symbol participates in canonical instance identity"
    );

    let mut changed_field_domain = resolved.clone();
    let symbol_resolved_trees::data::DataMember::Field(field) = &changed_field_domain
        .tables
        .declarations
        .data_members
        .span_or_empty(instance_members)[0]
    else {
        unreachable!()
    };
    let symbol_resolved_trees::types::TypeReference::Constrained(field_type) =
        &field.type_reference
    else {
        unreachable!()
    };
    let field_constraints = field_type.constraints;
    let [symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = changed_field_domain
        .tables
        .types
        .constraints
        .span_mut_or_empty(field_constraints)
    else {
        unreachable!()
    };
    domain.name = other_name;
    assert!(
        !plain_data_extension_shape_is_supported(&changed_field_domain, frontier),
        "the substituted field must retain the origin's exact declared domain"
    );

    let mut detached_domain_name = resolved.clone();
    let symbol_resolved_trees::types::TypeReference::Constrained(argument) = &detached_domain_name
        .tables
        .declarations
        .child_type_references
        .span_or_empty(origin_arguments)[0]
    else {
        unreachable!()
    };
    let constraints = argument.constraints;
    let [symbol_resolved_trees::types::TypeConstraint::Domain(domain)] = detached_domain_name
        .tables
        .types
        .constraints
        .span_mut_or_empty(constraints)
    else {
        unreachable!()
    };
    domain.name = symbol_resolved_trees::name::DiagnosticName::generated("Issued");
    assert!(
        !plain_data_extension_shape_is_supported(&detached_domain_name, frontier),
        "a same-spelled domain without an authored selection cannot mint identity"
    );

    let typed = lower_seeded_extension(extension, base)
        .expect("the exact declared-domain instance should use the seeded continuation");
    let instance = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell<Token in Issued>")
        .expect("typed declared-domain Cell instance");
    let [typed_trees::data::DataMember::Field(field)] = typed.data_members(instance) else {
        panic!("typed declared-domain Cell instance retains one field")
    };
    let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("typed instance field retains its constraint")
    };
    let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
        typed.type_reference_table.constraints(*constraints)
    else {
        panic!("typed instance field retains one declared domain")
    };
    assert_eq!(domain.symbol, issued_symbol);
    assert!(domain.arguments.is_empty());

    let (base, extension) = seeded_normalized_plain_data_inputs(
        "data Authored { value: u16; } data Token { value: u8; } domain Token::Root; domain Token::Issued = Token::Root;",
        "data Cell<T> { value: T; } data Generated { value: Cell<Token in Issued>; }",
    );
    assert!(
        !plain_data_extension_shape_is_supported(
            extension.trees(),
            base.typed().data_definitions().len(),
        ),
        "transparent domain aliases remain outside the retained continuation cohort"
    );
}
