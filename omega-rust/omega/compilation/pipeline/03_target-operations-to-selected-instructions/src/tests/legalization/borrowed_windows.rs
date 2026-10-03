//! A borrowed window's move and store legalize as the two directions of one
//! extent copy.
//!
//! `self.inner = copy` over a mutable-borrowed `Outer { tag: i32, inner:
//! Inner }`: a leaf copy produces the replacement, the window move copies the
//! displaced `inner` out at offset 8, and the store copies the replacement's
//! home back to offset 8.
use crate::legalize_target_operations;
use abstract_operations::AbstractOperation;
use abstract_operations_to_target_operations::TargetLoweringRequest;
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId,
};
use target::NativeTarget;
use target_operations::TargetUnitOperation;
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

fn integer(bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, bits).unwrap())
}

fn field(id: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type,
    }
}

fn record(
    id: u64,
    identity: &str,
    fields: Vec<StructuralFieldDeclaration>,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: identity.into(),
        shape: StructuralTypeShape::Record { fields },
    }
}

fn inner() -> StructuralTypeId {
    StructuralTypeId::new(10).unwrap()
}

fn operation(id: u64) -> OperationId {
    OperationId::new(id).unwrap()
}

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}

fn root(access: StructuralAccess) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(20).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn result(id: u64) -> StructuralOperationResult {
    StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: place(id),
        structural_type: inner(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    }
}

/// Copy `inner` out as the replacement, open the window on `inner`, and
/// reseat it with the copy.
fn source(access: StructuralAccess) -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    for declaration in [
        record(
            10,
            "test::Inner",
            vec![
                field(1, "first", StructuralFieldType::Scalar(integer(64))),
                field(2, "second", StructuralFieldType::Scalar(integer(32))),
            ],
        ),
        record(
            20,
            "test::Outer",
            vec![
                field(1, "tag", StructuralFieldType::Scalar(integer(32))),
                field(2, "inner", StructuralFieldType::Structural(inner())),
            ],
        ),
    ] {
        source.structural_types.make_mut().push(declaration);
    }
    let function = &mut source.functions[0];
    function.structural_parameters.push(root(access));
    function.operations.splice(
        0..0,
        [
            AbstractOperation::StructuralLeafCopy {
                psi_operation: operation(1),
                result: result(2),
                source: place(1),
                path: vec![StructuralPathSegment::Field("inner".into())].into(),
            },
            AbstractOperation::MoveStructuralField {
                psi_operation: operation(2),
                result: result(3),
                source: abstract_operations::StructuralWindowRoot::BorrowedParameter(root(access)),
                path: Vec::new(),
                field: Some(StructuralFieldId::new(2).unwrap()),
            },
            AbstractOperation::StoreStructuralField {
                psi_operation: operation(3),
                destination: abstract_operations::StructuralWindowRoot::BorrowedParameter(root(
                    access,
                )),
                path: Vec::new(),
                field: Some(StructuralFieldId::new(2).unwrap()),
                value: StructuralArgument {
                    place: place(2),
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                },
            },
        ],
    );
    source
}

fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let source = source(StructuralAccess::MutableBorrow);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(native),
    )
    .expect("a verified window pair lowers to its two extent copies");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("the window pair keeps canonical custody");
    (source, target, unit)
}

fn returned_reference_source(projected: bool) -> abstract_operations::AbstractOperationPlan {
    use abstract_operations::{
        AbstractFunctionResult, AbstractStructuralCallResult, StructuralWindowRoot,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId};
    use terminal_psi::{StructuralReferenceResultSource, StructuralResultDeclaration};

    let mut source = source(StructuralAccess::MutableBorrow);
    let bag_type = StructuralTypeId::new(40).unwrap();
    source.structural_types.make_mut().push(record(
        40,
        "test::Bag",
        vec![
            field(1, "padding", StructuralFieldType::Scalar(integer(64))),
            field(
                2,
                "outer",
                StructuralFieldType::Structural(
                    root(StructuralAccess::MutableBorrow).structural_type,
                ),
            ),
        ],
    ));
    if projected {
        source.functions[0].structural_parameters[0].structural_type = bag_type;
        let AbstractOperation::StructuralLeafCopy { path, .. } =
            &mut source.functions[0].operations[0]
        else {
            unreachable!()
        };
        *path = vec![
            StructuralPathSegment::Field("outer".into()),
            StructuralPathSegment::Field("inner".into()),
        ]
        .into();
    }
    let reference_type = StructuralTypeId::new(30).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: reference_type,
            identity: "test::OuterReference".into(),
            shape: StructuralTypeShape::Reference {
                referent: root(StructuralAccess::MutableBorrow).structural_type,
                access: StructuralAccess::MutableBorrow,
            },
        });
    source
        .structural_types
        .make_mut()
        .sort_by_key(|declaration| declaration.id);
    let reference = |place| StructuralOperationResult {
        structural_type: reference_type,
        multiplicity: StructuralMultiplicity::Affine,
        place,
        ..result(8)
    };
    let mut callee = source.functions[0].clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(2).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.structural_parameters[0].place = place(20);
    let ingress = StructuralArgument {
        place: place(20),
        access: StructuralAccess::MutableBorrow,
        path: if projected {
            vec![StructuralPathSegment::Field("outer".into())]
        } else {
            Vec::new()
        },
    };
    callee.result = AbstractFunctionResult::Structural(StructuralResultDeclaration {
        place: place(22),
        structural_type: reference_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: vec![StructuralReferenceResultSource {
            path: Vec::new(),
            sources: vec![ingress.clone()],
        }],
    });
    callee.operations = vec![
        AbstractOperation::EstablishReference {
            psi_operation: operation(20),
            result: reference(place(21)),
            source: ingress,
        },
        AbstractOperation::ReturnStructural {
            psi_edge: EdgeId::new(2).unwrap(),
            source: place(21),
            returned_claims: Vec::new(),
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    let carrier = StructuralWindowRoot::OwnedResult {
        producer: operation(10),
        result: reference(place(8)),
    };
    let caller = &mut source.functions[0];
    for operation in &mut caller.operations {
        match operation {
            AbstractOperation::MoveStructuralField { source, path, .. } => {
                *source = carrier.clone();
                *path = vec![StructuralPathSegment::Referent];
            }
            AbstractOperation::StoreStructuralField {
                destination, path, ..
            } => {
                *destination = carrier.clone();
                *path = vec![StructuralPathSegment::Referent];
            }
            _ => {}
        }
    }
    // Snapshot the replacement before the call suspends the original root.
    caller.operations.insert(
        1,
        AbstractOperation::CallStructural {
            psi_operation: operation(10),
            result: AbstractStructuralCallResult::Structural {
                result: reference(place(8)),
                returned_claim_transfers: Vec::new(),
                selected_evidence: Vec::new(),
            },
            callee: callee.machine,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place(1),
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    caller.operations.insert(
        4,
        AbstractOperation::ReleaseReference {
            psi_operation: operation(11),
            source: place(8),
        },
    );
    // Reading after replacement retains the carrier's actual mutable access;
    // an observation must not rewrite that declaration as a shared loan.
    caller.operations.insert(
        4,
        AbstractOperation::StructuralScalarFieldRead {
            psi_operation: operation(30),
            result: abstract_operations::AbstractResult {
                value: semantic_vocabulary::ValueId::new(30).unwrap(),
                scalar_type: integer(64),
            },
            source: place(8),
            path: vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(2).unwrap(),
            )],
            field: StructuralFieldId::new(1).unwrap(),
        },
    );
    source.functions.push(callee);
    source
}

#[test]
fn returned_reference_window_loads_the_pointer_and_replays_exact_custody() {
    for (native, projected) in [
        (NativeTarget::linux_x64(), false),
        (NativeTarget::linux_arm64(), false),
        (NativeTarget::macos_arm64(), false),
        (NativeTarget::windows_x64(), false),
        (NativeTarget::linux_x64(), true),
    ] {
        let source = returned_reference_source(projected);
        let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        // Reference-return calls require the callee's verified contract even
        // when its requirements and crash roster are empty.
        unit.functions[1].verified_contract = Some(terminal_psi::MachineContract {
            id: semantic_vocabulary::ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        });
        unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
            StructuralAccess::Owned,
        ] {
            let mut changed = legal.plan().clone();
            let observation = changed.scalar_functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|row| row.operation == operation(30))
                .expect("read after replacement");
            let legalized_operations::LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                source: argument,
                ..
            } = &mut observation.kind
            else {
                unreachable!()
            };
            argument.access = access;
            assert!(
                crate::validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "observation must retain the exact carrier declaration, not substitute {access:?}"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // A returned carrier grants field access, not the separate root-window
        // operation reserved for mutable borrowed entry/block parameters.
        let mut root_window = source.clone();
        let AbstractOperation::MoveStructuralField { field, result, .. } =
            &mut root_window.functions[0].operations[2]
        else {
            unreachable!()
        };
        *field = None;
        result.structural_type = root(StructuralAccess::MutableBorrow).structural_type;
        assert!(
            abstract_operations_to_target_operations::lower_to_target_operations(
                &root_window,
                TargetLoweringRequest::new(native),
            )
            .is_err(),
            "a same-typed whole-referent copy cannot turn a field window into a root window"
        );
        for mutation in 0..4 {
            let mut changed = target.clone();
            let store = changed.functions[0].graph.blocks[0]
                .operations
                .iter_mut()
                .find(|row| matches!(row, TargetUnitOperation::StoreStructuralField { .. }))
                .unwrap();
            let TargetUnitOperation::StoreStructuralField {
                destination,
                path,
                field,
                byte_offset,
                ..
            } = store
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    let abstract_operations::StructuralWindowRoot::OwnedResult { producer, .. } =
                        destination
                    else {
                        unreachable!()
                    };
                    *producer = operation(1);
                }
                1 => path.clear(),
                2 => *byte_offset = 0,
                3 => *field = None,
                _ => unreachable!(),
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "mutation {mutation}"
            );
        }
        for operation in [operation(2), operation(3), operation(30)] {
            let mut changed = selected.plan().clone();
            let block = &mut changed.functions[0].blocks[0];
            let pointer_load = block
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Load64 { byte_offset: 0 }
                    ) && instruction.provenance.operations.contains(&operation)
                })
                .expect("each window direction loads the retained carrier's pointer word");
            block.instructions.remove(pointer_load);
            assert!(
                crate::validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed,
                )
                .is_err(),
                "the referent must never be replaced by carrier storage"
            );
        }
    }
}

/// Legalization re-resolves the field extent and the value's producer, so a
/// retained row naming another offset, field, path, root or home rejects.
#[test]
fn window_rows_reject_extent_field_and_home_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64());
    let displaced_home = target.functions[0].graph.blocks[0]
        .operations
        .iter()
        .find_map(|row| match row {
            TargetUnitOperation::StructuralLeafCopy {
                psi_operation,
                result_home,
                ..
            } if *psi_operation == operation(2) => Some(result_home.clone()),
            _ => None,
        })
        .unwrap();
    for mutation in 0..6 {
        let mut changed = target.clone();
        for row in &mut changed.functions[0].graph.blocks[0].operations {
            match (mutation, row) {
                (0, TargetUnitOperation::StoreStructuralField { byte_offset, .. }) => {
                    *byte_offset = 0
                }
                (1, TargetUnitOperation::StoreStructuralField { field, .. }) => {
                    *field = Some(StructuralFieldId::new(1).unwrap())
                }
                (2, TargetUnitOperation::StoreStructuralField { value, .. }) => {
                    *value = displaced_home.place()
                }
                (
                    3,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        path,
                        ..
                    },
                ) if *psi_operation == operation(2) => {
                    *path = vec![StructuralPathSegment::Field("tag".into())].into()
                }
                (
                    4,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        byte_offset,
                        ..
                    },
                ) if *psi_operation == operation(2) => *byte_offset = 0,
                (
                    5,
                    TargetUnitOperation::StructuralLeafCopy {
                        psi_operation,
                        source,
                        ..
                    },
                ) if *psi_operation == operation(2) => *source = place(9),
                _ => {}
            }
        }
        assert_ne!(changed, target, "mutation {mutation} changed a row");
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}

/// Only a mutable-borrowed root opens a window: target lowering refuses the
/// same pair over a shared borrow instead of writing through it.
#[test]
fn window_pair_over_a_shared_root_rejects_in_target_lowering() {
    let source = source(StructuralAccess::SharedBorrow);
    let error = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        TargetLoweringRequest::new(NativeTarget::linux_x64()),
    )
    .expect_err("a shared root opens no window");
    assert!(
        matches!(
            error,
            abstract_operations_to_target_operations::LoweringError::UnsupportedControlFlow { site, .. }
                if site.file().ends_with("control_flow/borrowed_windows.rs")
        ),
        "{error:?}"
    );
}

#[test]
fn owned_parameter_reseats_window_through_exact_abi_transport() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let mut source = source(StructuralAccess::MutableBorrow);
        // The replacement comes from the caller, not from a synthetic leaf
        // copy. Keep the move/store pair and its nonzero destination offset.
        source.functions[0].operations.remove(0);
        source.functions[0]
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place: place(2),
                position: 1,
                is_self: false,
                structural_type: inner(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(native),
        )
        .expect("owned argument can reseat the window");
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let mut changed = target.clone();
        let TargetUnitOperation::StoreStructuralField { value, .. } =
            &mut changed.functions[0].graph.blocks[0].operations[1]
        else {
            unreachable!()
        };
        *value = place(3);
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "the same-typed displaced value cannot replace the declared parameter"
        );
    }
}

#[test]
fn state_loan_window_uses_the_arriving_address_through_selection_replay() {
    use abstract_operations::{AbstractBlockEntry, AbstractStructuralBinding};
    use semantic_vocabulary::{BlockId, EdgeId, StructuralCaseId, ValueId};

    let mut source = source(StructuralAccess::MutableBorrow);
    let status = StructuralTypeId::new(30).unwrap();
    let case = StructuralCaseId::new(1).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: status,
            identity: "test::Status".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![terminal_psi::StructuralCaseDeclaration {
                    id: case,
                    identity: "Ready".into(),
                    fields: Vec::new(),
                }],
            },
        });
    let outer = source
        .structural_types
        .make_mut()
        .iter_mut()
        .find(|declaration| declaration.id == root(StructuralAccess::MutableBorrow).structural_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &mut outer.shape else {
        unreachable!()
    };
    fields.push(field(3, "status", StructuralFieldType::Structural(status)));
    let function = &mut source.functions[0];
    let mut arrival = root(StructuralAccess::MutableBorrow);
    arrival.place = place(4);
    for operation in &mut function.operations {
        match operation {
            AbstractOperation::StructuralLeafCopy { source, .. } => *source = arrival.place,
            AbstractOperation::MoveStructuralField { source, .. } => {
                *source =
                    abstract_operations::StructuralWindowRoot::BorrowedParameter(arrival.clone());
            }
            AbstractOperation::StoreStructuralField { destination, .. } => {
                *destination =
                    abstract_operations::StructuralWindowRoot::BorrowedParameter(arrival.clone());
            }
            _ => {}
        }
    }
    function.operations.insert(
        0,
        AbstractOperation::StructuralCaseMembership {
            psi_operation: operation(9),
            result: abstract_operations::AbstractResult {
                value: ValueId::new(9).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
            source: arrival.place,
            path: vec![StructuralPathSegment::Field("status".into())],
            case,
        },
    );
    let body = BlockId::new(2).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::Jump {
            psi_edge: EdgeId::new(2).unwrap(),
            target: body,
            bindings: Vec::new(),
            structural_bindings: vec![AbstractStructuralBinding {
                parameter: arrival.place,
                argument: StructuralArgument {
                    place: place(1),
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                },
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    function.block_entries.push(AbstractBlockEntry {
        block: body,
        operation_offset: 1,
        parameters: Vec::new(),
        structural_parameters: vec![arrival],
    });
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            TargetLoweringRequest::new(native),
        )
        .expect("state loan can open and restore a field window");
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        let mut changed = target.clone();
        let destination = changed.functions[0].graph.blocks[1]
            .operations
            .iter_mut()
            .find_map(|operation| {
                if let TargetUnitOperation::StoreStructuralField { destination, .. } = operation {
                    Some(destination)
                } else {
                    None
                }
            })
            .unwrap();
        // Equal referent types do not let a stale incoming carrier replace the
        // transferred loan's declaration.
        *destination = abstract_operations::StructuralWindowRoot::BorrowedParameter(root(
            StructuralAccess::MutableBorrow,
        ));
        assert!(legalize_target_operations(&changed, &source, &unit).is_err());
    }
}
