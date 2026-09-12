//! An owned incoming record is ordinary initialized child storage.
use super::*;
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

#[test]
fn owned_parameter_child_rejoins_exact_captured_fragments() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = nested_fixture(target, 2);
        let child = source.blocks[0].instructions.remove(2);
        let LegalizedScalarInstructionKind::EstablishRecord { result, shape, .. } = child.kind
        else {
            panic!("child");
        };
        source
            .provenance
            .operations
            .retain(|operation| *operation != child.operation);
        source.call_plan = evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: vec![shape],
                result: source
                    .call_plan
                    .result
                    .as_ref()
                    .map(|placement| placement.shape),
            },
        )
        .unwrap();
        let signature = source.structural.as_mut().unwrap();
        signature
            .structural_places
            .retain(|place| place.id != result.place);
        signature
            .parameters
            .push(legalized_operations::LegalizedCallUnitParameter {
                semantic: terminal_psi::StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    is_self: false,
                    structural_type: result.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place: result.place,
                    structural_type: result.structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    projected_qualifications: Vec::new(),
                    shape,
                    placement: source.call_plan.parameters[0].clone(),
                },
            });
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
        .unwrap_or_else(|error| panic!("parameter {target:?}: {error:?}"));
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        for mutation in 0..4 {
            let mut changed = source.clone();
            let parameter = &mut changed.structural.as_mut().unwrap().parameters[0];
            match mutation {
                0 => parameter.semantic.access = StructuralAccess::SharedBorrow,
                1 => parameter.semantic.multiplicity = StructuralMultiplicity::Linear,
                2 => parameter.semantic.structural_type = StructuralTypeId::new(3).unwrap(),
                3 => parameter.target.placement.locations.clear(),
                _ => unreachable!(),
            }
            assert!(validate(&changed, &selected).is_err());
        }
        let mut changed = selected.clone();
        let register = changed
            .virtual_registers
            .iter_mut()
            .find(|register| {
                matches!(
                    register.origin,
                    selected_instructions::VirtualRegisterOrigin::StructuralParameter { .. }
                )
            })
            .unwrap();
        register.entry_fixed_view = None;
        assert!(validate(&source, &changed).is_err());
        let mut copy_source = source.clone();
        let parameter = &mut copy_source.structural.as_mut().unwrap().parameters[0];
        parameter.semantic.multiplicity = StructuralMultiplicity::Unrestricted;
        parameter.target.multiplicity = StructuralMultiplicity::Unrestricted;
        let copied = build(
            0,
            &copy_source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate(&copy_source, &copied).unwrap();
        let incoming = copied
            .virtual_registers
            .iter()
            .find(|register| {
                matches!(
                    register.origin,
                    selected_instructions::VirtualRegisterOrigin::StructuralParameter { .. }
                )
            })
            .unwrap()
            .id;
        let mut aliased = copied.clone();
        let store = aliased
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .rfind(|row| {
                matches!(
                    row.kind,
                    SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 2
                    }
                )
            })
            .expect("nested two-byte child is copied into parent storage");
        assert_ne!(store.operands[0].virtual_register, incoming);
        store.operands[0].virtual_register = incoming;
        assert!(
            validate(&copy_source, &aliased).is_err(),
            "owned copy cannot write through the incoming value"
        );
    }
}
