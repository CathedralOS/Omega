use crate::lower_machine;
use crate::tests::checked_source;
use semantic_vocabulary::{ScalarType, StructuralPlaceKind};
use terminal_production::{ProgramEntryTerminalReceiptError, TerminalArtifactProductionError};
use terminal_psi::{
    BindingRelevance, OperationKind, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachineResult, Terminator,
};

#[test]
fn terminal_production_request_preserves_configuration_across_evidence_products() {
    let checked = checked_source("data Main {} machine Main::launch() {}");
    for selections in [
        optimization::PsiOptimizationSelections::default(),
        optimization::PsiOptimizationSelections::new([
            optimization::PsiOptimization::DeadPureScalarElimination,
        ])
        .expect("unique selection"),
    ] {
        let request = || terminal_production::TerminalProductionRequest {
            checked: &checked,
            machine: terminal_production::TerminalMachineSelection::Name("Main::launch"),
            optimization_selections: selections.clone(),
        };
        let artifact = request().produce_artifact().expect("portable artifact");
        let scoped = request()
            .produce_checked_artifact()
            .expect("checked artifact");
        let callbacks = request()
            .produce_with_callback_custody(Box::new([11u64, 29u64]))
            .expect("callback artifact");
        let entry = request()
            .produce_program_entry([7; 32])
            .expect("entry artifact");
        assert_eq!(artifact.optimization().selections(), &selections);
        assert_eq!(&artifact, scoped.artifact());
        assert_eq!(&artifact, callbacks.artifact());
        assert_eq!(&artifact, entry.artifact());
        assert_eq!(
            scoped.boundary_operator_scope(),
            callbacks.boundary_operator_scope()
        );
        assert_eq!(
            scoped.boundary_operator_scope(),
            entry.boundary_operator_scope()
        );
        assert_eq!(entry.receipt().source_signature_identity(), [7; 32]);
        assert_eq!(entry.receipt().source_machine_name(), "Main::launch");
        assert_eq!(
            entry.receipt().terminal_psi_identity(),
            artifact.manifest().semantic()
        );
    }
}

#[test]
fn terminal_production_request_returns_nonclone_callback_custody_after_production_rejection() {
    #[derive(Debug)]
    struct CallbackCustody(Box<[u64; 2]>);

    let checked = checked_source("data Main {} machine Main::launch() {}");
    let custody = CallbackCustody(Box::new([11, 29]));
    let allocation = custody.0.as_ptr();
    let request = terminal_production::TerminalProductionRequest {
        checked: &checked,
        machine: terminal_production::TerminalMachineSelection::Name("Main::missing"),
        optimization_selections: optimization::PsiOptimizationSelections::new([
            optimization::PsiOptimization::ControlFlowCleanup,
        ])
        .expect("unique selection"),
    };
    let rejected = request
        .produce_with_callback_custody(custody)
        .expect_err("an unknown machine rejects production before lowering completes");
    assert!(matches!(
        rejected.error(),
        TerminalArtifactProductionError::Lowering(_)
    ));
    let (_, custody) = rejected.into_parts();
    assert_eq!(custody.0.as_ptr(), allocation);
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_with_callback_custody(custody)
        .expect("returned custody can retry identity production");
    let (_, _, custody, _, _, _) = produced.into_parts();
    assert_eq!(custody.0.as_ptr(), allocation);
    assert_eq!(*custody.0, [11, 29]);
}

#[test]
fn callback_custody_crosses_terminal_production_in_exact_order_and_returns_on_rejection() {
    let checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let custody = vec![(11u64, "first"), (29u64, "second")];
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_with_callback_custody(custody.clone())
        .expect("opaque callback custody crosses canonical Terminal production");
    assert_eq!(produced.callback_custody(), &custody);
    produced.artifact().validate().expect("canonical artifact");
    let (_, _, returned, _, _, _) = produced.into_parts();
    assert_eq!(returned, custody);

    let swapped = vec![(29u64, "second"), (11u64, "first")];
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_with_callback_custody(swapped.clone())
        .expect("opaque callback custody preserves caller-provided order");
    assert_eq!(produced.callback_custody(), &swapped);

    let rejected = terminal_production::TerminalProductionRequest::new(&checked, "Main::missing")
        .produce_with_callback_custody(custody.clone())
        .expect_err("missing Terminal machine rejects transactionally");
    let TerminalArtifactProductionError::Lowering(error) = rejected.error() else {
        panic!("missing machine must reject during lowering");
    };
    assert_eq!(error.to_string(), "MachineNotFound(\"Main::missing\")");
    let (_, returned) = rejected.into_parts();
    assert_eq!(returned, custody);
}

#[test]
fn checked_boundary_operator_scope_rejects_terminal_artifact_substitution() {
    let first = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let produced = terminal_production::TerminalProductionRequest::new(&first, "Main::launch")
        .produce_checked_artifact()
        .expect("checked Terminal production");

    let second = checked_source(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Main {}
            machine Main::launch() { Helper::touch(); }
        "#,
    );
    let substituted = terminal_production::TerminalProductionRequest::new(&second, "Main::launch")
        .produce_artifact()
        .expect("distinct canonical Terminal artifact");
    let first_lowered = lower_machine(&first, "Main::launch").expect("first source lowers");
    assert_eq!(
        lowered_psi_to_terminal_psi::checked_boundary_operator_scope(
            &first,
            &substituted,
            &first_lowered,
        )
        .unwrap_err(),
        "checked boundary-operator scope semantics differ from the published artifact",
    );
    assert!(
        produced
            .boundary_operator_scope()
            .validate_for_artifact(&substituted)
            .is_err()
    );
}

#[test]
fn checked_boundary_operator_scope_retains_the_complete_exact_demand_roster() {
    let demand_source = checked_source(
        r#"
            boundary operator == Number::equal(left: i32, right: i32) -> bool;

            machine launch(left: i32, right: i32) -> bool {
                left == right
            }
        "#,
    );
    let [expected] = demand_source
        .facts
        .operators
        .boundary_applications
        .as_slice()
    else {
        panic!("one exact checked boundary-operator demand")
    };
    let expected = expected.clone();
    // This milestone closes scope custody only. Source-free operation matching
    // remains the next D29/D32 join, so use an independently lowerable Terminal
    // fixture and verify that its companion retains the exact checked row.
    let mut checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    checked.facts.operators.boundary_applications = vec![expected.clone()];
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_checked_artifact()
        .expect("checked Terminal production retains exact D29 demand custody");

    assert_eq!(
        produced.boundary_operator_scope().applications(),
        std::slice::from_ref(&expected)
    );
    assert!(produced.boundary_operator_scope().occurrences().is_empty());
    assert!(!produced.boundary_operator_scope().is_empty());
}

#[test]
fn program_entry_receipt_binds_checked_source_to_canonical_terminal_entry() {
    let checked = checked_source(
        r#"
            data Main {}
            machine Main::launch() {}
        "#,
    );
    let source_signature_identity = [0x5a; 32];
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_program_entry(source_signature_identity)
        .expect("produce checked Unit ProgramEntry artifact");
    let receipt = produced.receipt();
    let decoded = terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("decode canonical semantic module");

    assert_eq!(
        receipt.source_signature_identity(),
        source_signature_identity
    );
    assert_eq!(receipt.source_machine_name(), "Main::launch");
    assert_eq!(receipt.terminal_entry(), decoded.entry);
    assert_eq!(
        receipt.terminal_psi_identity(),
        produced.artifact().manifest().semantic()
    );
    assert!(
        decoded
            .machines
            .iter()
            .any(|machine| machine.id == receipt.terminal_entry()
                && machine.result == TerminalMachineResult::Unit)
    );
    produced
        .artifact()
        .validate()
        .expect("receipt-coupled artifact replays");
}

#[test]
fn program_entry_receipt_retains_two_granted_extent_roots_and_their_boundary_handoff() {
    let checked = checked_source(
        r#"
            data Extent [linear] {
                base: addr;
                length: u64;
            }

            boundary machine no_wrap(base: addr, length: u64) -> bool;

            domain Extent::Granted
            requires
                no_wrap(self.base, self.length)
            established by
                ProgramStorageEntry::enter;

            boundary trait ProgramStorageEntry {
                machine enter(
                    image: Extent in Granted,
                    initial_storage: Extent in Granted
                );
            }

            data ProgramLocalProducer {}
            machine ProgramLocalProducer::handoff(
                image: Extent in Granted,
                initial_storage: Extent in Granted
            )
            reaches ProgramStorageEntry invokes ProgramStorageEntry;
            {
                ProgramStorageEntry::enter(image, initial_storage);
            }
        "#,
    );
    let source_signature_identity = [0xa5; 32];
    let produced = terminal_production::TerminalProductionRequest::new(
        &checked,
        "ProgramLocalProducer::handoff",
    )
    .produce_program_entry(source_signature_identity)
    .expect("produce exact two-root Unit ProgramEntry artifact");
    let receipt = produced.receipt();
    let decoded = terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("decode canonical two-root semantic module");

    assert_eq!(
        receipt.source_signature_identity(),
        source_signature_identity
    );
    assert_eq!(
        receipt.source_machine_name(),
        "ProgramLocalProducer::handoff"
    );
    assert_eq!(receipt.terminal_entry(), decoded.entry);
    assert_eq!(
        receipt.terminal_psi_identity(),
        produced.artifact().manifest().semantic()
    );

    let entry = decoded
        .machines
        .iter()
        .find(|machine| machine.id == receipt.terminal_entry())
        .expect("receipt names one retained Terminal entry");
    let [image, initial_storage] = entry.structural_parameters.as_slice() else {
        panic!("ProgramStorage handoff must retain two structural inputs")
    };
    assert_eq!((image.position, initial_storage.position), (0, 1));
    assert!(!image.is_self && !initial_storage.is_self);
    assert_ne!(image.place, initial_storage.place);
    assert_eq!(image.structural_type, initial_storage.structural_type);
    assert_eq!(image.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(initial_storage.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(image.access, StructuralAccess::Owned);
    assert_eq!(initial_storage.access, StructuralAccess::Owned);
    let [image_domain] = image.qualifications.as_slice() else {
        panic!("Image must retain exactly one qualification")
    };
    let [storage_domain] = initial_storage.qualifications.as_slice() else {
        panic!("InitialStorage must retain exactly one qualification")
    };
    assert_eq!(image_domain, storage_domain);
    let domain = decoded
        .structural_domains
        .iter()
        .find(|domain| domain.id == *image_domain)
        .expect("Granted domain declaration remains in the canonical artifact");
    assert_eq!(domain.identity, "Extent::Granted");
    assert_eq!(domain.carrier, image.structural_type);
    let carrier = decoded
        .structural_types
        .iter()
        .find(|declaration| declaration.id == image.structural_type)
        .expect("Extent carrier declaration remains in the canonical artifact");
    assert_eq!(carrier.identity, "named(name(Extent))");
    let StructuralTypeShape::Record { fields } = &carrier.shape else {
        panic!("Extent carrier must remain a record")
    };
    assert!(matches!(fields.as_slice(), [base, length]
        if base.identity == "base"
            && base.relevance == BindingRelevance::Relevant
            && matches!(base.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.is_address())
            && length.identity == "length"
            && length.relevance == BindingRelevance::Relevant
            && matches!(length.field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer)) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64)));

    assert!(
        matches!(entry.structural_places.as_slice(), [image_place, storage_place]
        if image_place.id == image.place
            && image_place.kind == StructuralPlaceKind::Parameter { position: 0, is_self: false }
            && storage_place.id == initial_storage.place
            && storage_place.kind == StructuralPlaceKind::Parameter { position: 1, is_self: false })
    );
    let [image_claim, storage_claim] = entry.entry_claims.as_slice() else {
        panic!("ProgramStorage handoff must retain two entry claims")
    };
    assert_eq!(image_claim.input, image.place);
    assert_eq!(storage_claim.input, initial_storage.place);
    assert!(image_claim.path.is_empty() && storage_claim.path.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("ProgramStorage handoff must remain straight-line")
    };
    let [call] = block.operations.as_slice() else {
        panic!("ProgramStorage handoff must retain one Unit call")
    };
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &call.kind
    else {
        panic!("ProgramStorage handoff operation must remain BoundaryCall")
    };
    let boundary = decoded
        .boundary_machines
        .iter()
        .find(|candidate| candidate.id == *boundary)
        .expect("explicit ProgramStorage requirement remains a bodyless boundary");
    assert_eq!(boundary.structural_parameters.len(), 2);
    assert_eq!(boundary.structural_parameters[0].position, 0);
    assert_eq!(boundary.structural_parameters[1].position, 1);
    assert!(
        matches!(structural_arguments.as_slice(), [image_argument, storage_argument]
        if image_argument.place == image.place
            && image_argument.access == StructuralAccess::Owned
            && image_argument.path.is_empty()
            && storage_argument.place == initial_storage.place
            && storage_argument.access == StructuralAccess::Owned
            && storage_argument.path.is_empty())
    );
    assert!(
        matches!(completion_receipts.as_slice(), [image_receipt, storage_receipt]
        if image_receipt.claim == image_claim.claim
            && image_receipt.argument_index == 0
            && storage_receipt.claim == storage_claim.claim
            && storage_receipt.argument_index == 1)
    );
    assert!(matches!(
        block.terminator,
        Terminator::ReturnUnit {
            ref trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));
    produced
        .artifact()
        .validate()
        .expect("two-root receipt-coupled artifact replays");
}

#[test]
fn program_entry_receipt_rejects_a_scalar_result_machine() {
    let checked = checked_source(
        r#"
            data Helper {}
            machine Helper::touch() {}
            data Token { value: u64; }
            machine Token::drop(&mut self) { Helper::touch(); }
            data Main {}
            machine Main::launch(token: Token) -> u64 { 7u64 }
        "#,
    );
    let error = terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
        .produce_program_entry([0x11; 32])
        .expect_err("ProgramEntry receipt requires a Unit result");
    assert!(
        matches!(
            &error,
            TerminalArtifactProductionError::EntryReceipt(
                ProgramEntryTerminalReceiptError::NonUnitEntry
            )
        ),
        "unexpected receipt rejection: {error:?}"
    );
}
