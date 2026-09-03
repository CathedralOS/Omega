use omega_abstract_operations_to_target_operations::lower_to_target_operations;
#[cfg(feature = "installed-artifact")]
use omega_image_emission::bind_installed_artifact;
use omega_image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, encode_installation_record, validate_installation_record,
};
use omega_machine_emission::emit_machine_code;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::ProfileDecisionId;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

#[cfg(feature = "installed-artifact")]
mod installed_runtime {
    use std::collections::BTreeMap;

    use omega_executable_installation::{
        AdmissionReceiptId, Artifact, ArtifactAdmissionEvidence, ArtifactAuthorityCommitments,
        ArtifactEntry, ArtifactId, ArtifactRelocationKind, CodePlacementAuthority, CodePlacementId,
        DecodedArtifactRelocation, EntrySetId, FinalValidationCertificate, FinalValidationId,
        InstallAuthority, InstallationAudience, InstallationDiagnostic, InstallationReceipt,
        InstallationScopeId, InstalledCode, InstalledCodeId, MachineContractSetId,
        MachineFootprintId, MaterializationReceipt, PlacementPlanId, RelocationSetId,
        WxEnforcement, admit_executable, install_validated, materialize_admitted_artifact,
        materialize_and_freeze, validate_final_placement,
    };
    use omega_image_emission::{
        ExecutableImage, ObjectArtifact, project_installed_artifact_memory_images,
    };
    use omega_object_file::{RelocationKind, SectionKind};
    use psi_core::MachineId;
    use psi_extents::{
        AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRightId,
        ExtentRights, ExtentRootGrant, MappingEraId,
    };
    use psi_layout_plans::{
        ArtifactInstallationScopeId, DataSymbolId, EntryStubId, PlacementConstraints,
        PlacementPhase, PlacementSite, RelocationTarget,
    };

    fn install_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn entry(machine: MachineId) -> EntryStubId {
        EntryStubId::from_normalized_identity(machine.get()).expect("machine-backed entry")
    }

    fn data(index: usize) -> DataSymbolId {
        DataSymbolId::from_normalized_identity(
            0x1000_u64 + u64::try_from(index).expect("table index"),
        )
        .expect("table-backed data symbol")
    }

    fn relocation_kind(kind: RelocationKind) -> ArtifactRelocationKind {
        match kind {
            RelocationKind::Absolute64 => ArtifactRelocationKind::Absolute64,
            RelocationKind::X86_64Relative32 => ArtifactRelocationKind::X86Relative32,
            RelocationKind::Aarch64Page21 => ArtifactRelocationKind::Aarch64Page21,
            RelocationKind::Aarch64PageOffset12 => ArtifactRelocationKind::Aarch64PageOffset12,
            RelocationKind::Aarch64Branch26 => ArtifactRelocationKind::Aarch64Branch26,
        }
    }

    pub fn install(object: &ObjectArtifact, image: &ExecutableImage) -> InstalledCode {
        let memory = project_installed_artifact_memory_images(object, image)
            .expect("project exact text/data memory image");
        let data_offset = memory.data_offset().expect("dynamic table data offset");
        let entries = object
            .functions()
            .iter()
            .map(|function| {
                ArtifactEntry::from_canonical_decode(
                    entry(function.machine),
                    u64::try_from(function.text_offset).expect("function text offset"),
                )
            })
            .collect::<Vec<_>>();
        let entry_addresses = object
            .functions()
            .iter()
            .map(|function| {
                (
                    entry(function.machine),
                    memory.layout().text_address
                        + u64::try_from(function.text_offset).expect("function text offset"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let data_addresses = object
            .dynamic_conformance_tables()
            .iter()
            .enumerate()
            .map(|(index, table)| {
                (
                    data(index),
                    memory.layout().data_address
                        + u64::try_from(table.data_offset).expect("table data offset"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let relocations = object
            .relocations()
            .records()
            .map(|(_, relocation)| {
                let destination_offset = match relocation.section {
                    SectionKind::Text => relocation.offset,
                    SectionKind::Data => data_offset
                        .checked_add(relocation.offset)
                        .expect("flattened data relocation offset"),
                    other => panic!("unsupported installed relocation section: {other:?}"),
                };
                let target = object
                    .functions()
                    .iter()
                    .find(|function| function.symbol == relocation.symbol_handle)
                    .map(|function| RelocationTarget::Entry(entry(function.machine)))
                    .or_else(|| {
                        object
                            .dynamic_conformance_tables()
                            .iter()
                            .position(|table| table.symbol == relocation.symbol_handle)
                            .map(|index| RelocationTarget::Data(data(index)))
                    })
                    .expect("closed installed relocation target");
                DecodedArtifactRelocation {
                    kind: relocation_kind(relocation.kind),
                    destination_offset: u64::try_from(destination_offset)
                        .expect("installed relocation offset"),
                    target,
                    addend: relocation.addend,
                }
            })
            .collect::<Vec<_>>();

        let scope = ArtifactInstallationScopeId::from_normalized_identity(1).expect("scope");
        let constraints =
            PlacementConstraints::new(None, 16, PlacementPhase::Load, None, Some(scope))
                .expect("placement constraints");
        let contracts = install_id(2, MachineContractSetId::from_normalized_identity);
        let footprint = install_id(3, MachineFootprintId::from_normalized_identity);
        let artifact = Artifact::from_canonical_decode(
            install_id(4, ArtifactId::from_normalized_identity),
            object.target().architecture,
            memory.encoded().to_vec(),
            contracts,
            footprint,
            install_id(5, PlacementPlanId::from_normalized_identity),
            constraints,
            install_id(6, EntrySetId::from_normalized_identity),
            entries,
            install_id(7, RelocationSetId::from_normalized_identity),
            relocations,
            ArtifactAuthorityCommitments::from_canonical_evidence(
                contracts,
                b"dynamic-table-test-contracts-v1",
                footprint,
                b"dynamic-table-test-footprint-v1",
                None,
                Some((scope, b"dynamic-table-test-scope-v1")),
            ),
        )
        .expect("normalized dynamic-table artifact");
        let admitted = admit_executable(
            &artifact,
            ArtifactAdmissionEvidence::from_validator(
                install_id(8, AdmissionReceiptId::from_normalized_identity),
                &artifact,
                true,
            ),
        )
        .expect("admitted dynamic-table artifact");
        let rights = ExtentRights::from_normalized_identities([extent_id(
            9,
            ExtentRightId::from_normalized_identity,
        )]);
        let extent = ExtentRootGrant::from_admitted_provider(
            psi_extents::ExtentProviderIssuance::from_normalized_identities([
                10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
            ])
            .expect("extent issuance"),
            extent_id(23, ExtentLineageId::from_normalized_identity),
            extent_id(24, AddressSpaceId::from_normalized_identity),
            rights.clone(),
            extent_id(25, ExtentProvenanceId::from_normalized_identity),
            extent_id(26, MappingEraId::from_normalized_identity),
        )
        .mint(
            memory.layout().text_address,
            u64::try_from(memory.encoded().len()).expect("installed extent length"),
        )
        .expect("placement extent");
        let placement = CodePlacementAuthority::from_admitted_provider(
            install_id(27, CodePlacementId::from_normalized_identity),
            install_id(1, InstallationScopeId::from_normalized_identity),
            InstallationAudience::DormantLocal,
            &extent,
            rights,
            constraints,
            PlacementSite {
                base_address: memory.layout().text_address,
                phase: PlacementPhase::Load,
                machine_regime: None,
                installation_scope: Some(scope),
            },
        )
        .claim(extent)
        .expect("code placement");
        let materialized =
            materialize_admitted_artifact(&admitted, &placement, |target| match target {
                RelocationTarget::Entry(identity) => entry_addresses.get(&identity).copied(),
                RelocationTarget::Data(identity) => data_addresses.get(&identity).copied(),
            })
            .expect("materialized dynamic-table artifact");
        assert_eq!(materialized.bytes(), memory.materialized());
        let frozen = materialize_and_freeze(
            &admitted,
            placement,
            materialized.clone(),
            MaterializationReceipt::from_materialized(
                &materialized,
                install_id(29, MachineFootprintId::from_normalized_identity),
                true,
            ),
        )
        .expect("frozen dynamic-table artifact");
        let validation = FinalValidationCertificate::from_validator(
            install_id(30, FinalValidationId::from_normalized_identity),
            &frozen,
            true,
        );
        let validated = validate_final_placement(frozen, &validation)
            .expect("validated dynamic-table artifact");
        let authority = InstallAuthority::from_admitted_provider(&validated);
        let receipt = InstallationReceipt::from_provider(
            install_id(31, InstalledCodeId::from_normalized_identity),
            &validated,
            true,
            WxEnforcement::HardwareEnforced,
        );
        install_validated(validated, authority, receipt).expect("installed dynamic-table artifact")
    }
}

fn machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
            machine alternate(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }

            machine alternate(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.measure();
        }
    "#;
    compile_source(source, target)
}

fn changed_conformance_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        Secondary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Secondary;
            let result: i32 = erased.measure();
        }
    "#;
    compile_source(source, target)
}

fn dynamic_unit_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let source = r#"
        trait Touch {
            machine touch(&self);
        }

        data Item { value: i32; }

        Primary: Item satisfies Touch {
            machine touch(&self) {}
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            erased.touch();
        }
    "#;
    compile_source(source, target)
}

fn changed_conformance_unit_machine_plan(
    target: NativeTarget,
) -> omega_machine_code::MachineCodePlan {
    let source = r#"
        trait Touch { machine touch(&self); }
        data Item { value: i32; }
        Primary: Item satisfies Touch { machine touch(&self) {} }
        Secondary: Item satisfies Touch { machine touch(&self) {} }
        data Main { decoy: Item; selected: Item; }
        machine Main::run(&mut self) {
            let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Secondary;
            erased.touch();
        }
    "#;
    compile_source(source, target)
}

fn stored_dynamic_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let source = r#"
        trait Measure { machine measure(&self) -> bool; }
        data Item [copy] { value: bool; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> bool { transition { _ -> self.value } }
        }
        data Holder<'item> { handler: &'item dyn Measure; }
        data Main [copy] { item: Item; }
        machine Main::run<'item>(&self) {
            let erased: &'item dyn Measure = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: bool = holder.handler.measure();
        }
    "#;
    compile_source(source, target)
}

fn compile_source(source: &str, target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    let target_plan = lower_to_target_operations(&abstract_plan, target)
        .expect("lower rebound dynamic call to target operations");
    let assigned = assign_registers(&target_plan).expect("assign rebound descriptor");
    emit_machine_code(&assigned).expect("emit rebound descriptor and indirect call")
}

#[test]
fn stored_dynamic_descriptor_replays_through_object_and_final_image() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let plan = stored_dynamic_machine_plan(target);
        let machine_call = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .and_then(|function| function.stored_dynamic_calls.first())
            .expect("stored dynamic machine call");
        let artifact = build_object_artifact(&plan).expect("replay stored dynamic call");
        let object_call = artifact
            .entry_function()
            .stored_dynamic_calls
            .first()
            .expect("stored dynamic object call");
        assert_eq!(object_call, machine_call);
        let [table] = artifact.dynamic_conformance_tables() else {
            panic!("one stored descriptor table expected")
        };
        assert_eq!(
            table.application,
            machine_call.establishment.stored.application
        );
        assert_eq!(table.slots.len(), 1);
        let relocation_count = artifact
            .relocations()
            .records()
            .filter(|(_, relocation)| {
                relocation.origin
                    == omega_object_file::RelocationOrigin::SemanticOperation {
                        function_symbol_handle: artifact.entry_function().symbol,
                        operation_identity: machine_call.establishment.psi_operation.get(),
                    }
                    && relocation.symbol_handle == table.symbol
                    && relocation.section == omega_object_file::SectionKind::Text
            })
            .count();
        assert_eq!(
            relocation_count,
            if target.architecture == omega_target::Architecture::X86_64 {
                1
            } else {
                2
            }
        );
        let image = emit_executable_image(&artifact, 3)
            .expect("link stored descriptor call and private table");
        assert_eq!(
            image
                .functions()
                .iter()
                .find(|function| function.machine == plan.entry)
                .and_then(|function| function.stored_dynamic_calls.first()),
            Some(object_call)
        );
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("retain stored descriptor installation custody");
        let [installed_call] = installation.stored_dynamic_calls() else {
            panic!("one installed stored descriptor call expected")
        };
        assert_eq!(
            installed_call.establishment_operation,
            machine_call.establishment.psi_operation
        );
        assert_eq!(installed_call.operation, machine_call.psi_operation);
        assert_eq!(
            installed_call.descriptor_ordinal,
            machine_call.establishment.stored.descriptor.ordinal
        );
        assert_eq!(
            installed_call.descriptor_home_byte_offset,
            machine_call.establishment.descriptor_home_byte_offset
        );
        validate_installation_record(&installation, &image)
            .expect("replay stored descriptor installation custody");
        let encoded = encode_installation_record(&installation)
            .expect("encode stored descriptor installation custody");
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone())
        );
        let mut operation_pair = Vec::new();
        operation_pair
            .extend_from_slice(&installed_call.establishment_operation.get().to_le_bytes());
        operation_pair.extend_from_slice(&installed_call.operation.get().to_le_bytes());
        let pair_offset = encoded
            .windows(operation_pair.len())
            .position(|window| window == operation_pair)
            .expect("canonical stored operation pair");
        let mut drifted = encoded;
        drifted[pair_offset + 8] ^= 0x80;
        let drifted = decode_installation_record(&drifted)
            .expect("alternate nonzero operation remains structurally decodable");
        assert!(validate_installation_record(&drifted, &image).is_err());
    }
}

#[test]
fn object_replay_rejects_stored_descriptor_byte_or_slot_substitution() {
    let plan = stored_dynamic_machine_plan(NativeTarget::linux_x64());
    let mut wrong_slot = plan.clone();
    let call = wrong_slot
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_slot.entry)
        .and_then(|function| function.stored_dynamic_calls.first_mut())
        .expect("stored dynamic call");
    call.selected_table_byte_offset ^= 8;
    assert!(build_object_artifact(&wrong_slot).is_err());

    let mut wrong_descriptor = plan.clone();
    let caller = wrong_descriptor
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_descriptor.entry)
        .expect("entry caller");
    let call = caller
        .stored_dynamic_calls
        .first()
        .expect("stored dynamic call");
    caller.bytes[call.establishment.instance.code_offset] ^= 1;
    assert!(build_object_artifact(&wrong_descriptor).is_err());

    let mut wrong_table_address = plan;
    let caller = wrong_table_address
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_table_address.entry)
        .expect("entry caller");
    let call = caller
        .stored_dynamic_calls
        .first()
        .expect("stored dynamic call");
    caller.bytes[call.establishment.table_address.code_offset] ^= 1;
    assert!(build_object_artifact(&wrong_table_address).is_err());
}

#[test]
fn rebound_dynamic_unit_call_replays_without_scalar_result_evidence() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let plan = dynamic_unit_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.dynamic_calls.as_slice() else {
            panic!("one rebound dynamic Unit call expected: {caller:#?}")
        };
        assert!(call.result.is_none());
        assert!(call.call_plan.result.is_none());
        assert!(caller.unit_scalar_homes.is_empty());
        assert_eq!(call.dynamic_dispatch.application.rows.len(), 1);
        assert_ne!(
            call.initial_instance.source.path,
            call.rebound_instance.source.path
        );

        if target == NativeTarget::linux_x64() {
            let mut wrong_access = plan.clone();
            let caller = wrong_access
                .functions
                .iter_mut()
                .find(|function| function.machine == wrong_access.entry)
                .expect("entry caller");
            caller.unit_parameters[0].access = psi_terminal::StructuralAccess::Owned;
            assert!(
                build_object_artifact(&wrong_access).is_err(),
                "parameter access cannot be substituted independently of its home"
            );
        }

        let artifact = build_object_artifact(&plan).expect("replay result-less dynamic call");
        let [table] = artifact.dynamic_conformance_tables() else {
            panic!("one Unit conformance table expected")
        };
        assert_eq!(table.slots.len(), 1);
        let image = emit_executable_image(&artifact, 3)
            .expect("link result-less dynamic call and private table");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("retain result-less dynamic installation custody");
        assert_eq!(installation.dynamic_calls().len(), 1);
        validate_installation_record(&installation, &image)
            .expect("installation replay retains Unit dynamic custody");
        let encoded = encode_installation_record(&installation).expect("encode Unit installation");
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation),
            "canonical installation retains borrowed parameter access"
        );
    }
}

#[test]
fn changed_conformance_unit_replays_only_the_live_table_without_a_result() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = changed_conformance_unit_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.dynamic_calls.as_slice() else {
            panic!("one changed-conformance Unit call expected: {caller:#?}")
        };
        assert!(call.result.is_none());
        assert!(call.call_plan.result.is_none());
        assert_ne!(
            call.dynamic_dispatch.initial_application.commitment,
            call.dynamic_dispatch.application.commitment
        );
        assert!(
            call.dynamic_dispatch
                .initial_application
                .realization_callables
                .is_empty()
        );

        let artifact = build_object_artifact(&plan)
            .expect("materialize the live changed-conformance Unit table");
        let [table] = artifact.dynamic_conformance_tables() else {
            panic!("only the live changed-conformance Unit table should materialize")
        };
        assert_eq!(table.application, call.dynamic_dispatch.application);
        assert_ne!(
            table.application.commitment,
            call.dynamic_dispatch.initial_application.commitment
        );
        let image =
            emit_executable_image(&artifact, 3).expect("link the changed-conformance Unit table");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("retain changed-conformance Unit installation custody");
        assert_eq!(installation.dynamic_calls().len(), 1);
        assert_eq!(
            installation.dynamic_calls()[0].application_commitment,
            call.dynamic_dispatch.application.commitment
        );
        validate_installation_record(&installation, &image)
            .expect("replay changed-conformance Unit installation");
        let encoded = encode_installation_record(&installation)
            .expect("encode changed-conformance Unit installation");
        assert_eq!(decode_installation_record(&encoded), Ok(installation));
    }
}

#[test]
fn rebound_dynamic_call_materializes_complete_private_table_and_executes_image_replay() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let plan = machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.dynamic_calls.as_slice() else {
            panic!("one rebound dynamic call expected: {caller:#?}")
        };
        assert_eq!(call.dynamic_dispatch.application.rows.len(), 2);
        assert_ne!(
            call.initial_instance.source.path,
            call.rebound_instance.source.path
        );
        assert!(call.indirect_call_byte_count > 0);
        assert!(
            caller
                .internal_calls
                .iter()
                .all(|direct| { direct.target != call.dynamic_dispatch.dispatch.realization })
        );

        let artifact = build_object_artifact(&plan).expect("materialize exact private table");
        let [table] = artifact.dynamic_conformance_tables() else {
            panic!("one deduplicated conformance table expected")
        };
        assert_eq!(table.application.rows.len(), 2);
        assert_eq!(table.slots.len(), 2);
        assert_eq!(artifact.data_bytes(), &[0; 16]);
        let data_relocations = artifact
            .relocations()
            .records()
            .filter(|(_, relocation)| relocation.section == omega_object_file::SectionKind::Data)
            .collect::<Vec<_>>();
        assert_eq!(data_relocations.len(), 2);
        assert!(data_relocations.iter().all(|(_, relocation)| {
            relocation.kind == omega_object_file::RelocationKind::Absolute64
                && relocation.byte_width == 8
                && relocation.origin
                    == omega_object_file::RelocationOrigin::Materialization {
                        object_symbol_handle: table.symbol,
                    }
        }));

        let image = emit_executable_image(&artifact, 3)
            .expect("direct image replay must retain relocated table data");
        assert_eq!(image.output().final_data_bytes.len(), 16);
        assert_ne!(image.output().final_data_bytes, vec![0; 16]);
        assert_ne!(&image.output().final_data_bytes[..8], &[0; 8]);
        assert_ne!(&image.output().final_data_bytes[8..], &[0; 8]);
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("dynamic table installation custody");
        assert_eq!(installation.dynamic_conformance_tables().len(), 1);
        assert_eq!(installation.dynamic_calls().len(), 1);
        assert_eq!(installation.image_sections().data_byte_count, 16);
        validate_installation_record(&installation, &image)
            .expect("installation record must replay exact dynamic data");
        let encoded = encode_installation_record(&installation).expect("encode installation");
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone()),
            "canonical installation codec retains dynamic table custody"
        );

        let call = &installation.dynamic_calls()[0];
        let replacement = installation
            .functions()
            .iter()
            .map(|function| function.machine)
            .find(|machine| *machine != call.realization)
            .expect("different in-artifact function");
        let commitment = installation.dynamic_conformance_tables()[0]
            .application_commitment
            .as_bytes();
        let commitment_offset = encoded
            .windows(commitment.len())
            .position(|window| window == commitment)
            .expect("encoded dynamic application commitment");
        let selected_slot =
            usize::try_from(call.selected_table_byte_offset / 8).expect("selected dynamic slot");
        let selected_target_offset = commitment_offset + 68 + selected_slot * 24;
        let mut substituted = encoded;
        substituted[selected_target_offset..selected_target_offset + 8]
            .copy_from_slice(&replacement.get().to_le_bytes());
        assert_eq!(
            decode_installation_record(&substituted),
            Err(omega_image_emission::InstallationError::InvalidDynamicCall(
                call.machine
            )),
            "a different valid function cannot replace the selected table realization"
        );
    }
}

#[test]
fn changed_conformance_rebound_materializes_only_the_latest_private_table() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = changed_conformance_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.dynamic_calls.as_slice() else {
            panic!("one changed-conformance rebound call expected: {caller:#?}")
        };
        assert_ne!(
            call.dynamic_dispatch.initial_application.commitment,
            call.dynamic_dispatch.application.commitment
        );
        assert!(
            call.dynamic_dispatch
                .initial_application
                .realization_callables
                .is_empty()
        );
        assert_eq!(
            call.dynamic_dispatch
                .application
                .realization_callables
                .len(),
            1
        );

        let artifact = build_object_artifact(&plan)
            .expect("materialize the latest changed-conformance private table");
        let [table] = artifact.dynamic_conformance_tables() else {
            panic!("only the live rebound conformance table should materialize")
        };
        assert_eq!(table.application, call.dynamic_dispatch.application);
        assert_ne!(
            table.application.commitment,
            call.dynamic_dispatch.initial_application.commitment
        );

        let image = emit_executable_image(&artifact, 3)
            .expect("link the changed-conformance rebound table");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("retain changed-conformance installation custody");
        assert_eq!(installation.dynamic_conformance_tables().len(), 1);
        assert_eq!(installation.dynamic_calls().len(), 1);
        assert_eq!(
            installation.dynamic_calls()[0].application_commitment,
            call.dynamic_dispatch.application.commitment
        );
        validate_installation_record(&installation, &image)
            .expect("installation replay retains the live changed conformance");
        let encoded = encode_installation_record(&installation)
            .expect("encode changed-conformance installation");
        assert_eq!(decode_installation_record(&encoded), Ok(installation));
    }
}

#[test]
fn object_replay_rejects_dynamic_slot_and_descriptor_byte_substitution() {
    let plan = machine_plan(NativeTarget::linux_x64());
    let mut wrong_slot = plan.clone();
    let call = wrong_slot
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_slot.entry)
        .and_then(|function| function.dynamic_calls.first_mut())
        .expect("dynamic call");
    call.selected_table_byte_offset ^= 8;
    assert!(build_object_artifact(&wrong_slot).is_err());

    let mut wrong_descriptor = plan;
    let caller = wrong_descriptor
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_descriptor.entry)
        .expect("entry caller");
    let call = caller.dynamic_calls.first().expect("dynamic call");
    caller.bytes[call.rebound_instance.code_offset] ^= 1;
    assert!(build_object_artifact(&wrong_descriptor).is_err());
}

#[cfg(feature = "installed-artifact")]
#[test]
fn normalized_runtime_installs_and_binds_relocated_dynamic_table_data() {
    let object = build_object_artifact(&machine_plan(NativeTarget::linux_x64()))
        .expect("dynamic-table object");
    let image = emit_executable_image(&object, 3).expect("dynamic-table image");
    let installation =
        build_installation_record(&image, ProfileDecisionId::new(2).expect("profile decision"))
            .expect("dynamic-table installation record");
    let installed = installed_runtime::install(&object, &image);

    let joined = bind_installed_artifact(object, image, installation, installed)
        .expect("exact relocated dynamic table must bind to installed occurrence");
    assert_eq!(joined.installation().dynamic_conformance_tables().len(), 1);
    assert_eq!(joined.installation().dynamic_calls().len(), 1);
}
