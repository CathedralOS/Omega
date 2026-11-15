//! A plan must retain its schema bytes after construction and transport every
//! initialized array element. These checks supply no normalized access rights.

use crate::{
    BuildTimeValue, InvocationTarget, PreparedBuildMachineProgram, SourceCompletionAccounting,
};
use typed_trees::expression::ExpressionNode;

use super::schema_customer_source as customer_source;

const SOURCE: &str = include_str!("access_plan_construction.omg");

fn facts() -> layout_plans::LayoutTargetFacts {
    // The same explicit target-neutral fixture model as the actual VecCell
    // consumer. It is neither native execution nor inferred host authority.
    layout_plans::LayoutTargetFacts {
        profile: "linux_x86_64".into(),
        pointer_size: 8,
        pointer_alignment: 8,
        case_tag_size: 4,
        case_tag_alignment: 4,
        little_endian: true,
    }
}

fn target(facts: &layout_plans::LayoutTargetFacts) -> InvocationTarget<'_> {
    InvocationTarget {
        name: Some(facts.profile.as_str()),
        layout_target: Some(facts),
        address_model: None,
        carrier_layout: Some(semantic_vocabulary::TargetCarrierLayout {
            pointer_byte_size: 8,
            pointer_byte_alignment: 8,
            descriptor_byte_size: 16,
            descriptor_byte_alignment: 8,
            case_tag_byte_size: 4,
            case_tag_byte_alignment: 4,
        }),
    }
}

fn field<'a>(value: &'a BuildTimeValue, name: &str) -> &'a BuildTimeValue {
    let BuildTimeValue::Struct { fields, .. } = value else {
        panic!("actual returned record")
    };
    let mut matching = fields.iter().filter(|(key, _)| key == name);
    let (_, result) = matching.next().expect("complete returned field roster");
    assert!(matching.next().is_none(), "unique returned field");
    result
}

fn array(value: &BuildTimeValue) -> &[BuildTimeValue] {
    let BuildTimeValue::Array(values) = value else {
        panic!("ordinary complete array")
    };
    values
}

fn empty_case<'a>(value: &'a BuildTimeValue, name: &str) -> &'a [(String, BuildTimeValue)] {
    let BuildTimeValue::Case { variant, payload } = value else {
        panic!("ordinary returned case")
    };
    assert_eq!(variant, name);
    payload
}

fn assert_zero_tail(plan: &BuildTimeValue) {
    let fields = array(field(plan, "fields"));
    assert_eq!(fields.len(), 32);
    for entry in fields {
        let description = field(entry, "field");
        for name in [
            "member_identity",
            "case_identity",
            "owner_identity",
            "owner_application",
            "name",
            "qualified_type",
            "source_revision",
        ] {
            assert_eq!(field(description, name), &BuildTimeValue::Text(Vec::new()));
        }
        for name in ["declaration_position", "stable_number"] {
            assert_eq!(field(description, name), &BuildTimeValue::Int(0));
        }
        for name in ["has_stable_number", "is_erased"] {
            assert_eq!(field(description, name), &BuildTimeValue::Bool(false));
        }
        assert!(empty_case(field(entry, "access"), "Inaccessible").is_empty());
    }
    for name in ["readers", "writers"] {
        let rules = array(field(plan, name));
        assert_eq!(rules.len(), 32);
        for rule in rules {
            assert!(array(field(rule, "callers")).is_empty());
            let payload = empty_case(field(rule, "region"), "Subject");
            let [(name, schema)] = payload else {
                panic!("complete zero Subject payload")
            };
            assert_eq!(name, "schema");
            assert_eq!(field(schema, "storage"), &BuildTimeValue::Text(Vec::new()));
        }
    }
    for name in ["field_count", "reader_count", "writer_count"] {
        assert_eq!(field(plan, name), &BuildTimeValue::Int(0));
    }
    let reach = field(plan, "reach");
    assert_eq!(field(reach, "service_count"), &BuildTimeValue::Int(0));
    let services = array(field(reach, "services"));
    assert_eq!(services.len(), 32);
    assert!(
        services
            .iter()
            .all(|value| value == &BuildTimeValue::Int(0))
    );
}

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let library =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../source/library");
    customer_source::actual_library_source(
        &library,
        source,
        semantic_vocabulary::PackageKeyIdentity::from_digest([0xA1; 32])
            .expect("retained test package"),
    )
    .expect("type actual candidate core and alloc with matching compiler catalog")
}

fn reflected_schema(
    typed: &typed_trees::TypedTrees,
    prepared: &PreparedBuildMachineProgram,
    requester_name: &str,
    invocation_target: InvocationTarget<'_>,
    accounting: &mut SourceCompletionAccounting,
) -> crate::MeasuredEvaluation<BuildTimeValue> {
    let (requester, expression) = query_call(typed, requester_name);
    let call = typed
        .expression_table
        .call_node(expression)
        .expect("same query call");
    let declaration = typed
        .machine_by_symbol(call.target_symbol)
        .or_else(|| typed.machine_holding_state(call.target_symbol))
        .expect("actual query declaration");
    assert_eq!(
        typed.core_vocabulary(declaration.symbol),
        Some(language_core::core_vocabulary::CoreVocabulary::Reflection(
            language_core::core_vocabulary::ReflectionDeclaration::Schema,
        )),
        "candidate library bytes must match the compiler's exact core catalog",
    );
    let schema = prepared
        .entry_call(requester, expression, &[])
        .expect("same lexical request")
        .invoke_recorded(invocation_target, accounting)
        .expect("genuine complete TypeSchema");
    let BuildTimeValue::Text(bytes) = field(schema.value(), "storage") else {
        panic!("actual graph bytes")
    };
    assert!(!bytes.is_empty(), "no empty schema approximation");
    schema
}

#[test]
fn actual_access_plan_constructor_keeps_schema_backing_and_complete_zero_tails() {
    let typed = typed_source(SOURCE);
    let prepared = PreparedBuildMachineProgram::prepare(&typed, None).expect("retain source");
    let facts = facts();
    let mut accounting = SourceCompletionAccounting::default();
    let schema = reflected_schema(
        &typed,
        &prepared,
        "describe_cell",
        target(&facts),
        &mut accounting,
    );
    let roundtrip = typed
        .realized_machine_named("return_plan")
        .expect("actual source transport declaration")
        .symbol;
    for (name, case) in [
        ("construct_restricted_plan", "Only"),
        ("construct_unrestricted_plan", "Unrestricted"),
    ] {
        let symbol = typed
            .realized_machine_named(name)
            .expect("actual source constructor")
            .symbol;
        let measured = prepared
            .entry(symbol, symbol, std::slice::from_ref(schema.value()))
            .expect("actual source entry")
            .invoke_recorded(target(&facts), &mut accounting)
            .expect("the complete AccessPlan is constructed through the checked caller");
        let plan = measured.value();
        assert_eq!(
            field(plan, "schema"),
            schema.value(),
            "borrowed constructor input preserves the complete immutable schema"
        );
        let payload = empty_case(field(plan, "issuance"), case);
        if case == "Only" {
            let [(name, routes)] = payload else {
                panic!("complete Only payload")
            };
            assert_eq!(name, "routes");
            assert!(
                array(routes).is_empty(),
                "explicit deny-all data, not unrestricted"
            );
        } else {
            assert!(payload.is_empty());
        }
        assert_zero_tail(plan);
        let returned = prepared
            .entry(roundtrip, roundtrip, std::slice::from_ref(plan))
            .expect("ordinary checked plan transport")
            .invoke_recorded(target(&facts), &mut accounting)
            .expect("the entire returned plan can be materialized again");
        assert_eq!(returned.value(), plan);
        let mut truncated = plan.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut truncated else {
            unreachable!()
        };
        let (_, BuildTimeValue::Array(entries)) = fields
            .iter_mut()
            .find(|(name, _)| name == "fields")
            .unwrap()
        else {
            unreachable!()
        };
        entries.pop();
        let error = prepared
            .entry(roundtrip, roundtrip, &[truncated])
            .and_then(|entry| entry.invoke_recorded(target(&facts), &mut accounting))
            .expect_err("zero active count cannot waive an incomplete initialized tail");
        assert!(
            error.contains("extent") || error.contains("length"),
            "{error}"
        );
    }
    assert!(accounting.has_complete_usage());
    accounting
        .validate_charges()
        .expect("same invocation work/result accounting");
}

#[test]
fn a_selected_access_plan_returns_its_declared_route_element_lifetimes() {
    use typed_trees::domain::DomainIssuance;
    use typed_trees::reflection::construction::{
        declaration_source_revision, exact_symbol_identity,
    };

    const SOURCE: &str = include_str!("selected_access_plan_family.omg");
    let empty = SOURCE.replace("routes: [reflect::callable(stamp)]", "routes: []");
    let facts = facts();
    for (source, route_count) in [(empty.as_str(), 0), (SOURCE, 1)] {
        let typed = typed_source(source);
        let prepared = PreparedBuildMachineProgram::prepare(&typed, None).expect("retain source");
        let mut accounting = SourceCompletionAccounting::default();
        let schema = reflected_schema(
            &typed,
            &prepared,
            "describe_ticket",
            target(&facts),
            &mut accounting,
        );
        let arguments = std::slice::from_ref(schema.value());
        let domain = typed
            .domain_definitions
            .iter()
            .map(|(_, row)| row)
            .find(|row| row.name.as_str() == "Ticket::Stamped")
            .expect("the authored governed domain");
        let DomainIssuance::Planned(policy) = domain.issuance() else {
            panic!("the authored domain retains its actual plan selection");
        };
        let conformance = typed
            .conformance_by_symbol(policy.implementation())
            .expect("the selected source policy");
        let [row] = conformance.rows.as_slice() else {
            panic!("one exact plan requirement");
        };
        let selected = prepared
            .entry_selected(
                domain.symbol,
                domain.symbol,
                &policy.selection,
                row.requirement,
                arguments,
            )
            .expect("prepare the domain's selected call")
            .invoke_recorded(target(&facts), &mut accounting)
            .expect("the selected plan publishes and executes with the declared lifetime family");
        let ordinary = typed
            .realized_machine_named("construct_direct_plan")
            .expect("the ordinary constructor control")
            .symbol;
        let direct = prepared
            .entry(ordinary, ordinary, arguments)
            .expect("prepare the ordinary call")
            .invoke_recorded(target(&facts), &mut accounting)
            .expect("the ordinary plan executes");
        assert_eq!(selected.value(), direct.value());
        assert_eq!(field(selected.value(), "schema"), schema.value());
        assert_zero_tail(selected.value());

        let [(name, routes)] = empty_case(field(selected.value(), "issuance"), "Only") else {
            panic!("the selected plan returns the complete Only payload");
        };
        assert_eq!(name, "routes");
        assert_eq!(array(routes).len(), route_count);
        if let [route] = array(routes) {
            let issuer = typed
                .realized_machine_named("stamp")
                .expect("the reflected issuer")
                .symbol;
            let (identity, hermetic) =
                exact_symbol_identity(&typed, issuer).expect("issuer identity");
            assert!(hermetic);
            let revision = declaration_source_revision(&typed, issuer).expect("issuer source");
            let description = empty_case(route, "Machine");
            assert_eq!(description.len(), 3);
            for (name, value) in description {
                let BuildTimeValue::Text(bytes) = value else {
                    panic!("the reflected issuer has immutable identity bytes");
                };
                match name.as_str() {
                    "declaration_identity" => assert_eq!(bytes, identity.as_bytes()),
                    "source_revision" => assert_eq!(bytes, &revision),
                    "callable_identity" => assert!(!bytes.is_empty()),
                    _ => panic!("unexpected reflected issuer field {name}"),
                }
            }
        }
        assert!(accounting.has_complete_usage());
        accounting
            .validate_charges()
            .expect("complete invocation usage");
    }
}

#[test]
fn ordinary_lifetime_invocation_keeps_schema_images_and_refuses_local_escape() {
    let source = r#"
        use omega::language::core::reflect;
        data Cell { value: u32; }
        machine describe() -> reflect::TypeSchema { reflect::schema<Cell>() }
        machine carry<'text>(schema: reflect::TypeSchema<'text>) -> reflect::TypeSchema<'text> { schema }
        machine local_escape<'text>() -> &'text [u8] {
            let bytes: [u8; 1] = [7u8];
            let view: &[u8] = &bytes[..];
            view
        }
    "#;
    let typed = typed_source(source);
    let prepared = PreparedBuildMachineProgram::prepare(&typed, None).unwrap();
    let facts = facts();
    let mut accounting = SourceCompletionAccounting::default();
    let schema = reflected_schema(
        &typed,
        &prepared,
        "describe",
        target(&facts),
        &mut accounting,
    );
    let carry = typed.realized_machine_named("carry").unwrap().symbol;
    let returned = prepared
        .entry(carry, carry, &[schema.value().clone()])
        .unwrap()
        .invoke_recorded(target(&facts), &mut accounting)
        .unwrap();
    assert_eq!(
        returned.value(),
        schema.value(),
        "the real query image survives the ordinary lifetime-bound call"
    );
    let local_escape = typed.realized_machine_named("local_escape").unwrap().symbol;
    let before = accounting.receipts().len();
    let error = prepared
        .entry(local_escape, local_escape, &[])
        .err()
        .expect("a fresh call lifetime cannot retain this callee's local array");
    assert!(error.contains("lifetime"), "{error}");
    assert!(
        !error.contains("not supported yet"),
        "the actual local-storage contract must reject: {error}"
    );
    assert_eq!(
        accounting.receipts().len(),
        before,
        "no evaluation of the escaping local borrow"
    );
    accounting.validate_charges().unwrap();
}

fn last_receipt(
    accounting: &SourceCompletionAccounting,
) -> std::sync::Arc<crate::PureTerminalEvaluationReceipt> {
    let Some(crate::PureEvaluationRecord::Invocation(receipt)) = accounting.receipts().last()
    else {
        panic!("the actual checked invocation has its own receipt")
    };
    receipt.clone()
}

fn query_call(
    typed: &typed_trees::TypedTrees,
    name: &str,
) -> (
    symbols::SymbolHandle,
    typed_trees::expression::ExpressionHandle,
) {
    let author = typed.realized_machine_named(name).unwrap().symbol;
    let mut expressions = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            (typed_trees::references::StaticArgumentCall::Expression(handle, call)
                .lexical_owner(typed)
                == author)
                .then_some(handle)
        });
    let call = expressions
        .next()
        .expect("the source author owns its query");
    assert!(expressions.next().is_none(), "one exact declaration query");
    (author, call)
}

fn field_mut<'a>(value: &'a mut BuildTimeValue, name: &str) -> &'a mut BuildTimeValue {
    let BuildTimeValue::Struct { fields, .. } = value else {
        panic!("record data")
    };
    &mut fields.iter_mut().find(|(key, _)| key == name).unwrap().1
}

// This is ordinary checked plan construction, not a selected policy call. It
// exercises the semantic decoder with real descriptors before any authority
// consumer exists, and checks that a genuine record type cannot bless bad data.
#[test]
fn actual_access_plan_normalization_preserves_rules_and_authenticates_descriptions() {
    let typed = typed_source(SOURCE);
    let prepared = PreparedBuildMachineProgram::prepare(&typed, None).unwrap();
    let facts = facts();
    let mut accounting = SourceCompletionAccounting::default();
    let private_query = typed
        .realized_machine_named("inaccessible_cell_field")
        .unwrap()
        .symbol;
    let error = prepared
        .entry(private_query, private_query, &[])
        .err()
        .expect("this source cannot adopt another source file's private field");
    assert!(
        error.contains("a field query is outside its author's declaration scope"),
        "{error}"
    );
    assert!(
        accounting.receipts().is_empty(),
        "visibility refusal precedes execution"
    );
    let mut descriptions = Vec::new();
    for name in [
        "describe_rule_cell",
        "describe_machine",
        "describe_requirement",
    ] {
        let (author, expression) = query_call(&typed, name);
        let value = prepared
            .entry_call(author, expression, &[])
            .unwrap()
            .invoke_recorded(target(&facts), &mut accounting)
            .unwrap_or_else(|error| panic!("actual {name}: {error}"));
        descriptions.push((value.value().clone(), last_receipt(&accounting)));
    }
    let (schema, query) = &descriptions[0];
    let builder = typed
        .realized_machine_named("construct_observed_plan")
        .unwrap()
        .symbol;
    let arguments = [
        schema.clone(),
        descriptions[1].0.clone(),
        descriptions[2].0.clone(),
    ];
    let produced = prepared
        .entry(builder, builder, &arguments)
        .unwrap()
        .invoke_recorded(target(&facts), &mut accounting)
        .expect("nonempty plan is constructed and returned through ordinary checked source");
    let result = last_receipt(&accounting);
    let normalized = crate::terminal_values::access_plan_result(query, &result)
        .expect("exact query and result correspondence");
    let issuers = normalized.issuance.as_ref().expect("explicit Only choice");
    assert_eq!(issuers.len(), 2);
    assert_eq!(
        issuers.iter().filter(|(required, _)| *required).count(),
        1,
        "a public requirement must remain distinct from a concrete machine"
    );
    let fields = normalized.schema.governed_fields().unwrap();
    assert_eq!(
        fields.len(),
        1,
        "the actual source-owned RuleCell value member"
    );
    let member = fields[0].0;
    assert_eq!(
        normalized.readers.len(),
        3,
        "parent and repeated field rows intersect"
    );
    assert!(
        normalized
            .readers
            .iter()
            .any(|(region, callers)| *region == normalized.schema.root() && callers.len() == 2)
    );
    assert!(
        normalized
            .readers
            .iter()
            .any(|(region, callers)| *region == member && callers.len() == 1)
    );
    assert!(
        normalized
            .readers
            .iter()
            .any(|(region, callers)| *region == member && callers.is_empty())
    );
    assert_eq!(
        normalized.writers,
        vec![(member, Vec::new())],
        "explicit empty is denial"
    );
    assert_eq!(normalized.observations.len(), 2);
    assert!(
        normalized.observations.contains(&(member, None)),
        "Inaccessible is an authored row"
    );
    assert!(
        normalized
            .observations
            .iter()
            .any(|(region, demand)| *region == member
                && demand.is_some_and(|(model, permissions, exposure)| model
                    == access_plans::ObservationModel::Stable
                    && permissions.read
                    && !permissions.write
                    && exposure == access_plans::AccessExposure::Exported))
    );

    let transport = typed
        .realized_machine_named("transport_observed_plan")
        .unwrap()
        .symbol;
    // Each altered value is still supplied to an ordinary checked call. The
    // receiving decoder must reject its semantic correspondence independently.
    for (component, reason) in [
        ("member_identity", "outside the actual schema subject"),
        (
            "source_revision",
            "another owner, application or source revision",
        ),
    ] {
        let mut altered = produced.value().clone();
        let BuildTimeValue::Array(entries) = field_mut(&mut altered, "fields") else {
            unreachable!()
        };
        *field_mut(field_mut(&mut entries[0], "field"), component) =
            BuildTimeValue::Text(b"foreign".to_vec());
        prepared
            .entry(transport, transport, &[schema.clone(), altered])
            .unwrap()
            .invoke_recorded(target(&facts), &mut accounting)
            .unwrap();
        let error = crate::terminal_values::access_plan_result(query, &last_receipt(&accounting))
            .unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
    let mut altered = produced.value().clone();
    let BuildTimeValue::Case { payload, .. } = field_mut(&mut altered, "issuance") else {
        unreachable!()
    };
    let (_, BuildTimeValue::Array(routes)) = &mut payload[0] else {
        unreachable!()
    };
    let BuildTimeValue::Case { variant, .. } = &mut routes[0] else {
        unreachable!()
    };
    *variant = "Requirement".into();
    prepared
        .entry(transport, transport, &[schema.clone(), altered])
        .unwrap()
        .invoke_recorded(target(&facts), &mut accounting)
        .unwrap();
    let error =
        crate::terminal_values::access_plan_result(query, &last_receipt(&accounting)).unwrap_err();
    assert!(
        error.contains("callable description is not bound to the actual source"),
        "{error}"
    );

    let lookalike = typed
        .realized_machine_named("construct_lookalike")
        .unwrap()
        .symbol;
    prepared
        .entry(lookalike, lookalike, std::slice::from_ref(schema))
        .unwrap()
        .invoke_recorded(target(&facts), &mut accounting)
        .unwrap();
    let error =
        crate::terminal_values::access_plan_result(query, &last_receipt(&accounting)).unwrap_err();
    assert!(
        error.contains("not the genuine core AccessPlan declaration"),
        "{error}"
    );
    accounting.validate_charges().unwrap();
}
