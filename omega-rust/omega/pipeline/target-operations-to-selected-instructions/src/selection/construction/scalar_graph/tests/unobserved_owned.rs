//! Raw projection controls; complete native admission is exercised by source tests.
use super::*;
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn source(native: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = super::control::graph(
        native,
        legalized_operations::LegalizedScalarComparison::Equal,
        false,
    );
    let identity = StructuralTypeId::new(1).unwrap();
    let place = |ordinal| semantic_vocabulary::PlaceId::new(ordinal).unwrap();
    let declaration = |ordinal| StructuralParameterDeclaration {
        place: place(ordinal),
        position: 0,
        is_self: false,
        structural_type: identity,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8)],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        structural_types: vec![StructuralTypeDeclaration {
            id: identity,
            identity: "test::Payload".into(),
            shape: StructuralTypeShape::Record {
                fields: (1..=2)
                    .map(|ordinal| StructuralFieldDeclaration {
                        id: semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap(),
                        identity: format!("field{ordinal}"),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    })
                    .collect(),
            },
        }],
        parameters: vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: declaration(1),
            target: target_operations::TargetStructuralParameter {
                place: place(1),
                structural_type: identity,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                projected_qualifications: Vec::new(),
                shape: ValueShape::integer(16, 8),
                placement: source.call_plan.parameters[0].clone(),
            },
        }],
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    for block in &mut source.blocks {
        let ordinal = block.id.get();
        let owner = if block.id == source.entry_block {
            place(1)
        } else {
            block.structural_parameters.push(declaration(ordinal + 10));
            place(ordinal + 10)
        };
        let bind = |edge: &mut legalized_operations::LegalizedScalarSuccessor| {
            edge.structural_bindings
                .push(abstract_operations::AbstractStructuralBinding {
                    parameter: place(edge.target.get() + 10),
                    argument: terminal_psi::StructuralArgument {
                        place: owner,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                });
        };
        match &mut block.terminator {
            LegalizedScalarTerminator::Jump { successor, .. } => bind(successor),
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                bind(when_true);
                bind(when_false);
            }
            LegalizedScalarTerminator::Return(returned) => {
                returned.ownership = vec![optimization_unit::OwnershipEvent::Cleanup(vec![
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(owner),
                ])]
            }
            _ => unreachable!(),
        }
    }
    source
}

#[test]
fn unused_owned_bindings_select_and_replay_without_homes_or_copies() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let source = source(native);
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            native,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                candidate,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(selected.structural, source.structural);
        assert_eq!(
            source.call_plan.parameters[0].shape,
            ValueShape::integer(16, 8)
        );
        for block in &selected.blocks {
            let edges: Vec<_> = match &block.terminator {
                SelectedTerminator::Jump { successor, .. } => vec![successor],
                SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } => vec![when_nonzero, when_zero],
                _ => Vec::new(),
            };
            for edge in edges {
                let semantic = source
                    .blocks
                    .iter()
                    .flat_map(|block| match &block.terminator {
                        LegalizedScalarTerminator::Jump { successor, .. } => vec![successor],
                        LegalizedScalarTerminator::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => vec![when_true, when_false],
                        _ => Vec::new(),
                    })
                    .find(|candidate| candidate.edge == edge.psi_edge)
                    .unwrap();
                assert_eq!(edge.source_target, semantic.target);
                assert_eq!(
                    edge.structural_bindings
                        .iter()
                        .map(|binding| &binding.semantic)
                        .collect::<Vec<_>>(),
                    semantic.structural_bindings.iter().collect::<Vec<_>>()
                );
                assert_eq!(edge.structural_bindings.len(), 1);
                assert_eq!(
                    edge.structural_bindings[0].transport,
                    selected_instructions::SelectedStructuralTransport::Unused
                );
                assert_eq!(
                    edge.structural_bindings[0].semantic.argument.access,
                    StructuralAccess::Owned
                );
            }
        }
        assert!(selected.memory_accesses.is_empty());
        assert!(selected.local_storage_slots.is_empty());
        assert!(selected.virtual_registers.iter().all(|register| !matches!(
            register.origin,
            VirtualRegisterOrigin::StructuralParameter { .. }
                | VirtualRegisterOrigin::AbiTransport { .. }
        )));
        let mut prepared = selected.clone();
        crate::selection::edge_transfers::prepare(
            0,
            &mut prepared,
            &constraints,
            environment.constraints(),
        )
        .unwrap();
        let projected =
            crate::selection::edge_transfers::project(0, &prepared, &constraints).unwrap();
        assert_eq!(projected, selected);
        assert!(prepared.memory_accesses.is_empty());
        let mut changed = selected.clone();
        changed
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                    block: BlockId::new(4).unwrap(),
                    place: semantic_vocabulary::PlaceId::new(14).unwrap(),
                },
                byte_size: 16,
                alignment: 8,
            });
        assert!(
            validate(&changed).is_err(),
            "unused owned data cannot acquire a descriptor home"
        );
        let mut changed = selected.clone();
        let binding = changed
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                SelectedTerminator::Jump { successor, .. } => {
                    successor.structural_bindings.first_mut()
                }
                _ => None,
            })
            .unwrap();
        binding.transport = selected_instructions::SelectedStructuralTransport::Descriptor {
            argument: VirtualRegisterId(0),
            destination: selected_instructions::LocalStorageSlotId::StructuralBlockParameter {
                block: BlockId::new(4).unwrap(),
                place: semantic_vocabulary::PlaceId::new(14).unwrap(),
            },
        };
        assert!(
            validate(&changed).is_err(),
            "invented physical transport must reject"
        );
        let mut observed = source.clone();
        let row = observed
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .next()
            .unwrap();
        row.kind = LegalizedScalarInstructionKind::ByteSequenceLength {
            source: semantic_vocabulary::PlaceId::new(1).unwrap(),
            length_byte_offset: 8,
        };
        assert!(!crate::unobserved_owned_input::accepts(&observed));
        assert!(
            build(
                0,
                &observed,
                native,
                &constraints,
                environment.physical(),
                environment.constraints()
            )
            .is_err()
        );
    }
}
