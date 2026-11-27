use super::*;
use crate::descriptor_results::{
    DescriptorCallOrigin, DescriptorInvocation, DescriptorInvocationOwner,
    DescriptorInvocationTarget, DescriptorPayloadTarget,
    terminal_borrowed_storage_graph,
};
use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorContentsQuery, DescriptorContentsStep,
};
use crate::storage::borrowed_storage::Projection;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    ScalarQualificationSetId, StructuralTypeId,
};
use std::collections::BTreeSet;
use terminal_psi::{
    Block, ByteSequenceCarrier, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalIdentityIndex, TerminalMachineResult, ValueDeclaration,
};
fn machine(id: u64) -> TerminalMachine {
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: MachineId::new(id).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: terminal_psi::MachineContract {
            selected_application: None,
            declaration: None,
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            entry_claims: Vec::new(),
            declared_service_reach: None,
            closed_reach_application: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}

fn sequence_capture_module() -> TerminalModule {
    let bytes = StructuralTypeId::new(1).unwrap();
    let sequence = StructuralTypeId::new(2).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex::ABSENT,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    };
    let mut module = TerminalModule::declarations();
    module.types.structural_types = vec![
        declaration(
            bytes,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        ),
        declaration(
            sequence,
            StructuralTypeShape::ElementView {
                element: bytes,
                access: None,
            },
        ),
    ];
    let mut body = machine(1);
    for position in 0..2 {
        let place = PlaceId::new(position as u64 + 1).unwrap();
        body.parameter_order
            .push(terminal_psi::BoundaryParameterKind::Structural);
        body.structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                structural_type: bytes,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        body.structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter { position },
            });
    }
    for (operation, place) in [(1, 3), (2, 4), (3, 5)] {
        body.structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: PlaceId::new(place).unwrap(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(operation).unwrap(),
                    structural_type: sequence,
                },
            });
    }
    let argument = |place| {
        terminal_psi::StructuralArgument::whole(
            PlaceId::new(place).unwrap(),
            StructuralAccess::Owned,
        )
    };
    body.blocks[0].operations = vec![
        sequence_result(
            1,
            3,
            sequence,
            OperationKind::EstablishStructuralArray {
                elements: vec![argument(1), argument(2)],
            },
        ),
        sequence_result(
            2,
            4,
            sequence,
            OperationKind::StructuralLeafCopy {
                source: PlaceId::new(3).unwrap(),
                path: Vec::new(),
            },
        ),
        sequence_result(
            3,
            5,
            sequence,
            OperationKind::EstablishStructuralArray {
                elements: vec![argument(2), argument(2)],
            },
        ),
        Operation::unsuspended(
            OperationId::new(4).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition:
                    terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                destination: PlaceId::new(3).unwrap(),
                path: Vec::new(),
                field: None,
                value: argument(5),
            },
        ),
    ];
    module.machines = vec![body];
    module
}

fn sequence_result(
    id: u64,
    place: u64,
    ty: StructuralTypeId,
    kind: OperationKind,
) -> Operation {
    Operation::unsuspended(
        OperationId::new(id).unwrap(),
        OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: PlaceId::new(place).unwrap(),
            structural_type: ty,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        }),
        kind,
    )
}

fn reference_flow_module(record: bool) -> TerminalModule {
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let pair = StructuralTypeId::new(3).unwrap();
    let declaration = |id, copyable, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex::ABSENT,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: if copyable {
            terminal_psi::StructuralMultiplicity::Unrestricted
        } else {
            terminal_psi::StructuralMultiplicity::Affine
        },
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    };
    let field = |number: u32| terminal_psi::StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        id: semantic_vocabulary::StructuralFieldId::new(u64::from(number)).unwrap(),
        identity: TerminalIdentityIndex(number),
        inline_carrier: None,
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: terminal_psi::StructuralFieldType::Structural(reference),
        storage: None,
    };
    let mut module = TerminalModule::declarations();
    module.types.structural_types = vec![
        declaration(
            scalar,
            true,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        declaration(
            reference,
            true,
            StructuralTypeShape::Reference {
                referent: scalar,
                access: StructuralAccess::SharedBorrow,
            },
        ),
        declaration(
            pair,
            false,
            StructuralTypeShape::Record {
                fields: vec![field(1), field(2)],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ),
    ];
    let mut body = machine(1);
    let ty = if record { pair } else { reference };
    let source = PlaceId::new(1).unwrap();
    body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    body.structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            structural_type: ty,
            multiplicity: if record {
                StructuralMultiplicity::Affine
            } else {
                StructuralMultiplicity::Unrestricted
            },
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    body.structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        });
    for (operation, place) in [(1, 2), (2, 3)] {
        body.structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: PlaceId::new(place).unwrap(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(operation).unwrap(),
                    structural_type: ty,
                },
            });
    }
    module.machines = vec![body];
    module
}

#[test]
fn successor_diagnostics_keep_actual_operands_and_target_parameters() {
    use crate::descriptor_results::{
        DescriptorGraphError, terminal_reference_graph_from,
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

    let mut module = TerminalModule::declarations();
    let mut body = machine(1);
    let target = BlockId::new(2).unwrap();
    let actual = ValueId::new(1).unwrap();
    let edge = EdgeId::new(1).unwrap();
    body.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Scalar(ValueDeclaration {
            id: actual,
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        }),
        OperationKind::BooleanConstant { value: true },
    ));
    body.blocks[0].terminator = Terminator::Jump {
        edge,
        target,
        arguments: vec![actual],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![],
        cleanup_actions: vec![],
    };
    let mut arrival = Block::plain(
        target,
        vec![],
        Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: vec![],
        },
    );
    arrival.parameters.push(ValueDeclaration {
        id: ValueId::new(2).unwrap(),
        scalar_type: ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        ),
        qualifications: ScalarQualificationSetId::ZERO,
    });
    body.blocks.push(arrival);
    module.machines.push(body);
    let failure = terminal_reference_graph_from(&module, [MachineId::new(1).unwrap()])
        .err()
        .expect("a Boolean actual cannot initialize an integer parameter");
    assert_eq!(
        failure.detail(),
        Some("descriptor arrival scalar type differs from target parameter")
    );
    assert!(matches!(failure, DescriptorGraphError::IncompleteMachine {
        successor: Some((found, destination)), ..
    } if found == edge && destination == target));
    let detail = failure.in_module(&module);
    assert!(detail.contains("terminator Jump"), "{detail}");
    assert!(
        detail.contains("scalar actuals [(ValueId(1), Some(Boolean))]"),
        "{detail}"
    );
    assert!(
        detail.contains("ValueId(2)") && detail.contains("Integer"),
        "{detail}"
    );

    let Terminator::Jump { arguments, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    arguments.clear();
    let failure = terminal_reference_graph_from(&module, [MachineId::new(1).unwrap()])
        .err()
        .expect("missing actuals remain refused");
    assert_eq!(
        failure.detail(),
        Some("descriptor arrival scalar arity differs from target parameters")
    );
    let Terminator::Jump { arguments, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    arguments.push(actual);
    module.machines[0].blocks[1].parameters[0].scalar_type = ScalarType::Boolean;
    assert!(
        terminal_reference_graph_from(&module, [MachineId::new(1).unwrap()]).is_ok()
    );
}

#[test]
fn nonreturning_local_calls_retain_write_effects_and_actual_backing() {
    use crate::descriptor_results::{DescriptorAnalysis, DescriptorSourceOrigin};
    use semantic_vocabulary::{InvocationSite, ScalarType, ValueId};
    // An ordinary borrowed object has exact storage. An owned
    // pointer without its pointee supply must remain unknown.
    for indirect in [false, true] {
        let path = if indirect {
            vec![StructuralPathSegment::Referent]
        } else {
            vec![]
        };
        let mut module = reference_flow_module(false);
        let scalar = StructuralTypeId::new(1).unwrap();
        module.types.structural_types[1].shape = StructuralTypeShape::Reference {
            referent: scalar,
            access: StructuralAccess::MutableBorrow,
        };
        module.types.structural_types[1].multiplicity = StructuralMultiplicity::Affine;
        let actual = PlaceId::new(1).unwrap();
        let formal = PlaceId::new(7).unwrap();
        let mut caller = module.machines.remove(0);
        caller.structural_places.truncate(1);
        caller.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        if !indirect {
            caller.structural_parameters[0].structural_type = scalar;
            caller.structural_parameters[0].multiplicity =
                StructuralMultiplicity::Unrestricted;
        }
        let mut helper = machine(u64::MAX);
        helper.contract.id = ContractId::new(2).unwrap();
        helper.parameter_order = caller.parameter_order.clone();
        helper.structural_parameters = caller.structural_parameters.clone();
        helper.structural_parameters[0].place = formal;
        if !indirect {
            helper.structural_parameters[0].access = StructuralAccess::MutableBorrow;
        }
        helper.structural_places = caller.structural_places.clone();
        helper.structural_places[0].id = formal;
        helper.blocks[0].operations = vec![
            Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    qualifications: ScalarQualificationSetId::ZERO,
                }),
                OperationKind::BooleanConstant { value: true },
            ),
            Operation::unsuspended(
                OperationId::new(2).unwrap(),
                OperationResult::Unit,
                OperationKind::WriteOnlyPrimitiveStore {
                    destination: formal,
                    path: path.clone(),
                    value: ValueId::new(1).unwrap(),
                    range_obligation: None,
                },
            ),
        ];
        helper.blocks[0].terminator = Terminator::Crash {
            edge: EdgeId::new(1).unwrap(),
            cause: semantic_vocabulary::CrashCause::Trap,
            site_guard: vec![],
            frontier_lower_bound: vec![],
        };
        caller.blocks[0].operations.clear();
        caller.blocks[0].terminator = Terminator::InvokeNoNormal {
            edge: EdgeId::new(1).unwrap(),
            static_reach_binding: None,
            required_contract: None,
            callee: helper.id,
            arguments: vec![],
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            structural_arguments: vec![terminal_psi::StructuralArgument::whole(
                actual,
                if indirect {
                    StructuralAccess::Owned
                } else {
                    StructuralAccess::MutableBorrow
                },
            )],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        };
        module.machines = vec![caller, helper];
        let caller = module.machines[0].id;
        let helper = module.machines[1].id;
        module.machines.reverse();
        let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
            &module,
            [caller],
            &[],
        )
        .unwrap();
        assert_eq!(
            graph.iter().map(|row| row.id).collect::<Vec<_>>(),
            [helper, caller],
            "a sorted sparse lookup must preserve module output order"
        );
        let body = graph.iter().find(|row| row.id == caller).unwrap();
        let cut = (BlockId::new(1).unwrap(), 0);
        let call = body
            .known_invocation(&cut)
            .expect("the exact local terminal call has an account");
        assert_eq!(
            semantic_vocabulary::InvocationSite::from(call.owner),
            InvocationSite::InvokeNoNormal(EdgeId::new(1).unwrap())
        );
        assert_eq!(call.callee(), Some(helper));
        assert!(!body.unresolved_contents_writes.contains(&cut));
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
            &module.types.structural_types,
            &graph,
        );
        assert_eq!(
            analysis.storage_access_frame(caller).unwrap(),
            Some([].as_slice())
        );
        let accesses = analysis
            .storage_access_frame(helper)
            .unwrap()
            .unwrap()
            .to_vec();
        assert!(accesses.iter().any(|&input| matches!(
            analysis.storage_access(helper, input),
            Some((access_vocabulary::AccessOperation::Write, _))
        )));
        if indirect {
            assert!(
                analysis.storage_writes(caller, cut).unwrap().is_none(),
                "an unprovided pointee cannot become a known-empty write"
            );
            continue;
        }
        let written = analysis.storage_writes(caller, cut).unwrap().unwrap();
        assert!(
            written
                .iter()
                .any(|row| row.root == actual && row.path.is_empty()),
            "a write before Trap still changes the actual referent: {written:?}"
        );
        let origin = DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
            source: formal,
            projection: Projection::new(&path),
        });
        let actuals = analysis
            .substitute_call_source(caller, cut, &origin)
            .unwrap();
        assert_eq!(
            actuals,
            vec![DescriptorSourceOrigin::Backing(
                DescriptorCallOrigin::Argument {
                    source: actual,
                    projection: Projection::new(&path),
                }
            )]
        );
        let mut duplicate = module.clone();
        duplicate.machines.push(
            module
                .machines
                .iter()
                .find(|row| row.id == helper)
                .unwrap()
                .clone(),
        );
        assert!(matches!(
            crate::descriptor_results::terminal_borrowed_storage_graph_from(&duplicate, [caller], &[]),
            Err(crate::descriptor_results::DescriptorGraphError::MissingOrAmbiguousMachine(id)) if id == helper
        ));
        assert!(terminal_borrowed_storage_graph(&duplicate).is_empty());
        let mut missing = module.clone();
        missing.machines.retain(|row| row.id != helper);
        let missing_graph =
            crate::descriptor_results::terminal_borrowed_storage_graph_from(
                &missing,
                [caller],
                &[],
            )
            .unwrap();
        let mut unknown = DescriptorAnalysis::with_borrowed_storage_slots(
            &missing.types.structural_types,
            &missing_graph,
        );
        assert!(unknown.storage_access_frame(caller).unwrap().is_none());
        assert!(unknown.storage_writes(caller, cut).unwrap().is_none());
    }
}

#[test]
fn borrowing_a_carrier_slot_does_not_copy_its_existing_permission() {
    use crate::descriptor_lifetimes::{
        descriptor_entry_lifetime, descriptor_location_lifetime,
    };
    let mut module = reference_flow_module(false);
    let reference = StructuralTypeId::new(2).unwrap();
    let child_type = StructuralTypeId::new(3).unwrap();
    module.types.structural_types[2].shape = StructuralTypeShape::Reference {
        referent: reference,
        access: StructuralAccess::SharedBorrow,
    };
    module.types.structural_types[2].multiplicity =
        terminal_psi::StructuralMultiplicity::Unrestricted;
    let body = &mut module.machines[0];
    let source = PlaceId::new(1).unwrap();
    let child = PlaceId::new(2).unwrap();
    body.structural_places.pop();
    body.structural_places[1].kind = StructuralPlaceKind::OperationResult {
        producer: OperationId::new(1).unwrap(),
        structural_type: child_type,
    };
    let mut formation = sequence_result(
        1,
        2,
        child_type,
        OperationKind::EstablishReference {
            source: terminal_psi::StructuralArgument::whole(
                source,
                StructuralAccess::SharedBorrow,
            ),
            storage: None,
        },
    );
    let OperationResult::Structural(result) = &mut formation.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    body.blocks[0].operations = vec![formation];
    let identity = body.id;
    let graph =
        crate::descriptor_results::terminal_reference_graph_from(&module, [identity])
            .unwrap();
    let row = &graph[0];
    let after = (row.entry.0, 1);
    let parent = descriptor_entry_lifetime(
        &row.flow,
        row.entry,
        &DescriptorLocation::whole(source),
    );
    let child_life = descriptor_location_lifetime(
        &row.flow,
        row.entry,
        &DescriptorLocation::whole(child),
    );
    assert_eq!(
        parent.surviving[&after],
        BTreeSet::from([DescriptorLocation::whole(source)])
    );
    assert_eq!(
        child_life.surviving[&after],
        BTreeSet::from([DescriptorLocation::whole(child)])
    );
    assert!(row.flow[&row.entry].authority_derivations.is_empty());
    assert!(
        !row.flow[&row.entry].derivations.is_empty(),
        "the child still captures its real carrier storage"
    );
}

#[test]
fn projected_release_retires_only_its_reference_leaf() {
    use crate::descriptor_lifetimes::descriptor_location_lifetime;
    let mut module = reference_flow_module(true);
    let body = &mut module.machines[0];
    let source = PlaceId::new(1).unwrap();
    let carrier = PlaceId::new(2).unwrap();
    let ty = StructuralTypeId::new(3).unwrap();
    let mut moved = sequence_result(
        1,
        2,
        ty,
        OperationKind::MoveStructuralValue {
            source: terminal_psi::StructuralArgument::whole(
                source,
                StructuralAccess::Owned,
            ),
        },
    );
    let OperationResult::Structural(result) = &mut moved.result else {
        unreachable!();
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    body.structural_places.pop();
    body.blocks[0].operations = vec![
        moved,
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source: carrier,
                path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(1))],
            },
        ),
    ];
    let id = body.id;
    let graph = crate::descriptor_results::terminal_reference_graph_from(&module, [id])
        .unwrap();
    let row = &graph[0];
    let birth = (row.entry.0, 0);
    let release = (row.entry.0, 1);
    let after = (row.entry.0, 2);
    let leaf = |number| DescriptorLocation {
        root: carrier,
        path: vec![DescriptorPathSegment::Field(
            semantic_vocabulary::StructuralFieldId::new(number).unwrap(),
        )],
    };
    assert_eq!(
        row.flow[&release].observed_inputs().collect::<Vec<_>>(),
        vec![&leaf(1)]
    );
    assert_eq!(row.flow[&release].definitions, vec![leaf(1)]);
    let released = descriptor_location_lifetime(&row.flow, birth, &leaf(1));
    let sibling = descriptor_location_lifetime(&row.flow, birth, &leaf(2));
    assert!(!released.authorized[&after].contains(&leaf(1)));
    assert!(sibling.authorized[&after].contains(&leaf(2)));
}

#[test]
fn moving_a_carrier_retires_its_old_name_while_copying_keeps_the_source() {
    use crate::descriptor_lifetimes::descriptor_location_lifetime;
    let mut module = reference_flow_module(false);
    let body = &mut module.machines[0];
    let source = PlaceId::new(1).unwrap();
    let moved = PlaceId::new(2).unwrap();
    let copied = PlaceId::new(3).unwrap();
    let ty = StructuralTypeId::new(2).unwrap();
    body.blocks[0].operations = vec![
        sequence_result(
            1,
            2,
            ty,
            OperationKind::MoveStructuralValue {
                source: terminal_psi::StructuralArgument::whole(
                    source,
                    StructuralAccess::Owned,
                ),
            },
        ),
        sequence_result(
            2,
            3,
            ty,
            OperationKind::StructuralLeafCopy {
                source: moved,
                path: Vec::new(),
            },
        ),
    ];
    let id = body.id;
    let graph = crate::descriptor_results::terminal_reference_graph_from(&module, [id])
        .unwrap();
    let row = &graph[0];
    let moving = (row.entry.0, 0);
    let copying = (row.entry.0, 1);
    let after = (row.entry.0, 2);
    assert!(
        row.flow[&moving]
            .definitions
            .contains(&DescriptorLocation::whole(source))
    );
    assert!(
        !row.flow[&copying]
            .definitions
            .contains(&DescriptorLocation::whole(moved))
    );
    let lifetime = descriptor_location_lifetime(
        &row.flow,
        moving,
        &DescriptorLocation::whole(moved),
    );
    assert!(lifetime.authorized[&after].contains(&DescriptorLocation::whole(moved)));
    assert!(lifetime.authorized[&after].contains(&DescriptorLocation::whole(copied)));
    let names = crate::descriptors::descriptor_lifetimes::value_locations(
        &row.flow,
        moving,
        &DescriptorLocation::whole(source),
        true,
    );
    assert!(!names[&copying].contains(&DescriptorLocation::whole(source)));
    assert!(names[&copying].contains(&DescriptorLocation::whole(moved)));
}

#[test]
fn constructing_with_fresh_permission_uses_its_retained_custody() {
    let mut module = reference_flow_module(false);
    let body = &mut module.machines[0];
    let backing = PlaceId::new(1).unwrap();
    let source = PlaceId::new(2).unwrap();
    let carrier = PlaceId::new(3).unwrap();
    let sibling = PlaceId::new(4).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let pair = StructuralTypeId::new(3).unwrap();
    let mut parameter = body.structural_parameters[0].clone();
    parameter.place = sibling;
    parameter.position = 1;
    body.structural_parameters.push(parameter);
    body.parameter_order
        .push(terminal_psi::BoundaryParameterKind::Structural);
    body.structural_parameters[0].structural_type = scalar;
    body.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    body.structural_places[2].kind = StructuralPlaceKind::OperationResult {
        producer: OperationId::new(2).unwrap(),
        structural_type: pair,
    };
    body.structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: sibling,
            kind: StructuralPlaceKind::Parameter { position: 1 },
        });
    let field = |number, place| terminal_psi::RecordFieldInitializer {
        field: semantic_vocabulary::StructuralFieldId::new(number).unwrap(),
        value: RecordFieldValue::Structural(terminal_psi::StructuralArgument::whole(
            place,
            StructuralAccess::Owned,
        )),
    };
    body.blocks[0].operations = vec![
        sequence_result(
            1,
            2,
            reference,
            OperationKind::EstablishReference {
                source: terminal_psi::StructuralArgument::whole(
                    backing,
                    StructuralAccess::SharedBorrow,
                ),
                storage: None,
            },
        ),
        sequence_result(
            2,
            3,
            pair,
            OperationKind::EstablishRecord {
                fields: vec![field(1, source), field(2, sibling)],
            },
        ),
    ];
    for operation in &mut body.blocks[0].operations {
        let OperationResult::Structural(result) = &mut operation.result else {
            unreachable!();
        };
        result.multiplicity = StructuralMultiplicity::Affine;
    }
    let id = body.id;
    let graph = crate::descriptor_results::terminal_reference_graph_from(&module, [id])
        .unwrap();
    let row = &graph[0];
    let constructor = (row.entry.0, 1);
    let after = (row.entry.0, 2);
    let source = DescriptorLocation::whole(source);
    assert!(row.flow[&constructor].definitions.contains(&source));
    assert!(
        !row.flow[&constructor]
            .definitions
            .contains(&DescriptorLocation::whole(sibling))
    );
    let mut analysis =
        crate::descriptor_results::DescriptorAnalysis::with_reference_slots(
            &module.types.structural_types,
            &graph,
        );
    let original = analysis.symbolic_source_origins(id, constructor, &source);
    assert!(original.is_some());
    let stored = DescriptorLocation {
        root: carrier,
        path: vec![DescriptorPathSegment::Field(
            semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
        )],
    };
    assert_eq!(
        analysis.symbolic_source_origins(id, after, &stored),
        original,
        "the destination reads the origin before source consumption",
    );
    assert!(
        analysis
            .symbolic_source_origins(id, after, &source)
            .is_none()
    );
    assert!(
        analysis
            .symbolic_source_origins(id, after, &DescriptorLocation::whole(sibling))
            .is_some()
    );
}

// Replacing a descriptor cannot retarget a previously copied
// sequence. Its two elements keep distinct sources at construction.
#[test]
fn stored_sequence_ordinals_keep_the_completed_constructor_cut() {
    use crate::descriptors::replay::engine::{Origin, Replay};
    let module = sequence_capture_module();
    let id = module.machines[0].id;
    let block = module.machines[0].entry;
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &module,
        [id],
        &[],
    )
    .unwrap();
    let row = &graph[0];
    let mut replay = Replay::with_slot_kind(
        &module.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(id, 0).unwrap();
    let current = states.get(&(block, 4)).unwrap();
    let origin = |source| {
        Origin::Backing(DescriptorCallOrigin::Argument {
            source: PlaceId::new(source).unwrap(),
            projection: Projection::new(&[]),
        })
    };
    let query = |root| DescriptorContentsQuery {
        root: PlaceId::new(root).unwrap(),
        steps: vec![DescriptorContentsStep::any_element(
            StructuralTypeId::new(1).unwrap(),
        )],
    };
    assert_eq!(
        replay.read_contents(row, current, &query(4)),
        Some(BTreeSet::from([origin(1), origin(2)]))
    );
    assert_eq!(
        replay.read_contents(row, current, &query(3)),
        Some(BTreeSet::from([origin(2)]))
    );
    let targets = replay.contents_targets(row, current, &query(4)).unwrap();
    assert_eq!(
        targets,
        (0..2)
            .map(|index| DescriptorPayloadTarget::Storage {
                location: DescriptorLocation {
                    root: PlaceId::new(3).unwrap(),
                    path: vec![DescriptorPathSegment::FixedIndex(index)]
                },
                completed: Some((id, (block, 1))),
            })
            .collect()
    );
    for (index, source) in [(0, 1), (1, 2)] {
        let ordinal = DescriptorContentsQuery {
            root: PlaceId::new(3).unwrap(),
            steps: vec![DescriptorContentsStep::Address(
                DescriptorPathSegment::FixedIndex(index),
            )],
        };
        assert_eq!(
            replay.read_contents(row, current, &ordinal),
            Some(BTreeSet::from([origin(source)]))
        );
    }
    let outside = DescriptorContentsQuery {
        root: PlaceId::new(3).unwrap(),
        steps: vec![DescriptorContentsStep::Address(
            DescriptorPathSegment::FixedIndex(2),
        )],
    };
    assert!(replay.read_contents(row, current, &outside).is_none());
    assert!(
        crate::descriptor_lifetimes::descriptor_location(
            &module.types.structural_types,
            PlaceId::new(3).unwrap(),
            StructuralTypeId::new(2).unwrap(),
            &[StructuralPathSegment::FixedIndex(0)],
            None,
        )
        .is_none(),
        "an internal constructor ordinal is not a proved source index"
    );
    let mut replacements = BTreeMap::new();
    assert!(
        replay
            .contents_replacements(
                row,
                current,
                &query(4),
                Some(BTreeSet::from([origin(2)])),
                &mut replacements
            )
            .is_none(),
        "captured contents do not grant a live write"
    );

    // Equal machine/block/instruction numbers in another immutable
    // owner cannot supply this constructor's history.
    let mut foreign = sequence_capture_module();
    let OperationKind::EstablishStructuralArray { elements } =
        &mut foreign.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    elements.swap(0, 1);
    let foreign_graph =
        crate::descriptor_results::terminal_borrowed_storage_graph_from(
            &foreign,
            [id],
            &[],
        )
        .unwrap();
    let mut other = Replay::with_slot_kind(
        &foreign.types.structural_types,
        &foreign_graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let other_states = other.states(id, 0).unwrap();
    let wrong = crate::descriptors::replay::engine::DescriptorReadDepth::at(
        &foreign_graph[0],
        current,
        Some(&other_states),
    );
    assert!(
        replay
            .read_contents_with_depth(row, current, &query(4), &wrong)
            .is_none()
    );

    // Local owned construction is not permission to publish a
    // result borrowing that local backing without its public join.
    assert!(
        replay
            .exported(
                row,
                &Origin::Storage(DescriptorLocation::whole(PlaceId::new(3).unwrap()))
            )
            .is_none()
    );
}

// Capture retains the pointer value; following that pointer still
// observes current contents under the already-held mutable loan.
#[test]
fn stored_sequence_reference_capture_keeps_the_referent_live() {
    use crate::descriptors::replay::engine::{Origin, Replay};
    let mut module = sequence_capture_module();
    let bytes = StructuralTypeId::new(1).unwrap();
    let sequence = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let mut pointer = module.types.structural_types[0].clone();
    pointer.id = reference;
    pointer.shape = StructuralTypeShape::Reference {
        referent: bytes,
        access: StructuralAccess::MutableBorrow,
    };
    module.types.structural_types.push(pointer);
    module.types.structural_types[1].shape = StructuralTypeShape::ElementView {
        element: reference,
        access: None,
    };
    let body = &mut module.machines[0];
    body.structural_parameters[0].structural_type = reference;
    body.structural_places.retain(|place| place.id.get() <= 3);
    body.blocks[0].operations = vec![
        sequence_result(
            1,
            3,
            sequence,
            OperationKind::EstablishStructuralArray {
                elements: vec![terminal_psi::StructuralArgument::whole(
                    PlaceId::new(1).unwrap(),
                    StructuralAccess::Owned,
                )],
            },
        ),
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition:
                    terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                destination: PlaceId::new(1).unwrap(),
                path: vec![StructuralPathSegment::Referent],
                field: None,
                value: terminal_psi::StructuralArgument::whole(
                    PlaceId::new(2).unwrap(),
                    StructuralAccess::Owned,
                ),
            },
        ),
    ];
    let id = body.id;
    let block = body.entry;
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &module,
        [id],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &module.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(id, 0).unwrap();
    let current = states.get(&(block, 2)).unwrap();
    let pointer = DescriptorContentsQuery {
        root: PlaceId::new(3).unwrap(),
        steps: vec![DescriptorContentsStep::any_element(reference)],
    };
    assert_eq!(
        replay.read_contents(&graph[0], current, &pointer),
        Some(BTreeSet::from([Origin::Incoming(
            DescriptorLocation::whole(PlaceId::new(1).unwrap())
        )]))
    );
    let mut pointee = pointer;
    pointee.steps.push(DescriptorContentsStep::Address(
        DescriptorPathSegment::Referent,
    ));
    let direct_query = DescriptorContentsQuery {
        root: PlaceId::new(1).unwrap(),
        steps: vec![DescriptorContentsStep::Address(
            DescriptorPathSegment::Referent,
        )],
    };
    let replacement = Origin::Backing(DescriptorCallOrigin::Argument {
        source: PlaceId::new(2).unwrap(),
        projection: Projection::new(&[]),
    });
    let direct = replay.read_contents(&graph[0], current, &direct_query);
    let through_sequence = replay.read_contents(&graph[0], current, &pointee);
    let constructor = replay.read_contents(
        &graph[0],
        states.get(&(block, 1)).unwrap(),
        &direct_query,
    );
    assert_eq!(
        direct,
        Some(BTreeSet::from([replacement.clone()])),
        "the existing direct read after the write"
    );
    assert_ne!(
        direct, constructor,
        "the write changed the live pointee after construction"
    );
    assert_eq!(
        through_sequence, direct,
        "a captured pointer must use the same live read, not its old pointee snapshot"
    );
    assert!(
        replay
            .contents_targets(&graph[0], current, &pointee)
            .unwrap()
            .iter()
            .all(|target| !matches!(
                target,
                DescriptorPayloadTarget::Storage {
                    completed: Some(_),
                    ..
                }
            )),
        "the live referent is not frozen at the pointer's capture cut"
    );

    let mut redirected = module.clone();
    let body = &mut redirected.machines[0];
    for (number, ty) in [(4, reference), (5, bytes)] {
        let position = u32::try_from(body.structural_parameters.len()).unwrap();
        let place = PlaceId::new(number).unwrap();
        body.parameter_order
            .push(terminal_psi::BoundaryParameterKind::Structural);
        body.structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                structural_type: ty,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        body.structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter { position },
            });
    }
    for (operation, path, value) in [
        (3, Vec::new(), 4),
        (4, vec![StructuralPathSegment::Referent], 5),
    ] {
        body.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(operation).unwrap(),
            OperationResult::Unit,
            OperationKind::StoreStructuralField {
                disposition:
                    terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                destination: PlaceId::new(1).unwrap(),
                path,
                field: None,
                value: terminal_psi::StructuralArgument::whole(
                    PlaceId::new(value).unwrap(),
                    StructuralAccess::Owned,
                ),
            },
        ));
    }
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &redirected,
        [id],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &redirected.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(id, 0).unwrap();
    let current = states.get(&(block, 4)).unwrap();
    let next = Origin::Backing(DescriptorCallOrigin::Argument {
        source: PlaceId::new(5).unwrap(),
        projection: Projection::new(&[]),
    });
    let direct = replay
        .read_contents(&graph[0], current, &direct_query)
        .unwrap();
    let retained = replay.read_contents(&graph[0], current, &pointee).unwrap();
    assert!(
        direct.contains(&next),
        "the current variable now names the replacement pointer's referent"
    );
    assert!(
        retained.contains(&replacement),
        "the sequence keeps the first pointer and its live referent"
    );
    assert!(
        !retained.contains(&next),
        "a current-root shortcut must not retarget the captured pointer"
    );
    assert_ne!(retained, direct);
}

// A call reads the actual sequence in its caller's cut. Equal
// block and operation numbers in the callee do not name that cut.
#[test]
fn stored_sequence_call_inputs_keep_the_callers_capture() {
    use crate::descriptors::replay::engine::{Origin, Replay};
    let mut module = sequence_capture_module();
    let bytes = StructuralTypeId::new(1).unwrap();
    let sequence = StructuralTypeId::new(2).unwrap();
    let reader_id = MachineId::new(2).unwrap();
    module.machines[0].structural_places.push(
        terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(6).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(5).unwrap(),
                structural_type: bytes,
            },
        },
    );
    module.machines[0].blocks[0]
        .operations
        .push(sequence_result(
            5,
            6,
            bytes,
            OperationKind::CallStructural {
                callee: reader_id,
                structural_arguments: vec![terminal_psi::StructuralArgument::whole(
                    PlaceId::new(4).unwrap(),
                    StructuralAccess::Owned,
                )],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            },
        ));
    let mut reader = machine(2);
    reader.contract.id = ContractId::new(2).unwrap();
    reader.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    reader.structural_parameters = vec![StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        structural_type: sequence,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }];
    reader.structural_places = vec![
        terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::Parameter { position: 0 },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(3).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(3).unwrap(),
                structural_type: bytes,
            },
        },
    ];
    let integer = semantic_vocabulary::IntegerType::new(
        semantic_vocabulary::IntegerSign::Unsigned,
        64,
    )
    .unwrap();
    let scalar = |id, value| {
        Operation::unsuspended(
            OperationId::new(id).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(id).unwrap(),
                scalar_type: ScalarType::Integer(integer),
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
            },
        )
    };
    reader.blocks[0].operations = vec![
        scalar(1, 0),
        scalar(2, 2),
        sequence_result(
            3,
            3,
            bytes,
            OperationKind::ElementViewRead {
                source: terminal_psi::StructuralArgument::whole(
                    PlaceId::new(1).unwrap(),
                    StructuralAccess::SharedBorrow,
                ),
                index: ValueId::new(1).unwrap(),
                length: ValueId::new(2).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                path: Vec::new(),
            },
        ),
    ];
    reader.result =
        TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(3).unwrap(),
            structural_type: bytes,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        });
    reader.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(3).unwrap(),
        returned_claims: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    module.machines.push(reader);
    let caller = module.machines[0].id;
    let block = module.machines[0].entry;
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &module,
        [caller],
        &[],
    )
    .unwrap();
    let mut duplicate_module = module.clone();
    let duplicate = &mut duplicate_module.machines[0];
    let duplicate_block = &mut duplicate.blocks[0];
    let (ordinal, constructor) = duplicate_block
        .operations
        .iter_mut()
        .enumerate()
        .find(|(_, operation)| {
            matches!(
                operation.kind,
                OperationKind::EstablishStructuralArray { .. }
            )
        })
        .unwrap();
    let OperationKind::EstablishStructuralArray { elements } = &mut constructor.kind
    else {
        unreachable!()
    };
    elements.push(elements[0].clone());
    let expected: Vec<_> = elements.iter().map(|element| element.place).collect();
    let constructor_position = (duplicate_block.id, u32::try_from(ordinal).unwrap());
    let duplicates =
        crate::descriptor_results::terminal_borrowed_storage_graph(&duplicate_module);
    let duplicate = duplicates.iter().find(|row| row.id == caller).unwrap();
    assert_eq!(
        duplicate.flow[&constructor_position]
            .inputs
            .iter()
            .map(|(input, _)| input.as_ref().unwrap().root)
            .collect::<Vec<_>>(),
        expected,
        "captured constructor operands preserve order and duplicate elements"
    );
    let mut replay = Replay::with_slot_kind(
        &module.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(caller, 0).unwrap();
    let row = graph.iter().find(|row| row.id == caller).unwrap();
    // This owner conservatively tracks every possible indexed
    // origin; the separate bounds owner still owes its proof.
    let expected = (1..=2)
        .map(|source| {
            Origin::Backing(DescriptorCallOrigin::Argument {
                source: PlaceId::new(source).unwrap(),
                projection: Projection::new(&[]),
            })
        })
        .collect();
    assert_eq!(
        replay.read(
            row,
            states.get(&(block, 5)).unwrap(),
            &DescriptorLocation::whole(PlaceId::new(6).unwrap())
        ),
        Some(expected)
    );

    // A local owned allocation still cannot be substituted for a
    // public owned-result contract or an escaping borrowed source.
    let mut escaping = sequence_capture_module();
    let body = &mut escaping.machines[0];
    body.result =
        TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(4).unwrap(),
            structural_type: sequence,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        });
    body.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(4).unwrap(),
        returned_claims: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &escaping,
        [caller],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &escaping.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    assert!(replay.summary(caller, 0).is_none());

    // Moving the roster carries its own storage, while each
    // ordinal must retain the distinct caller view it contains.
    let mut moved = sequence_capture_module();
    moved.types.structural_types[1].multiplicity =
        terminal_psi::StructuralMultiplicity::Affine;
    let body = &mut moved.machines[0];
    body.blocks[0].operations.truncate(1);
    body.structural_places.retain(|place| place.id.get() <= 3);
    body.result =
        TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(3).unwrap(),
            structural_type: sequence,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        });
    body.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: PlaceId::new(3).unwrap(),
        returned_claims: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &moved,
        [caller],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &moved.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let result = replay
        .summary(caller, 0)
        .and_then(|summary| summary.result.slots_without_contents().cloned())
        .unwrap();
    assert_eq!(result.len(), 1);
    let [
        crate::descriptors::replay::DescriptorStoredOrigin::Carried { element, elements },
    ] = result[0].origins.as_slice()
    else {
        panic!("the returned roster needs its complete element account");
    };
    assert_eq!(*element, bytes);
    assert_eq!(elements.len(), 2);
    for (ordinal, slots) in elements.iter().enumerate() {
        assert_eq!(slots.len(), 1);
        assert!(slots[0].path.is_empty());
        assert!(matches!(slots[0].origins.as_slice(),
            [crate::descriptors::replay::DescriptorStoredOrigin::Parameter { position, projection }]
            if *position == ordinal && *projection == Projection::new(&[])));
    }

    // The caller has the same numeric block/instruction/place
    // coordinates as the constructor. Read its rebased account,
    // then move the result and reuse its old descriptor home.
    let mut returned = moved.clone();
    let mut receiver = sequence_capture_module().machines.remove(0);
    receiver.id = MachineId::new(2).unwrap();
    receiver.contract.id = ContractId::new(2).unwrap();
    receiver.blocks[0].operations[0].kind = OperationKind::CallStructural {
        callee: caller,
        structural_arguments: (1..=2)
            .map(|source| {
                terminal_psi::StructuralArgument::whole(
                    PlaceId::new(source).unwrap(),
                    StructuralAccess::Owned,
                )
            })
            .collect(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        selected_evidence: Vec::new(),
    };
    receiver.blocks[0].operations[1].kind = OperationKind::MoveStructuralField {
        source: PlaceId::new(3).unwrap(),
        path: Vec::new(),
        field: None,
    };
    let OperationKind::StoreStructuralField { disposition, .. } =
        &mut receiver.blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *disposition = terminal_psi::StructuralStoreDisposition::Vacant;
    let receiver_id = receiver.id;
    let receiver_entry = receiver.entry;
    returned.machines.push(receiver);
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &returned,
        [receiver_id],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &returned.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(receiver_id, 0).unwrap();
    let receiver = graph.iter().find(|row| row.id == receiver_id).unwrap();
    let current = states.get(&(receiver_entry, 4)).unwrap();
    for (ordinal, source) in [(0, 1), (1, 2)] {
        let query = DescriptorContentsQuery {
            root: PlaceId::new(4).unwrap(),
            steps: vec![DescriptorContentsStep::Address(
                DescriptorPathSegment::FixedIndex(ordinal),
            )],
        };
        assert_eq!(
            replay.read_contents(receiver, current, &query),
            Some(BTreeSet::from([Origin::Backing(
                DescriptorCallOrigin::Argument {
                    source: PlaceId::new(source).unwrap(),
                    projection: Projection::new(&[]),
                }
            )]))
        );
    }
    let query = DescriptorContentsQuery {
        root: PlaceId::new(3).unwrap(),
        steps: vec![DescriptorContentsStep::any_element(bytes)],
    };
    let after_call = states.get(&(receiver_entry, 1)).unwrap();
    let constructor = graph.iter().find(|row| row.id == caller).unwrap();
    let constructor_states = replay.states(caller, 0).unwrap();
    let wrong = crate::descriptors::replay::engine::DescriptorReadDepth::at(
        constructor,
        after_call,
        Some(&constructor_states),
    );
    assert!(
        replay
            .read_contents_with_depth(receiver, after_call, &query, &wrong)
            .is_none(),
        "callee instruction numbers cannot supply caller capture history"
    );

    // Packing a borrowed view into an owned outer roster does
    // not transfer ownership of the local storage it points at.
    let mut borrowed_leaf = moved.clone();
    let borrowed = StructuralTypeId::new(3).unwrap();
    let outer = StructuralTypeId::new(4).unwrap();
    let mut declaration = borrowed_leaf.types.structural_types[1].clone();
    declaration.id = borrowed;
    declaration.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    declaration.shape = StructuralTypeShape::ElementView {
        element: bytes,
        access: Some(StructuralAccess::SharedBorrow),
    };
    borrowed_leaf.types.structural_types.push(declaration);
    let mut declaration = borrowed_leaf.types.structural_types[1].clone();
    declaration.id = outer;
    declaration.shape = StructuralTypeShape::ElementView {
        element: borrowed,
        access: None,
    };
    borrowed_leaf.types.structural_types.push(declaration);
    let argument = |place, access| {
        terminal_psi::StructuralArgument::whole(PlaceId::new(place).unwrap(), access)
    };
    let body = &mut borrowed_leaf.machines[0];
    for (operation, place, ty) in [(2, 4, borrowed), (3, 5, outer)] {
        body.structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: PlaceId::new(place).unwrap(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(operation).unwrap(),
                    structural_type: ty,
                },
            });
    }
    body.blocks[0].operations.extend([
        sequence_result(
            2,
            4,
            borrowed,
            OperationKind::EstablishElementView {
                destination: PlaceId::new(4).unwrap(),
                source: argument(3, StructuralAccess::SharedBorrow),
                element: bytes,
                region: terminal_psi::ElementViewRegion::WholeCollection,
            },
        ),
        sequence_result(
            3,
            5,
            outer,
            OperationKind::CallStructural {
                callee: reader_id,
                structural_arguments: vec![argument(4, StructuralAccess::Owned)],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            },
        ),
    ]);
    let TerminalMachineResult::Structural(result) = &mut body.result else {
        unreachable!()
    };
    result.place = PlaceId::new(5).unwrap();
    result.structural_type = outer;
    let Terminator::ReturnStructural { source, .. } = &mut body.blocks[0].terminator
    else {
        unreachable!()
    };
    *source = PlaceId::new(5).unwrap();
    let mut pack = moved.machines[0].clone();
    pack.id = reader_id;
    pack.contract.id = ContractId::new(2).unwrap();
    pack.parameter_order.truncate(1);
    pack.structural_parameters.truncate(1);
    pack.structural_parameters[0].structural_type = borrowed;
    pack.structural_places.retain(|row| row.id.get() != 2);
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut pack.structural_places[1].kind
    else {
        unreachable!()
    };
    *structural_type = outer;
    pack.blocks[0].operations = vec![sequence_result(
        1,
        3,
        outer,
        OperationKind::EstablishStructuralArray {
            elements: vec![argument(1, StructuralAccess::Owned)],
        },
    )];
    let TerminalMachineResult::Structural(result) = &mut pack.result else {
        unreachable!()
    };
    result.structural_type = outer;
    borrowed_leaf.machines.push(pack);
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &borrowed_leaf,
        [caller],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &borrowed_leaf.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    assert!(
        replay
            .summary(reader_id, 0)
            .and_then(|summary| summary.result.slots_without_contents().cloned())
            .is_some(),
        "the helper can carry a view borrowed from its caller"
    );
    assert!(
        replay.summary(caller, 0).is_none(),
        "re-export must not turn a borrowed local leaf into owned storage"
    );

    // Each return keeps its own extent. A shorter literal roster
    // must merge without claiming the longer one's ordinal.
    let mut alternatives = moved.clone();
    let body = &mut alternatives.machines[0];
    body.parameters = vec![ValueDeclaration {
        id: semantic_vocabulary::ValueId::new(1).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::ZERO,
    }];
    body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Scalar];
    body.structural_parameters.clear();
    for row in &mut body.structural_places {
        row.kind = match row.id.get() {
            1 | 2 => StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: (row.id.get() - 1) as u32,
                structural_type: bytes,
            },
            3 => StructuralPlaceKind::OperationResult {
                producer: OperationId::new(3).unwrap(),
                structural_type: sequence,
            },
            _ => unreachable!(),
        };
    }
    body.structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: PlaceId::new(5).unwrap(),
            kind: StructuralPlaceKind::OperationResult {
                producer: OperationId::new(4).unwrap(),
                structural_type: sequence,
            },
        });
    let mut long = body.blocks[0].clone();
    long.id = BlockId::new(2).unwrap();
    long.operations[0].id = OperationId::new(3).unwrap();
    let mut short = long.clone();
    short.id = BlockId::new(3).unwrap();
    short.operations = vec![sequence_result(
        4,
        5,
        sequence,
        OperationKind::EstablishStructuralArray {
            elements: vec![terminal_psi::StructuralArgument::whole(
                PlaceId::new(1).unwrap(),
                StructuralAccess::Owned,
            )],
        },
    )];
    short.terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(4).unwrap(),
        source: PlaceId::new(5).unwrap(),
        returned_claims: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let edge = |number, target| SuccessorEdge {
        edge: EdgeId::new(number).unwrap(),
        target: BlockId::new(target).unwrap(),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    body.blocks[0].operations = (1..=2)
        .map(|number| {
            Operation::unsuspended(
                OperationId::new(number).unwrap(),
                OperationResult::Unit,
                OperationKind::EstablishByteSequenceLiteral {
                    destination: PlaceId::new(number).unwrap(),
                    bytes: vec![number as u8],
                    qualifications: Vec::new(),
                },
            )
        })
        .collect();
    body.blocks[0].terminator = Terminator::Conditional {
        condition: body.parameters[0].id,
        when_true: edge(2, 2),
        when_false: edge(3, 3),
    };
    body.blocks.extend([long, short]);
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &alternatives,
        [caller],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &alternatives.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let result = replay
        .summary(caller, 0)
        .and_then(|summary| summary.result.slots_without_contents().cloned())
        .unwrap();
    let mut lengths = result[0]
        .origins
        .iter()
        .map(|origin| {
            let crate::descriptors::replay::DescriptorStoredOrigin::Carried {
                elements,
                ..
            } = origin
            else {
                panic!("each alternative must retain its actual roster");
            };
            assert!(
                elements
                    .iter()
                    .flatten()
                    .all(|slot| slot.origins.iter().all(|origin| matches!(
                        origin,
                        crate::descriptors::replay::DescriptorStoredOrigin::Literal { .. }
                    )))
            );
            elements.len()
        })
        .collect::<Vec<_>>();
    lengths.sort_unstable();
    assert_eq!(lengths, [1, 2]);

    let mut receiver = machine(2);
    receiver.contract.id = ContractId::new(2).unwrap();
    receiver.parameters = alternatives.machines[0].parameters.clone();
    receiver.parameter_order = vec![terminal_psi::BoundaryParameterKind::Scalar];
    receiver.structural_places = vec![terminal_psi::StructuralPlaceDeclaration {
        id: PlaceId::new(3).unwrap(),
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(1).unwrap(),
            structural_type: sequence,
        },
    }];
    receiver.blocks[0].operations = vec![sequence_result(
        1,
        3,
        sequence,
        OperationKind::CallStructuralWithScalarArguments {
            callee: caller,
            arguments: vec![receiver.parameters[0].id],
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    )];
    let receiver_id = receiver.id;
    let receiver_entry = receiver.entry;
    alternatives.machines.push(receiver);
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &alternatives,
        [receiver_id],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &alternatives.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(receiver_id, 0).unwrap();
    let receiver = graph.iter().find(|row| row.id == receiver_id).unwrap();
    let current = states.get(&(receiver_entry, 1)).unwrap();
    let query = DescriptorContentsQuery {
        root: PlaceId::new(3).unwrap(),
        steps: vec![DescriptorContentsStep::any_element(bytes)],
    };
    assert_eq!(
        replay.read_contents(receiver, current, &query),
        Some(
            (1..=2)
                .map(|source| Origin::Backing(DescriptorCallOrigin::Literal {
                    machine: caller,
                    place: PlaceId::new(source).unwrap(),
                }))
                .collect()
        ),
        "whole-roster enumeration retains both return alternatives"
    );
    let query = DescriptorContentsQuery {
        root: PlaceId::new(3).unwrap(),
        steps: vec![DescriptorContentsStep::Address(
            DescriptorPathSegment::FixedIndex(1),
        )],
    };
    assert!(
        replay.read_contents(receiver, current, &query).is_none(),
        "the shorter return cannot supply ordinal one"
    );

    // Follow the contents of each nested roster, including the
    // longer return's second element. A plain enumeration above
    // can read an installed slot without traversing its account.
    let mut nested = alternatives.clone();
    let outer = StructuralTypeId::new(3).unwrap();
    let mut declaration = nested.types.structural_types[1].clone();
    declaration.id = outer;
    declaration.shape = StructuralTypeShape::ElementView {
        element: sequence,
        access: None,
    };
    nested.types.structural_types.push(declaration);
    let argument = |place| {
        terminal_psi::StructuralArgument::whole(
            PlaceId::new(place).unwrap(),
            StructuralAccess::Owned,
        )
    };
    let body = &mut nested.machines[0];
    for (operation, place) in [(3, 6), (4, 7)] {
        body.structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: PlaceId::new(place).unwrap(),
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(operation).unwrap(),
                    structural_type: sequence,
                },
            });
        body.blocks[0].operations.push(sequence_result(
            operation,
            place,
            sequence,
            OperationKind::EstablishStructuralArray {
                elements: vec![argument(place - 5)],
            },
        ));
    }
    for (block_index, operation, place, inputs) in [
        (1, 5, 3, vec![argument(6), argument(7)]),
        (2, 6, 5, vec![argument(6)]),
    ] {
        body.structural_places
            .iter_mut()
            .find(|row| row.id == PlaceId::new(place).unwrap())
            .unwrap()
            .kind = StructuralPlaceKind::OperationResult {
            producer: OperationId::new(operation).unwrap(),
            structural_type: outer,
        };
        body.blocks[block_index].operations = vec![sequence_result(
            operation,
            place,
            outer,
            OperationKind::EstablishStructuralArray { elements: inputs },
        )];
    }
    let TerminalMachineResult::Structural(result) = &mut body.result else {
        unreachable!()
    };
    result.structural_type = outer;
    let receiver = &mut nested.machines[1];
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut receiver.structural_places[0].kind
    else {
        unreachable!()
    };
    *structural_type = outer;
    let OperationResult::Structural(result) =
        &mut receiver.blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result.structural_type = outer;
    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
        &nested,
        [receiver_id],
        &[],
    )
    .unwrap();
    let mut replay = Replay::with_slot_kind(
        &nested.types.structural_types,
        &graph,
        crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
    );
    let states = replay.states(receiver_id, 0).unwrap();
    let receiver = graph.iter().find(|row| row.id == receiver_id).unwrap();
    let current = states.get(&(receiver_entry, 1)).unwrap();
    let mut query = DescriptorContentsQuery {
        root: PlaceId::new(3).unwrap(),
        steps: vec![
            DescriptorContentsStep::any_element(sequence),
            DescriptorContentsStep::any_element(bytes),
        ],
    };
    assert_eq!(
        replay.read_contents(receiver, current, &query),
        Some(
            (1..=2)
                .map(|source| Origin::Backing(DescriptorCallOrigin::Literal {
                    machine: caller,
                    place: PlaceId::new(source).unwrap(),
                }))
                .collect()
        ),
        "nested enumeration follows each alternative's actual element contents",
    );
    query.steps[0] =
        DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(1));
    assert!(
        replay.read_contents(receiver, current, &query).is_none(),
        "nested traversal cannot make ordinal one exist in the shorter return"
    );
}

#[test]
fn cleanup_calls_keep_exact_edges_action_order_and_unique_owners() {
    let body = machine(1);
    let mut module = TerminalModule::declarations();
    module.machines.push(body);
    let mut graphs = terminal_borrowed_storage_graph(&module);
    let graph = &mut graphs[0];
    let position = (module.machines[0].entry, 0);
    for (edge, action_ordinal) in [(1, 0), (1, 2), (2, 0), (2, 2)] {
        let owner = DescriptorInvocationOwner::Cleanup {
            edge: EdgeId::new(edge).unwrap(),
            action_ordinal,
        };
        let call = || DescriptorInvocation {
            scalar_arguments: Vec::new(),
            owner,
            target: DescriptorInvocationTarget::Machine(MachineId::new(2).unwrap()),
            arguments: Vec::new(),
            argument_contents: Vec::new(),
            result: None,
        };
        assert!(graph.insert_call(position, call()).is_some());
        assert!(graph.insert_call(position, call()).is_none());
        assert_eq!(graph.invocation(position, owner).unwrap().owner, owner);
    }
    let owners: Vec<_> = graph
        .calls_at(position)
        .iter()
        .map(|call| call.owner)
        .collect();
    assert_eq!(owners.len(), 4);
    assert!(owners.windows(2).all(|rows| rows[0] < rows[1]));
    assert!(graph.direct_call(position).is_none());
    assert!(
        graph
            .invocation(
                position,
                DescriptorInvocationOwner::Cleanup {
                    edge: EdgeId::new(1).unwrap(),
                    action_ordinal: 1,
                }
            )
            .is_none()
    );
    // One normal exit cannot secretly select two cleanup edges.
    assert!(graph.validate_invocation_owners().is_none());
    graph.calls.get_mut(&position).unwrap().as_mut().unwrap().retain(|call| {
        matches!(call.owner, DescriptorInvocationOwner::Cleanup { edge, .. } if edge == EdgeId::new(1).unwrap())
    });
    assert!(graph.validate_invocation_owners().is_some());
    let second = graph.calls_at(position)[1].owner;
    let calls = graph.calls.get_mut(&position).unwrap().as_mut().unwrap();
    calls[1].owner = calls[0].owner;
    let duplicate = calls[0].owner;
    assert!(graph.invocation(position, duplicate).is_none());
    assert!(graph.validate_invocation_owners().is_none());
    let calls = graph.calls.get_mut(&position).unwrap().as_mut().unwrap();
    calls[1].owner = second;
    calls.swap(0, 1);
    assert!(
        graph.validate_invocation_owners().is_none(),
        "an unsorted raw roster cannot enter prepared replay"
    );
}

#[test]
fn decomposition_observes_contained_descriptors_without_consuming_them() {
    let view = StructuralTypeId::new(1).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let source = PlaceId::new(1).unwrap();
    let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
    let mut body = machine(1);
    body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    body.structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            structural_type: carrier,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    body.structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        });
    body.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::DecomposeStructuralValue {
            subject: terminal_psi::StructuralArgument::whole(
                source,
                StructuralAccess::Owned,
            ),
        },
    ));
    let declaration = |id, shape| StructuralTypeDeclaration {
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(1),
        declaration_identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    };
    let mut nominal = declaration(
        carrier,
        StructuralTypeShape::Record {
            fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: field,
                identity: TerminalIdentityIndex(2),
                declaration_identity: TerminalIdentityIndex(2),
                relevance: terminal_psi::BindingRelevance::Relevant,
                storage: None,
                field_type: terminal_psi::StructuralFieldType::Structural(view),
                inline_carrier: None,
            }],
            storage: terminal_psi::StructuralRecordStorage::Packed,
        },
    );
    nominal.nominal_cleanup = Some(terminal_psi::NominalCleanupSelection {
        declaration: terminal_psi::NominalDeclarationIdentity::new(nominal.identity),
        cleanup_machine: body.id,
    });
    let mut module = TerminalModule::declarations();
    module.types.structural_types = vec![
        declaration(
            view,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
        ),
        nominal,
    ];
    module.machines = vec![body.clone()];
    let graphs = terminal_borrowed_storage_graph(&module);
    assert_eq!(graphs.len(), 1);
    let position = (body.entry, 0);
    let step = &graphs[0].flow[&position];
    assert_eq!(
        step.observed_inputs().cloned().collect::<Vec<_>>(),
        [DescriptorLocation::whole(source)]
    );
    assert!(step.definitions.is_empty());
    assert!(step.derivations.is_empty());
    assert!(!step.may_escape);
    assert!(!graphs[0].payload_invalidations.contains_key(&position));
    let next = &graphs[0].flow[&(body.entry, 1)];
    assert!(next.definitions.is_empty());
}

// Moving a borrowed sequence value retires its descriptor, not
// its elements. A real element write still needs a known backing
// and cannot publish an empty or descriptor-only write frame.
#[test]
fn moving_sequence_values_does_not_write_their_backing() {
    let element = StructuralTypeId::new(1).unwrap();
    let sequence = StructuralTypeId::new(2).unwrap();
    let source = PlaceId::new(1).unwrap();
    let destination = PlaceId::new(2).unwrap();
    let operation = OperationId::new(1).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex::ABSENT,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    };
    for shape in [
        StructuralTypeShape::ElementView {
            element,
            access: None,
        },
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: None,
        }),
    ] {
        let mut module = TerminalModule::declarations();
        module.types.structural_types = vec![
            declaration(
                element,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            declaration(sequence, shape),
        ];
        let mut body = machine(1);
        body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
        body.structural_parameters
            .push(StructuralParameterDeclaration {
                place: source,
                position: 0,
                structural_type: sequence,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        body.structural_places = vec![
            terminal_psi::StructuralPlaceDeclaration {
                id: source,
                kind: StructuralPlaceKind::Parameter { position: 0 },
            },
            terminal_psi::StructuralPlaceDeclaration {
                id: destination,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: sequence,
                },
            },
        ];
        for kind in [
            OperationKind::MoveStructuralValue {
                source: terminal_psi::StructuralArgument::whole(
                    source,
                    StructuralAccess::Owned,
                ),
            },
            OperationKind::MoveStructuralField {
                source,
                path: Vec::new(),
                field: None,
            },
        ] {
            body.blocks[0].operations = vec![Operation::unsuspended(
                operation,
                OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    place: destination,
                    structural_type: sequence,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                }),
                kind,
            )];
            module.machines = vec![body.clone()];
            let graph = terminal_borrowed_storage_graph(&module);
            assert_eq!(graph.len(), 1);
            let position = (body.entry, 0);
            assert!(
                graph[0].flow[&position]
                    .inputs
                    .iter()
                    .all(|(_, access)| access.is_none()),
                "moving a whole descriptor does not read its backing"
            );
            if matches!(
                module.types.structural_types[1].shape,
                StructuralTypeShape::ByteSequence(_)
            ) {
                let length = OperationKind::ByteSequenceLength {
                    source: terminal_psi::StructuralArgument::whole(
                        source,
                        StructuralAccess::SharedBorrow,
                    ),
                };
                let uses = length.storage_uses();
                assert!(
                    matches!(
                        crate::descriptor_results::descriptor_storage_use_source(
                            &module.types.structural_types, graph[0].place_types.get(&source).copied(), &length, uses[0].0, |_| None,
                        ),
                        crate::descriptor_lifetimes::DescriptorFlowSource::Unavailable(root) if root == source
                    ),
                    "a byte descriptor address is not an exact byte-payload footprint"
                );
            }
            assert_eq!(
                graph[0].payload_invalidations[&position],
                [Some(DescriptorContentsQuery::from_location(
                    &DescriptorLocation::whole(source)
                ))]
            );
            let mut analysis = crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                &module.types.structural_types, &graph,
            );
            assert_eq!(
                analysis.storage_writes(body.id, position).unwrap(),
                Some(vec![
                    DescriptorLocation::whole(source),
                    DescriptorLocation::whole(destination),
                ])
            );
            let frames =
                crate::borrowed_storage::inferred_write_frames(&module).unwrap();
            assert_eq!(frames.get(body.id), Some(&Some(Vec::new())));
        }

        // Move a real field rather than the whole source root.
        // The sibling's entry contents must survive the same cut,
        // while the moved field cannot retain its former value.
        let record = StructuralTypeId::new(3).unwrap();
        let moved = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
        let sibling = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
        let field =
            |id, identity, field_type| terminal_psi::StructuralFieldDeclaration {
                declaration_identity: Default::default(),
                id,
                identity: TerminalIdentityIndex(identity),
                inline_carrier: None,
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type,
                storage: None,
            };
        module.types.structural_types.push(declaration(
            record,
            StructuralTypeShape::Record {
                fields: vec![
                    field(
                        moved,
                        1,
                        terminal_psi::StructuralFieldType::Structural(sequence),
                    ),
                    field(
                        sibling,
                        2,
                        terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                    ),
                ],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        ));
        let mut field_body = body.clone();
        field_body.structural_parameters[0].structural_type = record;
        field_body.blocks[0].operations[0].kind = OperationKind::MoveStructuralField {
            source,
            path: Vec::new(),
            field: Some(moved),
        };
        module.machines = vec![field_body.clone()];
        let graph = terminal_borrowed_storage_graph(&module);
        assert_eq!(graph.len(), 1);
        let accesses: Vec<_> = graph[0].flow[&(field_body.entry, 0)]
            .inputs
            .iter()
            .filter(|(_, access)| access.is_some())
            .collect();
        assert_eq!(accesses.len(), 1);
        let (location, Some((access_vocabulary::AccessOperation::Take, access))) =
            accesses[0]
        else {
            panic!("the projected move retains its take operation");
        };
        assert!(
            location.is_none(),
            "the event is not another old-content observation"
        );
        assert_eq!(
            graph[0].flow[&(field_body.entry, 0)]
                .observed_inputs()
                .collect::<Vec<_>>(),
            vec![&DescriptorLocation {
                root: source,
                path: vec![crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                    moved
                )],
            }],
            "the moved field cannot become its disjoint sibling"
        );
        if matches!(
            module.types.structural_types[1].shape,
            StructuralTypeShape::ByteSequence(_)
        ) {
            assert!(
                matches!(access, crate::descriptor_lifetimes::DescriptorFlowSource::Unavailable(root) if *root == source),
                "the byte-payload footprint remains unavailable"
            );
        } else {
            assert!(
                matches!(access, crate::descriptor_lifetimes::DescriptorFlowSource::Contents(query)
                if query.root == source && query.steps == [DescriptorContentsStep::Address(
                    crate::descriptor_lifetimes::DescriptorPathSegment::Field(moved))]),
                "the element descriptor keeps its actual typed field query"
            );
        }
        let frames = crate::borrowed_storage::inferred_write_frames(&module).unwrap();
        assert_eq!(frames.get(field_body.id), Some(&Some(Vec::new())));
        let mut analysis =
            crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                &module.types.structural_types,
                &graph,
            );
        assert_eq!(
            analysis
                .storage_writes(field_body.id, (field_body.entry, 0))
                .unwrap(),
            Some(vec![
                DescriptorLocation {
                    root: source,
                    path: vec![
                        crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                            moved
                        )
                    ]
                },
                DescriptorLocation::whole(destination),
            ])
        );
        for (field, survives) in [(sibling, true), (moved, false)] {
            let payload = crate::descriptor_results::DescriptorPayloadTarget::current(
                DescriptorLocation {
                    root: source,
                    path: vec![
                        crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                            field,
                        ),
                    ],
                },
            );
            assert_eq!(
                crate::descriptor_results::require_unchanged_entry_storage(
                    &module.types.structural_types,
                    &graph,
                    &mut analysis,
                    field_body.id,
                    (field_body.entry, 1),
                    &payload,
                )
                .is_ok(),
                survives,
            );
        }

        // An unknown caller-supplied backing is still unknown.
        // Use the real element-write operation, not a forged
        // summary, to retain the conservative effect boundary.
        body.structural_places.retain(|place| place.id == source);
        module.types.structural_types[1].shape = StructuralTypeShape::ElementView {
            element,
            access: Some(StructuralAccess::MutableBorrow),
        };
        let index = ValueId::new(1).unwrap();
        let value = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let integer = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            )
            .unwrap(),
        );
        body.parameters = vec![
            ValueDeclaration {
                id: index,
                scalar_type: integer,
                qualifications: ScalarQualificationSetId::ZERO,
            },
            ValueDeclaration {
                id: value,
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            },
            ValueDeclaration {
                id: length,
                scalar_type: integer,
                qualifications: ScalarQualificationSetId::ZERO,
            },
        ];
        body.parameter_order = vec![
            terminal_psi::BoundaryParameterKind::Structural,
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Scalar,
            terminal_psi::BoundaryParameterKind::Scalar,
        ];
        body.blocks[0].operations = vec![Operation::unsuspended(
            operation,
            OperationResult::Unit,
            OperationKind::ElementViewWrite {
                destination: terminal_psi::StructuralArgument::whole(
                    source,
                    StructuralAccess::MutableBorrow,
                ),
                path: Vec::new(),
                index,
                value,
                length,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            },
        )];
        module.machines = vec![body.clone()];
        let graph = terminal_borrowed_storage_graph(&module);
        assert_eq!(graph.len(), 1);
        assert!(matches!(
            graph[0].payload_invalidations[&(body.entry, 0)].as_slice(),
            [Some(DescriptorContentsQuery { steps, .. })]
                if matches!(steps.as_slice(), [DescriptorContentsStep::Element { .. }])
        ));
        let accesses: Vec<_> = graph[0].flow[&(body.entry, 0)]
            .inputs
            .iter()
            .filter_map(|(_, access)| access.as_ref())
            .collect();
        assert!(
            matches!(accesses.as_slice(),
            [(access_vocabulary::AccessOperation::Write, crate::descriptor_lifetimes::DescriptorFlowSource::Contents(query))]
                if query.root == source && matches!(query.steps.as_slice(),
                    [DescriptorContentsStep::Element { selector: Some(selector), .. }]
                        if **selector == semantic_vocabulary::ScalarTerm::value(index, integer))),
            "the backing write retains its evaluated index at the input cut"
        );
        assert_eq!(
            crate::borrowed_storage::inferred_write_frames(&module)
                .unwrap()
                .get(body.id),
            Some(&None)
        );
        // A variable array selection must not hide the later
        // view-backing write when it is widened to the array.
        let array = StructuralTypeId::new(4).unwrap();
        module.types.structural_types.push(declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: sequence,
                length: terminal_psi::ArrayExtent::Literal(1),
            },
        ));
        body.structural_parameters[0].structural_type = array;
        let OperationKind::ElementViewWrite { destination, .. } =
            &mut body.blocks[0].operations[0].kind
        else {
            unreachable!("the same backing-write control")
        };
        destination.path = vec![StructuralPathSegment::RuntimeIndex {
            index,
            obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
        }];
        module.machines = vec![body.clone()];
        let graph = terminal_borrowed_storage_graph(&module);
        assert_eq!(graph.len(), 1);
        assert!(matches!(
            graph[0].payload_invalidations[&(body.entry, 0)].as_slice(),
            [Some(DescriptorContentsQuery { steps, .. })]
                if matches!(steps.as_slice(), [DescriptorContentsStep::Element { .. }, DescriptorContentsStep::Element { .. }])
        ));
        assert_eq!(
            crate::borrowed_storage::inferred_write_frames(&module)
                .unwrap()
                .get(body.id),
            Some(&None)
        );
    }
}

#[test]
fn stored_and_forwarded_scalars_keep_evaluated_binders_and_both_edge_cuts() {
    let mut module = TerminalModule::declarations();
    let carrier = StructuralTypeId::new(1).unwrap();
    module.types.structural_types.push(StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: carrier,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        scalar_restriction: None,
    });
    let scalar = |id| ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::ZERO,
    };
    let mut machine = machine(1);
    let original = scalar(1);
    let saved = scalar(2);
    let formal = scalar(3);
    let home = PlaceId::new(1).unwrap();
    machine.parameters.push(original);
    machine.parameter_order = vec![
        terminal_psi::BoundaryParameterKind::Scalar,
        terminal_psi::BoundaryParameterKind::Structural,
    ];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: home,
            position: 0,
            structural_type: carrier,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    machine
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: home,
            kind: StructuralPlaceKind::Parameter { position: 0 },
        });
    machine.blocks[0].operations = vec![
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(saved),
            OperationKind::BooleanNot {
                operand: original.id,
            },
        ),
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: home,
                path: Vec::new(),
                value: saved.id,
                range_obligation: None,
            },
        ),
    ];
    let target = BlockId::new(2).unwrap();
    let edge = |id, actual| SuccessorEdge {
        edge: EdgeId::new(id).unwrap(),
        target,
        arguments: vec![actual],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: original.id,
        when_true: edge(1, saved.id),
        when_false: edge(2, original.id),
    };
    machine.blocks.push(Block::plain(
        target,
        vec![formal],
        Terminator::ReturnUnit {
            edge: EdgeId::new(3).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ));
    let captures = TerminalScalarCaptures::new(&module, &machine).unwrap();
    let entry = machine.entry;
    assert_eq!(
        captures.writes[&(entry, 1)].as_ref().unwrap(),
        &vec![(
            DescriptorLocation::whole(home),
            ScalarTerm::value(saved.id, ScalarType::Boolean)
        )]
    );
    let edges = &captures.arrivals[&(entry, 2)];
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0][0].0, formal.id);
    assert_eq!(
        edges[0][0].1.value_term(),
        Some(ScalarTerm::value(saved.id, ScalarType::Boolean))
    );
    assert_eq!(
        edges[1][0].1.value_term(),
        Some(ScalarTerm::value(original.id, ScalarType::Boolean))
    );
    assert_ne!(
        edges[0][0].1.value_term(),
        Some(ScalarTerm::BooleanNot {
            operand: Box::new(ScalarTerm::value(original.id, ScalarType::Boolean))
        })
    );
    machine.blocks[0].operations[1].kind = OperationKind::WriteOnlyPrimitiveStore {
        destination: PlaceId::new(99).unwrap(),
        path: Vec::new(),
        value: saved.id,
        range_obligation: None,
    };
    let incomplete = TerminalScalarCaptures::new(&module, &machine).unwrap();
    assert!(incomplete.writes[&(entry, 1)].is_err());
    machine.structural_places[0].kind = StructuralPlaceKind::ProviderAttachment {
        attachment: carrier,
        field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
        boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
    };
    assert!(matches!(
        TerminalScalarCaptures::new(&module, &machine),
        Err("scalar capture parameter changes its declared storage kind")
    ));
}
