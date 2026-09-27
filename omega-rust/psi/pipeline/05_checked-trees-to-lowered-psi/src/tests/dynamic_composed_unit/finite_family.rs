//! Finite generic requirement families behind dynamic dispatch.
//!
//! A requirement declared with an explicit OR/equality roster expands to one
//! Terminal conformance-table row per canonical value tuple; each family row
//! names the tuple's bare specialization instance, not the provider template
//! the checked row still records. These tests pin that expansion and the
//! tuple-keyed joins for every dynamic lane: direct, rebound, forwarded and
//! joined scalar calls, and the result-less Unit call.

use super::{
    FAMILY_DYNAMIC_INTEGER_SOURCE, FAMILY_DYNAMIC_UNIT_SOURCE,
    JOINED_FAMILY_DYNAMIC_BOOLEAN_SOURCE, REBOUND_FAMILY_DYNAMIC_INTEGER_SOURCE,
    assert_dynamic_unit_artifact_executes, unsupported_message,
};
use crate::TerminalMachineSelection;
use crate::tests::lower_machine;
use checked_trees::CheckedDynamicBinding::{Direct, Joined, Rebound};
use checked_trees::CheckedDynamicDispatchPlan::{Scalar, Unit};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, OperationResult};

const WIDTH_16: &str = "named(integer-const(16))";
const WIDTH_32: &str = "named(integer-const(32))";

#[test]
fn rebound_family_call_retains_every_tuple_row_callable() {
    let checked = crate::front_end::checked_program(REBOUND_FAMILY_DYNAMIC_INTEGER_SOURCE);
    let [Scalar(Rebound { latest, .. })] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_slice()
    else {
        panic!("one checked rebound family plan expected")
    };
    assert_eq!(latest.family_tuple.as_ref(), [WIDTH_32.to_owned()]);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("the rebound family call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the rebound family module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    let [dispatch] = catalog.indirect_dispatches.as_slice() else {
        panic!("one rebound family dispatch expected: {catalog:#?}")
    };
    assert_eq!(dispatch.family_tuple.as_slice(), [WIDTH_32]);
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one rebound family conformance application expected")
    };
    // A rebound descriptor must carry the complete tuple-keyed table: every
    // roster row binds its own specialization callable so the descriptor can
    // be re-dispatched through either tuple later.
    let [sixteen, thirty_two] = application.rows.as_slice() else {
        panic!("the rebound family application expands to one row per tuple")
    };
    assert_eq!(sixteen.family_tuple.as_slice(), [WIDTH_16]);
    assert_eq!(thirty_two.family_tuple.as_slice(), [WIDTH_32]);
    let [sixteen_callable, thirty_two_callable] = application.realization_callables.as_slice()
    else {
        panic!("both roster tuple callables are retained for the rebound table")
    };
    assert_eq!(
        sixteen.realization_callable_identity.as_deref(),
        Some(sixteen_callable.source_callable_identity.as_str())
    );
    assert_eq!(
        thirty_two.realization_callable_identity.as_deref(),
        Some(thirty_two_callable.source_callable_identity.as_str())
    );
    assert_eq!(
        dispatch.realization_callable_identity,
        thirty_two_callable.source_callable_identity
    );
    assert_eq!(dispatch.realization, thirty_two_callable.machine);

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the rebound family module encodes canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("the rebound family module decodes"),
        lowered.semantic_module,
    );
}

#[test]
fn joined_family_call_materializes_the_tuple_roster_once() {
    let checked = crate::front_end::checked_program(JOINED_FAMILY_DYNAMIC_BOOLEAN_SOURCE);
    let [
        Scalar(Joined {
            when_true,
            when_false,
            ..
        }),
    ] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_slice()
    else {
        panic!("one checked joined family plan expected")
    };
    assert_eq!(when_true.call.family_tuple.as_ref(), [WIDTH_32.to_owned()]);
    assert_eq!(when_false.call.family_tuple.as_ref(), [WIDTH_32.to_owned()]);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("the joined family call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the joined family module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    assert_eq!(catalog.selections.len(), 2);
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one joined family parameter expected: {catalog:#?}")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one joined family parameter dispatch expected")
    };
    // Both branches share the provider template, so the tuple-keyed
    // realization roster materializes once even though the selections bind
    // different conformances.
    let [sixteen, thirty_two] = parameter.requirements.as_slice() else {
        panic!("the joined interface expands to one requirement per tuple")
    };
    assert_eq!(sixteen.family_tuple.as_slice(), [WIDTH_16]);
    assert_eq!(thirty_two.family_tuple.as_slice(), [WIDTH_32]);
    assert_eq!(dispatch.requirement_slot, thirty_two.slot);
    for application in &lowered.semantic_module.closed_conformance_applications {
        assert_eq!(application.rows.len(), 2);
        assert_eq!(application.realization_callables.len(), 2);
    }
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the joined family module encodes canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("the joined family module decodes"),
        lowered.semantic_module,
    );
}

#[test]
fn lowers_a_family_unit_call_without_a_scalar_result() {
    let checked = crate::front_end::checked_program(FAMILY_DYNAMIC_UNIT_SOURCE);
    let [Unit(Direct(plan))] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_slice()
    else {
        panic!("one checked family Unit plan expected")
    };
    assert_eq!(plan.family_tuple.as_ref(), [WIDTH_16.to_owned()]);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("the family Unit call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the family Unit module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [dispatch] = catalog.direct_dispatches.as_slice() else {
        panic!("one direct family Unit dispatch expected: {catalog:#?}")
    };
    assert_eq!(dispatch.family_tuple.as_slice(), [WIDTH_16]);
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one family Unit conformance application expected")
    };
    let [sixteen, thirty_two] = application.rows.as_slice() else {
        panic!("the family Unit requirement expands to one row per tuple")
    };
    assert_eq!(sixteen.family_tuple.as_slice(), [WIDTH_16]);
    assert_eq!(thirty_two.family_tuple.as_slice(), [WIDTH_32]);
    let [callable] = application.realization_callables.as_slice() else {
        panic!("one selected family Unit callable expected")
    };
    assert_eq!(
        callable.result,
        terminal_psi::ClosedConformanceCallableResult::Unit
    );
    assert_eq!(
        sixteen.realization_callable_identity.as_deref(),
        Some(callable.source_callable_identity.as_str())
    );
    assert!(thirty_two.realization_callable_identity.is_none());
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("family Unit caller");
    let [operation] = caller.blocks[0].operations.as_slice() else {
        panic!("one direct family Unit operation expected")
    };
    assert_eq!(operation.result, OperationResult::Unit);
    assert!(matches!(
        operation.kind,
        OperationKind::CallUnit { callee, .. } if callee == callable.machine
    ));

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the family Unit module encodes canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("the family Unit module decodes"),
        lowered.semantic_module,
    );
    assert_dynamic_unit_artifact_executes(&artifact);
}

#[test]
fn rejects_a_family_plan_whose_tuple_drifted_from_its_realization() {
    let mut checked = crate::front_end::checked_program(FAMILY_DYNAMIC_INTEGER_SOURCE);
    let [Scalar(Direct(plan))] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_mut_slice()
    else {
        unreachable!("checked above")
    };
    // Point the plan at the sibling tuple: the retained row then no longer
    // names the recorded specialization instance, so the join must reject
    // rather than let the 16-tuple row stand in for it.
    plan.family_tuple = Box::from([WIDTH_32.to_owned()]);
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic dispatch lost its exact selected conformance row"
    );
}
