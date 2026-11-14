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

#[test]
fn actual_access_plan_constructor_keeps_schema_backing_and_complete_zero_tails() {
    let library =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../source/library");
    let typed = customer_source::actual_library_source(
        &library,
        SOURCE,
        semantic_vocabulary::PackageKeyIdentity::from_digest([0xA1; 32])
            .expect("retained test package"),
    )
    .expect("type actual candidate core and alloc with matching compiler catalog");
    let requester = typed
        .realized_machine_named("describe_cell")
        .expect("actual source query author")
        .symbol;
    let mut calls = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            (typed_trees::references::StaticArgumentCall::Expression(expression, call)
                .lexical_owner(&typed)
                == requester)
                .then_some(expression)
        });
    let expression = calls.next().expect("actual retained schema query");
    assert!(
        calls.next().is_none(),
        "one exact request in the source query author"
    );
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
    let prepared = PreparedBuildMachineProgram::prepare(&typed, None).expect("retain source");
    let facts = facts();
    let mut accounting = SourceCompletionAccounting::default();
    let schema = prepared
        .entry_call(requester, expression, &[])
        .expect("same lexical request")
        .invoke_recorded(target(&facts), &mut accounting)
        .expect("genuine complete TypeSchema");
    let BuildTimeValue::Text(bytes) = field(schema.value(), "storage") else {
        panic!("actual graph bytes")
    };
    assert!(!bytes.is_empty(), "no empty schema approximation");
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
