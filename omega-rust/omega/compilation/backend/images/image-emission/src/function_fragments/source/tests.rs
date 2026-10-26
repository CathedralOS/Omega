use super::{AbstractOperation, ieee_literal_retained, requires_graph_storage_replay};
use semantic_vocabulary::{IeeeFloatValue, OperationId, ValueId};
use target_operations::TargetUnitOperation;

#[test]
fn atomic_storage_requires_retained_graph_replay() {
    use abstract_operations::atomic::AbstractAtomicLocation;
    use abstract_operations::{AbstractAtomicEvent, AtomicModificationAfter};
    use semantic_vocabulary::{PlaceId, StructuralFieldId};

    let operation = AbstractOperation::AtomicEvent {
        psi_operation: OperationId::new(1).unwrap(),
        event: AbstractAtomicEvent::Store {
            location: AbstractAtomicLocation {
                root: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
            },
            ordering: terminal_psi::MemoryOrdering::NoOrdering,
            value: ValueId::new(1).unwrap(),
        },
        reads_from: None,
        modification_after: Some(AtomicModificationAfter::InitialResidency),
        resident_obligation: None,
    };
    assert!(requires_graph_storage_replay(&[operation]));
}

#[test]
fn replay_requirement_is_specific_to_primitive_storage_operations() {
    use abstract_operations::AbstractResult;
    use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
        StructuralParameterDeclaration,
    };

    let operation = OperationId::new(1).unwrap();
    let scalar = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    };
    let result = StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        claims: vec![],
    };
    let existing = [AbstractOperation::BooleanConstant {
        psi_operation: operation,
        result: scalar.value,
        value: true,
    }];
    assert!(!requires_graph_storage_replay(&[]));
    assert!(!requires_graph_storage_replay(&existing));
    for operation in [
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation: operation,
            path: Vec::new(),
            destination: abstract_operations::StructuralScalarStoreDestination::Parameter(
                StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    structural_type: result.structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                },
            ),
            value: scalar,
            range_obligation: None,
        },
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation: operation,
            result: result.clone(),
            value: scalar,
            range_obligation: None,
        },
        AbstractOperation::PrimitiveLocalStore {
            psi_operation: operation,
            destination: result.place,
            value: scalar,
        },
        AbstractOperation::PrimitiveScalarRead {
            psi_operation: operation,
            path: Vec::new(),
            result: scalar,
            source: result.place,
        },
    ] {
        assert!(requires_graph_storage_replay(std::slice::from_ref(
            &operation
        )));
        let mut mixed = existing.to_vec();
        mixed.push(operation);
        assert!(requires_graph_storage_replay(&mixed));
    }
}

#[test]
fn ieee_literal_publication_requires_unique_operation_value_type_and_bits() {
    let operation = OperationId::new(1).unwrap();
    let result = ValueId::new(1).unwrap();
    for value in [
        IeeeFloatValue::Binary32(0x7fc0_0041),
        IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
    ] {
        let source = AbstractOperation::IeeeFloatConstant {
            psi_operation: operation,
            result,
            value,
        };
        let target = TargetUnitOperation::IeeeFloatConstant {
            psi_operation: operation,
            result,
            value,
        };
        assert!(ieee_literal_retained(
            &source,
            std::slice::from_ref(&target)
        ));
        assert!(!ieee_literal_retained(&source, &[]));
        assert!(!ieee_literal_retained(
            &source,
            &[target.clone(), target.clone()]
        ));
        for mutation in 0..4 {
            let mut changed = target.clone();
            let TargetUnitOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            } = &mut changed
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_operation = OperationId::new(2).unwrap(),
                1 => *result = ValueId::new(2).unwrap(),
                2 => {
                    *value = match *value {
                        IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary32(bits ^ 1),
                        IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary64(bits ^ 1),
                    }
                }
                _ => {
                    *value = match *value {
                        IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary64(u64::from(bits)),
                        IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary32(bits as u32),
                    }
                }
            }
            assert!(
                !ieee_literal_retained(&source, &[changed]),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn concatenation_publication_retains_ordered_sources_result_and_bound() {
    use semantic_vocabulary::{ObligationId, PlaceId, StructuralTypeId};
    use terminal_psi::control_flow::ByteSequenceOperand;
    let operation = OperationId::new(1).unwrap();
    let result = terminal_psi::StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(3).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let left = ByteSequenceOperand::Whole {
        source: terminal_psi::StructuralArgument::whole(
            PlaceId::new(1).unwrap(),
            terminal_psi::StructuralAccess::SharedBorrow,
        ),
        length: ValueId::new(1).unwrap(),
    };
    let right = ByteSequenceOperand::Whole {
        source: terminal_psi::StructuralArgument::whole(
            PlaceId::new(2).unwrap(),
            terminal_psi::StructuralAccess::SharedBorrow,
        ),
        length: ValueId::new(2).unwrap(),
    };
    let obligation = ObligationId::new(1).unwrap();
    let abstracted = AbstractOperation::EstablishByteSequenceConcatenation {
        psi_operation: operation,
        result: result.clone(),
        left: left.clone(),
        right: right.clone(),
        capacity_obligation: obligation,
    };
    let target = TargetUnitOperation::EstablishByteSequenceConcatenation {
        psi_operation: operation,
        result,
        left,
        right,
        capacity_obligation: obligation,
    };
    assert!(requires_graph_storage_replay(std::slice::from_ref(
        &abstracted
    )));
    assert!(super::byte_concatenation_retained(&abstracted, [&target]));
    assert!(!super::byte_concatenation_retained(
        &abstracted,
        [&target, &target]
    ));
    let mut changed = target.clone();
    if let TargetUnitOperation::EstablishByteSequenceConcatenation { left, right, .. } =
        &mut changed
    {
        std::mem::swap(left, right);
    }
    assert!(!super::byte_concatenation_retained(&abstracted, [&changed]));
    let mut changed = target.clone();
    if let TargetUnitOperation::EstablishByteSequenceConcatenation {
        capacity_obligation,
        ..
    } = &mut changed
    {
        *capacity_obligation = ObligationId::new(2).unwrap();
    }
    assert!(!super::byte_concatenation_retained(&abstracted, [&changed]));
    let mut changed = target;
    if let TargetUnitOperation::EstablishByteSequenceConcatenation { result, .. } = &mut changed {
        result.place = PlaceId::new(4).unwrap();
    }
    assert!(!super::byte_concatenation_retained(&abstracted, [&changed]));
}

mod temporary_stack;
