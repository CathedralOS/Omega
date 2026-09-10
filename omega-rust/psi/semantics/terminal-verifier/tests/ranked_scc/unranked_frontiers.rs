use super::*;
use terminal_verifier::reconstruct_structural_ownership_frontiers;

#[test]
fn unranked_cycle_reconstructs_every_frontier() {
    let module = unranked_effectful_unit_cycle();
    require_complete_frontiers(&module);
}

fn require_complete_frontiers(module: &TerminalModule) {
    let frontiers = reconstruct_structural_ownership_frontiers(module).unwrap();
    let machine = &module.machines[0];
    let reconstructed = frontiers.machine(machine.id).unwrap();
    for block in &machine.blocks {
        assert!(
            reconstructed.block_entry(block.id).is_some(),
            "missing block {:?}",
            block.id
        );
        for operation in &block.operations {
            assert!(reconstructed.operation_entry(operation.id).is_some());
            assert!(reconstructed.operation_exit(operation.id).is_some());
        }
        for edge in block.terminator.edges() {
            assert!(reconstructed.edge_entry(edge).is_some());
            if matches!(
                block.terminator,
                Terminator::Jump { .. } | Terminator::Conditional { .. }
            ) {
                assert!(reconstructed.edge_exit(edge).is_some());
            }
        }
    }
}

fn multiple_entry_cycle() -> TerminalModule {
    let mut module = unranked_effectful_unit_cycle();
    let machine = &mut module.machines[0];
    let entry = machine.blocks[0].clone();
    let mut left = entry.clone();
    left.id = id(2, BlockId::new);
    left.operations[0].id = id(6, OperationId::new);
    let mut right = entry;
    right.id = id(3, BlockId::new);
    right.operations[0].id = id(7, OperationId::new);
    for (block, true_target, false_target, first_edge) in [
        (&mut machine.blocks[0], 2, 3, 10),
        (&mut left, 3, 4, 20),
        (&mut right, 2, 4, 30),
    ] {
        let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &mut block.terminator
        else {
            panic!("conditional fixture")
        };
        when_true.target = id(true_target, BlockId::new);
        when_false.target = id(false_target, BlockId::new);
        when_true.edge = id(first_edge, EdgeId::new);
        when_false.edge = id(first_edge + 1, EdgeId::new);
    }
    machine.blocks.extend([left, right]);
    module
}

#[test]
fn multiple_entry_cycle_and_exit_join_have_order_independent_frontiers() {
    let mut module = multiple_entry_cycle();
    require_complete_frontiers(&module);
    let expected = reconstruct_structural_ownership_frontiers(&module).unwrap();
    module.machines[0].blocks.reverse();
    require_complete_frontiers(&module);
    assert_eq!(
        reconstruct_structural_ownership_frontiers(&module).unwrap(),
        expected
    );
}

#[test]
fn cyclic_conditional_edges_reject_nonexistent_discards() {
    for block_id in [1, 2, 3] {
        for positive in [true, false] {
            let mut module = multiple_entry_cycle();
            let block = module.machines[0]
                .blocks
                .iter_mut()
                .find(|block| block.id == id(block_id, BlockId::new))
                .unwrap();
            let Terminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut block.terminator
            else {
                panic!("conditional fixture")
            };
            let successor = if positive { when_true } else { when_false };
            let edge = successor.edge;
            successor
                .trivial_affine_discards
                .push(id(999, PlaceId::new));
            assert!(matches!(validate_module(&module),
                Err(ModuleError::EdgeAffineDiscardsInvalid { edge: actual }) if actual == edge));
        }
    }
}

#[test]
fn cyclic_jump_rejects_nonexistent_discard() {
    let mut module = multiple_entry_cycle();
    let edge = id(40, EdgeId::new);
    module.machines[0].blocks[2].terminator = Terminator::Jump {
        edge,
        target: id(3, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: vec![id(999, PlaceId::new)],
        residual_affine_discards: Vec::new(),
    };
    assert!(matches!(validate_module(&module),
        Err(ModuleError::EdgeAffineDiscardsInvalid { edge: actual }) if actual == edge));
}

#[test]
fn unranked_cycle_exit_rejects_nonexistent_cleanup() {
    let mut module = unranked_effectful_unit_cycle();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[0].blocks[1].terminator
    else {
        panic!("Unit exit")
    };
    trivial_affine_discards.push(id(999, PlaceId::new));
    let error = validate_module(&module).expect_err("nonexistent cleanup must reject");
    assert!(
        matches!(error, ModuleError::UnitReturnAffineDiscardsMismatch { .. }),
        "{error:?}"
    );
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::Module(
            ModuleError::UnitReturnAffineDiscardsMismatch { .. }
        ))
    ));
}

#[test]
fn cyclic_scalar_return_rejects_nonexistent_cleanup() {
    let mut module = unranked_effectful_unit_cycle();
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(20, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[1].terminator = Terminator::Return {
        edge: id(12, EdgeId::new),
        value: machine.parameters[0].id,
        cleanup_actions: vec![terminal_psi::TerminalAffineCleanupAction::DiscardRoot(id(
            999,
            PlaceId::new,
        ))],
    };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ScalarReturnAffineDiscardsMismatch { .. })
    ));
}
