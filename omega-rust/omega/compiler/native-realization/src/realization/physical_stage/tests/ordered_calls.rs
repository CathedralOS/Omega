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
        target::TargetProfile::WindowsX64,
        target::TargetProfile::MacosArm64,
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

#[test]
fn terminal_scalar_returning_calls_reach_coordinated_native_artifact() {
    // This signature comes from an actual checked Unit declaration, while the
    // independently authored Terminal body below is not claimed as source output.
    let checked =
        crate::tests::fixtures::checked_source::checked("data Main {} machine Main::launch() {}");
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .unwrap();
    let artifact = scalar_call_artifact();
    let profile = proof_admission::AdmissionProfile::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    for target_profile in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
        target::TargetProfile::WindowsX64,
        target::TargetProfile::MacosArm64,
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
            assert!(!object.entry_function().unit_call_stacks.is_empty());
            assert!(object.entry_function().unit_scalar_homes.is_empty());
            let image = image_emission::emit_executable_image(&object, 3).unwrap();
            image_emission::validate_executable_image(&object, &image).unwrap();
            let demand = image_emission::derive_stack_demand(&object, object.entry()).unwrap();
            assert!(demand.ceiling_bytes() > 0);
            let complete_request = crate::NativeRealizationRequest {
                subsystem: 3,
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
            let replayed_artifact =
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
                    .unwrap();
            let native =
                crate::realize_native_artifact(replayed_artifact, complete_request).unwrap();
            native.validate().unwrap();
            let record = image_emission::build_installation_record(
                native.image(),
                semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
            )
            .unwrap();
            let encoded = image_emission::encode_installation_record(&record).unwrap();
            let decoded = image_emission::decode_installation_record(&encoded).unwrap();
            image_emission::validate_installation_record(&decoded, native.image()).unwrap();
            assert_eq!(
                image_emission::derive_installation_stack_demand(
                    &decoded,
                    native.image(),
                    native.object().entry()
                )
                .unwrap(),
                image_emission::derive_stack_demand(native.object(), native.object().entry())
                    .unwrap(),
            );
        }
    }
}
fn scalar_call_artifact() -> terminal_codec::CanonicalTerminalArtifact {
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerValue, MachineId, OperationId,
        StructuralTypeId, ValueId,
    };
    use terminal_psi::*;
    let (semantic, _) = super::conditional_fixture::artifact(
        super::conditional_fixture::Comparison::Equal,
        IntegerSign::Unsigned,
    );
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let template = module.machines[0].clone();
    let attachment = StructuralTypeId::new(400).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: attachment,
        identity: "test::Main".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let scalar_type = template.parameters[0].scalar_type;
    let declaration = |id| ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type,
    };
    let mut machines = Vec::new();
    for base in [100, 200, 300] {
        let mut machine = template.clone();
        machine.id = MachineId::new(base).unwrap();
        machine.attachment = (base == 100).then_some(attachment);
        machine.contract.id = ContractId::new(base).unwrap();
        machine.parameters = if base == 100 {
            Vec::new()
        } else {
            vec![declaration(base + 1)]
        };
        machine.result = if base == 100 {
            TerminalMachineResult::Unit
        } else {
            TerminalMachineResult::Scalar(declaration(base + 2))
        };
        machine.entry = BlockId::new(base).unwrap();
        let mut operations = Vec::new();
        if base == 100 {
            operations.push(Operation {
                id: OperationId::new(base).unwrap(),
                result: OperationResult::Scalar(declaration(base + 1)),
                kind: OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(37),
                },
            });
        }
        if base != 300 {
            operations.push(Operation {
                id: OperationId::new(base + 1).unwrap(),
                result: OperationResult::Scalar(declaration(base + 3)),
                kind: OperationKind::Call {
                    callee: MachineId::new(base + 100).unwrap(),
                    arguments: vec![ValueId::new(base + 1).unwrap()],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            });
        }
        machine.blocks = vec![Block {
            id: machine.entry,
            parameters: Vec::new(),
            operations,
            terminator: if base == 100 {
                Terminator::ReturnUnit {
                    edge: EdgeId::new(base).unwrap(),
                    trivial_affine_discards: Vec::new(),
                }
            } else {
                Terminator::Return {
                    edge: EdgeId::new(base).unwrap(),
                    value: ValueId::new(base + if base == 300 { 1 } else { 3 }).unwrap(),
                    cleanup_actions: Vec::new(),
                }
            },
        }];
        machines.push(machine);
    }
    module.entry = machines[0].id;
    module.machines = machines;
    let proof = ProofBundle::default();
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module, &proof).unwrap();
    terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &proof, &optimization, None)
        .unwrap()
}
