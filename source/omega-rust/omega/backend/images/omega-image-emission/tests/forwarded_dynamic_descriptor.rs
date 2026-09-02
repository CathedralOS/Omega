use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_image_emission::{
    ObjectError, build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, encode_installation_record, validate_executable_image,
    validate_installation_record,
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
        trait Measure { machine measure(&self) -> i32; }
        data Item { value: i32; }
        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 { transition { _ -> self.value } }
        }
        data Main { decoy: Item; selected: Item; }
        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }
        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;
    machine_plan_from_source(source, target)
}

fn changed_conformance_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    machine_plan_from_source(
        r#"
            trait Measure { machine measure(&self) -> i32; }
            data Item { value: i32; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> i32 { transition { _ -> self.value } }
            }
            Secondary: Item satisfies Measure {
                machine measure(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { decoy: Item; selected: Item; }
            machine Main::run(&mut self) {
                let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
                erased = &self.selected as &dyn Item::Secondary;
                let result: i32 = forward(erased);
            }
            machine forward(erased: &dyn Measure) -> i32 {
                let result: i32 = erased.measure();
                transition { _ -> result }
            }
        "#,
        target,
    )
}

fn direct_scalar_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    machine_plan_from_source(
        r#"
            trait Measure { machine measure(&self) -> i32; }
            data Item { value: i32; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> i32 { transition { _ -> self.value } }
            }
            data Main { selected: Item; }
            machine Main::run(&mut self) {
                let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
                let result: i32 = forward(erased);
            }
            machine forward(erased: &dyn Measure) -> i32 {
                let result: i32 = erased.measure();
                transition { _ -> result }
            }
        "#,
        target,
    )
}

fn unit_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    machine_plan_from_source(
        r#"
            trait Touch { machine touch(&self); }
            data Item { value: i32; }
            Primary: Item satisfies Touch { machine touch(&self) {} }
            data Main { decoy: Item; selected: Item; }
            machine Main::run(&mut self) {
                let mut erased: &dyn Touch = &self.decoy as &dyn Item::Primary;
                erased = &self.selected as &dyn Item::Primary;
                forward(erased);
            }
            machine forward(erased: &dyn Touch) { erased.touch(); }
        "#,
        target,
    )
}

fn direct_unit_machine_plan(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    machine_plan_from_source(
        r#"
            trait Touch { machine touch(&self); }
            data Item { value: i32; }
            Primary: Item satisfies Touch { machine touch(&self) {} }
            data Main { selected: Item; }
            machine Main::run(&mut self) {
                let erased: &dyn Touch = &self.selected as &dyn Item::Primary;
                forward(erased);
            }
            machine forward(erased: &dyn Touch) { erased.touch(); }
        "#,
        target,
    )
}

fn machine_plan_from_source(
    source: &str,
    target: NativeTarget,
) -> omega_machine_code::MachineCodePlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower forwarded descriptor source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("admit Terminal artifact");
    let target_plan =
        lower_to_target_operations(&abstract_plan, target).expect("lower target operations");
    let assigned = assign_registers(&target_plan).expect("assign target operations");
    emit_machine_code(&assigned).expect("emit caller and helper")
}

#[test]
fn object_image_and_installation_replay_direct_selection_scalar_forwarding() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = direct_scalar_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one direct-selection scalar call expected")
        };
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one direct-selection scalar argument expected")
        };
        assert!(matches!(
            argument.custody.source,
            omega_abstract_operations::AbstractDynamicDescriptorSource::Selection { .. }
        ));
        assert!(call.semantic_result.is_some());
        assert!(call.result.is_some());

        let object =
            build_object_artifact(&plan).expect("replay direct-selection scalar object evidence");
        let image = emit_executable_image(&object, 3)
            .expect("link direct-selection scalar forwarded image");
        validate_executable_image(&object, &image)
            .expect("replay direct-selection scalar linked image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("build direct-selection scalar installation");
        let [installed_call] = installation.forwarded_dynamic_descriptor_calls() else {
            panic!("one installed direct-selection scalar call expected")
        };
        assert!(installed_call.semantic_result.is_some());
        assert!(installed_call.result.is_some());
        let encoded = encode_installation_record(&installation).expect("encode installation");
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone())
        );
        validate_installation_record(&installation, &image)
            .expect("replay direct-selection scalar installed evidence");
    }
}

#[test]
fn object_image_and_installation_replay_changed_conformance_forwarding() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = changed_conformance_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one changed-conformance forwarded call expected")
        };
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one changed-conformance descriptor argument expected")
        };
        let omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
            initial_application,
            application,
            ..
        } = &argument.custody.source
        else {
            panic!("changed-conformance forwarding must retain rebound custody")
        };
        assert_ne!(initial_application.commitment, application.commitment);
        assert!(initial_application.realization_callables.is_empty());
        assert_eq!(application.realization_callables.len(), 1);

        let object = build_object_artifact(&plan)
            .expect("replay changed-conformance forwarded object evidence");
        assert!(object.dynamic_conformance_tables().is_empty());
        let [table] = object.forwarded_dynamic_descriptor_tables() else {
            panic!("only the live changed-conformance forwarded table should materialize")
        };
        assert_eq!(table.application.commitment, application.commitment);
        assert_ne!(table.application.commitment, initial_application.commitment);
        let image =
            emit_executable_image(&object, 3).expect("link changed-conformance forwarded image");
        validate_executable_image(&object, &image)
            .expect("replay changed-conformance linked image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("build changed-conformance installation");
        validate_installation_record(&installation, &image)
            .expect("replay changed-conformance installed evidence");
        let encoded = encode_installation_record(&installation).expect("encode installation");
        assert_eq!(decode_installation_record(&encoded), Ok(installation));
    }
}

#[test]
fn object_image_and_installation_replay_result_less_forwarding() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = unit_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one result-less forwarded call expected")
        };
        assert!(call.semantic_result.is_none());
        assert!(call.result.is_none());
        assert!(caller.unit_scalar_homes.is_empty());
        let helper = plan
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .unwrap();
        let [parameter_call] = helper.dynamic_parameter_calls.as_slice() else {
            panic!("one helper parameter-slot call expected")
        };
        assert!(parameter_call.source_value.is_none());
        assert!(parameter_call.scalar_type.is_none());

        let object = build_object_artifact(&plan).expect("replay result-less object evidence");
        let image = emit_executable_image(&object, 3).expect("link result-less forwarded image");
        validate_executable_image(&object, &image).expect("replay result-less linked image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("build result-less installation");
        let [installed_call] = installation.forwarded_dynamic_descriptor_calls() else {
            panic!("one installed result-less forwarded call expected")
        };
        assert!(installed_call.semantic_result.is_none());
        assert!(installed_call.result.is_none());
        let [installed_parameter_call] = installation.dynamic_parameter_calls() else {
            panic!("one installed result-less parameter call expected")
        };
        assert!(installed_parameter_call.source_value.is_none());
        let encoded = encode_installation_record(&installation).expect("encode installation");
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone())
        );
        validate_installation_record(&installation, &image)
            .expect("replay result-less installed evidence");
    }
}

#[test]
fn object_image_and_installation_replay_direct_selection_forwarding() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = direct_unit_machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one direct-selection forwarded call expected")
        };
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one direct-selection descriptor argument expected")
        };
        assert!(matches!(
            argument.custody.source,
            omega_abstract_operations::AbstractDynamicDescriptorSource::Selection { .. }
        ));
        assert!(call.semantic_result.is_none());
        assert!(call.result.is_none());

        let object = build_object_artifact(&plan).expect("replay direct-selection object evidence");
        let image =
            emit_executable_image(&object, 3).expect("link direct-selection forwarded image");
        validate_executable_image(&object, &image).expect("replay direct-selection linked image");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("build direct-selection installation");
        let [installed_call] = installation.forwarded_dynamic_descriptor_calls() else {
            panic!("one installed direct-selection call expected")
        };
        assert!(installed_call.semantic_result.is_none());
        assert!(installed_call.result.is_none());
        let encoded = encode_installation_record(&installation).expect("encode installation");
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone())
        );
        validate_installation_record(&installation, &image)
            .expect("replay direct-selection installed evidence");
    }
}

#[test]
fn object_construction_binds_forwarded_tables_through_adapters() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = machine_plan(target);
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one forwarded descriptor call expected")
        };
        let object = build_object_artifact(&plan).expect("bind forwarded descriptor object");
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one forwarded argument expected")
        };
        let [table] = object.forwarded_dynamic_descriptor_tables() else {
            panic!("one forwarded table expected")
        };
        assert_eq!(
            table.application.commitment,
            argument.adapters[0].identity.application
        );
        assert_eq!(table.slots.len(), argument.adapters.len());
        assert_eq!(
            object.forwarded_dynamic_descriptor_adapters().len(),
            argument.adapters.len()
        );
        for (slot, adapter) in table
            .slots
            .iter()
            .zip(object.forwarded_dynamic_descriptor_adapters())
        {
            assert_eq!(slot.adapter, adapter.record.identity);
            assert_eq!(slot.adapter_symbol, adapter.symbol);
            assert_eq!(adapter.bytes(&object), adapter.record.bytes);
        }
        assert_eq!(
            object.relocations().records().count(),
            plan.functions
                .iter()
                .map(|function| function.internal_calls.len())
                .sum::<usize>()
                + argument.adapters.len() * 2
                + match target.architecture {
                    omega_target::Architecture::X86_64 => 1,
                    omega_target::Architecture::Aarch64 => 2,
                }
        );
        let image = emit_executable_image(&object, 3).expect("emit forwarded descriptor image");
        validate_executable_image(&object, &image).expect("replay forwarded descriptor image");
        assert_eq!(
            image.forwarded_dynamic_descriptor_adapters(),
            object.forwarded_dynamic_descriptor_adapters()
        );
        assert_eq!(
            image.forwarded_dynamic_descriptor_tables(),
            object.forwarded_dynamic_descriptor_tables()
        );
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).expect("profile decision"))
                .expect("build forwarded descriptor installation");
        assert_eq!(
            installation.forwarded_dynamic_descriptor_adapters().len(),
            argument.adapters.len()
        );
        assert_eq!(installation.forwarded_dynamic_descriptor_tables().len(), 1);
        let [installed_call] = installation.forwarded_dynamic_descriptor_calls() else {
            panic!("one installed forwarded descriptor call expected")
        };
        assert_eq!(installed_call.semantic_result, call.semantic_result);
        assert_eq!(installed_call.result, call.result);
        assert!(
            installation
                .functions()
                .iter()
                .find(|function| function.machine == caller.machine)
                .expect("installed caller")
                .unit_scalar_homes
                .contains(&installed_call.result.as_ref().unwrap().home)
        );
        assert_eq!(installation.dynamic_parameter_calls().len(), 1);
        let bytes = encode_installation_record(&installation).expect("encode installation");
        assert_eq!(decode_installation_record(&bytes), Ok(installation.clone()));
        validate_installation_record(&installation, &image)
            .expect("replay forwarded descriptor installation");
    }
}

#[test]
fn object_construction_rejects_forwarded_adapter_byte_drift() {
    let mut plan = machine_plan(NativeTarget::linux_x64());
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let call = caller
        .forwarded_dynamic_descriptor_calls
        .first_mut()
        .expect("forwarded descriptor call");
    let caller_id = caller.machine;
    let operation = call.psi_operation;
    call.dynamic_arguments[0].adapters[0].bytes[0] ^= 1;
    assert_eq!(
        build_object_artifact(&plan),
        Err(ObjectError::InvalidForwardedDynamicDescriptorEvidence {
            caller: caller_id,
            operation,
        })
    );
}

#[test]
fn object_construction_rejects_fabricated_results_on_unit_forwarding() {
    let mut plan = unit_machine_plan(NativeTarget::linux_x64());
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .expect("entry caller");
    let call = caller
        .forwarded_dynamic_descriptor_calls
        .first_mut()
        .expect("result-less forwarded call");
    call.semantic_result = Some(omega_abstract_operations::AbstractResult {
        value: psi_core::ValueId::new(999).unwrap(),
        scalar_type: psi_core::ScalarType::Boolean,
    });
    let caller_machine = caller.machine;
    let operation = call.psi_operation;
    assert_eq!(
        build_object_artifact(&plan),
        Err(ObjectError::InvalidForwardedDynamicDescriptorEvidence {
            caller: caller_machine,
            operation,
        })
    );

    let mut plan = unit_machine_plan(NativeTarget::linux_x64());
    let helper = plan
        .functions
        .iter_mut()
        .find(|function| !function.dynamic_parameter_calls.is_empty())
        .expect("forward helper");
    let parameter_call = &mut helper.dynamic_parameter_calls[0];
    parameter_call.scalar_type = Some(psi_core::ScalarType::Boolean);
    let helper_machine = helper.machine;
    let operation = parameter_call.psi_operation;
    assert_eq!(
        build_object_artifact(&plan),
        Err(ObjectError::InvalidDynamicParameterCallEvidence {
            caller: helper_machine,
            operation,
        })
    );
}

#[test]
fn object_construction_rejects_forwarded_scalar_result_custody_drift() {
    fn assert_rejected(plan: &omega_machine_code::MachineCodePlan) {
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .expect("entry caller");
        let call = caller
            .forwarded_dynamic_descriptor_calls
            .first()
            .expect("forwarded descriptor call");
        assert_eq!(
            build_object_artifact(plan),
            Err(ObjectError::InvalidForwardedDynamicDescriptorEvidence {
                caller: caller.machine,
                operation: call.psi_operation,
            })
        );
    }

    let original = machine_plan(NativeTarget::linux_x64());

    let mut drifted = original.clone();
    let caller = drifted
        .functions
        .iter_mut()
        .find(|function| function.machine == drifted.entry)
        .expect("entry caller");
    caller.forwarded_dynamic_descriptor_calls[0]
        .result
        .as_mut()
        .unwrap()
        .home
        .source_value = psi_core::ValueId::new(999).unwrap();
    assert_rejected(&drifted);

    let mut drifted = original.clone();
    let caller = drifted
        .functions
        .iter_mut()
        .find(|function| function.machine == drifted.entry)
        .expect("entry caller");
    let result_offset = caller.forwarded_dynamic_descriptor_calls[0]
        .result
        .as_ref()
        .unwrap()
        .code_offset;
    caller.bytes[result_offset] ^= 1;
    assert_rejected(&drifted);

    let mut drifted = original;
    let caller = drifted
        .functions
        .iter_mut()
        .find(|function| function.machine == drifted.entry)
        .expect("entry caller");
    caller.unit_scalar_homes.clear();
    assert_rejected(&drifted);
}
