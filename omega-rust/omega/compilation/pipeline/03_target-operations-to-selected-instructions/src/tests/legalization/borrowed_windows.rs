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
                field: StructuralFieldId::new(2).unwrap(),
            },
            AbstractOperation::StoreStructuralField {
                psi_operation: operation(3),
                destination: abstract_operations::StructuralWindowRoot::BorrowedParameter(root(
                    access,
                )),
                path: Vec::new(),
                field: StructuralFieldId::new(2).unwrap(),
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
                    *field = StructuralFieldId::new(1).unwrap()
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
    use semantic_vocabulary::{BlockId, EdgeId};

    let mut source = source(StructuralAccess::MutableBorrow);
    let function = &mut source.functions[0];
    function.operations.remove(0);
    let mut arrival = root(StructuralAccess::MutableBorrow);
    arrival.place = place(4);
    for operation in &mut function.operations {
        match operation {
            AbstractOperation::MoveStructuralField { source, .. } => {
                *source =
                    abstract_operations::StructuralWindowRoot::BorrowedParameter(arrival.clone());
            }
            AbstractOperation::StoreStructuralField {
                destination, value, ..
            } => {
                *destination =
                    abstract_operations::StructuralWindowRoot::BorrowedParameter(arrival.clone());
                value.place = place(3);
            }
            _ => {}
        }
    }
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
        let TargetUnitOperation::StoreStructuralField { destination, .. } =
            &mut changed.functions[0].graph.blocks[1].operations[1]
        else {
            unreachable!()
        };
        // Equal referent types do not let a stale incoming carrier replace the
        // transferred loan's declaration.
        *destination = abstract_operations::StructuralWindowRoot::BorrowedParameter(root(
            StructuralAccess::MutableBorrow,
        ));
        assert!(legalize_target_operations(&changed, &source, &unit).is_err());
    }
}
