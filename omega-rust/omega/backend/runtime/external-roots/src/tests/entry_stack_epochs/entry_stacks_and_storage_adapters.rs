use crate::tests::{
    TestObject, TestStackDemand, body_domains, boundary, entry_id,
    generated_program_storage_adapter_bound_input, generated_program_storage_boundary,
    installed_code, installed_program_storage_wrapper, interrupted_boundary, root_id,
    selected_interrupt_completion,
};
use crate::{
    AdapterStackRealizationOrigin, ArrivalStackRealizationOrigin, DomainStackDemand,
    ExternalRootId, NestingRelationId, ProviderStackSummary, ResolvedRootServiceReach,
    RootProviderId, StackDomain, StackLocalEvidence, StackNestingRelation,
    X86_64GeneratedProgramStorageAdapterEmission, bind_direct_generated_entry_stack_realization,
    bind_installed_entry_stack, bind_x86_64_generated_program_storage_adapter_stack_realization,
    compose_bound_entry_stack_epochs, derive_generated_program_storage_adapter_live_frame_demand,
};
use calling_conventions::{EntryStackStage, StackDomainRef};
use isa_x86_64::{
    canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
    validate_x86_64_resolved_semantic_unit_wrapper,
};
use std::collections::BTreeSet;

#[test]
fn root_service_reach_substitutes_selected_rows_and_rejects_absence() {
    let selected = selected_interrupt_completion();
    let requirement = "InterruptCompletion::complete".to_owned();
    let reach = ResolvedRootServiceReach::from_selected_provider_closure(
        vec!["Timer".into()],
        vec![requirement.clone()],
        &selected,
    )
    .expect("selected provider closes the root reach");

    assert_eq!(reach.concrete(), ["Timer"]);
    assert_eq!(
        reach.installation_requirements(),
        ["InterruptCompletion::complete"]
    );
    assert_eq!(reach.effective(), ["PortIo", "Timer"]);
    assert_eq!(reach.resolutions().len(), 1);
    assert_eq!(
        reach.selected_provider_closure_report_fingerprint(),
        selected.report_fingerprint()
    );
    assert_eq!(
        reach.selected_provider_closure_digest(),
        selected.identity_digest()
    );

    let error = ResolvedRootServiceReach::from_selected_provider_closure(
        Vec::new(),
        vec!["Missing::requirement".into()],
        &selected,
    )
    .expect_err("an installed root cannot retain an unresolved reach row");
    assert!(error.0.contains("remains unresolved at final admission"));
}

#[test]
fn direct_generated_entry_derives_one_exact_body_epoch_without_provider_attestation() {
    let entry = entry_id(0x801);
    let code = installed_code(0x802, entry);
    let boundary = boundary();
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x81; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: vec![0; 64],
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::from([0x812]),
        admitted_commitments: BTreeSet::from([[0x81; 32], [0x82; 32]]),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x803, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x804, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);
    let bound = bind_direct_generated_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
    )
    .expect("direct generated entry derives its realization");

    assert_eq!(
        bound.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::NoHardwareArrival
    );
    assert_eq!(
        bound.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::None
    );
    assert_eq!(bound.realization_evidence().validation_receipt(), None);
    let StackLocalEvidence::TerminalEntry(bound_body) = bound.body_evidence() else {
        panic!("direct generated entry retains terminal body evidence")
    };
    assert_eq!(
        bound_body.admitted_stack_contribution_report_identities(),
        &BTreeSet::from([0x812]),
    );
    assert_eq!(
        bound_body.admitted_stack_contribution_commitments(),
        &BTreeSet::from([[0x81; 32], [0x82; 32]]),
    );
    let contexts = &bound
        .realization_evidence()
        .realization()
        .realization()
        .contexts;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].epochs.len(), 1);
    assert_eq!(contexts[0].epochs[0].stage, EntryStackStage::Body);
    assert_eq!(
        contexts[0].epochs[0].active_domain,
        StackDomainRef::Interrupted
    );

    let fixed_boundary = interrupted_boundary();
    let error = bind_direct_generated_entry_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&fixed_boundary, &[(1, StackDomainRef::Interrupted)]),
    )
    .expect_err("direct entry cannot replay a closure for another public stack disposition");
    assert!(
        error
            .0
            .contains("drifted from the boundary stack disposition")
    );

    let exact_composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x805, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("exact installed body provenance composes");
    let compose_alternate = |admitted_reports: BTreeSet<u64>,
                             admitted_commitments: BTreeSet<[u8; 32]>| {
        let demand = TestStackDemand {
            identity: psi,
            entry: machine,
            contributing: BTreeSet::from([machine]),
            admitted_reports,
            admitted_commitments,
        };
        let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
            .expect("alternate demand binds its own exact projection");
        let summary = ProviderStackSummary::from_entry(
            root,
            provider,
            boundary.plan().state.stack,
            installed,
        );
        let bound = bind_direct_generated_entry_stack_realization(
            &summary,
            &boundary,
            &code,
            entry,
            body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        )
        .expect("alternate body evidence remains structurally valid");
        compose_bound_entry_stack_epochs(
            &StackNestingRelation {
                identity: root_id(0x805, NestingRelationId::from_normalized_identity),
                edges: BTreeSet::new(),
            },
            [&bound],
        )
        .expect("alternate installed body provenance composes separately")
    };
    let substitute_composition = compose_alternate(
        BTreeSet::from([0x812]),
        BTreeSet::from([[0x81; 32], [0x91; 32]]),
    );
    assert_eq!(
        exact_composition.composition(),
        substitute_composition.composition(),
        "pure epoch composition remains numeric and structural",
    );
    assert_ne!(
        exact_composition, substitute_composition,
        "exact bound composition retains the strong provenance distinction",
    );
    assert_ne!(
        exact_composition.report_fingerprint(),
        substitute_composition.report_fingerprint(),
        "compact-equal admitted reports cannot hide a strong commitment substitution",
    );
    let omitted_composition = compose_alternate(BTreeSet::new(), BTreeSet::new());
    assert_eq!(
        exact_composition.composition(),
        omitted_composition.composition(),
        "foreign-free and admitted-foreign bodies may have equal numeric demand",
    );
    assert_ne!(
        exact_composition, omitted_composition,
        "omitting both provenance sets remains distinguishable from a foreign-bearing body",
    );
    assert_ne!(
        exact_composition.report_fingerprint(),
        omitted_composition.report_fingerprint(),
        "the bound report retains presence versus omission of admitted foreign premises",
    );
}

#[test]
fn installed_entry_stack_rejects_incomplete_or_zero_foreign_provenance() {
    let entry = entry_id(0x806);
    let code = installed_code(0x807, entry);
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x82; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: vec![0; 64],
    };

    for (admitted_reports, admitted_commitments) in [
        (BTreeSet::from([7]), BTreeSet::new()),
        (BTreeSet::new(), BTreeSet::from([[7; 32]])),
        (BTreeSet::from([0]), BTreeSet::from([[7; 32]])),
        (BTreeSet::from([7]), BTreeSet::from([[0; 32]])),
    ] {
        let demand = TestStackDemand {
            identity: psi,
            entry: machine,
            contributing: BTreeSet::from([machine]),
            admitted_reports,
            admitted_commitments,
        };
        let error = bind_installed_entry_stack(&demand, &artifact, &code, entry)
            .expect_err("invalid admitted foreign provenance must fail closed");
        assert!(
            error
                .0
                .contains("incomplete or zero admitted same-stack provenance")
        );
    }
}

#[test]
fn generated_program_storage_adapter_replays_emitted_operations_and_composes_three_epochs() {
    let entry = entry_id(0x831);
    let boundary = generated_program_storage_boundary();
    let request =
        canonical_x86_64_semantic_unit_wrapper_encoding_request(target::NativeTarget::uefi_x64());
    let template =
        encode_x86_64_semantic_unit_wrapper_template(request).expect("canonical wrapper template");
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        &template,
        template.relocation(),
        16,
        32,
    )
    .expect("resolved private continuation call");
    let (code, installed_image) = installed_program_storage_wrapper(0x832, entry, resolved.bytes());
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x83; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: installed_image,
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure binds exact installed bytes");
    let root = root_id(0x833, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x834, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);

    let bound = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect("generated adapter binds exact installed entry and body evidence");

    assert_eq!(
        bound.realization_evidence().arrival_origin(),
        ArrivalStackRealizationOrigin::NoHardwareArrival
    );
    assert_eq!(
        bound.realization_evidence().adapter_origin(),
        AdapterStackRealizationOrigin::GeneratedProgramStorageSemanticWrapper
    );
    assert_eq!(bound.realization_evidence().validation_receipt(), None);
    let generated = bound
        .realization_evidence()
        .generated_adapter()
        .expect("exact generated-adapter evidence");
    assert_eq!(generated.request(), request);
    assert_eq!(generated.resolved_bytes(), resolved.bytes());
    assert_eq!(generated.resolution(), resolved.resolution());
    assert_ne!(generated.report_fingerprint(), 0);
    let epochs = &bound
        .realization_evidence()
        .realization()
        .realization()
        .contexts[0]
        .epochs;
    assert_eq!(
        epochs.iter().map(|epoch| epoch.stage).collect::<Vec<_>>(),
        [
            EntryStackStage::Enter,
            EntryStackStage::Body,
            EntryStackStage::Exit,
        ]
    );
    for epoch in epochs {
        assert_eq!(epoch.active_domain, StackDomainRef::Interrupted);
        assert_eq!(
            epoch.occupancy_by_domain,
            [calling_conventions::StackOccupancy {
                domain: StackDomainRef::Interrupted,
                bytes: 72,
                alignment: 16,
            }]
        );
    }

    let composition = compose_bound_entry_stack_epochs(
        &StackNestingRelation {
            identity: root_id(0x835, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        },
        [&bound],
    )
    .expect("generated adapter epochs compose with the Terminal body");
    assert_eq!(
        composition.domain(StackDomain::Interrupted),
        Some(DomainStackDemand {
            bytes: 144,
            alignment: 16,
        }),
        "the live 72-byte wrapper frame aligns before the 64-byte body WCSU",
    );
}

#[test]
fn generated_program_storage_adapter_derives_live_frame_demand_from_exact_epochs() {
    let bound = generated_program_storage_adapter_bound_input();
    let demand = derive_generated_program_storage_adapter_live_frame_demand(&bound)
        .expect("exact generated wrapper derives one live-frame contribution");
    assert_eq!(demand.bytes(), 72);
    assert_eq!(demand.alignment(), 16);
    assert_eq!(
        demand.installed_code(),
        bound.realization_evidence().installed_code()
    );
    assert_eq!(demand.entry(), bound.realization_evidence().entry());
    assert_eq!(
        demand.semantic_boundary_commitment(),
        bound.realization_evidence().boundary_contract_commitment(),
    );

    let missing_origin = bound.clone().without_generated_adapter_origin_for_test();
    let error = derive_generated_program_storage_adapter_live_frame_demand(&missing_origin)
        .expect_err("a generated-frame claim without generated origin must reject");
    assert!(error.0.contains("wrong realization origin or custody"));

    let drifted_epoch = bound.with_first_adapter_epoch_bytes_for_test(80);
    let error = derive_generated_program_storage_adapter_live_frame_demand(&drifted_epoch)
        .expect_err("an independently valid but drifted epoch must reject");
    assert!(error.0.contains("exact Enter/Body/Exit occupancy"));
}

#[test]
fn generated_program_storage_adapter_rejects_mutation_and_installed_subject_substitution() {
    let entry = entry_id(0x841);
    let boundary = generated_program_storage_boundary();
    let request =
        canonical_x86_64_semantic_unit_wrapper_encoding_request(target::NativeTarget::uefi_x64());
    let template =
        encode_x86_64_semantic_unit_wrapper_template(request).expect("canonical wrapper template");
    let resolved = resolve_x86_64_semantic_unit_wrapper_private_continuation(
        &template,
        template.relocation(),
        16,
        32,
    )
    .expect("resolved wrapper");
    let (code, installed_image) = installed_program_storage_wrapper(0x842, entry, resolved.bytes());
    let machine = semantic_vocabulary::MachineId::new(1).expect("machine identity");
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x84; 32]),
    };
    let artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: installed_image,
    };
    let demand = TestStackDemand {
        identity: psi,
        entry: machine,
        contributing: BTreeSet::from([machine]),
        admitted_reports: BTreeSet::new(),
        admitted_commitments: BTreeSet::new(),
    };
    let installed = bind_installed_entry_stack(&demand, &artifact, &code, entry)
        .expect("terminal stack closure");
    let root = root_id(0x843, ExternalRootId::from_normalized_identity);
    let provider = root_id(0x844, RootProviderId::from_normalized_identity);
    let summary =
        ProviderStackSummary::from_entry(root, provider, boundary.plan().state.stack, installed);

    let mut mutated_template = template.bytes().to_vec();
    mutated_template[3] ^= 8;
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: &mutated_template,
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("reserve-byte mutation must reject before epoch derivation");
    assert!(
        error
            .0
            .contains("template failed exact emitted-operation replay")
    );

    let mut mutated_resolved = resolved.bytes().to_vec();
    mutated_resolved[81] ^= 1;
    assert!(
        validate_x86_64_resolved_semantic_unit_wrapper(
            &template,
            template.relocation(),
            16,
            32,
            &mutated_resolved,
        )
        .is_err(),
        "redirecting the resolved private call must fail target replay",
    );
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: &mutated_resolved,
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("resolved call redirection must reject before epoch derivation");
    assert!(
        error
            .0
            .contains("continuation call failed exact emitted-operation replay")
    );

    let mut changed_installed_wrapper = resolved.bytes().to_vec();
    changed_installed_wrapper[4] ^= 1;
    let (changed_code, changed_image) =
        installed_program_storage_wrapper(0x842, entry, &changed_installed_wrapper);
    let changed_artifact = TestObject {
        identity: psi,
        entry: machine,
        bytes: changed_image,
    };
    let changed_installed =
        bind_installed_entry_stack(&demand, &changed_artifact, &changed_code, entry)
            .expect("mutated installed image has its own exact Terminal body evidence");
    let changed_summary = ProviderStackSummary::from_entry(
        root,
        provider,
        boundary.plan().state.stack,
        changed_installed,
    );
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &changed_summary,
        &boundary,
        &changed_code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("validated emitted bytes cannot substitute a mutated installed entry interval");
    assert!(error.0.contains("exact installed entry interval"));

    let foreign_entry = entry_id(0x845);
    let foreign_code = installed_code(0x846, foreign_entry);
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &boundary,
        &foreign_code,
        foreign_entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("body evidence cannot cross to another installed artifact and entry");
    assert!(error.0.contains("different installed entry"));

    let incompatible_boundary = crate::tests::boundary();
    let error = bind_x86_64_generated_program_storage_adapter_stack_realization(
        &summary,
        &incompatible_boundary,
        &code,
        entry,
        body_domains(&boundary, &[(1, StackDomainRef::Interrupted)]),
        X86_64GeneratedProgramStorageAdapterEmission {
            request,
            template_bytes: template.bytes(),
            resolved_bytes: resolved.bytes(),
            wrapper_section_offset: 16,
            continuation_section_offset: 32,
        },
    )
    .expect_err("the generated wrapper cannot bind a different continuation ABI");
    assert!(error.0.contains("receiver-free Microsoft-x64"));
}
