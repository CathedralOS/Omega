use omega_object_file::{
    RelocationKind, RelocationOrigin, SectionKind, SymbolKind, object_symbol_name,
};
use omega_target::NativeTarget;
use omega_terminal_image_emission::{
    TerminalInstallationError, TerminalObjectError, build_terminal_installation_record,
    build_terminal_object_artifact, can_emit_terminal_executable_image,
    decode_terminal_installation_record, derive_terminal_unit_stack_demand,
    emit_terminal_executable_image, emit_terminal_object_container,
    encode_terminal_installation_record, terminal_installation_fingerprint,
    validate_terminal_installation_record,
};
use omega_terminal_machine_code::{
    TerminalAarch64ReturnLinkEvidence, TerminalBoundarySettlementRecord,
    TerminalInternalCallRelocation, TerminalMachineCodeFunction, TerminalMachineCodePlan,
    TerminalNativeFuelAttribution, TerminalNativeFuelSite, TerminalPortEffectRecord,
    TerminalScalarStackEvidence, TerminalScalarStackMutation, TerminalScalarStackMutationKind,
    TerminalStackAdjustmentPair, TerminalUnitCallStackEvidence, TerminalUnitStackEvidence,
};
use omega_terminal_target_operations::{
    TerminalMetadataOnlyPortRealization, TerminalProviderExecutionBinding,
    TerminalProviderPlanIdentity, TerminalPsiProvenance,
};
use psi_core::{BoundaryMachineId, EdgeId, MachineId, OperationId, ProfileDecisionId, ServiceId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[test]
fn object_artifact_owns_canonical_function_spans_and_psi_provenance() {
    let plan = two_function_plan();
    let artifact = build_terminal_object_artifact(&plan).expect("terminal object artifact");

    assert_eq!(artifact.terminal_psi(), plan.terminal_psi);
    assert_eq!(artifact.target(), plan.target);
    assert_eq!(artifact.entry(), machine_id(2));
    assert_eq!(artifact.relocations().record_count(), 0);
    assert_eq!(artifact.functions().len(), 2);
    assert_eq!(artifact.functions()[0].text_offset, 0);
    assert_eq!(artifact.functions()[0].byte_count, 6);
    assert_eq!(artifact.functions()[0].bytes(&artifact), &integer_return(3));
    assert_eq!(artifact.functions()[1].text_offset, 6);
    assert_eq!(artifact.functions()[1].bytes(&artifact), &integer_return(7));
    assert_eq!(
        artifact.functions()[1].provenance,
        TerminalPsiProvenance {
            operations: vec![operation_id(2)],
            edges: vec![edge_id(2)],
        }
    );

    let symbols = artifact
        .object()
        .layout
        .symbols
        .iter()
        .map(|(_, symbol)| symbol)
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0].name, "omega_terminal_machine_1");
    assert_eq!(symbols[0].kind, SymbolKind::Function);
    assert_eq!(symbols[1].name, "main");
    assert_eq!(
        object_symbol_name(artifact.object(), artifact.object().layout.entry_symbol),
        "main"
    );
    let sections = artifact
        .object()
        .layout
        .sections
        .iter()
        .map(|(_, section)| section)
        .collect::<Vec<_>>();
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].kind, SectionKind::Text);
    assert_eq!(sections[0].size, 12);

    let container = emit_terminal_object_container(&artifact);
    assert_eq!(container.terminal_psi, plan.terminal_psi);
    assert_eq!(&container.output.bytes[..8], b"OMGOBJ\0\0");
    assert_eq!(&container.output.bytes[8..12], &5_u32.to_le_bytes());
    assert_eq!(container.output.text_bytes, 12);
    assert_eq!(container.output.data_bytes, 0);
    assert_eq!(container.output.bss_bytes, 0);
    assert_eq!(container.output.symbols, 2);
    assert_eq!(container.output.relocations, 0);
}

#[test]
fn object_boundary_rejects_noncanonical_or_incomplete_machine_code_plans() {
    let mut reordered = two_function_plan();
    reordered.functions.swap(0, 1);
    assert_eq!(
        build_terminal_object_artifact(&reordered),
        Err(TerminalObjectError::NonCanonicalFunctionOrder {
            previous: machine_id(2),
            current: machine_id(1),
        })
    );

    let mut missing_entry = two_function_plan();
    missing_entry.entry = machine_id(3);
    assert_eq!(
        build_terminal_object_artifact(&missing_entry),
        Err(TerminalObjectError::EntryFunctionMissing(machine_id(3)))
    );

    let mut empty_function = two_function_plan();
    empty_function.functions[0].bytes.clear();
    assert_eq!(
        build_terminal_object_artifact(&empty_function),
        Err(TerminalObjectError::EmptyFunction(machine_id(1)))
    );
}

#[test]
fn x86_internal_call_is_a_typed_relocation_and_the_only_final_text_mutation() {
    let mut plan = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut plan);
    let full_width_operation = operation_id(u64::from(u32::MAX) + 1);
    plan.functions[1].provenance.operations[0] = full_width_operation;
    plan.functions[1].internal_calls[0].psi_operation = full_width_operation;
    let artifact = build_terminal_object_artifact(&plan).expect("terminal object artifact");
    assert_eq!(artifact.functions()[1].unit_stack.unwrap().frame_bytes, 0);
    assert_eq!(
        artifact.functions()[1].unit_stack.unwrap().local_peak_bytes,
        16
    );
    assert_eq!(artifact.functions()[1].unit_call_stacks.len(), 1);
    assert_eq!(
        artifact.functions()[1].unit_call_stacks[0].caller_live_bytes,
        16
    );

    assert_eq!(artifact.relocations().record_count(), 1);
    let (_, relocation) = artifact.relocations().records().next().expect("relocation");
    assert_eq!(relocation.kind, RelocationKind::X86_64Relative32);
    assert_eq!(relocation.section, SectionKind::Text);
    assert_eq!(relocation.offset, 11);
    assert_eq!(relocation.byte_width, 4);
    assert_eq!(relocation.symbol_handle, artifact.functions()[0].symbol);
    assert_eq!(
        relocation.origin.semantic_operation_identity(),
        Some(u64::from(u32::MAX) + 1)
    );
    assert_eq!(
        relocation.origin,
        RelocationOrigin::SemanticOperation {
            function_symbol_handle: artifact.functions()[1].symbol,
            operation_identity: u64::from(u32::MAX) + 1,
        }
    );

    let container = emit_terminal_object_container(&artifact);
    assert_eq!(container.output.relocations, 1);
    let image = emit_terminal_executable_image(&artifact, 3).expect("Linux x86-64 image");
    let output = image.output();
    assert_eq!(&output.final_text_bytes[11..15], &[0xf1, 0xff, 0xff, 0xff]);
    assert_eq!(output.final_image_relocations, 1);
    let evidence = output
        .compiler_text_validation
        .expect("relocation evidence");
    assert_ne!(
        evidence.encoded_text_fingerprint,
        evidence.final_compiler_text_fingerprint
    );
    assert_eq!(evidence.text_relocation_count, 1);

    let record = build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
        .expect("installation record");
    validate_terminal_installation_record(&record, &image).expect("image binding");
}

#[test]
fn aarch64_internal_call_patches_only_the_branch_immediate() {
    let plan = internal_call_plan(NativeTarget::linux_arm64());
    let artifact = build_terminal_object_artifact(&plan).expect("terminal object artifact");
    let (_, relocation) = artifact.relocations().records().next().expect("relocation");
    assert_eq!(relocation.kind, RelocationKind::Aarch64Branch26);
    assert_eq!(relocation.offset, 8);

    let image = emit_terminal_executable_image(&artifact, 3).expect("Linux AArch64 image");
    let output = image.output();
    assert_eq!(&output.final_text_bytes[8..12], &[0xfe, 0xff, 0xff, 0x97]);
    assert_eq!(output.final_image_relocations, 1);
    assert_eq!(
        output
            .compiler_text_validation
            .expect("relocation evidence")
            .text_relocation_count,
        1
    );
}

#[test]
fn object_boundary_rejects_unproved_internal_call_relocations() {
    let mut unknown_target = internal_call_plan(NativeTarget::linux_x64());
    unknown_target.functions[1].internal_calls[0].target = machine_id(3);
    assert_eq!(
        build_terminal_object_artifact(&unknown_target),
        Err(TerminalObjectError::UnknownInternalCallTarget {
            caller: machine_id(2),
            target: machine_id(3),
        })
    );

    let mut invalid_site = internal_call_plan(NativeTarget::linux_x64());
    invalid_site.functions[1].bytes[0] = 0x90;
    assert_eq!(
        build_terminal_object_artifact(&invalid_site),
        Err(TerminalObjectError::InvalidInternalCallSite {
            caller: machine_id(2),
            operation: operation_id(2),
            offset: 1,
        })
    );

    let mut duplicate_site = internal_call_plan(NativeTarget::linux_x64());
    let duplicate_call = duplicate_site.functions[1].internal_calls[0];
    duplicate_site.functions[1]
        .internal_calls
        .push(duplicate_call);
    assert_eq!(
        build_terminal_object_artifact(&duplicate_site),
        Err(TerminalObjectError::NonCanonicalInternalCallOrder(
            machine_id(2)
        ))
    );

    let mut missing_provenance = internal_call_plan(NativeTarget::linux_x64());
    missing_provenance.functions[1]
        .provenance
        .operations
        .clear();
    assert_eq!(
        build_terminal_object_artifact(&missing_provenance),
        Err(TerminalObjectError::InternalCallOperationNotInProvenance {
            caller: machine_id(2),
            operation: operation_id(2),
        })
    );

    let mut duplicate_operation = internal_call_plan(NativeTarget::linux_x64());
    duplicate_operation.functions[1].bytes = vec![0xe8, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0, 0xc3];
    duplicate_operation.functions[1]
        .internal_calls
        .push(TerminalInternalCallRelocation {
            psi_operation: operation_id(2),
            target: machine_id(1),
            unit_stack: None,
            offset: 6,
        });
    assert_eq!(
        build_terminal_object_artifact(&duplicate_operation),
        Err(TerminalObjectError::DuplicateInternalCallOperation {
            caller: machine_id(2),
            operation: operation_id(2),
        })
    );
}

#[test]
fn object_boundary_rejects_drifted_unit_stack_evidence() {
    let mut missing_call = internal_call_plan(NativeTarget::linux_x64());
    missing_call.functions[1].unit_stack = Some(TerminalUnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    assert_eq!(
        build_terminal_object_artifact(&missing_call),
        Err(TerminalObjectError::MissingUnitCallStackEvidence {
            caller: machine_id(2),
            operation: operation_id(2),
        })
    );

    let mut removed_allocation = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut removed_allocation);
    removed_allocation.functions[1].bytes.drain(0..4);
    removed_allocation.functions[1].internal_calls[0].offset -= 4;
    assert_eq!(
        build_terminal_object_artifact(&removed_allocation),
        Err(TerminalObjectError::InvalidUnitStackEncoding {
            machine: machine_id(2),
            operation: Some(operation_id(2)),
            offset: 0,
        })
    );

    let mut missing_adjustment = internal_call_plan(NativeTarget::linux_x64());
    missing_adjustment.functions[1].unit_stack = Some(TerminalUnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    missing_adjustment.functions[1].internal_calls[0].unit_stack =
        Some(TerminalUnitCallStackEvidence { outbound: None });
    assert_eq!(
        build_terminal_object_artifact(&missing_adjustment),
        Err(TerminalObjectError::MissingX86UnitCallStackAdjustment {
            caller: machine_id(2),
            operation: operation_id(2),
        })
    );

    let mut unclaimed_adjustment = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut unclaimed_adjustment);
    unclaimed_adjustment.functions[1].bytes.splice(
        13..13,
        [
            0x48, 0x83, 0xec, 0x08, // unclaimed sub rsp, 8
            0x48, 0x83, 0xc4, 0x08, // unclaimed add rsp, 8
        ],
    );
    assert_eq!(
        build_terminal_object_artifact(&unclaimed_adjustment),
        Err(TerminalObjectError::UnclaimedUnitStackAdjustment {
            machine: machine_id(2),
            offset: 13,
        })
    );

    let mut unclaimed_aarch64_adjustment = internal_call_plan(NativeTarget::linux_arm64());
    account_aarch64_unit_call(&mut unclaimed_aarch64_adjustment);
    insert_aarch64_word(
        &mut unclaimed_aarch64_adjustment.functions[1].bytes,
        8,
        0xd100_43ff,
    );
    insert_aarch64_word(
        &mut unclaimed_aarch64_adjustment.functions[1].bytes,
        12,
        0x9100_43ff,
    );
    let caller = &mut unclaimed_aarch64_adjustment.functions[1];
    caller.internal_calls[0].offset = 16;
    let stack = caller.unit_stack.as_mut().expect("AArch64 Unit stack");
    stack
        .frame
        .as_mut()
        .expect("AArch64 Unit frame")
        .release_offset = 24;
    stack
        .aarch64_return_link
        .as_mut()
        .expect("AArch64 return link")
        .load_offset = 20;
    assert_eq!(
        build_terminal_object_artifact(&unclaimed_aarch64_adjustment),
        Err(TerminalObjectError::UnclaimedUnitStackAdjustment {
            machine: machine_id(2),
            offset: 8,
        })
    );
}

#[test]
fn x86_unit_stack_scan_uses_instruction_boundaries_not_immediate_substrings() {
    let mut plan = two_function_plan();
    plan.functions[0].bytes = vec![
        0x48, 0xb8, // mov rax, imm64
        0x48, 0x83, 0xec, 0x08, 0x48, 0x83, 0xc4, 0x08, // immediate payload
        0xc3,
    ];
    plan.functions[0].unit_stack = Some(TerminalUnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    build_terminal_object_artifact(&plan)
        .expect("stack-like immediate bytes are not stack instructions");
}

#[test]
fn object_replays_linear_scalar_stack_peaks_and_rejects_mutations() {
    let mut x86 = two_function_plan();
    x86.functions.truncate(1);
    x86.entry = machine_id(1);
    x86.functions[0].bytes = vec![
        0x50, // push rax
        0x52, // push rdx
        0x5a, // pop rdx
        0x58, // pop rax
        0xc3, // ret
    ];
    x86.functions[0].scalar_stack = Some(TerminalScalarStackEvidence {
        mutations: vec![
            scalar_mutation(0, 1, TerminalScalarStackMutationKind::X86Push),
            scalar_mutation(1, 1, TerminalScalarStackMutationKind::X86Push),
            scalar_mutation(2, 1, TerminalScalarStackMutationKind::X86Pop),
            scalar_mutation(3, 1, TerminalScalarStackMutationKind::X86Pop),
        ],
        stack_alignment: 16,
    });
    let artifact = build_terminal_object_artifact(&x86).expect("x86 scalar stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .unwrap()
            .local_peak_bytes,
        16
    );
    assert_eq!(
        omega_terminal_image_emission::derive_terminal_stack_demand(&artifact, machine_id(1))
            .expect("scalar stack demand")
            .ceiling_bytes(),
        16
    );

    let mut removed_push = x86.clone();
    removed_push.functions[0].bytes.remove(0);
    assert_eq!(
        build_terminal_object_artifact(&removed_push),
        Err(TerminalObjectError::InvalidScalarStackEvidence {
            machine: machine_id(1),
            offset: 1,
        })
    );

    let mut injected_push = x86.clone();
    injected_push.functions[0].bytes.insert(4, 0x50);
    assert_eq!(
        build_terminal_object_artifact(&injected_push),
        Err(TerminalObjectError::UnclaimedScalarStackMutation {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut branch = x86.clone();
    branch.functions[0].bytes.splice(4..4, [0xeb, 0]);
    assert_eq!(
        build_terminal_object_artifact(&branch),
        Err(TerminalObjectError::NonLinearScalarControlFlow {
            machine: machine_id(1),
            offset: 4,
        })
    );

    let mut unsupported_lea = x86.clone();
    unsupported_lea.functions[0].bytes = vec![
        0x48, 0x8d, 0x64, 0x24, 0x08, // lea rsp, [rsp + 8]
        0xc3,
    ];
    unsupported_lea.functions[0]
        .scalar_stack
        .as_mut()
        .expect("scalar evidence")
        .mutations
        .clear();
    assert_eq!(
        build_terminal_object_artifact(&unsupported_lea),
        Err(TerminalObjectError::UnsupportedScalarStackMutation {
            machine: machine_id(1),
            offset: 0,
        })
    );

    let mut call = internal_call_plan(NativeTarget::linux_x64());
    call.functions[1].scalar_stack = Some(TerminalScalarStackEvidence {
        mutations: Vec::new(),
        stack_alignment: 16,
    });
    assert_eq!(
        build_terminal_object_artifact(&call),
        Err(TerminalObjectError::ScalarStackCallNotSupported(
            machine_id(2)
        ))
    );

    let mut aarch64 = two_function_plan();
    aarch64.target = NativeTarget::linux_arm64();
    aarch64.functions.truncate(1);
    aarch64.entry = machine_id(1);
    aarch64.functions[0].bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xd100_43ff, // sub sp, sp, #16
        0x9100_43ff, // add sp, sp, #16
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    aarch64.functions[0].scalar_stack = Some(TerminalScalarStackEvidence {
        mutations: vec![
            scalar_mutation(
                0,
                4,
                TerminalScalarStackMutationKind::Allocate { byte_size: 16 },
            ),
            scalar_mutation(
                4,
                4,
                TerminalScalarStackMutationKind::Allocate { byte_size: 16 },
            ),
            scalar_mutation(
                8,
                4,
                TerminalScalarStackMutationKind::Release { byte_size: 16 },
            ),
            scalar_mutation(
                12,
                4,
                TerminalScalarStackMutationKind::Release { byte_size: 16 },
            ),
        ],
        stack_alignment: 16,
    });
    let artifact = build_terminal_object_artifact(&aarch64).expect("AArch64 scalar stack artifact");
    assert_eq!(
        artifact.functions()[0]
            .scalar_stack
            .unwrap()
            .local_peak_bytes,
        32
    );

    let mut injected_aarch64 = aarch64;
    insert_aarch64_word(&mut injected_aarch64.functions[0].bytes, 16, 0xd100_43ff);
    assert_eq!(
        build_terminal_object_artifact(&injected_aarch64),
        Err(TerminalObjectError::UnclaimedScalarStackMutation {
            machine: machine_id(1),
            offset: 16,
        })
    );

    let mut unsupported_aarch64 = two_function_plan();
    unsupported_aarch64.target = NativeTarget::linux_arm64();
    unsupported_aarch64.functions.truncate(1);
    unsupported_aarch64.entry = machine_id(1);
    unsupported_aarch64.functions[0].bytes = aarch64_words(&[
        0xa9bf_7bfd, // stp x29, x30, [sp, #-16]!
        0xd65f_03c0, // ret
    ]);
    unsupported_aarch64.functions[0].scalar_stack = Some(TerminalScalarStackEvidence {
        mutations: Vec::new(),
        stack_alignment: 16,
    });
    assert_eq!(
        build_terminal_object_artifact(&unsupported_aarch64),
        Err(TerminalObjectError::UnsupportedScalarStackMutation {
            machine: machine_id(1),
            offset: 0,
        })
    );
}

#[test]
fn scalar_stack_decoder_ignores_stack_opcode_bytes_inside_immediates() {
    let mut plan = two_function_plan();
    plan.functions.truncate(1);
    plan.entry = machine_id(1);
    plan.functions[0].bytes = vec![
        0x48, 0xb8, // mov rax, imm64
        0x50, 0x48, 0x83, 0xec, 8, 0x58, 0x90, 0x90, // immediate payload
        0xc3,
    ];
    plan.functions[0].scalar_stack = Some(TerminalScalarStackEvidence {
        mutations: Vec::new(),
        stack_alignment: 16,
    });
    build_terminal_object_artifact(&plan)
        .expect("stack opcodes inside an immediate are not instructions");
}

#[test]
fn terminal_unit_stack_demand_composes_the_exact_call_closure() {
    let mut plan = internal_call_plan(NativeTarget::linux_x64());
    plan.functions[0].unit_stack = Some(TerminalUnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    account_x86_unit_call(&mut plan);
    let artifact = build_terminal_object_artifact(&plan).expect("accounted Unit artifact");
    let demand = derive_terminal_unit_stack_demand(&artifact, machine_id(2))
        .expect("acyclic Unit closure stack demand");
    assert_eq!(demand.terminal_psi(), plan.terminal_psi);
    assert_eq!(demand.target(), plan.target);
    assert_eq!(demand.entry(), machine_id(2));
    assert_eq!(demand.ceiling_bytes(), 16);
    assert_eq!(demand.stack_alignment(), 16);
    assert_eq!(
        demand
            .contributing_machines()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [machine_id(1), machine_id(2)]
    );

    let mut unaccounted = internal_call_plan(NativeTarget::linux_x64());
    account_x86_unit_call(&mut unaccounted);
    let artifact = build_terminal_object_artifact(&unaccounted).expect("partly accounted artifact");
    assert_eq!(
        derive_terminal_unit_stack_demand(&artifact, machine_id(2)),
        Err(TerminalObjectError::UnaccountedTerminalUnitStack(
            machine_id(1)
        ))
    );
}

#[test]
fn supported_writers_preserve_exact_terminal_text_and_complete_regions() {
    let targets = [
        (NativeTarget::linux_x64(), b"\x7fELF".as_slice()),
        (NativeTarget::linux_arm64(), b"\x7fELF".as_slice()),
        (NativeTarget::macos_arm64(), b"\xcf\xfa\xed\xfe".as_slice()),
        (NativeTarget::windows_x64(), b"MZ".as_slice()),
    ];

    for (target, magic) in targets {
        let bytes = match target.architecture {
            omega_target::Architecture::X86_64 => integer_return(7),
            omega_target::Architecture::Aarch64 => {
                vec![0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
            }
        };
        let machine = machine_id(1);
        let plan = TerminalMachineCodePlan {
            terminal_psi: identity(),
            target,
            entry: machine,
            functions: vec![TerminalMachineCodeFunction {
                machine,
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: bytes.clone(),
                unit_stack: None,
                scalar_stack: None,
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            }],
        };
        let artifact = build_terminal_object_artifact(&plan).expect("artifact");
        let image = emit_terminal_executable_image(&artifact, 3)
            .unwrap_or_else(|error| panic!("{target:?} image failed: {error}"));
        assert_eq!(image.terminal_psi(), plan.terminal_psi);
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
                .expect("installation record");
        assert_eq!(
            installation.subsystem(),
            matches!(target.object_format, omega_target::ObjectFormat::Coff).then_some(3)
        );
        let installation_bytes =
            encode_terminal_installation_record(&installation).expect("installation bytes");
        assert_eq!(
            decode_terminal_installation_record(&installation_bytes),
            Ok(installation)
        );
        let image = image.output();

        assert!(image.bytes.starts_with(magic), "{target:?} image magic");
        assert_eq!(image.final_text_bytes, bytes, "{target:?} final text");
        assert_eq!(image.text_bytes, bytes.len());
        assert_eq!(image.relocations, 0);
        assert_eq!(image.final_image_imports, 0);
        assert_eq!(image.final_image_relocations, 0);
        assert!(image.executable_regions.unclassified_gaps.is_empty());
        assert_eq!(image.executable_regions.regions.len(), 1);
        assert_eq!(
            image.executable_regions.regions[0].symbol,
            artifact_symbol(&artifact)
        );
        let evidence = image
            .compiler_text_validation
            .expect("exact terminal text should publish validation evidence");
        assert_eq!(
            evidence.encoded_text_fingerprint,
            evidence.final_compiler_text_fingerprint
        );
        assert_eq!(evidence.text_relocation_count, 0);
        assert_eq!(evidence.checked_instruction_validation_count, 0);
    }
}

#[test]
fn installation_record_is_canonical_and_binds_exact_image_and_target_facts() {
    let plan = two_function_plan();
    let artifact = build_terminal_object_artifact(&plan).expect("artifact");
    let image = emit_terminal_executable_image(&artifact, 3).expect("Linux image");
    let record = build_terminal_installation_record(
        &image,
        ProfileDecisionId::new(11).expect("profile decision"),
    )
    .expect("installation record");

    assert_eq!(record.terminal_psi(), plan.terminal_psi);
    assert_eq!(record.target(), plan.target);
    assert_eq!(record.subsystem(), None);
    assert!(record.selected_provider_plans().is_empty());
    let bytes = encode_terminal_installation_record(&record).expect("canonical bytes");
    assert_eq!(&bytes[..8], b"PSIINST\0");
    assert_eq!(
        decode_terminal_installation_record(&bytes),
        Ok(record.clone())
    );
    validate_terminal_installation_record(&record, &image).expect("exact image binding");
    assert_eq!(
        terminal_installation_fingerprint(&record)
            .expect("installation fingerprint")
            .to_string(),
        "f1a637482775555fd523fdd5386ca8b09cc4a66cd39adaa3090d5d10e1798b14"
    );

    let mut changed_plan = plan;
    changed_plan.functions[1].bytes = integer_return(8);
    let changed_artifact = build_terminal_object_artifact(&changed_plan).expect("changed artifact");
    let changed_image =
        emit_terminal_executable_image(&changed_artifact, 3).expect("changed Linux image");
    assert_eq!(
        validate_terminal_installation_record(&record, &changed_image),
        Err(TerminalInstallationError::ImageBindingMismatch)
    );
}

#[test]
fn installation_decoder_rejects_alternate_and_malformed_encodings() {
    let artifact = build_terminal_object_artifact(&two_function_plan()).expect("artifact");
    let image = emit_terminal_executable_image(&artifact, 3).expect("image");
    let record = build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
        .expect("record");
    let bytes = encode_terminal_installation_record(&record).expect("bytes");

    let mut future = bytes.clone();
    future[8..10].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&future),
        Err(TerminalInstallationError::UnsupportedFormatMarker(4))
    );

    let mut wrong_pointer_width = bytes.clone();
    wrong_pointer_width[48..56].copy_from_slice(&4_u64.to_le_bytes());
    assert!(matches!(
        decode_terminal_installation_record(&wrong_pointer_width),
        Err(TerminalInstallationError::UnsupportedTarget(_))
    ));

    let mut zero_profile = bytes.clone();
    zero_profile[68..76].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_terminal_installation_record(&zero_profile),
        Err(TerminalInstallationError::ZeroProfileDecision)
    );

    assert_eq!(
        decode_terminal_installation_record(&bytes[..bytes.len() - 1]),
        Err(TerminalInstallationError::UnexpectedEnd)
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_terminal_installation_record(&trailing),
        Err(TerminalInstallationError::TrailingBytes(1))
    );
}

#[test]
fn privileged_effect_and_exact_provider_execution_survive_installation() {
    let port_operation = operation_id(1);
    let settlement_operation = operation_id(2);
    let service = ServiceId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let provider_plan = TerminalProviderPlanIdentity::new(7).unwrap();
    let provider_execution =
        TerminalProviderExecutionBinding::from_execution_record(provider_plan, 8, 9, 10, 11)
            .unwrap();
    let realization = TerminalMetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let mut bytes = omega_x86_encoding::encode_immediate_port_write(0x20, 0x20).to_vec();
    bytes.push(0xc3);
    let plan = TerminalMachineCodePlan {
        terminal_psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(1),
        functions: vec![TerminalMachineCodeFunction {
            machine: machine_id(1),
            provenance: TerminalPsiProvenance {
                operations: vec![port_operation, settlement_operation],
                edges: vec![edge_id(1)],
            },
            bytes,
            unit_stack: None,
            scalar_stack: None,
            internal_calls: Vec::new(),
            fuel_attribution: vec![
                TerminalNativeFuelAttribution {
                    schedule: psi_core::FuelScheduleIdentity::new(1).unwrap(),
                    site: TerminalNativeFuelSite::Operation(port_operation),
                    units: 1,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 27,
                },
                TerminalNativeFuelAttribution {
                    schedule: psi_core::FuelScheduleIdentity::new(1).unwrap(),
                    site: TerminalNativeFuelSite::Operation(settlement_operation),
                    units: 1,
                    operation_ordinal: 1,
                    code_offset: 27,
                    byte_count: 0,
                },
                TerminalNativeFuelAttribution {
                    schedule: psi_core::FuelScheduleIdentity::new(1).unwrap(),
                    site: TerminalNativeFuelSite::Edge(edge_id(1)),
                    units: 1,
                    operation_ordinal: 2,
                    code_offset: 27,
                    byte_count: 1,
                },
            ],
            port_effects: vec![TerminalPortEffectRecord {
                psi_operation: port_operation,
                service,
                port: 0x20,
                value: 0x20,
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 27,
            }],
            boundary_settlements: vec![TerminalBoundarySettlementRecord {
                psi_operation: settlement_operation,
                boundary,
                provider_execution: provider_execution.into(),
                realization,
                argument_places: Vec::new(),
                claim_settlements: Vec::new(),
                operation_ordinal: 1,
                code_offset: 27,
            }],
        }],
    };
    let artifact = build_terminal_object_artifact(&plan).expect("effect artifact");
    assert_eq!(artifact.fuel_attribution().len(), 3);
    assert_eq!(artifact.fuel_attribution()[1].attribution.byte_count, 0);
    assert_eq!(artifact.port_effects()[0].effect.service, service);
    assert_eq!(
        artifact.boundary_settlements()[0].settlement.realization,
        realization
    );
    let image = emit_terminal_executable_image(&artifact, 3).expect("effect image");
    assert_eq!(image.fuel_attribution(), artifact.fuel_attribution());
    assert_eq!(
        build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()),
        Err(TerminalInstallationError::ProviderExecutionClosureMismatch)
    );

    let mut wrong_bytes = plan.clone();
    wrong_bytes.functions[0].bytes[0] ^= 1;
    assert!(matches!(
        build_terminal_object_artifact(&wrong_bytes),
        Err(TerminalObjectError::PortEffectBytesMismatch { .. })
    ));
    let mut wrong_schedule = plan.clone();
    wrong_schedule.functions[0].fuel_attribution[0].schedule =
        psi_core::FuelScheduleIdentity::new(2).unwrap();
    assert_eq!(
        build_terminal_object_artifact(&wrong_schedule),
        Err(TerminalObjectError::InvalidFuelAttribution(machine_id(1)))
    );
    let mut wrong_realization = plan;
    wrong_realization.functions[0].boundary_settlements[0]
        .realization
        .value = 0x21;
    assert!(matches!(
        build_terminal_object_artifact(&wrong_realization),
        Err(TerminalObjectError::BoundaryRealizationMismatch { .. })
    ));
}

#[test]
fn image_boundary_rejects_noncanonical_pointer_facts() {
    let mut plan = two_function_plan();
    plan.target.pointer_size = 4;
    assert!(!can_emit_terminal_executable_image(plan.target));
    let artifact = build_terminal_object_artifact(&plan).expect("owned artifact");
    assert!(emit_terminal_executable_image(&artifact, 3).is_err());
}

fn artifact_symbol(artifact: &omega_terminal_image_emission::TerminalObjectArtifact) -> &str {
    object_symbol_name(artifact.object(), artifact.entry_function().symbol)
}

fn two_function_plan() -> TerminalMachineCodePlan {
    TerminalMachineCodePlan {
        terminal_psi: identity(),
        target: NativeTarget::linux_x64(),
        entry: machine_id(2),
        functions: vec![
            TerminalMachineCodeFunction {
                machine: machine_id(1),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: integer_return(3),
                unit_stack: None,
                scalar_stack: None,
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
            TerminalMachineCodeFunction {
                machine: machine_id(2),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: integer_return(7),
                unit_stack: None,
                scalar_stack: None,
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
        ],
    }
}

fn internal_call_plan(target: NativeTarget) -> TerminalMachineCodePlan {
    let (callee, caller, call_offset) = match target.architecture {
        omega_target::Architecture::X86_64 => (integer_return(3), vec![0xe8, 0, 0, 0, 0, 0xc3], 1),
        omega_target::Architecture::Aarch64 => (
            vec![0x60, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6],
            vec![0x00, 0x00, 0x00, 0x94, 0xc0, 0x03, 0x5f, 0xd6],
            0,
        ),
    };
    TerminalMachineCodePlan {
        terminal_psi: identity(),
        target,
        entry: machine_id(2),
        functions: vec![
            TerminalMachineCodeFunction {
                machine: machine_id(1),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(1)],
                    edges: vec![edge_id(1)],
                },
                bytes: callee,
                unit_stack: None,
                scalar_stack: None,
                internal_calls: Vec::new(),
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
            TerminalMachineCodeFunction {
                machine: machine_id(2),
                provenance: TerminalPsiProvenance {
                    operations: vec![operation_id(2)],
                    edges: vec![edge_id(2)],
                },
                bytes: caller,
                unit_stack: None,
                scalar_stack: None,
                internal_calls: vec![TerminalInternalCallRelocation {
                    psi_operation: operation_id(2),
                    target: machine_id(1),
                    unit_stack: None,
                    offset: call_offset,
                }],
                fuel_attribution: Vec::new(),
                port_effects: Vec::new(),
                boundary_settlements: Vec::new(),
            },
        ],
    }
}

fn account_x86_unit_call(plan: &mut TerminalMachineCodePlan) {
    let caller = &mut plan.functions[1];
    caller.bytes = vec![
        0x48, 0x83, 0xec, 0x08, // sub rsp, 8
        0xe8, 0, 0, 0, 0, // call rel32
        0x48, 0x83, 0xc4, 0x08, // add rsp, 8
        0xc3, // ret
    ];
    caller.unit_stack = Some(TerminalUnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    caller.internal_calls[0].offset = 5;
    caller.internal_calls[0].unit_stack = Some(TerminalUnitCallStackEvidence {
        outbound: Some(TerminalStackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 9,
            release_byte_count: 4,
        }),
    });
}

fn scalar_mutation(
    offset: usize,
    byte_count: usize,
    kind: TerminalScalarStackMutationKind,
) -> TerminalScalarStackMutation {
    TerminalScalarStackMutation {
        offset,
        byte_count,
        kind,
    }
}

fn account_aarch64_unit_call(plan: &mut TerminalMachineCodePlan) {
    let frame = TerminalStackAdjustmentPair {
        byte_size: 16,
        allocation_offset: 0,
        allocation_byte_count: 4,
        release_offset: 12,
        release_byte_count: 4,
    };
    let link = TerminalAarch64ReturnLinkEvidence {
        frame_byte_offset: 0,
        store_offset: 4,
        load_offset: 8,
    };
    plan.functions[0].bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xf900_03fe, // str x30, [sp]
        0xf940_03fe, // ldr x30, [sp]
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    plan.functions[0].unit_stack = Some(TerminalUnitStackEvidence {
        frame: Some(frame),
        aarch64_return_link: Some(link),
        stack_alignment: 16,
    });

    let caller = &mut plan.functions[1];
    caller.bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xf900_03fe, // str x30, [sp]
        0x9400_0000, // bl immediate
        0xf940_03fe, // ldr x30, [sp]
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    caller.unit_stack = Some(TerminalUnitStackEvidence {
        frame: Some(TerminalStackAdjustmentPair {
            release_offset: 16,
            ..frame
        }),
        aarch64_return_link: Some(TerminalAarch64ReturnLinkEvidence {
            load_offset: 12,
            ..link
        }),
        stack_alignment: 16,
    });
    caller.internal_calls[0].offset = 8;
    caller.internal_calls[0].unit_stack = Some(TerminalUnitCallStackEvidence { outbound: None });
}

fn aarch64_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn insert_aarch64_word(bytes: &mut Vec<u8>, offset: usize, word: u32) {
    bytes.splice(offset..offset, word.to_le_bytes());
}

fn integer_return(value: u8) -> Vec<u8> {
    vec![0xb8, value, 0, 0, 0, 0xc3]
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).expect("machine")
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).expect("operation")
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).expect("edge")
}

fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}
