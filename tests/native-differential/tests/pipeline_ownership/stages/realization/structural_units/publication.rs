//! Shared structural calls retain ABI and provider origins through publication.

use crate::tests::*;

#[test]
fn structural_call_publication_preserves_owned_indirect_arguments() {
    for selections in [
        OptimizationSelections::default(),
        OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
    ] {
        let (semantic, proof) = structural_extent_call_unit_artifact();
        let object = stage(&semantic, &proof, selections, &[], &[]);
        let published = publish(&object, &[]);
        assert_eq!(published.text_bytes().len(), 91);
        let [caller, callee] = published.functions() else {
            panic!("one structural caller and one structural callee");
        };
        assert_eq!(caller.internal_unit_calls.len(), 1);
        assert!(callee.internal_unit_calls.is_empty());
        assert_eq!(caller.unit_parameter_homes.len(), 2);
        assert_eq!(callee.unit_parameter_homes.len(), 2);
        assert_eq!(caller.unit_call_stacks.len(), 1);
        assert_eq!(caller.unit_call_stacks[0].active_frame_bytes, 0);
        assert_eq!(caller.unit_call_stacks[0].transient_bytes, 80);
        assert_eq!(caller.unit_call_stacks[0].caller_live_bytes, 80);
        let call = &caller.internal_unit_calls[0];
        assert_eq!(call.target, callee.machine);
        assert_eq!(call.arguments.len(), 2);
        assert_eq!(call.code_offset, 0);
        assert_eq!(call.byte_count, 89);
        for function in published.functions() {
            for (home, register) in function
                .unit_parameter_homes
                .iter()
                .zip([MachineRegister::X86Rcx, MachineRegister::X86Rdx])
            {
                assert_eq!(
                    home.location,
                    machine_code::StructuralSourceLocation::IncomingIndirectPointer { register }
                );
            }
        }
        let mut changed = published.clone();
        changed.functions_mut_for_test()[0].unit_parameter_homes[0].location =
            machine_code::StructuralSourceLocation::IncomingIndirectPointer {
                register: MachineRegister::X86R8,
            };
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
        let mut changed = published.clone();
        changed.functions_mut_for_test()[0].internal_unit_calls[0].arguments[0].source_location =
            machine_code::StructuralSourceLocation::Stack { byte_offset: 0 };
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
        let mut changed = published.clone();
        changed.functions_mut_for_test()[0].internal_unit_calls[0].arguments[0].bytes[0] ^= 1;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
        let mut changed = published.clone();
        changed.functions_mut_for_test()[0].internal_unit_calls[0].arguments[0].call_stack_bytes +=
            8;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
        let mut changed = published.clone();
        changed.functions_mut_for_test()[0].internal_unit_calls[0].target = caller.machine;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
    }
}

#[test]
fn installed_structural_provider_call_reaches_shared_publication() {
    let (semantic, proof, selected) = provider_artifact();
    for selections in [
        OptimizationSelections::default(),
        OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
    ] {
        let object = stage(&semantic, &proof, selections, &selected, &[]);
        let published = publish(&object, &[]);
        let call = &published.functions()[0].internal_unit_calls[0];
        let machine_code::InternalUnitCallSource::InstalledProvider {
            boundary, provider, ..
        } = &call.source
        else {
            panic!("publication must not relabel a provider call as authored CallUnit");
        };
        assert_eq!(*boundary, provider.boundary);
        assert_eq!(call.target, provider.candidate);
        assert_eq!(provider.provider_identity, "StructuralProvider");
        let mut changed = published.clone();
        changed.functions_mut_for_test()[0].internal_unit_calls[0].source =
            machine_code::InternalUnitCallSource::Authored;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
        let mut changed = published.clone();
        let machine_code::InternalUnitCallSource::InstalledProvider { provider, .. } =
            &mut changed.functions_mut_for_test()[0].internal_unit_calls[0].source
        else {
            unreachable!()
        };
        provider.provider_identity.push_str("Changed");
        assert!(
            image_emission::validate_function_fragment_object_artifact(&object, &changed).is_err()
        );
    }
}

#[test]
fn claim_completion_prefixes_publish_as_metadata_without_instruction_spans() {
    let (semantic, proof, boundary) = completion_artifact();
    let execution = omega_native_differential_test::admit_native_provider(
        NativeTarget::uefi_x64(),
        "Extent::complete",
        31_000,
        calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(16, 8)],
            result: None,
        },
    );
    let settlements = [AdmittedBoundarySettlement {
        boundary,
        execution: AdmittedBoundaryExecution::Provider(&execution),
        realization: target_operations::ClaimCompletionOnlyRealization.into(),
    }];
    for selections in [
        OptimizationSelections::default(),
        OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
    ] {
        let source = stage(&semantic, &proof, selections, &[], &settlements);
        let published = publish(&source, &[&execution]);
        assert_eq!(published.text_bytes(), &[0xc3]);
        assert_eq!(published.boundary_settlements().len(), 2);
        for (index, row) in published.boundary_settlements().iter().enumerate() {
            assert_eq!(row.settlement.code_offset, 0);
            assert_eq!(row.settlement.byte_count, 0);
            assert_eq!(row.settlement.operation_ordinal, index);
            assert_eq!(row.settlement.completion_receipts.len(), 1);
        }
        let mut changed = published.clone();
        changed.boundary_settlements_mut_for_test().pop();
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &changed).is_err()
        );
        let mut changed = published.clone();
        changed.boundary_settlements_mut_for_test()[0]
            .settlement
            .completion_receipts[0]
            .argument_index = 1;
        assert!(
            image_emission::validate_function_fragment_object_artifact(&source, &changed).is_err()
        );
        let image = image_emission::emit_executable_image(&published, 10).unwrap();
        assert!(
            image_emission::build_installation_record(
                &image,
                semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
            )
            .is_err(),
            "metadata-only settlement still requires its admitted execution"
        );
    }
}

fn stage(
    semantic: &[u8],
    proof: &[u8],
    selections: OptimizationSelections,
    providers: &[terminal_psi_to_abstract_operations::SelectedProviderAdapter],
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> StagedOptimizedRelocationFreeObjectContainer {
    let optimized = optimize_artifact_sections(
        semantic,
        proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .expect("verified structural source");
    let post_terminal = optimized.selections().project_post_terminal();
    let target = if providers.is_empty() {
        lower_optimized_to_target_operations_with_provider_executions(
            optimized,
            NativeTarget::uefi_x64(),
            settlements,
        )
        .expect("structural target lowering")
    } else {
        let installation =
            terminal_psi_to_abstract_operations::admit_provider_installation_for_optimization(
                optimized.plan(),
                semantic,
                proof,
                &AdmissionProfile::default(),
                providers,
            )
            .expect("independently admitted structural provider");
        lower_optimized_to_target_operations_with_provider_executions_and_installation(
            optimized,
            NativeTarget::uefi_x64(),
            settlements,
            installation,
        )
        .expect("structural target retains provider selection")
    };
    assert!(target_operations_to_selected_instructions::is_fragment_publication_program(&target));
    let physical = stage_optimized_verified_physical_pipeline(target, post_terminal.selections())
        .expect("shared structural physical pipeline");
    let fragments = stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("shared structural fragments");
    let text =
        stage_optimized_relocation_free_text_section(fragments).expect("shared structural text");
    stage_optimized_relocation_free_object_container(text).expect("shared structural object")
}

fn publish(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    executions: &[&dyn installation_evidence::ProviderExecutionEvidence],
) -> image_emission::ObjectArtifact {
    let object = image_emission::build_function_fragment_object_artifact(source)
        .expect("shared structural object publication");
    image_emission::validate_function_fragment_object_artifact(source, &object)
        .expect("independent structural object replay");
    let image = image_emission::emit_executable_image(&object, 10)
        .expect("structural PE image construction, not callable-entry admission");
    image_emission::validate_executable_image(&object, &image).expect("image replay");
    let installation = image_emission::build_installation_record_with_provider_executions(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        executions.iter().copied(),
    )
    .expect("structural installation");
    let bytes = image_emission::encode_installation_record(&installation).unwrap();
    let decoded = image_emission::decode_installation_record(&bytes).unwrap();
    assert_eq!(decoded, installation);
    image_emission::validate_installation_record(&decoded, &image).expect("installation replay");
    let mut changed = decoded.clone();
    changed.functions_mut_for_test()[0].unit_parameter_homes[0].location =
        machine_code::StructuralSourceLocation::Stack { byte_offset: 0 };
    assert_installation_rejects(&changed, &image);
    if !decoded.internal_unit_calls().is_empty() {
        let mut changed = decoded.clone();
        changed.internal_unit_calls_mut_for_test()[0]
            .custody
            .arguments[0]
            .call_stack_bytes += 8;
        assert_installation_rejects(&changed, &image);
        if let machine_code::InternalUnitCallSource::InstalledProvider { .. } =
            &decoded.internal_unit_calls()[0].custody.source
        {
            let mut changed = decoded.clone();
            changed.internal_unit_calls_mut_for_test()[0].custody.source =
                machine_code::InternalUnitCallSource::Authored;
            // Still a well-shaped authored call; only comparison with the
            // admitted image can reject this change of semantic origin.
            let encoded = image_emission::encode_installation_record(&changed).unwrap();
            let changed = image_emission::decode_installation_record(&encoded).unwrap();
            assert!(image_emission::validate_installation_record(&changed, &image).is_err());
        }
    }
    if !decoded.boundary_settlements().is_empty() {
        let mut changed = decoded.clone();
        changed.boundary_settlements_mut_for_test()[0]
            .settlement
            .completion_receipts[0]
            .argument_index = 1;
        assert_installation_rejects(&changed, &image);
    }
    object
}

fn assert_installation_rejects(
    changed: &image_emission::InstallationRecord,
    image: &image_emission::ExecutableImage,
) {
    assert!(image_emission::validate_installation_record(changed, image).is_err());
    if let Ok(encoded) = image_emission::encode_installation_record(changed) {
        let decoded = image_emission::decode_installation_record(&encoded).unwrap();
        assert!(image_emission::validate_installation_record(&decoded, image).is_err());
    }
}

fn completion_artifact() -> (Vec<u8>, Vec<u8>, semantic_vocabulary::BoundaryMachineId) {
    let (semantic, proof) = structural_extent_unit_leaf_artifact();
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let machine = &mut module.machines[0];
    for parameter in &mut machine.structural_parameters {
        parameter.multiplicity = StructuralMultiplicity::Linear;
    }
    machine.entry_claims = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| terminal_psi::EntryClaim {
            claim: semantic_vocabulary::ClaimId::new(index as u64 + 1).unwrap(),
            input: parameter.place,
            path: Vec::new(),
        })
        .collect();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(3_630).unwrap();
    let mut boundary_parameter = machine.structural_parameters[0].clone();
    boundary_parameter.place = PlaceId::new(3_631).unwrap();
    module
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: "Extent::complete".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![boundary_parameter],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    machine.blocks[0].operations = machine
        .entry_claims
        .iter()
        .enumerate()
        .map(|(index, claim)| Operation {
            id: OperationId::new(3_632 + index as u64).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: vec![terminal_psi::StructuralArgument {
                    place: claim.input,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }],
                completion_receipts: vec![terminal_psi::CompletionReceipt {
                    claim: claim.claim,
                    argument_index: 0,
                }],
            },
        })
        .collect();
    (
        terminal_codec::encode_module(&module).unwrap(),
        proof,
        boundary,
    )
}

fn provider_artifact() -> (
    Vec<u8>,
    Vec<u8>,
    Vec<terminal_psi_to_abstract_operations::SelectedProviderAdapter>,
) {
    use terminal_psi::{
        BoundaryMachineDeclaration, ProviderCandidateConformance, ProviderParameterRefinement,
        ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
    };
    let (semantic, proof) = structural_extent_call_unit_artifact();
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(3_620).unwrap();
    let provider_type = StructuralTypeId::new(3_621).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: provider_type,
        identity: "named(name(StructuralProvider))".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module.machines[1].attachment = Some(provider_type);
    let parameters = module.machines[0].structural_parameters.clone();
    let mut boundary_parameters = parameters.clone();
    for (index, parameter) in boundary_parameters.iter_mut().enumerate() {
        parameter.place = PlaceId::new(3_622 + index as u64).unwrap();
    }
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: boundary,
        identity: "StructuralSink::accept".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: boundary_parameters,
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary,
            requirement_identity: "StructuralSink::accept".into(),
            provider_identity: "StructuralProvider".into(),
            candidate_identity: "StructuralProvider::accept".into(),
            candidate: module.machines[1].id,
            signature: ProviderSignature {
                parameters: parameters
                    .iter()
                    .map(|parameter| ProviderSignatureParameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        access: parameter.access,
                        qualifications: parameter.qualifications.clone(),
                        projected_qualifications: parameter.projected_qualifications.clone(),
                    })
                    .collect(),
            },
            refinement: ProviderRefinement {
                positional_parameters: (0..2)
                    .map(|index| ProviderParameterRefinement {
                        boundary_index: index,
                        candidate_index: index,
                    })
                    .collect(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module.machines[0].blocks[0].operations[0].kind = OperationKind::BoundaryCall {
        boundary,
        arguments: Vec::new(),
        structural_arguments: parameters
            .iter()
            .map(|parameter| terminal_psi::StructuralArgument {
                place: parameter.place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            })
            .collect(),
        completion_receipts: Vec::new(),
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        proof,
        vec![
            terminal_psi_to_abstract_operations::SelectedProviderAdapter {
                requirement_identity: "StructuralSink::accept".into(),
                provider_identity: "StructuralProvider".into(),
                machine_identity: "StructuralProvider::accept".into(),
            },
        ],
    )
}
