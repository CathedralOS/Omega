//! Array availability follows dominance; unrestricted values carry no disposal debt.

use super::*;
use terminal_psi::SuccessorEdge;

fn jump(edge: u64, target: u64) -> Terminator {
    Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: vec![],
        structural_arguments: vec![],
        trivial_affine_discards: vec![],
        residual_affine_discards: vec![],
    }
}

fn selected_call(dimensions: &[u64]) -> TerminalModule {
    let mut module = array_call(dimensions);
    let caller = &mut module.machines[0];
    caller.parameters.push(boolean_declaration(value_id(90)));
    caller.result = TerminalMachineResult::Unit;
    caller
        .structural_places
        .retain(|place| !matches!(place.kind, StructuralPlaceKind::Result));
    let operations = caller.blocks[0].operations.split_off(1);
    caller.blocks[0].terminator = Terminator::Conditional {
        condition: value_id(90),
        when_true: SuccessorEdge {
            edge: edge_id(10),
            target: block_id(3),
            arguments: vec![],
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
        when_false: SuccessorEdge {
            edge: edge_id(11),
            target: block_id(4),
            arguments: vec![],
            structural_arguments: vec![],
            trivial_affine_discards: vec![],
        },
    };
    for (identity, operations, terminator) in [
        (3, operations, jump(12, 5)),
        (4, vec![], jump(13, 5)),
        (
            5,
            vec![],
            Terminator::ReturnUnit {
                edge: edge_id(14),
                trivial_affine_discards: vec![],
            },
        ),
    ] {
        caller.blocks.push(Block {
            id: block_id(identity),
            parameters: vec![],
            structural_parameters: vec![],
            operations,
            terminator,
        });
    }
    module
}

#[test]
fn branch_local_array_construction_and_call_results_rejoin_without_disposal() {
    for dimensions in [&[2][..], &[2, 0], &[0, 2]] {
        let module = selected_call(dimensions);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("branch-local unrestricted payloads do not change the ownership frontier");
        let mut reordered = module;
        reordered.machines[0].blocks.reverse();
        verify_module(
            &reordered,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("availability depends on dominance, not serialized block order");
    }
}

#[test]
fn branch_local_array_cannot_be_called_after_a_nondominating_join() {
    for dimensions in [&[2][..], &[0, 2]] {
        let mut module = selected_call(dimensions);
        let caller = &mut module.machines[0];
        let call = caller.blocks[1].operations.pop().unwrap();
        caller.blocks[3].operations.push(call);
        assert!(matches!(validate_module(&module),
            Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation { place, .. })
                if place == place_id(1)));
    }
}

#[test]
fn branch_local_array_cannot_be_returned_after_a_nondominating_join() {
    for source in [1, 2] {
        let mut module = selected_call(&[0, 2]);
        let original = array_call(&[0, 2]);
        let caller = &mut module.machines[0];
        caller.result = original.machines[0].result.clone();
        caller.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(10),
            kind: StructuralPlaceKind::Result,
        });
        caller.blocks[3].terminator = Terminator::ReturnStructural {
            edge: edge_id(14),
            source: place_id(source),
            returned_claims: vec![],
            trivial_affine_discards: vec![],
        };
        assert!(
            matches!(validate_module(&module),
            Err(ModuleError::StructuralReturnSourceNotLive { place, .. })
                if place == place_id(source)),
            "branch-local source {source}"
        );
    }
}

#[test]
fn array_defined_before_selection_remains_available_after_join() {
    for dimensions in [&[2][..], &[0, 2]] {
        let mut module = selected_call(dimensions);
        let caller = &mut module.machines[0];
        let call = caller.blocks[1].operations.pop().unwrap();
        let constructor = caller.blocks[1].operations.pop().unwrap();
        caller.blocks[0].operations.push(constructor);
        caller.blocks[3].operations.push(call);
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect("array availability survives both successor paths");
    }
}
