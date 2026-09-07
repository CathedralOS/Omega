//! Ranked mutable-receiver custody through object and image replay.

use crate::tests::fixtures::checked_source::checked;

const RANKED_RECEIVER_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root { token: Token; }

    machine Root::countdown(&mut self, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(remaining - 1)
            _ -> done()
        }
        state done(&mut self) {}
    }
"#;

#[test]
fn ranked_mutable_receiver_survives_both_native_object_and_image_replays() {
    assert_ranked_receiver_replays(
        RANKED_RECEIVER_COUNTDOWN_SOURCE,
        calling_conventions::ValueShape::integer(4, 4),
    );
}

#[test]
fn ranked_mutable_receiver_with_wide_record_survives_native_replays() {
    let source = RANKED_RECEIVER_COUNTDOWN_SOURCE.replace(
        "data Token { value: i32; }",
        "data Token { first: u64; second: u64; third: u64; }",
    );
    assert_ranked_receiver_replays(&source, calling_conventions::ValueShape::integer(24, 8));
}

fn assert_ranked_receiver_replays(source: &str, referent: calling_conventions::ValueShape) {
    let checked = checked(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::countdown")
        .expect("lower ranked receiver Terminal Psi");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode ranked receiver semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode ranked receiver proof");
    let admitted =
        terminal_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("admit ranked receiver custody");
    let terminal_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
        ranked,
    ) = admitted
    else {
        panic!("ranked receiver must use dedicated native custody")
    };

    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                &ranked, target,
            )
            .expect("lower ranked receiver target operations");
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("assign ranked receiver");
        let emitted =
            machine_emission::emit_machine_code(&assigned).expect("emit ranked receiver countdown");
        let record = emitted.functions[0]
            .ranked_u32_countdown
            .as_ref()
            .expect("receiver custody reaches machine code");
        let [replay] = record.custody.semantic_replay.machines[0]
            .structural_parameters
            .as_slice()
        else {
            panic!("one replay receiver")
        };
        let [physical] = record.structural_parameters.as_slice() else {
            panic!("one physical receiver")
        };
        assert!(replay.is_self);
        assert_eq!(replay.access, terminal_psi::StructuralAccess::MutableBorrow);
        assert_eq!(
            replay.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
        );
        assert_eq!(physical.place, replay.place);
        assert_eq!(physical.structural_type, replay.structural_type);
        assert_eq!(physical.access, replay.access);
        assert_eq!(physical.multiplicity, replay.multiplicity);
        assert_eq!(
            physical.shape,
            calling_conventions::ValueShape::borrowed_reference(
                referent.byte_size,
                referent.alignment
            )
        );
        let receiver_register = if target == target::NativeTarget::linux_x64() {
            target_operations::MachineRegister::X86Rsi
        } else {
            target_operations::MachineRegister::Aarch64X(1)
        };
        assert_eq!(
            physical.placement.locations,
            vec![calling_conventions::ValueLocation::Indirect {
                pointer: calling_conventions::IndirectPointerLocation::Register(receiver_register),
                copy_stack_byte_offset: None,
                byte_size: referent.byte_size,
                alignment: referent.alignment,
            }]
        );
        let expected_bytes = if target == target::NativeTarget::linux_x64() {
            isa_x86_64::encode_ranked_u32_countdown_in_edi().to_vec()
        } else {
            isa_aarch64::encode_ranked_u32_countdown_in_w0().to_vec()
        };
        assert_eq!(
            emitted.functions[0].bytes, expected_bytes,
            "receiver metadata does not change the exact countdown body"
        );
        assert!(record.cleanup_actions.is_empty());
        assert!(
            record
                .custody
                .structural_frontiers
                .header_entry
                .owned_places()
                .is_empty()
        );
        assert_eq!(
            record.custody.structural_frontiers.header_entry,
            record.custody.structural_frontiers.backedge_exit
        );

        let object = image_emission::build_object_artifact(&emitted)
            .expect("replay receiver object custody");
        let image = image_emission::emit_executable_image(&object, 0)
            .expect("emit ranked receiver final image");
        image_emission::validate_executable_image(&object, &image)
            .expect("replay ranked receiver image custody");
        assert_eq!(
            image.functions()[0].ranked_u32_countdown.as_ref(),
            Some(record)
        );
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).expect("profile decision"),
        )
        .expect("install ranked receiver image custody");
        let installation_bytes = image_emission::encode_installation_record(&installation)
            .expect("encode ranked receiver installation");
        let decoded = image_emission::decode_installation_record(&installation_bytes)
            .expect("decode ranked receiver installation");
        image_emission::validate_installation_record(&decoded, &image)
            .expect("bind ranked receiver installation to its image");

        let assert_invalid = |candidate: &machine_code::MachineCodePlan| {
            assert!(matches!(
                image_emission::build_object_artifact(candidate),
                Err(image_emission::ObjectError::InvalidRankedCountdown(machine))
                    if machine == emitted.entry
            ));
        };
        for shape in [
            calling_conventions::ValueShape::integer(8, 8),
            calling_conventions::ValueShape::borrowed_reference(
                referent.byte_size + referent.alignment,
                referent.alignment,
            ),
            calling_conventions::ValueShape::borrowed_reference(
                referent.byte_size,
                if referent.alignment == 4 { 8 } else { 4 },
            ),
        ] {
            let call_plan = calling_conventions::evaluate_call_plan(
                calling_conventions::CallingPolicy::native_for_target(target),
                &calling_conventions::CallSignature {
                    parameters: vec![calling_conventions::ValueShape::integer(4, 4), shape],
                    result: None,
                },
            )
            .expect("forged receiver has a coherent generic ABI plan");

            let mut forged = assigned.clone();
            let assigned_target_operations::AssignedOperation::RankedU32Countdown(countdown) =
                &mut forged.functions[0].operation
            else {
                panic!("ranked assigned operation")
            };
            countdown.structural_parameters[0].shape = shape;
            countdown.structural_parameters[0].placement = call_plan.parameters[1].clone();
            countdown.call_plan = call_plan.clone();
            assert!(
                matches!(
                    machine_emission::emit_machine_code(&forged),
                    Err(machine_emission::EmissionError::InvalidRankedCountdown(machine))
                        if machine == emitted.entry
                ),
                "emitter must replay receiver referent layout for {shape:?}"
            );

            let mut forged = emitted.clone();
            let record = forged.functions[0].ranked_u32_countdown.as_mut().unwrap();
            record.structural_parameters[0].shape = shape;
            record.structural_parameters[0].placement = call_plan.parameters[1].clone();
            record.call_plan = call_plan;
            assert_invalid(&forged);
        }
        let mut forged = emitted.clone();
        forged.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .custody
            .semantic_replay
            .machines[0]
            .structural_parameters[0]
            .is_self = false;
        assert_invalid(&forged);

        let mut forged = emitted.clone();
        forged.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .structural_parameters[0]
            .access = terminal_psi::StructuralAccess::Owned;
        assert_invalid(&forged);

        let mut forged = emitted.clone();
        forged.functions[0]
            .ranked_u32_countdown
            .as_mut()
            .unwrap()
            .call_plan
            .parameters[1]
            .locations
            .clear();
        assert_invalid(&forged);
    }
}
