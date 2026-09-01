use super::*;
use crate::realization::providers::settlements::validate_source_evaluated_import_coverage;
use crate::{NativeBoundaryRealization, NativeProviderSettlement};

#[test]
fn exact_rejoined_import_reaches_machine_object_and_dynamic_elf_on_both_targets() {
    let requirement = "omega::test::Foreign::leaf()";
    let machine = psi_core::MachineId::new(800).unwrap();
    let block = psi_core::BlockId::new(800).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(800).unwrap();
    let operation = psi_core::OperationId::new(800).unwrap();
    let return_edge = psi_core::EdgeId::new(800).unwrap();
    let psi = psi_terminal::TerminalPsiIdentity {
        vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x80; 32]),
    };
    let abstract_plan = omega_abstract_operations::AbstractOperationPlan {
        psi,
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: requirement.into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![omega_abstract_operations::AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: omega_abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                omega_abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: operation,
                    result: None,
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                omega_abstract_operations::AbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };

    for profile in [
        omega_target::TargetProfile::LinuxX64,
        omega_target::TargetProfile::LinuxArm64,
    ] {
        let target = profile.native_target();
        let selected_plan = import_plan(b"selected_leaf", profile);
        let report_identity = selected_plan.report_fingerprint();
        let locator = match &selected_plan.rows[0].binding {
            ProviderBinding::Import { evaluated } => evaluated.locator().clone(),
            _ => unreachable!(),
        };
        let boundary_entry_plan = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            &omega_calling_conventions::CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .unwrap()
        .plan()
        .clone();
        let external = omega_calling_conventions::ExternalBindingRow {
            target_name: selected_plan.target.clone(),
            trait_name: selected_plan.schema.trait_name.clone(),
            method: "leaf".into(),
            requirement_identity: requirement.into(),
            table_type: String::new(),
            boundary_entry_plan: Some(boundary_entry_plan.clone()),
            binding: omega_calling_conventions::ExternalBindingKind::Import {
                locator: locator.clone(),
            },
        };
        let provider_plan_commitment =
            omega_task_plans::SameStackProviderPlanCommitment::from_digest(
                *selected_plan.identity_digest().as_bytes(),
            );
        let same_stack = omega_task_plans::admit_same_stack_contribution(
            omega_task_plans::SameStackContributionAdmissionCandidate {
                provider_plan_report_identity: report_identity,
                provider_plan_commitment,
                requirement_identity: requirement.into(),
                receipt: omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(801).unwrap(),
                bytes: 64,
                alignment: 16,
            },
            report_identity,
            provider_plan_commitment,
            requirement,
        )
        .unwrap();
        let foreign = rejoin_normalized_foreign_call(
            &selected_plan,
            &[external],
            &same_stack,
            report_identity,
            requirement,
            target,
        )
        .unwrap();
        let evidence = TestProviderExecution {
            requirement: requirement.into(),
            provider_plan_report_identity: report_identity,
        };
        let selected_plans = omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![
            selected_plan.clone(),
        ])
        .unwrap();
        let exact_plan = &selected_plans.plans()[0];
        let normalized_settlement = NativeProviderSettlement {
            provider_execution: &evidence,
            provider_plan: exact_plan,
            realization: NativeBoundaryRealization::NormalizedForeignCall(&same_stack),
        };
        validate_source_evaluated_import_coverage(
            &abstract_plan,
            &selected_plans,
            &[normalized_settlement],
        )
        .expect("the demanded evaluated import has one normalized settlement");
        assert!(
            validate_source_evaluated_import_coverage(&abstract_plan, &selected_plans, &[],)
                .is_err(),
            "a demanded evaluated import cannot omit its settlement",
        );
        let builtin_substitution = NativeProviderSettlement {
            provider_execution: &evidence,
            provider_plan: exact_plan,
            realization: NativeBoundaryRealization::Builtin(
                omega_target_operations::LinuxExitGroupI32Realization.into(),
            ),
        };
        assert!(
            validate_source_evaluated_import_coverage(
                &abstract_plan,
                &selected_plans,
                &[builtin_substitution],
            )
            .is_err(),
            "a demanded evaluated import cannot fall back to a builtin realization",
        );
        let target_plan = omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
            &abstract_plan,
            target,
            &[omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement {
                boundary,
                execution: omega_abstract_operations_to_target_operations::AdmittedBoundaryExecution::Provider(&evidence),
                realization: omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(foreign),
            }],
        )
        .unwrap();
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .unwrap();
        let machine_code = omega_machine_emission::emit_machine_code(&assigned).unwrap();
        let [call] = machine_code.functions[0].foreign_calls.as_slice() else {
            panic!("one retained foreign call")
        };
        assert_eq!(call.locator, locator);
        assert_eq!(call.call_plan, boundary_entry_plan.call);
        assert_eq!(call.same_stack_contribution, same_stack);
        let object = omega_image_emission::build_object_artifact(&machine_code).unwrap();
        assert_eq!(object.object().layout.normalized_imports.len(), 1);
        assert_eq!(object.relocations().record_count(), 1);
        let interpreter = omega_target::normalize_elf_interpreter_plan(
            match profile {
                omega_target::TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".to_vec(),
                omega_target::TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".to_vec(),
                _ => unreachable!(),
            },
            profile,
        )
        .unwrap();
        let image = omega_image_emission::emit_dynamic_elf_image(&object, interpreter).unwrap();
        assert_eq!(image.output().final_image_imports, 1);
        assert_eq!(image.output().final_image_relocations, 1);
    }
}
