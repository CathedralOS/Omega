use omega_isa_x86_64::x86_64_physical_register_model;
use omega_machine_optimizer::{X86XorZeroInstructionDisposition, X86XorZeroPhysicalWrite};
use omega_physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use omega_register_model::{
    RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    ValidatedPhysicalRegisterModel, validate_physical_register_model,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEncodedEffects, MachineLatencyKnowledge, MachineSizeKnowledge,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, VirtualRegisterId,
};
use psi_core::IntegerValue;

use super::{SelectedFormEncodingState, encode_row};
use crate::SelectedFormMachineDisposition;
use crate::materialization::MaterializationDisposition;

fn fixture() -> (
    ValidatedPhysicalRegisterModel,
    SelectedInstruction,
    PostAllocationMachineInstruction,
    X86XorZeroInstructionDisposition,
) {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let rax = physical.model().view_named("rax").unwrap();
    let rflags = physical.model().view_named("rflags").unwrap();
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
    let disposition = X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
        destination: X86XorZeroPhysicalWrite {
            instruction,
            operand: 0,
            virtual_register,
            class: rax.class,
            view: rax.id,
            storage_units: rax.units.clone(),
            write_units: rax.write_units.clone(),
            write_semantics: rax.write_semantics,
        },
        rflags_units: rflags.units.clone(),
        baseline_byte_count: 10,
        selected_byte_count: 3,
    };
    (physical, selected, machine, disposition)
}

#[test]
fn xor_zero_admission_reconstructs_canonical_bytes_and_transformed_flags() {
    let (physical, selected, machine, disposition) = fixture();
    let row = encode_row(
        omega_target::NativeTarget::linux_x64(),
        &selected,
        &machine,
        &physical,
        SelectedFormMachineDisposition::RetainedV1,
        Some(MaterializationDisposition::X86XorZero(&disposition)),
    )
    .unwrap();
    let SelectedFormEncodingState::Encoded { bytes, footprint } = row.state else {
        panic!("XOR-zero must own selected bytes")
    };
    assert_eq!(bytes, [0x48, 0x31, 0xc0]);
    assert!(footprint.register_reads.is_empty());
    assert_eq!(footprint.register_writes, [machine.operands[0].view]);
    assert_eq!(
        footprint.implicit_clobbers,
        physical.model().view_named("rflags").unwrap().units
    );
}

#[test]
fn xor_zero_admission_rejects_baseline_destination_count_and_flag_corruption() {
    let (physical, selected, machine, disposition) = fixture();
    let mut corruptions = Vec::new();

    let mut wrong_baseline = machine.clone();
    wrong_baseline.alternative.size = MachineSizeKnowledge::ExactBytes(9);
    corruptions.push((wrong_baseline, disposition.clone()));

    let mut wrong_destination = disposition.clone();
    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 { destination, .. } =
        &mut wrong_destination
    else {
        unreachable!()
    };
    destination.view = physical.model().view_named("rbx").unwrap().id;
    corruptions.push((machine.clone(), wrong_destination));

    let mut wrong_count = disposition.clone();
    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
        selected_byte_count,
        ..
    } = &mut wrong_count
    else {
        unreachable!()
    };
    *selected_byte_count = 4;
    corruptions.push((machine.clone(), wrong_count));

    let mut wrong_flags = disposition;
    let X86XorZeroInstructionDisposition::XorZeroMaterializationV1 { rflags_units, .. } =
        &mut wrong_flags
    else {
        unreachable!()
    };
    rflags_units.clear();
    corruptions.push((machine, wrong_flags));

    for (machine, disposition) in corruptions {
        assert!(
            encode_row(
                omega_target::NativeTarget::linux_x64(),
                &selected,
                &machine,
                &physical,
                SelectedFormMachineDisposition::RetainedV1,
                Some(MaterializationDisposition::X86XorZero(&disposition)),
            )
            .is_err()
        );
    }
}
