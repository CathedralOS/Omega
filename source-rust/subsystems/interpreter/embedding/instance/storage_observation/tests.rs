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

fn continuation_fixture(
    shift: bool,
) -> (TerminalProgram<'static>, crate::TerminalProjectedHostAtom) {
    use semantic_vocabulary::{MachineId, OperationId, SuspensionCrossingId};
    use terminal_psi::{
        CheckedMachineDeclaration, NominalDeclarationIdentity, Operation, OperationKind,
        OperationResult,
    };

    let (mut program, mut atom) = projected_fixture(true);
    let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
    let module = admitted.fixture_module_mut();
    let mut names = terminal_psi::TerminalIdentityTableBuilder::default();
    if shift {
        names.intern("additional package");
    }
    let field = names.intern("facet");
    let owner = StructuralTypeId::new(if shift { 2 } else { 1 }).unwrap();
    let token = StructuralTypeId::new(if shift { 1 } else { 2 }).unwrap();
    let carrier = &mut module.types.structural_types[0];
    carrier.identity = names.intern("host carrier");
    carrier.id = owner;
    let StructuralTypeShape::Record { fields, .. } = &mut carrier.shape else {
        unreachable!()
    };
    fields[0].identity = field;
    fields[0].field_type = terminal_psi::StructuralFieldType::Structural(token);
    module.types.structural_types[1].identity = names.intern("empty host value");
    module.types.structural_types[1].id = token;
    module.types.structural_types.sort_by_key(|ty| ty.id);
    atom.value.structural_type = token;
    atom.path = vec![terminal_psi::StructuralPathSegment::Field(field)];

    let callee = MachineId::new(2).unwrap();
    let cut = OperationId::new(1).unwrap();
    let crossing = SuspensionCrossingId::new(1).unwrap();
    let mut helper = module.machines[0].clone();
    helper.id = callee;
    helper.structural_places.clear();
    helper.structural_parameters.clear();
    helper.parameter_order.clear();
    module.machines.push(helper);
    for (machine, name) in module.machines.iter_mut().zip(["entry", "helper"]) {
        let name = names.intern(name);
        let identity = NominalDeclarationIdentity::new(name);
        machine.declaration = Some(CheckedMachineDeclaration {
            machine: identity.clone(),
            state: identity,
            semantic_declaration: name,
            callable: name,
            result_type: name,
            parameter_types: Vec::new(),
            contract: [1; 32],
            published_may_suspend: false,
            published_may_block: false,
            specialization: None,
        });
    }
    let entry = &mut module.machines[0];
    entry.structural_parameters[0].structural_type = owner;
    let mut call = Operation::unsuspended(
        cut,
        OperationResult::Unit,
        OperationKind::CallUnit {
            callee,
            arguments: vec![],
            structural_arguments: vec![],
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            claim_transfers: vec![],
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
    );
    call.suspension_crossing = Some(crossing);
    entry.blocks[0].operations.push(call);
    module
        .execution
        .suspension_call_plans
        .push(terminal_psi::TerminalSuspensionCallPlan {
            operation: cut,
            crossing,
            target: terminal_psi::TerminalSuspensionCallTarget::Machine(callee),
            live_value_count: 0,
            live_values: Vec::new(),
            loans: Default::default(),
        });
    module.identities = names.build();
    admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
        terminal_codec::terminal_psi_identity(module).unwrap(),
    );
    (program, atom)
}

fn continuation_sponsor() -> build_model::accounting::BuildEvaluationSponsor {
    use build_model::accounting::BuildEvaluationSponsor;
    use build_model::contracts::{
        BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
        TerminalStorageScheduleIdentity,
    };
    BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000).unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap()
}

#[test]
fn checked_entry_relocates_the_actual_issued_field_and_requires_one_invocation() {
    use semantic_vocabulary::{MachineId, OperationId};

    let (old, before) = continuation_fixture(false);
    let (new, after) = continuation_fixture(true);
    assert_ne!(before.value.structural_type, after.value.structural_type);
    assert_ne!(before.path, after.path);
    let site = (MachineId::new(1).unwrap(), OperationId::new(1).unwrap());
    let proof = terminal_semantics::continuation::CheckedPrefix::check(
        old.module(),
        site,
        new.module(),
        site,
    )
    .unwrap();
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let parked = |issued| {
        let mut instance = projected_accounted_instance(
            &old,
            std::slice::from_ref(&before),
            Some(account.clone()),
        )
        .unwrap();
        instance.install_compiler_observations(issued, &[]).unwrap();
        let cut = instance.select_entry_call(site.0, site.1).unwrap();
        assert!(
            instance
                .invoke_until_entry_call(&cut, &mut terminal_work::WorkMeter::with_budget(100))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            instance.borrowed_entry_host_atom(0, &before.path).unwrap(),
            Some(before.value.clone())
        );
        instance
    };
    let other = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let wrong = projected_accounted_instance(&new, &[], Some(other)).unwrap();
    assert!(
        TerminalInstance::continue_checked_entry(parked(None), wrong, &proof)
            .is_err_and(|message| message.contains("same invocation"))
    );
    let issued_target = |argument| {
        Box::new(crate::layout_target::IssuedLayoutTarget {
            facts: layout_plans::LayoutTargetFacts {
                profile: "linux_x86_64".into(),
                pointer_size: 8,
                pointer_alignment: 8,
                case_tag_size: 4,
                case_tag_alignment: 4,
                little_endian: true,
            },
            arguments: vec![argument],
        })
    };
    for on_receiver in [false, true] {
        let previous = parked((!on_receiver).then(|| issued_target(before.value.clone())));
        let mut successor = projected_accounted_instance(&new, &[], Some(account.clone())).unwrap();
        successor
            .install_compiler_observations(
                on_receiver.then(|| issued_target(after.value.clone())),
                &[],
            )
            .unwrap();
        assert!(
            TerminalInstance::continue_checked_entry(previous, successor, &proof)
                .is_err_and(|message| message.contains("separate handoff"))
        );
    }
    let successor = projected_accounted_instance(&new, &[], Some(account.clone())).unwrap();
    let mut instance =
        TerminalInstance::continue_checked_entry(parked(None), successor, &proof).unwrap();
    assert!(matches!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(100))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
    assert_eq!(
        instance.borrowed_entry_host_atom(0, &after.path).unwrap(),
        Some(after.value)
    );
}

#[test]
fn checked_entry_refuses_to_discard_the_receivers_call_observer() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, InvocationSite, OperationId, ValueId,
    };
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

    let (mut program, _) = continuation_fixture(false);
    let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
    let module = admitted.fixture_module_mut();
    let value = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: Default::default(),
    };
    let [entry, helper] = module.machines.as_mut_slice() else {
        unreachable!()
    };
    helper.parameters.push(value);
    helper.parameter_order.push(BoundaryParameterKind::Scalar);
    let operations = &mut entry.blocks[0].operations;
    let cut = operations[0].id;
    let OperationKind::CallUnit { arguments, .. } = &mut operations[0].kind else {
        unreachable!()
    };
    arguments.push(value.id);
    operations.insert(
        0,
        Operation::unsuspended(
            OperationId::new(2).unwrap(),
            OperationResult::Scalar(value),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(17),
            },
        ),
    );
    let site = (entry.id, cut);
    let selector = crate::TerminalCallObservation {
        caller: entry.id,
        site: InvocationSite::Operation(cut),
        callee: helper.id,
        argument: 0,
    };
    admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
        terminal_codec::terminal_psi_identity(module).unwrap(),
    );
    let proof = terminal_semantics::continuation::CheckedPrefix::check(
        program.module(),
        site,
        program.module(),
        site,
    )
    .unwrap();
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let instantiate =
        || projected_accounted_instance(&program, &[], Some(account.clone())).unwrap();
    let mut uninterrupted = instantiate();
    uninterrupted
        .install_compiler_observations(None, &[selector])
        .unwrap();
    uninterrupted
        .invoke(&mut terminal_work::WorkMeter::with_budget(100))
        .unwrap();
    let records = uninterrupted.take_call_observations();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].call, selector);

    let mut previous = instantiate();
    let watch = previous.select_entry_call(site.0, cut).unwrap();
    assert!(
        previous
            .invoke_until_entry_call(&watch, &mut terminal_work::WorkMeter::with_budget(100))
            .unwrap()
            .is_none()
    );
    let mut successor = instantiate();
    successor
        .install_compiler_observations(None, &[selector])
        .unwrap();
    assert!(
        TerminalInstance::continue_checked_entry(previous, successor, &proof)
            .is_err_and(|message| message.contains("separate handoff"))
    );
}

#[test]
fn a_rejoined_case_carries_its_original_scalar_payload_at_the_new_field() {
    use crate::{TerminalBoundaryStorageValue, TerminalStorageContents as Contents};
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment as Path, MachineId, OperationId, StructuralCaseId,
    };
    use terminal_psi::{StructuralCaseDeclaration, StructuralFieldType};

    fn sum(
        shift: bool,
    ) -> (
        TerminalProgram<'static>,
        StructuralCaseId,
        StructuralFieldId,
    ) {
        let (mut program, _) = continuation_fixture(shift);
        let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
        let module = admitted.fixture_module_mut();
        let owner = module.machines[0].structural_parameters[0].structural_type;
        let mut names = module.identities.clone().into_builder();
        let ty = module
            .types
            .structural_types
            .iter_mut()
            .find(|t| t.id == owner)
            .unwrap();
        let StructuralTypeShape::Record { fields, .. } = &ty.shape else {
            unreachable!()
        };
        let mut field = fields[0].clone();
        let payload = StructuralFieldId::new(if shift { 24 } else { 21 }).unwrap();
        let tag = StructuralCaseId::new(if shift { 11 } else { 9 }).unwrap();
        field.id = payload;
        field.field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
        ty.shape = StructuralTypeShape::Sum {
            cases: vec![StructuralCaseDeclaration {
                id: tag,
                identity: names.intern("present"),
                declaration_identity: names.intern("carrier::present"),
                fields: vec![field],
            }],
        };
        module.identities = names.build();
        admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
            terminal_codec::terminal_psi_identity(module).unwrap(),
        );
        (program, tag, payload)
    }

    let (old, old_case, old_field) = sum(false);
    let (new, new_case, new_field) = sum(true);
    let site = (MachineId::new(1).unwrap(), OperationId::new(1).unwrap());
    let proof = terminal_semantics::continuation::CheckedPrefix::check(
        old.module(),
        site,
        new.module(),
        site,
    )
    .unwrap();
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 200)
        .unwrap();
    let scalar = |value| Contents::Scalar {
        value: TerminalScalarValue::Boolean(value),
        stored_scalar: ScalarType::Boolean,
    };
    let bind = |program: &TerminalProgram<'static>, case, field, value| {
        let owner = program.module().machines[0].structural_parameters[0].structural_type;
        let frame = TerminalExecution::start_with_program(
            program.program.clone(),
            &[],
            crate::TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue::minted(42, owner, vec![])],
                contents: &[crate::TerminalStructuralContents {
                    argument_index: 0,
                    values: vec![
                        TerminalBoundaryStorageValue {
                            path: vec![],
                            contents: Contents::Case(case),
                        },
                        TerminalBoundaryStorageValue {
                            path: vec![Path::Case(case), Path::Field(field)],
                            contents: scalar(value),
                        },
                    ],
                }],
                ..Default::default()
            },
            None,
            Some(account.clone()),
        )
        .unwrap();
        TerminalInstance::new(program.clone(), frame, Vec::new())
    };
    let park = || {
        let mut instance = bind(&old, old_case, old_field, true);
        let cut = instance.select_entry_call(site.0, site.1).unwrap();
        assert!(
            instance
                .invoke_until_entry_call(&cut, &mut terminal_work::WorkMeter::with_budget(100))
                .unwrap()
                .is_none()
        );
        instance
    };
    let mut exposed = park();
    exposed
        .execution
        .as_mut()
        .unwrap()
        .exposed_address_selectors
        .push(crate::storage::storage_addresses::ExposedStorageSelector::Field(old_field));
    assert!(
        TerminalInstance::continue_checked_entry(
            exposed,
            bind(&new, new_case, new_field, false),
            &proof
        )
        .is_err_and(|message| message.contains("encoded in an exposed address"))
    );
    let mut instance = TerminalInstance::continue_checked_entry(
        park(),
        bind(&new, new_case, new_field, false),
        &proof,
    )
    .unwrap();
    instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(100))
        .unwrap();
    assert_eq!(
        instance.observe_storage(crate::TerminalObservationRoot::ReturnedArgument(0), &[]).unwrap(),
        TerminalStorageObservationValue::Storage(Contents::Case(new_case))
    );
    assert_eq!(
        instance
            .observe_storage(crate::TerminalObservationRoot::ReturnedArgument(0), &[Path::Case(new_case), Path::Field(new_field)])
            .unwrap(),
        TerminalStorageObservationValue::Storage(scalar(true))
    );
}

#[test]
fn a_live_byte_slice_keeps_its_original_allocation_after_publication() {
    use semantic_vocabulary::{MachineId, OperationId, StructuralPlaceKind};
    use terminal_psi::{ByteSequenceCarrier, Operation, OperationKind, OperationResult};

    let place = PlaceId::new(2).unwrap();
    let fixture = |shift| {
        let (mut program, _) = continuation_fixture(shift);
        let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
        let module = admitted.fixture_module_mut();
        let mut names = module.identities.clone().into_builder();
        let mut ty = module.types.structural_types[0].clone();
        ty.id = StructuralTypeId::new(3).unwrap();
        ty.identity = names.intern("borrowed bytes");
        ty.multiplicity = StructuralMultiplicity::Unrestricted;
        ty.no_code_affine_cleanup = false;
        ty.descriptor_cleanup = false;
        ty.shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        });
        module.identities = names.build();
        let entry = &mut module.machines[0];
        entry
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    structural_type: ty.id,
                },
            });
        module.types.structural_types.push(ty);
        entry.blocks[0].operations.insert(
            0,
            Operation::unsuspended(
                OperationId::new(2).unwrap(),
                OperationResult::Unit,
                OperationKind::EstablishByteSequenceLiteral {
                    destination: place,
                    bytes: b"prepared".to_vec(),
                    qualifications: Vec::new(),
                },
            ),
        );
        admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
            terminal_codec::terminal_psi_identity(module).unwrap(),
        );
        program
    };
    let old = fixture(false);
    let new = fixture(true);
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut before = projected_accounted_instance(&old, &[], Some(account.clone())).unwrap();
    let site = (MachineId::new(1).unwrap(), OperationId::new(1).unwrap());
    let cut = before.select_entry_call(site.0, site.1).unwrap();
    assert!(
        before
            .invoke_until_entry_call(&cut, &mut terminal_work::WorkMeter::with_budget(100))
            .unwrap()
            .is_none()
    );
    let bytes = |instance: &TerminalInstance<'_, '_>| {
        instance
            .execution
            .as_ref()
            .unwrap()
            .byte_sequence_values
            .require(&place)
            .unwrap()
            .immutable()
            .unwrap()
            .clone()
    };
    let slice = bytes(&before).subslice(3, 7).unwrap();
    let prefix = terminal_semantics::continuation::CheckedPrefix::check(
        old.module(),
        site,
        new.module(),
        site,
    )
    .unwrap();
    let receiver = projected_accounted_instance(&new, &[], Some(account)).unwrap();
    let resumed = TerminalInstance::continue_checked_entry(before, receiver, &prefix).unwrap();
    let other = bytes(&resumed).subslice(3, 7).unwrap();
    assert_eq!(other.bytes(), b"pare");
    assert!(other == slice);
}

fn nested_continuation_fixture(
    shift: bool,
) -> (TerminalProgram<'static>, crate::TerminalProjectedHostAtom) {
    use semantic_vocabulary::{MachineId, OperationId, SuspensionCrossingId};
    use terminal_psi::OperationKind;
    let helper = MachineId::new(2).unwrap();
    let leaf = MachineId::new(3).unwrap();
    let second = OperationId::new(2).unwrap();
    let inner = OperationId::new(3).unwrap();
    let (mut program, atom) = continuation_fixture(shift);
    let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
    let module = admitted.fixture_module_mut();
    let mut leaf_machine = module.machines[1].clone();
    leaf_machine.id = leaf;
    let mut names = module.identities.clone().into_builder();
    let name = names.intern("inner suspension");
    let declaration = leaf_machine.declaration.as_mut().unwrap();
    declaration.machine = terminal_psi::NominalDeclarationIdentity::new(name);
    declaration.state = terminal_psi::NominalDeclarationIdentity::new(name);
    declaration.semantic_declaration = name;
    declaration.callable = name;
    declaration.result_type = name;
    module.identities = names.build();
    module.machines.push(leaf_machine);
    let prototype = module.machines[0].blocks[0].operations[0].clone();
    let plan = module.execution.suspension_call_plans[0].clone();
    for (owner, target, id, crossing) in [(0, helper, second, 2), (1, leaf, inner, 3)] {
        let mut op = prototype.clone();
        op.id = id;
        let crossing = SuspensionCrossingId::new(crossing).unwrap();
        op.suspension_crossing = Some(crossing);
        let OperationKind::CallUnit { callee, .. } = &mut op.kind else {
            unreachable!()
        };
        *callee = target;
        module.machines[owner].blocks[0].operations.push(op);
        module
            .execution
            .suspension_call_plans
            .push(terminal_psi::TerminalSuspensionCallPlan {
                operation: id,
                crossing,
                target: terminal_psi::TerminalSuspensionCallTarget::Machine(target),
                ..plan.clone()
            });
    }
    admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
        terminal_codec::terminal_psi_identity(module).unwrap(),
    );
    (program, atom)
}

#[test]
fn an_unfinished_helper_returns_its_scalar_only_in_the_checked_successor() {
    use semantic_vocabulary::{MachineId, OperationId, ValueId};
    use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};
    let root = MachineId::new(1).unwrap();
    let helper = MachineId::new(2).unwrap();
    let first = OperationId::new(1).unwrap();
    let second = OperationId::new(2).unwrap();
    let wait = OperationId::new(3).unwrap();
    let boolean = |id| ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    };
    let (fixture, _) = nested_continuation_fixture(false);
    let mut module = fixture.module().clone();
    let mut names = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
    for name in module.identities.entries() {
        names.intern(name);
    }
    let result_type = names.intern("bool");
    module.identities = names.build();
    let entry = &mut module.machines[0];
    for (i, op) in entry.blocks[0].operations.iter_mut().enumerate() {
        op.kind = OperationKind::CallStructuralScalar {
            callee: helper,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        };
        op.result = OperationResult::Scalar(boolean(i as u64 + 1));
    }
    entry.result = TerminalMachineResult::Scalar(boolean(2));
    entry.declaration.as_mut().unwrap().result_type = result_type;
    let returning = |value| Terminator::Return {
        edge: EdgeId::new(1).unwrap(),
        value,
        cleanup_actions: Vec::new(),
    };
    entry.blocks[0].terminator = returning(boolean(2).id);
    module.machines[1].result = TerminalMachineResult::Scalar(boolean(1));
    module.machines[1].declaration.as_mut().unwrap().result_type = result_type;
    module.machines[1].blocks[0].terminator = Terminator::PendingNormalCompletion {
        call: wait,
        scalars: Vec::new(),
        storage: Vec::new(),
    };
    let open = terminal_codec::InvocationTerminalCode::from_prefix(module.clone()).unwrap();
    let before = TerminalProgram::retain(open.retain_code());
    assert!(before.closed_module().is_err());
    assert!(before.closed_types().is_ok());
    let block = &mut module.machines[1].blocks[0];
    block.operations.push(Operation::unsuspended(
        OperationId::new(4).unwrap(),
        OperationResult::Scalar(boolean(1)),
        OperationKind::BooleanConstant { value: true },
    ));
    block.terminator = returning(boolean(1).id);
    let after = TerminalProgram::retain(
        terminal_codec::InvocationTerminalCode::new(module)
            .unwrap()
            .retain_code(),
    );
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let instantiate =
        |program| projected_accounted_instance(program, &[], Some(account.clone())).unwrap();
    let mut meter = terminal_work::WorkMeter::with_budget(100);
    let mut raw = instantiate(&before);
    let mut handler = crate::embedding::instance::ImportDispatch {
        bindings: &mut raw.bindings,
        storage_resolver: None,
    };
    assert!(
        raw.execution
            .as_mut()
            .unwrap()
            .resume(&mut meter, &mut handler)
            .is_err()
    );
    let mut previous = instantiate(&before);
    assert!(previous.invoke(&mut meter).is_err());
    assert!(!previous.invoked);
    let incomplete = previous.select_entry_call(root, first).unwrap();
    assert!(
        previous
            .invoke_until_entry_call(&incomplete, &mut meter)
            .is_err()
    );
    let mut other = instantiate(&before);
    let wrong = other
        .select_call_stack(&[(root, second), (helper, wait)])
        .unwrap();
    let err = other
        .invoke_until_entry_call(&wrong, &mut meter)
        .unwrap_err();
    assert!(err.to_string().contains("missing result"), "{err}");
    let cut = previous
        .select_call_stack(&[(root, first), (helper, wait)])
        .unwrap();
    assert!(
        previous
            .invoke_until_entry_call(&cut, &mut meter)
            .unwrap()
            .is_none()
    );
    assert!(previous.invoke(&mut meter).is_err());
    let stack = previous.completed_call_stack().unwrap();
    let proof = terminal_semantics::continuation::CheckedPrefix::check_stack(
        before.module(),
        &stack,
        after.module(),
        (helper, wait),
    )
    .unwrap();
    let mut resumed =
        TerminalInstance::continue_checked_entry(previous, instantiate(&after), &proof).unwrap();
    assert!(matches!(
        resumed.invoke(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    ));
}

#[test]
fn nested_continuation_resumes_the_second_caller_with_its_issued_field() {
    use semantic_vocabulary::{MachineId, OperationId};
    use terminal_semantics::continuation::CheckedPrefix;

    let root = MachineId::new(1).unwrap();
    let helper = MachineId::new(2).unwrap();
    let first = OperationId::new(1).unwrap();
    let second = OperationId::new(2).unwrap();
    let inner = OperationId::new(3).unwrap();
    let (old, before) = nested_continuation_fixture(false);
    let (new, after) = nested_continuation_fixture(true);
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let park = |caller| {
        let mut instance = projected_accounted_instance(
            &old,
            std::slice::from_ref(&before),
            Some(account.clone()),
        )
        .unwrap();
        let cut = instance
            .select_call_stack(&[(root, caller), (helper, inner)])
            .unwrap();
        assert!(
            instance
                .invoke_until_entry_call(&cut, &mut terminal_work::WorkMeter::with_budget(100))
                .unwrap()
                .is_none()
        );
        instance
    };
    let first_stack = park(first).completed_call_stack().unwrap();
    let second_stack = park(second).completed_call_stack().unwrap();
    assert_eq!(
        second_stack
            .iter()
            .map(|site| site.operation)
            .collect::<Vec<_>>(),
        [second, inner]
    );
    let wrong =
        CheckedPrefix::check_stack(old.module(), &first_stack, new.module(), (helper, inner))
            .unwrap();
    let receive = || projected_accounted_instance(&new, &[], Some(account.clone())).unwrap();
    assert!(TerminalInstance::continue_checked_entry(park(second), receive(), &wrong).is_err());
    let proof =
        CheckedPrefix::check_stack(old.module(), &second_stack, new.module(), (helper, inner))
            .unwrap();
    let mut instance =
        TerminalInstance::continue_checked_entry(park(second), receive(), &proof).unwrap();
    instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(100))
        .unwrap();
    assert_eq!(
        instance.borrowed_entry_host_atom(0, &after.path).unwrap(),
        Some(after.value)
    );
}

#[test]
fn a_projected_helper_cannot_return_to_its_omitted_suffix_from_another_caller() {
    use semantic_vocabulary::{MachineId, OperationId};
    let root = MachineId::new(1).unwrap();
    let helper = MachineId::new(2).unwrap();
    let first = OperationId::new(1).unwrap();
    let second = OperationId::new(2).unwrap();
    let inner = OperationId::new(3).unwrap();
    let (program, _) = nested_continuation_fixture(false);
    let run = |caller, projected: &[bool]| {
        let mut instance = projected_instance(&program, &[]).unwrap();
        let cut =
            instance.select_projected_call_stack(&[(root, caller), (helper, inner)], projected)?;
        instance.invoke_until_entry_call(&cut, &mut terminal_work::WorkMeter::with_budget(100))
    };
    assert!(run(first, &[true, true]).unwrap().is_none());
    assert!(run(second, &[true, false]).unwrap().is_none());
    let error = run(second, &[false, true]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("a call reached an omitted continuation"),
        "{error}"
    );
    assert!(
        run(first, &[true])
            .unwrap_err()
            .to_string()
            .contains("projection flags")
    );
}

fn branch_before_calls(
    machine: &mut terminal_psi::TerminalMachine,
    flag: bool,
    first: Vec<terminal_psi::Operation>,
    second: Vec<terminal_psi::Operation>,
) {
    use semantic_vocabulary::{BlockId, OperationId, ValueId};
    use terminal_psi::{
        Operation, OperationKind, OperationResult, SuccessorEdge, ValueDeclaration,
    };
    let mut left = machine.blocks[0].clone();
    left.id = BlockId::new(2).unwrap();
    let mut right = left.clone();
    right.id = BlockId::new(3).unwrap();
    left.operations = first;
    right.operations = second;
    let condition = ValueId::new(1).unwrap();
    let start = &mut machine.blocks[0];
    start.operations = vec![Operation::unsuspended(
        OperationId::new(4).unwrap(),
        OperationResult::Scalar(ValueDeclaration {
            id: condition,
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        OperationKind::BooleanConstant { value: flag },
    )];
    let edge = |id, target| SuccessorEdge {
        edge: EdgeId::new(id).unwrap(),
        target,
        arguments: vec![],
        erased_arguments: vec![],
        erased_proof_arguments: vec![],
        structural_arguments: vec![],
        cleanup_actions: vec![],
    };
    start.terminator = Terminator::Conditional {
        condition,
        when_true: edge(2, left.id),
        when_false: edge(3, right.id),
    };
    machine.blocks.extend([left, right]);
}

#[test]
fn alternative_preparations_select_the_actual_caller_before_guarding_its_shared_helper() {
    use semantic_vocabulary::{MachineId, OperationId};
    let root = MachineId::new(1).unwrap();
    let helper = MachineId::new(2).unwrap();
    let first = OperationId::new(1).unwrap();
    let second = OperationId::new(2).unwrap();
    let wait = OperationId::new(3).unwrap();
    for flag in [true, false] {
        let (mut program, _) = nested_continuation_fixture(false);
        let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
        let module = admitted.fixture_module_mut();
        let entry = &mut module.machines[0];
        let mut left = std::mem::take(&mut entry.blocks[0].operations);
        let right = left.split_off(1);
        branch_before_calls(entry, flag, left, right);
        admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
            terminal_codec::terminal_psi_identity(module).unwrap(),
        );
        let mut instance = projected_instance(&program, &[]).unwrap();
        let arm = |caller, projected| {
            instance
                .select_projected_call_stack(&[(root, caller), (helper, wait)], &[false, projected])
                .unwrap()
        };
        assert!(
            instance
                .select_entry_calls(vec![arm(first, true), arm(first, true)])
                .is_err()
        );
        assert!(
            instance
                .select_entry_calls(vec![arm(first, false), arm(second, true)])
                .is_err()
        );
        let outer = instance.select_entry_call(root, first).unwrap();
        assert!(
            instance
                .select_entry_calls(vec![outer, arm(first, true)])
                .is_err()
        );
        let unmatched = instance
            .select_entry_calls(vec![
                instance.select_entry_call(root, first).unwrap(),
                arm(second, true),
            ])
            .unwrap();
        let both = instance
            .select_entry_calls(vec![arm(second, true), arm(first, true)])
            .unwrap();
        let mut meter = terminal_work::WorkMeter::with_budget(100);
        assert!(matches!(
            instance
                .invoke_until_entry_calls(&both, &mut terminal_work::WorkMeter::with_budget(0))
                .unwrap(),
            Some(TerminalExecutionStatus::SponsorExhausted(_))
        ));
        assert!(instance.invoke(&mut meter).is_err());
        assert_eq!(instance.completed_call_path_index(), None);
        assert!(
            instance
                .invoke_until_entry_calls(&both, &mut meter)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            instance.completed_call_path_index(),
            Some(usize::from(flag))
        );
        let stack = instance.completed_call_stack().unwrap();
        assert_eq!(stack[0].operation, if flag { first } else { second });
        assert_eq!(stack[1].operation, wait);
        let err = instance.invoke(&mut meter).unwrap_err();
        assert!(err.to_string().contains("must keep guarding"), "{err}");
        assert_eq!(instance.completed_call_stack().unwrap(), stack);
        if flag {
            let mut other = projected_instance(&program, &[]).unwrap();
            let err = other
                .invoke_until_entry_calls(&unmatched, &mut meter)
                .unwrap_err();
            assert!(
                err.to_string()
                    .contains("a call reached an omitted continuation"),
                "{err}"
            );
            assert_eq!(other.completed_call_path_index(), None);
        }
    }
}

#[test]
fn a_sealed_unit_entry_can_skip_preparation_but_cannot_return_through_an_open_tail() {
    use semantic_vocabulary::{MachineId, OperationId};
    let entry = MachineId::new(1).unwrap();
    let wait = OperationId::new(1).unwrap();
    for flag in [true, false] {
        let (fixture, _) = continuation_fixture(false);
        let mut module = fixture.module().clone();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
        for name in module.identities.entries() {
            identities.intern(name);
        }
        module.identities = identities.build();
        let machine = &mut module.machines[0];
        machine.structural_parameters.clear();
        machine.parameter_order.clear();
        machine.structural_places.clear();
        let waits = std::mem::take(&mut machine.blocks[0].operations);
        branch_before_calls(machine, flag, waits, Vec::new());
        machine.blocks[1].terminator = Terminator::PendingNormalCompletion {
            call: wait,
            scalars: Vec::new(),
            storage: Vec::new(),
        };
        let open = terminal_codec::InvocationTerminalCode::from_prefix(module).unwrap();
        let program = TerminalProgram::retain(open.retain_code());
        let mut instance = projected_instance(&program, &[]).unwrap();
        let mut meter = terminal_work::WorkMeter::with_budget(100);
        let err = instance.invoke(&mut meter).unwrap_err();
        assert!(
            err.to_string()
                .contains("must arm every pending normal completion"),
            "{err}"
        );
        let cut = instance.select_entry_call(entry, wait).unwrap();
        let status = instance.invoke_until_entry_call(&cut, &mut meter).unwrap();
        if flag {
            assert_eq!(status, None);
            let before = instance.completed_call_stack().unwrap();
            assert!(instance.invoke(&mut meter).is_err());
            assert_eq!(instance.completed_call_stack().unwrap(), before);
        } else {
            let completed = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
            assert_eq!(status.as_ref(), Some(&completed));
            assert_eq!(instance.invoke(&mut meter).unwrap(), completed);
            assert_eq!(instance.completed_call_path_index(), None);
        }
    }
}

#[test]
fn returning_from_a_branch_without_waiting_requires_a_complete_caller() {
    use semantic_vocabulary::{MachineId, OperationId};
    let (mut program, _) = nested_continuation_fixture(false);
    let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
    let module = admitted.fixture_module_mut();
    let helper = &mut module.machines[1];
    let waits = std::mem::take(&mut helper.blocks[0].operations);
    branch_before_calls(helper, false, waits, Vec::new());
    admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
        terminal_codec::terminal_psi_identity(module).unwrap(),
    );
    let chain = [
        (MachineId::new(1).unwrap(), OperationId::new(1).unwrap()),
        (MachineId::new(2).unwrap(), OperationId::new(3).unwrap()),
    ];
    let run = |projected: &[bool]| {
        let mut instance = projected_instance(&program, &[]).unwrap();
        let cut = instance
            .select_projected_call_stack(&chain, projected)
            .unwrap();
        let mut work = terminal_work::WorkMeter::with_budget(100);
        let result = instance.invoke_until_entry_call(&cut, &mut work)?;
        if let Some(TerminalExecutionStatus::Complete(value)) = &result {
            assert_eq!(
                instance.invoke(&mut work)?,
                TerminalExecutionStatus::Complete(value.clone())
            );
        }
        Ok::<_, crate::TerminalInstanceError>(result)
    };
    assert!(matches!(
        run(&[false, true]).unwrap(),
        Some(TerminalExecutionStatus::Complete(
            TerminalExecutionResult::Unit
        ))
    ));
    let error = run(&[true, false]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("a call reached an omitted continuation"),
        "{error}"
    );
}

#[test]
fn a_suspended_helper_returns_a_borrow_of_the_same_authored_field() {
    use semantic_vocabulary::{MachineId, OperationId, StructuralPlaceKind};
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralArgument, StructuralOperationResult,
        StructuralPlaceDeclaration, StructuralReferenceResultSource,
    };

    let root = MachineId::new(1).unwrap();
    let helper = MachineId::new(2).unwrap();
    let caller = OperationId::new(2).unwrap();
    let wait = OperationId::new(3).unwrap();
    let borrow = OperationId::new(4).unwrap();
    let fixture = |shift| {
        let (mut program, atom) = nested_continuation_fixture(shift);
        let admitted = std::sync::Arc::get_mut(&mut program.program).unwrap();
        let module = admitted.fixture_module_mut();
        let mut names = module.identities.clone().into_builder();
        let shared = StructuralAccess::SharedBorrow;
        let mut ty = module.types.structural_types[0].clone();
        ty.id = StructuralTypeId::new(3).unwrap();
        ty.identity = names.intern("borrow of issued field");
        ty.multiplicity = StructuralMultiplicity::Unrestricted;
        ty.shape = StructuralTypeShape::Reference {
            referent: atom.value.structural_type,
            access: shared,
        };
        let result = StructuralOperationResult {
            place: PlaceId::new(2).unwrap(),
            structural_type: ty.id,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        module.types.structural_types.push(ty);
        module.identities = names.build();
        let [entry, child, ..] = module.machines.as_mut_slice() else {
            unreachable!()
        };
        let mut parameter = entry.structural_parameters[0].clone();
        parameter.access = shared;
        let argument = StructuralArgument {
            place: parameter.place,
            path: Vec::new(),
            access: shared,
        };
        let field = StructuralArgument {
            path: atom.path.clone(),
            ..argument.clone()
        };
        child.structural_parameters = vec![parameter];
        child.parameter_order = entry.parameter_order.clone();
        child.structural_places = entry.structural_places.clone();
        child.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: result.place,
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![StructuralReferenceResultSource {
                path: Vec::new(),
                sources: vec![field.clone().into()],
            }],
        });
        for (machine, producer) in [(&mut *entry, caller), (&mut *child, borrow)] {
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: result.place,
                kind: StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: result.structural_type,
                },
            });
        }
        let operations = &mut entry.blocks[0].operations;
        let first = operations.remove(0).id;
        module
            .execution
            .suspension_call_plans
            .retain(|plan| plan.operation != first);
        operations[0].result = OperationResult::Structural(result.clone());
        operations[0].kind = OperationKind::CallStructural {
            callee: helper,
            structural_arguments: vec![argument],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        };
        operations.push(Operation::unsuspended(
            OperationId::new(5).unwrap(),
            OperationResult::Unit,
            OperationKind::ReleaseReference {
                source: result.place,
                path: Vec::new(),
            },
        ));
        child.blocks[0].terminator = Terminator::ReturnStructural {
            edge: EdgeId::new(1).unwrap(),
            source: result.place,
            returned_claims: Vec::new(),
            cleanup_actions: Vec::new(),
        };
        child.blocks[0].operations.push(Operation::unsuspended(
            borrow,
            OperationResult::Structural(result),
            OperationKind::EstablishReference {
                source: field,
                storage: None,
            },
        ));
        admitted.identity = terminal_codec::TerminalExecutionIdentity::portable(
            terminal_codec::terminal_psi_identity(module).unwrap(),
        );
        (program, atom)
    };
    let (old, before) = fixture(false);
    let (new, after) = fixture(true);
    let sponsor = continuation_sponsor();
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut previous =
        projected_accounted_instance(&old, &[before], Some(account.clone())).unwrap();
    let cut = previous
        .select_call_stack(&[(root, caller), (helper, wait)])
        .unwrap();
    assert!(
        previous
            .invoke_until_entry_call(&cut, &mut terminal_work::WorkMeter::with_budget(100))
            .unwrap()
            .is_none()
    );
    let proof = terminal_semantics::continuation::CheckedPrefix::check_stack(
        old.module(),
        &previous.completed_call_stack().unwrap(),
        new.module(),
        (helper, wait),
    )
    .unwrap();
    let receiver = projected_accounted_instance(&new, &[], Some(account)).unwrap();
    let mut resumed = TerminalInstance::continue_checked_entry(previous, receiver, &proof).unwrap();
    assert!(matches!(
        resumed
            .invoke(&mut terminal_work::WorkMeter::with_budget(100))
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
    assert_eq!(
        resumed.borrowed_entry_host_atom(0, &after.path).unwrap(),
        Some(after.value)
    );
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
