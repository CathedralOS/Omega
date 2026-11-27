use super::*;
use crate::embedding::program::TerminalProgram;
use crate::engine::calls::frames::tests::empty_program;
use crate::{TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue};
use semantic_vocabulary::{EdgeId, ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    BoundaryParameterKind, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, TerminalMachineResult, Terminator,
};

fn instance(
    value: TerminalScalarValue,
) -> Result<TerminalInstance<'static, 'static>, TerminalInterpretError> {
    let mut program = empty_program();
    let module = std::sync::Arc::get_mut(&mut program)
        .unwrap()
        .fixture_module_mut();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    let mut identities = module.identities.clone().into_builder();
    module
        .types
        .structural_types
        .push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: structural_type,
            identity: identities.intern("borrowed Boolean"),
            quotient_carrier: None,
            owns_linear_claim: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            scalar_restriction: None,
            requires: Vec::new(),
        });
    module.identities = identities.build();
    let machine = &mut module.machines[0];
    machine.parameter_order = vec![BoundaryParameterKind::Structural];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: place,
        returned_claims: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let execution = TerminalExecution::start_with_program(
        program.clone(),
        &[],
        crate::TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue::minted(
                42,
                structural_type,
                Vec::new(),
            )],
            primitive_values: &[crate::TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value,
            }],
            ..Default::default()
        },
        None,
        None,
    )?;
    Ok(TerminalInstance::new(
        TerminalProgram { program },
        execution,
        Vec::new(),
    ))
}

fn projected_fixture(
    no_code_affine_cleanup: bool,
) -> (TerminalProgram<'static>, crate::TerminalProjectedHostAtom) {
    let mut program = empty_program();
    let module = std::sync::Arc::get_mut(&mut program)
        .unwrap()
        .fixture_module_mut();
    let mut identities = module.identities.clone().into_builder();
    let field_identity = identities.intern("facet");
    let field = terminal_psi::StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id: StructuralFieldId::new(1).unwrap(),
        identity: field_identity,
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: terminal_psi::StructuralFieldType::Structural(
            StructuralTypeId::new(2).unwrap(),
        ),
        storage: None,
    };
    for (id, name, fields) in [
        (1, "host carrier", vec![field]),
        (2, "empty host value", Vec::new()),
    ] {
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                id: StructuralTypeId::new(id).unwrap(),
                identity: identities.intern(name),
                quotient_carrier: None,
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Affine,
                no_code_affine_cleanup,
                descriptor_cleanup: no_code_affine_cleanup,
                nominal_cleanup: None,
                shape: StructuralTypeShape::Record {
                    fields,
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
                scalar_restriction: None,
                requires: Vec::new(),
            });
    }
    module.identities = identities.build();
    let machine = &mut module.machines[0];
    let place = PlaceId::new(1).unwrap();
    machine
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 0 },
        });
    machine.parameter_order = vec![BoundaryParameterKind::Structural];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }];
    let atom = crate::TerminalProjectedHostAtom {
        argument_index: 0,
        path: vec![terminal_psi::StructuralPathSegment::Field(field_identity)],
        value: TerminalStructuralValue::minted(100, StructuralTypeId::new(2).unwrap(), Vec::new()),
    };
    (TerminalProgram { program }, atom)
}

fn projected_instance(
    program: &TerminalProgram<'static>,
    atoms: &[crate::TerminalProjectedHostAtom],
) -> Result<TerminalInstance<'static, 'static>, TerminalInterpretError> {
    projected_accounted_instance(program, atoms, None)
}

fn projected_accounted_instance(
    program: &TerminalProgram<'static>,
    atoms: &[crate::TerminalProjectedHostAtom],
    account: Option<build_model::accounting::EvaluationInvocationAccount>,
) -> Result<TerminalInstance<'static, 'static>, TerminalInterpretError> {
    let entry = program
        .program
        .machine(program.program.resolve_entry(None)?)?;
    let arguments: Vec<_> = entry
        .structural_parameters
        .iter()
        .map(|parameter| {
            TerminalStructuralValue::minted(
                42 + u64::from(parameter.position),
                parameter.structural_type,
                Vec::new(),
            )
        })
        .collect();
    let initialized: Vec<_> = entry
        .structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .collect();
    let execution = TerminalExecution::start_with_program(
        program.program.clone(),
        &[],
        crate::TerminalStructuralInputs {
            arguments: &arguments,
            zero_initialized_arguments: &initialized,
            projected_host_atoms: atoms,
            ..Default::default()
        },
        None,
        account,
    )?;
    Ok(TerminalInstance::new(
        program.clone(),
        execution,
        Vec::new(),
    ))
}

#[test]
fn whole_borrowed_marker_copies_keep_the_issued_value_and_distinct_homes() {
    let (mut program, mut marker) = projected_fixture(true);
    marker.path.clear();
    let mut other = marker.clone();
    other.argument_index = 1;
    let module = std::sync::Arc::get_mut(&mut program.program)
        .unwrap()
        .fixture_module_mut();
    let machine = &mut module.machines[0];
    let first = &mut machine.structural_parameters[0];
    first.structural_type = marker.value.structural_type;
    let source = first.place;
    let destination = PlaceId::new(2).unwrap();
    let second = StructuralParameterDeclaration {
        place: destination,
        position: 1,
        ..first.clone()
    };
    machine.structural_parameters.push(second);
    machine
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: destination,
            kind: semantic_vocabulary::StructuralPlaceKind::Parameter { position: 1 },
        });
    machine
        .parameter_order
        .push(BoundaryParameterKind::Structural);
    assert!(matches!(
        projected_instance(&program, &[marker.clone(), other.clone()]),
        Err(TerminalInterpretError::StructuralArgumentAliasing(100))
    ));

    let module = std::sync::Arc::get_mut(&mut program.program)
        .unwrap()
        .fixture_module_mut();
    module.types.structural_types[1].multiplicity = StructuralMultiplicity::Unrestricted;
    let machine = &mut module.machines[0];
    for parameter in &mut machine.structural_parameters {
        parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    }
    machine.blocks[0]
        .operations
        .push(terminal_psi::Operation::unsuspended(
            semantic_vocabulary::OperationId::new(1).unwrap(),
            terminal_psi::OperationResult::Unit,
            terminal_psi::OperationKind::StoreStructuralField {
                disposition: terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                destination,
                path: Vec::new(),
                field: None,
                value: terminal_psi::StructuralArgument::whole(source, StructuralAccess::Owned),
            },
        ));

    let mut conflict = other.clone();
    conflict
        .value
        .qualifications
        .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap());
    assert!(matches!(
        projected_instance(&program, &[marker.clone(), conflict]),
        Err(TerminalInterpretError::StructuralArgumentAliasing(100))
    ));
    assert!(projected_instance(&program, &[marker.clone(), marker.clone()]).is_err());
    let mut different = other.clone();
    different.value.opaque_identity = 101;
    for (atoms, expected) in [
        (vec![marker.clone(), other], Some(&marker.value)),
        (vec![marker.clone(), different.clone()], Some(&marker.value)),
        (vec![different], None),
    ] {
        let mut instance = projected_instance(&program, &atoms).unwrap();
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(100))
            .unwrap();
        for index in [0, 1] {
            assert_eq!(
                instance
                    .borrowed_entry_host_atom(index, &[])
                    .unwrap()
                    .as_ref(),
                expected
            );
            assert_eq!(
                instance.execution.as_ref().unwrap().borrowed_entry_storage[index as usize]
                    .2
                    .opaque_identity,
                42 + u64::from(index)
            );
        }
    }
}

#[test]
fn projected_host_atoms_require_exact_paths_types_memberships_and_fresh_identities() {
    let (program, atom) = projected_fixture(true);
    let (uncertified, same_slot) = projected_fixture(false);
    assert!(projected_instance(&uncertified, &[same_slot]).is_err());
    for identity in [1, 42] {
        let mut invalid = atom.clone();
        invalid.value.opaque_identity = identity;
        assert!(matches!(projected_instance(&program, &[invalid]),
            Err(TerminalInterpretError::StructuralArgumentAliasing(actual)) if actual == identity));
    }
    let mut invalid = atom.clone();
    invalid.argument_index = 1;
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut invalid = atom.clone();
    invalid.path.clear(); // The root is a nonempty record.
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut invalid = atom.clone();
    invalid.path = vec![terminal_psi::StructuralPathSegment::Field(
        terminal_psi::TerminalIdentityIndex(u32::MAX),
    )];
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut invalid = atom.clone();
    invalid.value.structural_type = StructuralTypeId::new(1).unwrap();
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut invalid = atom.clone();
    invalid.value.path = invalid.path.clone();
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut invalid = atom.clone();
    invalid.path = vec![terminal_psi::StructuralPathSegment::Referent];
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut invalid = atom.clone();
    invalid
        .value
        .qualifications
        .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap());
    assert!(projected_instance(&program, &[invalid]).is_err());
    let mut duplicate = atom.clone();
    duplicate.value.opaque_identity = 101;
    assert!(projected_instance(&program, &[atom.clone(), duplicate]).is_err());
    assert!(projected_instance(&program, &[atom.clone(), atom]).is_err());
}

#[test]
fn projected_host_observation_preserves_absence_and_replacement() {
    let (program, atom) = projected_fixture(true);
    let mut instance = projected_instance(&program, std::slice::from_ref(&atom)).unwrap();
    assert!(instance.borrowed_entry_host_atom(0, &atom.path).is_err());
    assert!(matches!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(100))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
    assert_eq!(
        instance.borrowed_entry_host_atom(0, &atom.path).unwrap(),
        Some(atom.value.clone())
    );
    assert!(instance.borrowed_entry_host_atom(1, &atom.path).is_err());
    assert!(instance.borrowed_entry_host_atom(0, &[]).is_err());
    let execution = instance.execution.as_mut().unwrap();
    let (_, _, root) = &execution.borrowed_entry_storage[0];
    let selected = execution
        .projected_empty_host_value(root, &atom.path)
        .unwrap();
    let home = StructuralRuntimePlace::from(&selected);
    execution
        .structural_host_atoms
        .insert(
            home.clone(),
            crate::storage::structural_storage::HostStructuralPayload::Modified,
        )
        .unwrap();
    assert!(
        instance.borrowed_entry_host_atom(0, &atom.path).is_err(),
        "invalid payload state is not an ordinary absent value"
    );
    let execution = instance.execution.as_mut().unwrap();
    execution
        .structural_host_atoms
        .insert(
            home.clone(),
            crate::storage::structural_storage::HostStructuralPayload::Opaque(
                atom.value.opaque_identity,
            ),
        )
        .unwrap();
    let retirement = execution.prepare_subtree_retirement(&home).unwrap();
    execution.prune_subtree(retirement).unwrap();
    assert_eq!(
        execution.borrowed_entry_storage[0].2.opaque_identity, 42,
        "replacing the value does not change its stable borrowed home"
    );
    assert_eq!(
        instance.borrowed_entry_host_atom(0, &atom.path).unwrap(),
        None
    );

    let mut absent = projected_instance(&program, &[]).unwrap();
    absent
        .invoke(&mut terminal_work::WorkMeter::with_budget(100))
        .unwrap();
    assert_eq!(
        absent.borrowed_entry_host_atom(0, &atom.path).unwrap(),
        None
    );
}

#[test]
fn projected_host_tokens_are_reserved_before_local_allocation() {
    let (program, mut atom) = projected_fixture(true);
    atom.value.opaque_identity = 0;
    let mut instance = projected_instance(&program, std::slice::from_ref(&atom)).unwrap();
    let execution = instance.execution.as_mut().unwrap();
    let identity = execution.local_structural_identities.allocate().unwrap();
    assert!(
        ![0, 1, 42].contains(&identity),
        "projected, static and supplied identities are reserved"
    );
}

#[test]
fn entry_and_result_observe_typed_storage_and_reject_unknown_coordinates() {
    for value in [false, true] {
        let mut instance = instance(TerminalScalarValue::Boolean(value)).unwrap();
        assert_eq!(instance.observe_storage(crate::TerminalObservationRoot::ArgumentAtHostCall(0), &[]).unwrap_err(),
            TerminalStorageObservationError::PendingHostComputationRequired);
        assert_eq!(
            instance.observe_storage(crate::TerminalObservationRoot::ReturnedArgument(0), &[]).unwrap_err(),
            TerminalStorageObservationError::NormalReturnRequired
        );
        assert_eq!(
            instance.observe_storage(crate::TerminalObservationRoot::Result, &[]).unwrap_err(),
            TerminalStorageObservationError::NormalReturnRequired
        );
        assert!(matches!(
            instance
                .invoke(&mut terminal_work::WorkMeter::with_budget(100))
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(_))
        ));
        let expected =
            TerminalStorageObservationValue::Storage(crate::TerminalStorageContents::Scalar {
                value: TerminalScalarValue::Boolean(value),
                stored_scalar: ScalarType::Boolean,
            });
        assert_eq!(instance.observe_storage(crate::TerminalObservationRoot::ReturnedArgument(0), &[]).unwrap(), expected);
        assert_eq!(instance.observe_storage(crate::TerminalObservationRoot::Result, &[]).unwrap(), expected);
        let unknown = [Path::Field(StructuralFieldId::new(99).unwrap())];
        assert_eq!(
            instance.observe_storage(crate::TerminalObservationRoot::ReturnedArgument(0), &unknown).unwrap_err(),
            TerminalStorageObservationError::UnknownPath
        );
        assert_eq!(
            instance.observe_storage(crate::TerminalObservationRoot::Result, &unknown).unwrap_err(),
            TerminalStorageObservationError::UnknownPath
        );
        assert_eq!(
            instance.observe_storage(crate::TerminalObservationRoot::ReturnedArgument(1), &[]).unwrap_err(),
            TerminalStorageObservationError::NotBorrowedEntryArgument { argument_index: 1 }
        );
    }
}
