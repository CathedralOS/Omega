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
