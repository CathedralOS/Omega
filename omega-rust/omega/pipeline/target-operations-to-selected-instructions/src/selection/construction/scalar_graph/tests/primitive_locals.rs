//! Original frame identity survives borrowed calls and fresh scalar reads.
use super::*;
mod hostile;
mod mixed_references;
mod reentry;
mod widths;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn local_fixture(target: target::NativeTarget, unit_call: bool) -> LegalizedScalarFunction {
    let mut source = fixture(target, 0);
    source.attachment = None;
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let place = PlaceId::new(1).unwrap();
    let identity = StructuralTypeId::new(1).unwrap();
    let producer = OperationId::new(2).unwrap();
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        result: None,
        structural_types: vec![StructuralTypeDeclaration {
            id: identity,
            identity: "u64".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar),
        }]
        .into(),
        parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: identity,
            },
        }],
    });
    source.blocks[0].instructions[1].result = None;
    source.blocks[0].instructions[1].kind =
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
            result: StructuralOperationResult {
                place,
                structural_type: identity,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            value: abstract_operations::AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            },
            shape: ValueShape::integer(8, 8),
        };
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(8, 8)],
            result: (!unit_call).then_some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    source.blocks[0].instructions[2].kind = LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
        structural_result: None,
        source: LegalizedCallUnitSource::AuthoredCallUnit, callee: MachineId::new(10).unwrap(),
        arguments: vec![LegalizedScalarArgument::Structural {
            semantic: terminal_psi::StructuralArgument { place, access: StructuralAccess::MutableBorrow, path: Vec::new() },
            target: target_operations::TargetStructuralArgument {
                place, access: StructuralAccess::MutableBorrow, path: Vec::new(), root_structural_type: identity, structural_type: identity,
                shape: ValueShape::borrowed_reference(8, 8), source_byte_offset: 0, fixed_array_length: None, element_stride: None,
                source: target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal { psi_operation: producer },
                destination: call_plan.parameters[0].clone(),
            },
        }],
        result_placement: call_plan.result.clone(), call_plan, claim_transfers: Vec::new(), requirement_obligations: Vec::new(), crash_continuations: Vec::new(),
    });
    source.blocks[0].instructions[2].ownership =
        vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())];
    if unit_call {
        source.blocks[0].instructions[2].result = None;
    }
    source.blocks[0].instructions[3].kind =
        LegalizedScalarInstructionKind::PrimitiveScalarRead { source: place };
    source
}

#[test]
fn primitive_local_call_and_read_replay_rejects_source_and_physical_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for unit_call in [false, true] {
            let source = local_fixture(target, unit_call);
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
            assert_eq!(selected.local_storage_slots.len(), 1);
            assert!(
                selected.memory_accesses.iter().any(|access| access.role
                    == selected_instructions::SelectedMemoryAccessRole::WritePlace)
            );
            assert!(
                selected.memory_accesses.iter().any(|access| access.role
                    == selected_instructions::SelectedMemoryAccessRole::ReadPlace)
            );
            for mutation in 0..5 {
                let mut changed = selected.clone();
                match mutation {
                    0 => changed.local_storage_slots[0].byte_size = 4,
                    1 => changed.memory_accesses[0].place = PlaceId::new(99).unwrap(),
                    2 => {
                        let store = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
                            .unwrap();
                        store.operands.swap(0, 1);
                    }
                    3 => {
                        let read = changed.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| matches!(row.kind, SelectedInstructionKind::Load64 { .. }))
                            .unwrap();
                        read.kind = SelectedInstructionKind::Load64 { byte_offset: 8 };
                    }
                    _ => changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
                        .unwrap()
                        .provenance
                        .fuel
                        .clear(),
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "physical mutation {mutation}"
                );
            }
            for mutation in 0..4 {
                let mut changed = source.clone();
                match mutation {
                    0 => {
                        let LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                            shape, ..
                        } = &mut changed.blocks[0].instructions[1].kind
                        else {
                            panic!("local");
                        };
                        *shape = ValueShape::integer(4, 4);
                    }
                    1 => {
                        changed.blocks[0].instructions[3]
                            .result
                            .as_mut()
                            .unwrap()
                            .scalar_type = ScalarType::Boolean
                    }
                    2 => {
                        let LegalizedScalarInstructionKind::Call(call) =
                            &mut changed.blocks[0].instructions[2].kind
                        else {
                            panic!("call");
                        };
                        let LegalizedScalarArgument::Structural { target, .. } =
                            &mut call.arguments[0]
                        else {
                            panic!("reference");
                        };
                        target.source = target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal { psi_operation: OperationId::new(99).unwrap() };
                    }
                    _ => changed.blocks[0].instructions.swap(1, 2),
                }
                assert!(
                    validate(&changed, &selected).is_err(),
                    "source mutation {mutation}"
                );
                assert!(
                    build(
                        0,
                        &changed,
                        target,
                        &constraints,
                        environment.physical(),
                        environment.constraints()
                    )
                    .is_err(),
                    "construction mutation {mutation}"
                );
            }
        }
    }
}
