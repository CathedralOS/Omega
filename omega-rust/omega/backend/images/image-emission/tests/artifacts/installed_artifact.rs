//! Installed-artifact custody over the complete placed image: projection and
//! the installed-code join replay the retained executable/data inventories,
//! reject unclassified gaps, and bind only when the installed occurrence's
//! bytes equal the complete encoded and materialized images. A compiler prefix
//! cannot stand in for writer-retained placement.

use crate::{callback_private_plan, dynamic_conformance_table_plan, internal_call_plan};
use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    ExecutableImage, InstallationError, InstalledArtifactMemoryImages, ObjectArtifact,
    bind_installed_artifact, bind_installed_compiler_private_function_entry,
    build_installation_record, build_object_artifact, build_object_artifact_with_private_functions,
    emit_executable_image, project_installed_artifact_memory_images, validate_installation_record,
};
use object_file::{ObjectSymbolHandle, RelocationKind, SectionKind};
use semantic_vocabulary::ProfileDecisionId;
use symbols::SymbolHandle;
use target::NativeTarget;

/// Run the executable-installation ladder over one flattened artifact image:
/// container admission, placement-authority claim, materialization under the
/// supplied symbolic resolver, frozen placement, final validation, and the
/// installation receipt. The result is the `InstalledCode` occurrence that
/// `bind_installed_artifact` may join to the emitted image only when the
/// artifact carries the complete encoded bytes and the resolver materializes
/// the complete final image.
fn install_flattened_image(
    architecture: target::Architecture,
    code: Vec<u8>,
    entries: Vec<executable_installation::ArtifactEntry>,
    relocations: Vec<executable_installation::DecodedArtifactRelocation>,
    placement_base: u64,
    resolve: impl FnMut(layout_plans::RelocationTarget) -> Option<u64>,
) -> executable_installation::InstalledCode {
    let scope = layout_plans::ArtifactInstallationScopeId::from_normalized_identity(0x7101)
        .expect("artifact installation scope");
    let constraints = layout_plans::PlacementConstraints::new(
        None,
        16,
        layout_plans::PlacementPhase::Load,
        None,
        Some(scope),
    )
    .expect("placement constraints");
    let extent_len = u64::try_from(code.len()).expect("extent length");
    let contracts = executable_installation::MachineContractSetId::from_normalized_identity(0x7102)
        .expect("contract set");
    let footprint = executable_installation::MachineFootprintId::from_normalized_identity(0x7103)
        .expect("footprint");
    let artifact = executable_installation::Artifact::from_canonical_decode(
        executable_installation::ArtifactId::from_normalized_identity(0x7104)
            .expect("artifact identity"),
        architecture,
        code,
        contracts,
        footprint,
        executable_installation::PlacementPlanId::from_normalized_identity(0x7105)
            .expect("placement plan"),
        constraints,
        executable_installation::EntrySetId::from_normalized_identity(0x7106).expect("entry set"),
        entries,
        executable_installation::RelocationSetId::from_normalized_identity(0x7107)
            .expect("relocation set"),
        relocations,
        executable_installation::ArtifactAuthorityCommitments::from_canonical_evidence(
            contracts,
            b"test-machine-contracts-v1",
            footprint,
            b"test-machine-footprint-v1",
            None,
            Some((scope, b"test-installation-scope-v1")),
        ),
    )
    .expect("canonical artifact");
    let admitted = executable_installation::admit_executable(
        &artifact,
        executable_installation::ArtifactAdmissionEvidence::from_validator(
            executable_installation::AdmissionReceiptId::from_normalized_identity(0x7108)
                .expect("admission receipt"),
            &artifact,
            true,
        ),
    )
    .expect("admitted artifact");
    let rights = extents::ExtentRights::from_normalized_identities([
        extents::ExtentRightId::from_normalized_identity(0x7109).expect("extent right"),
    ]);
    let extent = extents::ExtentRootGrant::from_admitted_provider(
        extents::ExtentProviderIssuance::from_normalized_identities([
            0x710a, 0x710b, 0x710c, 0x710d, 0x710e, 0x710f, 0x7110, 0x7111, 0x7112, 0x7113, 0x7114,
            0x7115, 0x7116,
        ])
        .expect("extent issuance"),
        extents::ExtentLineageId::from_normalized_identity(0x7117).expect("lineage"),
        extents::AddressSpaceId::from_normalized_identity(0x7118).expect("address space"),
        rights.clone(),
        extents::ExtentProvenanceId::from_normalized_identity(0x7119).expect("provenance"),
        extents::MappingEraId::from_normalized_identity(0x711a).expect("era"),
    )
    .mint(placement_base, extent_len)
    .expect("placement extent");
    let placement = executable_installation::CodePlacementAuthority::from_admitted_provider(
        executable_installation::CodePlacementId::from_normalized_identity(0x711b)
            .expect("placement"),
        // The authority's scope identity must equal the artifact installation
        // scope the placement site carries.
        executable_installation::InstallationScopeId::from_normalized_identity(0x7101)
            .expect("installation scope"),
        executable_installation::InstallationAudience::DormantLocal,
        &extent,
        rights,
        constraints,
        layout_plans::PlacementSite {
            base_address: placement_base,
            phase: layout_plans::PlacementPhase::Load,
            machine_regime: None,
            installation_scope: Some(scope),
        },
    )
    .claim(extent)
    .expect("placement claim");
    let materialized =
        executable_installation::materialize_admitted_artifact(&admitted, &placement, resolve)
            .expect("materialized artifact");
    let frozen = executable_installation::materialize_and_freeze(
        &admitted,
        placement,
        materialized.clone(),
        executable_installation::MaterializationReceipt::from_materialized(
            &materialized,
            executable_installation::MachineFootprintId::from_normalized_identity(0x711d)
                .expect("materialized footprint"),
            true,
        ),
    )
    .expect("frozen artifact");
    let validation = executable_installation::FinalValidationCertificate::from_validator(
        executable_installation::FinalValidationId::from_normalized_identity(0x711e)
            .expect("validation"),
        &frozen,
        true,
    );
    let validated = executable_installation::validate_final_placement(frozen, &validation)
        .expect("validated artifact");
    let authority = executable_installation::InstallAuthority::from_admitted_provider(&validated);
    let receipt = executable_installation::InstallationReceipt::from_provider(
        executable_installation::InstalledCodeId::from_normalized_identity(0x711f)
            .expect("installed code"),
        &validated,
        true,
        executable_installation::WxEnforcement::HardwareEnforced,
    );
    executable_installation::install_validated(validated, authority, receipt)
        .expect("installed code")
}

/// The projected memory images span the complete final image: compiler text,
/// inter-section padding, and initialized data — never a prefix.
fn assert_complete_projection(memory: &InstalledArtifactMemoryImages, image: &ExecutableImage) {
    let output = image.output();
    match memory.data_offset() {
        Some(data_offset) => {
            assert!(
                data_offset >= output.final_text_bytes.len(),
                "data placement follows the complete final text"
            );
            assert_eq!(
                memory.materialized().len(),
                data_offset + output.final_data_bytes.len()
            );
            assert_eq!(memory.encoded().len(), memory.materialized().len());
            assert_eq!(
                &memory.materialized()[..output.final_text_bytes.len()],
                output.final_text_bytes.as_slice(),
                "materialized text is the complete final text"
            );
            assert_eq!(
                &memory.materialized()[data_offset..],
                output.final_data_bytes.as_slice(),
                "materialized data is the complete final data"
            );
        }
        None => {
            assert!(output.final_data_bytes.is_empty());
            assert_eq!(
                memory.materialized(),
                output.final_text_bytes.as_slice(),
                "materialized image is the complete final text"
            );
        }
    }
}

/// The artifact entries, decoded relocations, and symbolic resolver an
/// installed occurrence needs for its materialized bytes to equal the
/// complete final image. The placement base is the image's own text address,
/// and the resolver returns final-image virtual addresses, so both absolute
/// and relative relocation transforms reproduce the values the image writer
/// baked into the final bytes.
///
/// Every text symbol the ladder mints a `RelocationTarget::Entry` for is also
/// admitted as an `ArtifactEntry` row — program functions, compiler-private
/// functions, and forwarded-descriptor adapters alike — matching the admitted
/// entry set the production installation carries. The returned `entry_stubs`
/// name each symbol's admitted stub so tests can select one.
fn install_parts(
    object: &ObjectArtifact,
    image: &ExecutableImage,
    memory: &InstalledArtifactMemoryImages,
) -> (
    u64,
    Vec<executable_installation::ArtifactEntry>,
    Vec<executable_installation::DecodedArtifactRelocation>,
    Vec<(layout_plans::RelocationTarget, u64)>,
    Vec<(ObjectSymbolHandle, layout_plans::EntryStubId)>,
) {
    let layout = image.output().final_image_layout;
    let base = layout.text_address;
    let mut entries = Vec::new();
    let mut entry_stubs = Vec::new();
    let mut symbol_targets: Vec<(ObjectSymbolHandle, (layout_plans::RelocationTarget, u64))> =
        Vec::new();
    let mut next_identity = 0x7400_u64;
    let mut next_text = |symbol: ObjectSymbolHandle, text_offset: usize| {
        let stub = layout_plans::EntryStubId::from_normalized_identity(next_identity)
            .expect("entry stub identity");
        next_identity += 1;
        (
            symbol,
            (
                layout_plans::RelocationTarget::Entry(stub),
                base + u64::try_from(text_offset).expect("text offset"),
            ),
            stub,
        )
    };
    let mut admit_text_entry = |symbol: ObjectSymbolHandle, text_offset: usize| {
        let (symbol, target, stub) = next_text(symbol, text_offset);
        symbol_targets.push((symbol, target));
        entry_stubs.push((symbol, stub));
        entries.push(
            executable_installation::ArtifactEntry::from_canonical_decode(
                stub,
                u64::try_from(text_offset).expect("text offset"),
            ),
        );
    };
    for function in object.functions() {
        admit_text_entry(function.symbol, function.text_offset);
    }
    for private in object.private_functions() {
        admit_text_entry(private.function.symbol, private.function.text_offset);
    }
    for adapter in object.forwarded_dynamic_descriptor_adapters() {
        admit_text_entry(adapter.symbol, adapter.text_offset);
    }
    drop(admit_text_entry);
    for table in object.dynamic_conformance_tables() {
        let data = layout_plans::DataSymbolId::from_normalized_identity(next_identity)
            .expect("data symbol identity");
        next_identity += 1;
        symbol_targets.push((
            table.symbol,
            (
                layout_plans::RelocationTarget::Data(data),
                layout.data_address + u64::try_from(table.data_offset).expect("data offset"),
            ),
        ));
    }
    for table in object.forwarded_dynamic_descriptor_tables() {
        let data = layout_plans::DataSymbolId::from_normalized_identity(next_identity)
            .expect("data symbol identity");
        next_identity += 1;
        symbol_targets.push((
            table.symbol,
            (
                layout_plans::RelocationTarget::Data(data),
                layout.data_address + u64::try_from(table.data_offset).expect("data offset"),
            ),
        ));
    }
    let data_offset = u64::try_from(memory.data_offset().unwrap_or(0)).expect("data offset");
    let relocations = object
        .relocations()
        .records()
        .map(|(_, relocation)| {
            let destination_offset = match relocation.section {
                SectionKind::Text => u64::try_from(relocation.offset).expect("text offset"),
                SectionKind::Data => {
                    data_offset + u64::try_from(relocation.offset).expect("data offset")
                }
                other => panic!("unsupported relocation section {other:?}"),
            };
            let target = symbol_targets
                .iter()
                .find(|(symbol, _)| *symbol == relocation.symbol_handle)
                .map(|(_, target)| *target)
                .unwrap_or_else(|| {
                    panic!(
                        "relocation {:?} targets a symbol outside the retained image sections",
                        relocation.symbol_handle
                    )
                });
            let kind = match relocation.kind {
                RelocationKind::Absolute64 => {
                    executable_installation::ArtifactRelocationKind::Absolute64
                }
                RelocationKind::X86_64Relative32 => {
                    executable_installation::ArtifactRelocationKind::X86Relative32
                }
                RelocationKind::Aarch64Page21 => {
                    executable_installation::ArtifactRelocationKind::Aarch64Page21
                }
                RelocationKind::Aarch64PageOffset12 => {
                    executable_installation::ArtifactRelocationKind::Aarch64PageOffset12
                }
                RelocationKind::Aarch64Branch26 => {
                    executable_installation::ArtifactRelocationKind::Aarch64Branch26
                }
            };
            executable_installation::DecodedArtifactRelocation {
                kind,
                destination_offset,
                target: target.0,
                addend: relocation.addend,
            }
        })
        .collect();
    let resolution: Vec<(layout_plans::RelocationTarget, u64)> = symbol_targets
        .into_iter()
        .map(|(_, target)| target)
        .collect();
    (base, entries, relocations, resolution, entry_stubs)
}

/// The admitted entry stub minted for one object text symbol, when the ladder
/// admitted one. Every `RelocationTarget::Entry` in the resolver map has a
/// matching `ArtifactEntry` row, so this is exactly the set
/// `InstalledCode::selected_entry_target` can select.
fn entry_stub(
    entry_stubs: &[(ObjectSymbolHandle, layout_plans::EntryStubId)],
    symbol: ObjectSymbolHandle,
) -> Option<layout_plans::EntryStubId> {
    entry_stubs
        .iter()
        .find(|(candidate, _)| *candidate == symbol)
        .map(|(_, stub)| *stub)
}

fn resolver(
    resolution: Vec<(layout_plans::RelocationTarget, u64)>,
) -> impl FnMut(layout_plans::RelocationTarget) -> Option<u64> {
    move |target| {
        resolution
            .iter()
            .find(|(candidate, _)| *candidate == target)
            .map(|(_, address)| *address)
    }
}

/// ELF lane over an image carrying initialized data: the placed data inventory
/// is non-vacuous, so dropping or drifting either inventory row must reject
/// projection, record replay, and the installed-code join. A truncated or
/// byte-drifted artifact cannot join even under an unchanged image.
#[test]
fn installed_artifact_join_requires_complete_placed_custody() {
    let object =
        build_object_artifact(&dynamic_conformance_table_plan()).expect("dynamic-table object");
    let image = emit_executable_image(&object, 3).expect("ELF image");
    assert!(
        !image.output().final_data_bytes.is_empty(),
        "the fixture must exercise the data-inventory leg"
    );
    assert!(
        !image.output().data_regions.regions.is_empty(),
        "compiler data is a retained placed region"
    );
    let memory = project_installed_artifact_memory_images(&object, &image)
        .expect("complete custody projects the installed memory images");
    assert_complete_projection(&memory, &image);

    let record = build_installation_record(&image, ProfileDecisionId::new(29).expect("profile"))
        .expect("installation record");
    let (base, entries, relocations, stub_addresses, _) = install_parts(&object, &image, &memory);
    let install = |code: Vec<u8>| {
        install_flattened_image(
            object.target().architecture,
            code,
            entries.clone(),
            relocations.clone(),
            base,
            resolver(stub_addresses.clone()),
        )
    };
    bind_installed_artifact(
        object.clone(),
        image.clone(),
        record.clone(),
        &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        install(memory.encoded().to_vec()),
    )
    .expect("the complete image binds installed-code custody");

    // A prefix-only artifact cannot join: its installed bytes never equal the
    // complete encoded and materialized images, whatever its resolver claims.
    let truncated_len = memory.encoded().len() - 1;
    assert!(
        bind_installed_artifact(
            object.clone(),
            image.clone(),
            record.clone(),
            &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
            install(memory.encoded()[..truncated_len].to_vec()),
        )
        .is_err(),
        "a truncated artifact must not bind the complete image"
    );
    let mut drifted_bytes = memory.encoded().to_vec();
    let last = drifted_bytes.len() - 1;
    drifted_bytes[last] ^= 0x01;
    assert!(
        bind_installed_artifact(
            object.clone(),
            image.clone(),
            record.clone(),
            &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
            install(drifted_bytes),
        )
        .is_err(),
        "a byte-drifted artifact must not bind the complete image"
    );

    // A resolver that substitutes a different in-extent target materializes
    // different bytes and cannot join the bound image.
    if let Some(first_relocation) = relocations.first() {
        let mut misresolved_targets = stub_addresses.clone();
        let (_, address) = misresolved_targets
            .iter_mut()
            .find(|(target, _)| *target == first_relocation.target)
            .expect("relocation target stub");
        *address += 4;
        assert!(
            bind_installed_artifact(
                object.clone(),
                image.clone(),
                record.clone(),
                &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
                install_flattened_image(
                    object.target().architecture,
                    memory.encoded().to_vec(),
                    entries.clone(),
                    relocations.clone(),
                    base,
                    resolver(misresolved_targets),
                ),
            )
            .is_err(),
            "a substituted relocation resolution must not materialize the bound image"
        );
    }

    // Every retained placement row is required independently: removing or
    // substituting either executable or data custody must reject projection,
    // record validation, and the installed-artifact join.
    let mutations: Vec<(&str, Box<dyn Fn(&mut ExecutableImage)>)> = vec![
        (
            "missing executable-region placement",
            Box::new(|image| {
                image.output_mut_for_test().executable_regions.regions.pop();
            }),
        ),
        (
            "missing data-region placement",
            Box::new(|image| {
                image.output_mut_for_test().data_regions.regions.pop();
            }),
        ),
        (
            "substituted executable-region placement",
            Box::new(|image| {
                image.output_mut_for_test().executable_regions.regions[0].address += 0x1000;
            }),
        ),
        (
            "substituted data-region placement",
            Box::new(|image| {
                image.output_mut_for_test().data_regions.regions[0].address += 8;
            }),
        ),
        (
            "inflated data extent",
            Box::new(|image| {
                image.output_mut_for_test().data_regions.data_byte_count += 8;
            }),
        ),
        (
            "final-byte drift",
            Box::new(|image| {
                image.output_mut_for_test().final_text_bytes[0] ^= 0xff;
            }),
        ),
    ];
    for (label, mutate) in mutations {
        let mut drifted = image.clone();
        mutate(&mut drifted);
        assert!(
            project_installed_artifact_memory_images(&object, &drifted).is_err(),
            "{label}: complete-custody projection must reject it",
        );
        assert_eq!(
            validate_installation_record(&record, &drifted),
            Err(InstallationError::InvalidImagePlacementCustody),
            "{label}: complete-custody replay must reject it",
        );
        assert!(
            bind_installed_artifact(
                object.clone(),
                drifted,
                record.clone(),
                &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
                install(memory.encoded().to_vec()),
            )
            .is_err(),
            "{label}: the installed-artifact join must reject it",
        );
    }

    // By-value opaque custody is the record's claim, not a fingerprint: a
    // record claiming an edge the installed artifact's coverage does not
    // retain cannot bind, and the artifact's own custody cannot substitute
    // for an absent claim in the record.
    let mut claimed = record.clone();
    *claimed.boundary_opaque_applications_mut_for_test() =
        boundary_applications::BoundaryOpaqueRepresentationApplications::new(vec![
            boundary_applications::BoundaryOpaqueRepresentationApplication {
                requirement_identity: "core::system::Table".into(),
                shape_root: 3,
                application_report_fingerprint: 0x5AA5,
                selected_application_commitment: [0x9C; 32],
            },
        ])
        .expect("custody");
    let error = bind_installed_artifact(
        object.clone(),
        image.clone(),
        claimed,
        &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        install(memory.encoded().to_vec()),
    )
    .expect_err("a record claiming custody the artifact does not retain must not bind");
    assert!(
        error.diagnostic().contains("by-value opaque custody"),
        "unexpected diagnostic: {}",
        error.diagnostic()
    );
    let empty_claim = bind_installed_artifact(
        object.clone(),
        image.clone(),
        record.clone(),
        claimed_custody(),
        install(memory.encoded().to_vec()),
    )
    .expect_err("an artifact retaining custody the record does not claim must not bind");
    assert!(
        empty_claim.diagnostic().contains("by-value opaque custody"),
        "unexpected diagnostic: {}",
        empty_claim.diagnostic()
    );
}

fn claimed_custody() -> &'static boundary_applications::BoundaryOpaqueRepresentationApplications {
    static CUSTODY: std::sync::LazyLock<
        boundary_applications::BoundaryOpaqueRepresentationApplications,
    > = std::sync::LazyLock::new(|| {
        boundary_applications::BoundaryOpaqueRepresentationApplications::new(vec![
            boundary_applications::BoundaryOpaqueRepresentationApplication {
                requirement_identity: "core::system::Table".into(),
                shape_root: 3,
                application_report_fingerprint: 0x5AA5,
                selected_application_commitment: [0x9C; 32],
            },
        ])
        .expect("custody")
    });
    &CUSTODY
}

/// Mach-O lane: an internal call is the typed relocation the writer applies to
/// final text, so the materialized image proves the relocated `bl` reached the
/// placed callee — and custody drift still rejects independently.
#[test]
fn installed_artifact_join_replays_typed_relocation_over_complete_macho_image() {
    let object = build_object_artifact(&internal_call_plan(NativeTarget::macos_arm64()))
        .expect("Mach-O object");
    let image = emit_executable_image(&object, 3).expect("Mach-O image");
    assert_eq!(image.output().final_image_imports, 0);
    let memory = project_installed_artifact_memory_images(&object, &image)
        .expect("complete custody projects the installed memory images");
    assert_complete_projection(&memory, &image);

    let record = build_installation_record(&image, ProfileDecisionId::new(31).expect("profile"))
        .expect("installation record");
    let (base, entries, relocations, stub_addresses, _) = install_parts(&object, &image, &memory);
    assert_eq!(relocations.len(), 1, "one typed internal-call relocation");
    bind_installed_artifact(
        object.clone(),
        image.clone(),
        record.clone(),
        &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        install_flattened_image(
            object.target().architecture,
            memory.encoded().to_vec(),
            entries.clone(),
            relocations.clone(),
            base,
            resolver(stub_addresses.clone()),
        ),
    )
    .expect("the relocated Mach-O image binds installed-code custody");

    // A resolver that substitutes a different in-extent callee address
    // materializes a different `bl` and cannot join the bound image.
    let callee_relocation = relocations[0];
    let mut misresolved = stub_addresses.clone();
    let (_, address) = misresolved
        .iter_mut()
        .find(|(target, _)| *target == callee_relocation.target)
        .expect("callee target");
    *address += 4;
    assert!(
        bind_installed_artifact(
            object.clone(),
            image.clone(),
            record.clone(),
            &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
            install_flattened_image(
                object.target().architecture,
                memory.encoded().to_vec(),
                entries.clone(),
                vec![callee_relocation],
                base,
                resolver(misresolved),
            ),
        )
        .is_err(),
        "a substituted callee resolution must not materialize the bound image"
    );

    let mutations: Vec<(&str, Box<dyn Fn(&mut ExecutableImage)>)> = vec![
        (
            "missing executable-region placement",
            Box::new(|image| {
                image.output_mut_for_test().executable_regions.regions.pop();
            }),
        ),
        (
            "substituted executable-region placement",
            Box::new(|image| {
                image.output_mut_for_test().executable_regions.regions[0].address += 0x1000;
            }),
        ),
        (
            "final-byte drift",
            Box::new(|image| {
                image.output_mut_for_test().final_text_bytes[0] ^= 0xff;
            }),
        ),
    ];
    for (label, mutate) in mutations {
        let mut drifted = image.clone();
        mutate(&mut drifted);
        assert!(
            project_installed_artifact_memory_images(&object, &drifted).is_err(),
            "{label}: complete-custody projection must reject it",
        );
        assert_eq!(
            validate_installation_record(&record, &drifted),
            Err(InstallationError::InvalidImagePlacementCustody),
            "{label}: complete-custody replay must reject it",
        );
        assert!(
            bind_installed_artifact(
                object.clone(),
                drifted,
                record.clone(),
                &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
                install_flattened_image(
                    object.target().architecture,
                    memory.encoded().to_vec(),
                    entries.clone(),
                    relocations.clone(),
                    base,
                    resolver(stub_addresses.clone()),
                ),
            )
            .is_err(),
            "{label}: the installed-artifact join must reject it",
        );
    }
}

/// The compiler-private entry-attribution gate replays the admitted artifact
/// entry set: the callback thunk's own `ArtifactEntry` binds the exact
/// installation row and installed occurrence, while a foreign function
/// identity, an unadmitted stub, or a semantic function's admitted entry each
/// reject. The bound attribution names only its own installed-code occurrence —
/// identical bytes installed under a different placement are not it.
#[test]
fn installed_private_function_entry_binds_exact_row_and_occurrence() {
    let plan = callback_private_plan();
    let object =
        build_object_artifact_with_private_functions(&plan).expect("callback private object");
    let [private] = object.private_functions() else {
        panic!("callback fixture retains one compiler-private function");
    };
    let private_identity = private.identity;
    let private_symbol = private.function.symbol;
    let image = emit_executable_image(&object, 3).expect("callback private image");
    let memory = project_installed_artifact_memory_images(&object, &image)
        .expect("complete custody projects the installed memory images");
    assert_complete_projection(&memory, &image);

    let record = build_installation_record(&image, ProfileDecisionId::new(59).expect("profile"))
        .expect("callback private installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let [row] = record.private_functions() else {
        panic!("the record retains the exact private-function row");
    };
    assert_eq!(row.identity, private_identity);
    let (base, entries, relocations, stub_addresses, entry_stubs) =
        install_parts(&object, &image, &memory);
    let private_stub = entry_stub(&entry_stubs, private_symbol)
        .expect("the private function's text symbol is an admitted entry");
    let install = |placement_base: u64| {
        install_flattened_image(
            object.target().architecture,
            memory.encoded().to_vec(),
            entries.clone(),
            relocations.clone(),
            placement_base,
            resolver(stub_addresses.clone()),
        )
    };
    let bound = bind_installed_artifact(
        object.clone(),
        image.clone(),
        record.clone(),
        &boundary_applications::BoundaryOpaqueRepresentationApplications::EMPTY,
        install(base),
    )
    .expect("the complete image binds installed-code custody");

    let attribution =
        bind_installed_compiler_private_function_entry(&bound, private_identity, private_stub)
            .expect("the admitted entry binds the exact private-function row");
    assert_eq!(attribution.private_function(), row);
    assert_eq!(attribution.entry(), private_stub);
    assert_eq!(attribution.artifact(), bound.artifact());
    assert_eq!(attribution.installed_code(), bound.installed_code());
    assert_eq!(
        attribution.occurrence_digest(),
        bound.installed().occurrence_digest(),
    );
    assert!(attribution.binds_installed_code(bound.installed()));

    // Identical bytes under a different placement are a different occurrence:
    // the attribution stays bound to its own installed-code evidence.
    let other = install(base + 0x1_0000);
    assert_ne!(
        attribution.occurrence_digest(),
        other.occurrence_digest(),
        "a foreign placement retains distinct occurrence evidence",
    );
    assert!(
        !attribution.binds_installed_code(&other),
        "a same-bytes occurrence under a foreign placement must not bind",
    );

    // A foreign compiler-private identity is not retained by this record.
    let foreign = MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_parts(15, 2),
            state: SymbolHandle::from_parts(13, 3),
            segment_index: 0,
        },
        0,
    )
    .expect("foreign callback thunk identity");
    let error = bind_installed_compiler_private_function_entry(&bound, foreign, private_stub)
        .expect_err("a foreign private identity cannot bind the retained row");
    assert_eq!(error.private_function(), foreign);
    assert_eq!(error.entry(), private_stub);
    assert!(
        error.diagnostic().contains("does not retain"),
        "unexpected diagnostic: {}",
        error.diagnostic(),
    );

    // An entry the artifact never admitted cannot bind the row.
    let unadmitted = layout_plans::EntryStubId::from_normalized_identity(0x7fff)
        .expect("unadmitted stub identity");
    let error =
        bind_installed_compiler_private_function_entry(&bound, private_identity, unadmitted)
            .expect_err("an entry outside the admitted set rejects");
    assert_eq!(error.entry(), unadmitted);
    assert!(
        error.diagnostic().contains("not admitted"),
        "unexpected diagnostic: {}",
        error.diagnostic(),
    );

    // Even an admitted entry cannot substitute: the entry function's stub
    // begins at its own text offset, not at the private row's interval.
    let entry_function_stub = entry_stub(&entry_stubs, object.entry_function().symbol)
        .expect("the entry function's admitted stub");
    let error = bind_installed_compiler_private_function_entry(
        &bound,
        private_identity,
        entry_function_stub,
    )
    .expect_err("a different admitted entry cannot begin the private interval");
    assert_eq!(error.entry(), entry_function_stub);
    assert!(
        error.diagnostic().contains("text offset"),
        "unexpected diagnostic: {}",
        error.diagnostic(),
    );
}
