use super::{
    Fixture, INSTALL_NAME, SCALAR_SYMBOL, SYMBOL, admit_import,
    assert_d41_parent_mutation_rejected, assert_physical_child_mutation_rejected,
    assert_physical_children_mutation_rejected, install_flattened_macho_image,
    replay_native_artifact_parts, terminal_authority_permission_policy, terminal_authority_policy,
};
use compiler::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
use native_realization as native;
use target::ForeignLocatorCandidate;
use task_plans::SameStackContributionAdmissionReceiptId;

#[test]
fn retained_source_evaluated_import_realizes_exact_macho_image() {
    let fixture = Fixture::new();
    let missing = fixture.compile_terminal();
    let missing_policy = terminal_authority_policy(&missing);
    let missing_permission_policy = terminal_authority_permission_policy(&missing);
    let diagnostics = {
        let retained = missing;
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: missing_policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(missing_permission_policy),
                image_request,
                imports: &[],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .expect_err("a demanded source-evaluated import requires external custody");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no supplied execution"))
    );

    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_4f05)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let policy_identity = policy.identity();
    let artifact = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &admission.execution,
                    &admission.same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!(
            "externally admitted import should realize a native Mach-O artifact:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact.validate().expect("native artifact replays");
    artifact
        .validate_for_terminal_authority_policy(policy_identity)
        .expect("native artifact retains the exact accepted foreign policy");
    assert_eq!(artifact.target(), target::NativeTarget::macos_arm64());
    assert_eq!(artifact.provider_executions().len(), 1);
    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one retained Mach-O foreign call expected")
    };
    assert_eq!(foreign_call.x86_floating_control, None);
    let control = foreign_call
        .aarch64_floating_control
        .expect("source-evaluated Mach-O call preserves complete FPCR");
    assert_eq!(control.target, target::NativeTarget::macos_arm64());
    assert_eq!(control.save_byte_count, 8);
    assert_eq!(control.restore_byte_count, 8);
    assert!(control.save_offset < foreign_call.text_offset);
    assert!(foreign_call.text_offset + 4 <= control.restore_offset);
    assert_eq!(artifact.image().output().format, "mach-o-arm64-executable");
    assert_eq!(artifact.image().output().final_image_imports, 1);
    assert_eq!(
        artifact.image().output().final_data_bytes,
        [0; 8],
        "one referenced Mach-O import has one exact lazy-binding pointer slot",
    );
    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized object import expected")
    };
    let ForeignLocatorCandidate::MachODylibSymbol {
        install_name,
        symbol,
    } = normalized.locator.locator()
    else {
        panic!("structured Mach-O locator must survive object construction")
    };
    assert_eq!(install_name, INSTALL_NAME);
    assert_eq!(symbol, SYMBOL);

    assert!(
        matches!(
            artifact.physical_evidence_scope(),
            native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
        ),
        "source-reviewed imports require complete D32 custody under the validated projection",
    );
    let physical = artifact
        .physical_evidence()
        .expect("source-evaluated import retains complete D32 evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 0);
    let [boundary_occurrence] = physical.projection().boundary_occurrences() else {
        panic!("one source boundary occurrence expected")
    };
    let [child] = physical.children() else {
        panic!("one source boundary call must produce exactly one D41 child")
    };
    assert_eq!(
        child.occurrence(),
        native::NativePhysicalOccurrence::Boundary(boundary_occurrence.identity()),
    );
    assert_eq!(child.projection(), physical.projection().identity());
    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("source-evaluated import must retain its D41 settlement parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement
    );
    assert_eq!(parent.target(), artifact.target());
    let [selected_plan] = artifact.selected_provider_plans() else {
        panic!("one exact selected provider plan expected")
    };
    assert_eq!(parent.selected_plan_digest(), selected_plan.plan_digest());
    assert_eq!(
        *parent.selected_plan_digest().as_bytes(),
        admission.same_stack.provider_plan_commitment().as_bytes(),
        "D41 parent and opaque same-stack leaf must bind the same selected plan",
    );

    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("source-evaluated import must be an admitted-provider D41 role")
    };
    let execution_record = foreign_call.provider_execution;
    let expected_execution = target_operations::ProviderExecutionBinding::from_execution_record(
        target_operations::ProviderPlanReportIdentity::new(
            execution_record.provider_plan_report_identity,
        )
        .expect("selected provider-plan report identity"),
        execution_record.provider_execution_report_identity,
        execution_record.provider_execution_report_fingerprint,
        execution_record.normalized_root_report_identity,
        execution_record.boundary_contract_report_fingerprint,
    )
    .expect("complete admitted-provider execution record");
    assert_eq!(*execution, expected_execution);
    assert_eq!(parent.execution(), expected_execution.into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(realization.locator, normalized.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.same_stack_contribution, admission.same_stack,);
    assert_eq!(
        realization.same_stack_contribution,
        foreign_call.same_stack_contribution,
    );
    let [image_foreign_call] = artifact.image().foreign_calls() else {
        panic!("one final-image foreign call expected")
    };
    assert_eq!(image_foreign_call, foreign_call);

    let object_function = artifact
        .object()
        .functions()
        .iter()
        .find(|function| function.machine == foreign_call.machine)
        .expect("foreign call owning object function");
    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Operation(parent.occurrence().operation())
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the zero-argument import")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved object relocation must target the normalized import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset,
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(child.final_image_span(), child.object_span());
    assert_eq!(
        child.machine_span().byte_count(),
        attribution.attribution.byte_count,
    );
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    let object_interval_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_interval_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);
    let machine_span = child.machine_span();
    let object_span = child.object_span();
    assert_eq!(
        &object_function.bytes(artifact.object())
            [machine_span.offset()..machine_span.offset() + machine_span.byte_count()],
        &artifact.object().text_bytes()
            [object_span.offset()..object_span.offset() + object_span.byte_count()],
    );
    let object_instruction = u32::from_le_bytes(
        artifact.object().text_bytes()
            [object_relocation.offset..object_relocation.offset + object_relocation.byte_width]
            .try_into()
            .expect("one AArch64 branch instruction"),
    );
    let final_instruction = u32::from_le_bytes(
        artifact.image().output().final_text_bytes
            [object_relocation.offset..object_relocation.offset + object_relocation.byte_width]
            .try_into()
            .expect("one final AArch64 branch instruction"),
    );
    assert_eq!(object_instruction & 0xfc00_0000, 0x9400_0000);
    assert_eq!(final_instruction & 0xfc00_0000, 0x9400_0000);
    assert_ne!(
        object_instruction, final_instruction,
        "Mach-O import lowering must relocate the admitted call to its exact image thunk",
    );

    let native::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(field) =
        child.relocation()
    else {
        panic!("admitted foreign call must retain unresolved import-field custody")
    };
    let boundary_plan_identity = calling_conventions::validate_boundary_entry_plan(
        realization.boundary_entry_plan.clone(),
        &calling_conventions::CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .expect("retained zero-argument boundary entry plan revalidates")
    .contract_commitment_digest();
    assert_eq!(
        *field.locator_identity(),
        realization.locator.identity_digest().as_bytes(),
    );
    assert_eq!(field.boundary_plan_identity(), &boundary_plan_identity);
    assert_eq!(field.caller(), boundary_occurrence.machine());
    assert_eq!(field.operation(), boundary_occurrence.operation());
    assert_eq!(field.offset(), object_relocation.offset);
    assert_eq!(field.byte_width(), object_relocation.byte_width);
    assert_eq!(field.addend(), object_relocation.addend);
    assert_eq!(field.kind(), object_relocation.kind);
    assert_ne!(field.final_image_symbol_identity(), &[0; 32]);

    let object_demand =
        image_emission::derive_stack_demand(artifact.object(), artifact.object().entry())
            .expect("object stack demand includes the admitted foreign leaf");
    assert!(
        object_demand
            .admitted_contribution_report_identities()
            .contains(&admission.same_stack.report_identity())
    );
    assert!(
        object_demand
            .admitted_contribution_commitments()
            .contains(&admission.same_stack.commitment())
    );

    let installation =
        image_emission::build_installation_record_with_selected_provider_plans_and_evidence(
            artifact.image(),
            semantic_vocabulary::ProfileDecisionId::new(1).expect("profile decision"),
            artifact
                .selected_provider_plans()
                .iter()
                .map(|plan| plan.report_identity()),
            artifact.provider_executions().iter(),
            None,
            boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        )
        .expect("installation retains the admitted foreign stack projection");
    let foreign_stacks = installation
        .functions()
        .iter()
        .flat_map(|function| &function.foreign_call_stacks)
        .collect::<Vec<_>>();
    let [foreign_stack] = foreign_stacks.as_slice() else {
        panic!("one installed foreign stack contribution expected")
    };
    assert_eq!(
        foreign_stack.provider_plan_report_identity,
        admission.plan_report_identity
    );
    assert_eq!(
        foreign_stack.contribution_report_identity,
        admission.same_stack.report_identity()
    );
    assert_eq!(
        foreign_stack.contribution_commitment,
        admission.same_stack.commitment()
    );
    assert_eq!(
        foreign_stack.contribution_bytes,
        admission.same_stack.bytes()
    );
    assert_eq!(
        foreign_stack.contribution_alignment,
        admission.same_stack.alignment()
    );

    let encoded = image_emission::encode_installation_record(&installation)
        .expect("foreign stack installation encodes");
    let decoded = image_emission::decode_installation_record(&encoded)
        .expect("foreign stack installation decodes");
    image_emission::validate_installation_record(&decoded, artifact.image())
        .expect("decoded foreign stack installation rejoins the exact image");
    let installation_demand = image_emission::derive_installation_stack_demand(
        &decoded,
        artifact.image(),
        artifact.object().entry(),
    )
    .expect("installation replays the admitted foreign stack demand");
    assert_eq!(installation_demand, object_demand);

    // Complete imported-image custody must hold before installed publication:
    // the placed executable/data inventories classify every final byte, the
    // exercised import thunk decodes to its exact paired binding slot, and the
    // installed occurrence binds the complete encoded and materialized images.
    // A matching compiler prefix plus a resolver-claimed thunk address is not
    // placement custody.
    let memory = image_emission::project_installed_artifact_memory_images(
        artifact.object(),
        artifact.image(),
    )
    .expect("complete imported-image placement projects over the final image");
    let thunk_offset = {
        // The import thunk is the only writer-appended executable region: it
        // occupies the final twelve bytes of placed text.
        let output = artifact.image().output();
        let offset = output
            .final_text_bytes
            .len()
            .checked_sub(12)
            .expect("one appended thunk");
        let matches = output
            .executable_regions
            .regions
            .iter()
            .filter(|region| region.section_offset == offset && region.byte_count == 12)
            .count();
        assert_eq!(matches, 1, "exactly one placed import thunk ends the text");
        u64::try_from(offset).expect("thunk offset")
    };
    {
        let slots = artifact
            .image()
            .output()
            .data_regions
            .regions
            .iter()
            .filter(|region| region.byte_count == 8)
            .count();
        assert_eq!(slots, 1, "exactly one placed import binding slot");
    }
    // The admitted call must branch to the placed thunk start.
    let final_branch = u32::from_le_bytes(
        artifact.image().output().final_text_bytes
            [object_relocation.offset..object_relocation.offset + 4]
            .try_into()
            .expect("final branch instruction"),
    );
    let branch_target =
        object_relocation.offset as i64 + i64::from((final_branch << 6) as i32 >> 4);
    assert_eq!(
        branch_target,
        i64::try_from(thunk_offset).expect("thunk offset"),
        "the admitted call must branch to the placed import thunk",
    );

    const PLACEMENT_BASE: u64 = 0x0001_0000;
    let entry_stub =
        layout_plans::EntryStubId::from_normalized_identity(0x5101).expect("entry stub");
    let thunk_stub =
        layout_plans::EntryStubId::from_normalized_identity(0x5102).expect("thunk entry stub");
    let entry_offset =
        u64::try_from(artifact.object().entry_function().text_offset).expect("entry text offset");
    let import_relocation = executable_installation::DecodedArtifactRelocation {
        kind: executable_installation::ArtifactRelocationKind::Aarch64Branch26,
        destination_offset: u64::try_from(object_relocation.offset).expect("relocation offset"),
        target: layout_plans::RelocationTarget::Entry(thunk_stub),
        addend: object_relocation.addend,
    };
    let install_complete = || {
        install_flattened_macho_image(
            memory.encoded().to_vec(),
            vec![
                executable_installation::ArtifactEntry::from_canonical_decode(
                    entry_stub,
                    entry_offset,
                ),
                executable_installation::ArtifactEntry::from_canonical_decode(
                    thunk_stub,
                    thunk_offset,
                ),
            ],
            vec![import_relocation],
            PLACEMENT_BASE,
            move |target| {
                (target == layout_plans::RelocationTarget::Entry(thunk_stub))
                    .then_some(PLACEMENT_BASE + thunk_offset)
            },
        )
    };
    // The source-imported component reaches installed publication only with
    // complete custody: canonical bytes keep the compiler prefix plus the
    // emitted thunk and zeroed binding slot, and the frozen image equals the
    // complete relocated final image.
    let bound = image_emission::bind_installed_artifact(
        artifact.object().clone(),
        artifact.image().clone(),
        installation.clone(),
        &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        install_complete(),
    )
    .expect("the imported Mach-O image binds complete installed-code custody");
    assert_eq!(bound.installation(), &installation);
    assert_eq!(bound.image().output().final_image_imports, 1);

    // A resolver-returned thunk address cannot substitute for retained
    // placement: an artifact whose code omits the thunk and binding slot —
    // while its resolver still claims the thunk's would-be address — must not
    // reach installed publication.
    let truncated = install_flattened_macho_image(
        artifact.object().text_bytes().to_vec(),
        vec![
            executable_installation::ArtifactEntry::from_canonical_decode(entry_stub, entry_offset),
        ],
        vec![import_relocation],
        PLACEMENT_BASE,
        move |target| {
            (target == layout_plans::RelocationTarget::Entry(thunk_stub))
                .then_some(PLACEMENT_BASE + thunk_offset)
        },
    );
    let error = image_emission::bind_installed_artifact(
        artifact.object().clone(),
        artifact.image().clone(),
        installation.clone(),
        &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        truncated,
    )
    .expect_err("a compiler-prefix artifact whose resolver claims an uninstalled thunk address must not bind");
    assert!(
        error.diagnostic().contains("materialized"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    // With complete canonical bytes, a resolver that substitutes a different
    // in-extent address for the thunk materializes a different branch and
    // still cannot join the bound image.
    let misresolved = install_flattened_macho_image(
        memory.encoded().to_vec(),
        vec![
            executable_installation::ArtifactEntry::from_canonical_decode(entry_stub, entry_offset),
            executable_installation::ArtifactEntry::from_canonical_decode(thunk_stub, thunk_offset),
        ],
        vec![import_relocation],
        PLACEMENT_BASE,
        move |target| {
            (target == layout_plans::RelocationTarget::Entry(thunk_stub))
                .then_some(PLACEMENT_BASE + thunk_offset - 4)
        },
    );
    assert!(
        image_emission::bind_installed_artifact(
            artifact.object().clone(),
            artifact.image().clone(),
            installation.clone(),
            &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
            misresolved,
        )
        .is_err(),
        "a substituted thunk resolution must not materialize the bound image",
    );

    // Every retained placement row is required independently: removing or
    // substituting either the thunk's or the binding slot's placed region
    // must reject the image for both record validation and the installed
    // artifact join.
    let thunk_offset_usize = usize::try_from(thunk_offset).expect("thunk offset");
    let slot_offset = artifact
        .image()
        .output()
        .data_regions
        .regions
        .iter()
        .find(|region| region.byte_count == 8)
        .expect("placed binding slot")
        .section_offset;
    let placement_mutations: Vec<(&str, Box<dyn Fn(&mut image_emission::ExecutableImage)>)> = vec![
        (
            "missing thunk placement",
            Box::new(move |image| {
                image
                    .output_mut_for_test()
                    .executable_regions
                    .regions
                    .retain(|region| {
                        !(region.section_offset == thunk_offset_usize && region.byte_count == 12)
                    });
            }),
        ),
        (
            "missing binding-slot placement",
            Box::new(move |image| {
                image
                    .output_mut_for_test()
                    .data_regions
                    .regions
                    .retain(|region| {
                        !(region.section_offset == slot_offset && region.byte_count == 8)
                    });
            }),
        ),
        (
            "substituted thunk placement",
            Box::new(move |image| {
                let region = image
                    .output_mut_for_test()
                    .executable_regions
                    .regions
                    .iter_mut()
                    .find(|region| {
                        region.section_offset == thunk_offset_usize && region.byte_count == 12
                    })
                    .expect("placed thunk region");
                region.address += 0x2000;
            }),
        ),
        (
            "substituted binding-slot placement",
            Box::new(move |image| {
                let region = image
                    .output_mut_for_test()
                    .data_regions
                    .regions
                    .iter_mut()
                    .find(|region| region.section_offset == slot_offset && region.byte_count == 8)
                    .expect("placed binding-slot region");
                region.address += 8;
            }),
        ),
    ];
    for (label, mutate) in placement_mutations {
        let mut mutated = artifact.image().clone();
        mutate(&mut mutated);
        assert_eq!(
            image_emission::validate_installation_record(&installation, &mutated),
            Err(image_emission::InstallationError::InvalidImagePlacementCustody),
            "{label}: complete-custody replay must reject it",
        );
        assert!(
            image_emission::bind_installed_artifact(
                artifact.object().clone(),
                mutated,
                installation.clone(),
                &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
                install_complete(),
            )
            .is_err(),
            "{label}: the installed artifact join must reject it",
        );
    }

    let parts = artifact.into_parts();
    let mut missing_provider = replay_native_artifact_parts(&parts);
    missing_provider.provider_executions.clear();
    assert!(
        native::NativeArtifact::from_replayed_parts(missing_provider).is_err(),
        "D41 replay must reject removal of its admitted provider execution",
    );
    let mut duplicate_provider = replay_native_artifact_parts(&parts);
    duplicate_provider
        .provider_executions
        .push(duplicate_provider.provider_executions[0].clone());
    assert!(
        native::NativeArtifact::from_replayed_parts(duplicate_provider).is_err(),
        "D41 replay must reject duplicate admitted provider execution",
    );

    let mut missing_child = replay_native_artifact_parts(&parts);
    let evidence = missing_child
        .physical_evidence
        .take()
        .expect("admitted foreign-call physical evidence")
        .into_parts();
    missing_child.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
        native::NativePhysicalEvidenceParts {
            projection: evidence.projection,
            children: Vec::new(),
            identity: evidence.identity,
        },
    ));
    assert!(
        native::NativeArtifact::from_replayed_parts(missing_child).is_err(),
        "D32 replay must reject a missing D41 child",
    );

    let mut duplicate_child = replay_native_artifact_parts(&parts);
    let evidence = duplicate_child
        .physical_evidence
        .take()
        .expect("admitted foreign-call physical evidence")
        .into_parts();
    let mut children = evidence.children;
    children.push(children[0].clone());
    duplicate_child.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
        native::NativePhysicalEvidenceParts {
            projection: evidence.projection,
            children,
            identity: evidence.identity,
        },
    ));
    assert!(
        native::NativeArtifact::from_replayed_parts(duplicate_child).is_err(),
        "D32 replay must reject duplicate D41 children",
    );

    assert_d41_parent_mutation_rejected(&parts, |parent| {
        parent.requirement_identity.push_str("::substituted");
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        parent.selected_plan_digest =
            native::NativeSelectedProviderPlanDigest::from_digest([7; 32]);
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { execution, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        *execution = target_operations::ProviderExecutionBinding::from_execution_record(
            target_operations::ProviderPlanReportIdentity::new(admission.plan_report_identity)
                .expect("selected provider-plan report identity"),
            0x4d41_4348_ffff,
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        )
        .expect("substituted provider execution is structurally complete");
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.locator = target::normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: INSTALL_NAME.to_vec(),
                symbol: b"_substituted_getpid".to_vec(),
            },
            target::TargetProfile::MacosArm64,
        )
        .expect("substituted locator remains structurally valid");
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.state.preemption =
            calling_conventions::Preemption::ProviderDefined;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.machine_bytes_digest[0] ^= 1;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.object_bytes_digest[0] ^= 1;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.final_image_bytes_digest[0] ^= 1;
    });
}

#[test]
fn retained_terminal_import_rejects_optimization_selection_substitution() {
    for (label, fixture, receipt_identity) in [
        (
            "unit",
            Fixture::new_named("optimized-macho"),
            0x4d41_4348_5f05,
        ),
        (
            "fixed-u32-argument",
            Fixture::new_macos_u32_argument_named("optimized-macho-u32-argument"),
            0x4d41_4348_6f05,
        ),
        (
            "fixed-i32-result",
            Fixture::new_macos_i32_result_named("optimized-macho-i32-result"),
            0x4d41_4348_7f05,
        ),
    ] {
        let retained = fixture.compile_terminal();
        let admission = admit_import(
            &retained,
            SameStackContributionAdmissionReceiptId::from_normalized_identity(receipt_identity)
                .unwrap(),
        );
        let policy = terminal_authority_policy(&retained);
        let permission_policy = terminal_authority_permission_policy(&retained);
        let optimizations = optimization_core::PostTerminalOptimizationSelections::new(
            optimization_core::OptimizationSelections::new([
                optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .expect("one physical optimization selection"),
        )
        .expect("one post-Terminal optimization selection");
        let diagnostics = {
            let image_request = native_realization::ExecutableImageEmissionRequest::direct(
                retained
                    .native_realization_proposal()
                    .expect("native proposal")
                    .subsystem(),
            );
            realize_retained_native_artifact(
                retained,
                RetainedNativeRealizationRequest {
                    profile: &proof_admission::AdmissionProfile::default(),
                    optimization_selections: &optimizations,
                    terminal_authority_policy: policy,
                    accepted_package_terminal_authority_permission_policy:
                        native_realization::current_terminal_authority_permission_policy(),
                    terminal_authority_permission_policy: Some(permission_policy),
                    image_request,
                    imports: &[SourceEvaluatedImportSettlement::new(
                        &admission.execution,
                        &admission.same_stack,
                    )],
                },
            )
            .map(|artifact| match artifact {
                native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
                native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                    panic!("direct image request returned dynamic ELF custody")
                }
            })
            .map_err(|(_, diagnostics)| diagnostics)
        }
        .expect_err("a retained Terminal proposal must not accept a substituted selection");
        assert_eq!(diagnostics.len(), 1, "unexpected diagnostics for {label}");
        assert!(
            diagnostics[0]
                .message
                .contains("selections differ from the exact post-Terminal build proposal"),
            "unexpected diagnostic for {label}: {}",
            diagnostics[0].message
        );
    }
}

#[test]
fn retained_source_evaluated_fixed_u32_import_requires_complete_d32_custody() {
    let fixture = Fixture::new_macos_u32_argument();
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_0605)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let artifact = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &admission.execution,
                    &admission.same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!(
            "fixed-scalar admitted import should realize complete D32 evidence:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact
        .validate()
        .expect("fixed-scalar native artifact replays");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("fixed-scalar import retains complete D32 evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 0);
    assert_eq!(physical.projection().boundary_occurrences().len(), 1);
    let [child] = physical.children() else {
        panic!("one fixed-scalar source call must produce exactly one D41 child")
    };
    assert!(matches!(
        child.occurrence(),
        native::NativePhysicalOccurrence::Boundary(_),
    ));
    assert_eq!(child.projection(), physical.projection().identity());

    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one fixed-scalar Mach-O foreign call expected")
    };
    assert_eq!(foreign_call.boundary_entry_plan.call.parameters.len(), 1);
    let [scalar_argument] = foreign_call.scalar_arguments.as_slice() else {
        panic!("one exact fixed-scalar object argument row expected")
    };
    assert_eq!(scalar_argument.parameter_index, 0);
    assert_eq!(
        scalar_argument.placement,
        foreign_call.boundary_entry_plan.call.parameters[0],
    );
    assert!(matches!(
        scalar_argument.source,
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(3),
            ..
        } if scalar_type
            == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32).unwrap()
    ));
    let ForeignLocatorCandidate::MachODylibSymbol {
        install_name,
        symbol,
    } = foreign_call.locator.locator()
    else {
        panic!("structured scalar Mach-O locator must survive object construction")
    };
    assert_eq!(install_name, INSTALL_NAME);
    assert_eq!(symbol, SCALAR_SYMBOL);

    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("fixed-scalar import must retain its D41 settlement parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement
    );
    assert_eq!(parent.target(), artifact.target());
    assert_eq!(
        parent.occurrence().operation_ordinal(),
        foreign_call.operation_ordinal,
    );
    let [selected_plan] = artifact.selected_provider_plans() else {
        panic!("one exact fixed-scalar provider plan expected")
    };
    assert_eq!(parent.selected_plan_digest(), selected_plan.plan_digest());
    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("fixed-scalar import must retain admitted-provider D41 custody")
    };
    assert_eq!(parent.execution(), (*execution).into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.boundary_entry_plan.call.parameters.len(), 1);
    assert_eq!(realization.same_stack_contribution, admission.same_stack);
    assert_eq!(
        realization.same_stack_contribution,
        foreign_call.same_stack_contribution,
    );

    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Operation(parent.occurrence().operation())
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the fixed-scalar import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    let object_interval_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= scalar_argument.code_offset);
    assert!(scalar_argument.code_offset + scalar_argument.byte_count <= object_interval_end);
    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized fixed-scalar object import expected")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved object relocation must target the scalar import")
    };
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_interval_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);

    let parts = artifact.into_parts();
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.parameters.clear();
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.parameters[0]
            .locations
            .clear();
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.machine_bytes_digest[0] ^= 1;
    });

    // The survivor/physical-child bijection must reject every child-set
    // mutation class: a dropped survivor, a duplicated child, a substituted
    // occurrence, and a child claiming a foreign parent role.
    assert_physical_children_mutation_rejected(&parts, |children| children.clear());
    assert_physical_children_mutation_rejected(&parts, |children| {
        children.push(children[0].clone());
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.occurrence = native::NativePhysicalOccurrence::Boundary(
            optimization_core::OptimizedBoundaryOccurrenceIdentity::from_bytes([0xa5; 32]),
        );
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.occurrence = native::NativePhysicalOccurrence::Operator(
            optimization_core::OptimizedOperatorOccurrenceIdentity::from_bytes([0x5a; 32]),
        );
    });
}

#[test]
fn retained_source_evaluated_fixed_i32_result_requires_complete_d32_custody() {
    let fixture = Fixture::new_macos_i32_result();
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_0705)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let artifact = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &admission.execution,
                    &admission.same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!(
            "fixed-result admitted import should realize complete D32 evidence:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact.validate().expect("fixed-result artifact replays");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("fixed-result import retains complete D32 evidence");
    let [child] = physical.children() else {
        panic!("one fixed-result source call must produce one D41 child")
    };
    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one fixed-result object call expected")
    };
    assert!(foreign_call.scalar_arguments.is_empty());
    let scalar_result = foreign_call
        .scalar_result
        .as_ref()
        .expect("object call retains its fixed scalar result");
    let i32_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap();
    let i32_shape = calling_conventions::ValueShape::integer(4, 4);
    assert_eq!(
        scalar_result.home.scalar_type,
        semantic_vocabulary::ScalarType::Integer(i32_type)
    );
    assert_eq!(scalar_result.home.shape, i32_shape);
    assert_eq!(
        foreign_call.boundary_entry_plan.call.result.as_ref(),
        Some(&scalar_result.source),
    );

    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("fixed-result import must retain its D41 parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement,
    );
    assert_eq!(
        parent.occurrence().operation_ordinal(),
        foreign_call.operation_ordinal,
    );
    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("fixed-result import must retain admitted-provider custody")
    };
    assert_eq!(parent.execution(), (*execution).into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.same_stack_contribution, admission.same_stack);

    let module = terminal_codec::decode_module(artifact.psi_artifact().semantic_bytes())
        .expect("decode fixed-result Terminal semantics");
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == parent.occurrence().machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == parent.occurrence().operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        panic!("D41 parent must rejoin one Terminal result producer")
    };
    let terminal_psi::OperationResult::Scalar(terminal_result) = operation.result else {
        panic!("Terminal boundary call must retain its scalar result")
    };
    assert_eq!(terminal_result.id, scalar_result.home.source_value);
    assert_eq!(
        terminal_result.scalar_type,
        semantic_vocabulary::ScalarType::Integer(i32_type),
    );

    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Operation(operation.id)
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the fixed-result import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset,
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    assert_eq!(child.final_image_span(), child.object_span());
    let object_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= scalar_result.code_offset);
    assert!(scalar_result.code_offset + scalar_result.byte_count <= object_end);

    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized fixed-result import expected")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved relocation must target the fixed-result import")
    };
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);
    let native::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(field) =
        child.relocation()
    else {
        panic!("fixed-result child must retain unresolved import-field custody")
    };
    assert_eq!(field.offset(), object_relocation.offset);
    assert_eq!(field.byte_width(), object_relocation.byte_width);
    let [image_foreign_call] = artifact.image().foreign_calls() else {
        panic!("one fixed-result image call expected")
    };
    assert_eq!(image_foreign_call, foreign_call);

    let parts = artifact.into_parts();
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.result = None;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.final_image_bytes_digest[0] ^= 1;
    });
}

fn macho_scalar_import_fixture(name: &str, member: &str, call: &str) -> Fixture {
    let leaf_name = member.split('(').next().unwrap();
    let signature = &member[leaf_name.len()..];
    let symbol_len = leaf_name.len() + 1;
    Fixture::with_source(
        name,
        "macos_arm64",
        &format!(
            r#"use omega::language::core::service;
use omega::language::core::external_binding;


pub boundary trait Boundary {{
    machine {member};
}}

macos_arm64 machine {leaf_name}_binding() -> ForeignBinding<26, {symbol_len}, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::MachODylibSymbol {{
            install_name: "/usr/lib/libSystem.B.dylib",
            symbol: "_{leaf_name}",
        }},
    }}
}}

machine {leaf_name}_leaf{signature} satisfies Boundary::{leaf_name} via {leaf_name}_binding();

data Main {{ boundary: Binding<Boundary>; }}
machine Main::main(&mut self) reaches Boundary {{
    {call}
}}
"#
        ),
        &format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("source-evaluated-macho-{name}-native");
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}}
"#
        ),
    )
}

#[test]
fn retained_source_evaluated_fixed_u64_argument_requires_complete_scalar_custody() {
    let fixture = macho_scalar_import_fixture(
        "u64-argument",
        "wait64(seconds: u64)",
        "self.boundary.wait64(3);",
    );
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_0805)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let artifact = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &admission.execution,
                    &admission.same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!(
            "fixed-scalar admitted import should realize complete evidence:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact
        .validate()
        .expect("fixed-scalar native artifact replays");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("fixed-scalar import retains complete evidence");
    assert_eq!(physical.projection().operator_occurrences().len(), 0);
    assert_eq!(physical.projection().boundary_occurrences().len(), 1);
    let [child] = physical.children() else {
        panic!("one fixed-scalar source call must produce exactly one D41 child")
    };
    assert!(matches!(
        child.occurrence(),
        native::NativePhysicalOccurrence::Boundary(_),
    ));
    assert_eq!(child.projection(), physical.projection().identity());

    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one fixed-scalar Mach-O foreign call expected")
    };
    assert_eq!(foreign_call.boundary_entry_plan.call.parameters.len(), 1);
    let [scalar_argument] = foreign_call.scalar_arguments.as_slice() else {
        panic!("one exact fixed-scalar object argument row expected")
    };
    assert_eq!(scalar_argument.parameter_index, 0);
    assert_eq!(
        scalar_argument.placement,
        foreign_call.boundary_entry_plan.call.parameters[0],
    );
    assert!(matches!(
        scalar_argument.source,
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(3),
            ..
        } if scalar_type
            == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).unwrap()
    ));
    let ForeignLocatorCandidate::MachODylibSymbol {
        install_name,
        symbol,
    } = foreign_call.locator.locator()
    else {
        panic!("structured scalar Mach-O locator must survive object construction")
    };
    assert_eq!(install_name, INSTALL_NAME);
    assert_eq!(symbol, b"_wait64");

    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("fixed-scalar import must retain its D41 settlement parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement
    );
    assert_eq!(parent.target(), artifact.target());
    assert_eq!(
        parent.occurrence().operation_ordinal(),
        foreign_call.operation_ordinal,
    );
    let [selected_plan] = artifact.selected_provider_plans() else {
        panic!("one exact fixed-scalar provider plan expected")
    };
    assert_eq!(parent.selected_plan_digest(), selected_plan.plan_digest());
    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("fixed-scalar import must retain admitted-provider D41 custody")
    };
    assert_eq!(parent.execution(), (*execution).into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.boundary_entry_plan.call.parameters.len(), 1);
    assert_eq!(realization.same_stack_contribution, admission.same_stack);
    assert_eq!(
        realization.same_stack_contribution,
        foreign_call.same_stack_contribution,
    );

    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Operation(parent.occurrence().operation())
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the fixed-scalar import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    let object_interval_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= scalar_argument.code_offset);
    assert!(scalar_argument.code_offset + scalar_argument.byte_count <= object_interval_end);
    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized fixed-scalar object import expected")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved object relocation must target the scalar import")
    };
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_interval_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);

    let parts = artifact.into_parts();
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.parameters.clear();
    });
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.parameters[0]
            .locations
            .clear();
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.machine_bytes_digest[0] ^= 1;
    });
    assert_physical_children_mutation_rejected(&parts, |children| children.clear());
    assert_physical_children_mutation_rejected(&parts, |children| {
        children.push(children[0].clone());
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.occurrence = native::NativePhysicalOccurrence::Boundary(
            optimization_core::OptimizedBoundaryOccurrenceIdentity::from_bytes([0xa5; 32]),
        );
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.occurrence = native::NativePhysicalOccurrence::Operator(
            optimization_core::OptimizedOperatorOccurrenceIdentity::from_bytes([0x5a; 32]),
        );
    });
}

#[test]
fn retained_source_evaluated_fixed_i64_result_requires_complete_scalar_custody() {
    let fixture = macho_scalar_import_fixture(
        "i64-result",
        "poll64() -> i64",
        "let observed: i64 = self.boundary.poll64();",
    );
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x4d41_4348_0905)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let artifact = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &admission.execution,
                    &admission.same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!(
            "fixed-result admitted import should realize complete evidence:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    artifact.validate().expect("fixed-result artifact replays");
    assert!(matches!(
        artifact.physical_evidence_scope(),
        native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
    ));
    let physical = artifact
        .physical_evidence()
        .expect("fixed-result import retains complete evidence");
    let [child] = physical.children() else {
        panic!("one fixed-result source call must produce one D41 child")
    };
    let [foreign_call] = artifact.object().foreign_calls() else {
        panic!("one fixed-result object call expected")
    };
    assert!(foreign_call.scalar_arguments.is_empty());
    let scalar_result = foreign_call
        .scalar_result
        .as_ref()
        .expect("object call retains its fixed scalar result");
    let i64_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
            .unwrap();
    let i64_shape = calling_conventions::ValueShape::integer(8, 8);
    assert_eq!(
        scalar_result.home.scalar_type,
        semantic_vocabulary::ScalarType::Integer(i64_type)
    );
    assert_eq!(scalar_result.home.shape, i64_shape);
    assert_eq!(
        foreign_call.boundary_entry_plan.call.result.as_ref(),
        Some(&scalar_result.source),
    );

    let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent() else {
        panic!("fixed-result import must retain its D41 parent")
    };
    assert_eq!(
        parent.requirement_identity(),
        admission.execution.requirement,
    );
    assert_eq!(
        parent.occurrence().operation_ordinal(),
        foreign_call.operation_ordinal,
    );
    let native::BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    } = parent.role()
    else {
        panic!("fixed-result import must retain admitted-provider custody")
    };
    assert_eq!(parent.execution(), (*execution).into());
    assert_eq!(realization.locator, foreign_call.locator);
    assert_eq!(
        realization.boundary_entry_plan,
        foreign_call.boundary_entry_plan,
    );
    assert_eq!(realization.same_stack_contribution, admission.same_stack);

    let module = terminal_codec::decode_module(artifact.psi_artifact().semantic_bytes())
        .expect("decode fixed-result Terminal semantics");
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == parent.occurrence().machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == parent.occurrence().operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        panic!("D41 parent must rejoin one Terminal result producer")
    };
    let terminal_psi::OperationResult::Scalar(terminal_result) = operation.result else {
        panic!("Terminal boundary call must retain its scalar result")
    };
    assert_eq!(terminal_result.id, scalar_result.home.source_value);
    assert_eq!(
        terminal_result.scalar_type,
        semantic_vocabulary::ScalarType::Integer(i64_type),
    );

    let matching_attributions = artifact
        .object()
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == foreign_call.machine
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Operation(operation.id)
                && attribution.attribution.operation_ordinal == foreign_call.operation_ordinal
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        panic!("one full semantic interval must own the fixed-result import")
    };
    assert_eq!(
        child.machine_span().offset(),
        attribution.attribution.code_offset,
    );
    assert_eq!(child.object_span().offset(), attribution.text_offset);
    assert_eq!(
        child.object_span().byte_count(),
        attribution.attribution.byte_count,
    );
    assert_eq!(child.final_image_span(), child.object_span());
    let object_end = child.object_span().offset() + child.object_span().byte_count();
    assert!(child.object_span().offset() <= scalar_result.code_offset);
    assert!(scalar_result.code_offset + scalar_result.byte_count <= object_end);

    let [normalized] = artifact
        .object()
        .object()
        .layout
        .normalized_imports
        .as_slice()
    else {
        panic!("one normalized fixed-result import expected")
    };
    let matching_relocations = artifact
        .object()
        .relocations()
        .records()
        .filter(|(_, relocation)| relocation.symbol_handle == normalized.symbol)
        .collect::<Vec<_>>();
    let [(_, object_relocation)] = matching_relocations.as_slice() else {
        panic!("one unresolved relocation must target the fixed-result import")
    };
    assert!(child.object_span().offset() <= object_relocation.offset);
    assert!(object_relocation.offset + object_relocation.byte_width <= object_end);
    assert!(child.object_span().byte_count() > object_relocation.byte_width);
    let native::PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(field) =
        child.relocation()
    else {
        panic!("fixed-result child must retain unresolved import-field custody")
    };
    assert_eq!(field.offset(), object_relocation.offset);
    assert_eq!(field.byte_width(), object_relocation.byte_width);
    let [image_foreign_call] = artifact.image().foreign_calls() else {
        panic!("one fixed-result image call expected")
    };
    assert_eq!(image_foreign_call, foreign_call);

    let parts = artifact.into_parts();
    assert_d41_parent_mutation_rejected(&parts, |parent| {
        let native::BoundaryTraitSettlementRole::AdmittedProvider { realization, .. } =
            &mut parent.role
        else {
            panic!("fixture D41 parent is admitted-provider custody")
        };
        realization.boundary_entry_plan.call.result = None;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.relocation = native::PhysicalRelocationDisposition::DirectInstructionBytes;
    });
    assert_physical_child_mutation_rejected(&parts, |child| {
        child.final_image_bytes_digest[0] ^= 1;
    });
}

#[test]
fn widened_scalar_foreign_boundary_retains_complete_physical_custody() {
    for (label, fixture, receipt_identity) in [
        (
            "bool-argument",
            macho_scalar_import_fixture(
                "bool-argument",
                "gate(open: bool)",
                "self.boundary.gate(true);",
            ),
            0x4d41_4348_0c05,
        ),
        (
            "bool-result",
            macho_scalar_import_fixture(
                "bool-result",
                "alive() -> bool",
                "let observed: bool = self.boundary.alive();",
            ),
            0x4d41_4348_0d05,
        ),
        (
            "f32-argument",
            macho_scalar_import_fixture(
                "f32-argument",
                "waitf32(seconds: f32)",
                "self.boundary.waitf32(3.0f32);",
            ),
            0x4d41_4348_0a05,
        ),
        (
            "f64-argument",
            macho_scalar_import_fixture(
                "f64-argument",
                "waitf64(seconds: f64)",
                "self.boundary.waitf64(3.0f64);",
            ),
            0x4d41_4348_0e05,
        ),
        (
            "f64-result",
            macho_scalar_import_fixture(
                "f64-result",
                "pollf64() -> f64",
                "let observed: f64 = self.boundary.pollf64();",
            ),
            0x4d41_4348_0b05,
        ),
    ] {
        let retained = fixture.compile_terminal();
        let admission = admit_import(
            &retained,
            SameStackContributionAdmissionReceiptId::from_normalized_identity(receipt_identity)
                .unwrap(),
        );
        let policy = terminal_authority_policy(&retained);
        let permission_policy = terminal_authority_permission_policy(&retained);
        let artifact = {
            let image_request = native_realization::ExecutableImageEmissionRequest::direct(
                retained
                    .native_realization_proposal()
                    .expect("native proposal")
                    .subsystem(),
            );
            realize_retained_native_artifact(
                retained,
                RetainedNativeRealizationRequest {
                    profile: &proof_admission::AdmissionProfile::default(),
                    optimization_selections:
                        &optimization_core::PostTerminalOptimizationSelections::default(),
                    terminal_authority_policy: policy,
                    accepted_package_terminal_authority_permission_policy:
                        native_realization::current_terminal_authority_permission_policy(),
                    terminal_authority_permission_policy: Some(permission_policy),
                    image_request,
                    imports: &[SourceEvaluatedImportSettlement::new(
                        &admission.execution,
                        &admission.same_stack,
                    )],
                },
            )
            .map(|artifact| match artifact {
                native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
                native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                    panic!("direct image request returned dynamic ELF custody")
                }
            })
            .map_err(|(_, diagnostics)| diagnostics)
        }
        .unwrap_or_else(|diagnostics| panic!("{label}: {diagnostics:#?}"));
        artifact.validate().expect("scalar native custody replays");
        assert!(matches!(
            artifact.physical_evidence_scope(),
            native::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
        ));
        let evidence = artifact
            .physical_evidence()
            .expect("complete scalar physical evidence");
        assert_eq!(
            evidence.projection().boundary_occurrences().len(),
            1,
            "{label}"
        );
        assert_eq!(evidence.children().len(), 1, "{label}");
    }
}
