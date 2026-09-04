//! Reauthenticated ordinary-callable record and nested-subrecord mutations.

use crate::tests::*;
use omega_calling_conventions::{CallingPolicy, MachineRegister};
use omega_object_file::ObjectLocalSymbolId;
use omega_optimization_core::{
    OptimizationSelectionIdentity, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use omega_regalloc::RegisterHomeIdentity;
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::{Architecture, ObjectFormat};
use psi_core::{EdgeId, ScalarType, ValueId};
use psi_terminal::SemanticFingerprint;

use super::fixture::staged_callable;

type RecordMutation = fn(&mut OptimizedOrdinaryCallableEntryRecord);

#[test]
fn every_representable_ordinary_callable_record_field_rejects_after_reauthentication() {
    let mut staged = staged_callable();
    let baseline = staged.entry().clone();
    assert!(!baseline.parameters[0].storage_units.is_empty());
    assert!(!baseline.result.storage_units.is_empty());
    assert!(!baseline.returns[0].storage_units.is_empty());
    // Vocabulary, hardening, and disposition are singleton in memory; their
    // closed tags are covered by the independent wire matrix.
    let mutations: [(&str, RecordMutation); 54] = [
        ("source_artifact", |record| {
            record.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"other source artifact")
        }),
        ("source_manifest", |record| {
            record.source_manifest = OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                b"other source manifest",
            )
        }),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xe1; 32])
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0xe2; 32])
        }),
        ("target.architecture", |record| {
            record.target.architecture = Architecture::Aarch64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::Coff
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size += 1
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment += 1
        }),
        ("semantic_entry", |record| {
            record.semantic_entry = MachineId::new(99_911).unwrap()
        }),
        ("selected", |record| {
            record.selected = SelectedInstructionPlanIdentity::from_bytes([0xe3; 32])
        }),
        ("register_homes", |record| {
            record.register_homes = RegisterHomeIdentity::from_bytes([0xe4; 32])
        }),
        ("physical_register_model", |record| {
            record.physical_register_model = PhysicalRegisterModelIdentity::from_bytes([0xe5; 32])
        }),
        ("exit_contract", |record| {
            record.exit_contract = WholeFunctionExitContractIdentity::from_bytes([0xe6; 32])
        }),
        ("object", |record| {
            record.object = RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"other object")
        }),
        ("object_container", |record| {
            record.object_container =
                RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"other container")
        }),
        ("semantic_entry_symbol", |record| {
            record.semantic_entry_symbol = ObjectLocalSymbolId::new(99_912).unwrap()
        }),
        ("semantic_entry_symbol_name", |record| {
            record.semantic_entry_symbol_name.push_str("_other")
        }),
        ("semantic_entry_section_offset", |record| {
            record.semantic_entry_section_offset += 1
        }),
        ("semantic_entry_byte_count", |record| {
            record.semantic_entry_byte_count += 1
        }),
        ("calling_policy", |record| {
            record.calling_policy = CallingPolicy::MicrosoftX64
        }),
        ("parameters.roster", |record| record.parameters.clear()),
        ("parameters[].ordinal", |record| {
            record.parameters[0].ordinal += 1
        }),
        ("parameters[].value", |record| {
            record.parameters[0].value = ValueId::new(99_913).unwrap()
        }),
        ("parameters[].scalar_type", |record| {
            record.parameters[0].scalar_type = record.result.declaration.scalar_type
        }),
        ("parameters[].shape.byte_size", |record| {
            record.parameters[0].shape.byte_size += 1
        }),
        ("parameters[].shape.alignment", |record| {
            record.parameters[0].shape.alignment += 1
        }),
        ("parameters[].virtual_register", |record| {
            record.parameters[0].virtual_register = VirtualRegisterId(99_914)
        }),
        ("parameters[].class", |record| {
            record.parameters[0].class = RegisterClassId(99)
        }),
        ("parameters[].abi_register", |record| {
            record.parameters[0].abi_register = MachineRegister::X86Rax
        }),
        ("parameters[].fixed_view", |record| {
            record.parameters[0].fixed_view = RegisterViewId(99)
        }),
        ("parameters[].assigned_view", |record| {
            record.parameters[0].assigned_view = RegisterViewId(98)
        }),
        ("parameters[].storage_units.roster", |record| {
            record.parameters[0].storage_units.clear()
        }),
        ("parameters[].storage_units[]", |record| {
            record.parameters[0].storage_units[0] = RegisterUnitId(99)
        }),
        ("result.declaration.id", |record| {
            record.result.declaration.id = ValueId::new(99_915).unwrap()
        }),
        ("result.declaration.scalar_type", |record| {
            record.result.declaration.scalar_type = ScalarType::Boolean
        }),
        ("result.shape.byte_size", |record| {
            record.result.shape.byte_size += 1
        }),
        ("result.shape.alignment", |record| {
            record.result.shape.alignment += 1
        }),
        ("result.abi_register", |record| {
            record.result.abi_register = MachineRegister::X86Rcx
        }),
        ("result.view", |record| {
            record.result.view = RegisterViewId(97)
        }),
        ("result.storage_units.roster", |record| {
            record.result.storage_units.clear()
        }),
        ("result.storage_units[]", |record| {
            record.result.storage_units[0] = RegisterUnitId(98)
        }),
        ("returns.roster", |record| record.returns.clear()),
        ("returns[].edge", |record| {
            record.returns[0].edge = EdgeId::new(99_916).unwrap()
        }),
        ("returns[].value", |record| {
            record.returns[0].value = ValueId::new(99_917).unwrap()
        }),
        ("returns[].selected_instruction", |record| {
            record.returns[0].selected_instruction = SelectedInstructionId(99_918)
        }),
        ("returns[].virtual_register", |record| {
            record.returns[0].virtual_register = VirtualRegisterId(99_919)
        }),
        ("returns[].view", |record| {
            record.returns[0].view = RegisterViewId(96)
        }),
        ("returns[].storage_units.roster", |record| {
            record.returns[0].storage_units.clear()
        }),
        ("returns[].storage_units[]", |record| {
            record.returns[0].storage_units[0] = RegisterUnitId(97)
        }),
        ("exit_policy", |record| {
            record.exit_policy = WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1
        }),
        ("entry_assumption", |record| {
            record.entry_assumption = WholeFunctionEntryAssumption::CallerLinkRegisterV1 {
                link_register: RegisterViewId(95),
            }
        }),
        ("stack_pointer", |record| {
            record.stack_pointer = RegisterViewId(94)
        }),
        ("stack_alignment", |record| record.stack_alignment += 1),
        ("red_zone_bytes", |record| record.red_zone_bytes += 1),
    ];

    for (field, mutate) in mutations {
        *staged.entry_mut() = baseline.clone();
        let record = staged.entry_mut();
        mutate(record);
        record.identity = record.recomputed_identity().unwrap();
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::RecordMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.entry_mut() = baseline;
    staged.entry_mut().identity =
        omega_optimization_core::OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(
            [0xe7; 32],
        );
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::RecordMismatch),
    );
}
