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

fn guarded_mixed_call() -> TerminalModule {
    let mut module = selected_call(&[2]);
    let callee = &mut module.machines[1];
    callee.parameters.push(boolean_declaration(value_id(91)));
    callee.contract.requires = vec![Proposition::Equal(
        boolean_value(91),
        ScalarTerm::boolean(true),
    )];
    let call = &mut module.machines[0].blocks[1].operations[1];
    let OperationKind::CallStructural {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        crash_continuations,
        ..
    } = call.kind.clone()
    else {
        panic!("array-returning call")
    };
    call.kind = OperationKind::CallStructuralWithScalarArguments {
        callee,
        arguments: vec![value_id(90)],
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations: vec![obligation_id(1)],
        crash_continuations,
    };
    module
}

fn guarded_call_evidence(module: &TerminalModule, argument: u64) -> ProofBundle {
    let reconstructed = reconstruct_terminal_obligations(module).expect("formed guarded call");
    let [site] = reconstructed.obligations() else {
        panic!("one callee requirement")
    };
    assert!(
        site.requirements.is_empty(),
        "the caller has no entry assumption"
    );
    assert_eq!(
        site.obligation.proposition,
        Proposition::Equal(boolean_value(argument), ScalarTerm::boolean(true)),
        "the callee formal substitutes the caller's branch condition"
    );
    let premise = site
        .semantic_axioms
        .iter()
        .position(|axiom| axiom == &site.obligation.proposition)
        .expect("selected true edge establishes the mixed call requirement");
    ProofBundle {
        evidence: vec![semantic_axiom_evidence(
            obligation_id(1),
            site.obligation.proposition.clone(),
            premise,
            1,
        )],
        ..ProofBundle::default()
    }
}

#[test]
fn mixed_structural_call_replays_its_selected_branch_requirement() {
    for forwarded in [false, true] {
        let mut module = guarded_mixed_call();
        if forwarded {
            let caller = &mut module.machines[0];
            let Terminator::Conditional { when_true, .. } = &mut caller.blocks[0].terminator else {
                panic!("selected call")
            };
            when_true.arguments.push(value_id(90));
            caller.blocks[1]
                .parameters
                .push(boolean_declaration(value_id(92)));
            let OperationKind::CallStructuralWithScalarArguments { arguments, .. } =
                &mut caller.blocks[1].operations[1].kind
            else {
                panic!("mixed call")
            };
            arguments[0] = value_id(92);
        }
        for reverse_blocks in [false, true] {
            if reverse_blocks {
                module.machines[0].blocks.reverse();
            }
            let evidence = guarded_call_evidence(&module, if forwarded { 92 } else { 90 });
            let verified = verify_module(&module, &evidence, &AdmissionProfile::default())
                .expect("mixed structural call uses the independently reconstructed selected edge");
            assert_eq!(verified.accepted_facts().len(), 1);
        }
    }
}

#[test]
fn mixed_structural_call_cannot_replay_a_fact_from_the_other_arm_or_before_a_join() {
    let original = guarded_mixed_call();
    let evidence = guarded_call_evidence(&original, 90);
    for after_join in [false, true] {
        let mut module = original.clone();
        let caller = &mut module.machines[0];
        if after_join {
            let call = caller.blocks[1].operations.pop().unwrap();
            let constructor = caller.blocks[1].operations.pop().unwrap();
            caller.blocks[0].operations.push(constructor);
            caller.blocks[3].operations.push(call);
        } else {
            let Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut caller.blocks[0].terminator
            else {
                panic!("selected call")
            };
            std::mem::swap(&mut when_true.target, &mut when_false.target);
        }
        validate_module(&module).expect("the negative control is structurally well formed");
        let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
        let [site] = reconstructed.obligations() else {
            panic!("one callee requirement")
        };
        assert!(!site.semantic_axioms.contains(&site.obligation.proposition));
        assert!(
            verify_module(&module, &evidence, &AdmissionProfile::default()).is_err(),
            "selected-edge evidence must not apply after_join={after_join}"
        );
    }
}
