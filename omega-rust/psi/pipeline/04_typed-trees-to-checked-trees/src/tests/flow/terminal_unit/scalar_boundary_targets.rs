//! A Unit closure consumes the scalar boundary wrapper's real checked body.

use super::super::super::TypeReferenceNode;
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedUnitEffectOperationPlan,
    PrimitiveType,
};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

const SCALAR_PARAMETERS_SOURCE: &str = r#"
    boundary trait Host {
        machine measure(flag: bool, value: i32) -> i32;
        machine send(value: i32);
    }
    data Scalar {}
    machine Scalar::measure(value: i32, flag: bool) -> i32
    reaches Host
    { let result: i32 = Host::measure(flag, value); result }
    data Root {}
    machine Root::run(flag: bool, value: i32)
    reaches Host
    { let result: i32 = Scalar::measure(value, flag); Host::send(result); }
"#;

#[test]
fn scalar_boundary_wrapper_retains_reordered_scalar_formals_and_actuals() {
    let original = checked(SCALAR_PARAMETERS_SOURCE);
    let root = machine_named(&original, "run");
    let target = machine_named(&original, "Scalar::measure");
    let wrapper = original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(target)
        .expect("parameterized wrapper retains its real boundary-return body");
    assert_eq!(
        wrapper
            .scalar_parameters
            .iter()
            .map(|parameter| { (parameter.source_position, parameter.primitive_type) })
            .collect::<Vec<_>>(),
        vec![(0, PrimitiveType::I32), (1, PrimitiveType::Bool)]
    );
    assert!(wrapper.structural_parameters.is_empty());
    let unit = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("ordinary Unit call retains scalar-only wrapper");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        scalar_arguments, ..
    } = &unit.operations[0]
    else {
        panic!("expected real scalar invocation")
    };
    assert!(matches!(
        &scalar_arguments[0],
        checked_trees::CheckedCallScalarArgument::Pure(CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::I32
        })
    ));
    assert!(
        matches!(&scalar_arguments[1], checked_trees::CheckedCallScalarArgument::Pure(
        CheckedScalarExpression::Boolean(expression)
    ) if matches!(expression.as_ref(), CheckedBooleanExpression::Parameter { position: 0 }))
    );
    for mutation in 0..4 {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_boundary_scalar_returns.machines;
        let index = plans
            .iter()
            .position(|plan| plan.machine == target)
            .unwrap();
        match mutation {
            0 => plans[index].scalar_parameters[0].source_position = 1,
            1 => plans[index].scalar_parameters[0].primitive_type = PrimitiveType::Bool,
            2 => {
                plans[index].scalar_parameters.pop();
            }
            _ => plans.push(plans[index].clone()),
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(root).is_none(),
            "scalar roster drift {mutation}"
        );
    }
}

#[test]
fn scalar_boundary_wrapper_does_not_erase_scalar_requirements() {
    let source = r#"
        boundary trait Host { machine measure(value: i32) -> i32; }
        data Scalar {}
        machine Scalar::measure(value: i32) -> i32
        requires 1 <= value
        reaches Host
        { let result: i32 = Host::measure(value); result }
    "#;
    let original = checked(source);
    let target = machine_named(&original, "Scalar::measure");
    assert!(
        original
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .for_machine(target)
            .is_some()
    );
    assert!(matches!(
        original
            .facts
            .contract_plans
            .for_machine(target)
            .unwrap()
            .closed_scalar_values
            .requires(),
        [Some(checked_trees::ClosedScalarContractValue::Predicate(_))]
    ));
    // Exact `+`/`-`/`*` arithmetic is closed contract vocabulary; a bitwise
    // operand is not.
    let unsupported = checked(&source.replace("1 <= value", "value & 1 <= 100"));
    let target = machine_named(&unsupported, "Scalar::measure");
    assert!(
        unsupported
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .for_machine(target)
            .is_none(),
        "unsupported authored requirement must not be discarded"
    );
}

#[test]
fn scalar_parameter_range_collector_keeps_unsupported_endpoint_rows() {
    let source = r#"
        boundary trait Host { machine measure(value: i32) -> i32; }
        data Scalar {}
        machine Scalar::measure(value: i32 [1..=100]) -> i32
        reaches Host
        { let result: i32 = Host::measure(value); result }
    "#;
    let mut checked = checked(source);
    let target = machine_named(&checked, "Scalar::measure");
    let mut missing = checked.clone();
    let contract = missing
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|contract| contract.machine == target)
        .unwrap();
    contract.closed_scalar_values = checked_trees::ClosedScalarValueContractPlan::new(
        Vec::new(),
        contract.closed_scalar_values.ensures().to_vec(),
        contract.closed_scalar_values.has_crash_clauses(),
        contract.closed_scalar_values.has_outcome_specific_clauses(),
    );
    let rebuilt =
        crate::execution::terminal_unit::returns::build_checked_boundary_scalar_return_plans(
            &missing.typed,
            &missing.facts,
        );
    assert!(
        rebuilt.for_machine(target).is_none(),
        "a missing implicit range is not an empty contract"
    );
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == target)
        .unwrap()
        .clone();
    let state = &checked.typed.machine_states(&machine)[0];
    let reference = checked.typed.state_parameters(state)[0].type_reference;
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
    let requirements =
        crate::values::lower_integer_parameter_range_requirements(&checked.typed, &machine);
    assert_eq!(
        requirements,
        vec![None],
        "unsupported range remains a rejecting slot"
    );
}

#[test]
fn scalar_boundary_wrapper_transfers_exact_linear_claim_with_mixed_signature() {
    let source = r#"
        pub data Receipt [linear] { value: u64; }
        boundary machine Receipt::settle(self, value: u16) -> u16
        reaches PortIo ensures true;
        data Wrapper {}
        machine Wrapper::measure(value: u16, receipt: Receipt) -> u16
        reaches PortIo
        { let result: u16 = receipt.settle(value); result }
        data Root {}
        machine Root::run(receipt: Receipt) reaches PortIo
        { let result: u16 = Wrapper::measure(70u16, receipt); }
    "#;
    let original = checked(source);
    let root = machine_named(&original, "run");
    let target = machine_named(&original, "Wrapper::measure");
    let wrapper = original
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .for_machine(target)
        .expect("real scalar boundary-return body");
    assert_eq!(wrapper.scalar_parameters[0].source_position, 0);
    assert_eq!(wrapper.structural_parameters[0].position, 1);
    assert_eq!(wrapper.entry_claims.len(), 1);
    let caller = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .expect("Unit call transfers linear custody to scalar wrapper");
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        claim_transfers,
        scalar_arguments,
        ..
    } = &caller.operations[0]
    else {
        panic!("ordinary scalar call with custody")
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(scalar_arguments.len(), 1);
    assert_eq!(claim_transfers.len(), 1);
    assert_eq!(claim_transfers[0].argument_index, 0);
    assert_eq!(
        claim_transfers[0].claim_identity,
        caller.entry_claims[0].claim_identity
    );
    assert_ne!(
        claim_transfers[0].claim_identity,
        wrapper.entry_claims[0].claim_identity
    );
    assert!(
        matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_discards, trivial_affine_local_discard_ordinals, ..
    }) if trivial_affine_discards.is_empty() && trivial_affine_local_discard_ordinals.is_empty())
    );
    for mutation in 0..4 {
        let mut changed = original.clone();
        let wrapper = changed
            .facts
            .flow
            .terminal_boundary_scalar_returns
            .machines
            .iter_mut()
            .find(|plan| plan.machine == target)
            .unwrap();
        match mutation {
            0 => wrapper.entry_claims.clear(),
            1 => wrapper.entry_claims[0].parameter_index = 1,
            2 => wrapper.structural_parameters[0].position = 0,
            _ => wrapper.structural_parameters[0]
                .type_identity
                .push_str("-foreign"),
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &changed.typed,
            &changed.facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &changed.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &changed.facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(root).is_none(),
            "mixed target custody mutation {mutation}"
        );
    }
}

#[test]
fn scalar_wrapper_transfer_keeps_unconsumed_empty_prefix_cleanup() {
    let source = r#"
        data Payload {}
        boundary trait Host { machine measure(payload: Payload) -> u16; }
        data Wrapper {}
        machine Wrapper::measure(payload: Payload) -> u16 reaches Host
        { let result: u16 = Host::measure(payload); result }
        data Root {}
        machine Root::run() reaches Host {
            let unused: Payload = Payload {};
            let consumed: Payload = Payload {};
            let result: u16 = Wrapper::measure(consumed);
        }
    "#;
    let checked = checked(source);
    let root = machine_named(&checked, "run");
    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root)
        .unwrap();
    assert!(
        matches!(caller.operations.last(), Some(CheckedUnitEffectOperationPlan::Complete {
        trivial_affine_local_discard_ordinals, ..
    }) if trivial_affine_local_discard_ordinals == &[0])
    );
}
