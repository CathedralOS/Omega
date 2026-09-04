use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::OperationKind;
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fuel::TerminalFuelMeter;
use psi_terminal_interpreter::{
    ProviderInstallationSelection, TerminalEffect, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalStructuralValue, admit_provider_installation_from_artifact,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait Signal {
        machine emit();
    }

    data FirstProvider {}
    machine FirstProvider::emit() satisfies Signal::emit {}

    data SecondProvider {}
    machine SecondProvider::emit() satisfies Signal::emit {}

    data Root {}
    machine Root::enter()
    reaches Signal
    {
        Signal::emit();
    }
"#;

const STRUCTURAL_PROVIDER_SOURCE: &str = r#"
    pub data Extent [linear] {
        base: addr;
        length: u64;
    }

    pub boundary machine no_wrap(base: addr, length: u64) -> bool;

    pub domain Extent::Granted
    requires
        no_wrap(self.base, self.length)
    established by
        ProgramEntry::enter;

    pub boundary trait ProgramEntry {
        machine enter(extent: Extent in Granted);
    }

    boundary machine Extent::settle(self)
    requires
        self in Extent::Granted
    ensures true;

    data ProgramProvider {}
    machine ProgramProvider::enter(extent: Extent in Granted)
        satisfies ProgramEntry::enter
    {
        extent.settle();
    }

    data Root {}
    machine Root::enter<machine Enter>(extent: Extent in Granted)
    where machine Enter satisfies ProgramEntry::enter;
    {
        Enter(extent);
    }
"#;

const PROGRAM_STORAGE_PROVIDER_SOURCE: &str = r#"
    pub data Extent [linear] {
        base: addr;
        length: u64;
    }

    pub boundary machine no_wrap(base: addr, length: u64) -> bool;

    pub domain Extent::Granted
    requires
        no_wrap(self.base, self.length)
    established by
        ProgramStorageEntry::enter;

    pub boundary trait ProgramStorageEntry {
        machine enter(
            image: Extent in Granted,
            initial_storage: Extent in Granted
        );
    }

    boundary machine Extent::settle(self)
    requires
        self in Extent::Granted
    ensures true;

    data ProgramStorageProvider {}
    machine ProgramStorageProvider::enter(
        image: Extent in Granted,
        initial_storage: Extent in Granted
    )
    satisfies ProgramStorageEntry::enter
    {
        image.settle();
        initial_storage.settle();
    }

    data ProgramLocalProducer {}
    machine ProgramLocalProducer::handoff<machine Enter>(
        image: Extent in Granted,
        initial_storage: Extent in Granted
    )
    where machine Enter satisfies ProgramStorageEntry::enter;
    {
        Enter(image, initial_storage);
    }
"#;

#[test]
fn checked_unit_provider_candidates_are_cataloged_without_selection_or_call_rewrite() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("zero-argument Unit provider catalog lowers");

    let module = &lowered.semantic_module;
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(module.provider_candidates.len(), 2);
    assert_eq!(module.machines.len(), 3, "root plus both adapter bodies");
    let boundary = module.boundary_machines[0].id;
    let requirement_identity = &module.provider_candidates[0].requirement_identity;
    assert!(!requirement_identity.is_empty());
    assert!(module.provider_candidates.iter().all(|candidate| {
        candidate.boundary == boundary
            && candidate.requirement_identity == *requirement_identity
            && candidate.signature.parameters.is_empty()
            && candidate.refinement.positional_parameters.is_empty()
            && candidate.refinement.required_domains.is_empty()
            && module
                .machines
                .iter()
                .any(|machine| machine.id == candidate.candidate)
    }));
    assert_eq!(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.provider_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["FirstProvider", "SecondProvider"]
    );
    assert_eq!(
        module
            .provider_candidates
            .iter()
            .map(|candidate| candidate.candidate_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["FirstProvider::emit", "SecondProvider::emit"]
    );
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    assert!(matches!(
        entry.blocks[0].operations[0].kind,
        OperationKind::BoundaryCall { boundary: called, .. } if called == boundary
    ));

    let encoded = encode_module(module).expect("encode catalog");
    let decoded = decode_module(&encoded).expect("decode and independently verify catalog shape");
    assert_eq!(decoded, *module);
}

#[test]
fn checked_unit_provider_candidates_retain_linear_qualified_structural_inputs() {
    let tokens = Lexer::new(STRUCTURAL_PROVIDER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("structural Unit provider catalog lowers");

    let module = &lowered.semantic_module;
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one exact ProgramEntry provider candidate")
    };
    assert_eq!(candidate.provider_identity, "ProgramProvider");
    assert_eq!(candidate.candidate_identity, "ProgramProvider::enter");
    let [signature] = candidate.signature.parameters.as_slice() else {
        panic!("provider signature retains one structural root")
    };
    assert_eq!(signature.position, 0);
    assert!(!signature.is_self);
    assert_eq!(
        signature.multiplicity,
        psi_terminal::StructuralMultiplicity::Linear
    );
    assert_eq!(signature.access, psi_terminal::StructuralAccess::Owned);
    let [qualification] = signature.qualifications.as_slice() else {
        panic!("provider signature retains the Granted qualification")
    };
    assert_eq!(
        candidate.refinement.positional_parameters,
        [psi_terminal::ProviderParameterRefinement {
            boundary_index: 0,
            candidate_index: 0,
        }]
    );
    assert_eq!(candidate.refinement.required_domains.len(), 1);
    assert_eq!(candidate.refinement.required_domains[0].argument_index, 0);
    assert_eq!(
        candidate.refinement.required_domains[0].domain,
        *qualification
    );

    let provider = module
        .machines
        .iter()
        .find(|machine| machine.id == candidate.candidate)
        .expect("provider body remains an ordinary Terminal machine");
    assert_eq!(provider.structural_parameters.len(), 1);
    assert_eq!(provider.entry_claims.len(), 1);
    assert!(matches!(
        provider.blocks[0].operations[0].kind,
        OperationKind::BoundaryCall {
            ref completion_receipts,
            ..
        } if completion_receipts.len() == 1
    ));

    let encoded = encode_module(module).expect("encode structural provider catalog");
    assert_eq!(
        decode_module(&encoded).expect("decode structural provider catalog"),
        *module
    );

    let mut access_tamper = module.clone();
    let candidate_machine = access_tamper.provider_candidates[0].candidate;
    access_tamper
        .machines
        .iter_mut()
        .find(|machine| machine.id == candidate_machine)
        .expect("candidate machine")
        .structural_parameters[0]
        .access = psi_terminal::StructuralAccess::SharedBorrow;
    assert!(matches!(
        psi_terminal_verifier::validate_module(&access_tamper),
        Err(psi_terminal_verifier::ModuleError::InvalidProviderCandidate { .. })
    ));
}

#[test]
fn installed_structural_provider_receives_and_settles_the_exact_linear_claim() {
    let tokens = Lexer::new(STRUCTURAL_PROVIDER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("structural provider artifact lowers");
    let [candidate] = lowered.semantic_module.provider_candidates.as_slice() else {
        panic!("one exact structural provider candidate")
    };
    let [signature] = candidate.signature.parameters.as_slice() else {
        panic!("one exact structural provider parameter")
    };
    let [qualification] = signature.qualifications.as_slice() else {
        panic!("one exact structural provider qualification")
    };
    let semantic = encode_module(&lowered.semantic_module).expect("semantic artifact");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof artifact");
    let installation = admit_provider_installation_from_artifact(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .expect("exact structural provider installation is admitted");
    let argument = TerminalStructuralValue {
        opaque_identity: 0xfeed,
        structural_type: signature.structural_type,
        qualifications: vec![*qualification],
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_provider_installation(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
        &[],
        std::slice::from_ref(&argument),
        &installation,
    )
    .expect("structural provider execution starts");
    assert_eq!(
        execution.resume(&mut TerminalFuelMeter::default()).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(
        matches!(execution.effects(), [TerminalEffect::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    }] if *boundary != candidate.boundary
        && structural_arguments == &[argument]
        && completion_receipts.len() == 1)
    );
}

#[test]
fn installed_program_storage_provider_transfers_and_settles_both_owned_extent_claims() {
    let tokens = Lexer::new(PROGRAM_STORAGE_PROVIDER_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
        &checked,
        "ProgramLocalProducer::handoff",
        [0xa5; 32],
    )
    .expect("receipt-coupled ProgramStorage artifact");
    let module = decode_module(produced.artifact().semantic_bytes()).expect("semantic module");
    let [candidate] = module.provider_candidates.as_slice() else {
        panic!("one exact ProgramStorage provider candidate")
    };
    let [image, initial_storage] = candidate.signature.parameters.as_slice() else {
        panic!("the exact two-Extent provider signature")
    };
    let ([image_domain], [storage_domain]) = (
        image.qualifications.as_slice(),
        initial_storage.qualifications.as_slice(),
    ) else {
        panic!("both Extent roots retain Granted")
    };
    assert_eq!(image_domain, storage_domain);
    assert_eq!(image.access, psi_terminal::StructuralAccess::Owned);
    assert_eq!(
        initial_storage.access,
        psi_terminal::StructuralAccess::Owned
    );
    let installation = admit_provider_installation_from_artifact(
        produced.artifact().semantic_bytes(),
        produced.artifact().proof_bytes(),
        &psi_proof_admission::AdmissionProfile::default(),
        &[ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        }],
    )
    .expect("exact ProgramStorage provider installation is admitted");
    let arguments = [
        TerminalStructuralValue {
            opaque_identity: 0x1a6e,
            structural_type: image.structural_type,
            qualifications: vec![*image_domain],
            path: Vec::new(),
        },
        TerminalStructuralValue {
            opaque_identity: 0x570a,
            structural_type: initial_storage.structural_type,
            qualifications: vec![*storage_domain],
            path: Vec::new(),
        },
    ];
    let mut execution = TerminalExecution::start_artifact_with_provider_installation(
        produced.artifact().semantic_bytes(),
        produced.artifact().proof_bytes(),
        &psi_proof_admission::AdmissionProfile::default(),
        &[],
        &arguments,
        &installation,
    )
    .expect("ProgramStorage provider execution starts");
    assert_eq!(
        execution.resume(&mut TerminalFuelMeter::default()).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(matches!(execution.effects(), [
        TerminalEffect::BoundaryCall { structural_arguments: image, completion_receipts: image_receipts, .. },
        TerminalEffect::BoundaryCall { structural_arguments: storage, completion_receipts: storage_receipts, .. },
    ] if image == &arguments[..1]
        && storage == &arguments[1..]
        && image_receipts.len() == 1
        && storage_receipts.len() == 1));
}
