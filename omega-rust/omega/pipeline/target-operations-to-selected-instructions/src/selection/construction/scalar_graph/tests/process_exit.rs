//! Raw selection/replay tests; provider admission is exercised by native integration.
use super::*;
use semantic_vocabulary::BoundaryMachineId;

#[test]
fn process_exit_retains_boundary_but_does_not_execute_nominal_return() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = fixture(target, 0);
        source.attachment = None;
        source.blocks[0].instructions.truncate(2);
        source.provenance.operations.truncate(2);
        let constant = &mut source.blocks[0].instructions[0];
        constant.result.as_mut().unwrap().scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        constant.kind = LegalizedScalarInstructionKind::Constant(IntegerValue::Signed(255));
        let row = &mut source.blocks[0].instructions[1];
        row.result = None;
        row.ownership = vec![optimization_unit::OwnershipEvent::ClaimCompletion(
            Vec::new(),
        )];
        row.kind = LegalizedScalarInstructionKind::HostedExitProcessI32 {
            boundary: BoundaryMachineId::new(1).unwrap(),
            source: ValueId::new(1).unwrap(),
        };
        let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
            panic!("fixture return");
        };
        returned.ownership = vec![optimization_unit::OwnershipEvent::Cleanup(Vec::new())];
        returned.fuel = vec![optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Edge(returned.edge),
            units: 999,
        }];
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert!(selected.local_storage_slots.is_empty());
        assert!(selected.memory_accesses.is_empty());
        let SelectedTerminator::HostedExitProcess { instruction, .. } =
            &selected.blocks[0].terminator
        else {
            panic!("exit terminal");
        };
        assert_eq!(
            instruction.provenance.fuel,
            source.blocks[0].instructions[1].fuel
        );
        assert!(instruction.provenance.edges.is_empty());
        for mutation in 0..5 {
            let mut changed = selected.clone();
            let SelectedTerminator::HostedExitProcess {
                instruction,
                nominal_return_edge,
            } = &mut changed.blocks[0].terminator
            else {
                unreachable!();
            };
            match mutation {
                0 => *nominal_return_edge = EdgeId::new(99).unwrap(),
                1 => instruction.kind = SelectedInstructionKind::ReturnUnit,
                2 => instruction.provenance.fuel.extend(returned_fuel(&source)),
                3 => instruction.provenance.values.clear(),
                4 => changed.boundary_settlements.clear(),
                _ => unreachable!(),
            }
            assert!(validate(&changed).is_err());
        }
    }
}

fn returned_fuel(source: &LegalizedScalarFunction) -> Vec<optimization_unit::FuelSettlement> {
    let LegalizedScalarTerminator::Return(returned) = &source.blocks[0].terminator else {
        unreachable!();
    };
    returned.fuel.clone()
}
