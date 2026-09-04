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
    let checked = checked(RANKED_RECEIVER_COUNTDOWN_SOURCE);
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower ranked receiver Terminal Psi");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode ranked receiver semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode ranked receiver proof");
    let admitted =
        omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
            &semantic,
            &proof,
            &psi_proof_admission::AdmissionProfile::default(),
        )
        .expect("admit ranked receiver custody");
    let omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(ranked) =
        admitted
    else {
        panic!("ranked receiver must use dedicated native custody")
    };

    for target in [
        omega_target::NativeTarget::linux_x64(),
        omega_target::NativeTarget::linux_arm64(),
    ] {
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                &ranked, target,
            )
            .expect("lower ranked receiver target operations");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("assign ranked receiver");
        let emitted = omega_machine_emission::emit_machine_code(&assigned)
            .expect("emit ranked receiver countdown");
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
        assert_eq!(replay.access, psi_terminal::StructuralAccess::MutableBorrow);
        assert_eq!(
            replay.multiplicity,
            psi_terminal::StructuralMultiplicity::Affine
        );
        assert_eq!(physical.place, replay.place);
        assert_eq!(physical.structural_type, replay.structural_type);
        assert_eq!(physical.access, replay.access);
        assert_eq!(physical.multiplicity, replay.multiplicity);
        assert_eq!(
            physical.shape,
            omega_calling_conventions::ValueShape::integer(8, 8)
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

        let object = omega_image_emission::build_object_artifact(&emitted)
            .expect("replay receiver object custody");
        let image = omega_image_emission::emit_executable_image(&object, 0)
            .expect("emit ranked receiver final image");
        omega_image_emission::validate_executable_image(&object, &image)
            .expect("replay ranked receiver image custody");

        let assert_invalid = |candidate: &omega_machine_code::MachineCodePlan| {
            assert!(matches!(
                omega_image_emission::build_object_artifact(candidate),
                Err(omega_image_emission::ObjectError::InvalidRankedCountdown(machine))
                    if machine == emitted.entry
            ));
        };
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
            .access = psi_terminal::StructuralAccess::Owned;
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
