use abstract_operations_to_target_operations::lower_to_target_operations;
use image_emission::{
    ObjectError, build_installation_record, build_object_artifact, decode_installation_record,
    encode_installation_record, validate_executable_image, validate_installation_record,
};
use machine_emission::emit_machine_code;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ProfileDecisionId;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations_to_assigned_target_operations::assign_registers;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    trait Measure { machine measure(&self) -> bool; }
    data Item [copy] { marker: bool; }
    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }
    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }
    data Main [copy] { first: Item; second: Item; }
    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }
        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = forward(selected);
        }
        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = forward(selected);
        }
    }
    machine forward(erased: &dyn Measure) -> bool {
        let result: bool = relay(erased);
        transition { _ -> result }
    }
    machine relay(erased: &dyn Measure) -> bool {
        let result: bool = finish(erased);
        transition { _ -> result }
    }
    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

const UNIT_SOURCE: &str = r#"
    trait Touch { machine touch(&self); }
    data Item { value: i32; }
    Primary: Item satisfies Touch { machine touch(&self) {} }
    Secondary: Item satisfies Touch { machine touch(&self) {} }
    data Main { first: Item; second: Item; }
    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }
        state take_first(&self) {
            let selected: &dyn Touch = &self.first as &dyn Item::Primary;
            forward(selected);
        }
        state take_second(&self) {
            let selected: &dyn Touch = &self.second as &dyn Item::Secondary;
            forward(selected);
        }
    }
    machine forward(erased: &dyn Touch) { relay(erased); }
    machine relay(erased: &dyn Touch) { finish(erased); }
    machine finish(erased: &dyn Touch) { erased.touch(); }
"#;

fn emitted_plan_from(source: &str, target: NativeTarget) -> machine_code::MachineCodePlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = checked_trees_to_terminal_psi::lower_machine(&checked, "Main::run")
        .expect("lower joined dynamic source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("admit Terminal artifact");
    let target_plan =
        lower_to_target_operations(&abstract_plan, target).expect("lower target operations");
    let assigned = assign_registers(&target_plan).expect("assign target operations");
    emit_machine_code(&assigned).expect("emit joined machine code")
}

fn emitted_plan(target: NativeTarget) -> machine_code::MachineCodePlan {
    emitted_plan_from(SOURCE, target)
}

#[test]
fn object_and_image_replay_joined_descriptor_control() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let emitted = emitted_plan(target);
        let expected_error = ObjectError::InvalidUnitDynamicDescriptorJoin(emitted.entry);

        let mut bad_condition_branch = emitted.clone();
        let caller = bad_condition_branch
            .functions
            .iter_mut()
            .find(|function| function.machine == bad_condition_branch.entry)
            .expect("joined caller");
        let condition = caller.semantic_code_attribution[0];
        let condition_end = condition.code_offset + condition.byte_count;
        caller.bytes[condition_end - 1] ^= 1;
        assert_eq!(
            build_object_artifact(&bad_condition_branch),
            Err(expected_error.clone())
        );

        let mut bad_join_branch = emitted.clone();
        let caller = bad_join_branch
            .functions
            .iter_mut()
            .find(|function| function.machine == bad_join_branch.entry)
            .expect("joined caller");
        let join = caller.semantic_code_attribution[2];
        caller.bytes[join.code_offset + join.byte_count - 1] ^= 1;
        assert_eq!(
            build_object_artifact(&bad_join_branch),
            Err(expected_error.clone())
        );

        let mut collapsed_source = emitted.clone();
        let caller = collapsed_source
            .functions
            .iter_mut()
            .find(|function| function.machine == collapsed_source.entry)
            .expect("joined caller");
        let first_source = caller.forwarded_dynamic_descriptor_calls[0].dynamic_arguments[0]
            .custody
            .source
            .clone();
        caller.forwarded_dynamic_descriptor_calls[1].dynamic_arguments[0]
            .custody
            .source = first_source;
        assert!(build_object_artifact(&collapsed_source).is_err());

        let object = build_object_artifact(&emitted).expect("validate joined object custody");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("link joined descriptor image");
        validate_executable_image(&object, &image).expect("replay joined descriptor image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("retain joined descriptor control in installation");
        let installed_caller = installation
            .functions()
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("installed joined caller");
        assert_eq!(
            installed_caller.unit_scalar_abi,
            emitted
                .functions
                .iter()
                .find(|function| function.machine == emitted.entry)
                .expect("emitted joined caller")
                .unit_scalar_abi
        );
        let bytes = encode_installation_record(&installation).expect("encode installation");
        let decoded = decode_installation_record(&bytes).expect("decode installation");
        assert_eq!(decoded, installation);
        validate_installation_record(&decoded, &image)
            .expect("replay joined descriptor installation");
    }
}

#[test]
fn object_and_image_replay_result_less_joined_descriptor_control() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let emitted = emitted_plan_from(UNIT_SOURCE, target);
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("result-less joined caller");
        assert_eq!(caller.forwarded_dynamic_descriptor_calls.len(), 2);

        let mut collapsed_source = emitted.clone();
        let caller = collapsed_source
            .functions
            .iter_mut()
            .find(|function| function.machine == collapsed_source.entry)
            .expect("result-less joined caller");
        let first_source = caller.forwarded_dynamic_descriptor_calls[0].dynamic_arguments[0]
            .custody
            .source
            .clone();
        caller.forwarded_dynamic_descriptor_calls[1].dynamic_arguments[0]
            .custody
            .source = first_source;
        assert!(build_object_artifact(&collapsed_source).is_err());

        let object =
            build_object_artifact(&emitted).expect("validate result-less joined object custody");
        let image = image_emission::emit_executable_image(&object, 3)
            .expect("link result-less joined descriptor image");
        validate_executable_image(&object, &image)
            .expect("replay result-less joined descriptor image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("retain result-less joined descriptor control in installation");
        let bytes = encode_installation_record(&installation).expect("encode installation");
        let decoded = decode_installation_record(&bytes).expect("decode installation");
        assert_eq!(decoded, installation);
        validate_installation_record(&decoded, &image)
            .expect("replay result-less joined descriptor installation");
    }
}
