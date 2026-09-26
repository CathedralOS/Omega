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
    FORWARDED_FAMILY_DYNAMIC_INTEGER_SOURCE, JOINED_FAMILY_DYNAMIC_BOOLEAN_SOURCE,
    REBOUND_FAMILY_DYNAMIC_INTEGER_SOURCE, assert_dynamic_unit_artifact_executes,
    unsupported_message,
};
use crate::TerminalMachineSelection;
use crate::tests::lower_machine;
use checked_trees::CheckedDynamicBinding::{Direct, Joined, Rebound};
use checked_trees::CheckedDynamicDispatchPlan::{Scalar, Unit};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, OperationResult};

const WIDTH_16: &str = "named(integer-const(16))";
const WIDTH_32: &str = "named(integer-const(32))";

/// Execute one direct family scalar artifact whose realization reads the
/// caller's one `i32` field, proving the tuple row's specialization instance
/// is the machine actually dispatched. The lane's entry machine returns
/// Unit, so completion is the observable result.
fn assert_family_integer_artifact_executes(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    field_value: i32,
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("family module decodes for execution");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("family entry machine");
    let [parameter] = entry.structural_parameters.as_slice() else {
        panic!("family entry requires one structural self parameter")
    };
    let [dispatch] = module.dynamic_dispatch.direct_dispatches.as_slice() else {
        panic!("direct family execution requires one direct dispatch")
    };
    let selection = module
        .dynamic_dispatch
        .selections
        .iter()
        .find(|selection| {
            selection.owner == dispatch.owner && selection.ordinal == dispatch.selection_ordinal
        })
        .expect("direct family dispatch retains its exact selection");
    let realization = module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("family dispatch realization machine");
    let field = realization
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::IntegerStructuralField { field, .. } => Some(field),
            _ => None,
        })
        .expect("family realization reads one i32 field");
    let argument = terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 1,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let value_field = terminal_interpreter::TerminalStructuralScalarFieldValue {
        argument_index: 0,
        path: selection.source.path.clone(),
        field,
        value: terminal_interpreter::TerminalScalarValue::Integer {
            scalar_type: semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Signed,
                32,
            )
            .expect("i32 is a valid Terminal scalar type"),
            value: semantic_vocabulary::IntegerValue::Signed(i128::from(field_value)),
        },
    };
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            scalar_fields: &[value_field],
            ..Default::default()
        },
    )
    .expect("family artifact starts");
    let mut meter = terminal_fuel::TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .expect("family artifact executes"),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit,
        ),
    );
}

#[test]
fn lowers_a_direct_family_call_to_the_selected_tuple_row() {
    let checked = crate::front_end::checked_program(FAMILY_DYNAMIC_INTEGER_SOURCE);
    let [Scalar(Direct(plan))] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_slice()
    else {
        panic!("one checked family plan expected")
    };
    assert_eq!(plan.family_tuple.as_ref(), [WIDTH_16.to_owned()]);
    // The checked plan names the tuple's specialization instance; the retained
    // selection row still names the provider template.
    let [row] = plan.selection.rows.as_slice() else {
        panic!("one checked selection row expected")
    };
    assert_ne!(row.realization_machine, plan.realization_machine);
    assert_eq!(plan.realization_callables.len(), 2);
    assert!(plan.realization_callables.iter().all(|callable| {
        !callable.family_tuple.is_empty() && callable.realization_machine != row.realization_machine
    }));

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("the family call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the family module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [dispatch] = catalog.direct_dispatches.as_slice() else {
        panic!("one direct family dispatch expected: {catalog:#?}")
    };
    assert_eq!(dispatch.family_tuple.as_slice(), [WIDTH_16]);
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one family conformance application expected")
    };
    let [sixteen, thirty_two] = application.rows.as_slice() else {
        panic!(
            "the family requirement expands to one row per declared tuple: {:?}",
            application.rows
        )
    };
    assert_eq!(sixteen.family_tuple.as_slice(), [WIDTH_16]);
    assert_eq!(thirty_two.family_tuple.as_slice(), [WIDTH_32]);
    assert_ne!(
        sixteen.realization_identity, thirty_two.realization_identity,
        "each tuple row names its own specialization instance state"
    );
    // A direct local call materializes only the selected tuple's instance;
    // the unselected roster row stays in the table as unbound evidence.
    let [callable] = application.realization_callables.as_slice() else {
        panic!("one selected family callable expected")
    };
    assert_eq!(
        sixteen.realization_callable_identity.as_deref(),
        Some(callable.source_callable_identity.as_str())
    );
    assert!(thirty_two.realization_callable_identity.is_none());
    assert_eq!(
        dispatch.realization_callable_identity,
        callable.source_callable_identity
    );
    assert_eq!(dispatch.realization, callable.machine);

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the family module encodes canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("the family module decodes"),
        lowered.semantic_module,
    );
    assert_family_integer_artifact_executes(&artifact, 17);
}

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
fn forwarded_family_call_exposes_every_tuple_row_on_the_parameter_interface() {
    let checked = crate::front_end::checked_program(FORWARDED_FAMILY_DYNAMIC_INTEGER_SOURCE);
    let [Scalar(Direct(plan))] = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .calls
        .as_slice()
    else {
        panic!("one checked forwarded family plan expected")
    };
    assert!(matches!(
        plan.origin,
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
    ));
    assert_eq!(plan.family_tuple.as_ref(), [WIDTH_16.to_owned()]);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("the forwarded family call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the forwarded family module verifies");
    let catalog = &lowered.semantic_module.dynamic_dispatch;
    let [parameter] = catalog.parameters.as_slice() else {
        panic!("one forwarded family parameter expected: {catalog:#?}")
    };
    let [argument] = catalog.arguments.as_slice() else {
        panic!("one forwarded family argument expected")
    };
    let [dispatch] = catalog.parameter_dispatches.as_slice() else {
        panic!("one forwarded family parameter dispatch expected")
    };
    // The parameter interface is the complete family surface: one requirement
    // per roster tuple, and the dispatch selects the call's tuple slot.
    let [sixteen, thirty_two] = parameter.requirements.as_slice() else {
        panic!("the parameter interface expands to one requirement per tuple")
    };
    assert_eq!(sixteen.family_tuple.as_slice(), [WIDTH_16]);
    assert_eq!(thirty_two.family_tuple.as_slice(), [WIDTH_32]);
    assert_eq!(dispatch.requirement_slot, sixteen.slot);
    assert_eq!(
        argument.source,
        terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal: 0 }
    );
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one forwarded family conformance application expected")
    };
    assert_eq!(application.rows.len(), 2);
    assert!(
        application
            .rows
            .iter()
            .all(|row| row.realization_callable_identity.is_some()),
        "every tuple row binds a callable on a descriptor interface: {:?}",
        application.rows
    );

    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the forwarded family module encodes canonically")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("the forwarded family module decodes"),
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
