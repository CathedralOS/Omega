use super::{
    DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE, DIRECT_DYNAMIC_INTEGER_STORE_SOURCE,
    MUTATING_REALIZATION_SOURCE, PROJECTED_MUTATING_REALIZATION_SOURCE, direct_dynamic_checked,
    direct_plan, direct_plan_mut, unsupported_message,
};
use crate::TerminalMachineSelection;
use crate::tests::{checked_source_with_core_service, lower_machine};
use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

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
    let checked = crate::front_end::checked_program(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
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
    let checked = crate::front_end::checked_program(MUTATING_REALIZATION_SOURCE);
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
    let checked = crate::front_end::checked_program(PROJECTED_MUTATING_REALIZATION_SOURCE);
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
    let mut checked = crate::front_end::checked_program(MUTATING_REALIZATION_SOURCE);
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

    let store = direct_plan(&crate::front_end::checked_program(
        DIRECT_DYNAMIC_INTEGER_STORE_SOURCE,
    ))
    .caller_structural_scalar_field_store
    .clone()
    .expect("checked caller field store");
    let mut combined = checked_source_with_core_service(DIRECT_DYNAMIC_INTEGER_CONTROL_SOURCE);
    direct_plan_mut(&mut combined).caller_structural_scalar_field_store = Some(store);
    assert_eq!(
        unsupported_message(&combined),
        "direct dynamic result control cannot also retain a caller field store"
    );

    let mut checked = crate::front_end::checked_program(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
    direct_plan_mut(&mut checked)
        .caller_structural_scalar_field_store
        .as_mut()
        .expect("checked caller field store")
        .field_identity = "missing".into();
    assert_eq!(
        unsupported_message(&checked),
        "direct dynamic store field is absent or ambiguous"
    );

    let mut checked = crate::front_end::checked_program(DIRECT_DYNAMIC_INTEGER_STORE_SOURCE);
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
