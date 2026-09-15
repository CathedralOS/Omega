//! Fixtures shared by the provider, float and console canaries.

#[path = "providers_float_and_console/console_reader.rs"]
mod console_reader;
#[path = "providers_float_and_console/console_writer.rs"]
mod console_writer;
#[path = "../fixture_rosters/providers_float_and_console.rs"]
pub(super) mod fixture_roster;
#[path = "providers_float_and_console/float_and_width_canaries.rs"]
mod float_and_width_canaries;
#[path = "providers_float_and_console/hosted_byte.rs"]
mod hosted_byte;
#[path = "providers_float_and_console/hosted_exit.rs"]
mod hosted_exit;
#[path = "providers_float_and_console/hosted_process_exit.rs"]
mod hosted_process_exit;
#[path = "providers_float_and_console/hosted_read.rs"]
mod hosted_read;
#[path = "providers_float_and_console/provider_adapters.rs"]
mod provider_adapters;
#[path = "providers_float_and_console/service_forwarding_and_policies.rs"]
mod service_forwarding_and_policies;

use crate::{
    CompileReport, CompileRequest, CompilerOptions, Path, RequestedCompileProduct,
    compile_rooted_backend_canary_without_output_for_target,
    reviewed_repository_fixture_package_inputs,
};
use native_realization as native;

fn checked_adapter_identity(checked: &checked_trees::CheckedTrees, machine_name: &str) -> String {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing checked adapter `{machine_name}`"));
    checked
        .normalized_machine_overload_identity(machine)
        .expect("checked adapter must have an entry overload")
        .identity()
}

fn assert_selected_operator_terminal_call(canary: &Path, label: &str, through_scalar_local: bool) {
    let root_path = canary.join("main.omg");
    let package_inputs =
        reviewed_repository_fixture_package_inputs(&root_path, Some("linux_x86_64"))
            .unwrap_or_else(|diagnostics| {
                panic!("{label} should derive reviewed package inputs: {diagnostics:#?}")
            });
    let mut request = CompileRequest::new(CompilerOptions {
        root_path,
        build_dir: None,
        target_name: Some("linux_x86_64".into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| {
            panic!("{label} should produce a canonical Terminal artifact: {diagnostics:#?}")
        });
    let retained = report
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{label} should retain its Terminal artifact"));
    retained
        .validate()
        .unwrap_or_else(|error| panic!("{label} Terminal artifact should replay: {error}"));
    let module = terminal_codec::decode_module(retained.artifact().semantic_bytes())
        .unwrap_or_else(|error| panic!("{label} Terminal semantics should decode: {error:?}"));
    let proposal = retained
        .native_realization_proposal()
        .unwrap_or_else(|| panic!("{label} should retain its native proposal"));
    let [operator_occurrence] = proposal.checked_boundary_operator_scope().occurrences() else {
        panic!("{label} should retain one exact checked-to-Terminal operator occurrence")
    };
    let [source_free_demand] = proposal.boundary_application_demands().rows() else {
        panic!("{label} should retain one exact source-free boundary application demand")
    };
    assert_eq!(
        source_free_demand.terminal_operation(),
        operator_occurrence.terminal_operation(),
        "{label} source-free demand should retain the exact Terminal occurrence",
    );
    assert!(
        !source_free_demand
            .requirement()
            .declaration()
            .canonical()
            .is_empty()
            && !source_free_demand.requirement().overload().is_empty(),
        "{label} source-free demand should retain exact nominal and overload identity",
    );
    assert_eq!(
        source_free_demand.application(),
        &boundary_applications::BoundaryApplication::Empty,
        "{label} nongeneric operator should retain the canonical empty application",
    );
    let [realization] = proposal.boundary_application_realizations().rows() else {
        panic!("{label} should retain one exact D29 realization companion")
    };
    assert_eq!(
        realization.terminal_operation(),
        source_free_demand.terminal_operation(),
    );
    assert_ne!(realization.selected_plan_digest(), &[0; 32]);
    assert_eq!(
        realization.role(),
        boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody,
    );
    let [coverage] = proposal.boundary_application_coverage().references() else {
        panic!("{label} should publish one reconstructible D29 coverage reference")
    };
    assert_eq!(
        coverage.terminal_operation(),
        source_free_demand.terminal_operation(),
    );
    assert!(
        module.machines.iter().any(|machine| {
            machine.blocks.iter().any(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .any(|(call_index, operation)| {
                        if operation.id != operator_occurrence.terminal_operation() {
                            return false;
                        }
                        let terminal_psi::OperationKind::Call { .. } = &operation.kind else {
                            return false;
                        };
                        let terminal_psi::OperationResult::Scalar(result) = &operation.result
                        else {
                            return false;
                        };
                        let consumed_result = if through_scalar_local {
                            block.operations[call_index + 1..]
                                .iter()
                                .find_map(|dependent| {
                                    let terminal_psi::OperationKind::ExactIntegerAdd {
                                        left, ..
                                    } = dependent.kind
                                    else {
                                        return None;
                                    };
                                    let terminal_psi::OperationResult::Scalar(dependent_result) =
                                        dependent.result
                                    else {
                                        return None;
                                    };
                                    (left == result.id).then_some(dependent_result.id)
                                })
                        } else {
                            Some(result.id)
                        };
                        consumed_result.is_some_and(|consumed_result| {
                            block.operations[call_index + 1..].iter().any(|consumer| {
                                matches!(
                                    &consumer.kind,
                                    terminal_psi::OperationKind::BoundaryCall { arguments, .. }
                                        if arguments == &[consumed_result]
                                )
                            })
                        })
                    })
            })
        }),
        "{label} should pass the selected scalar Call result to its later boundary consumer"
    );
}

fn assert_selected_operator_native_physical_call(
    canary: &Path,
    label: &str,
    expected_role: boundary_applications::BoundaryApplicationRealizationRole,
    expected_nested_scalar_calls: usize,
) -> Vec<CompileReport> {
    let mut reports = Vec::new();
    for target in ["linux_x86_64", "linux_arm64"] {
        let native_report = compile_rooted_backend_canary_without_output_for_target(canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{label} should retain checked-body physical custody for {target}: {diagnostics:#?}"
                )
            });
        let native = native_report
            .retained_native_artifact()
            .unwrap_or_else(|| panic!("{label} should retain its {target} native artifact"));
        native.validate().unwrap_or_else(|error| {
            panic!("{label} {target} native artifact should replay: {error}")
        });
        let physical = native_report
            .require_package_native_physical_evidence()
            .unwrap_or_else(|error| panic!("{label} {target} package gate should accept: {error}"));
        assert!(std::ptr::eq(
            physical,
            native
                .physical_evidence()
                .unwrap_or_else(|| panic!("{label} should retain complete {target} D32 evidence")),
        ));
        let coverage = native
            .boundary_application_coverage()
            .unwrap_or_else(|| panic!("{label} should retain exact D29 coverage custody"));
        let [realization] = coverage.realizations().rows() else {
            panic!("{label} should retain one exact checked-body realization")
        };
        assert_eq!(realization.role(), expected_role);
        let [demand] = coverage.demands().rows() else {
            panic!("{label} should retain one exact checked-body demand")
        };
        match expected_role {
            boundary_applications::BoundaryApplicationRealizationRole::NongenericCheckedBody => {
                assert_eq!(
                    demand.application(),
                    &boundary_applications::BoundaryApplication::Empty,
                );
            }
            boundary_applications::BoundaryApplicationRealizationRole::SpecializedCheckedBody => {
                let [
                    boundary_applications::BoundaryApplicationArgument::Type {
                        binder_ordinal,
                        type_identity,
                    },
                ] = demand.application().arguments()
                else {
                    panic!("{label} should retain one exact type application")
                };
                assert_eq!(*binder_ordinal, 0);
                assert!(type_identity.canonical().contains("i32"));
                let boundary_applications::BoundaryApplicationRealization::SpecializedCheckedBody {
                    realization_template,
                    realization_machine,
                    specialization_commitment,
                    realization_contract_commitment,
                    ..
                } = realization.realization()
                else {
                    panic!("{label} specialized role should retain its closed payload")
                };
                assert_ne!(realization_template, realization_machine);
                assert_ne!(*specialization_commitment, [0; 32]);
                assert_ne!(*realization_contract_commitment, [0; 32]);
            }
            boundary_applications::BoundaryApplicationRealizationRole::ExactCompilerIntrinsic => {
                panic!("{label} checked-body helper cannot accept a compiler intrinsic")
            }
        }
        let operator_children = physical
            .children()
            .iter()
            .filter(|child| {
                matches!(
                    child.parent(),
                    native_realization::PhysicalChildParent::OperatorApplicationCoverage(_)
                )
            })
            .collect::<Vec<_>>();
        let [operator_child] = operator_children.as_slice() else {
            panic!("{label} should retain one exact checked-body operator child")
        };
        assert!(matches!(
            operator_child.occurrence(),
            native_realization::NativePhysicalOccurrence::Operator(_)
        ));
        assert_eq!(
            operator_child.relocation(),
            native_realization::PhysicalRelocationDisposition::ResolvedInternalCall
        );
        assert!(operator_child.machine_span().byte_count() > 0);
        assert_eq!(
            physical.children().len(),
            physical.projection().operator_occurrences().len()
                + physical.projection().boundary_occurrences().len(),
            "{label} {target} physical children must cover the complete identity projection"
        );
        let nested_scalar_calls = native
            .object()
            .functions()
            .iter()
            .map(|function| function.scalar_call_stacks.len())
            .sum::<usize>();
        assert_eq!(
            nested_scalar_calls, expected_nested_scalar_calls,
            "{label} {target} should retain exactly {expected_nested_scalar_calls} scalar calls inside emitted scalar bodies",
        );
        reports.push(native_report);
    }
    reports
}

fn fused_service_field_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedUnitStructuralFieldType {
    for structural_type in &mut checked.facts.flow.terminal_unit_effects.structural_types {
        let checked_trees::CheckedUnitStructuralTypeShape::Record { fields } =
            &mut structural_type.shape
        else {
            continue;
        };
        for field in fields {
            if matches!(
                field.field_type,
                checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
            ) {
                return &mut field.field_type;
            }
        }
    }
    panic!("checked Service fixture has no fused structural field")
}

fn fused_service_parameter_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedUnitStructuralParameterPlan {
    fused_service_parameter_plan_mut(checked)
        .structural_parameters
        .iter_mut()
        .find(|parameter| parameter.fused_service_erasure.is_some())
        .expect("checked Service fixture has no fused structural parameter")
}

fn fused_service_parameter_plan_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut checked_trees::CheckedUnitEffectMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.fused_service_erasure.is_some())
        })
        .expect("checked Service fixture has no fused parameter machine")
}

fn unit_effect_plan_named<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a checked_trees::CheckedUnitEffectMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("checked Service fixture has no `{name}` machine"));
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine.symbol)
        .unwrap_or_else(|| panic!("checked Service fixture has no `{name}` Unit plan"))
}

fn unit_effect_plan_named_mut<'a>(
    checked: &'a mut checked_trees::CheckedTrees,
    name: &str,
) -> &'a mut checked_trees::CheckedUnitEffectMachinePlan {
    let symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("checked Service fixture has no `{name}` machine"))
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == symbol)
        .unwrap_or_else(|| panic!("checked Service fixture has no `{name}` Unit plan"))
}

fn replay_parts(parts: &native::NativeArtifactParts) -> native::NativeArtifactParts {
    let module = terminal_codec::decode_module(parts.psi_artifact.semantic_bytes())
        .expect("replay Terminal semantics");
    let proof = terminal_codec::decode_proof_bundle(parts.psi_artifact.proof_bytes())
        .expect("replay Terminal proof");
    let debug = parts
        .psi_artifact
        .debug_bytes()
        .map(|bytes| terminal_codec::decode_debug_map(&module, bytes).expect("debug map"));
    native::NativeArtifactParts {
        target: parts.target,
        psi_artifact: terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            parts.psi_artifact.optimization(),
            debug.as_ref(),
        )
        .expect("reconstruct canonical Terminal artifact"),
        object: parts.object.clone(),
        image: parts.image.clone(),
        selected_provider_closure_report_identity: parts.selected_provider_closure_report_identity,
        selected_provider_closure_digest: parts.selected_provider_closure_digest,
        selected_provider_plans: parts.selected_provider_plans.clone(),
        provider_executions: parts.provider_executions.clone(),
        terminal_authority_policy_identity: parts.terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity: parts
            .terminal_authority_permission_policy_identity,
        terminal_authority_closure_review: parts.terminal_authority_closure_review.clone(),
        boundary_application_coverage: parts.boundary_application_coverage.clone(),
        physical_evidence_scope: parts.physical_evidence_scope.clone(),
        physical_evidence: parts.physical_evidence.clone(),
    }
}
