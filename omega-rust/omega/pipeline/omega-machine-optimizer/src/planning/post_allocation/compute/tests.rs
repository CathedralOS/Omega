//! Exact alternative-selection fixtures.

use omega_register_model::{
    RegisterClassId, RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel,
    validate_physical_register_model,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineLatencyKnowledge, MachineSizeKnowledge, VirtualRegisterId,
};

use crate::PhysicalOperandFootprint;

use super::alternative::choose;

fn physical() -> ValidatedPhysicalRegisterModel {
    validate_physical_register_model(omega_isa_x86_64::x86_64_physical_register_model()).unwrap()
}

fn view(physical: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
    physical.model().view_named(name).unwrap().id
}

fn operand(number: u16, view: RegisterViewId) -> PhysicalOperandFootprint {
    PhysicalOperandFootprint {
        operand: number,
        virtual_register: VirtualRegisterId(u32::from(number)),
        class: RegisterClassId(0),
        view,
        access: if number == 2 {
            RegisterOperandAccess::Def
        } else {
            RegisterOperandAccess::Use
        },
        storage_units: Vec::new(),
        read_units: Vec::new(),
        write_units: Vec::new(),
        write_semantics: None,
    }
}

fn alternative(variant: u32, applicability: MachineAlternativeApplicability) -> MachineAlternative {
    MachineAlternative {
        key: MachineAlternativeKey {
            family: MachineAlternativeFamily::ExactSubtractI64,
            variant,
        },
        applicability,
        size: MachineSizeKnowledge::ExactBytes(3),
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: omega_selected_instructions::MachineEncodedEffects::fallthrough_v1(
            vec![0, 1],
            vec![2],
        ),
    }
}

fn subtract_alternatives() -> Vec<MachineAlternative> {
    vec![
        alternative(
            0,
            MachineAlternativeApplicability::ResultAliasesOperands {
                result: 2,
                left: 0,
                right: 1,
            },
        ),
        alternative(
            1,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 0,
                distinct_operand: 1,
            },
        ),
        alternative(
            2,
            MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
                result: 2,
                aliased_operand: 1,
                distinct_operand: 0,
            },
        ),
        alternative(
            3,
            MachineAlternativeApplicability::ResultDistinctFromOperands {
                result: 2,
                left: 0,
                right: 1,
            },
        ),
    ]
}

#[test]
fn x86_subtract_home_partition_selects_each_unique_variant() {
    let physical = physical();
    let rax = view(&physical, "rax");
    let rbx = view(&physical, "rbx");
    let rcx = view(&physical, "rcx");
    let alternatives = subtract_alternatives();
    for (views, expected) in [
        ([rax, rax, rax], 0),
        ([rax, rbx, rax], 1),
        ([rbx, rax, rax], 2),
        ([rax, rbx, rcx], 3),
    ] {
        let operands = views
            .into_iter()
            .enumerate()
            .map(|(number, view)| operand(u16::try_from(number).unwrap(), view))
            .collect::<Vec<_>>();
        assert_eq!(
            choose(7, &operands, &alternatives, &physical)
                .unwrap()
                .key
                .variant,
            expected
        );
    }
}

#[test]
fn x86_lea_add_accepts_r12_as_a_rex_extended_sib_index() {
    let physical = physical();
    let rax = view(&physical, "rax");
    let r12 = view(&physical, "r12");
    let add = MachineAlternative {
        key: MachineAlternativeKey {
            family: MachineAlternativeFamily::ExactAddI64,
            variant: 0,
        },
        applicability: MachineAlternativeApplicability::Always,
        size: MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 4,
            maximum_bytes: Some(5),
        },
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded: omega_selected_instructions::MachineEncodedEffects::fallthrough_v1(
            vec![0, 1],
            vec![2],
        ),
    };
    let operands = [operand(0, r12), operand(1, r12), operand(2, rax)];
    assert_eq!(
        choose(8, &operands, std::slice::from_ref(&add), &physical)
            .unwrap()
            .key,
        add.key
    );
}
