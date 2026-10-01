//! A field store into an owned block arrival writes that arrival's own home.
//!
//! The entry block zero-establishes one record and moves it into block 3's
//! owned parameter; block 3 stores a field into the arrival and reads it back.
//! With `forward`, block 3 instead moves the arrival on into block 4's owned
//! parameter, and block 4 holds the store and the read.
use super::id;
use crate::tests::fixtures::scalar_units::unit;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractResult, AbstractStructuralBinding,
    StructuralScalarStoreDestination,
};
use optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

const ENTRY: u64 = 2;
const ARRIVAL: u64 = 3;
const FORWARDED: u64 = 4;
const HOME: u64 = 10;
const ARRIVAL_PLACE: u64 = 11;
const FORWARDED_PLACE: u64 = 15;

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn arrival(place: u64) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place, PlaceId::new),
        position: 0,
        is_self: false,
        structural_type: id(12, StructuralTypeId::new),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn binding(parameter: u64, argument: u64) -> AbstractStructuralBinding {
    AbstractStructuralBinding {
        parameter: id(parameter, PlaceId::new),
        argument: StructuralArgument {
            place: id(argument, PlaceId::new),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    }
}

fn jump(
    edge: u64,
    target: u64,
    structural_bindings: Vec<AbstractStructuralBinding>,
) -> AbstractOperation {
    AbstractOperation::Jump {
        psi_edge: id(edge, EdgeId::new),
        target: id(target, BlockId::new),
        bindings: Vec::new(),
        structural_bindings,
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

/// Store into `destination`, read the field back from `read_place`, and
/// return it after discarding `read_place`.
fn store_and_read(
    destination: StructuralScalarStoreDestination,
    read_place: u64,
) -> Vec<AbstractOperation> {
    let stored = AbstractResult {
        value: id(13, ValueId::new),
        scalar_type: u64_type(),
    };
    let observed = AbstractResult {
        value: id(14, ValueId::new),
        scalar_type: u64_type(),
    };
    vec![
        AbstractOperation::IntegerConstant {
            psi_operation: id(21, OperationId::new),
            result: stored.value,
            scalar_type: stored.scalar_type,
            value: IntegerValue::Unsigned(70),
        },
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation: id(22, OperationId::new),
            destination,
            path: Vec::new(),
            field: id(1, StructuralFieldId::new),
            value: stored,
            range_obligation: None,
        },
        AbstractOperation::StructuralScalarFieldRead {
            psi_operation: id(23, OperationId::new),
            result: observed,
            source: id(read_place, PlaceId::new),
            path: Vec::new(),
            field: id(1, StructuralFieldId::new),
        },
        AbstractOperation::Return {
            psi_edge: id(24, EdgeId::new),
            result: observed.value,
            value: observed.value,
            scalar_type: observed.scalar_type,
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(id(
                read_place,
                PlaceId::new,
            ))],
        },
    ]
}

fn owned_block_store_plan(
    forward: bool,
    destination: StructuralScalarStoreDestination,
) -> AbstractOperationPlan {
    let structural_type = id(12, StructuralTypeId::new);
    let mut block_entries = vec![
        AbstractBlockEntry {
            block: id(ENTRY, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            block: id(ARRIVAL, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: vec![arrival(ARRIVAL_PLACE)],
            operation_offset: 2,
        },
    ];
    let mut operations = vec![
        AbstractOperation::EstablishZeroedStructural {
            psi_operation: id(30, OperationId::new),
            result: StructuralOperationResult {
                place: id(HOME, PlaceId::new),
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
        },
        jump(20, ARRIVAL, vec![binding(ARRIVAL_PLACE, HOME)]),
    ];
    if forward {
        operations.push(jump(
            25,
            FORWARDED,
            vec![binding(FORWARDED_PLACE, ARRIVAL_PLACE)],
        ));
        block_entries.push(AbstractBlockEntry {
            block: id(FORWARDED, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: vec![arrival(FORWARDED_PLACE)],
            operation_offset: 3,
        });
        operations.extend(store_and_read(destination, FORWARDED_PLACE));
    } else {
        operations.extend(store_and_read(destination, ARRIVAL_PLACE));
    }
    AbstractOperationPlan {
        psi: unit().psi,
        entry: id(1, MachineId::new),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::owned-arrival".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1, StructuralFieldId::new),
                    identity: "total".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(u64_type()),
                }],
            },
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: id(1, MachineId::new),
            attachment: None,
            entry: id(ENTRY, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: id(14, ValueId::new),
                scalar_type: u64_type(),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries,
            operations,
        }],
    }
}

fn owned_block_store_unit(
    forward: bool,
    destination: StructuralScalarStoreDestination,
) -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(
        &owned_block_store_plan(forward, destination),
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

fn arrival_destination(block: u64, place: u64) -> StructuralScalarStoreDestination {
    StructuralScalarStoreDestination::OwnedBlockParameter {
        block: id(block, BlockId::new),
        parameter: arrival(place),
    }
}

#[test]
fn owned_arrival_field_store_writes_its_own_block_home() {
    validate_psi_optimization_unit(&owned_block_store_unit(
        false,
        arrival_destination(ARRIVAL, ARRIVAL_PLACE),
    ))
    .expect("a live owned arrival permits scalar replacement");
    validate_psi_optimization_unit(&owned_block_store_unit(
        true,
        arrival_destination(FORWARDED, FORWARDED_PLACE),
    ))
    .expect("a forwarded arrival is the live owner in its own block");
}

#[test]
fn owned_arrival_store_rejects_substituted_or_borrowed_destinations() {
    let mut substitutions = Vec::new();
    // Another block's identity, or the entry, for the same declaration.
    substitutions.push(arrival_destination(ENTRY, ARRIVAL_PLACE));
    substitutions.push(arrival_destination(FORWARDED, ARRIVAL_PLACE));
    // The predecessor's producer home renamed as this block's arrival.
    substitutions.push(arrival_destination(ARRIVAL, HOME));
    // A declaration that is not the block's own: shared, linear, unrestricted
    // (copied), qualified, or at another position.
    for mutation in 0..5 {
        let mut parameter = arrival(ARRIVAL_PLACE);
        match mutation {
            0 => parameter.access = StructuralAccess::SharedBorrow,
            1 => parameter.multiplicity = StructuralMultiplicity::Linear,
            2 => parameter.multiplicity = StructuralMultiplicity::Unrestricted,
            3 => {
                parameter.qualifications =
                    vec![id(40, semantic_vocabulary::StructuralDomainId::new)]
            }
            _ => parameter.position = 1,
        }
        substitutions.push(StructuralScalarStoreDestination::OwnedBlockParameter {
            block: id(ARRIVAL, BlockId::new),
            parameter,
        });
    }
    // The block's exact declaration retagged as a loan.
    substitutions.push(StructuralScalarStoreDestination::Parameter(arrival(
        ARRIVAL_PLACE,
    )));
    for (index, destination) in substitutions.into_iter().enumerate() {
        assert!(
            validate_psi_optimization_unit(&owned_block_store_unit(false, destination)).is_err(),
            "substitution {index}"
        );
    }
}

#[test]
fn owned_arrival_store_after_forwarding_is_not_live() {
    // The exact catalog row of block 3's arrival is still a valid destination
    // shape; only the ownership frontier knows block 3 already moved it on.
    assert!(matches!(
        validate_psi_optimization_unit(&owned_block_store_unit(
            true,
            arrival_destination(ARRIVAL, ARRIVAL_PLACE),
        )),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place, .. })
            if place == id(ARRIVAL_PLACE, PlaceId::new)
    ));
}

/// The arrival is an owned two-element `u64` array, and the store is a
/// primitive element store at index 1; the read becomes a constant. An owned
/// result destination stays in its producer's block, before the value moves.
fn primitive_store_unit(destination: StructuralScalarStoreDestination) -> PsiOptimizationUnit {
    let into_result = matches!(
        destination,
        StructuralScalarStoreDestination::OwnedResult { .. }
    );
    let mut plan = owned_block_store_plan(false, destination);
    let element = id(9001, StructuralTypeId::new);
    let types = plan.structural_types.make_mut();
    types[0].shape = StructuralTypeShape::FixedArray { element, length: 2 };
    types.push(StructuralTypeDeclaration {
        id: element,
        identity: "test::owned-arrival-element".into(),
        shape: StructuralTypeShape::PrimitiveScalar(u64_type()),
    });
    let function = &mut plan.functions[0];
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        value,
        ..
    } = function.operations[3].clone()
    else {
        unreachable!()
    };
    function.operations[4] = AbstractOperation::IntegerConstant {
        psi_operation: id(23, OperationId::new),
        result: id(14, ValueId::new),
        scalar_type: u64_type(),
        value: IntegerValue::Unsigned(70),
    };
    let store = AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        path: vec![terminal_psi::StructuralPathSegment::FixedIndex(1)],
        value,
    };
    if into_result {
        // Move the stored constant and the store ahead of the jump.
        let constant = function.operations.remove(2);
        function.operations.remove(2);
        function.operations.insert(1, constant);
        function.operations.insert(2, store);
        function.block_entries[1].operation_offset += 2;
    } else {
        function.operations[3] = store;
    }
    reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

#[test]
fn owned_arrival_is_not_a_primitive_store_destination() {
    // Control: the same element store into the predecessor's owned result.
    validate_psi_optimization_unit(&primitive_store_unit(
        StructuralScalarStoreDestination::OwnedResult {
            producer: id(30, OperationId::new),
            result: StructuralOperationResult {
                place: id(HOME, PlaceId::new),
                structural_type: id(12, StructuralTypeId::new),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
        },
    ))
    .expect("an owned array result admits an element store");
    assert!(matches!(
        validate_psi_optimization_unit(&primitive_store_unit(arrival_destination(
            ARRIVAL,
            ARRIVAL_PLACE
        ))),
        Err(OptimizationUnitValidationError::InvalidWriteOnlyPrimitiveStore { .. })
    ));
}
