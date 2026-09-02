use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_image_emission::{
    InstallationError, build_installation_record, build_object_artifact, emit_executable_image,
};
use omega_machine_emission::emit_machine_code;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_core::ProfileDecisionId;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
            machine alternate(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }

            machine alternate(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = erased.measure();
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower rebound dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("lower verified Terminal artifact");
    let target_plan = lower_to_target_operations(&abstract_plan, target)
        .expect("lower rebound dynamic call to target operations");
    let assigned = assign_registers(&target_plan).expect("assign rebound descriptor");
    emit_machine_code(&assigned).expect("emit rebound descriptor and indirect call")
}

#[test]
fn rebound_dynamic_call_materializes_complete_private_table_and_executes_image_replay() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let plan = machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.dynamic_scalar_calls.as_slice() else {
            panic!("one rebound dynamic call expected: {caller:#?}")
        };
        assert_eq!(call.dynamic_dispatch.application.rows.len(), 2);
        assert_ne!(
            call.initial_instance.source.path,
            call.rebound_instance.source.path
        );
        assert!(call.indirect_call_byte_count > 0);
        assert!(
            caller
                .internal_calls
                .iter()
                .all(|direct| { direct.target != call.dynamic_dispatch.dispatch.realization })
        );

        let artifact = build_object_artifact(&plan).expect("materialize exact private table");
        let [table] = artifact.dynamic_conformance_tables() else {
            panic!("one deduplicated conformance table expected")
        };
        assert_eq!(table.application.rows.len(), 2);
        assert_eq!(table.slots.len(), 2);
        assert_eq!(artifact.data_bytes(), &[0; 16]);
        let data_relocations = artifact
            .relocations()
            .records()
            .filter(|(_, relocation)| relocation.section == omega_object_file::SectionKind::Data)
            .collect::<Vec<_>>();
        assert_eq!(data_relocations.len(), 2);
        assert!(data_relocations.iter().all(|(_, relocation)| {
            relocation.kind == omega_object_file::RelocationKind::Absolute64
                && relocation.byte_width == 8
                && relocation.origin
                    == omega_object_file::RelocationOrigin::Materialization {
                        object_symbol_handle: table.symbol,
                    }
        }));

        let image = emit_executable_image(&artifact, 3)
            .expect("direct image replay must retain relocated table data");
        assert_eq!(image.output().final_data_bytes.len(), 16);
        assert_ne!(image.output().final_data_bytes, vec![0; 16]);
        assert_ne!(&image.output().final_data_bytes[..8], &[0; 8]);
        assert_ne!(&image.output().final_data_bytes[8..], &[0; 8]);
        assert!(matches!(
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"),),
            Err(InstallationError::DynamicConformanceInstallationPending)
        ));
    }
}

#[test]
fn object_replay_rejects_dynamic_slot_and_descriptor_byte_substitution() {
    let plan = machine_plan(NativeTarget::linux_x64());
    let mut wrong_slot = plan.clone();
    let call = wrong_slot
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_slot.entry)
        .and_then(|function| function.dynamic_scalar_calls.first_mut())
        .expect("dynamic call");
    call.selected_table_byte_offset ^= 8;
    assert!(build_object_artifact(&wrong_slot).is_err());

    let mut wrong_descriptor = plan;
    let caller = wrong_descriptor
        .functions
        .iter_mut()
        .find(|function| function.machine == wrong_descriptor.entry)
        .expect("entry caller");
    let call = caller.dynamic_scalar_calls.first().expect("dynamic call");
    caller.bytes[call.rebound_instance.code_offset] ^= 1;
    assert!(build_object_artifact(&wrong_descriptor).is_err());
}
