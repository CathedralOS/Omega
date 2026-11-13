//! Actual library policy source under explicit retained test-package custody.
//! The invocation model below is a target-neutral fixture, not native execution
//! or ordinary package/lock admission evidence.

use crate::{
    BuildTimeValue, InvocationTarget, PreparedBuildMachineProgram, SourceCompletionAccounting,
};

#[path = "schema_customer_source.rs"]
mod customer_source;

const QUERY: &str = "use omega::language::core::reflect;\nuse vec::VecCell;\nmachine describe_cell() -> reflect::TypeSchema { reflect::schema<VecCell>() }";

fn source() -> (
    typed_trees::TypedTrees,
    typed_trees::expression::ExpressionHandle,
    symbols::SymbolHandle,
) {
    customer_source::actual_vec_cell_source(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../source/library"),
        QUERY,
    )
    .expect("prepare unchanged real library in retained test package")
}

fn facts() -> layout_plans::LayoutTargetFacts {
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
        name: &facts.profile,
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
fn field<'value>(value: &'value BuildTimeValue, name: &str) -> &'value BuildTimeValue {
    let BuildTimeValue::Struct { fields, .. } = value else {
        panic!("ordinary returned record");
    };
    let mut matching = fields.iter().filter(|(key, _)| key == name);
    let (_, value) = matching.next().expect("actual returned field");
    assert!(matching.next().is_none());
    value
}

#[test]
fn actual_vector_policy_uses_complete_schema_and_keeps_query_custody() {
    let (typed, expression, selected) = source();
    let requester = typed
        .realized_machine_named("describe_cell")
        .expect("actual query caller")
        .symbol;
    let prepared =
        PreparedBuildMachineProgram::prepare(&typed, None).expect("retain actual source");
    let query = prepared
        .entry_call(requester, expression)
        .expect("retain actual source query");
    let policy = prepared
        .entry(requester, selected)
        .expect("retain actual vector plan");
    let facts = facts();
    let mut accounting = SourceCompletionAccounting::default();
    let (query_receipt, policy_receipt, report, storage) = query
        .evaluate_layout_policy(&policy, target(&facts), &[], &mut accounting)
        .expect("execute unchanged vector plan with its complete Schema");
    assert_eq!(report.size, Some(4));
    assert_eq!(report.align, 4);
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.entries[0].field, "value");
    assert_eq!(
        report.entries[0].placement,
        layout_plans::LayoutPlacementReport::At { offset: 0 }
    );
    let application = super::super::schema_geometry::query_reference(query_receipt)
        .expect("the actual request retains its type operand");
    let mut rows = query_receipt
        .program()
        .source_records()
        .iter()
        .filter(|row| {
            row.application
                .as_ref()
                .is_some_and(|source| source.reference == application)
        });
    let row = rows
        .next()
        .expect("actual query module's source correspondence");
    assert!(rows.next().is_none());
    assert_eq!(row.fields.len(), 1);
    assert_eq!(storage.len(), 1);
    assert_eq!(storage[0].field, row.fields[0].field);
    assert_eq!(
        storage[0].storage,
        semantic_vocabulary::AuthoredFieldStorage::Direct { byte_offset: 0 }
    );
    assert!(
        query_receipt
            .program()
            .view()
            .module()
            .structural_type(row.record)
            .is_some()
    );
    let crate::PureTerminalEvaluationOutcome::Materialized { value, .. } = policy_receipt.outcome()
    else {
        panic!("actual whole AccessPlan result");
    };
    assert!(
        matches!(field(field(value, "layout"), "entries"), BuildTimeValue::Array(entries) if entries.len() == 64)
    );
    assert!(
        matches!(field(value, "fields"), BuildTimeValue::Array(entries) if entries.len() == 32)
    );
    assert_eq!(field(value, "field_count"), &BuildTimeValue::Int(1));
    query_receipt
        .validate_charge()
        .expect("actual query charges");
    policy_receipt
        .validate_charge()
        .expect("actual policy charges");

    // The same geometry with a changed opaque field key is a different input.
    // A completed policy result cannot be rebound to it after execution.
    let mut foreign_input =
        super::super::schema_geometry::schema_geometry(query_receipt, target(&facts))
            .expect("actual geometry roster");
    foreign_input.fields[0].key += 1;
    let error =
        super::super::schema_values::layout_value(query_receipt, policy_receipt, &foreign_input)
            .expect_err("the result retains its exact actual Schema argument");
    assert!(
        error.contains("not bound to this exact Schema input"),
        "{error}"
    );
}

#[test]
fn equal_handles_from_another_prepared_owner_do_not_supply_policy_custody() {
    let (typed, expression, selected) = source();
    let requester = typed
        .realized_machine_named("describe_cell")
        .expect("actual query caller")
        .symbol;
    let first = PreparedBuildMachineProgram::prepare(&typed, None).expect("first source owner");
    let second =
        PreparedBuildMachineProgram::prepare(&typed, None).expect("independent source owner");
    let query = first
        .entry_call(requester, expression)
        .expect("owned query");
    let foreign = second
        .entry(requester, selected)
        .expect("independent plan metadata");
    let facts = facts();
    let mut accounting = SourceCompletionAccounting::default();
    let error = query
        .evaluate_layout_policy(&foreign, target(&facts), &[], &mut accounting)
        .expect_err("equal raw source handles do not join retained owners");
    assert!(
        error.contains("different source or requester custody"),
        "{error}"
    );
    assert!(
        accounting.receipts().is_empty(),
        "no execution before custody joins"
    );
}
