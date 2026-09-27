use super::{
    INDEXED_CUSTODY_SOURCE, ORDINARY_INDEXED_CUSTODY_SOURCE, RESULT_BOUNDARY_BOUNDED_REACH_SOURCE,
    RESULT_BOUNDARY_CONTENT_CUSTODY_SOURCE, RejectSecondEffect, ResultBoundaryHandler,
    checked_result_boundary_source, checked_source,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{ContentAlgebraKind, ContentPlaceVersion};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue, TerminalStructuralResult, TerminalStructuralValue,
};
use terminal_psi::TerminalMachineResult;

#[test]
fn result_bearing_boundary_rejects_missing_canonical_contract_custody() {
    let mut checked = checked_result_boundary_source();
    let boundary = checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .boundary_machines[0]
        .machine;
    checked
        .facts
        .contract_plans
        .machines
        .retain(|contract| contract.machine != boundary);
    checked
        .facts
        .contract_plans
        .crash_capsules
        .retain(|capsule| capsule.target_machine() != boundary);

    assert_eq!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Root::enter")
        ),
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
            "result-bearing boundary target is missing its canonical contract identity",
        )),
    );
}

#[test]
fn result_bearing_boundary_rejects_compact_equal_commitment_substitution() {
    let mut checked = checked_result_boundary_source();
    let boundary = &mut checked
        .facts
        .flow
        .terminal_boundary_scalar_returns
        .boundary_machines[0];
    let retained_report = boundary.contract_report_fingerprint;
    boundary.contract_commitment =
        checked_trees::MachineContractCommitment::from_digest([0x5a; 32]);
    assert_eq!(boundary.contract_report_fingerprint, retained_report);

    assert_eq!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Root::enter")
        ),
        Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
            "result-bearing boundary target contract compatibility coordinate or strong commitment drifted",
        )),
    );
}

#[test]
fn source_content_custody_exit_retains_projection_and_commits_only_after_success() {
    let checked = crate::front_end::checked_program(RESULT_BOUNDARY_CONTENT_CUSTODY_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("content-bearing boundary custody should lower");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("one content custody root machine")
    };
    let [structural_claim] = machine.entry_claims.as_slice() else {
        panic!("one structural entry claim")
    };
    let [content_claim] = machine.content_entry_claims.as_slice() else {
        panic!("one content entry claim")
    };
    assert_eq!(content_claim.claim, structural_claim.claim);
    assert_eq!(content_claim.input.version, ContentPlaceVersion::Entry);
    assert_eq!(content_claim.input.root, structural_claim.input);
    assert!(content_claim.input.segments.is_empty());
    let [projection] = content_claim.projections.as_slice() else {
        panic!("one owner-unique content projection")
    };
    assert_eq!(projection.algebra.kind, ContentAlgebraKind::CountedQuantity);
    assert_ne!(projection.projection.projection_report_fingerprint, 0);
    let owner_projection = module.structural_domains[0]
        .content_projection
        .as_ref()
        .expect(
            "content-bearing domain retains its owner projection independently of installation",
        );
    assert_eq!(owner_projection.identity, projection.projection);
    assert_eq!(owner_projection.algebra, projection.algebra);

    let semantic = encode_module(module).expect("content custody semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("content custody semantics decode"),
        *module
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("content custody proof encodes");
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("content-bearing boundary custody verifies");

    let mut drifted = module.clone();
    drifted.machines[0].content_entry_claims[0].input.root =
        semantic_vocabulary::PlaceId::new(structural_claim.input.get() + 1)
            .expect("different place");
    assert!(matches!(
        terminal_verifier::validate_module_representation(&drifted),
        Err(terminal_verifier::ModuleError::ContentEntryClaimRequiresEntryParameter(_))
            | Err(terminal_verifier::ModuleError::ContentEntryClaimStructuralBindingMismatch(_))
    ));

    let parameter = &machine.structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 0x0c01_7e17,
                structural_type: parameter.structural_type,
                qualifications: parameter.qualifications.clone(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("content custody artifact starts");
    let initial_claims = execution.live_claim_frontier().collect::<Vec<_>>();
    assert_eq!(initial_claims, [content_claim.claim]);
    let mut meter = TerminalFuelMeter::unbounded();
    let mut rejecting = ResultBoundaryHandler { reject: true };
    assert!(matches!(
        execution.resume(&mut meter, &mut rejecting),
        Err(TerminalInterpretError::EffectRejected { .. })
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        initial_claims
    );
    assert!(execution.effects().is_empty());

    let mut accepting = ResultBoundaryHandler { reject: false };
    assert_eq!(
        execution
            .resume(&mut meter, &mut accepting)
            .expect("accepted content exit resumes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}

#[test]
fn result_bearing_boundary_retains_exact_bounded_installation_reach() {
    let checked = crate::front_end::checked_program(RESULT_BOUNDARY_BOUNDED_REACH_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("bounded result boundary should lower");
    let module = &lowered.semantic_module;

    // The entry declares the services its explicit invocation may reach, and
    // the invoked requirement keeps its own bounded installation dependency.
    let service_name = |service: &semantic_vocabulary::ServiceId| {
        module
            .services
            .iter()
            .find(|declaration| declaration.id == *service)
            .expect("reached service is declared")
            .identity
            .as_str()
    };
    let concrete = module
        .root_service_reach
        .concrete
        .iter()
        .map(service_name)
        .collect::<Vec<_>>();
    assert_eq!(
        concrete,
        ["InterruptCompletion", "MachineControl", "PortIo"]
    );
    let [dependency] = module
        .root_service_reach
        .installation_dependencies
        .as_slice()
    else {
        panic!("result-bearing root must retain one installation dependency")
    };
    assert!(
        dependency
            .requirement_identity
            .contains("InterruptCompletion::complete")
    );
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|service| {
            module
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .expect("bounded service is declared")
                .identity
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);

    let semantic = encode_module(module).expect("bounded result boundary semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("bounded result boundary semantics decode"),
        *module
    );
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("bounded result boundary verifies");

    let mut missing = module.clone();
    missing.root_service_reach.concrete.pop();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&missing),
        Err(terminal_verifier::ModuleError::RootConcreteServiceReachMismatch { .. })
    ));

    let mut drifted = module.clone();
    drifted.root_service_reach.installation_dependencies[0]
        .upper_bound
        .pop();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&drifted),
        Err(terminal_verifier::ModuleError::InstallationReachBoundaryMismatch(_))
    ));

    let mut padded = module.clone();
    let duplicate = *padded.root_service_reach.installation_dependencies[0]
        .upper_bound
        .last()
        .expect("bounded row is nonempty");
    padded.root_service_reach.installation_dependencies[0]
        .upper_bound
        .push(duplicate);
    assert!(matches!(
        terminal_verifier::validate_module_representation(&padded),
        Err(terminal_verifier::ModuleError::DuplicatePublishedService { .. })
    ));

    let mut duplicate_dependency = module.clone();
    duplicate_dependency
        .root_service_reach
        .installation_dependencies
        .push(
            duplicate_dependency
                .root_service_reach
                .installation_dependencies[0]
                .clone(),
        );
    assert_eq!(
        terminal_verifier::validate_module_representation(&duplicate_dependency),
        Err(terminal_verifier::ModuleError::InvalidInstallationReachDependency(1))
    );

    let mut unused_dependency = module.clone();
    let mut unused = unused_dependency
        .root_service_reach
        .installation_dependencies[0]
        .clone();
    unused.requirement_identity = "zzzz::unused_completion".into();
    unused_dependency
        .root_service_reach
        .installation_dependencies
        .push(unused);
    assert_eq!(
        terminal_verifier::validate_module_representation(&unused_dependency),
        Err(terminal_verifier::ModuleError::RootInstallationReachDependenciesMismatch)
    );
}

#[test]
fn literal_fixed_array_custody_reaches_verified_interpreted_terminal_psi() {
    let checked = crate::front_end::checked_program(INDEXED_CUSTODY_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("literal fixed-array custody should lower");
    let module = &lowered.semantic_module;
    let machine = module.machines.first().expect("one root machine");
    assert_eq!(machine.entry_claims.len(), 2);
    assert_eq!(
        machine.entry_claims[0].path,
        [terminal_psi::StructuralPathSegment::FixedIndex(0)]
    );
    assert_eq!(
        machine.entry_claims[1].path,
        [terminal_psi::StructuralPathSegment::FixedIndex(1)]
    );
    let [first, second] = machine.blocks[0].operations.as_slice() else {
        panic!("two indexed settlements")
    };
    for (operation, index) in [(first, 0), (second, 1)] {
        let terminal_psi::OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            panic!("indexed boundary settlement")
        };
        assert_eq!(
            structural_arguments[0].path,
            [terminal_psi::StructuralPathSegment::FixedIndex(index)]
        );
        assert_eq!(completion_receipts.len(), 1);
    }

    let semantic = encode_module(module).expect("indexed semantics encode");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("indexed proof encodes");
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("indexed custody verifies");
    let mut incomplete = module.clone();
    incomplete.machines[0].entry_claims.pop();
    assert!(matches!(
        terminal_verifier::validate_module_representation(&incomplete),
        Err(
            terminal_verifier::ModuleError::IncompleteFixedArrayEntryClaims {
                machine: invalid_machine,
                place: invalid_place,
            }
        ) if invalid_machine == machine.id && invalid_place == machine.structural_parameters[0].place
    ));
    let parameter = &machine.structural_parameters[0];
    let argument = TerminalStructuralValue {
        opaque_identity: 0x51b1,
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    )
    .expect("indexed custody artifact starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let effects = execution.effects();
    assert_eq!(effects.len(), 2);
    for (effect, index) in effects.iter().zip([0, 1]) {
        let terminal_interpreter::TerminalEffect::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = effect
        else {
            panic!("indexed boundary effect")
        };
        assert_eq!(
            structural_arguments[0].path,
            [terminal_psi::StructuralPathSegment::FixedIndex(index)]
        );
        assert_eq!(completion_receipts.len(), 1);
    }

    let mut rejected_execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 0x51b2,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("indexed custody rejection artifact starts");
    let mut rejecting = RejectSecondEffect::default();
    let mut meter = TerminalFuelMeter::unbounded();
    assert!(matches!(
        rejected_execution.resume(&mut meter, &mut rejecting),
        Err(TerminalInterpretError::EffectRejected { operation, .. })
            if operation == second.id
    ));
    assert_eq!(rejecting.accepted, 1);
    assert_eq!(rejected_execution.effects().len(), 1);
    assert_eq!(
        rejected_execution.live_claim_frontier().collect::<Vec<_>>(),
        [machine.entry_claims[1].claim]
    );
}

#[test]
fn literal_fixed_array_custody_crosses_ordinary_unit_calls_without_losing_siblings() {
    let checked = crate::front_end::checked_program(ORDINARY_INDEXED_CUSTODY_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("ordinary literal fixed-index custody should lower");
    let root = &lowered.semantic_module.machines[0];
    let [first, second] = root.blocks[0].operations.as_slice() else {
        panic!("root should call the helper once per sibling")
    };
    for (operation, index) in [(first, 0), (second, 1)] {
        let terminal_psi::OperationKind::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = &operation.kind
        else {
            panic!("indexed ordinary call")
        };
        assert_eq!(
            structural_arguments[0].path,
            [terminal_psi::StructuralPathSegment::FixedIndex(index)]
        );
        assert_eq!(
            claim_transfers,
            &[terminal_psi::ClaimTransfer {
                claim: root.entry_claims[index as usize].claim,
                argument_index: 0,
            }]
        );
    }

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encode");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier rebases the exact indexed caller claims");
    let mut wrong_claim = lowered.semantic_module.clone();
    let terminal_psi::OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut wrong_claim.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers[0].claim = root.entry_claims[1].claim;
    assert!(terminal_verifier::validate_module_representation(&wrong_claim).is_err());

    let mut nested_path = lowered.semantic_module.clone();
    let terminal_psi::OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut nested_path.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(terminal_psi::StructuralPathSegment::FixedIndex(0));
    assert!(matches!(
        terminal_verifier::validate_module_representation(&nested_path),
        Err(terminal_verifier::ModuleError::InvalidStructuralArgumentPath { .. })
    ));

    let mut wrong_index = lowered.semantic_module.clone();
    let terminal_psi::OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut wrong_index.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(1)];
    assert!(terminal_verifier::validate_module_representation(&wrong_index).is_err());

    let parameter = &root.structural_parameters[0];
    let argument = TerminalStructuralValue {
        opaque_identity: 0x51b3,
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[argument],
            ..Default::default()
        },
    )
    .expect("ordinary indexed artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        [root.entry_claims[1].claim],
        "returning from the first helper must restore the untouched sibling claim"
    );
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.effects().len(), 2);
}

#[test]
fn structural_return_cleans_locals_then_every_affine_tail_parameter_in_reverse_order() {
    let checked = checked_source();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine
                    && machine.name.as_str() == "Main::forward_with_local_and_drop_two"
            })
        })
        .expect("checker should publish the complete affine cleanup tail");
    assert_eq!(plan.structural_parameters.len(), 3);
    assert_eq!(plan.trivial_affine_discards, [2, 1]);

    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::forward_with_local_and_drop_two"),
    )
    .expect("multiple affine tail parameters should lower");
    let machine = &lowered.semantic_module.machines[0];
    let terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination: local } =
        machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let terminal_psi::Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        trivial_affine_discards,
        &[
            local,
            machine.structural_parameters[2].place,
            machine.structural_parameters[1].place,
        ]
    );

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the complete reverse cleanup tail");

    let TerminalMachineResult::Structural(result) = &machine.result else {
        unreachable!()
    };
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0x5eed + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("artifact starts with every structural parameter");
    let mut meter = TerminalFuelMeter::with_allowance(1);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: TerminalStructuralValue {
                    opaque_identity: arguments[0].opaque_identity,
                    structural_type: result.structural_type,
                    qualifications: result.qualifications.clone(),
                    path: Vec::new(),
                },
                claims: vec![machine.entry_claims[0].claim],
            }
        ))
    );
}

#[test]
fn affine_local_composes_with_claim_bearing_state_transition() {
    // `local_control` left the exact affine-local slice when the composed
    // state-graph route learned to replay each edge's checked Transfer event:
    // the `in Owned` claim on `region` binds the successor's parameter place
    // instead of becoming a fresh block claim, so the machine lowers, verifies,
    // and interprets rather than rejecting.
    let checked = checked_source();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::local_control"),
    )
    .expect("claim-bearing state transition lowers through the composed graph");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one composed machine")
    };
    let [entry_block, next_block] = machine.blocks.as_slice() else {
        panic!("entry and next blocks")
    };
    let terminal_psi::OperationKind::EstablishRecord { fields } = &entry_block.operations[0].kind
    else {
        panic!("trivial affine local establishment")
    };
    assert!(fields.is_empty());
    let local_place = machine
        .structural_places
        .iter()
        .find(|place| {
            matches!(
                place.kind,
                semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
            )
        })
        .expect("local operation result place");
    let terminal_psi::Terminator::Jump {
        target,
        trivial_affine_discards,
        ..
    } = &entry_block.terminator
    else {
        panic!("unconditional transition")
    };
    assert_eq!(*target, next_block.id);
    assert_eq!(trivial_affine_discards, &[local_place.id]);
    let terminal_psi::Terminator::ReturnStructural {
        source,
        returned_claims,
        ..
    } = &next_block.terminator
    else {
        panic!("structural return")
    };
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[machine.entry_claims[0].claim]);

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("proof encodes");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("claim transport verifies through the transition");

    let TerminalMachineResult::Structural(result) = &machine.result else {
        unreachable!()
    };
    let argument = TerminalStructuralValue {
        opaque_identity: 0x10ca1,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: std::slice::from_ref(&argument),
            ..Default::default()
        },
    )
    .expect("artifact starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![machine.entry_claims[0].claim],
            }
        ))
    );
}

#[test]
fn lowering_rejects_stale_affine_local_declaration_and_cleanup_rows() {
    fn checked_plan() -> (checked_trees::CheckedTrees, symbols::SymbolHandle) {
        let checked = checked_source();
        let symbol = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::forward_with_local")
            .expect("forward-with-local machine")
            .symbol;
        (checked, symbol)
    }

    let (mut checked, symbol) = checked_plan();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    plan.trivial_affine_local_discard_ordinals.clear();
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::forward_with_local")
        )
        .is_err()
    );

    let (mut checked, symbol) = checked_plan();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    plan.trivial_affine_locals[0].declaration_ordinal = 1;
    plan.trivial_affine_local_discard_ordinals[0] = 1;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::forward_with_local")
        )
        .is_err()
    );

    let (mut checked, symbol) = checked_plan();
    let scratch_type = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine && machine.name.as_str() == "Main::forward_and_drop"
            })
        })
        .unwrap()
        .structural_parameters[1]
        .type_identity
        .clone();
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap();
    plan.trivial_affine_locals[0].type_identity = scratch_type;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::forward_with_local")
        )
        .is_err()
    );
}
