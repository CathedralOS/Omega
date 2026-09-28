//! Empty construction retains ordered semantic charges without payload storage.
use super::super::{
    FuelSettlement, OperationId, PsiProvenance, SelectedTerminator, ValueDefinitionSite,
};
use super::{
    LegalizedScalarInstructionKind, PlaceId, SelectedFunction, SelectedSelectionConstraints,
    StructuralPlaceDeclaration, StructuralPlaceKind, build,
};
use crate::selection::construction::scalar_graph::tests::scalar_arrays::array_fixture;

#[test]
fn empty_array_constructor_retains_charge_without_physical_storage() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = array_fixture(target, 0);
        let constructor = source.blocks[0].instructions.pop().unwrap();
        source.blocks[0].instructions.clear();
        source.blocks[0].instructions.push(constructor.clone());
        source.provenance.operations = vec![constructor.operation];
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
        let validate = |selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert!(selected.local_storage_slots.is_empty());
        assert!(selected.memory_accesses.is_empty());
        assert!(selected.virtual_registers.is_empty());
        assert!(selected.blocks[0].instructions.is_empty());
        let SelectedTerminator::Return { instruction, .. } = &selected.blocks[0].terminator else {
            panic!("empty array return");
        };
        assert_eq!(instruction.provenance.operations, [constructor.operation]);
        assert_eq!(
            &instruction.provenance.fuel[..constructor.fuel.len()],
            &constructor.fuel
        );
        for mutation in 0..3 {
            let mut changed = selected.clone();
            let SelectedTerminator::Return { instruction, .. } = &mut changed.blocks[0].terminator
            else {
                panic!("return");
            };
            match mutation {
                0 => instruction.provenance.operations.clear(),
                1 => {
                    instruction.provenance.fuel.remove(0);
                }
                _ => instruction.provenance.fuel.push(constructor.fuel[0]),
            }
            assert!(validate(&changed).is_err(), "mutation {mutation}");
        }
    }
}

#[test]
fn consecutive_empty_constructor_charges_precede_the_next_ordinary_operation() {
    let target = target::NativeTarget::linux_arm64();
    let mut source = array_fixture(target, 0);
    let constructor = source.blocks[0].instructions.pop().unwrap();
    let mut second = constructor.clone();
    second.operation = OperationId::new(99).unwrap();
    second.fuel = vec![FuelSettlement {
        site: PsiProvenance::Operation(second.operation),
        units: 1,
    }];
    let LegalizedScalarInstructionKind::EstablishScalarArray { result, .. } = &mut second.kind
    else {
        panic!("array");
    };
    result.place = PlaceId::new(99).unwrap();
    source
        .structural
        .as_mut()
        .unwrap()
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: second.operation,
                structural_type: result.structural_type,
            },
        });
    source.blocks[0].instructions.insert(0, second.clone());
    source.blocks[0].instructions.insert(0, constructor.clone());
    source.provenance.operations = source.blocks[0]
        .instructions
        .iter()
        .map(|row| row.operation)
        .collect();
    let block = source.blocks[0].id;
    for (ordinal, row) in source.blocks[0].instructions.iter_mut().enumerate() {
        if let Some(result) = &mut row.result {
            result.definition_site = ValueDefinitionSite::Node {
                block,
                node: ordinal as u32,
            };
        }
    }
    let next = source.blocks[0].instructions[2].clone();
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
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
    let validate = |selected: &SelectedFunction| {
        crate::selection::validation::scalar_graph::validate(
            0,
            &source,
            selected,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
    };
    validate(&selected).unwrap();
    assert!(selected.local_storage_slots.is_empty());
    assert!(selected.memory_accesses.is_empty());
    let first = &selected.blocks[0].instructions[0];
    assert_eq!(
        first.provenance.operations,
        [constructor.operation, second.operation, next.operation]
    );
    let mut expected_fuel = constructor.fuel.clone();
    expected_fuel.extend(second.fuel.clone());
    expected_fuel.extend(next.fuel.clone());
    assert_eq!(first.provenance.fuel, expected_fuel);
    let mut changed = selected.clone();
    changed.blocks[0].instructions[0]
        .provenance
        .operations
        .swap(0, 1);
    assert!(validate(&changed).is_err());
    let mut changed = selected.clone();
    changed.blocks[0].instructions[0].provenance.fuel.swap(0, 1);
    assert!(validate(&changed).is_err());
}
