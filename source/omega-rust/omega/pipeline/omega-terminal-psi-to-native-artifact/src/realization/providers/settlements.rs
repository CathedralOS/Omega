use std::collections::BTreeSet;

use crate::realization::model::{
    NativeBoundaryRealization, NativeRealizationInput, NativeRealizationRequest,
};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_native_artifact::NativeProviderExecution;
use omega_target_operations::NormalizedForeignCallBinding;
use psi_diagnostics::Diagnostic;

fn selected_plan_from_exact_evidence<'facts>(
    selected: &'facts omega_effects::SelectedProviderPlanFacts,
    report_identity: u64,
    exact_plan: &omega_effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> Result<&'facts omega_effects::provider_plan::ProviderPlan, Vec<Diagnostic>> {
    selected
        .plan_by_exact_evidence(report_identity, exact_plan)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` does not carry exact evidence for selected plan {report_identity:#018x}"
            ))]
        })
}

fn rejoin_normalized_foreign_call(
    selected_plan: &omega_effects::provider_plan::ProviderPlan,
    external_binding_rows: &[omega_calling_conventions::ExternalBindingRow],
    same_stack: &omega_task_plans::AdmittedSameStackContribution,
    provider_plan_report_identity: u64,
    requirement: &str,
    target: omega_target::NativeTarget,
) -> Result<NormalizedForeignCallBinding, Vec<Diagnostic>> {
    let selected_commitment = omega_task_plans::SameStackProviderPlanCommitment::from_digest(
        *selected_plan.identity_digest().as_bytes(),
    );
    if same_stack.provider_plan_report_identity() != provider_plan_report_identity
        || same_stack.provider_plan_commitment() != selected_commitment
        || same_stack.requirement_identity() != requirement
    {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not carry same-stack custody for the exact selected provider row"
        ))]);
    }
    let selected_rows = selected_plan
        .rows
        .iter()
        .filter(|row| row.requirement_identity == requirement)
        .collect::<Vec<_>>();
    let [selected_row] = selected_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` resolves to {} selected provider rows",
            selected_rows.len()
        ))]);
    };
    let external_rows = external_binding_rows
        .iter()
        .filter(|row| row.requirement_identity == requirement)
        .collect::<Vec<_>>();
    let [external] = external_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` resolves to {} retained external-binding rows",
            external_rows.len()
        ))]);
    };
    let (
        omega_effects::provider_plan::ProviderBinding::Import {
            locator: selected_locator,
        },
        omega_calling_conventions::ExternalBindingKind::Import {
            locator: retained_locator,
        },
        Some(boundary_entry_plan),
    ) = (
        &selected_row.binding,
        &external.binding,
        &external.boundary_entry_plan,
    )
    else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not rejoin an evaluated import row with a calling plan"
        ))]);
    };
    if selected_locator != retained_locator || retained_locator.target().native_target() != target {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not match the exact selected locator and native target"
        ))]);
    }
    Ok(NormalizedForeignCallBinding {
        locator: retained_locator.clone(),
        boundary_entry_plan: boundary_entry_plan.clone(),
        same_stack_contribution: same_stack.clone(),
    })
}

pub(crate) fn settle_provider_executions<'request>(
    input: &NativeRealizationInput,
    request: &NativeRealizationRequest<'request>,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<NativeProviderExecution>,
    ),
    Vec<Diagnostic>,
> {
    let mut seen_requirements = BTreeSet::new();
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());
    for settlement in request.settlements {
        let evidence = settlement.provider_execution;
        let requirement = evidence.requirement_identity();
        if !seen_requirements.insert(requirement.to_owned()) {
            return Err(vec![Diagnostic::error(format!(
                "native realization received more than one provider execution for requirement `{requirement}`"
            ))]);
        }
        let selected_plan = selected_plan_from_exact_evidence(
            request.selected_provider_plans,
            evidence.provider_plan_report_identity(),
            settlement.provider_plan,
            requirement,
        )?;
        if !selected_plan
            .rows
            .iter()
            .any(|row| row.requirement_identity == requirement)
        {
            return Err(vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` is absent from selected plan `{}`",
                selected_plan.name
            ))]);
        }
        let realization = match settlement.realization {
            NativeBoundaryRealization::NormalizedForeignCall(same_stack) => {
                omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                    rejoin_normalized_foreign_call(
                        selected_plan,
                        request.external_binding_rows,
                        same_stack,
                        evidence.provider_plan_report_identity(),
                        requirement,
                        request.target,
                    )?,
                )
            }
            NativeBoundaryRealization::Builtin(realization) => {
                omega_target_operations::BoundarySettlementRealization::Builtin(realization)
            }
        };
        let matching_boundaries = input
            .plan()
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.identity == requirement)
            .collect::<Vec<_>>();
        let [boundary] = matching_boundaries.as_slice() else {
            return Err(vec![Diagnostic::error(match matching_boundaries.len() {
                0 => format!("native provider execution cites absent requirement `{requirement}`"),
                count => format!(
                    "native requirement `{requirement}` resolves to {count} boundary declarations"
                ),
            })]);
        };
        admitted.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            provider_execution: evidence,
            realization,
        });
        provider_executions.push(NativeProviderExecution::from_evidence(evidence));
    }
    provider_executions.sort_by(|left, right| {
        (
            left.requirement_identity(),
            left.provider_plan_report_identity(),
            left.provider_execution_report_identity(),
        )
            .cmp(&(
                right.requirement_identity(),
                right.provider_plan_report_identity(),
                right.provider_execution_report_identity(),
            ))
    });
    Ok((admitted, provider_executions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };

    fn import_plan(symbol: &[u8], profile: omega_target::TargetProfile) -> ProviderPlan {
        let requirement = "omega::test::Foreign::leaf()";
        ProviderPlan {
            name: "omega::test::foreign-provider".into(),
            provider_type: String::new(),
            provider_type_package_identity: None,
            target: match profile {
                omega_target::TargetProfile::LinuxX64 => "linux_x64",
                omega_target::TargetProfile::LinuxArm64 => "linux_arm64",
                _ => unreachable!(),
            }
            .into(),
            schema: ServiceSchema {
                trait_name: "omega::test::Foreign".into(),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: "leaf".into(),
                    requirement_owner: "omega::test::Foreign".into(),
                    requirement_owner_package_identity: None,
                    requirement_identity: requirement.into(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["omega::test::Foreign".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "leaf".into(),
                requirement_identity: requirement.into(),
                binding: ProviderBinding::Import {
                    locator: omega_target::normalize_foreign_locator(
                        omega_target::ForeignLocatorCandidate::ElfVersioned {
                            object: b"libomega-test.so".to_vec(),
                            symbol: symbol.to_vec(),
                            version: b"OMEGA_TEST_1".to_vec(),
                        },
                        profile,
                    )
                    .unwrap(),
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }

    #[test]
    fn compact_report_claim_cannot_substitute_different_exact_import_plan() {
        let selected_plan = import_plan(b"selected_leaf", omega_target::TargetProfile::LinuxX64);
        let substituted = import_plan(b"substituted_leaf", omega_target::TargetProfile::LinuxX64);
        let report_identity = selected_plan.report_fingerprint();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![
            selected_plan.clone(),
        ])
        .unwrap();
        assert!(
            selected_plan_from_exact_evidence(
                &selected,
                report_identity,
                &substituted,
                "omega::test::Foreign::leaf()",
            )
            .is_err()
        );
        assert_eq!(
            selected_plan_from_exact_evidence(
                &selected,
                report_identity,
                &selected_plan,
                "omega::test::Foreign::leaf()",
            )
            .unwrap(),
            &selected_plan
        );
    }

    #[derive(Debug)]
    struct TestProviderExecution {
        requirement: String,
        provider_plan_report_identity: u64,
    }

    impl omega_installation_evidence::ProviderExecutionEvidence for TestProviderExecution {
        fn requirement_identity(&self) -> &str {
            &self.requirement
        }

        fn provider_plan_report_identity(&self) -> u64 {
            self.provider_plan_report_identity
        }

        fn provider_execution_report_identity(&self) -> u64 {
            802
        }

        fn provider_execution_report_fingerprint(&self) -> u64 {
            803
        }

        fn normalized_root_report_identity(&self) -> u64 {
            804
        }

        fn boundary_contract_report_fingerprint(&self) -> u64 {
            805
        }
    }

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
                ProviderBinding::Import { locator } => locator.clone(),
                _ => unreachable!(),
            };
            let boundary_entry_plan =
                omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
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
            let target_plan = omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
                &abstract_plan,
                target,
                &[omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement {
                    boundary,
                    provider_execution: &evidence,
                    realization: omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(foreign),
                }],
            )
            .unwrap();
            let assigned = omega_target_operations_to_assigned_target_operations::assign_registers(
                &target_plan,
            )
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
                    omega_target::TargetProfile::LinuxX64 => {
                        b"/lib64/ld-linux-x86-64.so.2".to_vec()
                    }
                    omega_target::TargetProfile::LinuxArm64 => {
                        b"/lib/ld-linux-aarch64.so.1".to_vec()
                    }
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

    #[test]
    fn exact_rejoined_integer_literal_import_reaches_dynamic_elf_on_both_targets() {
        let requirement = "omega::test::Foreign::leaf()";
        let machine = psi_core::MachineId::new(810).unwrap();
        let block = psi_core::BlockId::new(810).unwrap();
        let boundary = psi_core::BoundaryMachineId::new(810).unwrap();
        let constant_operation = psi_core::OperationId::new(810).unwrap();
        let call_operation = psi_core::OperationId::new(811).unwrap();
        let return_edge = psi_core::EdgeId::new(810).unwrap();
        let value = psi_core::ValueId::new(810).unwrap();
        let integer_type = psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32).unwrap();
        let scalar_type = psi_core::ScalarType::Integer(integer_type);
        let immediate = psi_core::IntegerValue::Signed(37);
        let psi = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x81; 32]),
        };
        let abstract_plan = omega_abstract_operations::AbstractOperationPlan {
            psi,
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
                id: boundary,
                identity: requirement.into(),
                attachment: None,
                scalar_parameters: vec![scalar_type],
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
                    omega_abstract_operations::AbstractOperation::IntegerConstant {
                        psi_operation: constant_operation,
                        result: value,
                        scalar_type,
                        value: immediate,
                    },
                    omega_abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation: call_operation,
                        result: None,
                        boundary,
                        arguments: vec![value],
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
            let mut selected_plan = import_plan(b"selected_integer_leaf", profile);
            selected_plan.schema.methods[0].parameter_count = 1;
            selected_plan.schema.methods[0].parameter_type_identities = vec!["i32".into()];
            let report_identity = selected_plan.report_fingerprint();
            let locator = match &selected_plan.rows[0].binding {
                ProviderBinding::Import { locator } => locator.clone(),
                _ => unreachable!(),
            };
            let boundary_entry_plan =
                omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
                    omega_calling_conventions::CallingPolicy::native_for_target(target),
                    &omega_calling_conventions::CallSignature {
                        parameters: vec![omega_calling_conventions::ValueShape::integer(4, 4)],
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
                    receipt: omega_task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(811).unwrap(),
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
            let target_plan = omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
                &abstract_plan,
                target,
                &[omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement {
                    boundary,
                    provider_execution: &evidence,
                    realization: omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(foreign),
                }],
            )
            .unwrap();
            let omega_target_operations::TargetOperation::UnitBody(target_body) =
                &target_plan.functions[0].operation
            else {
                panic!("literal import lowers as a Unit body")
            };
            let omega_target_operations::TargetUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                ..
            } = &target_body.operations[1]
            else {
                panic!("literal import remains a normalized foreign call")
            };
            let [target_argument] = scalar_arguments.as_slice() else {
                panic!("one target argument")
            };
            assert_eq!(target_argument.source_value, value);
            assert_eq!(target_argument.scalar_type, integer_type);
            assert_eq!(target_argument.immediate, immediate);
            assert_eq!(target_argument.parameter_index, 0);

            let assigned = omega_target_operations_to_assigned_target_operations::assign_registers(
                &target_plan,
            )
            .unwrap();
            let omega_assigned_target_operations::AssignedOperation::UnitBody(assigned_body) =
                &assigned.functions[0].operation
            else {
                panic!("literal import assigns as a Unit body")
            };
            let omega_assigned_target_operations::AssignedUnitOperation::NormalizedForeignCall {
                scalar_arguments,
                ..
            } = &assigned_body.operations[1]
            else {
                panic!("assigned literal import remains a normalized foreign call")
            };
            assert_eq!(scalar_arguments, std::slice::from_ref(target_argument));

            let machine_code = omega_machine_emission::emit_machine_code(&assigned).unwrap();
            let [call] = machine_code.functions[0].foreign_calls.as_slice() else {
                panic!("one retained foreign call")
            };
            let [argument] = call.scalar_arguments.as_slice() else {
                panic!("one retained machine argument")
            };
            assert_eq!(call.locator, locator);
            assert_eq!(call.call_plan, boundary_entry_plan.call);
            assert_eq!(argument.source_value, value);
            assert_eq!(argument.scalar_type, integer_type);
            assert_eq!(argument.immediate, immediate);
            assert_eq!(argument.placement, target_argument.placement);
            assert!(argument.byte_count > 0);

            let mut changed_value = machine_code.clone();
            changed_value.functions[0].foreign_calls[0].scalar_arguments[0].immediate =
                psi_core::IntegerValue::Signed(38);
            assert!(omega_image_emission::build_object_artifact(&changed_value).is_err());
            let mut changed_carrier = machine_code.clone();
            changed_carrier.functions[0].foreign_calls[0].scalar_arguments[0].scalar_type =
                psi_core::IntegerType::address(32).unwrap();
            changed_carrier.functions[0].foreign_calls[0].scalar_arguments[0].immediate =
                psi_core::IntegerValue::Unsigned(37);
            assert!(omega_image_emission::build_object_artifact(&changed_carrier).is_err());
            let mut changed_bytes = machine_code.clone();
            changed_bytes.functions[0].bytes[argument.code_offset] ^= 1;
            assert!(omega_image_emission::build_object_artifact(&changed_bytes).is_err());
            let mut stripped_custody = machine_code.clone();
            stripped_custody.functions[0].foreign_calls[0]
                .scalar_arguments
                .clear();
            assert!(omega_image_emission::build_object_artifact(&stripped_custody).is_err());

            let object = omega_image_emission::build_object_artifact(&machine_code).unwrap();
            assert_eq!(object.object().layout.normalized_imports.len(), 1);
            assert_eq!(object.relocations().record_count(), 1);
            let interpreter = omega_target::normalize_elf_interpreter_plan(
                match profile {
                    omega_target::TargetProfile::LinuxX64 => {
                        b"/lib64/ld-linux-x86-64.so.2".to_vec()
                    }
                    omega_target::TargetProfile::LinuxArm64 => {
                        b"/lib/ld-linux-aarch64.so.1".to_vec()
                    }
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
}
