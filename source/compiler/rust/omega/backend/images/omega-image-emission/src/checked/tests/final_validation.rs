//! Final image, instruction-boundary, and executable-region validation regressions.

use super::*;
use crate::checked::{
    final_region_byte_fingerprint, retain_compiler_function_identity,
    validate_compiler_function_object_binding, validate_compiler_instruction_relocation_origins,
};
use std::collections::{HashMap, HashSet};

fn bind_encoded_function_object_symbol(
    object: &mut omega_object_file::ObjectPlan,
    function: &omega_machine_bytes::EncodedMachineFunction,
) -> omega_object_file::ObjectSymbolHandle {
    let name = omega_object_file::private_function_symbol_name(function.identity)
        .unwrap_or_else(|| format!("function_{}", object.layout.symbols.len()));
    let symbol = object.layout.symbols.insert(omega_object_file::SymbolPlan {
        name,
        section: omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text),
        offset: function.byte_offset,
        size: function.byte_count,
        kind: omega_object_file::SymbolKind::Function,
        import_library: String::new(),
    });
    object
        .layout
        .function_symbols
        .insert(omega_object_file::FunctionSymbolPlan {
            identity: function.identity,
            symbol,
        });
    symbol
}

#[test]
fn final_function_identity_partition_is_unique_valid_and_fingerprinted() {
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use psi_symbols::SymbolHandle;

    fn identity_fingerprint(identity: MachineFunctionIdentity) -> u64 {
        let mut identities = HashSet::new();
        let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
        retain_compiler_function_identity(0, identity, &mut identities, &mut fingerprint)
            .expect("valid function identity");
        fingerprint
    }

    let continuation = StateKey {
        machine: SymbolHandle::from_parts(1, 2),
        state: SymbolHandle::from_parts(3, 4),
        segment_index: 5,
    };
    let source = MachineFunctionIdentity::source(continuation);
    let callback = MachineFunctionIdentity::callback_thunk(continuation, 7).unwrap();
    assert_ne!(identity_fingerprint(source), identity_fingerprint(callback));
    assert_ne!(
        identity_fingerprint(callback),
        identity_fingerprint(MachineFunctionIdentity::callback_thunk(continuation, 8).unwrap())
    );
    assert_ne!(
        identity_fingerprint(callback),
        identity_fingerprint(
            MachineFunctionIdentity::callback_thunk(
                StateKey {
                    state: SymbolHandle::from_parts(3, 5),
                    ..continuation
                },
                7,
            )
            .unwrap()
        )
    );

    let mut source_identities = HashSet::from([source]);
    let mut source_fingerprint = identity_fingerprint(source);
    let duplicate = retain_compiler_function_identity(
        1,
        source,
        &mut source_identities,
        &mut source_fingerprint,
    )
    .expect_err("duplicate final function identity must reject");
    assert!(
        duplicate
            .message
            .contains("duplicates compiler-private identity")
    );

    let invalid = retain_compiler_function_identity(
        0,
        MachineFunctionIdentity::default(),
        &mut HashSet::new(),
        &mut 0xcbf2_9ce4_8422_2325u64,
    )
    .expect_err("invalid final function identity must reject");
    assert!(
        invalid
            .message
            .contains("invalid compiler-private identity")
    );
}

#[test]
fn final_function_identity_owns_one_exact_object_text_interval() {
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_machine_bytes::EncodedMachineFunction;
    use psi_symbols::SymbolHandle;

    let target = NativeTarget::linux_x64();
    let identity = MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: SymbolHandle::from_parts(1, 2),
            state: SymbolHandle::from_parts(3, 4),
            segment_index: 5,
        },
        7,
    )
    .expect("valid callback identity");
    let function = EncodedMachineFunction {
        symbol: "callback".into(),
        identity,
        byte_offset: 8,
        byte_count: 16,
        ..EncodedMachineFunction::default()
    };
    let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 1, 1);
    let symbol = bind_encoded_function_object_symbol(&mut object, &function);

    validate_compiler_function_object_binding(0, &function, &object)
        .expect("exact callback identity and text interval should rejoin");

    let missing = omega_object_file::ObjectPlan::with_capacities(target, 0, 0, 0);
    let diagnostic = validate_compiler_function_object_binding(0, &function, &missing)
        .expect_err("missing final object identity must reject");
    assert!(diagnostic.message.contains("one exact object text symbol"));

    let mut redirected = object.clone();
    let binding = redirected
        .layout
        .function_symbols
        .iter()
        .next()
        .expect("one function binding")
        .0;
    redirected.layout.function_symbols.get_mut(binding).identity =
        MachineFunctionIdentity::source(identity.associated_source_continuation());
    assert!(validate_compiler_function_object_binding(0, &function, &redirected).is_err());

    let mut duplicate = object.clone();
    duplicate
        .layout
        .function_symbols
        .insert(omega_object_file::FunctionSymbolPlan { identity, symbol });
    assert!(validate_compiler_function_object_binding(0, &function, &duplicate).is_err());

    let mut interval_drift = object.clone();
    interval_drift.layout.symbols.get_mut(symbol).size += 1;
    let diagnostic = validate_compiler_function_object_binding(0, &function, &interval_drift)
        .expect_err("object interval drift must reject");
    assert!(
        diagnostic
            .message
            .contains("does not match encoded interval")
    );

    let mut callback_entry = object;
    callback_entry.layout.entry_symbol = symbol;
    let diagnostic = validate_compiler_function_object_binding(0, &function, &callback_entry)
        .expect_err("callback identity cannot replace the process entry");
    assert!(
        diagnostic
            .message
            .contains("cannot own the object entry symbol")
    );
}

#[test]
fn final_source_wrapper_and_entry_linkage_names_are_canonical() {
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_machine_bytes::EncodedMachineFunction;
    use psi_symbols::SymbolHandle;

    let target = NativeTarget::linux_x64();
    let continuation = StateKey {
        machine: SymbolHandle::from_parts(1, 2),
        state: SymbolHandle::from_parts(3, 4),
        segment_index: 5,
    };
    let source = EncodedMachineFunction {
        symbol: "authored_display_name".into(),
        identity: MachineFunctionIdentity::source(continuation),
        byte_offset: 8,
        byte_count: 16,
        ..EncodedMachineFunction::default()
    };
    let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 1, 1);
    let source_symbol = bind_encoded_function_object_symbol(&mut object, &source);
    validate_compiler_function_object_binding(0, &source, &object)
        .expect("non-entry source uses its canonical private linkage name");

    let mut renamed_source = object.clone();
    renamed_source.layout.symbols.get_mut(source_symbol).name = "renamed".into();
    let diagnostic = validate_compiler_function_object_binding(0, &source, &renamed_source)
        .expect_err("source linkage substitution must reject");
    assert!(
        diagnostic
            .message
            .contains("canonical identity-derived name")
    );

    let mut entry = object.clone();
    entry.layout.entry_symbol = source_symbol;
    entry.layout.symbols.get_mut(source_symbol).name = omega_object_file::entry_symbol_name(target);
    validate_compiler_function_object_binding(0, &source, &entry)
        .expect("entry linkage uses the target's canonical public name");
    entry.layout.symbols.get_mut(source_symbol).name = "not_main".into();
    assert!(validate_compiler_function_object_binding(0, &source, &entry).is_err());

    let wrapper = EncodedMachineFunction {
        identity: MachineFunctionIdentity::program_storage_entry_wrapper(continuation)
            .expect("valid wrapper identity"),
        byte_offset: 24,
        ..source.clone()
    };
    let mut wrapper_object = omega_object_file::ObjectPlan::with_capacities(target, 0, 1, 1);
    bind_encoded_function_object_symbol(&mut wrapper_object, &wrapper);
    validate_compiler_function_object_binding(1, &wrapper, &wrapper_object)
        .expect("non-entry wrapper uses its canonical private linkage name");
}

#[test]
fn callback_function_address_store_replays_both_symbolic_addresses_on_both_isas() {
    use omega_calling_conventions::{
        MachineRegister, MachineStateSet, RegisterSet, StateFootprintEvidence,
        compose_state_footprints,
    };
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_machine_bytes::{
        CompilerInstructionValidationKind, EncodedMachineFunction, EncodedMachineInstruction,
        EncodedMachinePlan,
    };
    use omega_machine_instructions::{BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin};
    use omega_target::{Architecture, NativeTarget};
    use omega_target_operations::RuntimeStorageRegion;
    use psi_arena::HandleSpan;
    use psi_symbols::SymbolHandle;

    let key = StateKey {
        machine: SymbolHandle::from_arena_index(1),
        state: SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let source_identity = MachineFunctionIdentity::source(key);
    let callback_identity = MachineFunctionIdentity::callback_thunk(key, 7).unwrap();

    for target in [NativeTarget::windows_x64(), NativeTarget::linux_arm64()] {
        let architecture = target.architecture;
        let enter = match architecture {
            Architecture::X86_64 => omega_isa_x86_64::encode_function_enter_bytes().to_vec(),
            Architecture::Aarch64 => omega_isa_aarch64::encode_function_enter_bytes().to_vec(),
        };
        let store = match architecture {
            Architecture::X86_64 => {
                omega_isa_x86_64::encode_runtime_storage_function_address_write(
                    RuntimeStorageRegion::Machine,
                    24,
                )
                .unwrap()
                .0
            }
            Architecture::Aarch64 => {
                omega_isa_aarch64::encode_runtime_storage_function_address_write(24).unwrap()
            }
        };
        let leave = match architecture {
            Architecture::X86_64 => omega_isa_x86_64::encode_return_bytes().to_vec(),
            Architecture::Aarch64 => omega_isa_aarch64::encode_return_bytes().to_vec(),
        };
        let mut plan = EncodedMachinePlan::with_capacity(
            target,
            1,
            3,
            enter.len() + store.len() + leave.len(),
        );
        let enter_bytes = plan.code.bytes.insert_many(enter.iter().copied());
        let store_offset = enter.len();
        let store_bytes = plan.code.bytes.insert_many(store.iter().copied());
        let return_bytes = plan.code.bytes.insert_many(leave.iter().copied());
        let mut instruction_span = HandleSpan::empty();
        for instruction in [
            EncodedMachineInstruction {
                selected_instruction_index: 1,
                bytes: enter_bytes,
                compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionEnter),
                ..EncodedMachineInstruction::default()
            },
            EncodedMachineInstruction {
                selected_instruction_index: 2,
                bytes: store_bytes,
                compiler_validation_kind: Some(
                    CompilerInstructionValidationKind::CompilerBodyFunctionAddressStore {
                        function: callback_identity,
                        target_region: RuntimeStorageRegion::Machine,
                        target_offset: 24,
                    },
                ),
                ..EncodedMachineInstruction::default()
            },
            EncodedMachineInstruction {
                selected_instruction_index: 3,
                bytes: return_bytes,
                compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionReturn),
                ..EncodedMachineInstruction::default()
            },
        ] {
            plan.code
                .instructions
                .append_to_span(&mut instruction_span, instruction);
        }
        let function = plan.code.functions.insert(EncodedMachineFunction {
            symbol: "registrar".into(),
            identity: source_identity,
            byte_offset: 0,
            byte_count: plan.code.bytes.len(),
            instructions: instruction_span,
        });
        plan.code.byte_count = plan.code.bytes.len();

        let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 3, 2);
        let source_symbol =
            bind_encoded_function_object_symbol(&mut object, plan.code.functions.get(function));
        let callback_symbol = object.layout.symbols.insert(SymbolPlan {
            name: "callback_private".into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset: plan.code.byte_count,
            size: 4,
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object
            .layout
            .function_symbols
            .insert(omega_object_file::FunctionSymbolPlan {
                identity: callback_identity,
                symbol: callback_symbol,
            });
        let storage_symbol = object.layout.symbols.insert(SymbolPlan {
            name: omega_object_file::storage_region_symbol_name(
                RuntimeStorageRegion::Machine,
                "Main",
            ),
            section: SymbolSection::Section(SectionKind::Bss),
            offset: 0,
            size: 64,
            kind: SymbolKind::Object,
            import_library: String::new(),
        });

        let mut relocations = RelocationPlan::with_target(target);
        let sites = match architecture {
            Architecture::X86_64 => vec![
                (
                    store_offset + 2,
                    8,
                    RelocationKind::Absolute64,
                    callback_symbol,
                ),
                (
                    store_offset + 12,
                    8,
                    RelocationKind::Absolute64,
                    storage_symbol,
                ),
            ],
            Architecture::Aarch64 => vec![
                (
                    store_offset,
                    4,
                    RelocationKind::Aarch64Page21,
                    callback_symbol,
                ),
                (
                    store_offset + 4,
                    4,
                    RelocationKind::Aarch64PageOffset12,
                    callback_symbol,
                ),
                (
                    store_offset + 8,
                    4,
                    RelocationKind::Aarch64Page21,
                    storage_symbol,
                ),
                (
                    store_offset + 12,
                    4,
                    RelocationKind::Aarch64PageOffset12,
                    storage_symbol,
                ),
            ],
        };
        for (offset, byte_width, kind, symbol_handle) in sites {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: source_symbol,
                    selected_instruction_index: 2,
                },
                section: SectionKind::Text,
                offset,
                byte_width,
                symbol_handle,
                addend: 0,
                kind,
            });
        }

        let store_registers = match architecture {
            Architecture::X86_64 => {
                RegisterSet::new([MachineRegister::X86R14, MachineRegister::X86R15])
            }
            Architecture::Aarch64 => {
                RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
            }
        };
        let enter_footprint = match architecture {
            Architecture::X86_64 => StateFootprintEvidence::new(
                omega_isa_x86_64::function_enter_register_writes(),
                omega_isa_x86_64::function_enter_additional_machine_state(),
            ),
            Architecture::Aarch64 => StateFootprintEvidence::new(
                omega_isa_aarch64::function_enter_register_writes(),
                omega_isa_aarch64::function_enter_additional_machine_state(),
            ),
        };
        let return_footprint = match architecture {
            Architecture::X86_64 => StateFootprintEvidence::new(
                omega_isa_x86_64::return_register_writes(),
                omega_isa_x86_64::return_additional_machine_state(),
            ),
            Architecture::Aarch64 => StateFootprintEvidence::new(
                omega_isa_aarch64::return_register_writes(),
                omega_isa_aarch64::return_additional_machine_state(),
            ),
        };
        plan.semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x1234);
        plan.semantics.boundaries.footprints.fragments.extend([
            BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                evidence: compose_state_footprints([&enter_footprint, &return_footprint]),
            },
            BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
                evidence: StateFootprintEvidence::new(store_registers, MachineStateSet::empty()),
            },
        ]);
        let final_bytes = plan
            .code
            .bytes
            .iter()
            .map(|(_, byte)| *byte)
            .collect::<Vec<_>>();
        let evidence = validate_compiler_function_instruction_boundaries(
            architecture,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &plan.semantics,
        )
        .expect("exact callback address store must replay through final bytes");
        assert_eq!(evidence.instruction_count, 3);

        let mut redirected = relocations.clone();
        let redirected_record = redirected
            .record_set
            .records
            .iter()
            .find(|(_, record)| record.symbol_handle == callback_symbol)
            .unwrap()
            .0;
        redirected
            .record_set
            .records
            .get_mut(redirected_record)
            .symbol_handle = source_symbol;
        assert!(
            validate_compiler_function_instruction_boundaries(
                architecture,
                &plan.code,
                &final_bytes,
                &object,
                &redirected,
                &plan.semantics,
            )
            .is_err()
        );

        let first_record = relocations.records().next().unwrap().0;
        let mut wrong_kind = relocations.clone();
        wrong_kind.record_set.records.get_mut(first_record).kind = RelocationKind::X86_64Relative32;
        assert!(
            validate_compiler_function_instruction_boundaries(
                architecture,
                &plan.code,
                &final_bytes,
                &object,
                &wrong_kind,
                &plan.semantics,
            )
            .is_err()
        );
        let mut wrong_addend = relocations.clone();
        wrong_addend.record_set.records.get_mut(first_record).addend = 1;
        assert!(
            validate_compiler_function_instruction_boundaries(
                architecture,
                &plan.code,
                &final_bytes,
                &object,
                &wrong_addend,
                &plan.semantics,
            )
            .is_err()
        );
        let mut wrong_origin = relocations.clone();
        wrong_origin.record_set.records.get_mut(first_record).origin =
            RelocationOrigin::Instruction {
                function_symbol_handle: callback_symbol,
                selected_instruction_index: 2,
            };
        assert!(
            validate_compiler_function_instruction_boundaries(
                architecture,
                &plan.code,
                &final_bytes,
                &object,
                &wrong_origin,
                &plan.semantics,
            )
            .is_err()
        );
        let duplicate_record = relocations.records().next().unwrap().1.clone();
        let mut duplicate = relocations.clone();
        duplicate.push_record(duplicate_record);
        assert!(
            validate_compiler_function_instruction_boundaries(
                architecture,
                &plan.code,
                &final_bytes,
                &object,
                &duplicate,
                &plan.semantics,
            )
            .is_err()
        );

        let mut changed_opcode = final_bytes.clone();
        changed_opcode[store_offset + store.len() - 1] ^= 1;
        assert!(
            validate_compiler_function_instruction_boundaries(
                architecture,
                &plan.code,
                &changed_opcode,
                &object,
                &relocations,
                &plan.semantics,
            )
            .is_err()
        );
    }
}

#[test]
fn final_instruction_relocations_retain_the_exact_function_owner() {
    let target = NativeTarget::linux_x64();
    let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 1, 0);
    let symbol = object.layout.symbols.insert(omega_object_file::SymbolPlan {
        name: "function".into(),
        section: omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text),
        offset: 0,
        size: 8,
        kind: omega_object_file::SymbolKind::Function,
        import_library: String::new(),
    });
    let instruction_owners = HashMap::from([(41, symbol)]);
    let mut relocations = RelocationPlan::with_target(target);
    let relocation = relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: symbol,
            selected_instruction_index: 41,
        },
        section: SectionKind::Text,
        offset: 1,
        byte_width: 4,
        symbol_handle: symbol,
        addend: 0,
        kind: RelocationKind::X86_64Relative32,
    });

    validate_compiler_instruction_relocation_origins(&instruction_owners, &relocations)
        .expect("exact instruction owner should remain joined");

    let mut redirected = relocations.clone();
    if let RelocationOrigin::Instruction {
        function_symbol_handle,
        ..
    } = &mut redirected.record_set.records.get_mut(relocation).origin
    {
        *function_symbol_handle = psi_arena::Handle::invalid();
    }
    let diagnostic =
        validate_compiler_instruction_relocation_origins(&instruction_owners, &redirected)
            .expect_err("redirected function origin must reject");
    assert!(diagnostic.message.contains("exact final function symbol"));

    let mut unknown = relocations;
    if let RelocationOrigin::Instruction {
        selected_instruction_index,
        ..
    } = &mut unknown.record_set.records.get_mut(relocation).origin
    {
        *selected_instruction_index = 42;
    }
    assert!(
        validate_compiler_instruction_relocation_origins(&instruction_owners, &unknown).is_err()
    );
}

#[test]
fn final_executable_regions_retain_exact_compiler_function_identity_and_bytes() {
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_image::{
        FinalExecutableRegionOrigin, PlacedExecutableRegion, PlacedExecutableRegionInventory,
    };
    use omega_machine_bytes::EncodedMachineFunction;

    let target = NativeTarget::linux_x64();
    let final_bytes = [1, 2, 3, 4, 5, 6, 7, 8];
    let identity = MachineFunctionIdentity::source(StateKey {
        machine: psi_arena::Handle::from_parts(1, 2),
        state: psi_arena::Handle::from_parts(3, 4),
        segment_index: 5,
    });
    let mut plan =
        omega_machine_bytes::EncodedMachinePlan::with_capacity(target, 1, 0, final_bytes.len());
    plan.code.bytes.insert_many(final_bytes);
    plan.code.functions.insert(EncodedMachineFunction {
        symbol: "display".into(),
        identity,
        byte_offset: 0,
        byte_count: final_bytes.len(),
        ..EncodedMachineFunction::default()
    });
    plan.code.byte_count = final_bytes.len();
    let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 1, 1);
    let function = plan.code.functions.iter().next().unwrap().1;
    bind_encoded_function_object_symbol(&mut object, function);
    let (entry_symbol, symbol) = omega_object_file::object_function_symbol(&object, identity)
        .expect("exact object function");
    let symbol = symbol.name.clone();
    object.layout.entry_symbol = entry_symbol;
    let region = PlacedExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 0,
        address: 0x1000,
        byte_count: final_bytes.len(),
        byte_fingerprint: final_region_byte_fingerprint(&final_bytes),
        symbol,
        footprint: None,
    };
    let inventory = PlacedExecutableRegionInventory {
        text_address: 0x1000,
        text_byte_count: final_bytes.len(),
        text_fingerprint: final_region_byte_fingerprint(&final_bytes),
        inventory_fingerprint: 1,
        regions: vec![region],
        unclassified_gaps: Vec::new(),
    };

    let (exact_binding, entry_binding) =
        validate_executable_region_enumeration(&inventory, &plan.code, &object, &final_bytes)
            .expect("exact compiler-function region should rejoin");
    assert_ne!(exact_binding, 0);
    assert_eq!(entry_binding.function_identity, identity);
    assert_eq!(
        entry_binding.object_symbol_handle,
        object.layout.entry_symbol
    );
    assert_eq!(entry_binding.region_index, 0);
    assert_eq!(
        entry_binding.inventory_fingerprint,
        inventory.inventory_fingerprint
    );
    assert_eq!(
        entry_binding.final_region_binding_fingerprint,
        exact_binding
    );
    assert_ne!(entry_binding.evidence_fingerprint, 0);
    assert_eq!(
        entry_binding.evidence_fingerprint,
        entry_binding.recomputed_evidence_fingerprint()
    );

    let mut changed_inventory_identity = inventory.clone();
    changed_inventory_identity.inventory_fingerprint ^= 1;
    let (changed_binding, _) = validate_executable_region_enumeration(
        &changed_inventory_identity,
        &plan.code,
        &object,
        &final_bytes,
    )
    .expect("the row join should retain the supplied inventory identity");
    assert_ne!(changed_binding, exact_binding);

    for mutate in [
        |region: &mut PlacedExecutableRegion| region.symbol.push_str("_drift"),
        |region: &mut PlacedExecutableRegion| region.section_offset += 1,
        |region: &mut PlacedExecutableRegion| region.address += 1,
        |region: &mut PlacedExecutableRegion| region.byte_count -= 1,
        |region: &mut PlacedExecutableRegion| region.byte_fingerprint ^= 1,
    ] {
        let mut drifted = inventory.clone();
        mutate(&mut drifted.regions[0]);
        assert!(
            validate_executable_region_enumeration(&drifted, &plan.code, &object, &final_bytes,)
                .is_err()
        );
    }

    let mut wrong_namespace = inventory.clone();
    wrong_namespace.regions[0].origin = FinalExecutableRegionOrigin::ImportThunk;
    assert!(
        validate_executable_region_enumeration(
            &wrong_namespace,
            &plan.code,
            &object,
            &final_bytes,
        )
        .is_err()
    );

    let mut duplicate = inventory.clone();
    duplicate.regions.push(duplicate.regions[0].clone());
    assert!(
        validate_executable_region_enumeration(&duplicate, &plan.code, &object, &final_bytes,)
            .is_err()
    );
}

#[test]
fn compiler_validation_identity_without_a_footprint_derivation_rejects() {
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, StateGuardOperator};

    let place = Place::at(RuntimeStorageRegion::RuntimeFrame, 16)
        .with_step(PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::Machine,
            index_offset: 24,
            index_byte_size: 8,
            element_byte_size: 4,
        })
        .expect("indexed place");
    let diagnostic = require_compiler_instruction_footprint(
        omega_target::Architecture::Aarch64,
        &psi_arena::Arena::new(),
        CompilerInstructionValidationKind::PlaceValueGuard {
            place,
            byte_size: 4,
            expected_value: 7,
            failure_branch_distance: 12,
            operator: StateGuardOperator::Equal,
        },
        41,
    )
    .expect_err("an unsupported final-body footprint must not be omitted");

    assert!(diagnostic.message.contains("instruction #41"));
    assert!(
        diagnostic
            .message
            .contains("no target footprint derivation")
    );
}

#[test]
fn balanced_outgoing_stack_frame_final_bytes_and_footprint_replay_fail_closed() {
    use omega_machine_bytes::{
        CompilerInstructionValidationKind, EncodedMachineFunction, EncodedMachineInstruction,
    };
    use omega_machine_instructions::{BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin};
    use psi_arena::HandleSpan;

    let target = NativeTarget::uefi_x64();
    let enter = omega_isa_x86_64::encode_function_enter_bytes();
    let reserve = omega_isa_x86_64::encode_outgoing_stack_frame_reserve_bytes(72)
        .expect("exact outgoing frame reservation");
    let writes = [(32, 0x1000), (40, 0x800), (48, 0x8000), (56, 0x2000)].map(|(offset, value)| {
        omega_isa_x86_64::encode_outgoing_stack_u64_write_bytes(offset, value)
            .expect("exact outgoing word")
    });
    let address = omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(
        omega_calling_conventions::MachineRegister::X86Rcx,
        32,
    )
    .expect("exact RCX caller-copy address");
    let storage_address = omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(
        omega_calling_conventions::MachineRegister::X86Rdx,
        48,
    )
    .expect("exact RDX caller-copy address");
    let release = omega_isa_x86_64::encode_outgoing_stack_frame_release_bytes(72)
        .expect("exact outgoing frame release");
    let leave = omega_isa_x86_64::encode_return_bytes();
    let final_bytes = enter
        .into_iter()
        .chain(reserve.iter().copied())
        .chain(writes.iter().flatten().copied())
        .chain(address)
        .chain(storage_address)
        .chain(release.iter().copied())
        .chain(leave)
        .collect::<Vec<_>>();
    let mut plan =
        omega_machine_bytes::EncodedMachinePlan::with_capacity(target, 1, 10, final_bytes.len());
    let enter_bytes = plan.code.bytes.insert_many(enter);
    let reserve_bytes = plan.code.bytes.insert_many(reserve.iter().copied());
    let write_bytes = writes.map(|bytes| plan.code.bytes.insert_many(bytes));
    let address_bytes = plan.code.bytes.insert_many(address);
    let storage_address_bytes = plan.code.bytes.insert_many(storage_address);
    let release_bytes = plan.code.bytes.insert_many(release.iter().copied());
    let leave_bytes = plan.code.bytes.insert_many(leave);
    let first = plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 0,
        bytes: enter_bytes,
        compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionEnter),
        ..Default::default()
    });
    let reserve_row = plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 1,
        bytes: reserve_bytes,
        compiler_validation_kind: Some(
            CompilerInstructionValidationKind::OutgoingStackFrameReserve { byte_count: 72 },
        ),
        ..Default::default()
    });
    let write_rows = [(32, 0x1000), (40, 0x800), (48, 0x8000), (56, 0x2000)]
        .into_iter()
        .zip(write_bytes)
        .enumerate()
        .map(|(index, ((stack_byte_offset, value), bytes))| {
            plan.code.instructions.insert(EncodedMachineInstruction {
                selected_instruction_index: index as u32 + 2,
                bytes,
                compiler_validation_kind: Some(
                    CompilerInstructionValidationKind::OutgoingStackU64Write {
                        stack_byte_offset,
                        value,
                    },
                ),
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();
    let address_row = plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 6,
        bytes: address_bytes,
        compiler_validation_kind: Some(
            CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                register: omega_calling_conventions::MachineRegister::X86Rcx,
                stack_byte_offset: 32,
            },
        ),
        ..Default::default()
    });
    plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 7,
        bytes: storage_address_bytes,
        compiler_validation_kind: Some(
            CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                register: omega_calling_conventions::MachineRegister::X86Rdx,
                stack_byte_offset: 48,
            },
        ),
        ..Default::default()
    });
    let release_row = plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 8,
        bytes: release_bytes,
        compiler_validation_kind: Some(
            CompilerInstructionValidationKind::OutgoingStackFrameRelease { byte_count: 72 },
        ),
        ..Default::default()
    });
    plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 9,
        bytes: leave_bytes,
        compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionReturn),
        ..Default::default()
    });
    let function = plan.code.functions.insert(EncodedMachineFunction {
        symbol: "synthetic_wrapper".into(),
        identity: omega_control_flow::MachineFunctionIdentity::source(
            omega_control_flow::StateKey {
                machine: psi_symbols::SymbolHandle::from_arena_index(1),
                state: psi_symbols::SymbolHandle::from_arena_index(2),
                segment_index: 0,
            },
        ),
        byte_offset: 0,
        byte_count: final_bytes.len(),
        instructions: HandleSpan::from_parts(first, 10),
    });
    plan.code.byte_count = final_bytes.len();

    let mut semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();
    semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint = Some(0x5151);
    let enter_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::function_enter_register_writes(),
        omega_isa_x86_64::function_enter_additional_machine_state(),
    );
    let address_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::outgoing_stack_address_load_register_writes(
            omega_calling_conventions::MachineRegister::X86Rcx,
        ),
        omega_isa_x86_64::outgoing_stack_address_load_additional_machine_state(),
    );
    let storage_address_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::outgoing_stack_address_load_register_writes(
            omega_calling_conventions::MachineRegister::X86Rdx,
        ),
        omega_isa_x86_64::outgoing_stack_address_load_additional_machine_state(),
    );
    let adjust_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::outgoing_stack_frame_adjust_register_writes(),
        omega_isa_x86_64::outgoing_stack_frame_adjust_additional_machine_state(),
    );
    let write_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::outgoing_stack_u64_write_register_writes(),
        omega_isa_x86_64::outgoing_stack_u64_write_additional_machine_state(),
    );
    let return_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::return_register_writes(),
        omega_isa_x86_64::return_additional_machine_state(),
    );
    semantics
        .boundaries
        .footprints
        .fragments
        .push(BoundaryFootprintFragment {
            origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
            evidence: omega_calling_conventions::compose_state_footprints([
                &enter_footprint,
                &adjust_footprint,
                &write_footprint,
                &address_footprint,
                &storage_address_footprint,
                &adjust_footprint,
                &return_footprint,
            ]),
        });
    let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 1, 1);
    bind_encoded_function_object_symbol(&mut object, plan.code.functions.get(function));
    let relocations = RelocationPlan::with_target(target);
    let evidence = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &final_bytes,
        &object,
        &relocations,
        &semantics,
    )
    .expect("exact balanced outgoing frame should replay with no relocation");
    assert_eq!(evidence.instruction_count, 10);

    let mut duplicate_instruction_owner = plan.code.clone();
    let duplicate_index = duplicate_instruction_owner
        .instructions
        .get(reserve_row)
        .selected_instruction_index;
    duplicate_instruction_owner
        .instructions
        .get_mut(release_row)
        .selected_instruction_index = duplicate_index;
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &duplicate_instruction_owner,
        &final_bytes,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("one selected instruction cannot be retained twice");
    assert!(
        diagnostic
            .message
            .contains("more than one final function row")
    );

    let reserve_offset = enter.len();
    let writes_offset = reserve_offset + reserve.len();
    let address_offset = writes_offset + writes.iter().map(|bytes| bytes.len()).sum::<usize>();
    let release_offset = address_offset + address.len() + storage_address.len();
    for tamper_offset in [
        reserve_offset,
        writes_offset,
        writes_offset + 2,
        writes_offset + 10,
        address_offset,
        address_offset + 2,
        release_offset,
    ] {
        let mut tampered = final_bytes.clone();
        tampered[tamper_offset] ^= 1;
        assert!(
            validate_compiler_function_instruction_boundaries(
                omega_target::Architecture::X86_64,
                &plan.code,
                &tampered,
                &object,
                &relocations,
                &semantics,
            )
            .is_err()
        );
    }

    let mut redirected = plan.code.clone();
    redirected
        .instructions
        .get_mut(address_row)
        .compiler_validation_kind = Some(
        CompilerInstructionValidationKind::OutgoingStackAddressLoad {
            register: omega_calling_conventions::MachineRegister::X86Rdx,
            stack_byte_offset: 32,
        },
    );

    let mut redirected_write = plan.code.clone();
    redirected_write
        .instructions
        .get_mut(write_rows[0])
        .compiler_validation_kind =
        Some(CompilerInstructionValidationKind::OutgoingStackU64Write {
            stack_byte_offset: 40,
            value: 0x1000,
        });
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &redirected_write,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .is_err()
    );

    let mut redirected_value = plan.code.clone();
    redirected_value
        .instructions
        .get_mut(write_rows[0])
        .compiler_validation_kind =
        Some(CompilerInstructionValidationKind::OutgoingStackU64Write {
            stack_byte_offset: 32,
            value: 0x1001,
        });
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &redirected_value,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .is_err()
    );

    let mut mismatched_release = plan.code.clone();
    mismatched_release
        .instructions
        .get_mut(release_row)
        .compiler_validation_kind =
        Some(CompilerInstructionValidationKind::OutgoingStackFrameRelease { byte_count: 88 });
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &mismatched_release,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .is_err()
    );

    let mut nested_reserve = plan.code.clone();
    nested_reserve
        .instructions
        .get_mut(release_row)
        .compiler_validation_kind =
        Some(CompilerInstructionValidationKind::OutgoingStackFrameReserve { byte_count: 72 });
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &nested_reserve,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .is_err()
    );

    let mut orphan_address = plan.code.clone();
    orphan_address
        .instructions
        .get_mut(reserve_row)
        .compiler_validation_kind = None;
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &orphan_address,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .is_err()
    );
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &redirected,
            &final_bytes,
            &object,
            &relocations,
            &semantics,
        )
        .is_err()
    );

    let mut incomplete_footprint = semantics.clone();
    incomplete_footprint.boundaries.footprints.fragments[0].evidence =
        omega_calling_conventions::compose_state_footprints([&enter_footprint, &return_footprint]);
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &incomplete_footprint,
        )
        .is_err()
    );

    let mut missing_write_footprint = semantics.clone();
    missing_write_footprint.boundaries.footprints.fragments[0].evidence =
        omega_calling_conventions::compose_state_footprints([
            &enter_footprint,
            &adjust_footprint,
            &address_footprint,
            &storage_address_footprint,
            &adjust_footprint,
            &return_footprint,
        ]);
    assert!(
        validate_compiler_function_instruction_boundaries(
            omega_target::Architecture::X86_64,
            &plan.code,
            &final_bytes,
            &object,
            &relocations,
            &missing_write_footprint,
        )
        .is_err()
    );
}

#[test]
fn aarch64_indirect_call_replay_reconstructs_bytes_and_page_sites() {
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, HostBindingMechanism, ValueLocation, ValueShape,
        evaluate_call_plan,
    };
    use omega_target_operations::{
        InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
    };
    use std::sync::Arc;

    let operands = vec![
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 4,
            },
        },
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_count: 8,
            },
        },
        InstructionOperand {
            kind: InstructionOperandKind::ImmediateInteger(7),
        },
    ];
    let plan = evaluate_call_plan(
        CallingPolicy::Aapcs64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: Some(ValueShape::integer(4, 4)),
        },
    )
    .expect("AAPCS64 vtable plan");
    let mechanism = HostBindingMechanism::VtableField {
        table: Arc::from("Protocol"),
        field: Arc::from("invoke"),
        byte_offset: 8,
    };
    let (bytes, sites) =
        encode_aarch64_indirect_call_replay(&operands, &[], &mechanism, &plan, true, true)
            .expect("final AArch64 vtable replay");

    let lowered = operands
        .iter()
        .map(super::aarch64_outbound_syscall_operand)
        .collect::<Result<Vec<_>, _>>()
        .expect("AArch64 replay operands");
    let result_register = match plan.result.as_ref().expect("result").locations.as_slice() {
        [ValueLocation::Register { register, .. }] => *register,
        other => panic!("unexpected result placement: {other:?}"),
    };
    let inner =
        omega_isa_aarch64::encode_vtable_call_sequence_at_offset_value_returning_from_operands(
            lowered.iter().copied(),
            &plan.parameters,
            result_register,
            8,
        )
        .expect("AAPCS64 vtable bytes");
    let expected = omega_isa_aarch64::encode_foreign_float_control_prefix_bytes()
        .into_iter()
        .chain(inner)
        .chain(omega_isa_aarch64::encode_foreign_float_control_suffix_bytes())
        .collect::<Vec<_>>();
    assert_eq!(bytes, expected);
    assert_eq!(
        sites,
        vec![
            (
                36,
                super::OutboundCallRelocationTarget::Storage(RuntimeStorageRegion::RuntimeFrame)
            ),
            (
                12,
                super::OutboundCallRelocationTarget::Storage(RuntimeStorageRegion::Machine)
            ),
        ]
    );

    let table_operands = vec![
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            },
        },
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 16,
                byte_count: 8,
            },
        },
    ];
    let table_plan = evaluate_call_plan(
        CallingPolicy::Aapcs64,
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(24, 8)),
        },
    )
    .expect("AAPCS64 table-function plan");
    let table_mechanism = HostBindingMechanism::TableFunction {
        table: Arc::from("Services"),
        field: Arc::from("allocate"),
        byte_offset: 40,
    };
    let (_, table_sites) = encode_aarch64_indirect_call_replay(
        &table_operands,
        &[],
        &table_mechanism,
        &table_plan,
        true,
        true,
    )
    .expect("final AArch64 table-function replay");
    assert_eq!(
        table_sites,
        vec![
            (
                12,
                super::OutboundCallRelocationTarget::Storage(RuntimeStorageRegion::RuntimeFrame)
            ),
            (
                24,
                super::OutboundCallRelocationTarget::Storage(RuntimeStorageRegion::Machine)
            ),
        ]
    );
}

#[test]
fn outbound_syscall_storage_sites_cover_runtime_descriptors_and_addresses() {
    use omega_target_operations::{
        InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
    };

    let operands = vec![
        InstructionOperand {
            kind: InstructionOperandKind::ImmediateInteger(7),
        },
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16,
                is_bounded_buffer: false,
            },
        },
        InstructionOperand {
            kind: InstructionOperandKind::RuntimePointeeStringLength {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 24,
            },
        },
        InstructionOperand {
            kind: InstructionOperandKind::RuntimeStorageAddress {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
            },
        },
        InstructionOperand {
            kind: InstructionOperandKind::DataAddress {
                data: Handle::invalid(),
            },
        },
    ];

    let x86_sites =
        outbound_syscall_argument_storage_sites(omega_target::Architecture::X86_64, &operands)
            .expect("x86 descriptor/address sites");
    assert_eq!(
        x86_sites,
        vec![
            (
                omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 1) - 2,
                RuntimeStorageRegion::RuntimeFrame,
            ),
            (
                omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 2) - 2,
                RuntimeStorageRegion::Machine,
            ),
            (
                omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 3) - 2,
                RuntimeStorageRegion::RuntimeFrame,
            ),
        ]
    );

    let aarch64_operands = operands
        .iter()
        .map(super::aarch64_outbound_syscall_operand)
        .collect::<Result<Vec<_>, _>>()
        .expect("AArch64 descriptor/address operands");
    let aarch64_sites =
        outbound_syscall_argument_storage_sites(omega_target::Architecture::Aarch64, &operands)
            .expect("AArch64 descriptor/address sites");
    assert_eq!(
        aarch64_sites,
        vec![
            (
                omega_isa_aarch64::operand_width(&aarch64_operands[0]),
                RuntimeStorageRegion::RuntimeFrame,
            ),
            (
                aarch64_operands[..2]
                    .iter()
                    .map(omega_isa_aarch64::operand_width)
                    .sum(),
                RuntimeStorageRegion::Machine,
            ),
            (
                aarch64_operands[..3]
                    .iter()
                    .map(omega_isa_aarch64::operand_width)
                    .sum(),
                RuntimeStorageRegion::RuntimeFrame,
            ),
        ]
    );

    let symbols = vec![std::sync::Arc::<str>::from("literal.data")];
    let x86_data_sites = outbound_syscall_argument_data_sites(
        omega_target::Architecture::X86_64,
        &operands,
        &symbols,
    )
    .expect("x86 data-object site");
    assert_eq!(
        x86_data_sites,
        vec![(
            omega_isa_x86_64::syscall_data_relocation_byte_offset(&operands, 4) - 2,
            std::sync::Arc::<str>::from("literal.data"),
        )]
    );
    let aarch64_data_sites = outbound_syscall_argument_data_sites(
        omega_target::Architecture::Aarch64,
        &operands,
        &symbols,
    )
    .expect("AArch64 data-object site");
    assert_eq!(
        aarch64_data_sites,
        vec![(
            aarch64_operands[..4]
                .iter()
                .map(omega_isa_aarch64::operand_width)
                .sum(),
            std::sync::Arc::<str>::from("literal.data"),
        )]
    );
}

#[test]
fn rejects_native_image_when_encoded_text_size_differs_from_plan() {
    let target = NativeTarget::linux_arm64();
    let object = ObjectPlan::with_capacity(target, 0, 0);
    let relocations = RelocationPlan::with_target(target);
    let semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();

    let diagnostic = emit_checked_executable_image(
        ExecutableImageInput {
            target,
            callback_placement_identity_fingerprint: 0,
            object: &object,
            relocations: &relocations,
            encoded_machine_code: &omega_machine_bytes::EncodedMachinePlan::with_capacity(
                target, 0, 0, 0,
            )
            .code,
            encoded_machine_semantics: &semantics,
            text_bytes: &[0xaa, 0xbb],
            data_bytes: &[],
            subsystem: 3,
        },
        4,
    )
    .expect_err("encoded/planned byte mismatch should fail before image dispatch");

    assert!(diagnostic.message.contains("encoded 2 machine byte(s)"));
    assert!(diagnostic.message.contains("planned 4 byte(s)"));
}

#[test]
fn final_text_changes_only_inside_declared_relocation_bits() {
    let encoded = [0xe8, 0, 0, 0, 0, 0xc3];
    let mut relocated = encoded;
    relocated[1..5].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 1,
        },
        section: SectionKind::Text,
        offset: 1,
        byte_width: 4,
        symbol_handle: Handle::invalid(),
        addend: 0,
        kind: RelocationKind::X86_64Relative32,
    });

    let evidence = validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
        .expect("declared displacement bytes may change");
    assert_eq!(evidence.text_relocation_count, 1);
    assert_ne!(evidence.encoded_text_fingerprint, 0);
    assert_ne!(evidence.derivation_fingerprint, 0);
    let mut addend_relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    let mut addend_record = relocations
        .records()
        .next()
        .expect("relocation record")
        .1
        .clone();
    addend_record.addend = 4;
    addend_relocations.push_record(addend_record);
    let addend_evidence =
        validate_final_text_relocation_envelope(&encoded, &relocated, &addend_relocations)
            .expect("addend remains valid envelope evidence");
    assert_ne!(
        evidence.relocation_envelope_fingerprint, addend_evidence.relocation_envelope_fingerprint,
        "semantic addends must participate in the final relocation identity"
    );
    relocated[0] = 0x90;
    let diagnostic = validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
        .expect_err("an opcode mutation outside the displacement must reject");
    assert!(diagnostic.message.contains("byte 0"));
}

#[test]
fn compiler_functions_retain_a_complete_final_instruction_partition() {
    use omega_machine_bytes::{
        CheckedInstructionValidationKind, CompilerInstructionValidationKind,
        EncodedMachineFunction, EncodedMachineInstruction,
    };
    use omega_machine_instructions::{BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin};
    use psi_arena::HandleSpan;

    let target = NativeTarget::linux_x64();
    let mut object = omega_object_file::ObjectPlan::with_capacities(target, 0, 2, 1);
    let storage_symbol = object.layout.symbols.insert(SymbolPlan {
        name: omega_object_file::runtime_frame_storage_symbol_name(),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 0,
        size: 64,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    let enter = omega_isa_x86_64::encode_function_enter_bytes();
    let dispatch =
        omega_isa_x86_64::encode_dispatch_loop_enter_bytes(7).expect("dispatch loop entry");
    let guard = omega_isa_x86_64::encode_dispatch_guard_compare_static_bytes(
        4,
        4,
        9,
        16,
        omega_target_operations::StateGuardOperator::Equal,
        false,
    )
    .expect("static dispatch guard");
    let leave = omega_isa_x86_64::encode_return_bytes();
    let guard_byte_offset = enter.len() + dispatch.len();
    let mut final_guard = guard.clone();
    final_guard[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    let final_bytes = enter
        .into_iter()
        .chain(dispatch.iter().copied())
        .chain(final_guard)
        .chain(leave)
        .collect::<Vec<_>>();
    let mut relocations = RelocationPlan::with_target(target);
    let guard_relocation = relocations.push_record(RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: Handle::invalid(),
            selected_instruction_index: 6,
        },
        section: SectionKind::Text,
        offset: guard_byte_offset + 2,
        byte_width: 8,
        symbol_handle: storage_symbol,
        addend: 0,
        kind: RelocationKind::Absolute64,
    });
    let mut plan =
        omega_machine_bytes::EncodedMachinePlan::with_capacity(target, 1, 5, final_bytes.len());
    let enter_bytes = plan.code.bytes.insert_many(enter);
    let dispatch_bytes = plan.code.bytes.insert_many(dispatch);
    let guard_bytes = plan.code.bytes.insert_many(guard);
    let leave_bytes = plan.code.bytes.insert_many(leave);
    let first = plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 4,
        bytes: enter_bytes,
        compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionEnter),
        ..EncodedMachineInstruction::default()
    });
    let dispatch_row = plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 5,
        bytes: dispatch_bytes,
        compiler_validation_kind: Some(CompilerInstructionValidationKind::DispatchLoopEnter {
            entry_dispatch_index: 7,
        }),
        ..EncodedMachineInstruction::default()
    });
    plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 6,
        bytes: guard_bytes,
        compiler_validation_kind: Some(CompilerInstructionValidationKind::DispatchStaticGuard {
            operator: omega_target_operations::StateGuardOperator::Equal,
            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            byte_offset: 4,
            byte_size: 4,
            expected_value: 9,
            skip_byte_distance: 16,
            is_float: false,
        }),
        ..EncodedMachineInstruction::default()
    });
    plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 7,
        ..EncodedMachineInstruction::default()
    });
    plan.code.instructions.insert(EncodedMachineInstruction {
        selected_instruction_index: 8,
        bytes: leave_bytes,
        compiler_validation_kind: Some(CompilerInstructionValidationKind::FunctionReturn),
        ..EncodedMachineInstruction::default()
    });
    let function = plan.code.functions.insert(EncodedMachineFunction {
        symbol: std::sync::Arc::from("entry"),
        identity: omega_control_flow::MachineFunctionIdentity::source(
            omega_control_flow::StateKey {
                machine: psi_symbols::SymbolHandle::from_arena_index(1),
                state: psi_symbols::SymbolHandle::from_arena_index(2),
                segment_index: 0,
            },
        ),
        byte_offset: 0,
        byte_count: final_bytes.len(),
        instructions: HandleSpan::from_parts(first, 5),
    });
    let function_symbol =
        bind_encoded_function_object_symbol(&mut object, plan.code.functions.get(function));
    relocations
        .record_set
        .records
        .get_mut(guard_relocation)
        .origin = RelocationOrigin::Instruction {
        function_symbol_handle: function_symbol,
        selected_instruction_index: 6,
    };
    plan.code.byte_count = final_bytes.len();
    let mut semantics = omega_machine_bytes::EncodedMachineSemanticSummary::default();
    semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint = Some(0x1234);
    let enter_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::function_enter_register_writes(),
        omega_isa_x86_64::function_enter_additional_machine_state(),
    );
    let return_footprint = omega_calling_conventions::StateFootprintEvidence::new(
        omega_isa_x86_64::return_register_writes(),
        omega_isa_x86_64::return_additional_machine_state(),
    );
    semantics
        .boundaries
        .footprints
        .fragments
        .push(BoundaryFootprintFragment {
            origin: BoundaryFootprintFragmentOrigin::CallReturnMechanics,
            evidence: omega_calling_conventions::compose_state_footprints([
                &enter_footprint,
                &return_footprint,
            ]),
        });
    semantics
        .boundaries
        .footprints
        .fragments
        .push(BoundaryFootprintFragment {
            origin: BoundaryFootprintFragmentOrigin::DispatchScaffold,
            evidence: omega_calling_conventions::StateFootprintEvidence::new(
                omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                omega_calling_conventions::MachineStateSet::empty(),
            ),
        });
    semantics
        .boundaries
        .footprints
        .fragments
        .push(BoundaryFootprintFragment {
            origin: BoundaryFootprintFragmentOrigin::StaticGuardComparison,
            evidence: omega_calling_conventions::StateFootprintEvidence::new(
                omega_isa_x86_64::dispatch_guard_compare_static_register_writes(false),
                omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
            ),
        });

    let evidence = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &final_bytes,
        &object,
        &relocations,
        &semantics,
    )
    .expect("retained function rows should enumerate exact final boundaries");
    assert_eq!(evidence.function_count, 1);
    assert_eq!(evidence.instruction_count, 5);
    assert_eq!(evidence.zero_width_instruction_count, 1);
    assert_eq!(evidence.checked_assembly_instruction_count, 0);
    assert_eq!(evidence.fixed_mechanics_instruction_count, 2);
    assert_ne!(evidence.fixed_mechanics_footprint_fingerprint, 0);
    assert_eq!(evidence.body_specification_instruction_count, 2);
    assert_ne!(evidence.body_specification_footprint_fingerprint, 0);
    assert_eq!(
        evidence.composed_footprint_fingerprint,
        semantics
            .boundaries
            .footprints
            .composed_evidence()
            .evidence_fingerprint()
    );

    let mut unclassified = plan.code.clone();
    unclassified
        .instructions
        .get_mut(dispatch_row)
        .compiler_validation_kind = None;
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &unclassified,
        &final_bytes,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("a byte-bearing row without validation authority must reject");
    assert!(diagnostic.message.contains("exactly one"));

    let mut conflicting = plan.code.clone();
    conflicting
        .instructions
        .get_mut(dispatch_row)
        .checked_validation_kind = Some(CheckedInstructionValidationKind::FullFence);
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &conflicting,
        &final_bytes,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("a row with two validation authorities must reject");
    assert!(diagnostic.message.contains("exactly one"));

    let mut mismatched_mechanics = semantics.clone();
    mismatched_mechanics
        .boundaries
        .footprints
        .fragments
        .retain(|fragment| fragment.origin != BoundaryFootprintFragmentOrigin::CallReturnMechanics);
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &final_bytes,
        &object,
        &relocations,
        &mismatched_mechanics,
    )
    .expect_err("final call-return footprint without its StatePlan fragment must reject");
    assert!(diagnostic.message.contains("CallReturnMechanics"));

    let mut mismatched_semantics = semantics.clone();
    mismatched_semantics
        .boundaries
        .footprints
        .fragments
        .retain(|fragment| {
            fragment.origin != BoundaryFootprintFragmentOrigin::StaticGuardComparison
        });
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &final_bytes,
        &object,
        &relocations,
        &mismatched_semantics,
    )
    .expect_err("final guard footprint without its StatePlan fragment must reject");
    assert!(diagnostic.message.contains("StatePlan-validated"));

    let missing_relocations = RelocationPlan::with_target(target);
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &final_bytes,
        &object,
        &missing_relocations,
        &semantics,
    )
    .expect_err("a static guard without its retained relocation must reject");
    assert!(
        diagnostic
            .message
            .contains("storage-address relocation shape")
    );

    let mut mutated = final_bytes.clone();
    mutated[guard_byte_offset] ^= 0xff;
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &mutated,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("a static guard opcode mutation must reject");
    assert!(
        diagnostic
            .message
            .contains("fixed target instruction specification")
    );

    let mut mutated = final_bytes.clone();
    mutated[0] ^= 0xff;
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &mutated,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("mutated fixed mechanics must reject");
    assert!(
        diagnostic
            .message
            .contains("fixed target instruction specification")
    );

    let mut mutated = final_bytes.clone();
    mutated[enter.len()] ^= 0xff;
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &mutated,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("mutated dispatch specification bytes must reject");
    assert!(
        diagnostic
            .message
            .contains("fixed target instruction specification")
    );

    plan.code.functions.get_mut(function).instructions = HandleSpan::from_parts(first, 4);
    let diagnostic = validate_compiler_function_instruction_boundaries(
        omega_target::Architecture::X86_64,
        &plan.code,
        &final_bytes,
        &object,
        &relocations,
        &semantics,
    )
    .expect_err("a function without its retained return row must reject");
    assert!(
        diagnostic
            .message
            .contains("entry and return validation rows")
    );
}
