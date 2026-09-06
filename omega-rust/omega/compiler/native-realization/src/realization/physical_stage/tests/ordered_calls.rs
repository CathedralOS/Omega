//! Source entry reaches the same physical stages with empty and selected phases.

use super::*;

#[test]
fn source_ordered_calls_reach_executable_publication() {
    let checked = crate::tests::fixtures::checked_source::checked(
        r#"
        machine pick(left: u64, right: u64) -> u64
        requires true
        ensures result == right
        { transition { _ -> right } }
        data Main {}
        machine Main::launch() {
            let first: u64 = pick(7u64, 9u64);
            let second: u64 = pick(first, first);
            let third: u64 = pick(first, second);
        }
    "#,
    );
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .unwrap();
    let artifact =
        terminal_production::produce_terminal_artifact(&checked, "Main::launch").unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    for target_profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
    ] {
        let target = target_profile.native_target();
        let signature =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                target_profile.program_entry_slot(),
                selection.machine,
                selection.machine,
                selection.name.clone(),
                "entry".into(),
                "Main::launch() -> Unit".into(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
                Vec::new(),
            )
            .unwrap();
        for choices in [
            Vec::new(),
            vec![if target.architecture == target::Architecture::X86_64 {
                optimization_core::Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
            } else {
                optimization_core::Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
            }],
        ] {
            let all = optimization_core::OptimizationSelections::new(choices).unwrap();
            let selections = all.project_post_terminal();
            let request = NativeRealizationCoreRequest {
                target,
                profile: &profile,
                terminal_authority_policy: crate::current_terminal_authority_policy(),
                terminal_authority_permission_policy:
                    crate::current_terminal_authority_permission_policy(),
                program_entry: crate::NativeProgramEntrySettlement::new(&signature, None, &[]),
                optimization_selections: selections.selections(),
                selected_provider_plans: &providers,
                external_binding_rows: &[],
                settlements: &[],
                compiler_builtins: &[],
                boundary_application_coverage: None,
                ieee_float_fma: &[],
                native_callbacks: &[],
                callback_thunks: &[],
            };
            let input = lower_realization_input(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
            )
            .unwrap();
            let optimization = lower_realization_optimization_stage(input, &request).unwrap();
            let target_stage =
                lower_realization_target_stage(optimization, None, &[], &request).unwrap();
            let physical = lower_realization_physical_stage(target_stage, &request).unwrap();
            let NativePhysicalStageResult::Optimized(physical) = physical else {
                panic!("ordinary calls must leave the assigned route even with empty selections");
            };
            let (object, _) = emit_optimized_fragments(
                physical.physical,
                OptimizedFragmentPublicationRequest {
                    boundary_application_coverage: None,
                    optimized_plan: &physical.optimized_plan,
                    terminal: physical.terminal,
                    validation: physical.validation,
                    final_unit: physical.final_unit,
                },
            )
            .unwrap();
            assert_eq!(object.entry_function().unit_call_stacks.len(), 3);
            assert!(object.entry_function().unit_scalar_homes.is_empty());
            let image = image_emission::emit_executable_image(&object, 3).unwrap();
            image_emission::validate_executable_image(&object, &image).unwrap();
            let demand = image_emission::derive_stack_demand(&object, object.entry()).unwrap();
            assert!(demand.ceiling_bytes() > 0);
        }
    }
}
