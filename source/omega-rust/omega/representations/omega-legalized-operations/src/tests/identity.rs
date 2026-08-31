use super::*;

fn assert_identity_drift(
    original: LegalizedOperationPlanIdentity,
    corrupted: &LegalizedOperationPlan,
) {
    assert_ne!(legalized_operation_plan_identity(corrupted), original);
}

#[test]
fn call_aware_unit_identity_binds_semantic_and_target_custody() {
    let plan = call_aware_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_eq!(identity, legalized_operation_plan_identity(&plan));

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].recipe =
        StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].structural_types[0]
        .identity
        .push_str("::drift");
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call_plan
        .shadow_bytes += 8;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].parameters.swap(0, 1);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].parameters[0]
        .semantic
        .qualifications
        .clear();
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].parameters[0]
        .target
        .placement
        .locations
        .clear();
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .structural_places
        .swap(0, 1);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].entry_claims[0].claim = id::<ClaimId>(3);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .published_service_ceiling
        .push(id(1));
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .callee = id(3);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .arguments
        .swap(0, 1);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .arguments[0]
        .semantic
        .path
        .push(StructuralPathSegment::Field("base".into()));
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .arguments[0]
        .target
        .source_byte_offset = 8;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .claim_transfers
        .swap(0, 1);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .fuel[0]
        .units += 1;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .effect
        .output += 1;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .requirement_obligations[0] = psi_core::ObligationId::new(2).unwrap();
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .crash_continuations[0]
        .cause = psi_terminal::CrashCause::Abort;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    let OwnershipEvent::ClaimTransfer(claims) = &mut corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .ownership[0]
    else {
        panic!("call claim-transfer ownership");
    };
    claims.swap(0, 1);
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].call = None;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].return_fuel[0].units += 1;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0].return_effect.input += 1;
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    let OwnershipEvent::Cleanup(actions) =
        &mut corrupted.structural_unit_functions[0].return_ownership[0]
    else {
        panic!("return cleanup ownership");
    };
    actions.push(psi_terminal::TerminalAffineCleanupAction::DiscardRoot(id(
        1,
    )));
    assert_identity_drift(identity, &corrupted);
}

#[test]
fn call_aware_unit_roster_cannot_alias_value_less_unit_roster() {
    let call_aware = call_aware_plan();
    let call_aware_identity = legalized_operation_plan_identity(&call_aware);
    let function = &call_aware.structural_unit_functions[0];
    let mut erased = call_aware.clone();
    erased.structural_unit_functions.clear();
    erased.unit_functions.push(LegalizedUnitFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        recipe: UnitLegalizationRecipe::ReturnUnitV1,
        entry_block: function.entry_block,
        return_edge: function.return_edge,
        return_fuel: function.return_fuel.clone(),
    });
    assert_ne!(
        legalized_operation_plan_identity(&erased),
        call_aware_identity
    );
}
