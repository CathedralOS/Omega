//! Tests selection for sum rows the target plan already stores.
//!
//! Each scenario builds the target row directly and runs legalization and
//! selection. The row carries the tag, offset, layout, pointer chase, and
//! referent-region size that stage 02 records. The expected text is the
//! selection `origin/main` emitted for that same geometry.

use crate::BoundaryOriginFacts;
use abstract_operations::{
    AbstractBlock, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractResult, AbstractStructuralCaseSuccessor,
};
use calling_conventions::{CallSignature, CallingPolicy};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
    WorkScheduleIdentity,
};
use target::NativeTarget;
use target_operations::{
    TargetCasePointerSlot, TargetControlBlock, TargetControlCaseSuccessor, TargetControlGraph,
    TargetControlTerminator, TargetFunction, TargetOperationPlan, TargetStructuralCaseSource,
    TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TerminalPsiProvenance,
};
use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    TerminalIdentityTableBuilder, TerminalModule, TerminalPsiIdentity, VocabularyMarker,
};

const HOST: fn() -> NativeTarget = NativeTarget::linux_x64;

#[test]
fn a_literal_edge_can_retire_its_descriptor_home() {
    use crate::legalization_function_inputs::FunctionInputs;
    use crate::legalization_scalar_graph::control;
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::{StructuralPlaceDeclaration, TerminalAffineCleanupAction};

    let mut names = TerminalIdentityTableBuilder::default();
    let mut view = reference_type(1, names.intern("literal bytes"), ty(1));
    view.shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None,
        });
    let literal = StructuralPlaceDeclaration {
        id: place(1),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: view.id,
        },
    };
    let plan = plan(
        names,
        vec![view.clone()],
        function_with_body(
            Vec::new(),
            vec![
                entry(
                    1,
                    vec![
                        AbstractOperation::EstablishByteSequenceLiteral {
                            psi_operation: op(1),
                            place: literal,
                            structural_type: view,
                            bytes: b"dir/file".to_vec(),
                            qualifications: Vec::new(),
                        },
                        AbstractOperation::Jump {
                            psi_edge: edge(1),
                            target: block_id(2),
                            bindings: Vec::new(),
                            structural_bindings: Vec::new(),
                            cleanup_actions: vec![TerminalAffineCleanupAction::RetireHome(
                                literal.id,
                            )],
                        },
                    ],
                ),
                entry(
                    2,
                    vec![AbstractOperation::ReturnUnit {
                        psi_edge: edge(2),
                        cleanup_actions: Vec::new(),
                    }],
                ),
            ],
        ),
    );
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let function = &unit.functions[0];
    let inputs = FunctionInputs::new(function);
    let types = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let terminator = function.blocks[0].nodes.last().unwrap();
    control::validate(terminator, &[], &inputs, &types).unwrap();

    for wrong in [
        TerminalAffineCleanupAction::RetireHome(place(9)),
        TerminalAffineCleanupAction::DiscardRoot(literal.id),
    ] {
        // Make the operation and edge agree so the custody check itself must
        // refuse each mutation rather than just finding a mismatched edge.
        let mut forged = terminator.clone();
        let AbstractOperation::Jump {
            cleanup_actions, ..
        } = &mut forged.operation
        else {
            unreachable!()
        };
        cleanup_actions[0] = wrong.clone();
        forged.successors[0].cleanup_actions[0] = wrong.clone();
        assert!(
            control::validate(&forged, &[], &inputs, &types).is_err(),
            "{wrong:?}"
        );
    }
}

#[test]
fn byte_literal_lookup_refuses_every_producer_the_scan_refused() {
    use crate::legalization_function_inputs::FunctionInputs;
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::StructuralPlaceDeclaration;

    // One function establishes a single literal view. The indexed lookup must
    // find its producer, and refuse each mutation that leaves the place
    // without exactly one matching literal producer.
    let mut names = TerminalIdentityTableBuilder::default();
    let mut view = reference_type(1, names.intern("literal bytes"), ty(1));
    view.shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None,
        });
    let literal = StructuralPlaceDeclaration {
        id: place(1),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: view.id,
        },
    };
    let establish = AbstractOperation::EstablishByteSequenceLiteral {
        psi_operation: op(1),
        place: literal,
        structural_type: view.clone(),
        bytes: b"dir/file".to_vec(),
        qualifications: Vec::new(),
    };
    let plan = plan(
        names,
        vec![view.clone()],
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![
                    establish,
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let function = &unit.functions[0];
    assert_eq!(
        FunctionInputs::new(function).byte_literal(literal.id),
        Some((op(1), view.id))
    );
    assert_eq!(FunctionInputs::new(function).byte_literal(place(9)), None);

    let refused = |mutate: &dyn Fn(&mut optimization_unit::PsiOptimizationFunction)| {
        let mut changed = function.clone();
        mutate(&mut changed);
        FunctionInputs::new(&changed).byte_literal(literal.id)
    };
    // No producer remains for the declared literal.
    assert_eq!(
        refused(&|f| {
            f.blocks[0].nodes.remove(0);
        }),
        None
    );
    // Two producers establish the same place.
    assert_eq!(
        refused(&|f| {
            let duplicate = f.blocks[0].nodes[0].clone();
            f.blocks[0].nodes.insert(0, duplicate);
        }),
        None
    );
    // The place is declared twice.
    assert_eq!(refused(&|f| f.structural_places.push(literal)), None);
    // The declaration is not a literal home.
    assert_eq!(
        refused(&|f| {
            for declaration in &mut f.structural_places {
                if declaration.id == literal.id {
                    declaration.kind = StructuralPlaceKind::Parameter { position: 0 };
                }
            }
        }),
        None
    );
    // The producer names another type or declaration, or a type that is not
    // a borrowed byte view.
    let producer =
        |change: &dyn Fn(&mut StructuralPlaceDeclaration, &mut StructuralTypeDeclaration)| {
            refused(&|f| {
                if let AbstractOperation::EstablishByteSequenceLiteral {
                    place,
                    structural_type,
                    ..
                } = &mut f.blocks[0].nodes[0].operation
                {
                    change(place, structural_type);
                }
            })
        };
    assert_eq!(producer(&|_, declared| declared.id = ty(2)), None);
    assert_eq!(
        producer(&|place, _| {
            place.kind = StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 1,
                structural_type: view.id,
            }
        }),
        None
    );
    assert_eq!(
        producer(&|_, declared| {
            declared.shape =
                StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                    capacity: 8,
                })
        }),
        None
    );
}

#[test]
fn retained_instruction_metadata_projects_without_a_node_and_rejects_changed_custody() {
    let integer = u64_type();
    let plan = plan(
        TerminalIdentityTableBuilder::default(),
        Vec::new(),
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: op(1),
                        result: val(1),
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let target = target_plan(
        &plan,
        vec![block(
            1,
            vec![TargetUnitOperation::IntegerConstant {
                psi_operation: op(1),
                result: val(1),
                scalar_type: integer,
                value: IntegerValue::Unsigned(7),
            }],
            return_unit(1),
        )],
    );
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let node = &unit.functions[0].blocks[0].nodes[0];
    let instruction = &target.functions[0].graph.blocks[0].operations[0];
    let source = instruction.source.as_ref().unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let projected = crate::legalization_source::scalar_graph::instruction::project(
        unit.functions[0].machine,
        &unit.accepted_obligation_facts,
        fixture_types(),
        &target,
        &geometry,
        instruction,
    )
    .unwrap();
    assert_eq!(projected.operation, op(1));
    assert_eq!(projected.result.as_ref().unwrap().value, val(1));
    assert_eq!(
        projected.result.as_ref().unwrap().definition_site,
        node.definitions[0].site
    );
    assert_eq!(projected.work, node.work);
    assert_eq!(projected.effect, node.effect);
    assert_eq!(projected.ownership, node.ownership);
    // Exercise nonempty records independently of this constant's empty custody.
    // Projection transports annotations; it does not issue the claims they name.
    let mut carrying = source.clone();
    carrying
        .metadata
        .work
        .push(optimization_unit::WorkSettlement {
            site: op(1).into(),
            units: 9,
        });
    carrying.metadata.effect = optimization_unit::EffectLink {
        input: 3,
        output: 4,
    };
    carrying
        .metadata
        .ownership
        .push(optimization_unit::OwnershipEvent::ClaimTransfer(vec![
            semantic_vocabulary::ClaimId::new(7).unwrap(),
        ]));
    let carried = crate::legalization_source::scalar_graph::instruction::with_metadata(
        &carrying,
        projected.kind.clone(),
        fixture_types(),
    )
    .unwrap();
    assert_eq!(carried.work, carrying.metadata.work);
    assert_eq!(carried.effect, carrying.metadata.effect);
    assert_eq!(carried.ownership, carrying.metadata.ownership);
    let audit = |instruction: &target_operations::TargetInstruction| {
        crate::legalization_target::control_flow::validate_instruction_metadata(instruction, node)
    };
    audit(instruction).expect("exact instruction metadata");
    let mut missing = instruction.clone();
    missing.source = None;
    let failure = crate::legalization_source::scalar_graph::instruction::project(
        unit.functions[0].machine,
        &unit.accepted_obligation_facts,
        fixture_types(),
        &target,
        &geometry,
        &missing,
    )
    .expect_err("the old body cannot reconstruct missing target metadata");
    assert!(matches!(
        failure,
        crate::LegalizationError::InstructionMetadata {
            reason: "ordinary target instruction has no retained source metadata",
            ..
        }
    ));
    let mut annotated_cleanup = target.functions[0].graph.blocks[0].clone();
    annotated_cleanup
        .operations
        .push(target_operations::TargetInstruction {
            source: Some(source.clone()),
            kind: TargetUnitOperation::CleanupCall {
                owner: target_operations::CallSiteOwner::CleanupAction {
                    edge: edge(1),
                    action_ordinal: 0,
                },
                callee: machine(),
                call_plan: target.functions[0].graph.call_plan.clone(),
                arguments: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        });
    let failure = crate::legalization_control::project(
        machine(),
        &annotated_cleanup,
        &[],
        &[],
        fixture_types(),
    )
    .expect_err("cleanup keeps its cleanup owner, not an authored instruction identity");
    assert!(matches!(
        failure,
        crate::LegalizationError::InstructionMetadata {
            reason: "compiler cleanup call carries authored instruction metadata",
            ..
        }
    ));
    for mutation in 0..10 {
        let mut changed = instruction.clone();
        if mutation == 0 {
            changed.source = None;
        } else {
            let source = changed.source.as_mut().unwrap();
            match mutation {
                1 => source.operation = op(99),
                2 => source.result = None,
                3 => source.result.as_mut().unwrap().value = val(99),
                4 => source.result.as_mut().unwrap().scalar_type = ScalarType::Boolean,
                5 => {
                    source.result.as_mut().unwrap().site =
                        optimization_unit::ValueDefinitionSite::Node {
                            block: block_id(2),
                            node: 0,
                        }
                }
                6 => source
                    .metadata
                    .work
                    .push(optimization_unit::WorkSettlement {
                        site: op(99).into(),
                        units: 1,
                    }),
                7 => source.metadata.effect.input = source.metadata.effect.input.saturating_add(1),
                8 => {
                    source.metadata.effect.output = source.metadata.effect.output.saturating_add(1)
                }
                _ => source.metadata.ownership.push(
                    optimization_unit::OwnershipEvent::ClaimTransfer(vec![
                        semantic_vocabulary::ClaimId::new(99).unwrap(),
                    ]),
                ),
            }
        }
        let reason = match mutation {
            0 => "instruction lost its source annotations",
            1 => "instruction operation identity differs",
            2..=5 => "instruction scalar definition differs",
            6 => "instruction work charge differs",
            7..=8 => "instruction effect order differs",
            _ => "instruction custody events differ",
        };
        assert!(
            matches!(audit(&changed), Err(crate::LegalizationError::InstructionMetadata { reason: actual, .. }) if actual == reason),
            "forged instruction metadata {mutation}"
        );
    }
}

#[test]
fn retained_interrupt_geometry_does_not_authorize_an_unassociated_helper() {
    let integer = u64_type();
    let plan = plan(
        TerminalIdentityTableBuilder::default(),
        Vec::new(),
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: op(1),
                        result: val(1),
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(7),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let mut target = target_plan(
        &plan,
        vec![block(
            1,
            vec![TargetUnitOperation::IntegerConstant {
                psi_operation: op(1),
                result: val(1),
                scalar_type: integer,
                value: IntegerValue::Unsigned(7),
            }],
            return_unit(1),
        )],
    );
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let module = TerminalModule::for_entry(plan.entry);
    let interrupts = provider_admission::AdmittedInstalledInterruptCohort::empty(plan.psi);
    let evidence = crate::PreparationEvidence {
        optimization_unit: unit.identity,
        work_schedule: unit.work_schedule,
        accepted_obligation_facts: &[],
        ownership_frontier_facts: &[],
    };
    let origins = BoundaryOriginFacts {
        program_entry: None,
        assumed_bindings: &[],
        target_context: None,
        module: &module,
    };
    crate::legalization::prepare_execution_plan(
        &target,
        evidence,
        origins,
        &interrupts,
        fixture_types(),
    )
    .unwrap();
    // The selector-free helper has a representable operand. Only separately
    // admitted installation authority may authorize it, not that geometry.
    target.functions[0].graph.blocks[0].operations[0].kind = TargetUnitOperation::StaticInterrupt {
        psi_operation: op(1),
        boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
        operation: language_core::static_interrupts::StaticInterruptOperation::FinishCompletion,
        token: terminal_psi::StructuralArgument {
            place: place(1),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
        indices: Vec::new(),
        result_home: None,
    };
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    crate::legalization_source::scalar_graph::instruction::project(
        machine(),
        &[],
        fixture_types(),
        &target,
        &geometry,
        &target.functions[0].graph.blocks[0].operations[0],
    )
    .unwrap();
    let error = crate::legalization::prepare_execution_plan(
        &target,
        evidence,
        origins,
        &interrupts,
        fixture_types(),
    )
    .expect_err("operand geometry cannot invent an admitted helper association");
    assert!(
        matches!(error, crate::LegalizationError::InstalledInterruptAssociation {
        operation: Some(operation), reason: "interrupt helper has no admitted installation association",
    } if operation == op(1)),
        "{error:?}"
    );
}

#[test]
fn retained_terminator_projects_without_a_body_and_audit_refuses_changed_evidence() {
    let plan = plan(
        TerminalIdentityTableBuilder::default(),
        Vec::new(),
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                }],
            )],
        ),
    );
    let mut target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    // The shared geometry fixture omits the scalar ABI and provenance because
    // its original callers audit aggregates only. This control audits a whole
    // scalar function, so supply its real Unit ABI and exact return edge.
    target.functions[0].scalar_abi = Some(target_operations::ScalarFunctionAbi {
        call_plan: target.functions[0].graph.call_plan.clone(),
        parameters: Vec::new(),
        result: None,
    });
    target.functions[0].provenance.edges = vec![edge(1)];
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let projected = crate::legalization_control::project(
        machine(),
        &target.functions[0].graph.blocks[0],
        &[],
        &[],
        fixture_types(),
    )
    .unwrap();
    let legalized_operations::LegalizedScalarTerminator::Return(returned) = projected else {
        panic!("ordinary Unit return");
    };
    assert_eq!(returned.edge, edge(1));
    assert_eq!(returned.work, unit.functions[0].blocks[0].nodes[0].work);
    crate::legalization::audit_target_operation_plan(&target, &plan, &unit, fixture_types())
        .expect("retained exit matches source");
    for mutation in 0..4 {
        let mut changed = target.clone();
        let block = &mut changed.functions[0].graph.blocks[0];
        match mutation {
            0 => block.terminator_metadata.work.clear(),
            1 => block.terminator_metadata.effect.output += 1,
            2 => block.terminator_metadata.ownership.push(
                optimization_unit::OwnershipEvent::ClaimTransfer(vec![
                    semantic_vocabulary::ClaimId::new(9).unwrap(),
                ]),
            ),
            _ => block.terminator = return_unit(9),
        }
        assert!(
            crate::legalization::audit_target_operation_plan(
                &changed,
                &plan,
                &unit,
                fixture_types()
            )
            .is_err(),
            "forged exit {mutation}"
        );
    }
}

#[test]
fn function_storage_contract_projects_without_source_and_audits_every_field() {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("Stored"), &mut names, &[]);
    let plan = plan(
        names,
        vec![sum],
        function_with_body(
            vec![parameter(ty(1))],
            vec![entry(
                1,
                vec![AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                }],
            )],
        ),
    );
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let function = &target.functions[0];
    let projected = crate::legalization_scalar_graph::structural_contract(function)
        .unwrap()
        .unwrap();
    assert_eq!(
        projected.parameters[0].semantic,
        plan.functions[0].structural_parameters[0]
    );
    assert_eq!(projected.parameters[0].target, function.graph.parameters[0]);
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let source = crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let inputs = crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let audit = |function: &TargetFunction| {
        crate::legalization_scalar_graph::validate_structural_contract(
            function,
            &plan.functions[0],
            &inputs,
            &source,
        )
    };
    audit(function).expect("retained storage contract matches admitted input");
    for mutation in 0..8 {
        let mut changed = function.clone();
        if mutation == 0 {
            changed.structural_contract = None;
        } else {
            let contract = changed.structural_contract.as_mut().unwrap();
            match mutation {
                1 => contract.parameters.clear(),
                2 => contract.parameters[0].access = StructuralAccess::MutableBorrow,
                3 => contract.places.clear(),
                4 => contract
                    .published_service_ceiling
                    .push(semantic_vocabulary::ServiceId::new(9).unwrap()),
                5 => {
                    contract.result = Some(terminal_psi::StructuralResultDeclaration {
                        place: place(99),
                        structural_type: ty(1),
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        reference_sources: Vec::new(),
                    })
                }
                6 => contract.places[0].id = place(99),
                _ => contract.entry_claims.push(terminal_psi::EntryClaim {
                    claim: semantic_vocabulary::ClaimId::new(9).unwrap(),
                    input: place(1),
                    path: Vec::new(),
                }),
            }
        }
        assert!(
            audit(&changed).is_err(),
            "forged function storage contract {mutation}"
        );
    }
}

#[test]
fn selection_honors_the_stored_call_plan_without_recomputing_it() {
    use calling_conventions::{MachineRegister, ValueLocation};
    // The stored plan passes both u64 arguments in the opposite registers from
    // the plan `evaluate_call_plan` produces. Selection checks the arguments
    // against the stored plan and accepts it; it never recomputes the plan.
    // Comparing the stored plan with the callee's own plan is the independent
    // audit's job (`direct_calls` in legalization_target.rs).
    let policy = CallingPolicy::native_for_target(HOST());
    let shape = calling_conventions::ValueShape::for_integer(u64_type()).unwrap();
    let evaluated = calling_conventions::evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .unwrap();
    let register = |register| calling_conventions::ValuePlacement {
        shape,
        locations: vec![ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: shape.byte_size,
        }],
    };
    let mut stored = evaluated.clone();
    stored.parameters = vec![
        register(MachineRegister::X86Rsi),
        register(MachineRegister::X86Rdi),
    ];
    assert_ne!(
        stored, evaluated,
        "the stored plan is not the recomputed one"
    );
    let argument = |position: u32| target_operations::TargetUnitScalarCallArgument {
        parameter_index: position,
        source: TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation: op(1),
            source_value: val(u64::from(position) + 1),
            scalar_type: u64_type(),
            value: IntegerValue::Unsigned(u128::from(position)),
        },
        placement: stored.parameters[position as usize].clone(),
    };
    let row = TargetUnitOperation::Call {
        psi_operation: op(2),
        origin: target_operations::NativeCallOrigin::Authored,
        target: target_operations::NativeCallTarget::Machine(MachineId::new(2).unwrap()),
        call_plan: stored.clone(),
        result: target_operations::TargetCallResult::Unit,
        scalar_arguments: vec![argument(0), argument(1)],
        arguments: Vec::new(),
        access_bindings: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let legalized_operations::LegalizedScalarInstructionKind::Call(call) =
        crate::legalization_source::scalar_graph::instruction::call_instructions::project_call(
            machine(),
            &[],
            &row,
        )
        .unwrap()
    else {
        panic!("call");
    };
    assert_eq!(call.call_plan, stored);
    assert!(call.stored_plan_placements(policy).is_some());
    // An argument placed somewhere the stored plan does not put it is refused,
    // even where that placement is what a recomputed plan would choose.
    let mut moved = call.clone();
    let legalized_operations::LegalizedScalarArgument::Scalar { placement, .. } =
        &mut moved.arguments[0]
    else {
        panic!("scalar argument");
    };
    *placement = evaluated.parameters[0].clone();
    assert!(moved.stored_plan_placements(policy).is_none());
    let mut missing = call.clone();
    missing.arguments.pop();
    assert!(missing.stored_plan_placements(policy).is_none());
    let foreign = CallingPolicy::native_for_target(NativeTarget::linux_arm64());
    assert!(call.stored_plan_placements(foreign).is_none());
}

#[test]
fn scalar_parameter_projection_uses_retained_abi_without_a_source_function() {
    let plan = plan(
        TerminalIdentityTableBuilder::default(),
        Vec::new(),
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                }],
            )],
        ),
    );
    let mut target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let scalar_type = ScalarType::Integer(u64_type());
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: vec![calling_conventions::ValueShape::for_integer(u64_type()).unwrap()],
            result: None,
        },
    )
    .unwrap();
    target.functions[0].graph.scalar_parameters = vec![target_operations::ScalarAbiValue {
        value: val(7),
        scalar_type,
        placement: call_plan.parameters[0].clone(),
    }];
    let parameters =
        crate::legalization_source::scalar_graph::parameters(&target.functions[0], fixture_types())
            .unwrap();
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].value, val(7));
    assert_eq!(parameters[0].scalar_type.as_checking(), scalar_type);
    assert_eq!(parameters[0].placement, call_plan.parameters[0]);
    assert_eq!(
        parameters[0].definition_site,
        optimization_unit::ValueDefinitionSite::FunctionParameter(0)
    );
}

// Exercise the origin handoff itself; full call ABI construction is covered by
// the kept returned-window program, not this deliberately minimal target row.
#[test]
fn retained_call_descriptor_origins_do_not_need_old_bodies_but_audit_does() {
    use target_operations::{
        TargetCallResult, TargetDescriptorOrigin, TargetStructuralResultContract,
    };
    let mut names = TerminalIdentityTableBuilder::default();
    let mut view = reference_type(1, names.intern("bytes"), ty(1));
    view.shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        });
    let declaration = terminal_psi::StructuralResultDeclaration {
        place: place(3),
        structural_type: ty(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    };
    let result = structural_result(place(2), ty(1));
    let callee = MachineId::new(2).unwrap();
    let caller = function_with_body(
        vec![parameter(ty(1))],
        vec![entry(
            1,
            vec![
                AbstractOperation::CallStructural {
                    psi_operation: op(1),
                    callee: callee.into(),
                    arguments: Vec::new(),
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: place(1),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    result: abstract_operations::AbstractStructuralCallResult::Structural {
                        result: result.clone(),
                        selected_evidence: Vec::new(),
                    },
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let mut plan = plan(names, vec![view], caller);
    let mut forwarded = function_with_body(
        vec![parameter(ty(1))],
        vec![entry(
            1,
            vec![AbstractOperation::ReturnStructural {
                psi_edge: edge(2),
                source: place(1),
                returned_claims: Vec::new(),
                trivial_affine_locals: Vec::new(),
                cleanup_actions: Vec::new(),
            }],
        )],
    );
    forwarded.machine = callee;
    forwarded.result = AbstractFunctionResult::Structural(declaration.clone());
    plan.functions.push(forwarded);
    let expected = vec![TargetDescriptorOrigin::Argument {
        source: terminal_psi::StructuralArgument {
            place: place(1),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        },
        exact: true,
        indirect: false,
    }];
    let row = TargetUnitOperation::Call {
        psi_operation: op(1),
        origin: target_operations::NativeCallOrigin::Authored,
        target: target_operations::NativeCallTarget::Machine(callee),
        call_plan: calling_conventions::evaluate_call_plan(
            CallingPolicy::native_for_target(HOST()),
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap(),
        result: TargetCallResult::Structural {
            result,
            callee_result: TargetStructuralResultContract::Machine(declaration),
            result_home: None,
            reference_results: Vec::new(),
            descriptor_origins: expected.clone(),
        },
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        access_bindings: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let target = target_plan(&plan, vec![block(1, vec![row.clone()], return_unit(1))]);
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let graph = optimization_unit_semantics::descriptor_results::optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        &unit.structural_types,
        &unit.dynamic_dispatch,
        |_, node| Some(std::borrow::Cow::Borrowed(&node.operation)),
    )
    .expect("the metadata fixture has complete source descriptor structure");
    let origins = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
        &unit.structural_types,
        &graph,
    )
    .try_call_origins(machine(), place(2))
    .expect("the source call has borrowed backing");
    assert!(
        matches!(origins.as_slice(), [terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, .. }] if *source == place(1)),
        "source backing: {origins:?}"
    );
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let source = crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let native_unit =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    crate::legalization::validate_call_descriptor_origins(&target, &source, &native_unit)
        .expect("exact source backing");
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetUnitOperation::Call {
            result:
                TargetCallResult::Structural {
                    descriptor_origins, ..
                },
            ..
        } = &mut changed.functions[0].graph.blocks[0].operations[0].kind
        else {
            panic!("call result")
        };
        match mutation {
            0 => descriptor_origins.clear(),
            1 => *descriptor_origins = vec![TargetDescriptorOrigin::Empty],
            2 => *descriptor_origins = vec![TargetDescriptorOrigin::Opaque],
            3 => {
                let TargetDescriptorOrigin::Argument { source, .. } = &mut descriptor_origins[0]
                else {
                    panic!("argument")
                };
                source.place = place(99);
            }
            _ => {
                let TargetDescriptorOrigin::Argument { exact, .. } = &mut descriptor_origins[0]
                else {
                    panic!("argument")
                };
                *exact = false;
            }
        }
        assert!(
            crate::legalization::validate_call_descriptor_origins(&changed, &source, &native_unit)
                .is_err(),
            "tampered backing {mutation}"
        );
    }
    let caller = unit.functions[0].clone();
    let mut absent = unit.clone();
    absent.functions.clear();
    let absent =
        crate::native_geometry::NativeInput::with_types(&absent, &geometry, fixture_types());
    let lowered =
        crate::legalization_source::scalar_graph::instruction::call_instructions::project_call(
            caller.machine,
            &absent.accepted_obligation_facts,
            &row,
        )
        .expect("construction consumes target origins without source history");
    let legalized_operations::LegalizedScalarInstructionKind::Call(lowered) = lowered else {
        panic!("legalized call")
    };
    assert_eq!(lowered.descriptor_origins, expected);
    assert!(
        crate::legalization::validate_call_descriptor_origins(&target, &source, &absent).is_err(),
        "independent audit still requires source correspondence"
    );
}

#[test]
fn stored_sum_rows_select_the_same_instructions_as_main() {
    let dump = [
        tag_only(),
        payload_behind_pointer(),
        tag_only_behind_pointer(),
        sum_inside_record(),
        membership_behind_pointer(),
        establish_case(),
        activation_claim(),
    ]
    .join("");
    let expected = include_str!("sum_layout_selection.expected.txt");
    assert_eq!(dump, expected, "selected opcodes, offsets, tags, and sizes");
}

#[test]
fn retained_scalar_store_geometry_is_checked_against_its_source() {
    let mut names = TerminalIdentityTableBuilder::default();
    let field = StructuralFieldId::new(1).unwrap();
    let field_name = names.intern("value");
    let mut cell = reference_type(1, names.intern("Cell"), ty(1));
    cell.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![scalar_field(1, field_name, u64_type())],
    };
    let mut parameter = parameter(ty(1));
    parameter.access = StructuralAccess::MutableBorrow;
    let source = TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(1),
        source_value: ValueId::new(1).unwrap(),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(43),
    };
    let operation = AbstractOperation::StructuralScalarFieldStore {
        psi_operation: op(2),
        destination: abstract_operations::StructuralScalarStoreDestination::Parameter(
            parameter.clone(),
        ),
        path: Vec::new(),
        field,
        value: AbstractResult {
            value: source.source_value(),
            scalar_type: source.scalar_type(),
        },
        range_obligation: None,
    };
    let function = function_with_body(
        vec![parameter.clone()],
        vec![entry(
            1,
            vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: op(1),
                    result: source.source_value(),
                    scalar_type: ScalarType::Integer(u64_type()),
                    value: IntegerValue::Unsigned(43),
                },
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![cell], function);
    let mut target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let row = TargetUnitOperation::StructuralScalarFieldStore {
        psi_operation: op(2),
        destination: abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
        path: Vec::new(),
        field,
        destination_storage: target_operations::TargetStructuralArgumentSource::Placement(
            target.functions[0].graph.parameters[0].placement.clone(),
        ),
        field_byte_offset: 0,
        field_byte_size: 8,
        source,
        indices: Vec::new(),
    };
    target.functions[0].graph.blocks[0]
        .operations
        .push(target_operations::TargetInstruction {
            source: None,
            kind: row.clone(),
        });
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let function_inputs =
        crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = source_inputs.available(block_id(1));
        available.push((source.source_value(), source));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &function_inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("retained scalar store matches its source");
    for mutation in 0..4 {
        let mut forged = row.clone();
        let TargetUnitOperation::StructuralScalarFieldStore {
            field_byte_offset,
            field_byte_size,
            source: value,
            path,
            ..
        } = &mut forged
        else {
            panic!("store");
        };
        match mutation {
            0 => *field_byte_offset = 1,
            1 => *field_byte_size = 4,
            2 => {
                let TargetUnitScalarArgumentSource::IntegerImmediate { value, .. } = value else {
                    panic!("integer source");
                };
                *value = IntegerValue::Unsigned(47);
            }
            _ => path.push(StructuralPathSegment::FixedIndex(0)),
        }
        assert!(audit(&forged).is_err(), "tampered scalar store {mutation}");
    }
}

#[test]
fn retained_field_store_value_type_is_checked_against_its_source() {
    // Selection derives a structural field store's shape from the retained
    // value type alone, so the independent audit must refuse a row whose
    // value type, offset or field differs from the source operation.
    let mut names = TerminalIdentityTableBuilder::default();
    let inner_field = names.intern("x");
    let outer_field = names.intern("inner");
    let mut inner = reference_type(2, names.intern("Inner"), ty(2));
    inner.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![scalar_field(1, inner_field, u64_type())],
    };
    let mut outer = reference_type(1, names.intern("Outer"), ty(1));
    let mut nested = scalar_field(2, outer_field, u64_type());
    nested.field_type = StructuralFieldType::Structural(ty(2));
    outer.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![nested],
    };
    let mut parameter = parameter(ty(1));
    parameter.access = StructuralAccess::MutableBorrow;
    let field = StructuralFieldId::new(2).unwrap();
    let value = structural_result(place(3), ty(2));
    let operation = AbstractOperation::StoreStructuralField {
        disposition: terminal_psi::StructuralStoreDisposition::Vacant,
        psi_operation: op(2),
        destination: abstract_operations::StructuralWindowRoot::Parameter(parameter.clone()),
        path: Vec::new(),
        field: Some(field),
        value: terminal_psi::StructuralArgument {
            place: value.place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    };
    let function = function_with_body(
        vec![parameter.clone()],
        vec![entry(
            1,
            vec![
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: op(1),
                    result: value.clone(),
                },
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![outer, inner], function);
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let row = TargetUnitOperation::StoreStructuralField {
        psi_operation: op(2),
        destination: abstract_operations::StructuralWindowRoot::Parameter(parameter),
        path: Vec::new(),
        field: Some(field),
        value: value.place,
        value_type: ty(2),
        byte_offset: 0,
        indices: Vec::new(),
    };
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let function_inputs =
        crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = source_inputs.available(block_id(1));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &function_inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("retained field store matches its source");
    for mutation in 0..3 {
        let mut forged = row.clone();
        let TargetUnitOperation::StoreStructuralField {
            value_type,
            byte_offset,
            field,
            ..
        } = &mut forged
        else {
            panic!("store");
        };
        match mutation {
            0 => *value_type = ty(1),
            1 => *byte_offset = 4,
            _ => *field = Some(StructuralFieldId::new(1).unwrap()),
        }
        assert!(audit(&forged).is_err(), "tampered field store {mutation}");
    }
}

#[test]
fn retained_leaf_copy_selectors_are_checked_against_their_source() {
    // Selection takes a leaf copy's result and selector extent and proof key
    // from the target row, so the audit must refuse each one changed.
    let mut names = TerminalIdentityTableBuilder::default();
    let element_field = names.intern("x");
    let mut element = reference_type(2, names.intern("Element"), ty(2));
    element.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![scalar_field(1, element_field, u64_type())],
    };
    let mut array = reference_type(1, names.intern("Elements"), ty(1));
    array.shape = StructuralTypeShape::FixedArray {
        element: ty(2),
        length: terminal_psi::ArrayExtent::Literal(4),
    };
    let obligation = semantic_vocabulary::ObligationId::new(3).unwrap();
    let path = vec![StructuralPathSegment::RuntimeIndex {
        index: val(4),
        obligation,
    }];
    let result = structural_result(place(3), ty(2));
    let operation = AbstractOperation::StructuralLeafCopy {
        psi_operation: op(2),
        result: result.clone(),
        source: place(1),
        path: abstract_operations::StructuralCopyPath::Ordinary(path.clone()),
    };
    let function = function_with_body(
        vec![parameter(ty(1))],
        vec![entry(
            1,
            vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: op(1),
                    result: val(4),
                    scalar_type: ScalarType::Integer(u64_type()),
                    value: IntegerValue::Unsigned(1),
                },
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![array, element], function);
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let index_source = TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(1),
        source_value: val(4),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(1),
    };
    let layout =
        crate::legalization_scalar_graph::aggregate_results::type_home_layout(ty(2), &plan_view)
            .unwrap();
    let row = TargetUnitOperation::StructuralLeafCopy {
        psi_operation: op(2),
        result_home: home(op(2), &result, layout.clone()),
        source: place(1),
        path: abstract_operations::StructuralCopyPath::Ordinary(path),
        byte_offset: 0,
        indices: vec![target_operations::TargetStructuralRuntimeIndex {
            operand: index_source,
            stride: 8,
            extent: Some(4),
            obligation,
        }],
    };
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let function_inputs =
        crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = source_inputs.available(block_id(1));
        available.push((val(4), index_source));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &function_inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("retained leaf copy matches its source");
    for mutation in 0..4 {
        let mut forged = row.clone();
        let TargetUnitOperation::StructuralLeafCopy {
            result_home,
            indices,
            ..
        } = &mut forged
        else {
            panic!("leaf copy");
        };
        match mutation {
            0 => indices[0].extent = Some(5),
            1 => indices[0].obligation = semantic_vocabulary::ObligationId::new(9).unwrap(),
            2 => indices[0].stride = 16,
            _ => *result_home = home(op(9), &result, layout.clone()),
        }
        assert!(audit(&forged).is_err(), "tampered leaf copy {mutation}");
    }
}

#[test]
fn retained_byte_view_store_projects_and_audits_exact_operands() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::project_byte_view_store;
    use legalized_operations::LegalizedScalarInstructionKind;
    use terminal_psi::ByteSequenceCarrier;

    let carrier = ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    };
    let mut names = TerminalIdentityTableBuilder::default();
    let mut field = scalar_field(1, names.intern("bytes"), u64_type());
    field.field_type = StructuralFieldType::ByteSequence(carrier);
    let selected_field = field.id;
    let mut holder = reference_type(2, names.intern("Holder"), ty(2));
    holder.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![field],
    };
    let mut array = reference_type(1, names.intern("Holders"), ty(1));
    array.shape = StructuralTypeShape::FixedArray {
        element: ty(2),
        length: terminal_psi::ArrayExtent::Literal(4),
    };
    let mut bytes = reference_type(3, names.intern("Bytes"), ty(3));
    bytes.shape = StructuralTypeShape::ByteSequence(carrier);

    let mut stored = parameter(ty(1));
    stored.access = StructuralAccess::MutableBorrow;
    let mut replacement = parameter_at(2, ty(3));
    replacement.position = 1;
    let obligation = semantic_vocabulary::ObligationId::new(3).unwrap();
    let destination = terminal_psi::StructuralArgument {
        place: stored.place,
        access: stored.access,
        path: vec![StructuralPathSegment::RuntimeIndex {
            index: val(4),
            obligation,
        }],
    };
    let operation = AbstractOperation::StructuralByteViewFieldStore {
        psi_operation: op(2),
        destination: stored.place,
        path: destination.path.clone(),
        field: Some(selected_field),
        source: replacement.place,
        predicate_obligation: None,
    };
    let function = function_with_body(
        vec![stored, replacement],
        vec![entry(
            1,
            vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: op(1),
                    result: val(4),
                    scalar_type: ScalarType::Integer(u64_type()),
                    value: IntegerValue::Unsigned(1),
                },
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![array, holder, bytes], function);
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let index_source = TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(1),
        source_value: val(4),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(1),
    };
    let row = TargetUnitOperation::StructuralByteViewFieldStore {
        psi_operation: op(2),
        destination: destination.clone(),
        field: Some(selected_field),
        source: place(2),
        indices: vec![target_operations::TargetStructuralRuntimeIndex {
            operand: index_source,
            stride: 16,
            extent: Some(4),
            obligation,
        }],
    };
    let fact = optimization_unit::AcceptedObligationFact::new(
        psi(),
        [7; 32],
        machine(),
        optimization_unit::PsiProvenance::Operation(op(2)),
        obligation,
        vec![2],
    );
    let facts = std::slice::from_ref(&fact);
    let LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
        destination: projected,
        field,
        source,
        indices,
    } = project_byte_view_store(machine(), facts, &row).unwrap()
    else {
        panic!("byte descriptor store");
    };
    assert_eq!(projected, destination);
    assert_eq!((source, field), (place(2), Some(selected_field)));
    assert_eq!(indices.len(), 1);
    assert_eq!(
        (
            indices[0].stride,
            indices[0].extent,
            indices[0].accepted_fact
        ),
        (16, Some(4), fact.identity)
    );
    assert!(project_byte_view_store(machine(), &[], &row).is_err());

    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let function_inputs =
        crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = source_inputs.available(block_id(1));
        available.push((val(4), index_source));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &function_inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("retained descriptor store agrees with its source");
    for mutation in 0..4 {
        let mut changed = row.clone();
        let TargetUnitOperation::StructuralByteViewFieldStore {
            destination,
            source,
            indices,
            ..
        } = &mut changed
        else {
            panic!("byte descriptor store");
        };
        match mutation {
            0 => indices[0].extent = Some(5),
            1 => indices[0].obligation = semantic_vocabulary::ObligationId::new(9).unwrap(),
            2 => destination.access = StructuralAccess::SharedBorrow,
            _ => *source = place(1),
        }
        assert!(
            audit(&changed).is_err(),
            "corrupted descriptor store {mutation}"
        );
    }
}

#[test]
fn retained_storage_copies_stores_and_tag_tests_project_without_bodies() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::{
        project_structural_case_membership, project_structural_field_store,
        project_structural_leaf_copy,
    };
    use legalized_operations::LegalizedScalarInstructionKind as Kind;
    use target_operations::TargetStructuralRuntimeIndex;
    // Selection must build these three rows from the target row, the shared
    // type catalog and the admitted fact list alone; no earlier body is passed.
    let mut names = TerminalIdentityTableBuilder::default();
    let field_name = names.intern("value");
    let mut cell = reference_type(1, names.intern("Cell"), ty(1));
    cell.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![scalar_field(1, field_name, u64_type())],
    };
    let types = vec![cell];
    let geometry = crate::native_geometry::NativeGeometry::new(&types).unwrap();
    let obligation = semantic_vocabulary::ObligationId::new(3).unwrap();
    let index = TargetStructuralRuntimeIndex {
        operand: TargetUnitScalarArgumentSource::Parameter {
            parameter_index: 0,
            source_value: val(4),
            scalar_type: target_operations::selector_carrier_type(),
        },
        stride: 8,
        extent: Some(5),
        obligation,
    };
    let fact = optimization_unit::AcceptedObligationFact::new(
        psi(),
        [6; 32],
        machine(),
        optimization_unit::PsiProvenance::Operation(op(2)),
        obligation,
        vec![7],
    );
    let facts = std::slice::from_ref(&fact);
    let path = vec![StructuralPathSegment::RuntimeIndex {
        index: val(4),
        obligation,
    }];

    let membership = TargetUnitOperation::StructuralCaseMembership {
        psi_operation: op(2),
        result: boolean_result(),
        source: place(1),
        path: path.clone(),
        tag_byte_offset: 8,
        case: case_id(1),
        case_tag: 0,
        indices: vec![index.clone()],
        pointer_slots: vec![TargetCasePointerSlot {
            byte_offset: 0,
            index_start: 0,
            index_end: 1,
            scalar_descriptor: false,
        }],
    };
    let Kind::StructuralCaseMembership {
        indices,
        tag_byte_offset,
        ..
    } = project_structural_case_membership(machine(), facts, &membership).unwrap()
    else {
        panic!("membership");
    };
    assert_eq!(tag_byte_offset, 8);
    assert_eq!(
        (indices[0].extent, indices[0].accepted_fact),
        (Some(5), fact.identity)
    );
    assert!(project_structural_case_membership(machine(), &[], &membership).is_err());
    let mut overlong = membership.clone();
    let TargetUnitOperation::StructuralCaseMembership { pointer_slots, .. } = &mut overlong else {
        panic!("membership");
    };
    pointer_slots[0].index_end = 2;
    assert!(project_structural_case_membership(machine(), facts, &overlong).is_err());

    let result = structural_result(place(2), ty(1));
    let layout = TargetStructuralHomeLayout::Aggregate(geometry.shape(ty(1)).unwrap());
    let copy = TargetUnitOperation::StructuralLeafCopy {
        psi_operation: op(2),
        result_home: home(op(2), &result, layout.clone()),
        source: place(1),
        path: abstract_operations::StructuralCopyPath::Ordinary(path.clone()),
        byte_offset: 0,
        indices: vec![index.clone()],
    };
    let Kind::StructuralLeafCopy {
        result: copied,
        shape,
        indices,
        ..
    } = project_structural_leaf_copy(machine(), facts, &copy).unwrap()
    else {
        panic!("leaf copy");
    };
    assert_eq!(copied, result);
    assert_eq!(shape, layout.shape());
    assert_eq!(indices[0].accepted_fact, fact.identity);
    assert!(project_structural_leaf_copy(machine(), &[], &copy).is_err());
    let mut foreign = copy.clone();
    let TargetUnitOperation::StructuralLeafCopy { result_home, .. } = &mut foreign else {
        panic!("leaf copy");
    };
    *result_home = home(op(9), &result, layout);
    assert!(project_structural_leaf_copy(machine(), facts, &foreign).is_err());

    let store = TargetUnitOperation::StoreStructuralField {
        psi_operation: op(2),
        destination: abstract_operations::StructuralWindowRoot::Parameter(parameter(ty(1))),
        path,
        field: None,
        value: place(3),
        value_type: ty(1),
        byte_offset: 0,
        indices: vec![index],
    };
    let Kind::StoreStructuralField { shape, indices, .. } =
        project_structural_field_store(machine(), facts, &geometry, &store).unwrap()
    else {
        panic!("store");
    };
    assert_eq!(shape, geometry.shape(ty(1)).unwrap());
    assert_eq!(indices[0].extent, Some(5));
    assert!(project_structural_field_store(machine(), &[], &geometry, &store).is_err());
    let mut unknown = store;
    let TargetUnitOperation::StoreStructuralField { value_type, .. } = &mut unknown else {
        panic!("store");
    };
    *value_type = ty(9);
    assert!(project_structural_field_store(machine(), facts, &geometry, &unknown).is_err());
}

#[test]
fn retained_sequence_lengths_project_with_only_their_rows_and_facts() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::project_sequence_length;
    use legalized_operations::LegalizedScalarInstructionKind as Kind;
    use target_operations::{TargetIntegerExpression as Expression, TargetStructuralRuntimeIndex};

    let operation = op(3);
    let obligation = semantic_vocabulary::ObligationId::new(2).unwrap();
    let source = terminal_psi::StructuralArgument {
        place: place(1),
        path: vec![StructuralPathSegment::RuntimeIndex {
            index: val(1),
            obligation,
        }],
        access: StructuralAccess::SharedBorrow,
    };
    let index = TargetStructuralRuntimeIndex {
        operand: TargetUnitScalarArgumentSource::Parameter {
            parameter_index: 0,
            source_value: val(1),
            scalar_type: target_operations::selector_carrier_type(),
        },
        stride: 16,
        extent: Some(6),
        obligation,
    };
    let fact = optimization_unit::AcceptedObligationFact::new(
        psi(),
        [4; 32],
        machine(),
        optimization_unit::PsiProvenance::Operation(operation),
        obligation,
        vec![2],
    );
    let facts = std::slice::from_ref(&fact);
    let result = AbstractResult {
        value: val(2),
        scalar_type: ScalarType::Integer(u64_type()),
    };
    let definition = |expression| TargetUnitOperation::ScalarDefinition {
        result_home: target_operations::TargetUnitScalarHomeRequirement {
            defining_operation: operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
            shape: crate::legalization::scalar_graph_input::scalar_shape(result.scalar_type)
                .unwrap(),
        },
        expression: target_operations::TargetScalarExpression::Integer {
            scalar_type: u64_type(),
            expression,
        },
    };
    let byte = definition(Expression::ByteSequenceLength {
        psi_operation: operation,
        source_value: result.value,
        source: source.clone(),
        indices: vec![index.clone()],
        length_byte_offset: 0,
    });
    let element = definition(Expression::ElementViewLength {
        psi_operation: operation,
        source_value: result.value,
        source: source.clone(),
        indices: vec![index.clone()],
        length_byte_offset: 8,
    });
    let field = TargetUnitOperation::StructuralByteSequenceFieldLength {
        psi_operation: operation,
        result,
        source: source.clone(),
        field: StructuralFieldId::new(1).unwrap(),
        indices: vec![index],
    };
    for (row, expected_offset) in [(&byte, Some(0)), (&element, Some(8)), (&field, None)] {
        let (projected_source, indices, observed_offset) =
            match project_sequence_length(machine(), facts, operation, row).unwrap() {
                Kind::ByteSequenceLength {
                    source,
                    indices,
                    length_byte_offset,
                } => (source, indices, Some(length_byte_offset)),
                Kind::ElementViewLength {
                    source,
                    indices,
                    length_byte_offset,
                } => (source, indices, Some(length_byte_offset)),
                Kind::StructuralByteSequenceFieldLength {
                    source,
                    indices,
                    field,
                } => {
                    assert_eq!(field, StructuralFieldId::new(1).unwrap());
                    (source, indices, None)
                }
                actual => panic!("expected length, got {actual:?}"),
            };
        assert_eq!(projected_source, source);
        assert_eq!(observed_offset, expected_offset);
        assert_eq!(indices.len(), 1);
        assert_eq!(
            (
                indices[0].stride,
                indices[0].extent,
                indices[0].accepted_fact
            ),
            (16, Some(6), fact.identity)
        );
        assert!(project_sequence_length(machine(), &[], operation, row).is_err());
        assert!(project_sequence_length(machine(), facts, op(4), row).is_err());
    }
    let folded = definition(Expression::Immediate {
        source_value: result.value,
        value: IntegerValue::Unsigned(33),
    });
    assert!(matches!(
        project_sequence_length(machine(), &[], operation, &folded),
        Ok(Kind::Constant(IntegerValue::Unsigned(33)))
    ));
}

#[test]
fn retained_element_view_reads_use_the_exact_descriptor_index_and_leaf() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::project_element_view_read;
    use legalized_operations::LegalizedScalarInstructionKind;
    use semantic_vocabulary::{IeeeFloatFormat, ObligationId};
    use target_operations::{
        TargetBooleanExpression, TargetIntegerExpression, TargetScalarExpression,
        TargetUnitScalarHomeRequirement,
    };

    use terminal_psi::StructuralArgument;

    for scalar in [
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        ScalarType::Boolean,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
    ] {
        let mut names = TerminalIdentityTableBuilder::default();
        let sibling = scalar_field(1, names.intern("sibling"), u64_type());
        let mut field = scalar_field(2, names.intern("selected"), u64_type());
        field.field_type = StructuralFieldType::Scalar(scalar);
        let path = vec![StructuralPathSegment::Field(field.identity)];
        let mut element = reference_type(1, names.intern("Element"), ty(1));
        element.copyable = true;
        element.shape = StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![sibling, field],
        };
        let mut view_type = reference_type(2, names.intern("View"), ty(2));
        view_type.shape = StructuralTypeShape::ElementView {
            element: ty(1),
            access: Some(StructuralAccess::SharedBorrow),
        };
        let source = parameter(ty(2));
        let mut peer = parameter_at(2, ty(2));
        peer.position = 1;
        let bound = ObligationId::new(1).unwrap();
        let result = AbstractResult {
            value: val(3),
            scalar_type: scalar,
        };
        let read = AbstractOperation::ElementViewRead {
            psi_operation: op(3),
            result,
            source: StructuralArgument::whole(source.place, source.access),
            index: val(1),
            length: val(2),
            obligation: bound,
            path: path.clone(),
        };
        let function = function_with_body(
            vec![source.clone(), peer],
            vec![entry(
                1,
                vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: op(1),
                        result: val(1),
                        scalar_type: ScalarType::Integer(u64_type()),
                        value: IntegerValue::Unsigned(1),
                    },
                    AbstractOperation::ElementViewLength {
                        psi_operation: op(2),
                        result: AbstractResult {
                            value: val(2),
                            scalar_type: ScalarType::Integer(u64_type()),
                        },
                        source: terminal_psi::StructuralArgument::whole(
                            source.place,
                            source.access,
                        ),
                    },
                    read.clone(),
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        );
        let plan = plan(names, vec![element, view_type], function);
        let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
        let row =
            |position: usize, literal, element_byte_offset, path: Vec<StructuralPathSegment>| {
                let parameter = &target.functions[0].graph.parameters[position];
                let source = StructuralArgument::whole(parameter.place, source.access);
                let indices = Vec::new();
                let index = Box::new(TargetIntegerExpression::Immediate {
                    source_value: val(1),
                    value: IntegerValue::Unsigned(literal),
                });
                let source_value = result.value;
                let psi_operation = op(3);
                let length = val(2);
                let obligation = bound;
                macro_rules! read {
                ($module:ident :: $variant:ident $(, $format:ident)?) => {
                    $module::$variant {
                        $($format,)?
                        psi_operation, source_value, source, indices, index, length, obligation,
                        path, element_byte_offset,
                    }
                };
            }
                let expression = match scalar {
                    ScalarType::Integer(scalar_type) => TargetScalarExpression::Integer {
                        scalar_type,
                        expression: read!(TargetIntegerExpression::ElementViewRead),
                    },
                    ScalarType::Boolean => TargetScalarExpression::Boolean(read!(
                        TargetBooleanExpression::ElementViewRead
                    )),
                    ScalarType::IeeeFloat(format) => {
                        read!(TargetScalarExpression::IeeeFloatElementViewRead, format)
                    }
                };
                TargetUnitOperation::ScalarDefinition {
                    result_home: TargetUnitScalarHomeRequirement {
                        defining_operation: op(3),
                        source_value: result.value,
                        scalar_type: scalar,
                        shape: crate::legalization::scalar_graph_input::scalar_shape(scalar)
                            .unwrap(),
                    },
                    expression,
                }
            };
        let retained = row(0, 1, 8, path.clone());
        let fact = optimization_unit::AcceptedObligationFact::new(
            plan.psi,
            [7; 32],
            machine(),
            optimization_unit::PsiProvenance::Operation(op(3)),
            bound,
            vec![2],
        );
        let facts = std::slice::from_ref(&fact);
        assert_eq!(
            project_element_view_read(machine(), facts, op(3), &retained).unwrap(),
            Some(LegalizedScalarInstructionKind::ElementViewRead {
                source: StructuralArgument::whole(source.place, source.access),
                index: val(1),
                length: val(2),
                obligation: bound,
                accepted_fact: fact.identity,
                path: path.clone(),
                indices: Vec::new(),
            }),
        );
        assert!(project_element_view_read(machine(), &[], op(3), &retained).is_err());
        assert!(project_element_view_read(machine(), facts, op(4), &retained).is_err());
        let dynamic_path = vec![StructuralPathSegment::RuntimeIndex {
            index: val(1),
            obligation: bound,
        }];
        assert!(
            project_element_view_read(machine(), facts, op(3), &row(0, 1, 8, dynamic_path))
                .is_err()
        );

        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
                .unwrap();
        let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
        let plan_view =
            crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
        let unit_view =
            crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
        let inputs = crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
        let sources = crate::legalization_target::control_flow::sources::SourceInputs::new(
            &target.functions[0].graph,
            &unit.functions[0],
        );
        let audit = |row: &TargetUnitOperation| {
            crate::legalization_target::unit::validate_operation(
                &target.functions[0],
                row,
                &read,
                &[],
                &target.functions[0].graph.parameters,
                &mut sources.available(block_id(1)),
                &Default::default(),
                &inputs,
                &target,
                &plan_view,
                &unit_view,
            )
        };
        audit(&retained).expect("retained descriptor, index, and field agree with the source");
        assert!(
            audit(&row(1, 1, 8, path.clone())).is_err(),
            "different live descriptor"
        );
        assert!(
            audit(&row(0, 0, 8, path.clone())).is_err(),
            "wrong index literal"
        );
        assert!(
            audit(&row(0, 1, 0, path.clone())).is_err(),
            "wrong leaf offset"
        );
    }
}

#[test]
fn retained_primitive_store_projects_indexed_leaf_and_audits_exact_geometry() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::project_write_only_primitive_store;
    use abstract_operations::StructuralScalarStoreDestination;
    use legalized_operations::LegalizedScalarInstructionKind;
    use semantic_vocabulary::ObligationId;

    let mut names = TerminalIdentityTableBuilder::default();
    let scalar = ScalarType::Integer(u64_type());
    let mut leaf = reference_type(1, names.intern("Scalar"), ty(1));
    leaf.shape = StructuralTypeShape::PrimitiveScalar(scalar);
    let array = |id, identity, element| {
        let mut array = reference_type(id, identity, ty(id));
        array.shape = StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(2),
        };
        array
    };
    let inner = array(2, names.intern("Row"), ty(1));
    let outer = array(3, names.intern("Matrix"), ty(2));
    let mut parameter = parameter(ty(3));
    parameter.access = StructuralAccess::WriteOnlyBorrow;
    let destination = StructuralScalarStoreDestination::Parameter(parameter.clone());
    let bound = ObligationId::new(1).unwrap();
    let path = vec![
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::RuntimeIndex {
            index: val(1),
            obligation: bound,
        },
    ];
    let value = AbstractResult {
        value: val(2),
        scalar_type: scalar,
    };
    let store = AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation: op(3),
        destination: destination.clone(),
        path: path.clone(),
        value,
        range_obligation: None,
    };
    let constant = |ordinal, value| AbstractOperation::IntegerConstant {
        psi_operation: op(ordinal),
        result: val(ordinal),
        scalar_type: scalar,
        value: IntegerValue::Unsigned(value),
    };
    let function = function_with_body(
        vec![parameter],
        vec![entry(
            1,
            vec![
                constant(1, 1),
                constant(2, 93),
                store.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![leaf, inner, outer], function);
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let parameter = &target.functions[0].graph.parameters[0];
    let immediate = |ordinal, value| TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(ordinal),
        source_value: val(ordinal),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(value),
    };
    let selector = immediate(1, 1);
    let store_value = immediate(2, 93);
    let row = |byte_offset, byte_size, stride| TargetUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation: op(3),
        destination: destination.clone(),
        path: path.clone(),
        destination_type: plan.structural_types[2].clone(),
        destination_storage: target_operations::TargetStructuralArgumentSource::Placement(
            parameter.placement.clone(),
        ),
        source: store_value,
        byte_offset,
        byte_size,
        indices: vec![target_operations::TargetStructuralRuntimeIndex {
            operand: selector,
            stride,
            extent: Some(2),
            obligation: bound,
        }],
    };
    let retained = row(16, 8, 8);
    let fact = optimization_unit::AcceptedObligationFact::new(
        plan.psi,
        [7; 32],
        machine(),
        optimization_unit::PsiProvenance::Operation(op(3)),
        bound,
        vec![2],
    );
    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
        destination: projected_destination,
        path: projected_path,
        value: projected_value,
        byte_offset,
        byte_size,
        indices,
    } = project_write_only_primitive_store(machine(), std::slice::from_ref(&fact), &retained)
        .unwrap()
    else {
        panic!("primitive store");
    };
    assert_eq!(projected_destination, destination);
    assert_eq!(projected_path, path);
    assert_eq!(projected_value, value);
    assert_eq!((byte_offset, byte_size), (16, 8));
    assert_eq!(indices.len(), 1);
    assert_eq!(
        (
            indices[0].operand.value,
            indices[0].stride,
            indices[0].extent,
            indices[0].accepted_fact
        ),
        (val(1), 8, Some(2), fact.identity)
    );
    assert!(project_write_only_primitive_store(machine(), &[], &retained).is_err());

    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let inputs = crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let sources = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = sources.available(block_id(1));
        available.push((val(1), selector));
        available.push((val(2), store_value));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &store,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&retained).expect("target store agrees with the exact source leaf");
    assert!(audit(&row(0, 8, 8)).is_err(), "neighbor row offset");
    assert!(audit(&row(16, 4, 8)).is_err(), "narrower store width");
    assert!(audit(&row(16, 8, 16)).is_err(), "neighbor stride");
}

#[test]
fn retained_direct_byte_access_projects_and_audits_exact_operands() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::project_byte_access;
    use legalized_operations::LegalizedScalarInstructionKind as Kind;
    use semantic_vocabulary::ObligationId;
    use target_operations::{TargetByteView, TargetIntegerExpression as Expression};
    use target_operations::{TargetScalarExpression, TargetUnitScalarHomeRequirement};

    let mut names = TerminalIdentityTableBuilder::default();
    let mut bytes = reference_type(1, names.intern("Bytes"), ty(1));
    bytes.shape =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        });
    let mut receiver = parameter(ty(1));
    receiver.access = StructuralAccess::MutableBorrow;
    let place = receiver.place;
    let bound = ObligationId::new(1).unwrap();
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let result = AbstractResult {
        value: val(3),
        scalar_type: ScalarType::Integer(byte),
    };
    let read = AbstractOperation::ByteSequenceRead {
        psi_operation: op(3),
        result,
        source: place,
        index: val(1),
        length: val(2),
        obligation: bound,
    };
    let write = AbstractOperation::ByteSequenceWrite {
        psi_operation: op(4),
        destination: place,
        index: val(1),
        value: result.value,
        length: val(2),
        obligation: bound,
    };
    let function = function_with_body(
        vec![receiver],
        vec![entry(
            1,
            vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: op(1),
                    result: val(1),
                    scalar_type: ScalarType::Integer(u64_type()),
                    value: IntegerValue::Unsigned(1),
                },
                AbstractOperation::ByteSequenceLength {
                    psi_operation: op(2),
                    result: AbstractResult {
                        value: val(2),
                        scalar_type: ScalarType::Integer(u64_type()),
                    },
                    source: terminal_psi::StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: StructuralAccess::MutableBorrow,
                    },
                },
                read.clone(),
                write.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![bytes], function);
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let parameter = &target.functions[0].graph.parameters[0];
    let view = TargetByteView::Parameter {
        place,
        placement: parameter.placement.clone(),
    };
    let home = TargetUnitScalarHomeRequirement {
        defining_operation: op(3),
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: calling_conventions::ValueShape::integer(1, 1),
    };
    let read_row = |index, length| TargetUnitOperation::ScalarDefinition {
        result_home: home,
        expression: TargetScalarExpression::Integer {
            scalar_type: byte,
            expression: Expression::ByteSequenceRead {
                psi_operation: op(3),
                source_value: result.value,
                source: place,
                view: Box::new(view.clone()),
                index: Box::new(index),
                length,
                obligation: bound,
            },
        },
    };
    let store_index = TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(1),
        source_value: val(1),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(1),
    };
    let write_row = |view, value| TargetUnitOperation::ByteSequenceWrite {
        psi_operation: op(4),
        destination: place,
        view,
        index: store_index,
        value,
        length: val(2),
        obligation: bound,
    };
    let read_index = Expression::Immediate {
        source_value: val(1),
        value: IntegerValue::Unsigned(1),
    };
    let retained_read = read_row(read_index.clone(), val(2));
    let retained_write = write_row(view.clone(), TargetUnitScalarArgumentSource::Home(home));

    let facts = [op(3), op(4)].map(|operation| {
        optimization_unit::AcceptedObligationFact::new(
            plan.psi,
            [7; 32],
            machine(),
            optimization_unit::PsiProvenance::Operation(operation),
            bound,
            vec![2],
        )
    });
    assert_eq!(
        project_byte_access(machine(), &facts, op(3), &retained_read).unwrap(),
        Kind::ByteSequenceRead {
            source: place,
            index: val(1),
            length: val(2),
            obligation: bound,
            accepted_fact: facts[0].identity,
        }
    );
    assert_eq!(
        project_byte_access(machine(), &facts, op(4), &retained_write).unwrap(),
        Kind::ByteSequenceWrite {
            destination: place,
            index: val(1),
            value: result.value,
            length: val(2),
            obligation: bound,
            accepted_fact: facts[1].identity,
        }
    );
    assert!(project_byte_access(machine(), &facts[1..], op(3), &retained_read).is_err());
    assert!(project_byte_access(machine(), &facts[..1], op(4), &retained_write).is_err());
    assert!(project_byte_access(machine(), &facts, op(3), &retained_write).is_err());

    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let inputs = crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let sources = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation, operation: &AbstractOperation| {
        let mut available = sources.available(block_id(1));
        available.push((val(1), store_index));
        if matches!(operation, AbstractOperation::ByteSequenceWrite { .. }) {
            available.push((result.value, TargetUnitScalarArgumentSource::Home(home)));
        }
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&retained_read, &read).expect("exact read");
    audit(&retained_write, &write).expect("exact write");
    assert!(audit(&read_row(read_index.clone(), val(1)), &read).is_err());
    let forged_index = Expression::Immediate {
        source_value: val(1),
        value: IntegerValue::Unsigned(0),
    };
    assert!(audit(&read_row(forged_index, val(2)), &read).is_err());
    let forged_value = TargetUnitScalarArgumentSource::Home(TargetUnitScalarHomeRequirement {
        defining_operation: op(2),
        ..home
    });
    assert!(audit(&write_row(view, forged_value), &write).is_err());
    let forged_backing = TargetByteView::Parameter {
        place: self::place(2),
        placement: parameter.placement.clone(),
    };
    assert!(
        audit(
            &write_row(forged_backing, TargetUnitScalarArgumentSource::Home(home)),
            &write
        )
        .is_err()
    );
}

#[test]
fn retained_byte_field_read_preserves_both_bounds_and_audits_the_field() {
    use crate::legalization_source::scalar_graph::instruction::storage_instructions::project_structural_byte_sequence_field_read;
    use legalized_operations::LegalizedScalarInstructionKind;
    use semantic_vocabulary::ObligationId;

    let mut names = TerminalIdentityTableBuilder::default();
    let mut field = scalar_field(1, names.intern("path"), u64_type());
    field.field_type =
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        });
    let mut sibling = field.clone();
    sibling.id = StructuralFieldId::new(2).unwrap();
    sibling.identity = names.intern("known");
    let selected_field = field.id;
    let sibling_field = sibling.id;
    let mut holder = reference_type(1, names.intern("Holder"), ty(1));
    holder.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![field, sibling],
    };
    let mut array = reference_type(2, names.intern("Holders"), ty(2));
    array.shape = StructuralTypeShape::FixedArray {
        element: ty(1),
        length: terminal_psi::ArrayExtent::Literal(4),
    };
    let parameter = parameter(ty(2));
    let selector_bound = ObligationId::new(2).unwrap();
    let byte_bound = ObligationId::new(3).unwrap();
    let source = terminal_psi::StructuralArgument {
        place: parameter.place,
        access: parameter.access,
        path: vec![StructuralPathSegment::RuntimeIndex {
            index: val(1),
            obligation: selector_bound,
        }],
    };
    let result = AbstractResult {
        value: val(4),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    };
    let operation = AbstractOperation::StructuralByteSequenceFieldRead {
        psi_operation: op(4),
        result,
        source: source.place,
        path: source.path.clone(),
        field: selected_field,
        index: val(2),
        length: val(3),
        obligation: byte_bound,
    };
    let constant = |ordinal, value| AbstractOperation::IntegerConstant {
        psi_operation: op(ordinal),
        result: val(ordinal),
        scalar_type: ScalarType::Integer(u64_type()),
        value: IntegerValue::Unsigned(value),
    };
    let function = function_with_body(
        vec![parameter],
        vec![entry(
            1,
            vec![
                constant(1, 1),
                constant(2, 0),
                AbstractOperation::StructuralByteSequenceFieldLength {
                    psi_operation: op(3),
                    result: AbstractResult {
                        value: val(3),
                        scalar_type: ScalarType::Integer(u64_type()),
                    },
                    source: source.place,
                    path: source.path.clone(),
                    field: selected_field,
                },
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![holder, array], function);
    let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let integer_source = |ordinal, value| TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(ordinal),
        source_value: val(ordinal),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(value),
    };
    let selector = integer_source(1, 1);
    let index = integer_source(2, 0);
    let row = TargetUnitOperation::StructuralByteSequenceFieldRead {
        psi_operation: op(4),
        result,
        source: source.clone(),
        field: selected_field,
        indices: vec![target_operations::TargetStructuralRuntimeIndex {
            operand: selector,
            stride: 32,
            extent: Some(4),
            obligation: selector_bound,
        }],
        index,
        length: val(3),
        obligation: byte_bound,
    };
    let facts = [selector_bound, byte_bound].map(|obligation| {
        optimization_unit::AcceptedObligationFact::new(
            plan.psi,
            [7; 32],
            machine(),
            optimization_unit::PsiProvenance::Operation(op(4)),
            obligation,
            vec![2],
        )
    });
    let project = |facts| project_structural_byte_sequence_field_read(machine(), facts, &row);
    let LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
        source: projected_source,
        field,
        indices,
        index: projected_index,
        length,
        obligation,
        accepted_fact,
    } = project(&facts).unwrap()
    else {
        panic!("byte field read");
    };
    assert_eq!(projected_source, source);
    assert_eq!(
        (field, projected_index, length, obligation),
        (selected_field, val(2), val(3), byte_bound)
    );
    assert_eq!(accepted_fact, facts[1].identity);
    assert_eq!(indices.len(), 1);
    assert_eq!(
        (
            indices[0].operand.value,
            indices[0].stride,
            indices[0].extent,
            indices[0].accepted_fact
        ),
        (val(1), 32, Some(4), facts[0].identity)
    );
    assert!(
        project(&facts[..1]).is_err(),
        "each byte owes its own bound"
    );
    assert!(
        project(&facts[1..]).is_err(),
        "the enclosing array owes a separate bound"
    );

    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let inputs = crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let sources = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = sources.available(block_id(1));
        available.push((val(1), selector));
        available.push((val(2), index));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("read agrees with the exact field, selector and bounds");
    #[derive(Debug)]
    enum Damage {
        SiblingField,
        DifferentLength,
        WrongStride,
    }
    for damage in [
        Damage::SiblingField,
        Damage::DifferentLength,
        Damage::WrongStride,
    ] {
        let mut changed = row.clone();
        let TargetUnitOperation::StructuralByteSequenceFieldRead {
            field,
            length,
            indices,
            ..
        } = &mut changed
        else {
            panic!("byte field read");
        };
        match damage {
            Damage::SiblingField => *field = sibling_field,
            Damage::DifferentLength => *length = val(2),
            Damage::WrongStride => indices[0].stride = 16,
        }
        assert!(audit(&changed).is_err(), "{damage:?}");
    }
}

#[test]
fn retained_byte_length_audit_reconstructs_the_descriptor_and_exact_fold() {
    use target_operations::TargetIntegerExpression as Expression;
    use terminal_psi::ByteSequenceCarrier;

    let borrowed = ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::SharedBorrow),
    };
    let bounded = ByteSequenceCarrier::BoundedView {
        access: Some(StructuralAccess::SharedBorrow),
        capacity: 19,
    };
    for carrier in [borrowed, bounded] {
        let mut names = TerminalIdentityTableBuilder::default();
        let mut bytes = reference_type(1, names.intern("bytes"), ty(1));
        bytes.shape = StructuralTypeShape::ByteSequence(carrier);
        let parameter = parameter(ty(1));
        let source = terminal_psi::StructuralArgument {
            place: parameter.place,
            path: Vec::new(),
            access: parameter.access,
        };
        let result = AbstractResult {
            value: val(1),
            scalar_type: ScalarType::Integer(u64_type()),
        };
        let operation = AbstractOperation::ByteSequenceLength {
            psi_operation: op(1),
            result,
            source: source.clone(),
        };
        let function = function_with_body(
            vec![parameter],
            vec![entry(
                1,
                vec![
                    operation.clone(),
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        );
        let plan = plan(names, vec![bytes], function);
        let target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
                .unwrap();
        let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
        let plan_view =
            crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
        let unit_view =
            crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
        let function_inputs =
            crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
        let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
            &target.functions[0].graph,
            &unit.functions[0],
        );
        let audit = |expression| {
            let row = TargetUnitOperation::ScalarDefinition {
                result_home: target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: op(1),
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape: crate::legalization::scalar_graph_input::scalar_shape(
                        result.scalar_type,
                    )
                    .unwrap(),
                },
                expression: target_operations::TargetScalarExpression::Integer {
                    scalar_type: u64_type(),
                    expression,
                },
            };
            crate::legalization_target::unit::validate_operation(
                &target.functions[0],
                &row,
                &operation,
                &[],
                &target.functions[0].graph.parameters,
                &mut source_inputs.available(block_id(1)),
                &Default::default(),
                &function_inputs,
                &target,
                &plan_view,
                &unit_view,
            )
        };
        let descriptor = |length_byte_offset| Expression::ByteSequenceLength {
            psi_operation: op(1),
            source_value: result.value,
            source: source.clone(),
            indices: Vec::new(),
            length_byte_offset,
        };
        let immediate = |value| Expression::Immediate {
            source_value: result.value,
            value: IntegerValue::Unsigned(value),
        };
        if carrier == borrowed {
            audit(descriptor(8)).expect("borrowed descriptor length");
            assert!(audit(immediate(19)).is_err());
        } else {
            audit(immediate(19)).expect("exact bounded length");
            assert!(audit(immediate(20)).is_err());
        }
        assert!(audit(descriptor(0)).is_err(), "{carrier:?}");
    }
}

#[test]
fn retained_view_inputs_project_without_bodies_and_keep_independent_custody() {
    use target_operations::{
        TargetByteView, TargetElementView, TargetElementViewRegion, TargetIntegerExpression,
    };
    let mut names = TerminalIdentityTableBuilder::default();
    let mut scalar = reference_type(1, names.intern("Word"), ty(1));
    scalar.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(u64_type()));
    let mut array = scalar.clone();
    array.id = ty(2);
    array.identity = names.intern("Words");
    array.shape = StructuralTypeShape::FixedArray {
        element: ty(1),
        length: terminal_psi::ArrayExtent::Literal(4),
    };
    let mut view_type = scalar.clone();
    view_type.id = ty(3);
    view_type.identity = names.intern("WordsView");
    view_type.shape = StructuralTypeShape::ElementView {
        element: ty(1),
        access: Some(StructuralAccess::SharedBorrow),
    };
    let source = terminal_psi::StructuralArgument {
        place: place(1),
        access: StructuralAccess::SharedBorrow,
        path: Vec::new(),
    };
    let result = structural_result(place(2), ty(3));
    let operation = AbstractOperation::EstablishElementView {
        psi_operation: op(1),
        result: result.clone(),
        destination: result.place,
        source: source.clone(),
        element: ty(1),
        region: terminal_psi::ElementViewRegion::WholeCollection,
    };
    let function = function_with_body(
        vec![parameter(ty(2))],
        vec![entry(
            1,
            vec![
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![scalar, array, view_type], function);
    let mut target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let row = TargetUnitOperation::EstablishElementView {
        result: result.clone(),
        view: TargetElementView::Established {
            psi_operation: op(1),
            place: result.place,
            structural_type: result.structural_type,
            element: ty(1),
            root_structural_type: ty(2),
            source_byte_offset: 0,
            indices: Vec::new(),
            extent: target_operations::TargetViewExtent::Fixed(4),
            region: TargetElementViewRegion::WholeCollection,
            element_stride: 8,
            source: target_operations::TargetStructuralArgumentSource::Placement(
                target.functions[0].graph.parameters[0].placement.clone(),
            ),
            source_argument: source.clone(),
        },
    };
    target.functions[0].graph.blocks[0]
        .operations
        .push(target_operations::TargetInstruction {
            source: None,
            kind: row.clone(),
        });
    let project_view = |row: &TargetUnitOperation| {
        crate::legalization_source::scalar_graph::instruction::storage_instructions::project_established_view(machine(), &[], row)
    };
    let lowered = project_view(&row).unwrap();
    let legalized_operations::LegalizedScalarInstructionKind::EstablishElementView {
        source: actual_source,
        result: actual_result,
        ..
    } = lowered
    else {
        panic!("view");
    };
    assert_eq!(
        (actual_source, actual_result),
        (source.clone(), result.clone())
    );

    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let function_inputs =
        crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut source_inputs.available(block_id(1)),
            &Default::default(),
            &function_inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("view source and physical backing match the independent source");
    for mutation in 0..3 {
        let mut forged = row.clone();
        let TargetUnitOperation::EstablishElementView {
            view: TargetElementView::Established {
                source_argument, ..
            },
            ..
        } = &mut forged
        else {
            panic!("view");
        };
        match mutation {
            0 => source_argument.place = place(9),
            1 => source_argument.access = StructuralAccess::MutableBorrow,
            _ => source_argument
                .path
                .push(StructuralPathSegment::FixedIndex(1)),
        }
        assert!(audit(&forged).is_err(), "forged source argument {mutation}");
    }

    let start = ValueId::new(7).unwrap();
    let end = ValueId::new(8).unwrap();
    let obligation = semantic_vocabulary::ObligationId::new(1).unwrap();
    let fact = optimization_unit::AcceptedObligationFact::new(
        plan.psi,
        [6; 32],
        machine(),
        optimization_unit::PsiProvenance::Operation(op(1)),
        obligation,
        vec![7],
    );
    let immediate = || {
        Box::new(TargetIntegerExpression::Immediate {
            source_value: start,
            value: IntegerValue::Unsigned(0),
        })
    };
    let home = || {
        Box::new(TargetIntegerExpression::ScalarHome(
            target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: op(2),
                source_value: end,
                scalar_type: ScalarType::Integer(u64_type()),
                shape: calling_conventions::ValueShape::integer(8, 8),
            },
        ))
    };
    for byte in [false, true] {
        let row = if byte {
            TargetUnitOperation::ByteSequenceSubslice {
                result: result.clone(),
                view: TargetByteView::Subslice {
                    psi_operation: op(1),
                    place: result.place,
                    source: source.clone(),
                    indices: Vec::new(),
                    start: immediate(),
                    end: home(),
                    length: ValueId::new(9).unwrap(),
                    obligation,
                },
            }
        } else {
            TargetUnitOperation::ElementViewSubslice {
                result: result.clone(),
                view: TargetElementView::Subslice {
                    psi_operation: op(1),
                    place: result.place,
                    source: source.clone(),
                    indices: Vec::new(),
                    start: Box::new(TargetIntegerExpression::Parameter {
                        source_value: start,
                        parameter_index: 0,
                        location: target_operations::ScalarParameterLocation::Register(
                            calling_conventions::MachineRegister::X86Rdi,
                        ),
                    }),
                    end: Box::new(TargetIntegerExpression::BlockParameter(
                        target_operations::TargetScalarBlockValue {
                            block: block_id(1),
                            value: end,
                            scalar_type: ScalarType::Integer(u64_type()),
                        },
                    )),
                    length: ValueId::new(9).unwrap(),
                    obligation,
                },
            }
        };
        let project = |machine,
                       facts: &[optimization_unit::AcceptedObligationFact],
                       row: &TargetUnitOperation| {
            crate::legalization_source::scalar_graph::instruction::storage_instructions::project_subslice(machine, facts, row)
        };
        let lowered = project(machine(), std::slice::from_ref(&fact), &row).unwrap();
        match lowered {
            legalized_operations::LegalizedScalarInstructionKind::ByteSequenceSubslice {
                start: actual_start,
                end: actual_end,
                accepted_fact,
                ..
            } if byte => assert_eq!(
                (actual_start, actual_end, accepted_fact),
                (start, end, fact.identity)
            ),
            legalized_operations::LegalizedScalarInstructionKind::ElementViewSubslice {
                start: actual_start,
                end: actual_end,
                accepted_fact,
                ..
            } if !byte => assert_eq!(
                (actual_start, actual_end, accepted_fact),
                (start, end, fact.identity)
            ),
            _ => panic!("subslice kind changed"),
        }
        assert!(project(machine(), &[], &row).is_err());
        assert!(
            project(
                MachineId::new(2).unwrap(),
                std::slice::from_ref(&fact),
                &row
            )
            .is_err()
        );
        let mut unsupported = row;
        let (TargetUnitOperation::ByteSequenceSubslice {
            view: TargetByteView::Subslice { start, .. },
            ..
        }
        | TargetUnitOperation::ElementViewSubslice {
            view: TargetElementView::Subslice { start, .. },
            ..
        }) = &mut unsupported
        else {
            panic!("subslice");
        };
        **start = TargetIntegerExpression::BitwiseNot {
            psi_operation: op(3),
            operand: immediate(),
        };
        assert!(
            project(machine(), std::slice::from_ref(&fact), &unsupported).is_err(),
            "a recipe is not a materialized endpoint"
        );
    }
    let mut bytes = row;
    let TargetUnitOperation::EstablishElementView {
        view: TargetElementView::Established { region, .. },
        ..
    } = &mut bytes
    else {
        panic!("view");
    };
    *region = TargetElementViewRegion::RepresentationBytes {
        start: immediate(),
        end: home(),
        obligation,
    };
    let lowered = crate::legalization_source::scalar_graph::instruction::storage_instructions::project_established_view(machine(), std::slice::from_ref(&fact), &bytes).unwrap();
    assert!(
        matches!(lowered, legalized_operations::LegalizedScalarInstructionKind::EstablishElementView { region: terminal_psi::ElementViewRegion::RepresentationBytes { start: actual_start, end: actual_end, .. }, accepted_fact: Some(identity), .. } if actual_start == start && actual_end == end && identity == fact.identity)
    );
    assert!(project_view(&bytes).is_err());
}

#[test]
fn retained_reference_selectors_project_without_bodies_and_audit_exact_geometry() {
    let mut names = TerminalIdentityTableBuilder::default();
    let mut cell = reference_type(1, names.intern("Cell"), ty(1));
    cell.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![scalar_field(1, names.intern("value"), u64_type())],
    };
    let mut array = cell.clone();
    array.id = ty(2);
    array.identity = names.intern("Cells");
    array.shape = StructuralTypeShape::FixedArray {
        element: ty(1),
        length: terminal_psi::ArrayExtent::Literal(2),
    };
    let reference = reference_type(3, names.intern("CellRef"), ty(1));
    let obligation = semantic_vocabulary::ObligationId::new(1).unwrap();
    let index = TargetUnitScalarArgumentSource::IntegerImmediate {
        defining_operation: op(1),
        source_value: ValueId::new(1).unwrap(),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(1),
    };
    let source = terminal_psi::StructuralArgument {
        place: place(1),
        access: StructuralAccess::SharedBorrow,
        path: vec![StructuralPathSegment::RuntimeIndex {
            index: index.source_value(),
            obligation,
        }],
    };
    let mut result = structural_result(place(2), ty(3));
    result.multiplicity = StructuralMultiplicity::Affine;
    let operation = AbstractOperation::EstablishReference {
        psi_operation: op(2),
        result: result.clone(),
        source: source.clone(),
        storage: None,
    };
    let function = function_with_body(
        vec![parameter(ty(2))],
        vec![entry(
            1,
            vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: op(1),
                    result: index.source_value(),
                    scalar_type: ScalarType::Integer(u64_type()),
                    value: IntegerValue::Unsigned(1),
                },
                operation.clone(),
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    let plan = plan(names, vec![cell, array, reference], function);
    let mut target = target_plan(&plan, vec![block(1, Vec::new(), return_unit(1))]);
    let row = TargetUnitOperation::EstablishReference {
        psi_operation: op(2),
        result_home: TargetStructuralHomeRequirement {
            origin: TargetStructuralHomeOrigin::OperationResult {
                operation: op(2),
                result: result.clone(),
            },
            layout: TargetStructuralHomeLayout::Aggregate(
                calling_conventions::ValueShape::integer(8, 8),
            ),
        },
        source: source.clone(),
        storage: None,
        indices: vec![target_operations::TargetStructuralRuntimeIndex {
            operand: index,
            stride: 8,
            extent: Some(2),
            obligation,
        }],
    };
    target.functions[0].graph.blocks[0]
        .operations
        .push(target_operations::TargetInstruction {
            source: None,
            kind: row.clone(),
        });
    let fact = optimization_unit::AcceptedObligationFact::new(
        plan.psi,
        [6; 32],
        machine(),
        optimization_unit::PsiProvenance::Operation(op(2)),
        obligation,
        vec![7],
    );
    // No source function or geometry catalog is available to construction.
    let lowered = crate::legalization_source::scalar_graph::instruction::project_reference(
        machine(),
        std::slice::from_ref(&fact),
        &row,
    )
    .unwrap();
    let legalized_operations::LegalizedScalarInstructionKind::EstablishReference {
        result: actual,
        source: actual_source,
        indices,
        shape,
        ..
    } = lowered
    else {
        panic!("reference");
    };
    assert_eq!((actual, actual_source), (result, source));
    assert_eq!(shape, calling_conventions::ValueShape::integer(8, 8));
    assert_eq!(
        (
            indices[0].operand.value,
            indices[0].stride,
            indices[0].extent,
            indices[0].accepted_fact
        ),
        (index.source_value(), 8, Some(2), fact.identity)
    );
    assert!(
        crate::legalization_source::scalar_graph::instruction::project_reference(
            machine(),
            &[],
            &row
        )
        .is_err()
    );
    assert!(
        crate::legalization_source::scalar_graph::instruction::project_reference(
            MachineId::new(2).unwrap(),
            &[fact],
            &row,
        )
        .is_err()
    );

    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan_view =
        crate::native_geometry::NativeInput::with_types(&plan, &geometry, fixture_types());
    let unit_view =
        crate::native_geometry::NativeInput::with_types(&unit, &geometry, fixture_types());
    let function_inputs =
        crate::legalization_function_inputs::FunctionInputs::new(&unit.functions[0]);
    let source_inputs = crate::legalization_target::control_flow::sources::SourceInputs::new(
        &target.functions[0].graph,
        &unit.functions[0],
    );
    let audit = |row: &TargetUnitOperation| {
        let mut available = source_inputs.available(block_id(1));
        available.push((index.source_value(), index));
        crate::legalization_target::unit::validate_operation(
            &target.functions[0],
            row,
            &operation,
            &[],
            &target.functions[0].graph.parameters,
            &mut available,
            &Default::default(),
            &function_inputs,
            &target,
            &plan_view,
            &unit_view,
        )
    };
    audit(&row).expect("reference selector matches the independent source geometry");
    for mutation in 0..7 {
        let mut forged = row.clone();
        let TargetUnitOperation::EstablishReference {
            indices,
            result_home,
            source,
            ..
        } = &mut forged
        else {
            panic!("reference");
        };
        match mutation {
            0 => indices[0].stride = 4,
            1 => indices[0].extent = Some(3),
            2 => indices[0].obligation = semantic_vocabulary::ObligationId::new(2).unwrap(),
            3 => {
                let TargetUnitScalarArgumentSource::IntegerImmediate { value, .. } =
                    &mut indices[0].operand
                else {
                    panic!("index");
                };
                *value = IntegerValue::Unsigned(0);
            }
            4 => indices.clear(),
            5 => {
                result_home.layout = TargetStructuralHomeLayout::Aggregate(
                    calling_conventions::ValueShape::integer(16, 8),
                )
            }
            _ => source.path.clear(),
        }
        assert!(
            audit(&forged).is_err(),
            "forged reference geometry {mutation}"
        );
    }
}

fn tag_only() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("TagOnly"), &mut names, &[]);
    dispatch("tag-only", names, vec![sum], ty(1), Vec::new())
}

fn payload_behind_pointer() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let byte = names.intern("byte");
    let sum = sum_type(
        1,
        names.intern("Payload"),
        &mut names,
        &[scalar_field(1, byte, u32_type())],
    );
    let reference = reference_type(2, names.intern("PayloadRef"), ty(1));
    dispatch(
        "payload-behind-pointer",
        names,
        vec![sum, reference],
        ty(2),
        vec![StructuralPathSegment::Referent],
    )
}

fn tag_only_behind_pointer() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("Pointed"), &mut names, &[]);
    let reference = reference_type(2, names.intern("PointedRef"), ty(1));
    dispatch(
        "tag-only-behind-pointer",
        names,
        vec![sum, reference],
        ty(2),
        vec![StructuralPathSegment::Referent],
    )
}

fn sum_inside_record() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let prefix = names.intern("prefix");
    let body = names.intern("body");
    let sum = sum_type(1, names.intern("Inner"), &mut names, &[]);
    let record = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty(2),
        identity: names.intern("Outer"),
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![
                scalar_field(1, prefix, u32_type()),
                StructuralFieldDeclaration {
                    declaration_identity: Default::default(),
                    inline_carrier: None,
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: body,
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(ty(1)),
                    storage: None,
                },
            ],
        },
        scalar_restriction: None,
        requires: Vec::new(),
    };
    dispatch(
        "sum-inside-record",
        names,
        vec![sum, record],
        ty(2),
        vec![StructuralPathSegment::Field(body)],
    )
}

fn membership_behind_pointer() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let payload = names.intern("payload");
    let sum = sum_type(
        1,
        names.intern("Member"),
        &mut names,
        &[scalar_field(1, payload, u32_type())],
    );
    let reference = reference_type(2, names.intern("MemberRef"), ty(1));
    let plan = plan(
        names,
        vec![sum, reference],
        function_with_body(
            vec![parameter(ty(2))],
            vec![entry(
                1,
                vec![
                    AbstractOperation::StructuralCaseMembership {
                        psi_operation: op(1),
                        result: boolean_result(),
                        source: place(1),
                        path: vec![StructuralPathSegment::Referent],
                        case: case_id(1),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let geometry = geometry(&plan, ty(2), &[StructuralPathSegment::Referent]);
    assert_eq!(geometry.pointer_slots.len(), 1);
    let membership = TargetUnitOperation::StructuralCaseMembership {
        psi_operation: op(1),
        result: boolean_result(),
        source: place(1),
        path: vec![StructuralPathSegment::Referent],
        tag_byte_offset: 0,
        case: case_id(1),
        case_tag: 0,
        indices: Vec::new(),
        pointer_slots: carried_slots(&geometry),
    };
    let target = target_plan(&plan, vec![block(1, vec![membership], return_unit(1))]);
    select("membership-behind-pointer", &plan, target)
}

fn establish_case() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("Established"), &mut names, &[]);
    let result = structural_result(place(1), ty(1));
    let plan = plan(
        names,
        vec![sum],
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![
                    AbstractOperation::EstablishCase {
                        psi_operation: op(1),
                        result: result.clone(),
                        result_case: case_id(1),
                        fields: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let layout = sum_layout(&plan, ty(1));
    let target = target_plan(
        &plan,
        vec![block(
            1,
            vec![TargetUnitOperation::EstablishCase {
                psi_operation: op(1),
                result_home: home(op(1), &result, TargetStructuralHomeLayout::Sum(layout)),
                result_case: case_id(1),
                fields: Vec::new(),
            }],
            return_unit(1),
        )],
    );
    select("establish-case", &plan, target)
}

fn activation_claim() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let bound = names.intern("bound");
    let sum = sum_type(
        1,
        names.intern("Claim"),
        &mut names,
        &[scalar_field(1, bound, u64_type())],
    );
    let result = structural_result(place(1), ty(1));
    let integer = u64_type();
    let plan = plan(
        names,
        vec![sum],
        function_with_body(
            vec![parameter_at(2, ty(1))],
            vec![entry(
                1,
                vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: op(1),
                        result: val(1),
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(4),
                    },
                    AbstractOperation::EstablishActivationClaim {
                        psi_operation: op(2),
                        result: result.clone(),
                        bound: 8,
                        committed: val(1),
                        backing_type: ty(1),
                        established_case: case_id(1),
                        rejected_case: case_id(2),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let layout = sum_layout(&plan, ty(1));
    let claim = TargetUnitOperation::EstablishActivationClaim {
        psi_operation: op(2),
        result_home: home(op(2), &result, TargetStructuralHomeLayout::Sum(layout)),
        bound: 8,
        committed: TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation: op(1),
            source_value: val(1),
            scalar_type: integer,
            value: IntegerValue::Unsigned(4),
        },
        backing_type: ty(1),
        established_case: case_id(1),
        rejected_case: case_id(2),
    };
    let constant = TargetUnitOperation::IntegerConstant {
        psi_operation: op(1),
        result: val(1),
        scalar_type: integer,
        value: IntegerValue::Unsigned(4),
    };
    let target = target_plan(&plan, vec![block(1, vec![constant, claim], return_unit(1))]);
    select("activation-claim", &plan, target)
}

fn dispatch(
    name: &str,
    names: TerminalIdentityTableBuilder,
    types: Vec<StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
) -> String {
    let arms = vec![
        AbstractStructuralCaseSuccessor {
            psi_edge: edge(1),
            target: block_id(2),
            case: case_id(1),
            payloads: Vec::new(),
            structural_bindings: Vec::new(),
            cleanup_actions: Vec::new(),
        },
        AbstractStructuralCaseSuccessor {
            psi_edge: edge(2),
            target: block_id(3),
            case: case_id(2),
            payloads: Vec::new(),
            structural_bindings: Vec::new(),
            cleanup_actions: Vec::new(),
        },
    ];
    let plan = plan(
        names,
        types,
        function_with_body(
            vec![parameter(root)],
            vec![
                entry(
                    1,
                    vec![AbstractOperation::StructuralCase {
                        psi_block: block_id(1),
                        source: place(1),
                        path: path.clone(),
                        cases: arms.clone(),
                    }],
                ),
                entry(
                    2,
                    vec![AbstractOperation::ReturnUnit {
                        psi_edge: edge(3),
                        cleanup_actions: Vec::new(),
                    }],
                ),
                entry(
                    3,
                    vec![AbstractOperation::ReturnUnit {
                        psi_edge: edge(4),
                        cleanup_actions: Vec::new(),
                    }],
                ),
            ],
        ),
    );
    let geometry = geometry(&plan, root, &path);
    let selected = sum_layout(&plan, geometry.selected);
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let prepared =
        crate::native_geometry::NativeInput::with_types(&plan, &layouts, fixture_types());
    let root_layout =
        crate::legalization_scalar_graph::aggregate_results::type_home_layout(root, &prepared)
            .unwrap_or_else(|error| panic!("{name}: root layout {error:?}"));
    let source = TargetStructuralCaseSource::BorrowedParameter {
        parameter: abi_parameter(&plan, &plan.functions[0].structural_parameters[0]),
        layout: root_layout,
    };
    let terminator = TargetControlTerminator::StructuralCase {
        psi_block: block_id(1),
        indices: Vec::new(),
        source,
        path: path.clone(),
        byte_offset: geometry.byte_offset,
        layout: selected.clone(),
        pointer_slots: carried_slots(&geometry),
        region_byte_size: carried_region_size(&plan, &geometry, selected.shape.byte_size),
        cases: arms
            .iter()
            .enumerate()
            .map(|(ordinal, arm)| TargetControlCaseSuccessor {
                psi_edge: arm.psi_edge,
                case: arm.case,
                case_tag: i32::try_from(ordinal).unwrap(),
                target: arm.target,
                payloads: Vec::new(),
                structural_bindings: Vec::new(),
                structural_indices: Vec::new(),
                work: Vec::new(),
                cleanup_actions: Vec::new(),
            })
            .collect(),
    };
    let target = target_plan(
        &plan,
        vec![
            block(1, Vec::new(), terminator),
            block(2, Vec::new(), return_unit(3)),
            block(3, Vec::new(), return_unit(4)),
        ],
    );
    select(name, &plan, target)
}

fn validate_stored_geometry(
    target: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<(), crate::legalization::LegalizationError> {
    use crate::legalization_scalar_graph::structural_case::{
        dispatch_projection, membership_layout, source_result,
    };
    // These hand-built selection rows omit unrelated provenance metadata. Test
    // the validator's geometry checks directly, with the same rows selection uses.
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan = &crate::native_geometry::NativeInput::with_types(plan, &geometry, fixture_types());
    let function = &unit.functions[0];
    let inputs = crate::legalization_function_inputs::FunctionInputs::new(function);
    for block in &target.functions[0].graph.blocks {
        if let TargetControlTerminator::StructuralCase {
            source,
            path,
            pointer_slots,
            region_byte_size,
            ..
        } = &block.terminator
        {
            dispatch_projection(
                &inputs,
                source.place(),
                path,
                plan,
                pointer_slots,
                *region_byte_size,
            )?;
        }
        for operation in block.operations() {
            if let TargetUnitOperation::StructuralCaseMembership {
                source,
                path,
                case,
                pointer_slots,
                ..
            } = operation
            {
                membership_layout(
                    &inputs,
                    *source,
                    path,
                    *case,
                    plan,
                    source_result(function, *source),
                    pointer_slots,
                )?;
            }
        }
    }
    Ok(())
}

fn select(name: &str, plan: &AbstractOperationPlan, target: TargetOperationPlan) -> String {
    let unit = reconstruct_psi_optimization_unit_seed(plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap_or_else(|error| panic!("{name}: unit {error:?}"));
    // Selection trusts retained geometry; the independent validator must reject
    // a forged pointer chase or region size rather than ignore those fields.
    validate_stored_geometry(&target, plan, &unit)
        .unwrap_or_else(|error| panic!("{name}: target validation {error:?}"));
    for change_region_size in [false, true] {
        let mut forged = target.clone();
        let mut changed = false;
        for function in &mut forged.functions {
            for block in &mut function.graph.blocks {
                if let TargetControlTerminator::StructuralCase {
                    pointer_slots,
                    region_byte_size,
                    ..
                } = &mut block.terminator
                {
                    if change_region_size {
                        *region_byte_size += 1;
                    } else {
                        pointer_slots.push(TargetCasePointerSlot {
                            byte_offset: 0,
                            index_start: 0,
                            index_end: 0,
                            scalar_descriptor: false,
                        });
                    }
                    changed = true;
                }
                if !change_region_size {
                    for operation in &mut block.operations {
                        if let TargetUnitOperation::StructuralCaseMembership {
                            pointer_slots, ..
                        } = &mut operation.kind
                        {
                            pointer_slots.push(TargetCasePointerSlot {
                                byte_offset: 0,
                                index_start: 0,
                                index_end: 0,
                                scalar_descriptor: false,
                            });
                            changed = true;
                        }
                    }
                }
            }
        }
        if changed {
            assert!(
                validate_stored_geometry(&forged, plan, &unit).is_err(),
                "{name}: forged retained geometry accepted (region={change_region_size})"
            );
        }
    }
    let module = TerminalModule::for_entry(plan.entry);
    let interrupts = provider_admission::AdmittedInstalledInterruptCohort::empty(plan.psi);
    let prepared = crate::legalization::legalize_target_operations(
        &target,
        plan,
        &unit,
        BoundaryOriginFacts {
            program_entry: None,
            assumed_bindings: &[],
            target_context: None,
            module: &module,
        },
        &interrupts,
        fixture_types(),
    )
    .unwrap_or_else(|error| panic!("{name}: prepare {error:?}"));
    let environment = register_environment::baseline_target_register_environment(target.target)
        .unwrap_or_else(|error| panic!("{name}: registers {error:?}"));
    // From here selection sees the prepared plan and nothing behind it.
    let selected = target_operations_to_selected_instructions::select_instructions_for_plan(
        &prepared,
        &provider_admission::installed_interrupt_helpers::bind_installations(
            &interrupts,
            None,
            &[],
        ),
        &environment,
    )
    .unwrap_or_else(|error| panic!("{name}: select {error:?}"));
    let mut text = format!("== {name}\n");
    for function in selected.plan().functions.iter() {
        for slot in &function.local_storage_slots {
            text.push_str(&format!(
                "slot size={} align={}\n",
                slot.byte_size, slot.alignment
            ));
        }
        for block in function.blocks.iter() {
            text.push_str(&format!("block {:?} {:?}\n", block.id, block.origin));
            for instruction in &block.instructions {
                text.push_str(&format!("  {}\n", opcode(instruction)));
            }
            text.push_str(&format!(
                "  term {}\n",
                opcode(term_instruction(&block.terminator))
            ));
            for successor in term_successors(&block.terminator) {
                if let Some(case) = &successor.structural_case {
                    text.push_str(&format!(
                        "  case tag={} size={}\n",
                        case.case_tag,
                        case.source
                            .byte_size()
                            .map(|size| size.to_string())
                            .unwrap_or_else(|| "-".to_owned())
                    ));
                }
            }
        }
        for access in &function.memory_accesses {
            text.push_str(&format!(
                "access offset={} count={}\n",
                access.byte_offset, access.byte_count
            ));
        }
    }
    text
}

fn opcode(instruction: &SelectedInstruction) -> String {
    match instruction.kind {
        SelectedInstructionKind::MaterializeI64 { value } => format!("MaterializeI64 {value:?}"),
        other => format!("{other:?}"),
    }
}

fn term_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::EliminateEmpty { instruction, .. }
        | SelectedTerminator::InvokeNoNormal { instruction, .. }
        | SelectedTerminator::OperationInvokeNoNormal { instruction, .. }
        | SelectedTerminator::OperationTrap { instruction, .. }
        | SelectedTerminator::Crash { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

fn term_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchU64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        _ => Vec::new(),
    }
}

fn carried_slots(
    geometry: &crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
) -> Vec<TargetCasePointerSlot> {
    geometry
        .pointer_slots
        .iter()
        .map(|slot| TargetCasePointerSlot {
            byte_offset: slot.byte_offset,
            index_start: u32::try_from(slot.indices.start).expect("pointer index"),
            index_end: u32::try_from(slot.indices.end).expect("pointer index"),
            scalar_descriptor: slot.scalar_descriptor,
        })
        .collect()
}

fn carried_region_size(
    plan: &AbstractOperationPlan,
    geometry: &crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
    sum_size: u32,
) -> u32 {
    if geometry.pointer_slots.is_empty() && geometry.region_root == geometry.selected {
        sum_size
    } else {
        let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
        crate::structural_inputs::structural_reference_input::shape(geometry.region_root, &layouts)
            .expect("region size")
            .byte_size
    }
}

fn geometry(
    plan: &AbstractOperationPlan,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    crate::structural_inputs::structural_reference_input::reference_source_geometry(
        root, path, &layouts,
    )
    .expect("sum geometry")
}

fn sum_layout(
    plan: &AbstractOperationPlan,
    sum: StructuralTypeId,
) -> calling_conventions::ConventionalSumLayout {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan = crate::native_geometry::NativeInput::with_types(plan, &layouts, fixture_types());
    crate::legalization_scalar_graph::aggregate_results::sum_type_layout(sum, &plan)
        .expect("sum layout")
}

fn plan(
    names: TerminalIdentityTableBuilder,
    types: Vec<StructuralTypeDeclaration>,
    function: AbstractFunction,
) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: psi(),
        entry: machine(),
        structural_types: types.into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: Default::default(),
        identities: names.build(),
        functions: vec![function],
    }
}

fn function_with_body(
    structural_parameters: Vec<StructuralParameterDeclaration>,
    blocks: Vec<AbstractBlock>,
) -> AbstractFunction {
    AbstractFunction {
        machine: machine(),
        attachment: None,
        entry: block_id(1),
        parameters: Vec::new(),
        structural_parameters,
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        blocks,
    }
}

fn target_plan(
    plan: &AbstractOperationPlan,
    mut blocks: Vec<TargetControlBlock>,
) -> TargetOperationPlan {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let function = &plan.functions[0];
    let unit = reconstruct_psi_optimization_unit_seed(plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap();
    for block in &mut blocks {
        let source = unit.functions[0].block(block.block).expect("source block");
        for (position, instruction) in block.operations.iter_mut().enumerate() {
            if matches!(&instruction.kind, TargetUnitOperation::CleanupCall { .. }) {
                continue;
            }
            let node = &source.nodes[position];
            instruction.source = Some(target_operations::TargetInstructionSource {
                operation: node.operation.psi_operation().expect("source instruction"),
                result: node.definitions.first().copied(),
                metadata: target_operations::TargetOperationMetadata {
                    work: node.work.clone(),
                    effect: node.effect,
                    ownership: node.ownership.clone(),
                },
            });
        }
        let terminal = unit.functions[0]
            .block(block.block)
            .unwrap()
            .nodes
            .last()
            .unwrap();
        block.terminator_metadata = target_operations::TargetOperationMetadata {
            work: terminal.work.clone(),
            effect: terminal.effect,
            ownership: terminal.ownership.clone(),
        };
        let edge_work = |edge| {
            terminal
                .successors
                .iter()
                .find(|candidate| candidate.psi_edge == edge)
                .unwrap()
                .work
                .clone()
        };
        match &mut block.terminator {
            TargetControlTerminator::Jump { successor } => {
                successor.work = edge_work(successor.psi_edge)
            }
            TargetControlTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                when_true.work = edge_work(when_true.psi_edge);
                when_false.work = edge_work(when_false.psi_edge);
            }
            TargetControlTerminator::StructuralCase { cases, .. } => {
                for case in cases {
                    case.work = edge_work(case.psi_edge);
                }
            }
            _ => {}
        }
    }
    let shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            crate::structural_inputs::parameter_shape(parameter, &layouts).expect("parameter shape")
        })
        .collect();
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: shapes,
            result: None,
        },
    )
    .expect("call plan");
    let parameters = function
        .structural_parameters
        .iter()
        .zip(&call_plan.parameters)
        .map(|(parameter, placement)| TargetStructuralParameter {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            projected_qualifications: parameter.projected_qualifications.clone(),
            shape: placement.shape,
            placement: placement.clone(),
        })
        .collect();
    TargetOperationPlan {
        psi: plan.psi,
        target: HOST(),
        profile: None,
        entry: plan.entry,
        functions: vec![TargetFunction {
            machine: function.machine,
            attachment: None,
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_contract: (!function.structural_parameters.is_empty()
                || !unit.functions[0].declared_places.is_empty())
            .then(|| target_operations::TargetStructuralContract {
                parameters: function.structural_parameters.clone(),
                places: unit.functions[0].structural_places.clone(),
                entry_claims: function.entry_claims.clone(),
                published_service_ceiling: function.published_service_ceiling.clone(),
                result: function.result.structural().cloned(),
            }),
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: Vec::new(),
            },
            graph: TargetControlGraph {
                structural_types: plan.structural_types.clone(),
                call_plan,
                scalar_parameters: Vec::new(),
                parameters,
                entry: function.entry,
                blocks,
            },
        }],
        native_callback_arguments: Vec::new(),
        borrowed_storage_layouts: Default::default(),
    }
}

fn abi_parameter(
    plan: &AbstractOperationPlan,
    parameter: &StructuralParameterDeclaration,
) -> TargetStructuralParameter {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let shape =
        crate::structural_inputs::parameter_shape(parameter, &layouts).expect("parameter shape");
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .expect("parameter call plan");
    let placement = call_plan
        .parameters
        .into_iter()
        .next()
        .expect("parameter placement");
    TargetStructuralParameter {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        projected_qualifications: parameter.projected_qualifications.clone(),
        shape: placement.shape,
        placement,
    }
}

fn block(
    id: u64,
    operations: Vec<TargetUnitOperation>,
    terminator: TargetControlTerminator,
) -> TargetControlBlock {
    TargetControlBlock {
        block: block_id(id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: operations
            .into_iter()
            .map(|kind| target_operations::TargetInstruction { kind, source: None })
            .collect(),
        terminator,
        terminator_metadata: target_operations::TargetOperationMetadata {
            work: Vec::new(),
            effect: optimization_unit::EffectLink {
                input: 0,
                output: 0,
            },
            ownership: Vec::new(),
        },
    }
}

fn return_unit(id: u64) -> TargetControlTerminator {
    TargetControlTerminator::Return {
        psi_edge: edge(id),
        cleanup_actions: Vec::new(),
    }
}

fn home(
    operation: OperationId,
    result: &StructuralOperationResult,
    layout: TargetStructuralHomeLayout,
) -> TargetStructuralHomeRequirement {
    TargetStructuralHomeRequirement {
        origin: TargetStructuralHomeOrigin::OperationResult {
            operation,
            result: result.clone(),
        },
        layout,
    }
}

fn entry(id: u64, operations: Vec<AbstractOperation>) -> AbstractBlock {
    AbstractBlock {
        block: block_id(id),
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        operations,
    }
}

fn parameter(structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    parameter_at(1, structural_type)
}

fn parameter_at(id: u64, structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(id),
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }
}

fn structural_result(
    place: PlaceId,
    structural_type: StructuralTypeId,
) -> StructuralOperationResult {
    StructuralOperationResult {
        place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    }
}

fn sum_type(
    id: u64,
    identity: TerminalIdentityIndex,
    names: &mut TerminalIdentityTableBuilder,
    payload: &[StructuralFieldDeclaration],
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty(id),
        identity,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: case_id(1),
                    identity: names.intern("Left"),
                    fields: payload.to_vec(),
                },
                StructuralCaseDeclaration {
                    declaration_identity: Default::default(),
                    id: case_id(2),
                    identity: names.intern("Right"),
                    fields: Vec::new(),
                },
            ],
        },
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

fn reference_type(
    id: u64,
    identity: TerminalIdentityIndex,
    referent: StructuralTypeId,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty(id),
        identity,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        },
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

fn scalar_field(
    id: u64,
    identity: TerminalIdentityIndex,
    integer: IntegerType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(id).unwrap(),
        identity,
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
        storage: None,
    }
}

fn u32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn psi() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}

fn machine() -> MachineId {
    MachineId::new(1).unwrap()
}

fn block_id(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

fn op(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}

fn edge(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}

fn val(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}

fn ty(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn case_id(value: u64) -> StructuralCaseId {
    StructuralCaseId::new(value).unwrap()
}

fn boolean_result() -> AbstractResult {
    AbstractResult {
        value: val(1),
        scalar_type: ScalarType::Boolean,
    }
}

// Exercise the metadata boundary without asserting program acceptance.
#[test]
fn ordinary_comparison_metadata_requires_the_module_to_retain_its_operand_type() {
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    let mut function = function_with_body(
        Vec::new(),
        vec![entry(
            1,
            vec![
                AbstractOperation::IntegerEqual {
                    psi_operation: op(1),
                    result: ValueId::new(3).unwrap(),
                    left: ValueId::new(1).unwrap(),
                    right: ValueId::new(2).unwrap(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: Vec::new(),
                },
            ],
        )],
    );
    function.parameters = [1, 2]
        .map(|id| abstract_operations::AbstractParameter {
            value: ValueId::new(id).unwrap(),
            scalar_type: scalar,
        })
        .to_vec();
    let plan = plan(
        TerminalIdentityTableBuilder::default(),
        Vec::new(),
        function,
    );
    let unit = reconstruct_psi_optimization_unit_seed(&plan, WorkScheduleIdentity::new(1).unwrap())
        .expect("well-formed comparison metadata seed");
    let function = &unit.functions[0];
    let inputs = crate::legalization_function_inputs::FunctionInputs::new(function);
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let retaining = crate::native_geometry::ModuleTypes::of(&module_retaining(&[scalar]));
    let retained = crate::native_geometry::NativeInput::with_types(&plan, &geometry, &retaining);
    assert!(
        crate::legalization_scalar_graph::nodes::validate(
            &function.blocks[0],
            &inputs,
            &retained,
            &[],
            &plan.identities,
        )
        .is_ok(),
        "the comparison's operand type is one the module retains"
    );
    let closed = terminal_codec::ClosedTerminalModule::new(TerminalModule::declarations()).unwrap();
    assert!(closed.closed_types().scalar(scalar).is_err());
    let missing = crate::native_geometry::ModuleTypes::of(&closed);
    let missing = crate::native_geometry::NativeInput::with_types(&plan, &geometry, &missing);
    assert!(
        crate::legalization_scalar_graph::nodes::validate(
            &function.blocks[0],
            &inputs,
            &missing,
            &[],
            &plan.identities,
        )
        .is_err(),
        "a type the module does not retain refuses the comparison"
    );
}

#[test]
fn qualified_numeric_replay_needs_the_module_and_actual_numeric_meaning() {
    use semantic_vocabulary::{BoundaryMachineId, QualifiedScalarType, ScalarQualificationSetId};
    use target_operations::{
        BoundaryExecutionBinding, CompilerBuiltinExecution, IeeeFloatArithmeticOperation,
        IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain, NearestIeeeFloatArithmeticRealization,
        TargetUnitScalarHomeRequirement,
    };
    use terminal_psi::{
        BoundaryDeclarationRole, BoundaryMachineDeclaration, BoundaryMachineResult,
        NominalDeclarationIdentity,
    };
    let mut names = TerminalIdentityTableBuilder::default();
    let identity = names.intern("untrusted qualified numeric declaration");
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let boundary = BoundaryMachineId::new(1).unwrap();
    let declaration = BoundaryMachineDeclaration {
        id: boundary,
        declaration: None,
        numeric_intrinsic: None,
        role: BoundaryDeclarationRole::Callable,
        kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
        completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
        unconditional_termination: true,
        identity,
        owner: NominalDeclarationIdentity::new(identity),
        attachment: None,
        parameter_order: vec![],
        scalar_parameters: vec![],
        crash_routes: vec![],
        structural_parameters: vec![],
        result: BoundaryMachineResult::Scalar(QualifiedScalarType {
            scalar_type: scalar,
            qualifications: ScalarQualificationSetId::new(1),
        }),
        result_reference_regions: vec![],
        result_lifetimes: Some(vec![]),
        requires: vec![],
        normal_qualifications: vec![],
        scalar_domain_requirements: vec![],
        scalar_requires: vec![],
        scalar_ensures: vec![],
        reference_outputs: vec![],
        program_local_root_introductions: vec![],
        content_guarantees: vec![],
        fixed_service_reach: vec![],
        service_reach_is_installation_bound: false,
        owning_service: None,
        published_service_ceiling: vec![],
        published_synchronous_entries: vec![],
    };
    let plan = plan(
        names,
        vec![],
        function_with_body(
            vec![],
            vec![entry(
                1,
                vec![AbstractOperation::ReturnUnit {
                    psi_edge: edge(1),
                    cleanup_actions: vec![],
                }],
            )],
        ),
    );
    let mut target = target_plan(&plan, vec![block(1, vec![], return_unit(1))]);
    // An unrelated qualified foreign declaration is not a numeric demand.
    crate::legalization_scalar_graph::call_origin::validate_qualified_numeric_executions(
        &target,
        std::slice::from_ref(&declaration),
        None,
    )
    .unwrap();
    let realization = NearestIeeeFloatArithmeticRealization {
        operation: IeeeFloatArithmeticOperation::TruncateToInteger {
            target: IeeeFloatIntegerCarrier::I32,
            domain: IeeeFloatToIntegerDomain::Saturating,
        },
        format: semantic_vocabulary::IeeeFloatFormat::Binary32,
        x86_scalar_fma: None,
    };
    target.functions[0].graph.blocks[0]
        .operations
        .push(target_operations::TargetInstruction {
            source: None,
            kind: TargetUnitOperation::NearestIeeeFloatArithmetic {
                psi_operation: op(1),
                boundary,
                execution: BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                        operation: realization.operation,
                        format: realization.format,
                    },
                ),
                realization,
                result_home: TargetUnitScalarHomeRequirement {
                    defining_operation: op(1),
                    source_value: ValueId::new(1).unwrap(),
                    scalar_type: scalar,
                    shape: crate::legalization::scalar_graph_input::scalar_shape(scalar).unwrap(),
                },
                arguments: vec![],
            },
        });
    assert!(
        crate::legalization_scalar_graph::call_origin::validate_qualified_numeric_executions(
            &target,
            std::slice::from_ref(&declaration),
            None
        )
        .is_err()
    );
    let module = TerminalModule {
        boundary_machines: vec![declaration.clone()],
        ..TerminalModule::declarations()
    };
    assert!(
        crate::legalization_scalar_graph::call_origin::validate_qualified_numeric_executions(
            &target,
            std::slice::from_ref(&declaration),
            Some(&module)
        )
        .is_err()
    );
}

/// The types the fixtures' module retains: every integer width these
/// hand-built plans use, closed the way preparation closes an accepted
/// module's types.
fn fixture_types() -> &'static crate::native_geometry::ModuleTypes {
    static TYPES: std::sync::LazyLock<crate::native_geometry::ModuleTypes> =
        std::sync::LazyLock::new(|| {
            let integer =
                |bits| ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, bits).unwrap());
            crate::native_geometry::ModuleTypes::of(&module_retaining(&[
                integer(8),
                integer(32),
                integer(64),
            ]))
        });
    &TYPES
}

/// A closed module that retains exactly these scalar types, each declared
/// as a primitive scalar, so a fixture closes types the way preparation
/// closes them against an accepted module.
fn module_retaining(scalars: &[ScalarType]) -> terminal_codec::ClosedTerminalModule {
    let mut module = TerminalModule::declarations();
    let mut identities = TerminalIdentityTableBuilder::default();
    let rows = scalars
        .iter()
        .enumerate()
        .map(|(position, scalar)| {
            let identity = identities.intern(&format!("retained_scalar_{position}"));
            (identity, *scalar)
        })
        .collect::<Vec<_>>();
    module.identities = identities.build();
    for (position, (identity, scalar)) in rows.into_iter().enumerate() {
        module.structural_types.push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: StructuralTypeId::new(u64::try_from(position).unwrap() + 1).unwrap(),
            identity,
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
        });
    }
    terminal_codec::ClosedTerminalModule::new(module).unwrap()
}
