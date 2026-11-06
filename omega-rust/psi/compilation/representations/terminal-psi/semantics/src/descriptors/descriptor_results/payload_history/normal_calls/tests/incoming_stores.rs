use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, OperationId, ScalarQualificationSetId, StructuralPlaceKind,
    ValueId,
};
use terminal_psi::{
    BoundaryParameterKind, OperationResult, StructuralDomainPredicate,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPlaceDeclaration,
    StructuralStoreDisposition, SuccessorEdge, ValueDeclaration,
};

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}
fn block(id: u64) -> BlockId {
    BlockId::new(id).unwrap()
}
fn domain() -> StructuralDomainId {
    StructuralDomainId::new(1).unwrap()
}
fn edge(id: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(id).unwrap(),
        target: block(target),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    }
}
fn store(id: u64, destination: u64, value: u64) -> Operation {
    Operation::unsuspended(
        OperationId::new(id).unwrap(),
        OperationResult::Unit,
        OperationKind::StoreStructuralField {
            disposition: StructuralStoreDisposition::ReplaceDiscardable,
            destination: place(destination),
            path: Vec::new(),
            field: None,
            value: StructuralArgument::whole(place(value), StructuralAccess::Owned),
        },
    )
}

/// The entry owns a pointer, then lends its current pointee to a block formal.
/// Only the separate actual RHS may establish the post-store predicate.
fn module(entry_qualified: bool, rhs_qualified: bool) -> TerminalModule {
    let boolean = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let mut body = super::unit_machine();
    body.contract = terminal_psi::MachineContract::empty(ContractId::new(1).unwrap());
    let parameter = |root, position, carrier, access, qualified| StructuralParameterDeclaration {
        place: place(root),
        position,
        structural_type: carrier,
        access,
        multiplicity: if carrier == boolean {
            StructuralMultiplicity::Unrestricted
        } else {
            StructuralMultiplicity::Affine
        },
        qualifications: if qualified && carrier == boolean {
            vec![domain()]
        } else {
            Vec::new()
        },
        projected_qualifications: if qualified && carrier == reference {
            vec![StructuralPathQualification {
                path: vec![StructuralPathSegment::Referent],
                domain: domain(),
            }]
        } else {
            Vec::new()
        },
    };
    body.structural_parameters = vec![
        parameter(1, 0, reference, StructuralAccess::Owned, entry_qualified),
        parameter(3, 1, boolean, StructuralAccess::Owned, rhs_qualified),
        parameter(4, 2, reference, StructuralAccess::Owned, false),
        parameter(5, 3, boolean, StructuralAccess::Owned, false),
    ];
    body.parameter_order = vec![BoundaryParameterKind::Structural; 4];
    body.structural_places = body
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        })
        .collect();
    body.structural_places.push(StructuralPlaceDeclaration {
        id: place(2),
        kind: StructuralPlaceKind::BlockParameter {
            block: block(2),
            position: 0,
        },
    });
    let mut arrival = edge(1, 2);
    arrival.structural_arguments = vec![StructuralArgument {
        place: place(1),
        path: vec![StructuralPathSegment::Referent],
        access: StructuralAccess::MutableBorrow,
    }];
    body.blocks[0].terminator = Terminator::Jump {
        edge: arrival.edge,
        target: arrival.target,
        arguments: arrival.arguments,
        erased_arguments: arrival.erased_arguments,
        erased_proof_arguments: arrival.erased_proof_arguments,
        structural_arguments: arrival.structural_arguments,
        cleanup_actions: arrival.cleanup_actions,
    };
    let mut written = terminal_psi::Block::plain(
        block(2),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: EdgeId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        },
    );
    written.structural_parameters = vec![parameter(
        2,
        0,
        boolean,
        StructuralAccess::MutableBorrow,
        entry_qualified,
    )];
    written.operations = vec![store(1, 2, 3)];
    body.blocks.push(written);
    let declaration = |id, shape| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: id == boolean,
        descriptor_cleanup: id == reference,
        scalar_restriction: None,
        shape,
    };
    TerminalModule {
        structural_types: vec![
            declaration(
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            declaration(
                reference,
                StructuralTypeShape::Reference {
                    referent: boolean,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
        ],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain(),
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(3),
            carrier: boolean,
            content_projection: None,
            byte_predicate: None,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: Some(StructuralDomainPredicate {
                subject: place(77),
                proposition: Proposition::Equal(
                    ScalarTerm::boolean_field_path(place(77), Vec::new()),
                    ScalarTerm::Boolean(true),
                ),
            }),
            establishment_routes: Vec::new(),
        }],
        machines: vec![body],
        ..TerminalModule::declarations()
    }
}

fn observed(
    module: &TerminalModule,
    at: DescriptorPosition,
    root: u64,
) -> Result<PayloadMembershipContract, &'static str> {
    let graph = terminal_borrowed_storage_graph(module);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    PayloadMembershipHistory::for_module(module, &graph, &module.machines[0])
        .current_selected_payload_contract_in(
            &mut analysis,
            at,
            place(root),
            &[StructuralPathSegment::Referent],
        )
}

#[test]
fn incoming_whole_store_observes_actual_rhs_without_reseeding_the_entry_fact() {
    for entry in [false, true] {
        for rhs in [false, true] {
            let module = module(entry, rhs);
            assert_eq!(
                observed(&module, (block(2), 0), 1)
                    .unwrap()
                    .qualifications
                    .contains(&domain()),
                entry
            );
            let after = observed(&module, (block(2), 1), 1).unwrap();
            assert_eq!(
                after.qualifications.contains(&domain()),
                rhs,
                "entry={entry}, rhs={rhs}"
            );
        }
    }
}

#[test]
fn incoming_store_needs_operation_owned_geometry_and_does_not_become_a_write_address() {
    let module = module(false, true);
    let mut graph = terminal_borrowed_storage_graph(&module);
    let identity = module.machines[0].id;
    let destination = DescriptorLocation::whole(place(2));
    let query = DescriptorContentsQuery::from_location(&destination);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    assert!(
        analysis
            .exact_payload_target(identity, (block(2), 0), &query)
            .is_none()
    );
    assert!(
        analysis
            .unchanged_payload_store_targets(
                identity,
                (block(2), 0),
                &DescriptorLocation::whole(place(3)),
                &destination
            )
            .is_ok()
    );
    let mut preservation =
        PayloadMembershipHistory::for_module_preservation(&module, &graph, &module.machines[0]);
    assert!(
        preservation
            .current_selected_payload_contract_in(
                &mut analysis,
                (block(2), 1),
                place(1),
                &[StructuralPathSegment::Referent]
            )
            .is_err()
    );
    drop(preservation);
    drop(analysis);
    graph[0].payload_stores.clear();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut history = PayloadMembershipHistory::for_module(&module, &graph, &module.machines[0]);
    assert!(
        history
            .current_selected_payload_contract_in(
                &mut analysis,
                (block(2), 1),
                place(1),
                &[StructuralPathSegment::Referent]
            )
            .is_err()
    );
}

#[test]
fn incoming_written_fact_is_retired_by_another_writer_and_by_pointer_replacement() {
    let mut unknown = module(true, true);
    let value = ValueId::new(1).unwrap();
    unknown.machines[0].blocks[1].operations.extend([
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: value,
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::BooleanConstant { value: false },
        ),
        Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: place(2),
                path: Vec::new(),
                value,
                range_obligation: None,
            },
        ),
    ]);
    assert!(
        observed(&unknown, (block(2), 1), 1)
            .unwrap()
            .qualifications
            .contains(&domain())
    );
    assert!(observed(&unknown, (block(2), 3), 1).is_err());
    let mut replaced = module(true, true);
    replaced.machines[0].blocks[1]
        .operations
        .push(store(2, 1, 4));
    assert!(
        observed(&replaced, (block(2), 1), 1)
            .unwrap()
            .qualifications
            .contains(&domain())
    );
    assert!(observed(&replaced, (block(2), 2), 1).is_err());
}

#[test]
fn incoming_observations_intersect_actual_branch_stores() {
    for both_qualified in [false, true] {
        let mut module = module(false, true);
        let body = &mut module.machines[0];
        let choice = ValueId::new(1).unwrap();
        body.parameters.push(ValueDeclaration {
            id: choice,
            scalar_type: ScalarType::Boolean,
            qualifications: ScalarQualificationSetId::ZERO,
        });
        body.parameter_order.push(BoundaryParameterKind::Scalar);
        body.blocks[1].operations.clear();
        body.blocks[1].terminator = Terminator::Conditional {
            condition: choice,
            when_true: edge(3, 3),
            when_false: edge(4, 4),
        };
        for (id, rhs) in [(3, 3), (4, if both_qualified { 3 } else { 5 })] {
            let next = edge(id + 2, 5);
            let mut branch = terminal_psi::Block::plain(
                block(id),
                Vec::new(),
                Terminator::Jump {
                    edge: next.edge,
                    target: next.target,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                },
            );
            branch.operations = vec![store(id, 2, rhs)];
            body.blocks.push(branch);
        }
        body.blocks.push(terminal_psi::Block::plain(
            block(5),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(8).unwrap(),
                cleanup_actions: Vec::new(),
            },
        ));
        assert_eq!(
            observed(&module, (block(5), 0), 1)
                .unwrap()
                .qualifications
                .contains(&domain()),
            both_qualified
        );
    }
}

#[test]
fn a_runtime_selected_store_can_invalidate_an_earlier_incoming_observation() {
    use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId};
    let mut module = module(true, true);
    let boolean = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(3).unwrap();
    let index = ValueId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: array,
        identity: TerminalIdentityIndex(4),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape: StructuralTypeShape::FixedArray {
            element: boolean,
            length: terminal_psi::ArrayExtent::Literal(1),
        },
    });
    let body = &mut module.machines[0];
    body.parameters.push(ValueDeclaration {
        id: index,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: ScalarQualificationSetId::ZERO,
    });
    body.parameter_order.push(BoundaryParameterKind::Scalar);
    body.structural_parameters
        .push(StructuralParameterDeclaration {
            place: place(6),
            position: 4,
            structural_type: array,
            access: StructuralAccess::MutableBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    body.parameter_order.push(BoundaryParameterKind::Structural);
    body.structural_places.push(StructuralPlaceDeclaration {
        id: place(6),
        kind: StructuralPlaceKind::Parameter { position: 4 },
    });
    body.blocks[1].operations.push(Operation::unsuspended(
        OperationId::new(2).unwrap(),
        OperationResult::Unit,
        OperationKind::StoreStructuralField {
            disposition: StructuralStoreDisposition::ReplaceDiscardable,
            destination: place(6),
            path: vec![StructuralPathSegment::RuntimeIndex {
                index,
                obligation: ObligationId::new(1).unwrap(),
            }],
            field: None,
            value: StructuralArgument::whole(place(5), StructuralAccess::Owned),
        },
    ));
    let graph = terminal_borrowed_storage_graph(&module);
    assert!(!graph[0].payload_stores.contains_key(&(block(2), 1)));
    assert!(!graph[0].payload_invalidations[&(block(2), 1)].is_empty());
    assert!(
        observed(&module, (block(2), 1), 1)
            .unwrap()
            .qualifications
            .contains(&domain())
    );
    assert!(observed(&module, (block(2), 2), 1).is_err());
}
