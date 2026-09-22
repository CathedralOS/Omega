use super::{
    DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE, DIRECT_DYNAMIC_INTEGER_STORE_SOURCE,
    MUTATING_REALIZATION_SOURCE, PROJECTED_MUTATING_REALIZATION_SOURCE, direct_dynamic_checked,
    direct_plan, direct_plan_mut, unsupported_message,
};
use crate::TerminalMachineSelection;
use crate::tests::{checked_source, checked_source_with_core_service, lower_machine};
use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, Terminator};

#[test]
fn lowers_exact_named_dynamic_field_call_without_selecting_ambient_lookalike() {
    let checked = direct_dynamic_checked();
    let plan = direct_plan(&checked);
    let selected_callable_identity = plan.realization_identity.clone();
    let source_type_identity = plan.source_type_identity.clone();
    let ambient = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::measure")
        .expect("ambient lookalike");
    let ambient_callable_identity = checked
        .typed
        .normalized_machine_overload_identity(ambient)
        .expect("ambient callable identity")
        .identity();
    assert_ne!(ambient.symbol, plan.realization_machine);
    assert_ne!(ambient_callable_identity, selected_callable_identity);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("direct dynamic call lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("lowered direct dynamic module verifies");
    let module = &lowered.semantic_module;
    assert_eq!(module.machines.len(), 2);
    let [application] = module.closed_conformance_applications.as_slice() else {
        panic!("one exact conformance application expected")
    };
    assert_eq!(
        application.subject_identity.as_deref(),
        Some(source_type_identity.as_str())
    );
    let [selection] = module.dynamic_dispatch.selections.as_slice() else {
        panic!("one dynamic selection expected")
    };
    assert_eq!(selection.source.path.len(), 1);
    assert_eq!(
        selection.source.access,
        terminal_psi::StructuralAccess::SharedBorrow
    );
    let [dispatch] = module.dynamic_dispatch.direct_dispatches.as_slice() else {
        panic!("one direct dynamic dispatch expected")
    };
    assert_eq!(
        dispatch.realization_callable_identity,
        selected_callable_identity
    );
    assert!(application.realization_callables.iter().any(|callable| {
        callable.source_callable_identity == selected_callable_identity
            && callable.machine == dispatch.realization
    }));
    assert!(
        !application
            .realization_callables
            .iter()
            .any(|callable| callable.source_callable_identity == ambient_callable_identity)
    );
    let realization = module
        .machines
        .iter()
        .find(|machine| machine.id == dispatch.realization)
        .expect("selected realization machine");
    assert!(
        realization
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::BooleanStructuralField { .. }
            ))
    );
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                OperationKind::BooleanConstant { value: false }
            ))
    );

    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("direct dynamic module has canonical source-free encoding")
    .into_artifact();
}

#[test]
fn rejects_ambient_lookalike_substitution_in_checked_plan() {
    let mut checked = direct_dynamic_checked();
    let ambient = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Item::measure")
        .expect("ambient lookalike")
        .clone();
    let [ambient_state] = checked.typed.machine_states(&ambient) else {
        panic!("ambient lookalike has one state")
    };
    let ambient_state = ambient_state.clone();
    let ambient_identity = checked
        .typed
        .normalized_machine_overload_identity(&ambient)
        .expect("ambient identity")
        .identity();
    let ambient_contract = checked
        .facts
        .contract_plans
        .for_machine(ambient.symbol)
        .expect("ambient contract")
        .clone();
    let plan = direct_plan_mut(&mut checked);
    plan.realization_machine = ambient.symbol;
    plan.realization_state = ambient_state.symbol;
    plan.realization_identity = ambient_identity;
    plan.realization_contract_report_fingerprint = ambient_contract.report_fingerprint;
    plan.realization_contract_commitment = ambient_contract.commitment;

    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic dispatch lost its exact selected conformance row"
    );
}

#[test]
fn rejects_source_path_and_machine_contract_tampering() {
    let mut source_path = direct_dynamic_checked();
    direct_plan_mut(&mut source_path).source_path.clear();
    assert_eq!(
        unsupported_message(&source_path),
        "direct dynamic source must be one exact attachment field"
    );

    let mut contract = direct_dynamic_checked();
    direct_plan_mut(&mut contract).realization_contract_commitment =
        checked_trees::MachineContractCommitment::from_digest([0xA5; 32]);
    assert_eq!(
        unsupported_message(&contract),
        "direct dynamic machine requires an unsupported contract lane"
    );

    let mut caller_contract = direct_dynamic_checked();
    direct_plan_mut(&mut caller_contract).caller_contract_commitment =
        checked_trees::MachineContractCommitment::from_digest([0x5A; 32]);
    assert_eq!(
        unsupported_message(&caller_contract),
        "direct dynamic machine requires an unsupported contract lane"
    );
}

#[test]
fn lowers_checked_integer_field_store_through_the_selected_dynamic_realization() {
    let checked = checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    let plan = direct_plan(&checked);
    let store = plan
        .caller_structural_scalar_field_store
        .as_ref()
        .expect("checked caller field store");
    assert_eq!(store.field_identity, "value");

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("integer store route lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("integer store route verifies");
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("caller machine");
    assert_eq!(
        caller.structural_parameters[0].access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert!(matches!(
        caller.blocks[0].operations.as_slice(),
        [
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::StructuralScalarFieldStore { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::CallStructuralScalar { .. },
                ..
            }
        ]
    ));
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .expect("selected realization");
    assert!(matches!(
        realization.blocks[0].operations.as_slice(),
        [terminal_psi::Operation {
            kind: OperationKind::IntegerStructuralField { .. },
            ..
        }]
    ));

    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("integer store route has canonical source-free encoding")
    .into_artifact();
}

#[test]
fn lowers_checked_mutating_dynamic_realization_before_its_scalar_return() {
    let checked = checked_source(MUTATING_REALIZATION_SOURCE);
    let plan = direct_plan(&checked);
    assert_eq!(plan.realization_structural_scalar_field_stores.len(), 3);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("mutating realization lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("mutating realization module verifies");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .expect("selected realization");
    assert_eq!(
        realization.structural_parameters[0].access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert!(matches!(
        realization.blocks[0].operations.as_slice(),
        [
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::StructuralScalarFieldStore { path, .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::BooleanConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::StructuralScalarFieldStore { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::StructuralScalarFieldStore { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::IntegerStructuralField { .. },
                ..
            }
        ] if path.is_empty()
    ));
}

#[test]
fn lowers_nested_projected_mutating_realization_path_before_its_scalar_return() {
    let checked = checked_source(PROJECTED_MUTATING_REALIZATION_SOURCE);
    let plan = direct_plan(&checked);
    let [store] = plan.realization_structural_scalar_field_stores.as_slice() else {
        panic!("checked projected realization field store expected")
    };
    assert_eq!(
        store.carrier_path,
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field("envelope".into()),
            checked_trees::CheckedUnitStructuralPathSegment::Field("payload".into()),
        ]
    );
    assert_eq!(store.field_identity, "value");
    assert_eq!(store.primitive_type, typed_trees::types::PrimitiveType::U16);

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("projected realization lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("projected realization module verifies");
    let realization = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .expect("selected realization");
    assert!(matches!(
        realization.blocks[0].operations.as_slice(),
        [
            terminal_psi::Operation {
                kind: OperationKind::IntegerConstant { .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::StructuralScalarFieldStore { path, .. },
                ..
            },
            terminal_psi::Operation {
                kind: OperationKind::IntegerStructuralField { .. },
                ..
            }
        ] if path == &[
            terminal_psi::StructuralPathSegment::Field("envelope".into()),
            terminal_psi::StructuralPathSegment::Field("payload".into()),
        ]
    ));

    let mut redirected = lowered.semantic_module.clone();
    let redirected_store = redirected
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::StructuralScalarFieldStore { path, .. } => Some(path),
            _ => None,
        })
        .expect("nested store operation");
    redirected_store[1] = terminal_psi::StructuralPathSegment::Field("missing".into());
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::InvalidStructuralScalarFieldStore { .. })
    ));
}

#[test]
fn rejects_mutating_realization_body_that_drifted_from_checked_custody() {
    let mut checked = checked_source(MUTATING_REALIZATION_SOURCE);
    direct_plan_mut(&mut checked)
        .realization_structural_scalar_field_stores
        .first_mut()
        .expect("checked realization field store")
        .field_identity = "missing".into();

    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic selected body drifted from checked custody"
    );
}

#[test]
fn rejects_mutating_realization_store_order_drift() {
    let mut checked = checked_source(MUTATING_REALIZATION_SOURCE);
    direct_plan_mut(&mut checked)
        .realization_structural_scalar_field_stores
        .swap(0, 1);

    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic selected body drifted from checked custody"
    );
}

#[test]
fn rejects_third_mutating_realization_store_identity_drift() {
    let mut checked = checked_source(MUTATING_REALIZATION_SOURCE);
    direct_plan_mut(&mut checked)
        .realization_structural_scalar_field_stores
        .get_mut(2)
        .expect("third checked realization field store")
        .field_identity = "enabled".into();

    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic selected body drifted from checked custody"
    );
}

#[test]
fn lowers_dynamic_scalar_result_into_console_effect_control() {
    let checked = checked_source_with_core_service(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    assert!(
        direct_plan(&checked)
            .caller_structural_scalar_field_store
            .is_none()
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("direct dynamic result control lowers as one module");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("direct dynamic result control verifies");
    assert_eq!(lowered.semantic_module.machines.len(), 2);
    assert_eq!(lowered.semantic_module.boundary_machines.len(), 1);
    let caller = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("caller machine");
    assert_eq!(caller.blocks.len(), 3);
    assert!(matches!(
        caller.blocks[0].terminator,
        Terminator::Conditional { .. }
    ));
    assert_eq!(
        caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
            .count(),
        2
    );
    assert_eq!(lowered.source_call_occurrences.len(), 3);
    let _artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::run"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("direct dynamic result control has canonical source-free encoding")
    .into_artifact();

    let mut wrong_self = lowered.semantic_module.clone();
    let realization_attachment = wrong_self
        .machines
        .iter()
        .find(|machine| machine.id != wrong_self.entry)
        .and_then(|machine| machine.attachment)
        .expect("realization attachment");
    let caller = wrong_self
        .machines
        .iter_mut()
        .find(|machine| machine.id == wrong_self.entry)
        .expect("caller machine");
    caller.structural_parameters[0].structural_type = realization_attachment;
    let verification_error = terminal_verifier::validate_module(&wrong_self)
        .expect_err("provider specialization rejects a mismatched self type");
    assert!(
        matches!(
            verification_error,
            terminal_verifier::ModuleError::InvalidStructuralSelfParameter { .. }
        ),
        "unexpected error: {verification_error:?}"
    );
    let codec_error = terminal_codec::encode_module(&wrong_self)
        .expect_err("canonical encoding rejects a mismatched provider self type");
    assert!(
        matches!(
            codec_error,
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "provider-backed attachment specialization is incomplete"
            )
        ),
        "unexpected error: {codec_error:?}"
    );
}

#[test]
fn rejects_tampered_checked_dynamic_store_custody() {
    let mut guard = checked_source_with_core_service(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    direct_plan_mut(&mut guard)
        .unit_continuation
        .as_mut()
        .expect("checked dynamic result continuation")
        .guard =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(true)));
    assert_eq!(
        unsupported_message(&guard),
        "direct dynamic continuation guard drifted from checked scalar facts"
    );

    let store = direct_plan(&checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE))
        .caller_structural_scalar_field_store
        .clone()
        .expect("checked caller field store");
    let mut combined = checked_source_with_core_service(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    direct_plan_mut(&mut combined).caller_structural_scalar_field_store = Some(store);
    assert_eq!(
        unsupported_message(&combined),
        "direct dynamic result control cannot also retain a caller field store"
    );

    let mut checked = checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    direct_plan_mut(&mut checked)
        .caller_structural_scalar_field_store
        .as_mut()
        .expect("checked caller field store")
        .field_identity = "missing".into();
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic store field is absent or ambiguous"
    );

    let mut checked = checked_source(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    direct_plan_mut(&mut checked)
        .caller_structural_scalar_field_store
        .as_mut()
        .expect("checked caller field store")
        .carrier_path
        .clear();
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic caller store drifted from checked custody"
    );
}
