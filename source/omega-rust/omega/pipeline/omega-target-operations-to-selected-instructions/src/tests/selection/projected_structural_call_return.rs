//! Exact structural selection construction, replay corruption, and identity fences.

use crate::tests::fixtures::{
    microsoft_environment::microsoft_selection_environment,
    projected_structural_call_return::projected_fixture,
};
use crate::{
    legalize_target_operations, select_instructions, selected_instruction_plan_identity,
    validate_selected_instructions,
};
use omega_calling_conventions::{IndirectPointerLocation, ValueLocation};
use omega_selected_instructions::{SelectedStructuralFragmentSite, SelectedStructuralTransfer};
use psi_core::StructuralDomainId;

#[test]
fn exact_projected_structural_selection_replays_and_binds_every_target_fact() {
    let (abstract_plan, target, unit) =
        projected_fixture(omega_target::NativeTarget::windows_x64());
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
    let identity = selected.receipt().identity();
    let closure = &selected.plan().projected_structural_call_returns[0];
    assert_eq!(closure.fragments.len(), 8);
    assert!(matches!(
        closure.callee_return_transfer,
        SelectedStructuralTransfer::FixedViewCopy { .. }
    ));

    let mutations: Vec<Box<dyn Fn(&mut omega_selected_instructions::SelectedInstructionPlan)>> = vec![
        Box::new(|plan| {
            plan.projected_structural_call_returns[0].caller = psi_core::MachineId::new(9).unwrap()
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0].projected_qualifications[0].domain =
                StructuralDomainId::new(9).unwrap()
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0].fragments[0].site =
                SelectedStructuralFragmentSite::CallerArgumentSource
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0].fragments[0]
                .placement
                .shape
                .byte_size = 16
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0].fragments[0]
                .placement
                .locations[0] = ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(
                    omega_target_operations::MachineRegister::X86Rdi,
                ),
                copy_stack_byte_offset: Some(24),
                byte_size: 8,
                alignment: 8,
            }
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0]
                .call
                .argument
                .operand += 1
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0]
                .call
                .clobbers
                .pop();
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0]
                .caller_return
                .value
                .fixed_view = omega_register_model::RegisterViewId(u16::MAX)
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0]
                .callee_return
                .implicit_uses
                .clear()
        }),
        Box::new(|plan| {
            plan.projected_structural_call_returns[0].callee_return_transfer =
                SelectedStructuralTransfer::SameViewNoCopy {
                    register: omega_target_operations::MachineRegister::X86Rdi,
                }
        }),
    ];
    for mutate in mutations {
        let mut corrupted = selected.plan().clone();
        mutate(&mut corrupted);
        assert_ne!(selected_instruction_plan_identity(&corrupted), identity);
        assert!(
            validate_selected_instructions(
                &legalized,
                &constraints,
                &physical,
                &catalog,
                corrupted,
            )
            .is_err()
        );
    }
}

#[test]
fn projected_structural_selection_requires_explicit_call_authority_and_stays_disjoint() {
    let (abstract_plan, target, unit) =
        projected_fixture(omega_target::NativeTarget::windows_x64());
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, mut constraints) = microsoft_selection_environment();
    constraints.projected_structural_call = None;
    assert!(select_instructions(&legalized, &constraints, &physical, &catalog).is_err());

    let (ordinary_source, ordinary_target, ordinary_unit) =
        crate::tests::fixtures::structural_call::structural_call_fixture();
    let ordinary =
        legalize_target_operations(&ordinary_target, &ordinary_source, &ordinary_unit).unwrap();
    let (_, _, ordinary_constraints) = microsoft_selection_environment();
    let selected =
        select_instructions(&ordinary, &ordinary_constraints, &physical, &catalog).unwrap();
    assert!(selected.plan().projected_structural_call_returns.is_empty());
}
