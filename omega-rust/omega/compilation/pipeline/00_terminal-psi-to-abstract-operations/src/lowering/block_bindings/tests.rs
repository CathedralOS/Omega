use super::{LoweringError, TerminalModule, Terminator, validate_structural_block_bindings};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ByteSequenceCarrier, MachineContract, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, VocabularyMarker,
};

fn fixture() -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let target = BlockId::new(2).unwrap();
    let source = PlaceId::new(1).unwrap();
    let destination = PlaceId::new(2).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let parameter = StructuralParameterDeclaration {
        place: source,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![parameter.clone()],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: source,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: destination,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: target,
                        position: 0,
                    },
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: entry,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        edge: EdgeId::new(1).unwrap(),
                        target,
                        arguments: Vec::new(),
                        structural_arguments: vec![StructuralArgument {
                            place: source,
                            path: Vec::new(),
                            access: StructuralAccess::SharedBorrow,
                        }],
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    id: target,
                    parameters: Vec::new(),
                    structural_parameters: vec![StructuralParameterDeclaration {
                        place: destination,
                        ..parameter
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(2).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

#[test]
fn bounded_owner_arrivals_replay_exact_access_source_and_qualifications() {
    use optimization_unit::{
        attach_verified_module_context, reconstruct_psi_optimization_unit_seed,
    };
    use optimization_unit_semantics::validate_psi_optimization_unit;
    use semantic_vocabulary::{DomainSemanticId, FuelScheduleIdentity, StructuralDomainId};

    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut module = fixture();
        module.structural_types[0].shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 16 });
        let domain = StructuralDomainId::new(1).unwrap();
        module
            .structural_domains
            .push(terminal_psi::StructuralDomainDeclaration {
                id: domain,
                semantic_domain: DomainSemanticId::new(1).unwrap(),
                identity: "test::Utf8".into(),
                carrier: module.structural_types[0].id,
                content_projection: None,
                byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
                establishment_routes: Vec::new(),
            });
        let machine = &mut module.machines[0];
        machine.structural_parameters[0].access = access;
        machine.structural_parameters[0].qualifications = vec![domain];
        machine.blocks[1].structural_parameters[0].access = access;
        machine.blocks[1].structural_parameters[0].qualifications = vec![domain];
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        structural_arguments[0].access = access;
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("the exact borrowed owner contract verifies at both arrivals");
        let plan = super::super::lower_decoded_verified_module(&verified).unwrap();
        let seed = |plan: &abstract_operations::AbstractOperationPlan| {
            let mut unit =
                reconstruct_psi_optimization_unit_seed(plan, FuelScheduleIdentity::new(1).unwrap())
                    .unwrap();
            attach_verified_module_context(&mut unit, &module).unwrap();
            unit
        };
        validate_psi_optimization_unit(&seed(&plan)).expect("owner address transfer replays");
        for corruption in ["source", "access", "qualification"] {
            let mut changed = plan.clone();
            if corruption == "qualification" {
                changed.functions[0].structural_parameters[0]
                    .qualifications
                    .clear();
            } else {
                let abstract_operations::AbstractOperation::Jump {
                    structural_bindings,
                    ..
                } = &mut changed.functions[0].operations[0]
                else {
                    unreachable!()
                };
                if corruption == "source" {
                    structural_bindings[0].argument.place = PlaceId::new(99).unwrap();
                } else {
                    structural_bindings[0].argument.access = StructuralAccess::WriteOnlyBorrow;
                }
            }
            // Reconstruct derived rows from the hostile current graph: stale
            // identities must not be the reason the receiving check rejects.
            let rejected = reconstruct_psi_optimization_unit_seed(
                &changed,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .map(|mut unit| {
                attach_verified_module_context(&mut unit, &module).unwrap();
                validate_psi_optimization_unit(&unit).is_err()
            })
            .unwrap_or(true);
            assert!(rejected, "{access:?}: {corruption}");
        }
    }
}

#[test]
fn shared_view_block_parameters_retain_their_declarations() {
    let mut module = fixture();
    assert_eq!(validate_structural_block_bindings(&module), Ok(()));
    module.machines[0].blocks[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(validate_structural_block_bindings(&module), Ok(()));
    module.machines[0].blocks[1].structural_parameters[0].access =
        StructuralAccess::WriteOnlyBorrow;
    module.machines[0].blocks.swap(0, 1);
    assert_eq!(
        validate_structural_block_bindings(&module),
        Err(LoweringError::UnsupportedStructuralBlockParameters {
            machine: module.entry,
            block: BlockId::new(2).unwrap(),
        })
    );
}

#[test]
fn empty_record_block_parameters_keep_their_borrowed_places() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut module = fixture();
        module.structural_types[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
        module.machines[0].structural_parameters[0].access = access;
        module.machines[0].blocks[1].structural_parameters[0].access = access;
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut module.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        structural_arguments[0].access = access;
        assert_eq!(validate_structural_block_bindings(&module), Ok(()));
        assert_ne!(
            module.machines[0].structural_parameters[0].place,
            module.machines[0].blocks[1].structural_parameters[0].place,
        );
        let rejected = Err(LoweringError::UnsupportedStructuralBlockParameters {
            machine: module.entry,
            block: BlockId::new(2).unwrap(),
        });
        let mut missing = module.clone();
        missing.structural_types.clear();
        assert_eq!(validate_structural_block_bindings(&missing), rejected);
        module.machines[0].blocks[1].structural_parameters[0].access =
            StructuralAccess::WriteOnlyBorrow;
        assert_eq!(validate_structural_block_bindings(&module), rejected);
    }
}

#[test]
fn each_conditional_arm_is_checked_even_without_byte_operations() {
    for selected_arm in 0..2 {
        let mut module = fixture();
        let successor = |ordinal| SuccessorEdge {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            edge: EdgeId::new(ordinal).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        let mut successors = [successor(3), successor(4)];
        successors[selected_arm]
            .structural_arguments
            .push(StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::WriteOnlyBorrow,
            });
        let expected_edge = successors[selected_arm].edge;
        let [when_true, when_false] = successors;
        module.machines[0].blocks[0].terminator = Terminator::Conditional {
            condition: ValueId::new(1).unwrap(),
            when_true,
            when_false,
        };
        // Deliberately inspect the consumer gate directly: neither a missing
        // condition value nor the target declaration can conceal an arm.
        assert_eq!(
            validate_structural_block_bindings(&module),
            Err(LoweringError::UnsupportedStructuralSuccessorArguments {
                machine: module.entry,
                edge: expected_edge,
            })
        );
    }
}

/// The same edge as an address join: the parameter borrows one `u64` leaf and
/// the argument names it through a static field path beneath its root.
fn address_join_fixture(path: Vec<terminal_psi::StructuralPathSegment>) -> TerminalModule {
    let mut module = fixture();
    let scalar = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    );
    module.structural_types.push(StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: "u64".into(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar),
    });
    module.structural_types.push(StructuralTypeDeclaration {
        id: StructuralTypeId::new(3).unwrap(),
        identity: "test::Payload".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "left".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
            }],
        },
    });
    module.machines[0].blocks[1].structural_parameters[0].structural_type =
        StructuralTypeId::new(2).unwrap();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].path = path;
    module
}

#[test]
fn shared_address_joins_admit_whole_and_projected_arguments() {
    let field = || vec![terminal_psi::StructuralPathSegment::Field("left".into())];
    assert_eq!(
        validate_structural_block_bindings(&address_join_fixture(Vec::new())),
        Ok(())
    );
    assert_eq!(
        validate_structural_block_bindings(&address_join_fixture(field())),
        Ok(())
    );
    let mut record = address_join_fixture(Vec::new());
    record.machines[0].blocks[1].structural_parameters[0].structural_type =
        StructuralTypeId::new(3).unwrap();
    assert_eq!(validate_structural_block_bindings(&record), Ok(()));
    let mut indexed = address_join_fixture(field());
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut indexed.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(terminal_psi::StructuralPathSegment::FixedIndex(0));
    assert_eq!(validate_structural_block_bindings(&indexed), Ok(()));
}

#[test]
fn mutable_address_joins_admit_projected_arguments_of_their_own_access() {
    let mut mutable = address_join_fixture(vec![terminal_psi::StructuralPathSegment::Field(
        "left".into(),
    )]);
    let parameter = &mut mutable.machines[0].blocks[1].structural_parameters[0];
    parameter.structural_type = StructuralTypeId::new(3).unwrap();
    parameter.access = StructuralAccess::MutableBorrow;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut mutable.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(validate_structural_block_bindings(&mutable), Ok(()));
    // A shared projection cannot arrive in the mutable join either.
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut mutable.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert_eq!(
        validate_structural_block_bindings(&mutable),
        Err(LoweringError::UnsupportedStructuralSuccessorArguments {
            machine: MachineId::new(1).unwrap(),
            edge: EdgeId::new(1).unwrap(),
        })
    );
}

#[test]
fn address_joins_reject_widened_access_windows_and_descriptor_projections() {
    let edge = EdgeId::new(1).unwrap();
    let rejected_edge = Err(LoweringError::UnsupportedStructuralSuccessorArguments {
        machine: MachineId::new(1).unwrap(),
        edge,
    });
    // A write-only arrival transports the same address without granting reads.
    let mut exclusive = address_join_fixture(Vec::new());
    exclusive.machines[0].blocks[1].structural_parameters[0].access =
        StructuralAccess::WriteOnlyBorrow;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut exclusive.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::WriteOnlyBorrow;
    assert_eq!(validate_structural_block_bindings(&exclusive), Ok(()));
    // A projected argument may not widen the join's shared access.
    let mut widened = address_join_fixture(vec![terminal_psi::StructuralPathSegment::Field(
        "left".into(),
    )]);
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut widened.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    assert_eq!(validate_structural_block_bindings(&widened), rejected_edge);
    // Byte windows and reference hops do not name one static address.
    for segment in [
        terminal_psi::StructuralPathSegment::FixedByteRange { start: 0, end: 1 },
        terminal_psi::StructuralPathSegment::Referent,
    ] {
        assert_eq!(
            validate_structural_block_bindings(&address_join_fixture(vec![segment])),
            rejected_edge
        );
    }
    // A descriptor view still binds whole roots only.
    let mut descriptor = fixture();
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut descriptor.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::Field("left".into())];
    assert_eq!(
        validate_structural_block_bindings(&descriptor),
        rejected_edge
    );
}
