//! Current aggregate constructors and returns retain exact shared backing slots.
use super::verified_source_machine_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::validate_psi_optimization_unit;
use abstract_operations::AbstractOperation as Operation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralTypeShape,
};

fn returned_case() -> PsiOptimizationUnit {
    verified_source_machine_unit(
        r#"
        data Remainder { bytes: &[u8]; }
        data Verdict { case Empty; case Sound(value: Remainder); }
        machine decode(bytes: &[u8]) -> Verdict {
            transition bytes.len > 0 {
                true -> Verdict::Sound { value: Remainder { bytes: bytes[0..bytes.len] } }
                false -> Verdict::Empty
            }
        }
        "#,
        "decode",
    )
}

#[test]
fn returned_shared_byte_aggregate_replays_current_case_slots() {
    let mut unit = returned_case();
    validate_psi_optimization_unit(&unit).expect("both declared cases have exact backing custody");
    let function = &unit.functions[0];
    assert!(
        crate::descriptor_results::owned_aggregate_descriptor_returns(
            function,
            unit.functions.iter(),
            &unit.structural_types,
        )
    );

    // Removing a live descriptor binding leaves an unknown slot. The inactive
    // Empty alternative cannot supply provenance for the Sound alternative.
    let (block, node) = unit.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.nodes.iter().enumerate().find_map(|(node, operation)| {
                matches!(operation.operation, Operation::EstablishRecord { .. })
                    .then_some((block, node))
            })
        })
        .expect("the active payload constructs its remainder");
    let Operation::EstablishRecord { fields, .. } =
        &mut unit.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    fields.clear();
    refresh_node_derivatives(&mut unit, 0, block, node);
    assert!(
        !crate::descriptor_results::owned_aggregate_descriptor_returns(
            &unit.functions[0],
            unit.functions.iter(),
            &unit.structural_types,
        )
    );
    assert!(validate_psi_optimization_unit(&unit).is_err());
}

#[test]
fn returned_shared_byte_aggregate_rejects_implicit_or_mutable_slot_access() {
    let baseline = returned_case();
    for access in [None, Some(StructuralAccess::MutableBorrow)] {
        let mut unit = baseline.clone();
        let mut changed = 0;
        for declaration in unit.structural_types.make_mut() {
            if let StructuralTypeShape::Record { fields } = &mut declaration.shape {
                for field in fields {
                    if let StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: retained,
                    }) = &mut field.field_type
                    {
                        *retained = access;
                        changed += 1;
                    }
                }
            }
        }
        assert_eq!(changed, 1, "mutate the declared remainder slot itself");
        assert!(validate_psi_optimization_unit(&unit).is_err());
    }
}

#[test]
fn returned_shared_byte_aggregate_requires_its_owned_carrier() {
    let mut unit = returned_case();
    validate_psi_optimization_unit(&unit).expect("the original case returns own their carriers");
    let function = &mut unit.functions[0];
    let result_type = function.result.structural().unwrap().structural_type;
    let borrowed = PlaceId::new(10_000).unwrap();
    assert!(!function.declared_places.contains(&borrowed));
    let mut parameter = function.structural_parameters[0].clone();
    parameter.place = borrowed;
    parameter.position = function.structural_parameters.len() as u32;
    parameter.structural_type = result_type;
    parameter.access = StructuralAccess::SharedBorrow;
    let position = parameter.position;
    function.structural_parameters.push(parameter);
    function.declared_places.insert(borrowed);
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: borrowed,
            kind: StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
    let (block, node) = unit.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.nodes.iter().enumerate().find_map(|(node, operation)| {
                matches!(operation.operation, Operation::ReturnStructural { .. })
                    .then_some((block, node))
            })
        })
        .unwrap();
    let Operation::ReturnStructural { source, .. } =
        &mut unit.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    *source = borrowed;
    refresh_node_derivatives(&mut unit, 0, block, node);
    // Its slots really have external backing and the return type is unchanged.
    // The refusal must retain the separate requirement to own the carrier.
    let graph = crate::descriptor_results::optimized_graph(&unit)
        .expect("the counterfeit retains an exact current descriptor graph");
    let function = &unit.functions[0];
    let machine = graph
        .iter()
        .find(|machine| machine.id == function.machine)
        .unwrap();
    let (position, location) = machine
        .returns
        .as_ref()
        .unwrap()
        .iter()
        .find(|(_, location)| location.root == borrowed)
        .unwrap();
    assert!(
        terminal_semantics::descriptor_results::reconstruct_return_slots_external(
            &unit.structural_types,
            &graph,
            function.machine,
            *position,
            location.root,
        )
    );
    assert!(
        !crate::descriptor_results::owned_aggregate_descriptor_returns(
            &unit.functions[0],
            unit.functions.iter(),
            &unit.structural_types,
        )
    );
    assert!(validate_psi_optimization_unit(&unit).is_err());
}
