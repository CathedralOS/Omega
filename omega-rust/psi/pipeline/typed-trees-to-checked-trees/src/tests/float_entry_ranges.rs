//! Floating entry ranges retain their authored IEEE endpoints on the closed
//! scalar contract roster. The requires tail carries each range as a
//! `FloatRange` clause in the same constraint order so the closed scalar
//! vocabulary itself spells the authored IEEE window.
use super::{
    Lexer, ResolutionRequest, SymbolHandle, TypeReferenceNode, lower_symbol_resolved_trees,
    parse_syntax_trees, resolve,
};
use crate::lower_typed_trees;
use semantic_vocabulary::IeeeFloatValue;
use typed_trees::types::PrimitiveType;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

fn machine_named(checked: &checked_trees::CheckedTrees, name: &str) -> SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == name || machine.name.as_str().ends_with(&format!("::{name}"))
        })
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

fn contract_plan(
    checked: &checked_trees::CheckedTrees,
    machine: SymbolHandle,
) -> &checked_trees::MachineContractPlan {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .unwrap_or_else(|| panic!("missing contract plan"))
}

#[test]
fn exclusive_f64_entry_range_retains_authored_endpoint() {
    let checked = checked(
        r#"
        data Metrics {}
        machine Metrics::accept(&self, value: f64[0.0..1.5]) -> f64 { value }
    "#,
    );
    let machine = machine_named(&checked, "Metrics::accept");
    let plan = contract_plan(&checked, machine);
    let [requirement] = plan
        .closed_scalar_values
        .float_entry_ranges()
        .expect("one complete retained floating range")
    else {
        panic!("one retained floating range")
    };
    assert_eq!(
        *requirement,
        checked_trees::ClosedFloatRangeRequirement {
            position: 0,
            primitive_type: PrimitiveType::F64,
            minimum: IeeeFloatValue::Binary64(0.0f64.to_bits()),
            maximum: IeeeFloatValue::Binary64(1.5f64.to_bits()),
            maximum_inclusive: false,
        }
    );
    // An integer-predecessor normalization would land one ulp below the
    // authored endpoint; the retained bits stay authored.
    assert_ne!(
        requirement.maximum,
        IeeeFloatValue::Binary64(1.5f64.to_bits() - 1)
    );
    // The requires tail keeps the authored range as a supported clause row:
    // the closed scalar vocabulary carries the exact retained requirement
    // rather than an unsupported placeholder.
    assert_eq!(
        plan.closed_scalar_values.requires(),
        &[Some(checked_trees::ClosedScalarContractValue::FloatRange(
            *requirement,
        ))]
    );
}

#[test]
fn inclusive_f64_entry_range_retains_boundary_kind() {
    let checked = checked(
        r#"
        data Metrics {}
        machine Metrics::accept(&self, value: f64[0.0..=1.5]) -> f64 { value }
    "#,
    );
    let machine = machine_named(&checked, "Metrics::accept");
    let [requirement] = contract_plan(&checked, machine)
        .closed_scalar_values
        .float_entry_ranges()
        .expect("one retained floating range")
    else {
        panic!("one retained floating range")
    };
    assert_eq!(
        *requirement,
        checked_trees::ClosedFloatRangeRequirement {
            position: 0,
            primitive_type: PrimitiveType::F64,
            minimum: IeeeFloatValue::Binary64(0.0f64.to_bits()),
            maximum: IeeeFloatValue::Binary64(1.5f64.to_bits()),
            maximum_inclusive: true,
        }
    );
}

#[test]
fn f32_entry_range_names_its_dense_scalar_position() {
    let checked = checked(
        r#"
        data Carrier { raw: u64; }
        machine Carrier::accept(&self, flag: bool, value: f32[0.5..2.25], aux: Carrier) -> f32 { value }
    "#,
    );
    let machine = machine_named(&checked, "Carrier::accept");
    let [requirement] = contract_plan(&checked, machine)
        .closed_scalar_values
        .float_entry_ranges()
        .expect("one retained floating range")
    else {
        panic!("one retained floating range")
    };
    // `&self` and the structural `aux` never inhabit the scalar namespace;
    // the `bool` flag occupies dense scalar position zero.
    assert_eq!(
        *requirement,
        checked_trees::ClosedFloatRangeRequirement {
            position: 1,
            primitive_type: PrimitiveType::F32,
            minimum: IeeeFloatValue::Binary32(0.5f32.to_bits()),
            maximum: IeeeFloatValue::Binary32(2.25f32.to_bits()),
            maximum_inclusive: false,
        }
    );
}

#[test]
fn integer_endpoints_convert_once_into_the_float_carrier() {
    let checked = checked(
        r#"
        data Metrics {}
        machine Metrics::accept(&self, value: f64[0..2]) -> f64 { value }
    "#,
    );
    let machine = machine_named(&checked, "Metrics::accept");
    let [requirement] = contract_plan(&checked, machine)
        .closed_scalar_values
        .float_entry_ranges()
        .expect("one retained floating range")
    else {
        panic!("one retained floating range")
    };
    assert_eq!(
        *requirement,
        checked_trees::ClosedFloatRangeRequirement {
            position: 0,
            primitive_type: PrimitiveType::F64,
            minimum: IeeeFloatValue::Binary64(0.0f64.to_bits()),
            maximum: IeeeFloatValue::Binary64(2.0f64.to_bits()),
            maximum_inclusive: false,
        }
    );
}

#[test]
fn machines_without_floating_ranges_have_an_empty_complete_roster() {
    let checked = checked(
        r#"
        data Metrics {}
        machine Metrics::accept(&self, value: i32[0..100]) -> i32 { value }
    "#,
    );
    let machine = machine_named(&checked, "Metrics::accept");
    let plan = contract_plan(&checked, machine);
    assert_eq!(
        plan.closed_scalar_values.float_entry_ranges(),
        Some(&[][..]),
        "integer ranges do not enter the floating roster"
    );
    assert!(
        matches!(
            plan.closed_scalar_values.requires(),
            [Some(checked_trees::ClosedScalarContractValue::Predicate(_))]
        ),
        "integer ranges keep their closed scalar predicate"
    );
}

#[test]
fn corrupted_range_endpoint_loses_the_complete_roster() {
    let mut checked = checked(
        r#"
        data Metrics {}
        machine Metrics::accept(&self, value: f64[0.0..1.5]) -> f64 { value }
    "#,
    );
    let machine = machine_named(&checked, "Metrics::accept");
    let typed_machine = checked
        .typed
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .unwrap()
        .clone();
    let state = &checked.typed.machine_states(&typed_machine)[0];
    let reference = checked.typed.state_parameters(state)[1].type_reference;
    let TypeReferenceNode::Constrained { constraints, .. } =
        checked.typed.type_reference_table.type_reference(reference)
    else {
        panic!("authored parameter range")
    };
    let constraints = *constraints;
    let [typed_trees::types::TypeConstraintNode::Range { maximum, .. }] = checked
        .typed
        .type_reference_table
        .constraints_mut(constraints)
    else {
        panic!("one authored range")
    };
    *maximum = arena::Handle::invalid();
    let ranges =
        crate::values::lower_scalar_parameter_range_requirements(&checked.typed, &typed_machine);
    assert_eq!(
        ranges.float_entry_ranges, None,
        "an endpoint that cannot be retained exactly fails the whole roster"
    );
    assert_eq!(
        ranges.scalar_clauses,
        vec![None],
        "an unretained floating range keeps an explicit rejecting requires row"
    );
}
