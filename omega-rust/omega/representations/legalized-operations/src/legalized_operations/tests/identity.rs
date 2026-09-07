use super::*;

mod scalar_control;
mod scalar_operations;

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
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
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
            1 => {
                parameter.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
            }
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
