use machine_code::{
    MachineCodeFunction, MachineCodePlan, SemanticCodeAttribution, SemanticCodeSite,
    UnitAffineCleanupRecord, UnitStackEvidence,
};
use semantic_vocabulary::{EdgeId, MachineId};
use target::NativeTarget;
use target_operations::TerminalPsiProvenance;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::*;

#[test]
fn construction_rejects_detached_final_plan_and_object_terminal() {
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([17; 32]),
    };
    let retained = abstract_operations::AbstractOperationPlan {
        psi,
        entry: semantic_vocabulary::MachineId::new(1).unwrap(),
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: Vec::new(),
    };
    assert!(validate_final_plan(&retained, &retained, psi, psi).is_ok());
    let mut changed = retained.clone();
    changed.entry = semantic_vocabulary::MachineId::new(2).unwrap();
    assert!(validate_final_plan(&changed, &retained, psi, psi).is_err());
    let mut changed_terminal = psi;
    changed_terminal.program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([18; 32]);
    assert!(validate_final_plan(&retained, &retained, psi, changed_terminal).is_err());
}

#[test]
fn admitted_object_binding_rejects_a_different_valid_object() {
    let original = image_emission::build_object_artifact(&plan(1)).unwrap();
    let different = image_emission::build_object_artifact(&plan(2)).unwrap();
    let binding = FragmentPublicationBinding {
        object: Arc::new(original.clone()),
        identity: [7; 32],
    };
    assert!(binding.validate_object(&original).is_ok());
    assert!(binding.validate_object(&different).is_err());
}

fn terminal() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([17; 32]),
    }
}

fn plan(machine_raw: u64) -> MachineCodePlan {
    let machine = MachineId::new(machine_raw).expect("nonzero machine");
    let return_edge = EdgeId::new(7).expect("nonzero edge");
    MachineCodePlan {
        psi: terminal(),
        target: NativeTarget::linux_x64(),
        entry: machine,
        functions: vec![MachineCodeFunction {
            machine,
            attachment: None,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            structural_call_scalar_return: None,
            unit_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![return_edge],
            },
            bytes: vec![0xc3],
            x86_scalar_fma: Vec::new(),
            x86_scalar_fma_occurrences: Vec::new(),
            x86_floating_control: None,
            unit_stack: Some(UnitStackEvidence {
                frame: None,
                aarch64_return_link: None,
                stack_alignment: 16,
            }),
            unit_parameter_homes: Vec::new(),
            unit_parameters: Vec::new(),
            scalar_stack: None,
            internal_calls: Vec::new(),
            foreign_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            internal_unit_scalar_calls: Vec::new(),
            installed_provider_unit_scalar_calls: Vec::new(),
            dynamic_calls: Vec::new(),
            stored_dynamic_calls: Vec::new(),
            dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_parameter_calls: Vec::new(),
            forwarded_dynamic_descriptor_calls: Vec::new(),
            unit_scalar_homes: Vec::new(),
            unit_integer_constants: Vec::new(),
            unit_affine_scalar_records: Vec::new(),
            unit_structural_scalar_field_stores: Vec::new(),
            unit_write_only_primitive_stores: Vec::new(),
            scalar_structural_scalar_field_stores: Vec::new(),
            unit_affine_cleanup: Some(UnitAffineCleanupRecord {
                psi_edge: return_edge,
                structural_types: Vec::new(),
                locals: Vec::new(),
                actions: Vec::new(),
                code_offset: 0,
                byte_count: 1,
            }),
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            ranked_u32_countdown: None,
            semantic_code_attribution: vec![SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(return_edge),
                operation_ordinal: 0,
                code_offset: 0,
                byte_count: 1,
            }],
            port_effects: Vec::new(),
            boundary_settlements: Vec::new(),
            structural_return: None,
        }],
    }
}
