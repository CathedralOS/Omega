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

#[test]
fn strict_less_than_condition_has_distinct_ordered_identity() {
    let mut plan = call_aware_plan();
    plan.structural_unit_functions.clear();
    let machine = id(20);
    let entry = id::<BlockId>(21);
    let true_block = id::<BlockId>(22);
    let false_block = id::<BlockId>(23);
    let left = id::<ValueId>(24);
    let right = id::<ValueId>(25);
    let condition = id::<ValueId>(26);
    let comparison = id::<OperationId>(27);
    let true_edge = id::<EdgeId>(28);
    let false_edge = id::<EdgeId>(29);
    let true_return = id::<EdgeId>(30);
    let false_return = id::<EdgeId>(31);
    let left_parameter = LegalizedConditionParameter {
        source_value: left,
        parameter_index: 0,
        register: MachineRegister::X86Rdi,
        definition_site: omega_optimization_unit::ValueDefinitionSite::FunctionParameter(0),
    };
    let right_parameter = LegalizedConditionParameter {
        source_value: right,
        parameter_index: 1,
        register: MachineRegister::X86Rsi,
        definition_site: omega_optimization_unit::ValueDefinitionSite::FunctionParameter(1),
    };
    let comparison_fuel = vec![FuelSettlement {
        site: PsiProvenance::Operation(comparison),
        units: 1,
    }];
    let leaf = |source_value, value, operation, block, edge| LegalizedLeaf {
        return_edge: edge,
        source_value,
        return_fuel: vec![FuelSettlement {
            site: PsiProvenance::Edge(edge),
            units: 1,
        }],
        value: LegalizedLeafValue::Immediate {
            value: IntegerValue::Unsigned(value),
            constant_operation: operation,
            definition_site: omega_optimization_unit::ValueDefinitionSite::Node { block, node: 0 },
            constant_fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(operation),
                units: 1,
            }],
        },
    };
    let true_value = id::<ValueId>(32);
    let false_value = id::<ValueId>(33);
    let true_constant = id::<OperationId>(34);
    let false_constant = id::<OperationId>(35);
    plan.functions.push(LegalizedFunction {
        machine,
        attachment: None,
        provenance: TerminalPsiProvenance {
            operations: vec![comparison, true_constant, false_constant],
            edges: vec![true_edge, false_edge, true_return, false_return],
        },
        recipe: LegalizationRecipe::ReturnU64IntegerLessThanParametersConditionalV1,
        condition_source: condition,
        condition: LegalizedCondition::IntegerLessThanParametersV1 {
            operation: comparison,
            result_definition_site: omega_optimization_unit::ValueDefinitionSite::Node {
                block: entry,
                node: 0,
            },
            fuel: comparison_fuel,
            left: left_parameter.clone(),
            right: right_parameter.clone(),
        },
        entry_block: entry,
        true_block,
        false_block,
        branch_true_edge: true_edge,
        branch_false_edge: false_edge,
        branch_true_fuel: vec![FuelSettlement {
            site: PsiProvenance::Edge(true_edge),
            units: 1,
        }],
        branch_false_fuel: vec![FuelSettlement {
            site: PsiProvenance::Edge(false_edge),
            units: 1,
        }],
        branch_true_bindings: Vec::new(),
        branch_false_bindings: Vec::new(),
        when_true: leaf(true_value, 7, true_constant, true_block, true_return),
        when_false: leaf(false_value, 9, false_constant, false_block, false_return),
    });
    let identity = legalized_operation_plan_identity(&plan);

    let mut reversed = plan.clone();
    let LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. } =
        &mut reversed.functions[0].condition
    else {
        panic!("strict less-than fixture")
    };
    std::mem::swap(left, right);
    assert_ne!(legalized_operation_plan_identity(&reversed), identity);

    let mut equality = plan.clone();
    equality.functions[0].recipe = LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1;
    equality.functions[0].condition = LegalizedCondition::IntegerEqualParametersV1 {
        operation: comparison,
        result_definition_site: omega_optimization_unit::ValueDefinitionSite::Node {
            block: entry,
            node: 0,
        },
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(comparison),
            units: 1,
        }],
        left: left_parameter.clone(),
        right: right_parameter.clone(),
    };
    assert_ne!(legalized_operation_plan_identity(&equality), identity);

    let mut inclusive = plan.clone();
    inclusive.functions[0].recipe =
        LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1;
    inclusive.functions[0].condition = LegalizedCondition::IntegerLessOrEqualParametersV1 {
        operation: comparison,
        result_definition_site: omega_optimization_unit::ValueDefinitionSite::Node {
            block: entry,
            node: 0,
        },
        fuel: vec![FuelSettlement {
            site: PsiProvenance::Operation(comparison),
            units: 1,
        }],
        left: left_parameter,
        right: right_parameter,
    };
    let inclusive_identity = legalized_operation_plan_identity(&inclusive);
    assert_ne!(inclusive_identity, identity);
    assert_ne!(
        inclusive_identity,
        legalized_operation_plan_identity(&equality)
    );

    let mut inclusive_reversed = inclusive.clone();
    let LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. } =
        &mut inclusive_reversed.functions[0].condition
    else {
        panic!("inclusive comparison fixture")
    };
    std::mem::swap(left, right);
    assert_ne!(
        legalized_operation_plan_identity(&inclusive_reversed),
        inclusive_identity
    );

    assert_eq!(
        legalized_operation_plan_identity_v14_legacy(&plan),
        legalized_operation_plan_identity_v14_legacy(&plan.clone())
    );
    assert_ne!(
        legalized_operation_plan_identity_v14_legacy(&plan),
        legalized_operation_plan_identity(&plan)
    );
}
