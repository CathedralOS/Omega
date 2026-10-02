//! Exclusive transfers consume source names even when their descriptor still exists.
use super::{AbstractOperation, id};
use crate::tests::structural_transfers::transfer_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{BlockId, MachineId, PlaceId};
use terminal_psi::StructuralAccess;

fn mutable_transfer_unit() -> PsiOptimizationUnit {
    let mut candidate = transfer_unit();
    let function = &mut candidate.functions[0];
    function.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    function.blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.access = StructuralAccess::MutableBorrow;
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    candidate
}

#[test]
fn transferred_mutable_view_exposes_only_its_current_binding() {
    let mut candidate = mutable_transfer_unit();
    validate_psi_optimization_unit(&candidate).unwrap();
    let AbstractOperation::ByteSequenceLength { source, .. } =
        &mut candidate.functions[0].blocks[1].nodes[0].operation
    else {
        unreachable!()
    };
    *source = id(10, PlaceId::new);
    refresh_node_derivatives(&mut candidate, 0, 1, 0);
    assert_eq!(
        validate_psi_optimization_unit(&candidate),
        Err(
            OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: id(1, MachineId::new),
                block: id(3, BlockId::new),
                node: 0,
                place: id(10, PlaceId::new),
            }
        )
    );
}

const PRODUCED_MUTABLE_VIEW_SOURCE: &str = r#"
        data Counter { value: u64 in Wrapping; }
        data Room { counters: [Counter; 2]; }
        machine update(room: &mut Room) -> u64 {
            let view: &mut [Counter] = room.counters.as_mut_slice();
            transition { _ -> forward(view) }
            state forward(view: &mut [Counter]) -> u64 {
                transition { _ -> finish(view) }
            }
            state finish(view: &mut [Counter]) -> u64 {
                let index: u64 = 0;
                view[index].value = view[index].value + 1;
                view[index].value as u64
            }
        }
    "#;

fn produced_mutable_view_unit() -> PsiOptimizationUnit {
    crate::tests::structural_cases::verified_source_machine_unit(
        PRODUCED_MUTABLE_VIEW_SOURCE,
        "update",
    )
}

#[test]
fn produced_mutable_view_transfers_permission_without_moving_descriptor_custody() {
    let candidate = produced_mutable_view_unit();
    validate_psi_optimization_unit(&candidate)
        .expect("the affine descriptor lends the same permission across successive state names");
}

#[test]
fn produced_mutable_view_transfer_can_attenuate_to_shared() {
    let source = PRODUCED_MUTABLE_VIEW_SOURCE
        .replace(
            "state finish(view: &mut [Counter])",
            "state finish(view: &[Counter])",
        )
        .replace("view[index].value = view[index].value + 1;", "");
    let candidate = crate::tests::structural_cases::verified_source_machine_unit(&source, "update");
    validate_psi_optimization_unit(&candidate)
        .expect("whole shared attenuation keeps backing custody and retires exclusive spelling");
}

#[test]
fn produced_mutable_view_transfer_rejects_stale_produced_name() {
    let mut candidate = produced_mutable_view_unit();
    let (producer, result) = candidate.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishElementView {
                psi_operation,
                result,
                ..
            } => Some((*psi_operation, result.clone())),
            _ => None,
        })
        .unwrap();
    let stale = result.place;
    let (block, node) = candidate.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, row)| {
            row.nodes
                .iter()
                .position(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::StructuralScalarFieldStore { .. }
                    )
                })
                .map(|node| (block, node))
        })
        .unwrap();
    let AbstractOperation::StructuralScalarFieldStore { destination, .. } =
        &mut candidate.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    *destination =
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result };
    refresh_node_derivatives(&mut candidate, 0, block, node);
    assert!(
        matches!(validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { place, .. }) if place == stale),
        "affine descriptor cleanup does not keep its transferred mutable name usable"
    );
}

#[test]
fn produced_mutable_view_transfer_rejects_forged_source_authority() {
    let mut candidate = produced_mutable_view_unit();
    let (block, node) = candidate.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, row)| {
            row.nodes
                .iter()
                .position(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::EstablishElementView { .. }
                    )
                })
                .map(|node| (block, node))
        })
        .unwrap();
    let AbstractOperation::EstablishElementView { source, .. } =
        &mut candidate.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    source.access = StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut candidate, 0, block, node);
    assert!(
        validate_psi_optimization_unit(&candidate).is_err(),
        "an affine result does not promote shared establishment to mutable authority"
    );
}

#[test]
fn produced_mutable_view_transfer_rejects_stale_arrival_use() {
    let mut candidate = produced_mutable_view_unit();
    let arrival = candidate.functions[0]
        .blocks
        .iter()
        .filter(|block| !block.structural_parameters.is_empty())
        .find(|block| {
            block
                .nodes
                .iter()
                .any(|node| matches!(node.operation, AbstractOperation::Jump { .. }))
        })
        .unwrap()
        .structural_parameters[0]
        .place;
    let (block, node) = candidate.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, row)| {
            row.nodes
                .iter()
                .position(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::StructuralScalarFieldStore { .. }
                    )
                })
                .map(|node| (block, node))
        })
        .unwrap();
    let declaration = candidate.functions[0]
        .arrival_parameter(arrival)
        .unwrap()
        .clone();
    let AbstractOperation::StructuralScalarFieldStore { destination, .. } =
        &mut candidate.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    *destination = abstract_operations::StructuralScalarStoreDestination::Parameter(declaration);
    refresh_node_derivatives(&mut candidate, 0, block, node);
    assert!(
        matches!(validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { place, .. }) if place == arrival),
        "the old descriptor name cannot write after its exclusive handoff"
    );
}

#[test]
fn produced_mutable_view_projected_loan_preserves_parent_name() {
    let unit = crate::tests::structural_cases::verified_source_machine_unit(
        r#"
        data Counter { value: u64; }
        machine update(view: &mut [Counter]) -> u64 {
            transition view.len > 0 {
                true -> finish(&mut view[0])
                _ -> (0)
            }
            state finish(counter: &mut Counter) -> u64 { counter.value }
        }
    "#,
        "update",
    );
    crate::validate_psi_optimization_unit(&unit).unwrap();
    let function = &unit.functions[0];
    let types = unit
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    let available =
        crate::unit_validation::function_structure::current_mutable_borrows(function, &types);
    let (target, parent) = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .find_map(|edge| {
            edge.structural_bindings
                .iter()
                .find(|binding| !binding.argument.path.is_empty())
                .map(|binding| (edge.target, binding.argument.place))
        })
        .expect("the successor receives a projected record loan");
    assert!(
        available.may_be_live(target, parent),
        "lending one element does not transfer the parent descriptor's whole name"
    );
}

fn mutable_view_call_arguments(
    access: [StructuralAccess; 2],
    elements: [u64; 2],
) -> PsiOptimizationUnit {
    use terminal_psi::{
        StructuralArgument, StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
    };
    let mut candidate =
        crate::tests::fixtures::scalar_units::projected_shared_structural_scalar_call_unit();
    let element = candidate.structural_types[1].id;
    candidate.structural_types.make_mut()[0].shape = StructuralTypeShape::ElementView { element };
    candidate.functions[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    let root = candidate.functions[0].structural_parameters[0].place;
    let callee = &mut candidate.functions[1];
    callee.structural_parameters[0].access = access[0];
    let mut second = callee.structural_parameters[0].clone();
    second.place = PlaceId::new(900).unwrap();
    second.position = 1;
    second.access = access[1];
    callee.declared_places.insert(second.place);
    callee.structural_places.push(StructuralPlaceDeclaration {
        id: second.place,
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    callee.structural_parameters.push(second);
    let AbstractOperation::CallStructural {
        structural_arguments,
        ..
    } = &mut candidate.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *structural_arguments = access
        .into_iter()
        .zip(elements)
        .map(|(access, element)| StructuralArgument {
            place: root,
            path: vec![StructuralPathSegment::FixedIndex(element)],
            access,
        })
        .collect();
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    candidate
}

#[test]
fn mutable_view_call_allows_shared_repeats_and_disjoint_subloans() {
    for (access, elements) in [
        ([StructuralAccess::SharedBorrow; 2], [0, 0]),
        ([StructuralAccess::MutableBorrow; 2], [0, 1]),
        (
            [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ],
            [0, 1],
        ),
    ] {
        validate_psi_optimization_unit(&mutable_view_call_arguments(access, elements))
            .expect("call argument contracts preserve shared and spatially disjoint authority");
    }
}

#[test]
fn mutable_view_call_rejects_overlapping_exclusive_subloans() {
    for access in [
        [StructuralAccess::MutableBorrow; 2],
        [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ],
    ] {
        assert!(
            matches!(
                validate_psi_optimization_unit(&mutable_view_call_arguments(access, [0, 0])),
                Err(OptimizationUnitValidationError::StructuralCallContractMismatch { .. })
            ),
            "same-element overlap cannot supply an exclusive call argument"
        );
    }
}

fn mutable_view_edge_arguments(
    access: [StructuralAccess; 2],
    elements: [u64; 2],
) -> PsiOptimizationUnit {
    use terminal_psi::{
        StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape,
    };
    let mut candidate = mutable_transfer_unit();
    let element = semantic_vocabulary::StructuralTypeId::new(900).unwrap();
    candidate.structural_types.make_mut()[0].shape = StructuralTypeShape::ElementView { element };
    candidate
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: element,
            identity: "test::ProjectedCounter".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        });
    let function = &mut candidate.functions[0];
    let arrival = &mut function.blocks[1];
    arrival.structural_parameters[0].structural_type = element;
    arrival.structural_parameters[0].access = access[0];
    let mut second = arrival.structural_parameters[0].clone();
    second.place = PlaceId::new(901).unwrap();
    second.position = 1;
    second.access = access[1];
    function.declared_places.insert(second.place);
    function.structural_places.push(StructuralPlaceDeclaration {
        id: second.place,
        kind: semantic_vocabulary::StructuralPlaceKind::BlockParameter {
            block: arrival.id,
            position: 1,
        },
    });
    arrival.structural_parameters.push(second.clone());
    let AbstractOperation::ByteSequenceLength {
        psi_operation,
        result,
        ..
    } = arrival.nodes[0].operation
    else {
        unreachable!()
    };
    arrival.nodes[0].operation = AbstractOperation::IntegerConstant {
        psi_operation,
        result: result.value,
        scalar_type: result.scalar_type,
        value: semantic_vocabulary::IntegerValue::Unsigned(0),
    };
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut function.blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_bindings[0].argument.access = access[0];
    structural_bindings[0].argument.path = vec![StructuralPathSegment::FixedIndex(elements[0])];
    let mut second_binding = structural_bindings[0].clone();
    second_binding.parameter = second.place;
    second_binding.argument.access = access[1];
    second_binding.argument.path = vec![StructuralPathSegment::FixedIndex(elements[1])];
    structural_bindings.push(second_binding);
    refresh_node_derivatives(&mut candidate, 0, 0, 0);
    refresh_node_derivatives(&mut candidate, 0, 1, 0);
    candidate
}

#[test]
fn mutable_view_edge_allows_shared_repeats_and_disjoint_shared_mutable_loans() {
    for (access, elements) in [
        ([StructuralAccess::SharedBorrow; 2], [0, 0]),
        (
            [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ],
            [0, 1],
        ),
    ] {
        validate_psi_optimization_unit(&mutable_view_edge_arguments(access, elements))
            .expect("edge argument rules agree with Terminal shared and projected loans");
    }
}

#[test]
fn mutable_view_edge_preserves_single_exclusive_arrival_per_root() {
    for (access, elements) in [
        ([StructuralAccess::MutableBorrow; 2], [0, 1]),
        (
            [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ],
            [0, 0],
        ),
    ] {
        assert!(
            matches!(
                validate_psi_optimization_unit(&mutable_view_edge_arguments(access, elements)),
                Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
            ),
            "Terminal forbids duplicate mutable roots and shared/exclusive overlap on an edge"
        );
    }
}
