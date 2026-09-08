//! Shared descriptor calls retain the original pointer and independent replay.
use super::*;
mod literal_storage;
mod mixed;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(super) fn borrowed_call(target: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = fixture(target, 0);
    source.attachment = None;
    source.blocks[0].instructions.truncate(1);
    source.provenance.operations.truncate(1);
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let shape = ValueShape::borrowed_reference(16, 8);
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    let placement = source.call_plan.parameters[0].clone();
    assert!(
        matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(_),
                copy_stack_byte_offset: None,
                byte_size: 16,
                alignment: 8,
            }]
        ),
        "the derived borrow placement retains referent shape and transports its pointer"
    );
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        parameters: vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            target: target_operations::TargetStructuralParameter {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                projected_qualifications: Vec::new(),
                shape,
                placement: placement.clone(),
            },
        }],
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    source.blocks[0].instructions[0].kind =
        LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            source: LegalizedCallUnitSource::AuthoredCallUnit,
            callee: MachineId::new(2).unwrap(),
            call_plan: source.call_plan.clone(),
            arguments: vec![LegalizedScalarArgument::Structural {
                semantic: StructuralArgument {
                    place,
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                },
                target: target_operations::TargetStructuralArgument {
                    place,
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                    root_structural_type: structural_type,
                    structural_type,
                    shape,
                    source_byte_offset: 0,
                    fixed_array_length: None,
                    element_stride: None,
                    source: placement.clone().into(),
                    destination: placement,
                },
            }],
            result_placement: source.call_plan.result.clone(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        });
    source.blocks[0].instructions[0].ownership =
        vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())];
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(1).unwrap(),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    };
    source
}

#[test]
fn mixed_borrowed_calls_preserve_separate_scalar_and_pointer_placements() {
    for (target, maximum_scalars) in [
        (target::NativeTarget::linux_x64(), 5),
        (target::NativeTarget::linux_arm64(), 7),
        (target::NativeTarget::windows_x64(), 3),
        (target::NativeTarget::macos_arm64(), 7),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for scalar_count in 1..=maximum_scalars {
            let mut source = borrowed_call(target);
            let mut call_row = source.blocks[0].instructions.remove(0);
            call_row.operation = OperationId::new(10).unwrap();
            call_row.result.as_mut().unwrap().value = ValueId::new(10).unwrap();
            call_row.result.as_mut().unwrap().definition_site = ValueDefinitionSite::Node {
                block: source.entry_block,
                node: 1,
            };
            call_row.fuel[0].site = PsiProvenance::Operation(call_row.operation);
            let LegalizedScalarInstructionKind::Call(call) = &mut call_row.kind else {
                panic!("call");
            };
            call.call_plan = evaluate_call_plan(
                source.call_plan.policy,
                &CallSignature {
                    parameters: std::iter::repeat_n(ValueShape::integer(8, 8), scalar_count)
                        .chain(std::iter::once(ValueShape::borrowed_reference(16, 8)))
                        .collect(),
                    result: Some(ValueShape::integer(8, 8)),
                },
            )
            .unwrap();
            let mut borrowed = call.arguments.remove(0);
            let LegalizedScalarArgument::Structural {
                target: argument, ..
            } = &mut borrowed
            else {
                panic!("borrow");
            };
            argument.destination = call.call_plan.parameters[scalar_count].clone();
            call.arguments = call.call_plan.parameters[..scalar_count]
                .iter()
                .map(|placement| LegalizedScalarArgument::Scalar {
                    source: ValueId::new(1).unwrap(),
                    placement: placement.clone(),
                })
                .chain(std::iter::once(borrowed))
                .collect();
            source.blocks[0].instructions = vec![
                fixture(target, 0).blocks[0].instructions[0].clone(),
                call_row,
            ];
            source.provenance.operations =
                vec![OperationId::new(1).unwrap(), OperationId::new(10).unwrap()];
            returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
                value: ValueId::new(10).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
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
            assert!(selected.outgoing_arguments.is_empty());
            assert!(
                selected.memory_accesses.is_empty(),
                "references are never copied as values"
            );
            for mutation in 0..4 {
                let mut changed = source.clone();
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut changed.blocks[0].instructions[1].kind
                else {
                    panic!("call");
                };
                match mutation {
                    0 => call.arguments.swap(0, scalar_count),
                    1 => {
                        call.arguments.remove(0);
                    }
                    2 => {
                        let LegalizedScalarArgument::Scalar { placement, .. } =
                            &mut call.arguments[0]
                        else {
                            panic!("scalar");
                        };
                        *placement = call.call_plan.parameters[scalar_count].clone();
                    }
                    _ => {
                        let LegalizedScalarArgument::Structural { target, .. } =
                            &mut call.arguments[scalar_count]
                        else {
                            panic!("borrow");
                        };
                        let target_operations::TargetStructuralArgumentSource::Placement(placement) =
                            &target.source
                        else {
                            panic!("borrowed placement");
                        };
                        target.destination = placement.clone();
                    }
                }
                assert!(construct(&changed).is_err(), "input mutation {mutation}");
                assert!(
                    validate(&changed, &selected).is_err(),
                    "receiving mutation {mutation}"
                );
            }
            let mut changed = selected.clone();
            let call = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|instruction| {
                    matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. })
                })
                .unwrap();
            call.operands.swap(0, scalar_count);
            assert!(
                validate(&source, &changed).is_err(),
                "scalar and pointer register roles cannot be swapped"
            );
        }
    }
}

#[test]
fn borrowed_descriptor_call_forwards_pointer_and_replays_custody() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let source = borrowed_call(target);
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
        assert!(selected.outgoing_arguments.is_empty());
        assert!(
            selected.memory_accesses.is_empty(),
            "forwarding cannot read or copy descriptor bytes"
        );
        assert_eq!(selected.calls.len(), 1);
        let instructions = &selected.blocks[0].instructions;
        assert_eq!(instructions[0].kind, SelectedInstructionKind::CopyI64);
        assert_eq!(instructions[1].kind, SelectedInstructionKind::CopyI64);
        assert_eq!(
            instructions[1].operands[0].virtual_register,
            instructions[0].operands[1].virtual_register
        );
        assert_eq!(
            instructions[2].kind,
            SelectedInstructionKind::CallI64 {
                callee: MachineId::new(2).unwrap()
            }
        );
        assert_eq!(
            instructions[2].operands[0].virtual_register,
            instructions[1].operands[1].virtual_register
        );
        assert_eq!(
            selected.calls[0].call,
            match &source.blocks[0].instructions[0].kind {
                LegalizedScalarInstructionKind::Call(call) => call.clone(),
                _ => unreachable!(),
            }
        );

        for mutation in 0..9 {
            let mut changed = selected.clone();
            match mutation {
                0 => {
                    changed.blocks[0].instructions[1].operands[0].virtual_register =
                        VirtualRegisterId(0)
                }
                1 => {
                    changed.blocks[0].instructions[2].operands[0].virtual_register =
                        VirtualRegisterId(1)
                }
                2 => changed.blocks[0].instructions[2].operands.swap(0, 1),
                3 => changed.calls.clear(),
                4 => changed.calls[0].call.callee = MachineId::new(3).unwrap(),
                5 => {
                    changed.virtual_registers[2].origin = VirtualRegisterOrigin::AbiTransport {
                        instruction: SelectedInstructionId(1),
                        place: PlaceId::new(2).unwrap(),
                        byte_offset: 0,
                    }
                }
                6 => changed.blocks[0].instructions[2].provenance.fuel.clear(),
                7 => changed.blocks[0].instructions[2].operands[0].fixed_view = None,
                _ => changed.calls[0].ownership.clear(),
            }
            assert!(
                validate(&source, &changed).is_err(),
                "selected mutation {mutation}"
            );
        }

        let mut missing_transfer = source.clone();
        missing_transfer.blocks[0].instructions[0].ownership.clear();
        assert!(construct(&missing_transfer).is_err());
        assert!(validate(&missing_transfer, &selected).is_err());

        for mutation in 0..13 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut changed.blocks[0].instructions[0].kind
            else {
                unreachable!();
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                unreachable!();
            };
            match mutation {
                0 => semantic.access = StructuralAccess::Owned,
                1 => target.access = StructuralAccess::MutableBorrow,
                2 => target.place = PlaceId::new(2).unwrap(),
                3 => semantic.place = PlaceId::new(2).unwrap(),
                4 => target.root_structural_type = StructuralTypeId::new(2).unwrap(),
                5 => target.structural_type = StructuralTypeId::new(2).unwrap(),
                6 => target.shape = ValueShape::integer(16, 8),
                7 => target.source_byte_offset = 8,
                8 => target.fixed_array_length = Some(2),
                9 => target.element_stride = Some(8),
                10 => {
                    let target_operations::TargetStructuralArgumentSource::Placement(placement) =
                        &mut target.source
                    else {
                        panic!("borrowed placement");
                    };
                    placement.shape = ValueShape::integer(16, 8);
                }
                11 => target.destination.shape = ValueShape::integer(16, 8),
                _ => semantic
                    .path
                    .push(terminal_psi::StructuralPathSegment::FixedIndex(0)),
            }
            assert!(construct(&changed).is_err(), "source mutation {mutation}");
            assert!(
                validate(&changed, &selected).is_err(),
                "receiving mutation {mutation}"
            );
        }

        // Recomputing a value-copy ABI cannot authorize a source borrow.
        let mut copied = source.clone();
        let LegalizedScalarInstructionKind::Call(call) = &mut copied.blocks[0].instructions[0].kind
        else {
            unreachable!();
        };
        call.call_plan = evaluate_call_plan(
            call.call_plan.policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(16, 8)],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[0] else {
            unreachable!();
        };
        target.shape = ValueShape::integer(16, 8);
        target.destination = call.call_plan.parameters[0].clone();
        assert!(construct(&copied).is_err());
        assert!(validate(&copied, &selected).is_err());

        let mut copied = source.clone();
        let LegalizedScalarInstructionKind::Call(call) = &mut copied.blocks[0].instructions[0].kind
        else {
            unreachable!();
        };
        let ValueLocation::Indirect {
            copy_stack_byte_offset,
            ..
        } = &mut call.call_plan.parameters[0].locations[0]
        else {
            unreachable!();
        };
        *copy_stack_byte_offset = Some(0);
        let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[0] else {
            unreachable!();
        };
        target.destination = call.call_plan.parameters[0].clone();
        assert!(
            construct(&copied).is_err(),
            "borrowed placement cannot acquire a referent copy slot"
        );
        assert!(validate(&copied, &selected).is_err());
    }
}
