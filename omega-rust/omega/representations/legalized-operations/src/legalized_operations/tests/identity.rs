use super::*;

mod byte_literals;
mod primitive_store;
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
            5 => {
                function.blocks[0].instructions[4]
                    .result
                    .as_mut()
                    .unwrap()
                    .value = id(999)
            }
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
                    scalar_type: semantic_vocabulary::ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                }
            }
            14 => {
                function.blocks[0].instructions[0]
                    .result
                    .as_mut()
                    .unwrap()
                    .definition_site = optimization_unit::ValueDefinitionSite::FunctionParameter(0)
            }
            15 => call_mut(function, 0)
                .result_placement
                .as_mut()
                .unwrap()
                .locations
                .clear(),
            16 => call_mut(function, 0).requirement_obligations.push(id(999)),
            17 => *scalar_argument_mut(&mut call_mut(function, 0).arguments[0]).0 = id(999),
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
                .placement()
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
    for mutation in 0..23 {
        let mut changed = plan.clone();
        match mutation {
            0 => changed.scalar_functions[0].structural = None,
            1 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .structural_types[0]
                .identity
                .push_str("::drift"),
            2 => changed.scalar_functions[0].call_plan.shadow_bytes += 8,
            3 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters
                .swap(0, 1),
            4 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters[0]
                .semantic
                .qualifications
                .clear(),
            5 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .parameters[0]
                .target
                .placement
                .locations
                .clear(),
            6 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .structural_places
                .swap(0, 1),
            7 => {
                changed.scalar_functions[0]
                    .structural
                    .as_mut()
                    .unwrap()
                    .entry_claims[0]
                    .claim = id::<ClaimId>(3)
            }
            8 => changed.scalar_functions[0]
                .structural
                .as_mut()
                .unwrap()
                .published_service_ceiling
                .push(id(1)),
            9 => structural_call_mut(&mut changed).callee = id(3),
            10 => structural_call_mut(&mut changed).arguments.swap(0, 1),
            11 => structural_argument_mut(&mut structural_call_mut(&mut changed).arguments[0])
                .0
                .path
                .push(StructuralPathSegment::Field("base".into())),
            12 => {
                structural_argument_mut(&mut structural_call_mut(&mut changed).arguments[0])
                    .1
                    .source_byte_offset = 8
            }
            13 => structural_call_mut(&mut changed).claim_transfers.swap(0, 1),
            14 => changed.scalar_functions[0].blocks[0].instructions[0].fuel[0].units += 1,
            15 => {
                changed.scalar_functions[0].blocks[0].instructions[0]
                    .effect
                    .output += 1
            }
            16 => structural_call_mut(&mut changed).requirement_obligations[0] = id(2),
            17 => {
                structural_call_mut(&mut changed).crash_continuations[0].cause =
                    terminal_psi::CrashCause::Abort
            }
            18 => {
                let OwnershipEvent::ClaimTransfer(claims) =
                    &mut changed.scalar_functions[0].blocks[0].instructions[0].ownership[0]
                else {
                    panic!("transfer fixture");
                };
                claims.swap(0, 1);
            }
            19 => changed.scalar_functions[0].blocks[0].instructions.clear(),
            20 => scalar_return_mut(&mut changed.scalar_functions[0]).fuel[0].units += 1,
            21 => {
                scalar_return_mut(&mut changed.scalar_functions[0])
                    .effect
                    .input += 1
            }
            _ => {
                let OwnershipEvent::Cleanup(actions) =
                    &mut scalar_return_mut(&mut changed.scalar_functions[0]).ownership[0]
                else {
                    panic!("cleanup fixture");
                };
                actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(id(
                    1,
                )));
            }
        }
        assert_identity_drift(identity, &changed);
    }
}

#[test]
fn structural_signature_and_calls_cannot_alias_value_less_unit_graph() {
    let original = call_aware_plan();
    let mut erased = original.clone();
    let function = &mut erased.scalar_functions[0];
    function.structural = None;
    function.blocks[0].instructions.clear();
    function.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(erased.target),
        &CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .unwrap();
    assert_ne!(
        legalized_operation_plan_identity(&original),
        legalized_operation_plan_identity(&erased)
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
