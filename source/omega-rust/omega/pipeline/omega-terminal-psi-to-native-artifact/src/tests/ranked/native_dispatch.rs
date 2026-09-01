//! Ranked countdown machine bytes, semantic attribution, image custody, and corruption rejection.

use crate::tests::fixtures::checked_source::checked;

const RANKED_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

#[test]
fn ranked_native_dispatch_emits_exact_machine_body_and_semantic_code_attribution() {
    let checked = checked(RANKED_COUNTDOWN_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower ranked Terminal Psi");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode ranked semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode ranked proof");
    let admitted =
        omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("dispatch ranked native custody");
    let omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(ranked) =
        admitted
    else {
        panic!("ranked module must not enter ordinary native lowering")
    };

    for target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                &ranked, target,
            )
            .expect("lower ranked target operations");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("assign ranked register");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("emit ranked countdown machine code");
        let function = &emitted.functions[0];
        let record = function
            .ranked_u32_countdown
            .as_ref()
            .expect("retain ranked machine custody");
        assert_eq!(record.custody, ranked.countdown);
        assert_eq!(
            function.bytes,
            match target.architecture {
                omega_target::Architecture::X86_64 => {
                    omega_isa_x86_64::encode_ranked_u32_countdown_in_edi().to_vec()
                }
                omega_target::Architecture::Aarch64 => vec![
                    0x01, 0x00, 0x00, 0x14, 0x1f, 0x00, 0x00, 0x71, 0x60, 0x00, 0x00, 0x54, 0x00,
                    0x04, 0x00, 0x51, 0xfd, 0xff, 0xff, 0x17, 0xc0, 0x03, 0x5f, 0xd6,
                ],
            }
        );
        let graph = record.custody.graph;
        let covered = &record.custody.ranked_scc.covered_cyclic_edges[0];
        let psi_terminal::TerminalRankedGuard::UnsignedParameterPositive {
            edge: guard_edge, ..
        } = covered.guard;
        assert_eq!(
            function
                .semantic_code_attribution
                .iter()
                .map(|row| row.site)
                .collect::<Vec<_>>(),
            vec![
                omega_machine_code::SemanticCodeSite::Edge(graph.preheader_edge),
                omega_machine_code::SemanticCodeSite::Operation(graph.zero_operation),
                omega_machine_code::SemanticCodeSite::Operation(graph.compare_operation),
                omega_machine_code::SemanticCodeSite::Edge(guard_edge),
                omega_machine_code::SemanticCodeSite::Operation(graph.one_operation),
                omega_machine_code::SemanticCodeSite::Operation(graph.subtract_operation),
                omega_machine_code::SemanticCodeSite::Edge(covered.edge),
                omega_machine_code::SemanticCodeSite::Edge(graph.false_exit_edge),
                omega_machine_code::SemanticCodeSite::Edge(graph.return_edge),
            ]
        );
        assert_eq!(
            record.custody.fixed_fuel.ceiling_units(),
            5 + 6 * u64::from(u32::MAX)
        );

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("independently replay ranked machine code into object custody");
        let again = omega_image_emission::build_object_artifact(&emitted)
            .expect("ranked object replay is deterministic");
        assert_eq!(object, again);
        assert_eq!(object.functions().len(), 1);
        assert_eq!(object.functions()[0].bytes(&object), function.bytes);
        assert_eq!(
            object.functions()[0]
                .ranked_u32_countdown
                .as_ref()
                .expect("object retains ranked custody"),
            record
        );
        assert_eq!(object.relocations().record_count(), 0);
        assert_eq!(object.semantic_code_attribution().len(), 9);
        assert!(
            object
                .semantic_code_attribution()
                .iter()
                .zip(&function.semantic_code_attribution)
                .all(|(object, machine)| {
                    object.machine == function.machine
                        && object.attribution == *machine
                        && object.text_offset == machine.code_offset
                })
        );
        let container = omega_image_emission::emit_object_container(&object);
        assert_eq!(container.output.text_bytes, function.bytes.len());
        assert_eq!(container.output.relocations, 0);
        let image = omega_image_emission::emit_executable_image(&object, 0)
            .expect("ranked final-image replay should preserve object custody");
        omega_image_emission::validate_executable_image(&object, &image)
            .expect("ranked object/image custody should replay independently");
        assert_eq!(
            image.functions()[0].ranked_u32_countdown.as_ref(),
            Some(record)
        );
        let installation = omega_image_emission::build_installation_record(
            &image,
            psi_core::ProfileDecisionId::new(1).expect("profile decision"),
        )
        .expect("ranked final image should enter canonical installation custody");
        assert!(installation.functions()[0].ranked_u32_countdown);
        let installation_bytes = omega_image_emission::encode_installation_record(&installation)
            .expect("encode ranked installation");
        let decoded = omega_image_emission::decode_installation_record(&installation_bytes)
            .expect("decode ranked installation");
        assert!(decoded.functions()[0].ranked_u32_countdown);
        omega_image_emission::validate_installation_record(&decoded, &image)
            .expect("decoded ranked installation must bind its exact final image");
        let canonical = psi_terminal_codec::CanonicalTerminalArtifact::from_parts(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            None,
        )
        .expect("encode canonical ranked artifact");
        let native =
            omega_native_artifact::NativeArtifact::from_emitted_parts(
                omega_native_artifact::NativeArtifactEmissionParts {
                    target,
                    psi_artifact: canonical,
                    object,
                    image,
                    selected_provider_closure_report_identity: 1,
                    selected_provider_closure_digest:
                        omega_native_artifact::NativeSelectedProviderClosureDigest::from_digest(
                            [1; 32],
                        ),
                    selected_provider_plans: Vec::new(),
                    provider_executions: Vec::new(),
                    physical_evidence_scope:
                        omega_native_artifact::NativePhysicalEvidenceScope::Unavailable,
                },
            )
            .expect("ranked object and final image should enter native-artifact custody");
        native
            .validate()
            .expect("ranked native artifact should replay independently");

        let assert_invalid = |candidate: &omega_machine_code::MachineCodePlan| {
            assert!(matches!(
                omega_image_emission::build_object_artifact(candidate),
                Err(omega_image_emission::ObjectError::InvalidRankedCountdown(machine))
                    if machine == function.machine
            ));
        };

        let mut corrupted = emitted.clone();
        corrupted.functions[0].bytes[0] ^= 1;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].provenance.operations.swap(0, 1);
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .call_plan
            .parameters[0]
            .locations
            .clear();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .cleanup_actions
            .clear();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .graph
            .compare_operation = psi_core::OperationId::new(99).unwrap();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let replacement = psi_core::OperationId::new(99).unwrap();
        let original = corrupted.functions[0]
            .ranked_u32_countdown
            .as_ref()
            .unwrap()
            .custody
            .graph
            .zero_operation;
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .graph
            .zero_operation = replacement;
        corrupted.functions[0].provenance.operations[0] = replacement;
        corrupted.functions[0]
            .semantic_code_attribution
            .iter_mut()
            .find(|row| row.site == omega_machine_code::SemanticCodeSite::Operation(original))
            .unwrap()
            .site = omega_machine_code::SemanticCodeSite::Operation(replacement);
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let mut extra_type = corrupted.functions[0]
            .ranked_u32_countdown
            .as_ref()
            .unwrap()
            .structural_types[0]
            .clone();
        extra_type.id = psi_core::StructuralTypeId::new(99).unwrap();
        extra_type.identity.push_str("::substituted");
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .structural_types
            .push(extra_type);
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let substituted_shape = omega_calling_conventions::ValueShape::integer(8, 8);
        let substituted_call_plan = omega_calling_conventions::evaluate_call_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            &omega_calling_conventions::CallSignature {
                parameters: vec![
                    omega_calling_conventions::ValueShape::integer(4, 4),
                    substituted_shape,
                ],
                result: None,
            },
        )
        .unwrap();
        let substituted_placement = substituted_call_plan.parameters[1].clone();
        let ranked = corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap();
        ranked.structural_parameters[0].shape = substituted_shape;
        ranked.structural_parameters[0].placement = substituted_placement;
        ranked.call_plan = substituted_call_plan;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .proof_replay
            .clear();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .structural_frontiers
            .machine = psi_core::MachineId::new(2).unwrap();
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].attachment = None;
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let structural = &record.structural_parameters[0];
        corrupted.functions[0]
            .unit_parameters
            .push(omega_machine_code::UnitParameterRecord {
                place: structural.place,
                structural_type: structural.structural_type,
                multiplicity: structural.multiplicity,
                shape: structural.shape,
            });
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.functions[0].scalar_stack = Some(omega_machine_code::ScalarStackEvidence {
            mutations: Vec::new(),
            control_flow: omega_machine_code::ScalarControlFlowEvidence::Linear,
            stack_alignment: 16,
            cleanup_preservation: None,
        });
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        corrupted.target = match target.architecture {
            omega_target::Architecture::X86_64 => omega_target::NativeTarget::windows_x64(),
            omega_target::Architecture::Aarch64 => omega_target::NativeTarget::macos_arm64(),
        };
        assert_invalid(&corrupted);

        let mut corrupted = emitted.clone();
        let mut extra = corrupted.functions[0].clone();
        extra.machine = psi_core::MachineId::new(2).unwrap();
        extra.attachment = None;
        extra.provenance = omega_target_operations::TerminalPsiProvenance {
            operations: Vec::new(),
            edges: Vec::new(),
        };
        extra.ranked_u32_countdown = None;
        extra.semantic_code_attribution.clear();
        extra.bytes = match target.architecture {
            omega_target::Architecture::X86_64 => vec![0xc3],
            omega_target::Architecture::Aarch64 => 0xd65f03c0_u32.to_le_bytes().to_vec(),
        };
        corrupted.functions.push(extra);
        assert_invalid(&corrupted);
    }
}
