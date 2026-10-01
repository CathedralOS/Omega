//! `initialize(player: &mut Player, name: [u8; 16] in Utf8)` copies the
//! owned formal's live bytes into `player.name`, without borrowing its ABI.
//! This native consumer fixture supplies the exact admitted capacity fact;
//! source checking and proof production are covered by the Stage05 regression.

use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarTerm,
    ScalarType,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet,
    StructuralAccess, StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn id<T: PsiSemanticId>(value: u64) -> T {
    T::new(value).unwrap()
}

#[test]
fn completed_owned_byte_call_rejects_consumer_before_producer() {
    use abstract_operations::{AbstractFunctionResult, AbstractStructuralCallResult};
    use terminal_psi::{
        StructuralArgument, StructuralOperationResult, StructuralResultDeclaration,
    };

    // This control isolates operation order. Qualified owned-parameter return
    // is a separate unsupported lane; the preceding Stage02 and entry-home
    // tests retain the predicate-corruption controls for qualified bytes.
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    source.structural_types = vec![StructuralTypeDeclaration {
        id: id(1),
        identity: "Bytes8".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
    }]
    .into();
    let parameter = |place| StructuralParameterDeclaration {
        place: id(place),
        position: 0,
        is_self: false,
        structural_type: id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut producer = source.functions[0].clone();
    producer.machine = id(2);
    producer.entry = id(2);
    producer.block_entries[0].block = id(2);
    producer.structural_parameters = vec![parameter(2)];
    producer.result = AbstractFunctionResult::Structural(StructuralResultDeclaration {
        place: id(3),
        structural_type: id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    producer.operations = vec![AbstractOperation::ReturnStructural {
        psi_edge: id(2),
        source: id(2),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }];
    let mut consumer = source.functions[0].clone();
    consumer.machine = id(3);
    consumer.entry = id(3);
    consumer.block_entries[0].block = id(3);
    consumer.structural_parameters = vec![parameter(5)];
    consumer.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: id(3),
        cleanup_actions: Vec::new(),
    }];
    let call = |operation, callee, place, result| AbstractOperation::CallStructural {
        psi_operation: id(operation),
        callee: id(callee),
        result,
        arguments: Vec::new(),
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: id(place),
            path: Vec::new(),
            access: StructuralAccess::Owned,
        }],
    };
    source.functions[0].structural_parameters = vec![parameter(1)];
    source.functions[0].operations.splice(
        0..0,
        [
            call(
                1,
                2,
                1,
                AbstractStructuralCallResult::Structural {
                    result: StructuralOperationResult {
                        place: id(4),
                        structural_type: id(1),
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                    returned_claim_transfers: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            ),
            call(2, 3, 4, AbstractStructuralCallResult::Unit),
        ],
    );
    source.functions.extend([producer, consumer]);
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("owned bytes forward through their exact normal result before the consumer");
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap();
        let legalized = crate::legalize_target_operations(&target, &source, &unit)
            .expect("the in-order completed owner is a valid call argument");
        crate::validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
            .unwrap();
        let mut reordered = target.clone();
        reordered.functions[0].graph.blocks[0].operations.swap(0, 1);
        // Stage02's roster receipt is deliberately not a body-order judgment.
        // The mandatory receiving checker must reject the same retained rows
        // when their executable order no longer matches the source graph.
        assert!(
            crate::legalize_target_operations(&reordered, &source, &unit).is_err(),
            "{native:?}: consumer before completion"
        );
        assert!(
            crate::validate_legalized_operations(
                &reordered,
                &source,
                &unit,
                legalized.plan().clone()
            )
            .is_err(),
            "{native:?}: an unchanged legalized plan cannot launder reordered target calls"
        );
    }
}

#[test]
fn owned_qualified_byte_parameter_replaces_field_through_four_native_abis() {
    check_owned_byte_parameter_store(16, false);
}

#[test]
fn register_carried_owned_byte_parameter_retains_entry_home() {
    check_owned_byte_parameter_store(8, false);
}

#[test]
fn owned_record_byte_field_replacement_keeps_its_local_home() {
    check_owned_byte_parameter_store(8, true);
}

fn check_owned_byte_parameter_store(capacity: u64, owned_destination: bool) {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        predicates.insert(ByteSequencePredicate::ValidUtf8);
        let carrier = ByteSequenceCarrier::BoundedOwned { capacity };
        source.structural_types = vec![
            StructuralTypeDeclaration {
                id: id(1),
                identity: "Player".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: id(1),
                        identity: "name".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::PredicateByteSequence {
                            carrier,
                            predicates,
                        },
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: id(2),
                identity: format!("[u8;{capacity}]"),
                shape: StructuralTypeShape::ByteSequence(carrier),
            },
        ]
        .into();
        let function = &mut source.functions[0];
        function.structural_parameters = vec![
            StructuralParameterDeclaration {
                place: id(1),
                position: 0,
                is_self: false,
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: id(2),
                position: 1,
                is_self: false,
                structural_type: id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: vec![id(1)],
                projected_qualifications: Vec::new(),
            },
        ];
        let returned = function.operations.pop().unwrap();
        function.operations = vec![
            AbstractOperation::ByteSequenceLength {
                psi_operation: id(1),
                source: id(2),
                result: AbstractResult {
                    value: id(1),
                    scalar_type: ScalarType::Integer(integer),
                },
            },
            AbstractOperation::StructuralByteSequenceFieldStore {
                psi_operation: id(2),
                destination: id(1),
                path: Vec::new(),
                field: id(1),
                source: id(2),
                length: id(1),
                obligation: id(1),
            },
            returned,
        ];
        let source_parameter_index = if owned_destination {
            function.structural_parameters.remove(0);
            function.structural_parameters[0].position = 0;
            function.operations.insert(
                0,
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: id(3),
                    result: terminal_psi::StructuralOperationResult {
                        place: id(1),
                        structural_type: id(1),
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
            );
            function.result = abstract_operations::AbstractFunctionResult::Structural(
                terminal_psi::StructuralResultDeclaration {
                    place: id(4),
                    structural_type: id(1),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: Vec::new(),
                },
            );
            function.operations.pop();
            function.operations.extend([
                AbstractOperation::StructuralLeafCopy {
                    psi_operation: id(4),
                    source: id(1),
                    path: Vec::<terminal_psi::StructuralPathSegment>::new().into(),
                    result: terminal_psi::StructuralOperationResult {
                        place: id(3),
                        structural_type: id(1),
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
                AbstractOperation::ReturnStructural {
                    psi_edge: id(1),
                    source: id(3),
                    returned_claims: Vec::new(),
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            ]);
            0
        } else {
            1
        };
        let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .expect("reconstruct the actual owned parameter and byte operations");
        unit.structural_domains = vec![StructuralDomainDeclaration {
            id: id(1),
            semantic_domain: id(1),
            identity: format!("Utf8<[u8;{capacity}]>"),
            carrier: id(2),
            byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
            content_projection: None,
            establishment_routes: Vec::new(),
        }]
        .into();
        let capacity_goal = semantic_vocabulary::Proposition::LessOrEqual(
            ScalarTerm::value(id(1), ScalarType::Integer(integer)),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(capacity.into())).unwrap(),
        );
        let accepted = optimization_unit::AcceptedObligationFact::new(
            unit.psi,
            [4; 32],
            source.functions[0].machine,
            id(2),
            id(1),
            terminal_codec::canonical_proposition_order_key(&capacity_goal).unwrap(),
        );
        let unit = optimization_unit::attach_accepted_obligation_facts(unit, vec![accepted])
            .expect("retain the exact store capacity goal");
        optimization_unit_semantics::validate_psi_optimization_unit(&unit)
            .expect("independently validate the qualified owned source before native lowering");
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut changed = unit.clone();
            changed.functions[0].structural_parameters[source_parameter_index].access = access;
            changed.identity =
                optimization_unit::recompute_psi_optimization_unit_identity(&changed);
            assert!(
                optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
                "{access:?} cannot replace immutable owned copy-source authority"
            );
        }
        let mut unqualified = unit.clone();
        unqualified.functions[0].structural_parameters[source_parameter_index]
            .qualifications
            .clear();
        unqualified.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&unqualified);
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&unqualified).is_err(),
            "the capacity fact cannot supply the source's missing Utf8 declaration"
        );
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap_or_else(|error| {
            panic!("owned byte formal target lowering on {native:?}: {error:?}")
        });
        let parameter = &target.functions[0].graph.parameters[source_parameter_index];
        assert_eq!(parameter.access, StructuralAccess::Owned);
        assert_eq!(
            parameter.shape,
            calling_conventions::ValueShape::integer(u32::try_from(capacity + 8).unwrap(), 8)
        );
        let legalized =
            crate::legalize_target_operations(&target, &source, &unit).unwrap_or_else(|error| {
                panic!("owned byte formal legalization on {native:?}: {error:?}")
            });
        crate::validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
            .expect("independently replay the owned source and qualified destination");
        if owned_destination {
            for corruption in 0..3 {
                let mut changed = target.clone();
                let target_operations::TargetControlTerminator::ReturnStructural {
                    source: target_operations::TargetStructuralReturnSource::Home(home),
                    ..
                } = &mut changed.functions[0].graph.blocks[0].terminator
                else {
                    panic!("copied record returns its own home")
                };
                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    result,
                } = &mut home.origin
                else {
                    panic!("copied return has an operation origin")
                };
                match corruption {
                    0 => *operation = id(3),
                    1 => result.place = id(1),
                    2 => {
                        home.layout = target_operations::TargetStructuralHomeLayout::Aggregate(
                            calling_conventions::ValueShape::integer(8, 8),
                        )
                    }
                    _ => unreachable!(),
                }
                assert!(
                    crate::legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?}: forged copied-return home {corruption}"
                );
                assert!(
                    crate::validate_legalized_operations(
                        &changed,
                        &source,
                        &unit,
                        legalized.plan().clone()
                    )
                    .is_err(),
                    "{native:?}: receiving replay of forged copied-return home {corruption}"
                );
            }
            let mut changed = legalized.plan().clone();
            let copied = changed.scalar_functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|row| row.operation == id(4))
                .expect("copied return producer");
            let legalized_operations::LegalizedScalarInstructionKind::StructuralLeafCopy {
                shape,
                ..
            } = &mut copied.kind
            else {
                panic!("copied return")
            };
            *shape = calling_conventions::ValueShape::integer(8, 8);
            assert!(
                crate::validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "{native:?}: shortened copied-return payload"
            );
        }
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("owned byte formal selection on {native:?}: {error:?}"));
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .expect("replay live-length copying and owned backing geometry");

        if owned_destination {
            let slot = selected_instructions::LocalStorageSlotId::Structural {
                operation: id(3),
                place: id(1),
            };
            let home_index = selected.plan().functions[0]
                .local_storage_slots
                .iter()
                .position(|home| home.id == slot)
                .expect("record replacement uses its original completed home");
            for corruption in 0..3 {
                let mut changed = selected.plan().clone();
                let function = &mut changed.functions[0];
                match corruption {
                    0 => {
                        function.local_storage_slots.remove(home_index);
                    }
                    1 => function.local_storage_slots[home_index].byte_size -= 1,
                    2 => {
                        // The store may reuse the establishment's root pointer.
                        // Its own address step must still select inline bytes
                        // after the field's eight-byte live-length word.
                        let address = function
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.instructions)
                            .find(|instruction| {
                                instruction.provenance.operations.contains(&id(2))
                                    && matches!(instruction.kind,
                                    selected_instructions::SelectedInstructionKind::AddressOffset {
                                        byte_offset: 8,
                                    })
                            })
                            .expect("replacement addresses the owned field's inline bytes");
                        let selected_instructions::SelectedInstructionKind::AddressOffset {
                            byte_offset,
                        } = &mut address.kind
                        else {
                            unreachable!()
                        };
                        *byte_offset = 16;
                    }
                    _ => unreachable!(),
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed,
                    )
                    .is_err(),
                    "{native:?}: forged record destination home {corruption}"
                );
            }
        }

        let direct_fragments = parameter.placement.locations.iter().all(|location| {
            matches!(
                location,
                calling_conventions::ValueLocation::Register { .. }
            )
        });
        if direct_fragments {
            let slot =
                selected_instructions::LocalStorageSlotId::StructuralParameter { place: id(2) };
            let function = &selected.plan().functions[0];
            let home_index = function
                .local_storage_slots
                .iter()
                .position(|home| home.id == slot)
                .expect("register-carried owned bytes need their own addressable entry home");
            assert_eq!(
                function.local_storage_slots[home_index].byte_size,
                parameter.shape.byte_size
            );
            assert_eq!(
                function.local_storage_slots[home_index].alignment,
                parameter.shape.alignment
            );
            for corruption in 0..3 {
                let mut changed = selected.plan().clone();
                let function = &mut changed.functions[0];
                match corruption {
                    0 => {
                        function.local_storage_slots.remove(home_index);
                    }
                    1 => function.local_storage_slots[home_index].byte_size -= 1,
                    2 => {
                        let address = function.blocks.iter_mut().flat_map(|block| &mut block.instructions)
                            .find(|instruction| matches!(instruction.kind,
                                selected_instructions::SelectedInstructionKind::FrameAddress {
                                    slot: selected_instructions::FrameStorageSlotId::Local(actual), ..
                                } if actual == slot
                            )).expect("owned entry home address");
                        let selected_instructions::SelectedInstructionKind::FrameAddress {
                            byte_offset,
                            ..
                        } = &mut address.kind
                        else {
                            unreachable!()
                        };
                        *byte_offset = 8;
                    }
                    _ => unreachable!(),
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed,
                    )
                    .is_err(),
                    "{native:?} capacity {capacity}: forged owned entry home {corruption}"
                );
            }
        }

        let mut borrowed_signature = target.clone();
        borrowed_signature.functions[0].graph.parameters[source_parameter_index].access =
            StructuralAccess::SharedBorrow;
        assert!(
            crate::legalize_target_operations(&borrowed_signature, &source, &unit).is_err(),
            "an owned byte payload cannot be reinterpreted as a borrowed parameter"
        );
    }
}
