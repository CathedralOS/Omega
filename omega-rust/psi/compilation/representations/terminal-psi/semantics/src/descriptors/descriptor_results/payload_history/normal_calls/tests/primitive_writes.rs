use super::*;
use semantic_vocabulary::{BlockId, OperationId, ScalarQualificationSetId, ValueId};
use terminal_psi::{
    OperationResult, StructuralDomainPredicate, StructuralOperationResult,
    StructuralPathQualification, StructuralPlaceDeclaration, ValueDeclaration,
};

fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}

fn module(qualified: bool) -> TerminalModule {
    let mut module = super::reference_words::module(qualified);
    let body = &mut module.machines[0];
    let boolean = body.structural_parameters[0].structural_type;
    body.structural_parameters[0].access = StructuralAccess::Owned;
    module.structural_types[1].shape = StructuralTypeShape::Reference {
        referent: boolean,
        access: StructuralAccess::MutableBorrow,
    };
    module.structural_domains[0].predicate = Some(StructuralDomainPredicate {
        subject: place(77),
        proposition: semantic_vocabulary::Proposition::Equal(
            ScalarTerm::boolean_field_path(place(77), Vec::new()),
            ScalarTerm::Boolean(true),
        ),
    });
    let scalar = ValueId::new(1).unwrap();
    body.blocks[0].operations = vec![
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: scalar,
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::BooleanConstant { value: false },
        ),
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Unit,
            OperationKind::WriteOnlyPrimitiveStore {
                destination: place(1),
                path: Vec::new(),
                value: scalar,
                range_obligation: None,
            },
        ),
        Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Structural(StructuralOperationResult {
                place: place(2),
                structural_type: module.structural_types[1].id,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: if qualified {
                    vec![StructuralPathQualification {
                        path: vec![StructuralPathSegment::Referent],
                        domain: module.structural_domains[0].id,
                    }]
                } else {
                    Vec::new()
                },
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
            OperationKind::EstablishReference {
                source: StructuralArgument::whole(place(1), StructuralAccess::MutableBorrow),
                storage: None,
            },
        ),
    ];
    body.structural_places.truncate(1);
    body.structural_places.push(StructuralPlaceDeclaration {
        id: place(2),
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer: OperationId::new(3).unwrap(),
            structural_type: module.structural_types[1].id,
        },
    });
    module
}

#[test]
fn primitive_store_keeps_a_known_empty_duty_list_but_retires_scalar_truth() {
    for qualified in [false, true] {
        let module = module(qualified);
        let body = &module.machines[0];
        let graph = terminal_borrowed_storage_graph(&module);
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        let block = BlockId::new(1).unwrap();
        let mut preservation =
            PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
        let before = preservation
            .current_payload_contract_in(&mut analysis, (block, 1), place(1))
            .unwrap();
        assert_eq!(
            before
                .qualifications
                .contains(&module.structural_domains[0].id),
            qualified
        );
        let mut observations = PayloadMembershipHistory::for_module(&module, &graph, body);
        let current = observations
            .current_payload_contract_in(&mut analysis, (block, 2), place(1))
            .unwrap();
        assert!(current.qualifications.is_empty());
        assert!(
            current
                .scalar_observations
                .iter()
                .any(|row| row.path.is_empty() && row.value == Some(ScalarTerm::Boolean(false)))
        );
        let result = body.blocks[0].operations[2].result.structural().unwrap();
        let retained = preservation.reference_formation_memberships(
            &module.identities,
            &mut analysis,
            (block, 2),
            &StructuralArgument::whole(place(1), StructuralAccess::MutableBorrow),
            None,
            result,
        );
        if qualified {
            assert_eq!(
                retained.unwrap_err(),
                "requested payload root has no reached incoming membership history"
            );
        } else {
            assert_eq!(retained.unwrap(), Vec::new());
        }
    }
}

#[test]
fn a_read_footprint_cannot_prove_that_a_write_preserves_the_duty_roster() {
    let module = module(false);
    let body = &module.machines[0];
    let mut graph = terminal_borrowed_storage_graph(&module);
    let block = BlockId::new(1).unwrap();
    graph[0].primitive_payload_writes.clear();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let query = DescriptorContentsQuery::from_location(&DescriptorLocation::whole(place(1)));
    assert_eq!(
        analysis.payload_targets(body.id, (block, 1), &query),
        Some(vec![DescriptorPayloadTarget::Storage(
            DescriptorLocation::whole(place(1))
        )])
    );
    let mut history = PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
    assert_eq!(
        history
            .current_payload_contract_in(&mut analysis, (block, 2), place(1))
            .unwrap_err(),
        "requested payload root has no reached incoming membership history"
    );
}

#[test]
fn a_later_primitive_store_does_not_recover_a_previously_unknown_duty_list() {
    let module = module(false);
    let body = &module.machines[0];
    let mut graph = terminal_borrowed_storage_graph(&module);
    let block = BlockId::new(1).unwrap();
    graph[0]
        .payload_invalidations
        .insert((block, 0), vec![None]);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let query = DescriptorContentsQuery::from_location(&DescriptorLocation::whole(place(1)));
    assert!(
        analysis
            .unchanged_primitive_payload_write(body.id, (block, 1), &query)
            .is_ok()
    );
    let mut history = PayloadMembershipHistory::for_module_preservation(&module, &graph, body);
    assert_eq!(
        history
            .current_payload_contract_in(&mut analysis, (block, 2), place(1))
            .unwrap_err(),
        "requested payload root has no reached incoming membership history"
    );
}

#[test]
fn scalar_writer_proofs_bind_each_invocations_actual_value_without_caching_the_first() {
    use semantic_vocabulary::{ContractId, StructuralPlaceKind};
    use terminal_psi::BoundaryParameterKind;
    let mut module = module(true);
    let helper_id = MachineId::new(2).unwrap();
    let formal = ValueId::new(7).unwrap();
    let mut helper = super::unit_machine();
    helper.id = helper_id;
    helper.contract = terminal_psi::MachineContract::empty(ContractId::new(2).unwrap());
    helper.structural_parameters = module.machines[0].structural_parameters.clone();
    helper.structural_parameters[0].access = StructuralAccess::MutableBorrow;
    helper.structural_parameters[0].qualifications.clear();
    helper.structural_places = vec![StructuralPlaceDeclaration {
        id: place(1),
        kind: StructuralPlaceKind::Parameter { position: 0 },
    }];
    helper.parameters = vec![ValueDeclaration {
        id: formal,
        scalar_type: ScalarType::Boolean,
        qualifications: ScalarQualificationSetId::ZERO,
    }];
    helper.parameter_order = vec![
        BoundaryParameterKind::Structural,
        BoundaryParameterKind::Scalar,
    ];
    helper.blocks[0].operations = vec![Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Unit,
        OperationKind::WriteOnlyPrimitiveStore {
            destination: place(1),
            path: Vec::new(),
            value: formal,
            range_obligation: None,
        },
    )];
    let caller = &mut module.machines[0];
    caller
        .structural_places
        .retain(|place| matches!(place.kind, StructuralPlaceKind::Parameter { .. }));
    caller.blocks[0].operations.clear();
    for (ordinal, value) in [(1, true), (2, false)] {
        let operand = ValueId::new(ordinal).unwrap();
        caller.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(ordinal * 2 - 1).unwrap(),
            OperationResult::Scalar(ValueDeclaration {
                id: operand,
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            OperationKind::BooleanConstant { value },
        ));
        caller.blocks[0].operations.push(Operation::unsuspended(
            OperationId::new(ordinal * 2).unwrap(),
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: helper_id,
                arguments: vec![operand],
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: vec![StructuralArgument::whole(
                    place(1),
                    StructuralAccess::MutableBorrow,
                )],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
    }
    let caller_id = caller.id;
    let entry = caller.entry;
    let prior = terminal_payload_contract(caller, place(1)).unwrap();
    module.machines.push(helper);
    let graph = terminal_borrowed_storage_graph(&module);
    assert_eq!(graph.len(), 2);
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    let mut context = NormalPayloadContext::new(&module, &graph);
    let before = State {
        values: [(place(1), prior)].into(),
        ..Default::default()
    };
    for (position, value) in [(1, true), (3, false)] {
        let effects = context
            .call_write_effects(&mut analysis, caller_id, (entry, position))
            .unwrap();
        assert_eq!(effects.len(), 1);
        let Ok(super::super::write_conservation::PerformedWrite::Primitive(writer)) =
            &effects[0].performed
        else {
            panic!("the exact helper store must retain its primitive writer");
        };
        assert_eq!(writer.machine, helper_id);
        assert_eq!(writer.position, (module.machines[1].entry, 0));
        assert_eq!(writer.value, ScalarTerm::Boolean(value));
        let conserved = context
            .conserved_call_payloads(&mut analysis, caller_id, (entry, position), &before)
            .unwrap();
        assert_eq!(
            conserved.len(),
            usize::from(value),
            "the same helper's two actual values prove different post-states"
        );
    }
}

#[test]
fn scalar_field_preservation_proves_the_whole_rule_from_each_writers_actual_value() {
    use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
    };
    let mut module = module(true);
    module.machines.clear();
    let record = StructuralTypeId::new(4).unwrap();
    let fields = [
        StructuralFieldId::new(1).unwrap(),
        StructuralFieldId::new(2).unwrap(),
    ];
    let mut declaration = module.structural_types[0].clone();
    declaration.id = record;
    declaration.identity = TerminalIdentityIndex(20);
    declaration.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: fields
            .iter()
            .enumerate()
            .map(|(index, id)| StructuralFieldDeclaration {
                id: *id,
                identity: TerminalIdentityIndex(21 + index as u32),
                inline_carrier: Some(module.structural_types[0].id),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                storage: None,
            })
            .collect(),
    };
    module.structural_types.push(declaration);
    let subject = place(77);
    let predicate = semantic_vocabulary::Proposition::Conjunction(
        fields
            .map(|field| {
                semantic_vocabulary::Proposition::Equal(
                    ScalarTerm::boolean_field_path(subject, vec![Segment::Field(field)]),
                    ScalarTerm::Boolean(true),
                )
            })
            .to_vec(),
    );
    module.structural_domains[0].carrier = record;
    module.structural_domains[0].predicate = Some(StructuralDomainPredicate {
        subject,
        proposition: predicate,
    });
    let domain = module.structural_domains[0].id;
    let mut prior = PayloadMembershipContract {
        carrier: record,
        qualifications: vec![domain],
        projected_qualifications: Vec::new(),
        scalar_requirements: Vec::new(),
        scalar_observations: fields
            .map(|field| PayloadScalarObservation {
                path: vec![DescriptorPathSegment::Field(field)],
                value: Some(ScalarTerm::Boolean(true)),
                entry: None,
            })
            .to_vec(),
    };
    for field in fields {
        let path = [DescriptorPathSegment::Field(field)];
        assert_eq!(
            super::super::primitive_writes::preserves_domain(
                &module,
                &prior,
                domain,
                &path,
                &ScalarTerm::Boolean(true)
            ),
            Ok(())
        );
        assert_eq!(
            super::super::primitive_writes::preserves_domain(
                &module,
                &prior,
                domain,
                &path,
                &ScalarTerm::Boolean(false)
            ),
            Err("the actual scalar write does not prove the complete original predicate"),
            "pre-call field values cannot prove this writer's post-state"
        );
    }
    let value = ScalarTerm::value(ValueId::new(9).unwrap(), ScalarType::Boolean);
    let path = [DescriptorPathSegment::Field(fields[0])];
    assert!(
        super::super::primitive_writes::preserves_domain(&module, &prior, domain, &path, &value)
            .is_err()
    );
    prior
        .scalar_requirements
        .push(semantic_vocabulary::Proposition::Equal(
            value.clone(),
            ScalarTerm::Boolean(true),
        ));
    assert_eq!(
        super::super::primitive_writes::preserves_domain(&module, &prior, domain, &path, &value),
        Ok(())
    );
    module.structural_domains[0].complete_predicate_theory = false;
    assert_eq!(
        super::super::primitive_writes::preserves_domain(&module, &prior, domain, &path, &value),
        Err("the primitive preservation duty has unrepresented value requirements")
    );
    module.structural_domains[0].complete_predicate_theory = true;
    module.structural_domains[0]
        .predicate
        .as_mut()
        .unwrap()
        .proposition = semantic_vocabulary::Proposition::Equal(
        ScalarTerm::boolean_field_path(subject, vec![Segment::Field(fields[1])]),
        ScalarTerm::Boolean(true),
    );
    assert_eq!(
        super::super::primitive_writes::preserves_domain(
            &module,
            &prior,
            domain,
            &path,
            &ScalarTerm::Boolean(false)
        ),
        Ok(()),
        "an ordinary sibling write leaves the second field unchanged"
    );
    let StructuralTypeShape::Record {
        storage,
        fields: view_fields,
    } = &mut module.structural_types.last_mut().unwrap().shape
    else {
        unreachable!()
    };
    *storage = StructuralRecordStorage::AccessView(semantic_vocabulary::AuthoredRecordStorage {
        byte_size: 1,
        byte_alignment: 1,
    });
    for field in view_fields {
        field.storage = Some(semantic_vocabulary::AuthoredFieldStorage::Scalar {
            byte_offset: 0,
            stored_scalar: Some(ScalarType::Boolean),
        });
    }
    assert_eq!(
        super::super::primitive_writes::preserves_domain(
            &module,
            &prior,
            domain,
            &path,
            &ScalarTerm::Boolean(false)
        ),
        Err("scalar preservation needs independently proved external record geometry"),
        "different view field names do not prove their backing bytes are separate"
    );
}
