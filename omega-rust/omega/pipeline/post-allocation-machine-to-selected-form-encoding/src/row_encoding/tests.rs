use isa_x86_64::x86_64_physical_register_model;
use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use register_model::{
    RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEncodedEffects, MachineLatencyKnowledge, MachineSizeKnowledge,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
};
use semantic_vocabulary::IntegerValue;

use super::{SelectedFormEncodingState, encode_row};
use crate::SelectedFormMachineDisposition;

fn fixture() -> (
    ValidatedPhysicalRegisterModel,
    SelectedInstruction,
    PostAllocationMachineInstruction,
) {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap();
    let instruction = SelectedInstructionId(1);
    let virtual_register = VirtualRegisterId(1);
    let selected = SelectedInstruction {
        id: instruction,
        kind: SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0_u8.into()),
        },
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        operands: vec![SelectedOperand {
            operand: 0,
            virtual_register,
            access: RegisterOperandAccess::Def,
            class: rax.class,
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        }],
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
        provenance: SelectedInstructionProvenance::default(),
    };
    let machine = PostAllocationMachineInstruction {
        instruction,
        address: None,
        alternative: MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::MaterializeI64,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(10),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![0]),
        },
        operands: vec![PhysicalOperandFootprint {
            operand: 0,
            virtual_register,
            class: rax.class,
            view: rax.id,
            access: RegisterOperandAccess::Def,
            storage_units: rax.units.clone(),
            read_units: vec![],
            write_units: rax.write_units.clone(),
            write_semantics: Some(rax.write_semantics),
        }],
        implicit_unit_uses: vec![],
        implicit_unit_defs: vec![],
        implicit_unit_clobbers: vec![],
        unit_uses: vec![],
        unit_defs: rax.write_units.clone(),
        unit_clobbers: vec![],
    };
    (physical, selected, machine)
}

#[test]
fn current_machine_encoding_preserves_ordinary_bytes_and_rejects_retired_dispositions() {
    let (physical, selected, machine) = fixture();
    let target = target::NativeTarget::linux_x64();
    let row = encode_row(target, &selected, &machine, &physical, None).unwrap();
    let SelectedFormEncodingState::Encoded { bytes, .. } = &row.state else {
        panic!("ordinary materialization must own bytes")
    };
    assert_eq!(bytes, &[0x48, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0]);
    crate::validation::row::validate(target, &selected, &machine, &physical, &row).unwrap();
    let mut changed = row.clone();
    changed.machine_disposition = SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 {
        consumer: SelectedInstructionId(2),
    };
    assert!(
        crate::validation::row::validate(target, &selected, &machine, &physical, &changed).is_err()
    );
    let mut changed = row;
    let SelectedFormEncodingState::Encoded { bytes, .. } = &mut changed.state else {
        unreachable!()
    };
    bytes[0] ^= 1;
    assert!(
        crate::validation::row::validate(target, &selected, &machine, &physical, &changed).is_err()
    );
}
