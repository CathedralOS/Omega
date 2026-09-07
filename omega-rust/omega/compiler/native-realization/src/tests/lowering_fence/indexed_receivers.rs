//! Indexed receiver loans retain their exact native pointer projection.

use super::checked;

#[test]
fn indexed_write_only_receiver_reaches_both_linux_artifacts() {
    let checked = checked(
        "data Record [copy] { value: u16; }
         machine Record::replace(&write self) { self.value = 17; }
         machine forward(records: &write [Record; 2]) { records[1].replace(); }",
    );
    let artifact = terminal_production::produce_terminal_artifact(&checked, "forward").unwrap();
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target_plan = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            target,
        )
        .expect("indexed record receiver retains its target pointer projection");
        reject_target_substitutions(&target_plan);
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("indexed receiver assignment replays the projection");
        let emitted = machine_emission::emit_machine_code(&assigned).unwrap();
        reject_emitted_substitutions(&emitted);
        let object = image_emission::build_object_artifact(&emitted)
            .expect("indexed receiver object replays its pointer projection");
        let image = image_emission::emit_executable_image(&object, 3).unwrap();
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        image_emission::validate_installation_record(&installation, &image).unwrap();
        let encoded = image_emission::encode_installation_record(&installation).unwrap();
        assert_eq!(
            image_emission::decode_installation_record(&encoded).unwrap(),
            installation
        );
    }
}

fn reject_target_substitutions(plan: &target_operations::TargetOperationPlan) {
    for mutation in 0..7 {
        let mut changed = plan.clone();
        let caller = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == changed.entry)
            .unwrap();
        let target_operations::TargetOperation::UnitBody(body) = &mut caller.operation else {
            panic!("one Unit receiver caller")
        };
        let target_operations::TargetUnitOperation::Call { arguments, .. } =
            &mut body.operations[0]
        else {
            panic!("one indexed receiver call")
        };
        let argument = &mut arguments[0];
        assert_eq!(argument.source_byte_offset, 2);
        assert_eq!(
            argument.shape.class,
            calling_conventions::ValueClass::BorrowedReference
        );
        match mutation {
            0 => argument.source_byte_offset = 0,
            1 => argument.path[0] = terminal_psi::StructuralPathSegment::FixedIndex(0),
            2 => argument.path[0] = terminal_psi::StructuralPathSegment::FixedIndex(2),
            3 => argument.structural_type = argument.root_structural_type,
            4 => argument.shape = calling_conventions::ValueShape::integer(2, 2),
            5 => body.parameters[0].access = terminal_psi::StructuralAccess::SharedBorrow,
            6 => argument.fixed_array_length = Some(2),
            _ => unreachable!(),
        }
        assert!(
            target_operations_to_assigned_target_operations::assign_registers(&changed).is_err(),
            "assignment must reject receiver substitution {mutation}"
        );
    }
}

fn reject_emitted_substitutions(plan: &machine_code::MachineCodePlan) {
    for mutation in 0..7 {
        let mut changed = plan.clone();
        let caller = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == changed.entry)
            .unwrap();
        let argument = &mut caller.internal_unit_calls[0].arguments[0];
        match mutation {
            0 => argument.source_byte_offset = 0,
            1 => argument.path[0] = terminal_psi::StructuralPathSegment::FixedIndex(0),
            2 => argument.path[0] = terminal_psi::StructuralPathSegment::FixedIndex(2),
            3 => argument.structural_type = argument.root_structural_type,
            4 => argument.shape = calling_conventions::ValueShape::integer(2, 2),
            5 => argument.access = terminal_psi::StructuralAccess::SharedBorrow,
            6 => argument.element_stride = Some(2),
            _ => unreachable!(),
        }
        assert!(
            image_emission::build_object_artifact(&changed).is_err(),
            "object construction must reject receiver substitution {mutation}"
        );
    }
}
