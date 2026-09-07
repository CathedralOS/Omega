use super::*;

mod scalar_control;
mod u64_equal_zero_parameter;
mod u64_not_equal_zero_parameter;

fn assert_identity_drift(
    original: LegalizedOperationPlanIdentity,
    corrupted: &LegalizedOperationPlan,
) {
    assert_ne!(legalized_operation_plan_identity(corrupted), original);
}

fn scalar_return_mut(function: &mut LegalizedScalarFunction) -> &mut LegalizedScalarReturn {
    let LegalizedScalarTerminator::Return(returned) = &mut function.blocks[0].terminator else {
        panic!("return fixture")
    };
    returned
}

#[test]
fn scalar_graph_identity_binds_ordered_source_and_abi_custody() {
    let plan = scalar_call_unit_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_eq!(identity, legalized_operation_plan_identity(&plan));
    let mut empty = plan.clone();
    empty.scalar_functions.clear();
    assert_ne!(legalized_operation_plan_identity(&empty), identity);
    assert_ne!(
        legalized_operation_plan_identity_v16_legacy(&empty),
        legalized_operation_plan_identity_v16_legacy(&plan),
        "historical fingerprints cannot erase a new graph"
    );
    for mutation in 0..22 {
        let mut changed = plan.clone();
        let function = &mut changed.scalar_functions[0];
        match mutation {
            0 => function.attachment = Some(id(999)),
            1 => {
                function.blocks[0].instructions[0].kind =
                    LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(8))
            }
            2 => function.blocks[0].instructions.swap(0, 1),
            3 => call_mut(function, 0).callee = id(999),
            4 => call_mut(function, 0).arguments.swap(0, 1),
            5 => function.blocks[0].instructions[4].result = id(999),
            6 => function.blocks[0].instructions[3].fuel[0].units += 1,
            7 => function.blocks[0].instructions[3].effect.output += 1,
            8 => scalar_return_mut(function).edge = id(999),
            9 => scalar_return_mut(function).ownership.clear(),
            10 => function.call_plan.shadow_bytes += 8,
            11 => function.entry_block = id(999),
            12 => function.blocks[0].id = id(999),
            13 => {
                scalar_return_mut(function).value = LegalizedScalarReturnValue::Value {
                    value: id(114),
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                }
            }
            14 => {
                function.blocks[0].instructions[0].definition_site =
                    optimization_unit::ValueDefinitionSite::FunctionParameter(0)
            }
            15 => call_mut(function, 0).result_placement.locations.clear(),
            16 => call_mut(function, 0).requirement_obligations.push(id(999)),
            17 => call_mut(function, 0).arguments[0].source = id(999),
            18 => scalar_return_mut(function).fuel[0].units += 1,
            19 => scalar_return_mut(function).effect.input += 1,
            20 => function.blocks[0].instructions.pop().map(|_| ()).unwrap(),
            _ => function.provenance.operations.reverse(),
        }
        assert_identity_drift(identity, &changed);
    }
    let mut parameterized = plan.clone();
    parameterized.scalar_functions[0]
        .parameters
        .push(LegalizedScalarParameter {
            value: id(700),
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(0),
            placement: call_mut(&mut plan.clone().scalar_functions[0], 0).arguments[0]
                .placement
                .clone(),
        });
    let identity = legalized_operation_plan_identity(&parameterized);
    for mutation in 0..4 {
        let mut changed = parameterized.clone();
        let parameter = &mut changed.scalar_functions[0].parameters[0];
        match mutation {
            0 => parameter.value = id(701),
            1 => parameter.scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            2 => {
                parameter.definition_site =
                    optimization_unit::ValueDefinitionSite::FunctionParameter(1)
            }
            _ => parameter.placement.locations.clear(),
        }
        assert_identity_drift(identity, &changed);
    }
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
        .requirement_obligations[0] = semantic_vocabulary::ObligationId::new(2).unwrap();
    assert_identity_drift(identity, &corrupted);

    let mut corrupted = plan.clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .expect("call")
        .crash_continuations[0]
        .cause = terminal_psi::CrashCause::Abort;
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
    actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(id(
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
    erased.scalar_functions.push(LegalizedScalarFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        call_plan: evaluate_call_plan(
            CallingPolicy::native_for_target(erased.target),
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap(),
        parameters: Vec::new(),
        entry_block: function.entry_block,
        blocks: vec![LegalizedScalarBlock {
            id: function.entry_block,
            parameters: vec![],
            instructions: Vec::new(),
            terminator: LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: function.return_edge,
                value: LegalizedScalarReturnValue::Unit,
                fuel: function.return_fuel.clone(),
                effect: function.return_effect,
                ownership: function.return_ownership.clone(),
            }),
        }],
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
        definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(0),
    };
    let right_parameter = LegalizedConditionParameter {
        source_value: right,
        parameter_index: 1,
        register: MachineRegister::X86Rsi,
        definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(1),
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
            definition_site: optimization_unit::ValueDefinitionSite::Node { block, node: 0 },
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
    plan.functions.push(LegalizedFunction::Conditional(
        LegalizedConditionalFunction {
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
                result_definition_site: optimization_unit::ValueDefinitionSite::Node {
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
        },
    ));
    let identity = legalized_operation_plan_identity(&plan);

    let mut reversed = plan.clone();
    let LegalizedCondition::IntegerLessThanParametersV1 { left, right, .. } =
        &mut reversed.functions[0].conditional_mut().condition
    else {
        panic!("strict less-than fixture")
    };
    std::mem::swap(left, right);
    assert_ne!(legalized_operation_plan_identity(&reversed), identity);

    let mut equality = plan.clone();
    equality.functions[0].conditional_mut().recipe =
        LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1;
    equality.functions[0].conditional_mut().condition =
        LegalizedCondition::IntegerEqualParametersV1 {
            operation: comparison,
            result_definition_site: optimization_unit::ValueDefinitionSite::Node {
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
    inclusive.functions[0].conditional_mut().recipe =
        LegalizationRecipe::ReturnU64IntegerLessOrEqualParametersConditionalV1;
    inclusive.functions[0].conditional_mut().condition =
        LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation: comparison,
            result_definition_site: optimization_unit::ValueDefinitionSite::Node {
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
    let inclusive_identity = legalized_operation_plan_identity(&inclusive);
    assert_ne!(inclusive_identity, identity);
    assert_ne!(
        inclusive_identity,
        legalized_operation_plan_identity(&equality)
    );

    let mut inclusive_reversed = inclusive.clone();
    let LegalizedCondition::IntegerLessOrEqualParametersV1 { left, right, .. } =
        &mut inclusive_reversed.functions[0].conditional_mut().condition
    else {
        panic!("inclusive comparison fixture")
    };
    std::mem::swap(left, right);
    assert_ne!(
        legalized_operation_plan_identity(&inclusive_reversed),
        inclusive_identity
    );

    let equality_result = id::<ValueId>(36);
    let boolean_not = id::<OperationId>(37);
    let mut not_equal = equality.clone();
    not_equal.functions[0].conditional_mut().recipe =
        LegalizationRecipe::ReturnU64IntegerNotEqualParametersConditionalV1;
    not_equal.functions[0]
        .conditional_mut()
        .provenance
        .operations = vec![comparison, boolean_not, true_constant, false_constant];
    not_equal.functions[0].conditional_mut().condition =
        LegalizedCondition::IntegerNotEqualParametersV1 {
            equality_operation: comparison,
            equality_result,
            equality_result_definition_site: optimization_unit::ValueDefinitionSite::Node {
                block: entry,
                node: 0,
            },
            equality_fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(comparison),
                units: 1,
            }],
            boolean_not_operation: boolean_not,
            boolean_not_result: condition,
            boolean_not_result_definition_site: optimization_unit::ValueDefinitionSite::Node {
                block: entry,
                node: 1,
            },
            boolean_not_fuel: vec![FuelSettlement {
                site: PsiProvenance::Operation(boolean_not),
                units: 1,
            }],
            left: left_parameter,
            right: right_parameter,
        };
    let not_equal_identity = legalized_operation_plan_identity(&not_equal);
    assert_ne!(
        not_equal_identity,
        legalized_operation_plan_identity(&equality)
    );
    assert_ne!(not_equal_identity, inclusive_identity);

    let mut not_equal_reversed = not_equal.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 { left, right, .. } =
        &mut not_equal_reversed.functions[0].conditional_mut().condition
    else {
        panic!("not-equal comparison fixture")
    };
    std::mem::swap(left, right);
    assert_ne!(
        legalized_operation_plan_identity(&not_equal_reversed),
        not_equal_identity
    );

    let mut corrupted_not_result = not_equal.clone();
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        boolean_not_result, ..
    } = &mut corrupted_not_result.functions[0]
        .conditional_mut()
        .condition
    else {
        panic!("not-equal comparison fixture")
    };
    *boolean_not_result = equality_result;
    assert_ne!(
        legalized_operation_plan_identity(&corrupted_not_result),
        not_equal_identity
    );

    assert_eq!(
        legalized_operation_plan_identity_v14_legacy(&plan),
        legalized_operation_plan_identity_v14_legacy(&plan.clone())
    );
    assert_ne!(
        legalized_operation_plan_identity_v14_legacy(&plan),
        legalized_operation_plan_identity(&plan)
    );
    assert_eq!(
        legalized_operation_plan_identity_v15_legacy(&plan),
        legalized_operation_plan_identity_v15_legacy(&plan.clone())
    );
    assert_ne!(
        legalized_operation_plan_identity_v15_legacy(&plan),
        legalized_operation_plan_identity(&plan)
    );

    let mut signed = plan.clone();
    signed.functions[0].conditional_mut().recipe =
        LegalizationRecipe::ReturnU64I64LessThanParametersConditionalV1;
    let LegalizedCondition::IntegerLessThanParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left,
        right,
    } = signed.functions[0].conditional_mut().condition.clone()
    else {
        panic!("strict less-than fixture")
    };
    signed.functions[0].conditional_mut().condition = LegalizedCondition::I64LessThanParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left,
        right,
    };
    let signed_identity = legalized_operation_plan_identity(&signed);
    assert_ne!(signed_identity, identity);
    let mut signed_reversed = signed.clone();
    let LegalizedCondition::I64LessThanParametersV1 { left, right, .. } =
        &mut signed_reversed.functions[0].conditional_mut().condition
    else {
        panic!("signed strict less-than fixture")
    };
    std::mem::swap(left, right);
    assert_ne!(
        legalized_operation_plan_identity(&signed_reversed),
        signed_identity
    );

    let mut signed_inclusive = signed.clone();
    signed_inclusive.functions[0].conditional_mut().recipe =
        LegalizationRecipe::ReturnU64I64LessOrEqualParametersConditionalV1;
    let LegalizedCondition::I64LessThanParametersV1 {
        operation,
        result_definition_site,
        fuel,
        left,
        right,
    } = signed_inclusive.functions[0]
        .conditional_mut()
        .condition
        .clone()
    else {
        panic!("signed strict less-than fixture")
    };
    signed_inclusive.functions[0].conditional_mut().condition =
        LegalizedCondition::I64LessOrEqualParametersV1 {
            operation,
            result_definition_site,
            fuel,
            left,
            right,
        };
    let signed_inclusive_identity = legalized_operation_plan_identity(&signed_inclusive);
    assert_ne!(signed_inclusive_identity, signed_identity);
    assert_ne!(signed_inclusive_identity, inclusive_identity);

    let mut corruptions = Vec::new();
    let mut corrupted = signed_inclusive.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 { operation, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!()
    };
    *operation = id(99);
    corruptions.push(corrupted);
    let mut corrupted = signed_inclusive.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 {
        result_definition_site,
        ..
    } = &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!()
    };
    *result_definition_site = optimization_unit::ValueDefinitionSite::FunctionParameter(0);
    corruptions.push(corrupted);
    let mut corrupted = signed_inclusive.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 { fuel, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!()
    };
    fuel[0].units += 1;
    corruptions.push(corrupted);
    let mut corrupted = signed_inclusive.clone();
    let LegalizedCondition::I64LessOrEqualParametersV1 { left, right, .. } =
        &mut corrupted.functions[0].conditional_mut().condition
    else {
        unreachable!()
    };
    std::mem::swap(left, right);
    corruptions.push(corrupted);
    for corrupted in &corruptions {
        assert_ne!(
            legalized_operation_plan_identity(corrupted),
            signed_inclusive_identity
        );
    }

    assert_eq!(
        legalized_operation_plan_identity_v20_legacy(&plan),
        legalized_operation_plan_identity_v20_legacy(&plan.clone())
    );
    assert_ne!(
        legalized_operation_plan_identity_v20_legacy(&plan),
        legalized_operation_plan_identity(&plan)
    );
    assert_eq!(
        legalized_operation_plan_identity_v17_legacy(&plan),
        legalized_operation_plan_identity_v17_legacy(&plan.clone())
    );
    assert_ne!(
        legalized_operation_plan_identity_v17_legacy(&plan),
        legalized_operation_plan_identity(&plan)
    );
}

fn call_mut(function: &mut LegalizedScalarFunction, index: usize) -> &mut LegalizedScalarCall {
    function.blocks[0]
        .instructions
        .iter_mut()
        .filter_map(|instruction| match &mut instruction.kind {
            LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .nth(index)
        .expect("call fixture")
}
