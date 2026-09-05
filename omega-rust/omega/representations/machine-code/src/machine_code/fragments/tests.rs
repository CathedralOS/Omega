use optimization_core::FunctionFragmentEmissionIdentity;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use target_operations::TerminalPsiProvenance;
use terminal_psi::TerminalPsiIdentity;

use super::*;
use semantic_vocabulary::ValueId;

fn zero_span_plan() -> FunctionFragmentEmissionPlan {
    let mut plan = FunctionFragmentEmissionPlan {
        identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
        psi: TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
        target: NativeTarget::linux_x64(),
        entry: MachineId::new(1).unwrap(),
        functions: vec![FunctionFragment {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance::default(),
            byte_count: 0,
            bytes: Vec::new(),
            blocks: vec![FunctionFragmentBlockSpan {
                block: SelectedBlockId(0),
                offset: 0,
                byte_count: 0,
                instructions: vec![FunctionFragmentInstructionSpan {
                    instruction: SelectedInstructionId(0),
                    alternative: MachineAlternativeKey {
                        family: MachineAlternativeFamily::CompareI64Zero,
                        variant: 0,
                    },
                    offset: 0,
                    bytes: Vec::new(),
                    branch: None,
                    internal_machine_fixup: None,
                    provenance: SelectedInstructionProvenance::default(),
                    control: FunctionFragmentControlProvenance::None,
                }],
            }],
        }],
        structural_unit_functions: Vec::new(),
    };
    plan.identity = plan.recomputed_identity();
    plan
}

#[test]
fn fragment_identity_binds_zero_spans_aggregate_bytes_and_provenance() {
    let original = zero_span_plan();
    assert_eq!(original.identity, original.recomputed_identity());

    let mut changed = original.clone();
    changed.functions[0].bytes.push(0x90);
    assert_ne!(changed.recomputed_identity(), original.identity);

    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions[0]
        .provenance
        .values
        .push(ValueId::new(7).unwrap());
    assert_ne!(changed.recomputed_identity(), original.identity);

    let mut changed = original.clone();
    changed.functions[0].blocks[0].instructions.clear();
    assert_ne!(changed.recomputed_identity(), original.identity);
}
