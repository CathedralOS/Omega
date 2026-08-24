use psi_core::{ContentAlgebraKind, ContentPlaceVersion};
use psi_language_semantics::Multiplicity;
use psi_language_semantics::PermissionClaimIdentity;
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{TerminalMachineResult, Terminator};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralResult, TerminalStructuralValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Region [linear] { length: u64; }
    data Scratch { marker: u64; }
    data EmptyScratch {}
    data NominalScratch {}
    machine NominalScratch::drop(&mut self) {}
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length }
    }

    data Main {}
    machine Main::forward(region: Region in Owned) -> Region in Owned {
        region
    }
    machine Main::forward_and_drop(region: Region in Owned, scratch: Scratch) -> Region in Owned {
        region
    }
    machine Main::forward_with_local(region: Region in Owned) -> Region in Owned {
        let scratch: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::forward_with_local_and_drop(
        region: Region in Owned,
        scratch: Scratch
    ) -> Region in Owned {
        let local: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::local_partial_value(region: Region in Owned) -> Region in Owned {
        let scratch: Scratch = Scratch { marker: 1 };
        region
    }
    machine Main::local_nominal_cleanup(region: Region in Owned) -> Region in Owned {
        let scratch: NominalScratch = NominalScratch {};
        region
    }
    machine Main::forward_with_two_locals(region: Region in Owned) -> Region in Owned {
        let first: EmptyScratch = EmptyScratch {};
        let second: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::forward_and_drop_two(
        region: Region in Owned,
        first: Scratch,
        second: Scratch
    ) -> Region in Owned {
        region
    }
    machine Main::forward_with_local_and_drop_two(
        region: Region in Owned,
        first: Scratch,
        second: Scratch
    ) -> Region in Owned {
        let local: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::through_local(region: Region in Owned) -> Region in Owned {
        let forwarded: Region in Owned = region;
        forwarded
    }
    machine Main::contracted(region: Region in Owned) -> Region in Owned
    requires
        region in Region::Owned
    {
        region
    }
    machine Main::local_control(region: Region in Owned) -> Region in Owned {
        let scratch: EmptyScratch = EmptyScratch {};
        transition { _ -> next(region) }
        state next(region: Region in Owned) -> Region in Owned { region }
    }
    machine Main::main(&mut self) {}
"#;

const INDEXED_CUSTODY_SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Root {}
    machine Root::enter(receipts: [Receipt; 2])
    reaches PortIo
    {
        Receipt::settle(receipts[0]);
        Receipt::settle(receipts[1]);
    }
"#;

const RESULT_BOUNDARY_CUSTODY_SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self) -> bool
    reaches PortIo
    ensures true;

    data Root {}
    machine Root::enter(receipt: Receipt) -> bool
    reaches PortIo
    {
        let accepted: bool = receipt.settle();
        accepted
    }
"#;

const RESULT_BOUNDARY_CONTENT_CUSTODY_SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Region [linear] { length: u64; }
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length }
    }

    boundary trait PortIo {}

    boundary machine Region::retire(self) -> bool
    reaches PortIo
    ensures true;

    boundary machine Region::discard(self)
    reaches PortIo;

    data Root {}
    machine Root::enter(region: Region in Owned) -> bool
    reaches PortIo
    {
        let accepted: bool = region.retire();
        accepted
    }

    machine Root::exit(region: Region in Owned)
    reaches PortIo
    {
        region.discard();
    }
"#;

const RESULT_BOUNDARY_BOUNDED_REACH_SOURCE: &str = r#"
    boundary trait MachineControl {}
    boundary trait PortIo {}

    boundary trait InterruptCompletion {
        machine complete() -> bool
        reaches <= MachineControl + PortIo;
    }

    data Root {}
    machine Root::enter<machine Completion>() -> bool
    where machine Completion satisfies InterruptCompletion::complete;
    {
        let accepted: bool = Completion();
        accepted
    }
"#;

const ORDINARY_INDEXED_CUSTODY_SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::run(receipt: Receipt)
    reaches PortIo
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 2])
    reaches PortIo
    {
        Helper::run(receipts[0]);
        Helper::run(receipts[1]);
    }
"#;

const UNIT_AFFINE_LOCAL_SOURCE: &str = r#"
    data Empty {}
    data Token { value: u64; }
    data Root {}

    machine Root::cleanup(first: Token, second: Token) {
        let one: Empty = Empty {};
        let two: Empty = Empty {};
    }
"#;

#[test]
fn source_unit_retains_ordered_empty_affine_local_cleanup() {
    let tokens = Lexer::new(UNIT_AFFINE_LOCAL_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::cleanup")
        .expect("bounded Unit local lowering");
    let machine = lowered.semantic_module.machines.first().expect("machine");
    let locals = machine
        .structural_places
        .iter()
        .filter(|place| {
            matches!(
                place.kind,
                psi_core::StructuralPlaceKind::TrivialAffineLocal { .. }
            )
        })
        .map(|place| place.id)
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination: first },
                ..
            },
            psi_terminal::Operation {
                kind: psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination: second },
                ..
            }
        ] if [*first, *second] == locals.as_slice()
    ));
    let psi_terminal::Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("Unit return")
    };
    assert_eq!(
        trivial_affine_discards,
        &locals
            .iter()
            .rev()
            .copied()
            .chain(
                machine
                    .structural_parameters
                    .iter()
                    .rev()
                    .map(|parameter| parameter.place)
            )
            .collect::<Vec<_>>()
    );

    let semantic = encode_module(&lowered.semantic_module).expect("Unit semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("Unit semantics decode"),
        lowered.semantic_module,
        "the codec retains explicit local establishment and cleanup custody"
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs Unit local cleanup");

    let mut missing_declaration = lowered.semantic_module.clone();
    missing_declaration.machines[0]
        .structural_places
        .retain(|place| place.id != locals[0]);
    assert!(
        psi_terminal_verifier::validate_module_representation(&missing_declaration).is_err(),
        "an establishment operation cannot outlive its typed local declaration"
    );

    let mut reordered_declarations = lowered.semantic_module.clone();
    for place in &mut reordered_declarations.machines[0].structural_places {
        if let psi_core::StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            ..
        } = &mut place.kind
        {
            *declaration_ordinal = 1 - *declaration_ordinal;
        }
    }
    assert!(
        psi_terminal_verifier::validate_module_representation(&reordered_declarations).is_err(),
        "local declarations are canonical source-order custody"
    );

    let mut reordered_cleanup = lowered.semantic_module.clone();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut reordered_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.swap(0, 1);
    assert!(
        psi_terminal_verifier::validate_module_representation(&reordered_cleanup).is_err(),
        "reordered cleanup is not a valid terminal module"
    );

    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("Unit proof encodes");
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xaff1 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("Unit affine-local artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for expected_usage in 0..3 {
        assert!(matches!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(meter.usage().total_units(), expected_usage);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 3);
}

#[derive(Default)]
struct RejectSecondEffect {
    accepted: usize,
}

impl TerminalEffectHandler for RejectSecondEffect {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        if self.accepted == 1 {
            return Err(TerminalEffectRejection::new(
                "reject second indexed settlement",
            ));
        }
        self.accepted += 1;
        Ok(())
    }
}

struct ResultBoundaryHandler {
    reject: bool,
}

impl TerminalEffectHandler for ResultBoundaryHandler {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Ok(())
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<Option<TerminalScalarValue>, TerminalEffectRejection> {
        if self.reject {
            return Err(TerminalEffectRejection::new("provider rejected settlement"));
        }
        assert!(matches!(
            effect,
            TerminalEffect::BoundaryCall {
                result: Some(psi_core::ScalarType::Boolean),
                ..
            }
        ));
        Ok(Some(TerminalScalarValue::Boolean(true)))
    }
}

fn checked_source() -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn result_bearing_boundary_receipt_verifies_and_commits_only_after_success() {
    let tokens = Lexer::new(RESULT_BOUNDARY_CUSTODY_SOURCE)
        .tokenize()
        .expect("tokenize result boundary custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse result boundary custody");
    let resolved = lower_syntax_trees(&syntax).expect("resolve result boundary custody");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type result boundary custody");
    let checked = lower_typed_trees(typed).expect("check result boundary custody");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("result-bearing boundary custody should lower");
    let module = &lowered.semantic_module;
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(
        module.boundary_machines[0].result,
        Some(psi_core::ScalarType::Boolean)
    );
    let operation = &module.machines[0].blocks[0].operations[0];
    assert!(operation.result.scalar().is_some());
    let psi_terminal::OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &operation.kind
    else {
        panic!("result-bearing call must remain a terminal boundary operation")
    };
    assert_eq!(completion_receipts.len(), 1);

    let semantic = encode_module(module).expect("result boundary semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("result boundary semantics decode"),
        *module
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("result boundary proof encodes");
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("result-bearing boundary custody verifies");
    let mut mismatched_result = module.clone();
    mismatched_result.boundary_machines[0].result = None;
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&mismatched_result),
        Err(psi_terminal_verifier::ModuleError::BoundaryCallResultMismatch {
            operation: rejected,
            ..
        }) if rejected == operation.id
    ));
    let parameter = &module.machines[0].structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 0x5e77_1e,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("result boundary artifact starts");
    let initial_claims = execution.live_claim_frontier().collect::<Vec<_>>();
    assert_eq!(initial_claims.len(), 1);
    let mut meter = TerminalFuelMeter::unbounded();
    let mut rejecting = ResultBoundaryHandler { reject: true };
    assert!(matches!(
        execution.resume_with_effect_handler(&mut meter, &mut rejecting),
        Err(TerminalInterpretError::EffectRejected { operation: rejected, .. })
            if rejected == operation.id
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        initial_claims
    );
    assert!(execution.effects().is_empty());

    let mut accepting = ResultBoundaryHandler { reject: false };
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, &mut accepting)
            .expect("accepted boundary result resumes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}

#[test]
fn source_content_custody_exit_retains_projection_and_commits_only_after_success() {
    let tokens = Lexer::new(RESULT_BOUNDARY_CONTENT_CUSTODY_SOURCE)
        .tokenize()
        .expect("tokenize content custody exit");
    let syntax = parse_syntax_trees(&tokens).expect("parse content custody exit");
    let resolved = lower_syntax_trees(&syntax).expect("resolve content custody exit");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type content custody exit");
    let checked = lower_typed_trees(typed).expect("check content custody exit");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
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
    assert_ne!(projection.projection.projection_fingerprint, 0);

    let semantic = encode_module(module).expect("content custody semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("content custody semantics decode"),
        *module
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("content custody proof encodes");
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("content-bearing boundary custody verifies");

    let mut drifted = module.clone();
    drifted.machines[0].content_entry_claims[0].input.root =
        psi_core::PlaceId::new(structural_claim.input.get() + 1).expect("different place");
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&drifted),
        Err(psi_terminal_verifier::ModuleError::ContentEntryClaimRequiresEntryParameter(_))
            | Err(
                psi_terminal_verifier::ModuleError::ContentEntryClaimStructuralBindingMismatch(_)
            )
    ));

    let parameter = &machine.structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 0xc017_e17,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        }],
    )
    .expect("content custody artifact starts");
    let initial_claims = execution.live_claim_frontier().collect::<Vec<_>>();
    assert_eq!(initial_claims, [content_claim.claim]);
    let mut meter = TerminalFuelMeter::unbounded();
    let mut rejecting = ResultBoundaryHandler { reject: true };
    assert!(matches!(
        execution.resume_with_effect_handler(&mut meter, &mut rejecting),
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
            .resume_with_effect_handler(&mut meter, &mut accepting)
            .expect("accepted content exit resumes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}

#[test]
fn source_content_custody_unit_exit_retains_projection_and_consumes_claim() {
    let tokens = Lexer::new(RESULT_BOUNDARY_CONTENT_CUSTODY_SOURCE)
        .tokenize()
        .expect("tokenize Unit content custody exit");
    let syntax = parse_syntax_trees(&tokens).expect("parse Unit content custody exit");
    let resolved = lower_syntax_trees(&syntax).expect("resolve Unit content custody exit");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type Unit content custody exit");
    let checked = lower_typed_trees(typed).expect("check Unit content custody exit");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::exit")
        .expect("content-bearing Unit boundary custody should lower");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("one Unit content custody root machine")
    };
    let [structural_claim] = machine.entry_claims.as_slice() else {
        panic!("one Unit structural entry claim")
    };
    let [content_claim] = machine.content_entry_claims.as_slice() else {
        panic!("one Unit content entry claim")
    };
    assert_eq!(content_claim.claim, structural_claim.claim);
    assert_eq!(content_claim.input.version, ContentPlaceVersion::Entry);
    assert_eq!(content_claim.input.root, structural_claim.input);
    assert!(content_claim.input.segments.is_empty());
    assert_eq!(content_claim.projections.len(), 1);

    let semantic = encode_module(module).expect("Unit content custody semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("Unit content proof encodes");
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("content-bearing Unit boundary custody verifies");

    let parameter = &machine.structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 0xc017_017,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        }],
    )
    .expect("Unit content custody artifact starts");
    assert_eq!(execution.live_claim_frontier().count(), 1);
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("Unit content exit runs"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}

#[test]
fn result_bearing_boundary_retains_exact_bounded_installation_reach() {
    let tokens = Lexer::new(RESULT_BOUNDARY_BOUNDED_REACH_SOURCE)
        .tokenize()
        .expect("tokenize bounded result boundary");
    let syntax = parse_syntax_trees(&tokens).expect("parse bounded result boundary");
    let resolved = lower_syntax_trees(&syntax).expect("resolve bounded result boundary");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type bounded result boundary");
    let checked = lower_typed_trees(typed).expect("check bounded result boundary");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("bounded result boundary should lower");
    let module = &lowered.semantic_module;

    assert!(module.root_service_reach.concrete.is_empty());
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
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("bounded result boundary verifies");

    let mut missing = module.clone();
    missing.root_service_reach.installation_dependencies.clear();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&missing),
        Err(psi_terminal_verifier::ModuleError::RootConcreteServiceReachMismatch { .. })
    ));

    let mut drifted = module.clone();
    drifted.root_service_reach.installation_dependencies[0]
        .upper_bound
        .pop();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&drifted),
        Err(psi_terminal_verifier::ModuleError::InstallationReachBoundaryMismatch(_))
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
        psi_terminal_verifier::validate_module_representation(&padded),
        Err(psi_terminal_verifier::ModuleError::DuplicatePublishedService { .. })
    ));
}

#[test]
fn literal_fixed_array_custody_reaches_verified_interpreted_terminal_psi() {
    let tokens = Lexer::new(INDEXED_CUSTODY_SOURCE)
        .tokenize()
        .expect("tokenize indexed custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse indexed custody");
    let resolved = lower_syntax_trees(&syntax).expect("resolve indexed custody");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type indexed custody");
    let checked = lower_typed_trees(typed).expect("check indexed custody");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("literal fixed-array custody should lower");
    let module = &lowered.semantic_module;
    let machine = module.machines.first().expect("one root machine");
    assert_eq!(machine.entry_claims.len(), 2);
    assert_eq!(
        machine.entry_claims[0].path,
        [psi_terminal::StructuralPathSegment::FixedIndex(0)]
    );
    assert_eq!(
        machine.entry_claims[1].path,
        [psi_terminal::StructuralPathSegment::FixedIndex(1)]
    );
    let [first, second] = machine.blocks[0].operations.as_slice() else {
        panic!("two indexed settlements")
    };
    for (operation, index) in [(first, 0), (second, 1)] {
        let psi_terminal::OperationKind::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            panic!("indexed boundary settlement")
        };
        assert_eq!(
            structural_arguments[0].path,
            [psi_terminal::StructuralPathSegment::FixedIndex(index)]
        );
        assert_eq!(completion_receipts.len(), 1);
    }

    let semantic = encode_module(module).expect("indexed semantics encode");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("indexed proof encodes");
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("indexed custody verifies");
    let mut incomplete = module.clone();
    incomplete.machines[0].entry_claims.pop();
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&incomplete),
        Err(
            psi_terminal_verifier::ModuleError::IncompleteFixedArrayEntryClaims {
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
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("indexed custody artifact starts");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let effects = execution.effects();
    assert_eq!(effects.len(), 2);
    for (effect, index) in effects.iter().zip([0, 1]) {
        let psi_terminal_interpreter::TerminalEffect::BoundaryCall {
            structural_arguments,
            completion_receipts,
            ..
        } = effect
        else {
            panic!("indexed boundary effect")
        };
        assert_eq!(
            structural_arguments[0].path,
            [psi_terminal::StructuralPathSegment::FixedIndex(index)]
        );
        assert_eq!(completion_receipts.len(), 1);
    }

    let mut rejected_execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 0x51b2,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("indexed custody rejection artifact starts");
    let mut rejecting = RejectSecondEffect::default();
    let mut meter = TerminalFuelMeter::unbounded();
    assert!(matches!(
        rejected_execution.resume_with_effect_handler(&mut meter, &mut rejecting),
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
    let tokens = Lexer::new(ORDINARY_INDEXED_CUSTODY_SOURCE)
        .tokenize()
        .expect("tokenize ordinary indexed custody");
    let syntax = parse_syntax_trees(&tokens).expect("parse ordinary indexed custody");
    let resolved = lower_syntax_trees(&syntax).expect("resolve ordinary indexed custody");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type ordinary indexed custody");
    let checked = lower_typed_trees(typed).expect("check ordinary indexed custody");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("ordinary literal fixed-index custody should lower");
    let root = &lowered.semantic_module.machines[0];
    let [first, second] = root.blocks[0].operations.as_slice() else {
        panic!("root should call the helper once per sibling")
    };
    for (operation, index) in [(first, 0), (second, 1)] {
        let psi_terminal::OperationKind::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = &operation.kind
        else {
            panic!("indexed ordinary call")
        };
        assert_eq!(
            structural_arguments[0].path,
            [psi_terminal::StructuralPathSegment::FixedIndex(index)]
        );
        assert_eq!(
            claim_transfers,
            &[psi_terminal::ClaimTransfer {
                claim: root.entry_claims[index as usize].claim,
                argument_index: 0,
            }]
        );
    }

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encode");
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier rebases the exact indexed caller claims");
    let mut wrong_claim = lowered.semantic_module.clone();
    let psi_terminal::OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut wrong_claim.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers[0].claim = root.entry_claims[1].claim;
    assert!(psi_terminal_verifier::validate_module_representation(&wrong_claim).is_err());

    let mut nested_path = lowered.semantic_module.clone();
    let psi_terminal::OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut nested_path.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0]
        .path
        .push(psi_terminal::StructuralPathSegment::FixedIndex(0));
    assert!(matches!(
        psi_terminal_verifier::validate_module_representation(&nested_path),
        Err(psi_terminal_verifier::ModuleError::InvalidStructuralArgumentPath { .. })
    ));

    let mut wrong_index = lowered.semantic_module.clone();
    let psi_terminal::OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut wrong_index.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::FixedIndex(1)];
    assert!(psi_terminal_verifier::validate_module_representation(&wrong_index).is_err());

    let parameter = &root.structural_parameters[0];
    let argument = TerminalStructuralValue {
        opaque_identity: 0x51b3,
        structural_type: parameter.structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("ordinary indexed artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        [root.entry_claims[1].claim],
        "returning from the first helper must restore the untouched sibling claim"
    );
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded())
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.effects().len(), 2);
}

#[test]
fn whole_root_source_passthrough_reaches_verified_and_interpreted_terminal_psi() {
    let checked = checked_source();

    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .find(|plan| {
            checked.machines().iter().any(|machine| {
                machine.symbol == plan.machine && machine.name.as_str() == "Main::forward"
            })
        })
        .expect("checker should publish Main::forward's exact structural-return plan");
    assert_eq!(plan.structural_parameters.len(), 1);
    assert_eq!(
        plan.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(plan.result.multiplicity, Multiplicity::Linear);
    assert_eq!(
        plan.structural_parameters[0].type_identity,
        plan.result.type_identity
    );
    assert_eq!(
        plan.structural_parameters[0].qualifications,
        plan.result.qualifications
    );
    assert_eq!(plan.returned_parameter_index, 0);
    assert!(plan.trivial_affine_discards.is_empty());
    assert_eq!(plan.entry_claim.claim_identity, plan.transferred_claim);
    assert!(plan.entry_claim.path.is_empty());

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward")
        .expect("exact whole-root passthrough should lower");
    let module = &lowered.semantic_module;
    let [machine] = module.machines.as_slice() else {
        panic!("one source machine should produce one terminal machine")
    };
    let TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("source structural result should remain structural")
    };
    assert_eq!(machine.entry_claims.len(), 1);
    assert_eq!(machine.content_entry_claims.len(), 1);
    assert_eq!(machine.content_identity_reshuffles.len(), 1);
    let claim = machine.entry_claims[0].claim;
    assert_eq!(machine.content_entry_claims[0].claim, claim);
    assert_eq!(machine.content_identity_reshuffles[0].claim, claim);
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("whole-root source return should be an ownership transfer")
    };
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[claim]);
    assert!(trivial_affine_discards.is_empty());

    let semantic = encode_module(module).expect("canonical structural semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    psi_terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced structural transfer verifies");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("source-produced artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim]
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![claim],
            }
        ))
    );
}

#[test]
fn structural_return_discards_one_claim_free_affine_parameter_after_materialization() {
    let checked = checked_source();
    let plan = checked
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
        .expect("checker should publish the exact structural return plus affine cleanup");
    assert_eq!(plan.structural_parameters.len(), 2);
    assert_eq!(plan.returned_parameter_index, 0);
    assert_eq!(
        plan.structural_parameters[0].multiplicity,
        Multiplicity::Linear
    );
    assert_eq!(
        plan.structural_parameters[1].multiplicity,
        Multiplicity::Affine
    );
    assert_eq!(plan.trivial_affine_discards, [1]);

    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_and_drop")
        .expect("exact structural return plus affine cleanup should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine should produce one terminal machine")
    };
    let TerminalMachineResult::Structural(result) = &machine.result else {
        panic!("result should remain structural")
    };
    assert_eq!(machine.structural_parameters.len(), 2);
    let claim = machine.entry_claims[0].claim;
    let Terminator::ReturnStructural {
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("return should transfer custody and discard affine scratch")
    };
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert_eq!(returned_claims, &[claim]);
    assert_eq!(
        trivial_affine_discards,
        &[machine.structural_parameters[1].place]
    );

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs the exact affine cleanup");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let returned = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
        path: Vec::new(),
    };
    let scratch_parameter = &machine.structural_parameters[1];
    let scratch = TerminalStructuralValue {
        opaque_identity: 0xcafe,
        structural_type: scratch_parameter.structural_type,
        qualifications: scratch_parameter.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[returned.clone(), scratch],
    )
    .expect("artifact starts with both structural inputs");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        vec![claim]
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: returned,
                claims: vec![claim],
            }
        ))
    );
}

#[test]
fn structural_return_establishes_and_discards_one_trivial_affine_local() {
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
                    && machine.name.as_str() == "Main::forward_with_local"
            })
        })
        .expect("checker should publish the exact trivial affine local cleanup");
    assert_eq!(plan.trivial_affine_locals.len(), 1);
    assert_eq!(plan.trivial_affine_locals[0].declaration_ordinal, 0);
    assert_eq!(plan.trivial_affine_local_discard_ordinals, [0]);
    assert!(plan.trivial_affine_discards.is_empty());

    let lowered =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local")
            .expect("exact affine local structural return should lower");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("one source machine should produce one terminal machine")
    };
    let [operation] = machine.blocks[0].operations.as_slice() else {
        panic!("one local establishment operation should be explicit")
    };
    let psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination } = operation.kind
    else {
        panic!("local should use the exact establishment operation")
    };
    assert!(matches!(
        machine
            .structural_places
            .iter()
            .find(|place| place.id == destination)
            .expect("local place declaration")
            .kind,
        psi_core::StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            ..
        }
    ));
    let psi_terminal::Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("local source still returns structural custody")
    };
    assert_eq!(trivial_affine_discards, &[destination]);

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs local establishment and cleanup");
    let TerminalMachineResult::Structural(result) = &machine.result else {
        unreachable!()
    };
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
        path: Vec::new(),
    };
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![machine.entry_claims[0].claim],
            }
        ))
    );
}

#[test]
fn structural_return_establishes_multiple_locals_in_declaration_order_and_discards_in_reverse() {
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
                    && machine.name.as_str() == "Main::forward_with_two_locals"
            })
        })
        .expect("checker should publish every consecutive trivial affine local");
    assert_eq!(
        plan.trivial_affine_locals
            .iter()
            .map(|local| local.declaration_ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(plan.trivial_affine_local_discard_ordinals, [1, 0]);

    let lowered =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_two_locals")
            .expect("multiple affine locals should lower");
    let machine = &lowered.semantic_module.machines[0];
    let destinations = machine.blocks[0]
        .operations
        .iter()
        .map(|operation| match operation.kind {
            psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination } => destination,
            _ => panic!("each prefix local needs an establishment operation"),
        })
        .collect::<Vec<_>>();
    assert_eq!(destinations.len(), 2);
    for (declaration_ordinal, destination) in destinations.iter().enumerate() {
        assert!(matches!(
            machine
                .structural_places
                .iter()
                .find(|place| place.id == *destination)
                .expect("local place declaration")
                .kind,
            psi_core::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: actual,
                ..
            } if actual == declaration_ordinal as u32
        ));
    }
    let psi_terminal::Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(trivial_affine_discards, &[destinations[1], destinations[0]]);

    let semantic = encode_module(&lowered.semantic_module).expect("semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs dense establishment and reverse cleanup");

    let TerminalMachineResult::Structural(result) = &machine.result else {
        unreachable!()
    };
    let argument = TerminalStructuralValue {
        opaque_identity: 0x5eed,
        structural_type: result.structural_type,
        qualifications: result.qualifications.clone(),
        path: Vec::new(),
    };
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
    )
    .expect("artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: argument,
                claims: vec![machine.entry_claims[0].claim],
            }
        ))
    );
}

#[test]
fn structural_return_cleans_local_before_affine_parameter() {
    let checked = checked_source();
    let lowered =
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local_and_drop")
            .expect("combined local and parameter cleanup should lower");
    let machine = &lowered.semantic_module.machines[0];
    let psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination } =
        machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let psi_terminal::Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        trivial_affine_discards,
        &[destination, machine.structural_parameters[1].place]
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier should reconstruct local-before-parameter order");
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

    let lowered = psi_checked_trees_to_terminal::lower_machine(
        &checked,
        "Main::forward_with_local_and_drop_two",
    )
    .expect("multiple affine tail parameters should lower");
    let machine = &lowered.semantic_module.machines[0];
    let psi_terminal::OperationKind::EstablishTrivialAffineLocal { destination: local } =
        machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let psi_terminal::Terminator::ReturnStructural {
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
    psi_terminal_verifier::verify_module(
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
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &arguments,
    )
    .expect("artifact starts with every structural parameter");
    let mut meter = TerminalFuelMeter::with_allowance(1);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
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
fn producer_fences_locals_and_authored_contracts() {
    let checked = checked_source();
    let planned_names = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter()
        .map(|plan| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
                .expect("plan machine remains present")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        planned_names,
        [
            "Main::forward",
            "Main::forward_and_drop",
            "Main::forward_with_local",
            "Main::forward_with_local_and_drop",
            "Main::forward_with_two_locals",
            "Main::forward_and_drop_two",
            "Main::forward_with_local_and_drop_two"
        ]
    );
    assert!(psi_checked_trees_to_terminal::lower_machine(&checked, "Main::through_local").is_err());
    assert!(psi_checked_trees_to_terminal::lower_machine(&checked, "Main::contracted").is_err());
    for rejected in [
        "Main::local_partial_value",
        "Main::local_nominal_cleanup",
        "Main::local_control",
    ] {
        assert!(
            psi_checked_trees_to_terminal::lower_machine(&checked, rejected).is_err(),
            "{rejected} must remain outside the exact affine-local slice"
        );
    }
}

#[test]
fn lowering_rejects_a_stale_checked_claim_join() {
    let mut checked = checked_source();
    let forward_symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::forward")
        .expect("forward machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == forward_symbol)
        .expect("forward plan");
    plan.transferred_claim = PermissionClaimIdentity::Unknown;
    assert!(matches!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward"),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup"
        ))
    ));
}

#[test]
fn lowering_rejects_stale_structural_return_cleanup_coordinates() {
    let mut checked = checked_source();
    let symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::forward_and_drop")
        .expect("forward-and-drop machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_structural_returns
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .expect("forward-and-drop plan");
    plan.trivial_affine_discards.clear();
    assert!(matches!(
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_and_drop"),
        Err(psi_checked_trees_to_terminal::LoweringError::Unsupported(
            "structural result plan is not one exact whole-root linear transfer with affine cleanup"
        ))
    ));
}

#[test]
fn lowering_rejects_stale_affine_local_declaration_and_cleanup_rows() {
    fn checked_plan() -> (psi_checked_trees::CheckedTrees, psi_symbols::SymbolHandle) {
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
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local").is_err()
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
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local").is_err()
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
        psi_checked_trees_to_terminal::lower_machine(&checked, "Main::forward_with_local").is_err()
    );
}
