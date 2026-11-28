//! Two structurally distinct package subjects share one public declaration.
use super::*;
use crate::packages::check_package_sites;
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralRecordStorage, TerminalPackageCatalog, TerminalPackageEvent, TerminalPackageOpening,
    TerminalPackageSchema, TerminalPackageSite,
};

fn ty(id: u64) -> StructuralTypeId {
    StructuralTypeId::new(id).unwrap()
}
fn place(id: u64) -> PlaceId {
    PlaceId::new(id).unwrap()
}
fn field(id: u64) -> StructuralFieldId {
    StructuralFieldId::new(id).unwrap()
}
fn callable(id: u64) -> CallableContractId {
    CallableContractId::new(id).unwrap()
}

fn param(slot: u64, structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(slot),
        position: (slot - 1) as u32,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: vec![],
        projected_qualifications: vec![],
        additional_input_qualifications: vec![],
    }
}

fn declaration(id: u64, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: field(id),
        identity: TerminalIdentityIndex(id as u32),
        relevance: language_core::BindingRelevance::Relevant,
        field_type,
        storage: None,
    }
}

fn readers() -> TerminalModule {
    let mut program = module();
    program.execution.callables.descriptors.clear();
    let contract = program.execution.callables.contracts[0].clone();
    program.execution.callables.contracts.clear();
    // schema witness 1, left/right opened witnesses 2/3, concrete File/Socket 4/5.
    for i in 1..=5 {
        let mut contract = contract.clone();
        contract.id = callable(i);
        contract.identity = TerminalIdentityIndex(i as u32);
        contract.proof.id = ContractId::new(i).unwrap();
        contract.structural_parameters = vec![param(1, ty(i))];
        contract.proof.structural_parameters = contract.structural_parameters.clone();
        program.execution.callables.contracts.push(contract);
        let mut row = byte_view_descriptor(i, None);
        row.shape = if i <= 3 {
            StructuralTypeShape::Opaque
        } else {
            StructuralTypeShape::Record {
                fields: vec![],
                storage: StructuralRecordStorage::Packed,
            }
        };
        program.types.structural_types.push(row);
    }
    let mut reference = byte_view_descriptor(6, Some(StructuralAccess::MutableBorrow));
    reference.shape = StructuralTypeShape::Reference {
        referent: ty(1),
        access: StructuralAccess::MutableBorrow,
    };
    program.types.structural_types.push(reference);
    let mut reader = byte_view_descriptor(7, None);
    reader.shape = StructuralTypeShape::Record {
        fields: vec![
            declaration(1, StructuralFieldType::Structural(ty(6))),
            declaration(
                2,
                StructuralFieldType::Scalar(ScalarType::Callable(callable(1))),
            ),
        ],
        storage: StructuralRecordStorage::Packed,
    };
    program.types.structural_types.push(reader);
    let mut consumer = machine(1);
    consumer.structural_parameters = vec![param(1, ty(7)), param(2, ty(7))];
    let mut selected = value(1);
    selected.scalar_type = ScalarType::Callable(callable(2));
    let project = Operation::unsuspended(
        operation_id(1),
        OperationResult::Scalar(selected),
        O::PrimitiveScalarRead {
            source: place(1),
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(2))],
        },
    );
    let mut call = indirect_call(2, selected.id);
    if let O::CallMachine {
        contract,
        structural_arguments,
        ..
    } = &mut call.kind
    {
        *contract = callable(2);
        structural_arguments.push(StructuralArgument {
            place: place(1),
            access: StructuralAccess::MutableBorrow,
            path: vec![
                StructuralPathSegment::Field(TerminalIdentityIndex(1)),
                StructuralPathSegment::Referent,
            ],
        });
    }
    consumer.blocks[0].operations = vec![project, call];
    program.machines = vec![consumer];
    program.execution.packages = TerminalPackageCatalog {
        schemas: vec![TerminalPackageSchema {
            record: ty(7),
            witnesses: vec![ty(1)],
        }],
        openings: (1..=2)
            .map(|i| TerminalPackageOpening {
                machine: machine_id(1),
                record: ty(7),
                subject: StructuralArgument::whole(place(i), StructuralAccess::MutableBorrow),
                witnesses: vec![ty(i + 1)],
            })
            .collect(),
        sites: vec![
            TerminalPackageSite {
                machine: machine_id(1),
                operation: operation_id(1),
                event: TerminalPackageEvent::Project {
                    opening: 0,
                    field: field(2),
                },
            },
            TerminalPackageSite {
                machine: machine_id(1),
                operation: operation_id(2),
                event: TerminalPackageEvent::Invoke { opening: 0 },
            },
        ],
    };
    program
}

#[test]
fn package_value_queries_keep_the_origin_of_each_callback() {
    let mut program = readers();
    add_two_constructions(&mut program);
    let entry = &mut program.machines[0];
    for parameter in &entry.contract.structural_parameters {
        entry.structural_places.push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
            },
        });
    }
    for (slot, producer) in [(5, 3), (6, 4)] {
        entry.structural_places.push(StructuralPlaceDeclaration {
            id: place(slot),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(producer),
                structural_type: ty(7),
            },
        });
    }
    let boundary = (entry.entry, entry.blocks[0].operations.len() as u32);
    let body = entry.id;
    let projected = terminal_callable_values(&program).unwrap();
    let field_path = vec![DescriptorPathSegment::Field(field(2))];
    let opened = resolve_callable_value(
        &projected,
        CallableValueQuery::Scalar {
            machine: body,
            value: value(1).id,
            contract: callable(2),
        },
    )
    .unwrap();
    assert_eq!(
        opened.inputs,
        vec![CallableFormalInput {
            machine: body,
            parameter: CallableParameter::Structural {
                position: 0,
                path: field_path.clone()
            },
            contract: callable(1),
        }]
    );
    for (index, slot) in [5, 6].into_iter().enumerate() {
        let initialized = resolve_callable_value(
            &projected,
            CallableValueQuery::Storage {
                machine: body,
                position: boundary,
                location: DescriptorLocation {
                    root: place(slot),
                    path: field_path.clone(),
                },
                contract: callable(1),
            },
        )
        .unwrap();
        assert_eq!(
            initialized.inputs,
            vec![CallableFormalInput {
                machine: body,
                parameter: CallableParameter::Scalar(index),
                contract: callable(index as u64 + 4),
            }]
        );
        assert!(initialized.descriptors.is_empty());
        assert!(initialized.unresolved_ceilings.is_empty());
    }
}

#[test]
fn package_sites_retain_descriptors_and_require_the_same_state() {
    let mut program = readers();
    let sites = check_package_sites(&program).unwrap();
    assert_eq!(
        sites.projected_field(machine_id(1), operation_id(1)),
        Some(field(2))
    );
    assert_eq!(
        sites.invocation_contract(machine_id(1), operation_id(2)),
        Some(callable(2))
    );
    assert!(sites.admits_descriptor(&program, machine_id(1), operation_id(2), callable(4)));
    assert!(sites.admits_descriptor(&program, machine_id(1), operation_id(2), callable(5)));
    program.execution.callables.contracts[4].envelope.suspends = true;
    assert!(!sites.admits_descriptor(&program, machine_id(1), operation_id(2), callable(5)));
    if let O::CallMachine {
        structural_arguments,
        ..
    } = &mut program.machines[0].blocks[0].operations[1].kind
    {
        structural_arguments[0].place = place(2);
    }
    assert!(check_package_sites(&program).is_err());
}

#[test]
fn opened_descriptors_preserve_the_named_requirement() {
    let mut program = readers();
    let mut identities = program.identities.into_builder();
    let read = identities.intern("Preview::read");
    let other_read = identities.intern("Other::read");
    program.identities = identities.build();
    for contract in &mut program.execution.callables.contracts {
        contract.requirement = Some(read);
    }
    let sites = check_package_sites(&program).unwrap();
    for (requirement, admitted) in [(Some(read), true), (Some(other_read), false), (None, false)] {
        program.execution.callables.contracts[4].requirement = requirement;
        assert_eq!(
            sites.admits_descriptor(&program, machine_id(1), operation_id(2), callable(5)),
            admitted
        );
    }
}

#[test]
fn package_places_release_the_stored_carrier_and_read_its_declared_descriptor() {
    let mut program = readers();
    let machine = &program.machines[0];
    let read = &machine.blocks[0].operations[0];
    assert!(crate::packages::projects_stored_callable(
        &program,
        machine,
        read,
        ScalarType::Callable(callable(1))
    ));
    assert!(!crate::packages::projects_stored_callable(
        &program,
        machine,
        read,
        ScalarType::Callable(callable(3))
    ));
    let release = Operation::unsuspended(
        operation_id(3),
        OperationResult::Unit,
        O::ReleaseReference {
            source: place(1),
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(1))],
        },
    );
    let ops = &mut program.machines[0].blocks[0].operations;
    ops.push(release);
    check_package_sites(&program).expect("the stored reference may be ended");
    let O::ReleaseReference { path, .. } = &mut program.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    path.push(StructuralPathSegment::Referent);
    assert!(
        check_package_sites(&program).is_err(),
        "closing a reference grants no access to its pointee"
    );
}

#[test]
fn package_sites_refuse_missing_or_crossed_evidence() {
    for i in 0..2 {
        let mut program = readers();
        program.execution.packages.sites.remove(i);
        assert!(check_package_sites(&program).is_err());
    }
    let mut program = readers();
    program.execution.packages.openings[1].witnesses = vec![ty(2)];
    assert!(check_package_sites(&program).is_err());
    let mut program = readers();
    program.execution.packages.sites[1].event = TerminalPackageEvent::Invoke { opening: 1 };
    assert!(check_package_sites(&program).is_err());
}

fn add_two_constructions(program: &mut TerminalModule) {
    use terminal_psi::{RecordFieldInitializer, RecordFieldValue, StructuralOperationResult};
    for (i, concrete) in [4, 5].into_iter().enumerate() {
        let mut reference = program.types.structural_types[5].clone();
        let actual_ref = ty(8 + i as u64);
        reference.id = actual_ref;
        reference.identity = TerminalIdentityIndex(8 + i as u32);
        reference.shape = StructuralTypeShape::Reference {
            referent: ty(concrete),
            access: StructuralAccess::MutableBorrow,
        };
        program.types.structural_types.push(reference);
        let m = &mut program.machines[0];
        let mut supplied = value(2 + i as u64);
        supplied.scalar_type = ScalarType::Callable(callable(concrete));
        m.parameters.push(supplied);
        let mut state = param(3 + i as u64, actual_ref);
        state.access = StructuralAccess::Owned;
        let borrowed = StructuralArgument::whole(state.place, StructuralAccess::Owned);
        m.structural_parameters.push(state);
        let operation = operation_id(3 + i as u64);
        m.blocks[0].operations.push(Operation::unsuspended(
            operation,
            OperationResult::Structural(StructuralOperationResult {
                place: place(5 + i as u64),
                structural_type: ty(7),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: vec![],
                qualification_establishments: vec![],
                claims: vec![],
            }),
            O::EstablishRecord {
                fields: vec![
                    RecordFieldInitializer {
                        field: field(1),
                        value: RecordFieldValue::Structural(borrowed),
                    },
                    RecordFieldInitializer {
                        field: field(2),
                        value: RecordFieldValue::Scalar {
                            value: supplied.id,
                            range_obligation: None,
                        },
                    },
                ],
            },
        ));
        program.execution.packages.sites.push(TerminalPackageSite {
            machine: m.id,
            operation,
            event: TerminalPackageEvent::Construct {
                witnesses: vec![ty(concrete)],
            },
        });
    }
}

#[test]
fn package_sites_construct_two_instances_and_reject_crossed_callback() {
    let mut program = readers();
    add_two_constructions(&mut program);
    let sites = check_package_sites(&program).unwrap();
    for i in 0u64..2 {
        assert_eq!(
            sites.field_type(machine_id(1), operation_id(3 + i), field(2)),
            Some(&StructuralFieldType::Scalar(ScalarType::Callable(
                callable(4 + i)
            )))
        );
    }
    if let O::EstablishRecord { fields } = &mut program.machines[0].blocks[0].operations[2].kind {
        fields[1].value = terminal_psi::RecordFieldValue::Scalar {
            value: value(3).id,
            range_obligation: None,
        };
    }
    assert!(check_package_sites(&program).is_err());
}

#[test]
fn package_sites_only_retires_a_saved_descriptor_after_its_subject_changes() {
    let make = |destination| {
        let mut program = readers();
        let m = &mut program.machines[0];
        let mut replacement = param(3, ty(7));
        replacement.access = StructuralAccess::Owned;
        m.structural_parameters.push(replacement);
        for (slot, position) in [(1, 0), (2, 1), (3, 2)] {
            m.parameter_order.push(BoundaryParameterKind::Structural);
            m.structural_places.push(StructuralPlaceDeclaration {
                id: place(slot),
                kind: StructuralPlaceKind::Parameter { position },
            });
        }
        m.blocks[0].operations[1].id = operation_id(3);
        program.execution.packages.sites[1].operation = operation_id(3);
        m.blocks[0].operations.insert(
            1,
            Operation::unsuspended(
                operation_id(2),
                OperationResult::Unit,
                O::StoreStructuralField {
                    destination: place(destination),
                    path: vec![],
                    field: None,
                    disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                    value: StructuralArgument::whole(place(3), StructuralAccess::Owned),
                },
            ),
        );
        program
    };
    check_package_sites(&make(2))
        .expect("replacing the other package preserves the saved descriptor");
    assert!(check_package_sites(&make(1)).is_err());
}

fn shared_pair_projections() -> TerminalModule {
    let mut program = readers();
    let reference = &mut program.types.structural_types[5];
    reference.shape = StructuralTypeShape::Reference {
        referent: ty(1),
        access: StructuralAccess::SharedBorrow,
    };
    reference.multiplicity = StructuralMultiplicity::Unrestricted;
    for (id, referent) in [(8, 2), (9, 3)] {
        let mut opened = program.types.structural_types[5].clone();
        opened.id = ty(id);
        opened.identity = TerminalIdentityIndex(id as u32);
        opened.shape = StructuralTypeShape::Reference {
            referent: ty(referent),
            access: StructuralAccess::SharedBorrow,
        };
        program.types.structural_types.push(opened);
    }
    program.types.structural_types[6].shape = StructuralTypeShape::Record {
        fields: vec![
            declaration(1, StructuralFieldType::Structural(ty(6))),
            declaration(2, StructuralFieldType::Structural(ty(6))),
        ],
        storage: StructuralRecordStorage::Packed,
    };
    let m = &mut program.machines[0];
    m.blocks[0].operations.clear();
    program.execution.packages.sites.clear();
    // Both references are copied from the same incoming pair.
    for i in [1, 2] {
        m.parameter_order.push(BoundaryParameterKind::Structural);
        m.structural_places.push(StructuralPlaceDeclaration {
            id: place(i),
            kind: StructuralPlaceKind::Parameter {
                position: (i - 1) as u32,
            },
        });
    }
    for i in [1, 2] {
        let op = operation_id(i);
        m.blocks[0].operations.push(Operation::unsuspended(
            op,
            result(i + 2, ty(8)),
            O::StructuralLeafCopy {
                source: place(1),
                path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(
                    i as u32,
                ))],
            },
        ));
        m.structural_places.push(StructuralPlaceDeclaration {
            id: place(i + 2),
            kind: StructuralPlaceKind::OperationResult {
                producer: op,
                structural_type: ty(8),
            },
        });
        program.execution.packages.sites.push(TerminalPackageSite {
            machine: m.id,
            operation: op,
            event: TerminalPackageEvent::Project {
                opening: 0,
                field: field(i),
            },
        });
    }
    program
}

fn result(slot: u64, ty: StructuralTypeId) -> OperationResult {
    use terminal_psi::StructuralOperationResult;
    OperationResult::Structural(StructuralOperationResult {
        place: place(slot),
        structural_type: ty,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: vec![],
        projected_qualifications: vec![],
        additional_input_qualifications: vec![],
        qualification_establishments: vec![],
        claims: vec![],
    })
}

#[test]
fn package_sites_repack_only_references_from_the_same_live_opening() {
    use terminal_psi::{RecordFieldInitializer, RecordFieldValue};
    let mut program = shared_pair_projections();
    let m = &mut program.machines[0];
    let initializers = [1, 2]
        .into_iter()
        .map(|i| RecordFieldInitializer {
            field: field(i),
            value: RecordFieldValue::Structural(StructuralArgument::whole(
                place(i + 2),
                StructuralAccess::Owned,
            )),
        })
        .collect();
    m.blocks[0].operations.push(Operation::unsuspended(
        operation_id(3),
        result(5, ty(7)),
        O::EstablishRecord {
            fields: initializers,
        },
    ));
    m.structural_places.push(StructuralPlaceDeclaration {
        id: place(5),
        kind: StructuralPlaceKind::OperationResult {
            producer: operation_id(3),
            structural_type: ty(7),
        },
    });
    program.execution.packages.sites.push(TerminalPackageSite {
        machine: m.id,
        operation: operation_id(3),
        event: TerminalPackageEvent::Construct {
            witnesses: vec![ty(2)],
        },
    });
    check_package_sites(&program).expect("two shared references from one package may be repacked");
    let op = &mut program.machines[0].blocks[0].operations[1];
    if let O::StructuralLeafCopy { source, .. } = &mut op.kind {
        *source = place(2);
    }
    if let OperationResult::Structural(result) = &mut op.result {
        result.structural_type = ty(9);
    }
    program.machines[0].structural_places[3].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(2),
        structural_type: ty(9),
    };
    program.execution.packages.sites[1].event = TerminalPackageEvent::Project {
        opening: 1,
        field: field(2),
    };
    assert!(check_package_sites(&program).is_err());
}

#[test]
fn package_sites_reborrow_the_same_state_through_a_saved_callback() {
    let mut program = readers();
    let m = &mut program.machines[0];
    for (i, position) in [(1, 0), (2, 1)] {
        m.parameter_order.push(BoundaryParameterKind::Structural);
        m.structural_places.push(StructuralPlaceDeclaration {
            id: place(i),
            kind: StructuralPlaceKind::Parameter { position },
        });
    }
    let mut again = m.blocks[0].operations[1].clone();
    again.id = operation_id(3);
    m.blocks[0].operations.push(again);
    program.execution.packages.sites.push(TerminalPackageSite {
        machine: m.id,
        operation: operation_id(3),
        event: TerminalPackageEvent::Invoke { opening: 0 },
    });
    check_package_sites(&program)
        .expect("fresh reborrows preserve the stored callback and reference");
}

#[test]
fn dependent_store_requires_a_projected_reference_from_its_destination_opening() {
    let mut module = shared_pair_projections();
    let m = &mut module.machines[0];
    m.blocks[0].operations.push(Operation::unsuspended(
        operation_id(3),
        OperationResult::Unit,
        O::StoreStructuralField {
            destination: place(1),
            path: vec![],
            field: Some(field(1)),
            disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
            value: StructuralArgument::whole(place(4), StructuralAccess::Owned),
        },
    ));
    let site = TerminalPackageSite {
        machine: m.id,
        operation: operation_id(3),
        event: TerminalPackageEvent::Store {
            opening: 0,
            field: field(1),
        },
    };
    assert!(
        check_package_sites(&module).is_err(),
        "a missing receipt is refused"
    );
    module.execution.packages.sites.push(site);
    let checked = check_package_sites(&module).unwrap();
    assert_eq!(
        checked.stored_type(machine_id(1), operation_id(3)),
        Some(ty(8))
    );
    // A saved projected value survives a replacement on another subject only.
    for (destination, replacement_type, valid) in [
        (place(2), ty(7), true),
        (place(1), ty(7), false),
        (place(4), ty(8), false),
    ] {
        let mut deferred = module.clone();
        let machine = &mut deferred.machines[0];
        let mut replacement = param(5, replacement_type);
        replacement.position = 2;
        replacement.access = StructuralAccess::Owned;
        machine.structural_parameters.push(replacement);
        machine
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place(5),
            kind: StructuralPlaceKind::Parameter { position: 2 },
        });
        machine.blocks[0].operations[2].id = operation_id(4);
        deferred.execution.packages.sites[2].operation = operation_id(4);
        machine.blocks[0].operations.insert(
            2,
            Operation::unsuspended(
                operation_id(3),
                OperationResult::Unit,
                O::StoreStructuralField {
                    destination,
                    path: vec![],
                    field: None,
                    disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                    value: StructuralArgument::whole(place(5), StructuralAccess::Owned),
                },
            ),
        );
        assert_eq!(
            check_package_sites(&deferred).is_ok(),
            valid,
            "write to {destination:?}"
        );
    }
    let mut wrong = module.clone();
    let O::StoreStructuralField { destination, .. } =
        &mut wrong.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *destination = place(2);
    assert!(check_package_sites(&wrong).is_err());
    let mut wrong = module;
    let O::StructuralLeafCopy { source, .. } = &mut wrong.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *source = place(2);
    if let OperationResult::Structural(output) =
        &mut wrong.machines[0].blocks[0].operations[1].result
    {
        output.structural_type = ty(9);
    }
    wrong.machines[0].structural_places[3].kind = StructuralPlaceKind::OperationResult {
        producer: operation_id(2),
        structural_type: ty(9),
    };
    wrong.execution.packages.sites[1].event = TerminalPackageEvent::Project {
        opening: 1,
        field: field(2),
    };
    assert!(check_package_sites(&wrong).is_err());
}

#[test]
fn an_opened_callback_retains_each_guarantee_under_its_own_obligation_id() {
    let mut program = readers();
    for (index, contract) in program.execution.callables.contracts.iter_mut().enumerate() {
        for (offset, proposition) in [
            semantic_vocabulary::Proposition::Truth,
            semantic_vocabulary::Proposition::Falsehood,
        ]
        .into_iter()
        .enumerate()
        {
            contract.proof.ensures.push(terminal_psi::ContractClause {
                obligation: semantic_vocabulary::ObligationId::new(
                    100 + index as u64 * 2 + offset as u64,
                )
                .unwrap(),
                proposition,
            });
        }
    }
    let admit = |module: &TerminalModule| {
        crate::calls::callable_refinement::validate_callable_evidence_contracts_in(
            &module.execution.callables,
            &module.proofs.proposition_declarations,
            &module.proofs.proposition_applications,
            &module.proofs.evidence_terms,
            &module.proofs.evidence_contract_lanes,
        )?;
        check_package_sites(module).map(|_| ())
    };
    assert!(admit(&program).is_ok());
    let public = program.execution.callables.contracts[0]
        .proof
        .ensures
        .clone();
    let opened = &program.execution.callables.contracts[1].proof.ensures;
    let (first, second) = (opened[0].obligation, opened[1].obligation);
    for (left, right) in [(first, first), (second, first)] {
        let mut malformed = program.clone();
        let clauses = &mut malformed.execution.callables.contracts[1].proof.ensures;
        clauses[0].obligation = left;
        clauses[1].obligation = right;
        assert_eq!(
            admit(&malformed),
            Err("runtime callable guarantees are not in strict obligation order")
        );
    }
    // Independent headers may reuse labels. Reusing the public label never
    // establishes a different proposition or a missing guarantee.
    program.execution.callables.contracts[1].proof.ensures = public;
    assert!(admit(&program).is_ok());
    let mut changed = program.clone();
    changed.execution.callables.contracts[1].proof.ensures[0].proposition =
        semantic_vocabulary::Proposition::Falsehood;
    assert!(admit(&changed).is_err());
    program.execution.callables.contracts[1].proof.ensures.pop();
    assert!(admit(&program).is_err());
}

fn repeated_reader_with_output() -> TerminalModule {
    let mut program = readers();
    let output = ty(8);
    program
        .types
        .structural_types
        .push(byte_view_descriptor(8, None));
    for contract in &mut program.execution.callables.contracts {
        let output_formal = param(2, output);
        contract.structural_parameters.push(output_formal.clone());
        contract.proof.structural_parameters.push(output_formal);
    }
    let machine = &mut program.machines[0];
    machine.structural_parameters.push(param(3, output));
    for i in [1, 2, 3] {
        machine
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place(i),
            kind: StructuralPlaceKind::Parameter {
                position: (i - 1) as u32,
            },
        });
    }
    let call = &mut machine.blocks[0].operations[1];
    let O::CallMachine {
        structural_arguments,
        ..
    } = &mut call.kind
    else {
        unreachable!()
    };
    structural_arguments.push(StructuralArgument::whole(
        place(3),
        StructuralAccess::MutableBorrow,
    ));
    let mut again = call.clone();
    again.id = operation_id(3);
    machine.blocks[0].operations.push(again);
    program.execution.packages.sites.push(TerminalPackageSite {
        machine: machine.id,
        operation: operation_id(3),
        event: TerminalPackageEvent::Invoke { opening: 0 },
    });
    program
}

// The first call must accept the evaluated field and all its arguments before
// the second call can show that the observation expired.
fn assert_callback_expires(program: TerminalModule) {
    let mut once = program.clone();
    once.machines[0].blocks[0].operations.pop();
    once.execution.packages.sites.pop();
    check_package_sites(&once).expect("the first call uses the freshly checked field");
    assert!(
        check_package_sites(&program).is_err(),
        "the saved callback should expire"
    );
}

#[test]
fn package_sites_reuse_a_saved_callback_with_a_separate_primitive_output() {
    let program = repeated_reader_with_output();
    check_package_sites(&program).expect("the caller lends separate mutable bytes at both calls");
    let unsigned = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
            .unwrap(),
    );
    for shape in [
        StructuralTypeShape::PrimitiveScalar(unsigned),
        StructuralTypeShape::FixedArray {
            element: ty(9),
            length: terminal_psi::ArrayExtent::Literal(3),
        },
        StructuralTypeShape::ElementView {
            element: ty(9),
            access: None,
        },
    ] {
        let mut scalars = program.clone();
        scalars.types.structural_types[7].shape = shape;
        let mut element = byte_view_descriptor(9, None);
        element.shape = StructuralTypeShape::PrimitiveScalar(unsigned);
        scalars.types.structural_types.push(element);
        check_package_sites(&scalars)
            .expect("a separate primitive output cannot rewrite package storage");
    }
    for shape in [
        StructuralTypeShape::Opaque,
        StructuralTypeShape::PrimitiveScalar(ScalarType::Callable(callable(1))),
        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            semantic_vocabulary::IntegerType::address(64).unwrap(),
        )),
        StructuralTypeShape::Reference {
            referent: ty(7),
            access: StructuralAccess::MutableBorrow,
        },
    ] {
        let mut authority = program.clone();
        authority.types.structural_types[7].shape = shape;
        assert_callback_expires(authority);
    }
}

#[test]
fn package_sites_expire_the_saved_callback_if_a_call_lends_the_record_or_foreign_authority() {
    let program = repeated_reader_with_output();
    let mut whole = program.clone();
    for contract in &mut whole.execution.callables.contracts {
        contract.structural_parameters[1].structural_type = ty(7);
        contract.proof.structural_parameters[1].structural_type = ty(7);
    }
    for call in &mut whole.machines[0].blocks[0].operations[1..] {
        let O::CallMachine {
            structural_arguments,
            ..
        } = &mut call.kind
        else {
            unreachable!()
        };
        structural_arguments[1].place = place(1);
    }
    assert_callback_expires(whole);

    let mut slot = program.clone();
    let O::CallMachine {
        structural_arguments,
        ..
    } = &mut slot.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path.pop();
    assert!(
        check_package_sites(&slot).is_err(),
        "the reference slot cannot substitute for its pointee"
    );

    let mut foreign = program.clone();
    for contract in &mut foreign.execution.callables.contracts {
        contract
            .envelope
            .service_reach
            .push(semantic_vocabulary::ServiceId::new(1).unwrap());
    }
    assert_callback_expires(foreign);
}

#[test]
fn package_sites_keep_a_fixed_reader_index_across_a_scalar_output_write() {
    let mut program = repeated_reader_with_output();
    let mut index = value(9);
    index.scalar_type = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
            .unwrap(),
    );
    program.machines[0].parameters.push(index);
    program.machines[0]
        .parameter_order
        .push(BoundaryParameterKind::Scalar);
    let mut readers = byte_view_descriptor(9, None);
    readers.shape = StructuralTypeShape::FixedArray {
        element: ty(7),
        length: terminal_psi::ArrayExtent::Literal(2),
    };
    program.types.structural_types.push(readers);
    program.machines[0].structural_parameters[0].structural_type = ty(9);
    let indexed = |selector: StructuralPathSegment| {
        let mut indexed = program.clone();
        indexed.execution.packages.openings[0]
            .subject
            .path
            .push(selector.clone());
        for operation in &mut indexed.machines[0].blocks[0].operations {
            let path = match &mut operation.kind {
                O::PrimitiveScalarRead { path, .. } => path,
                O::CallMachine {
                    structural_arguments,
                    ..
                } => &mut structural_arguments[0].path,
                _ => unreachable!(),
            };
            path.insert(0, selector.clone());
        }
        indexed
    };
    check_package_sites(&indexed(StructuralPathSegment::FixedIndex(0)))
        .expect("a literal index is unaffected by an output write");
    assert_callback_expires(indexed(StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(9).unwrap(),
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
    }));
}

#[test]
fn a_saved_callback_survives_a_field_write_that_preserves_its_package_witness() {
    let mut program = callback_field_assignment();
    let final_site = TerminalPackageSite {
        operation: operation_id(4),
        ..program.execution.packages.sites[1].clone()
    };
    program.execution.packages.sites.push(final_site);
    let operations = &mut program.machines[0].blocks[0].operations;
    let mut final_store = operations[1].clone();
    final_store.id = operation_id(4);
    operations.push(final_store);
    check_package_sites(&program).expect("two stores of the same saved callback keep its witness");

    // An evidence-free store may update the independent tag. Reusing that
    // operation to write a dependent callback without a receipt must fail.
    let StructuralTypeShape::Record { fields, .. } =
        &mut program.types.structural_types.last_mut().unwrap().shape
    else {
        unreachable!()
    };
    fields.push(declaration(
        4,
        StructuralFieldType::Scalar(ScalarType::Boolean),
    ));
    program.execution.packages.sites.remove(1);
    let ops = &mut program.machines[0].blocks[0].operations;
    ops.insert(
        1,
        Operation::unsuspended(
            operation_id(2),
            OperationResult::Scalar(ValueDeclaration {
                scalar_type: ScalarType::Boolean,
                ..value(2)
            }),
            O::BooleanConstant { value: true },
        ),
    );
    ops[2] = Operation::unsuspended(
        operation_id(3),
        OperationResult::Unit,
        O::StructuralScalarFieldStore {
            destination: place(1),
            path: vec![],
            field: field(4),
            value: value(2).id,
            range_obligation: None,
        },
    );
    check_package_sites(&program).expect("the independent tag cannot replace the package witness");
    let O::StructuralScalarFieldStore {
        field: target,
        value: source,
        ..
    } = &mut program.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *target = field(2);
    *source = value(1).id;
    assert!(check_package_sites(&program).is_err());
}

#[test]
fn a_saved_callback_at_a_join_requires_both_paths_to_keep_the_same_package() {
    let mut program = callback_field_assignment();
    let machine = &mut program.machines[0];
    let branch = BlockId::new(2).unwrap();
    let join = BlockId::new(3).unwrap();
    let edge = |id, target| terminal_psi::SuccessorEdge {
        edge: EdgeId::new(id).unwrap(),
        target,
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![],
        cleanup_actions: vec![],
    };
    let first_store = machine.blocks[0].operations.pop().unwrap();
    let mut last_store = first_store.clone();
    last_store.id = operation_id(4);
    machine.parameters.push(ValueDeclaration {
        scalar_type: ScalarType::Boolean,
        ..value(2)
    });
    machine.parameter_order.push(BoundaryParameterKind::Scalar);
    let returned = std::mem::replace(
        &mut machine.blocks[0].terminator,
        Terminator::Conditional {
            condition: value(2).id,
            when_true: edge(2, branch),
            when_false: edge(3, join),
        },
    );
    // A receiver can serialize the join before its predecessor.
    machine
        .blocks
        .push(Block::plain(join, vec![], returned));
    machine.blocks[1].operations.push(last_store);
    machine.blocks.push(Block::plain(
        branch,
        vec![],
        Terminator::Jump {
            edge: EdgeId::new(4).unwrap(),
            target: join,
            arguments: vec![],
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            structural_arguments: vec![],
            cleanup_actions: vec![],
        },
    ));
    machine.blocks[2].operations.push(first_store);
    program.execution.packages.sites.push(TerminalPackageSite {
        operation: operation_id(4),
        ..program.execution.packages.sites[1].clone()
    });
    check_package_sites(&program)
        .expect("the saved callback survives a checked field update on one path");

    program.machines[0].blocks[2].operations[0].kind = O::StoreStructuralField {
        destination: place(1),
        path: vec![],
        field: None,
        value: StructuralArgument::whole(place(2), StructuralAccess::Owned),
        disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
    };
    assert!(
        check_package_sites(&program).is_err(),
        "a whole-package replacement cannot claim the old field Store receipt"
    );
    program.execution.packages.sites.remove(1);
    assert!(
        check_package_sites(&program).is_err(),
        "a whole-package write on either path expires the saved callback"
    );
}

fn callback_field_assignment() -> TerminalModule {
    let mut program = readers();
    let StructuralTypeShape::Record { fields, .. } =
        &mut program.types.structural_types.last_mut().unwrap().shape
    else {
        unreachable!()
    };
    fields.push(declaration(
        3,
        StructuralFieldType::Scalar(ScalarType::Callable(callable(1))),
    ));
    let machine = &mut program.machines[0];
    for position in 0..2 {
        machine
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place(u64::from(position) + 1),
            kind: StructuralPlaceKind::Parameter { position },
        });
    }
    let operations = &mut program.machines[0].blocks[0].operations;
    let O::PrimitiveScalarRead { path, .. } = &mut operations[0].kind else {
        unreachable!()
    };
    *path = vec![StructuralPathSegment::Field(TerminalIdentityIndex(3))];
    operations[1] = Operation::unsuspended(
        operation_id(2),
        OperationResult::Unit,
        O::StructuralScalarFieldStore {
            destination: place(1),
            path: vec![],
            field: field(2),
            value: value(1).id,
            range_obligation: None,
        },
    );
    let sites = &mut program.execution.packages.sites;
    sites[0].event = TerminalPackageEvent::Project {
        opening: 0,
        field: field(3),
    };
    sites[1].event = TerminalPackageEvent::Store {
        opening: 0,
        field: field(2),
    };
    program
}

#[test]
fn stored_callback_keeps_the_destination_packages_current_witness() {
    let mut program = callback_field_assignment();
    assert_eq!(
        check_package_sites(&program)
            .unwrap()
            .stored_field_type(machine_id(1), operation_id(2)),
        Some(&StructuralFieldType::Scalar(ScalarType::Callable(
            callable(2)
        )))
    );

    assert!(check_package_sites(&program).unwrap().admits_descriptor(
        &program,
        machine_id(1),
        operation_id(2),
        callable(4)
    ));

    let mut crossed = program.clone();
    let read = &mut crossed.machines[0].blocks[0].operations[0];
    let O::PrimitiveScalarRead { source, .. } = &mut read.kind else {
        unreachable!()
    };
    *source = place(2);
    let OperationResult::Scalar(result) = &mut read.result else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Callable(callable(3));
    crossed.execution.packages.sites[0].event = TerminalPackageEvent::Project {
        opening: 1,
        field: field(3),
    };
    assert!(check_package_sites(&crossed).is_err());

    let mut wrong_target = program.clone();
    let O::StructuralScalarFieldStore { destination, .. } =
        &mut wrong_target.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *destination = place(2);
    assert!(check_package_sites(&wrong_target).is_err());

    program.execution.packages.sites.pop();
    assert!(check_package_sites(&program).is_err());
}
