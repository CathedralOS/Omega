//! Two original output references remain distinct from local scratch and call results.
use super::hostile::append;
use super::*;

#[test]
fn mixed_incoming_primitive_references_and_local_reject_swapped_output_pointers() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = local_fixture(target, false);
        let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::borrowed_reference(8, 8),
                    ValueShape::borrowed_reference(8, 8),
                ],
                result: None,
            },
        )
        .unwrap();
        source.parameters = source.call_plan.parameters[..2]
            .iter()
            .enumerate()
            .map(|(position, placement)| LegalizedScalarParameter {
                value: ValueId::new(10 + position as u64).unwrap(),
                scalar_type: scalar,
                definition_site: ValueDefinitionSite::FunctionParameter(position as u32),
                placement: placement.clone(),
            })
            .collect();
        for position in 0..2 {
            let declaration = terminal_psi::StructuralParameterDeclaration {
                place: PlaceId::new(2 + position as u64).unwrap(),
                position: position as u32,
                is_self: false,
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            let signature = source.structural.as_mut().unwrap();
            signature
                .structural_places
                .push(StructuralPlaceDeclaration {
                    id: declaration.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: position as u32,
                        is_self: false,
                    },
                });
            signature
                .parameters
                .push(legalized_operations::LegalizedCallUnitParameter {
                    semantic: declaration.clone(),
                    target: target_operations::TargetStructuralParameter {
                        place: declaration.place,
                        structural_type: declaration.structural_type,
                        multiplicity: declaration.multiplicity,
                        access: declaration.access,
                        projected_qualifications: Vec::new(),
                        shape: ValueShape::borrowed_reference(8, 8),
                        placement: source.call_plan.parameters[2 + position].clone(),
                    },
                });
            append(
                &mut source,
                5 + position as u64,
                LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                    destination: declaration,
                    value: abstract_operations::AbstractResult {
                        value: ValueId::new(if position == 0 { 4 } else { 3 }).unwrap(),
                        scalar_type: scalar,
                    },
                    byte_size: 8,
                },
                None,
            );
        }
        let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[2].kind
        else {
            panic!("local call");
        };
        let mut reference = call.arguments[0].clone();
        call.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::borrowed_reference(8, 8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural {
            target: argument, ..
        } = &mut reference
        else {
            panic!("local reference");
        };
        argument.destination = call.call_plan.parameters[2].clone();
        call.arguments = call.call_plan.parameters[..2]
            .iter()
            .enumerate()
            .map(|(position, placement)| LegalizedScalarArgument::Scalar {
                source: ValueId::new(10 + position as u64).unwrap(),
                placement: placement.clone(),
            })
            .chain([reference])
            .collect();
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: source
                .parameters
                .iter()
                .enumerate()
                .map(|(parameter_index, parameter)| {
                    let [ValueLocation::Register { register, .. }] =
                        parameter.placement.locations.as_slice()
                    else {
                        panic!("two scalar inputs fit the target's argument registers");
                    };
                    SelectedFixedInputConstraint {
                        machine: source.machine,
                        source_value: parameter.value,
                        parameter_index,
                        register: *register,
                        fixed_view: environment.fixed_register_view(*register).unwrap(),
                    }
                })
                .collect(),
        };
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        let selected = construct(&source).unwrap();
        let validate = |source: &LegalizedScalarFunction, candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        // The synthetic fixture owes the same entry constraints as production.
        let mut missing_input = constraints.clone();
        missing_input.fixed_inputs.pop();
        assert!(
            build(
                0,
                &source,
                target,
                &missing_input,
                environment.physical(),
                environment.constraints(),
            )
            .is_err()
        );
        assert!(
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                &selected,
                target,
                &missing_input,
                environment.physical(),
                environment.constraints(),
            )
            .is_err()
        );
        let rows = &selected.blocks[0].instructions;
        let output = |raw| {
            rows.iter()
                .position(|row| {
                    row.provenance.operations == [OperationId::new(raw).unwrap()]
                        && matches!(row.kind, SelectedInstructionKind::Store { .. })
                })
                .unwrap()
        };
        let observed_output = output(5);
        let scalar_output = output(6);
        let scratch = output(2);
        assert_ne!(
            rows[observed_output].operands[0].virtual_register,
            rows[scalar_output].operands[0].virtual_register
        );
        assert_ne!(
            rows[observed_output].operands[0].virtual_register,
            rows[scratch].operands[0].virtual_register
        );
        assert_ne!(
            rows[observed_output].operands[1].virtual_register,
            rows[scalar_output].operands[1].virtual_register
        );
        for mutation in 0..3 {
            let mut changed = selected.clone();
            changed.blocks[0].instructions[observed_output].operands[mutation / 2]
                .virtual_register = match mutation {
                0 => rows[scalar_output].operands[0].virtual_register,
                1 => rows[scratch].operands[0].virtual_register,
                _ => rows[scalar_output].operands[1].virtual_register,
            };
            assert!(
                validate(&source, &changed).is_err(),
                "output substitution {mutation}"
            );
        }
        let mut changed = source.clone();
        let signature = changed.structural.as_mut().unwrap();
        let other = signature.parameters[1].target.placement.clone();
        signature.parameters[0].target.placement = other;
        assert!(construct(&changed).is_err());
        assert!(validate(&changed, &selected).is_err());
    }
}
