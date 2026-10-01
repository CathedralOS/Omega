//! A field store into an owned block arrival writes that arrival's own home.
//!
//! The entry zero-establishes one record and moves it into block 3's owned
//! parameter; block 3 stores a field into the arrival, reads it back, and
//! returns it. Legalization rejoins the store to the block-owned home, and
//! selection writes through the address `block_entry` gives that home.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractResult, AbstractStructuralBinding, StructuralScalarStoreDestination,
};
use legalized_operations::LegalizedScalarInstructionKind;
use selected_instructions::{SelectedMemoryAccessOrigin, SelectedMemoryAccessRole};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, PlaceId, PsiSemanticId,
    ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetStructuralArgumentSource, TargetUnitOperation};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

use crate::{legalize_target_operations, validate_legalized_operations};

pub(crate) const ENTRY: u64 = 1;
pub(crate) const ARRIVAL: u64 = 3;
pub(crate) const HOME: u64 = 10;
const ARRIVAL_PLACE: u64 = 11;
const STORE: u64 = 22;

pub(crate) fn id<T: PsiSemanticId>(value: u64) -> T {
    T::new(value).unwrap()
}

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

pub(crate) fn arrival() -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(ARRIVAL_PLACE),
        position: 0,
        is_self: false,
        structural_type: id(12),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn source() -> AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let structural_type: StructuralTypeId = id(12);
    source.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::OwnedArrival".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: id(1),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(u64_type()),
                },
                StructuralFieldDeclaration {
                    id: id(2),
                    identity: "bonus".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(u64_type()),
                },
            ],
        },
    }]
    .into();
    let stored = AbstractResult {
        value: id::<ValueId>(13),
        scalar_type: u64_type(),
    };
    let observed = AbstractResult {
        value: id::<ValueId>(14),
        scalar_type: u64_type(),
    };
    let function = &mut source.functions[0];
    function.result = AbstractFunctionResult::Scalar(observed);
    function.block_entries.push(AbstractBlockEntry {
        block: id(ARRIVAL),
        parameters: Vec::new(),
        structural_parameters: vec![arrival()],
        operation_offset: 2,
    });
    function.operations = vec![
        AbstractOperation::EstablishZeroedStructural {
            psi_operation: id(30),
            result: StructuralOperationResult {
                place: id(HOME),
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
        },
        AbstractOperation::Jump {
            psi_edge: id::<EdgeId>(20),
            target: id(ARRIVAL),
            bindings: Vec::new(),
            structural_bindings: vec![AbstractStructuralBinding {
                parameter: id(ARRIVAL_PLACE),
                argument: StructuralArgument {
                    place: id(HOME),
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: id(21),
            result: stored.value,
            scalar_type: stored.scalar_type,
            value: IntegerValue::Unsigned(70),
        },
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: id(STORE),
            destination: StructuralScalarStoreDestination::OwnedBlockParameter {
                block: id(ARRIVAL),
                parameter: arrival(),
            },
            path: Vec::new(),
            field: id::<StructuralFieldId>(2),
            value: stored,
            range_obligation: None,
        },
        AbstractOperation::StructuralScalarFieldRead {
            psi_operation: id(23),
            result: observed,
            source: id(ARRIVAL_PLACE),
            path: Vec::new(),
            field: id(2),
        },
        AbstractOperation::Return {
            psi_edge: id(24),
            result: observed.value,
            value: observed.value,
            scalar_type: observed.scalar_type,
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(id(ARRIVAL_PLACE))],
        },
    ];
    source
}

/// The legalized function whose block 3 stores into its owned arrival.
pub(crate) fn legalized_arrival() -> legalized_operations::LegalizedScalarFunction {
    let source = source();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(
            NativeTarget::macos_arm64(),
        ),
    )
    .unwrap();
    legalize_target_operations(&target, &source, &unit)
        .unwrap()
        .plan()
        .scalar_functions[0]
        .clone()
}

fn target_store(target: &mut target_operations::TargetOperationPlan) -> &mut TargetUnitOperation {
    target.functions[0]
        .graph
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation,
                TargetUnitOperation::StructuralScalarFieldStore { .. }
            )
        })
        .expect("arrival field store")
}

#[test]
fn owned_arrival_field_store_writes_the_block_home_on_every_abi() {
    let source = source();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("a live owned arrival admits a field store");
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let TargetUnitOperation::StructuralScalarFieldStore {
            destination_storage,
            field_byte_offset,
            ..
        } = target_store(&mut target.clone()).clone()
        else {
            unreachable!()
        };
        assert_eq!(
            destination_storage,
            TargetStructuralArgumentSource::BlockParameter {
                block: id(ARRIVAL),
                place: id(ARRIVAL_PLACE),
            }
        );
        assert_eq!(field_byte_offset, 8, "bonus follows value");
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        // The write lands in the arrival's own place, and the arrival's home
        // is the block-owned slot rather than the entry's producer home.
        let function = &selected.plan().functions[0];
        assert!(function.memory_accesses.iter().any(|access| {
            access.origin == SelectedMemoryAccessOrigin::Operation(id(STORE))
                && access.place == id::<PlaceId>(ARRIVAL_PLACE)
                && access.role == SelectedMemoryAccessRole::WritePlace
        }));
        assert!(function.memory_accesses.iter().any(|access| {
            access.origin == SelectedMemoryAccessOrigin::Block(id(ARRIVAL))
                && access.place == id::<PlaceId>(ARRIVAL_PLACE)
                && matches!(access.role, SelectedMemoryAccessRole::AddressLocal { .. })
        }));
    }
}

#[test]
fn owned_arrival_store_rejects_substituted_storage_and_destination() {
    let source = source();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let native = NativeTarget::macos_arm64();
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetUnitOperation::StructuralScalarFieldStore {
            destination,
            destination_storage,
            ..
        } = target_store(&mut changed)
        else {
            unreachable!()
        };
        let StructuralScalarStoreDestination::OwnedBlockParameter { block, parameter } =
            destination
        else {
            panic!("owned arrival destination");
        };
        match mutation {
            // The lender's producer home, not the arrival's own.
            0 => {
                *destination_storage = TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: id(30),
                }
            }
            // The right place under another block's identity.
            1 => {
                *destination_storage = TargetStructuralArgumentSource::BlockParameter {
                    block: id(ENTRY),
                    place: id(ARRIVAL_PLACE),
                }
            }
            2 => *block = id(ENTRY),
            // A substituted place, and a shared declaration of the arrival.
            3 => parameter.place = id(HOME),
            _ => parameter.access = StructuralAccess::SharedBorrow,
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "target mutation {mutation}"
        );
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                .is_err(),
            "replayed target mutation {mutation}"
        );
    }
    for mutation in 0..4 {
        let mut changed = legalized.plan().clone();
        let row = changed.scalar_functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| {
                matches!(
                    row.kind,
                    LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
                )
            })
            .expect("legalized arrival store");
        let LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination,
            byte_offset,
            ..
        } = &mut row.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                *destination = StructuralScalarStoreDestination::OwnedBlockParameter {
                    block: id(ENTRY),
                    parameter: arrival(),
                }
            }
            1 => *destination = StructuralScalarStoreDestination::Parameter(arrival()),
            2 => {
                let mut shared = arrival();
                shared.access = StructuralAccess::SharedBorrow;
                *destination = StructuralScalarStoreDestination::OwnedBlockParameter {
                    block: id(ARRIVAL),
                    parameter: shared,
                }
            }
            _ => *byte_offset = 0,
        }
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "legalized mutation {mutation}"
        );
    }
}
