use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, EntryStack, MachineState, MachineStateSet,
    Preemption, ValueShape,
};
use omega_compiler::{
    compile_to_checked, evaluate_calling_policy_plan, selected_external_root_provider_plan_id,
};
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "omega-calling-policy-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create calling-policy test directory");
    let main_path = directory.join("main.omg");
    fs::write(&main_path, source).expect("write calling-policy test program");
    main_path
}

const POLICY: &str = r#"
use omega::language::std::calling;

data NoResultPolicy { }

machine NoResultPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.has_result {
        true -> reject()
        _ -> accept()
    }

    state accept() -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.stack_alignment = 16;
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "return values are not supported",
            },
        }
    }
}

boundary trait Tick: Calling<NoResultPolicy> {
    machine tick();
}

data Main { }
machine Main::main(&mut self) { }
"#;

const INTERRUPT_POLICY: &str = r#"
use omega::language::std::calling;

data X86InterruptPolicy { }

machine X86InterruptPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::SystemVAMD64;
    output.call.stack_alignment = 16;
    output.call.entry_control = EntryControl::InterruptReturn;
    output.state.initial_regime = MachineRegime::X86Long64;
    output.state.interrupted_state.general_registers = true;
    output.state.interrupted_state.vector_registers = true;
    output.state.interrupted_state.flags = true;
    output.state.interrupted_state.instruction_pointer = true;
    output.state.interrupted_state.stack_pointer = true;
    output.state.saved_state.general_registers = true;
    output.state.saved_state.flags = true;
    output.state.saved_state.instruction_pointer = true;
    output.state.saved_state.stack_pointer = true;
    output.state.restored_state.general_registers = true;
    output.state.restored_state.flags = true;
    output.state.restored_state.instruction_pointer = true;
    output.state.restored_state.stack_pointer = true;
    output.state.stack = EntryStack::Dedicated { class: 1 };
    output.state.preemption = Preemption::Masked;
    BoundaryPlanResult::Accepted { plan: output }
}

boundary trait TimerRoot: Calling<X86InterruptPolicy> {
    machine tick();
}

machine timer_leaf()
    satisfies TimerRoot::tick
    via Binding::VtableSlot(0);

data Main { }
machine Main::main(&mut self) { }
"#;

#[test]
fn source_interrupt_policy_publishes_and_selects_the_complete_entry_plan() {
    let main_path = write_program("interrupt-entry", INTERRUPT_POLICY);
    let checked = compile_to_checked(&main_path, None).expect("interrupt policy should compile");
    let validated = evaluate_calling_policy_plan(
        &checked.typed,
        "X86InterruptPolicy::plan",
        &CallSignature::default(),
    )
    .expect("interrupt policy should validate");
    let plan = validated.plan();

    assert_eq!(plan.call.policy, CallingPolicy::SystemVAMD64);
    assert_eq!(plan.call.entry_control, EntryControl::InterruptReturn);
    assert_eq!(plan.state.stack, EntryStack::Dedicated { class: 1 });
    assert_eq!(plan.state.preemption, Preemption::Masked);
    let saved = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    assert_eq!(plan.state.saved_state, saved);
    assert_eq!(plan.state.restored_state, saved);
    assert!(
        plan.state
            .interrupted_state
            .contains_all(saved.union(MachineStateSet::new([MachineState::VectorRegisters])))
    );

    let timer = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "TimerRoot")
        .expect("TimerRoot boundary trait");
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, timer)
        .expect("TimerRoot service schema");
    assert_eq!(
        schema.methods[0].calling_plan_fingerprint,
        Some(validated.contract_fingerprint())
    );
    let selected = checked
        .selected_provider_plans()
        .plan_by_name("satisfies::TimerRoot")
        .expect("selected TimerRoot provider plan");
    assert_eq!(selected.rows.len(), 1);
    assert_eq!(
        selected_external_root_provider_plan_id(&checked, "TimerRoot")
            .expect("external-root bridge should retain the selected timer plan")
            .normalized_identity(),
        selected.identity_fingerprint()
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn source_policy_receives_signature_and_publishes_only_validated_acceptance() {
    let main_path = write_program("accepted", POLICY);
    let checked = compile_to_checked(&main_path, None).expect("policy program should compile");

    let validated = evaluate_calling_policy_plan(
        &checked.typed,
        "NoResultPolicy::plan",
        &CallSignature::default(),
    )
    .expect("empty signature should be accepted");

    assert_eq!(validated.plan().call.policy, CallingPolicy::MicrosoftX64);
    assert_eq!(validated.plan().call.stack_alignment, 16);
    assert_ne!(validated.contract_fingerprint(), 0);

    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, tick)
        .expect("Tick service schema");
    assert_eq!(schema.methods.len(), 1);
    assert_eq!(
        schema.methods[0].calling_plan_fingerprint,
        Some(validated.contract_fingerprint())
    );
    let retained = checked
        .typed
        .boundary_calling_plans
        .iter()
        .find(|identity| identity.fingerprint == validated.contract_fingerprint())
        .expect("typed lowering evidence for the published identity");
    assert_eq!(&retained.boundary_entry_plan, validated.plan());
    assert_eq!(
        checked.typed.boundary_entry_plan_for_arguments(
            retained.boundary_trait,
            &retained.boundary_arguments,
            retained.requirement_machine,
        ),
        Some(validated.plan()),
        "checked lowering must preserve the complete canonical inbound plan",
    );
}

#[test]
fn source_policy_rejection_preserves_the_authored_reason() {
    let main_path = write_program("rejected", POLICY);
    let checked = compile_to_checked(&main_path, None).expect("policy program should compile");

    let error = evaluate_calling_policy_plan(
        &checked.typed,
        "NoResultPolicy::plan",
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .expect_err("return-bearing signature should be rejected");

    assert!(error.contains("calling policy rejected the boundary"));
    assert!(error.contains("return values are not supported"));
}

#[test]
fn rejected_calling_relationship_is_a_compile_diagnostic() {
    let source = POLICY.replace("machine tick();", "machine tick() -> i64;");
    let main_path = write_program("relationship-rejected", &source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("a rejected Calling<C> relationship must fail compilation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("calling policy rejected the boundary"),
        "unexpected diagnostics:\n{rendered}"
    );
    assert!(
        rendered.contains("return values are not supported"),
        "unexpected diagnostics:\n{rendered}"
    );
    let rejection = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("calling policy rejected the boundary")
        })
        .expect("policy rejection diagnostic");
    let span = rejection
        .source_span
        .expect("policy rejection should retain the Calling<C> source span")
        .span;
    assert_eq!(&source[span.start..span.end], "Calling");
}

#[test]
fn policy_source_identity_is_absent_from_the_published_fingerprint() {
    let fingerprint = |name: &str| {
        let source = POLICY.replace("NoResultPolicy", name);
        let main_path = write_program(name, &source);
        let checked = compile_to_checked(&main_path, None).expect("policy program should compile");
        let tick = checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Tick")
            .expect("Tick boundary trait");
        omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, tick)
            .expect("Tick service schema")
            .methods[0]
            .calling_plan_fingerprint
            .expect("evaluated calling identity")
    };

    assert_eq!(fingerprint("FirstPolicy"), fingerprint("RenamedPolicy"));
}

#[test]
fn generic_boundary_conformance_selects_and_publishes_its_policy_instance() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {\n    machine tick();\n}",
        "boundary trait Tick<C>: Calling<C> {\n    machine tick(&mut self);\n}\n\ndata TickProvider { count: i64; }\nTickProvider satisfies Tick<NoResultPolicy>;\nmachine TickProvider::tick(&mut self) satisfies Tick::tick {\n    self.count = 1;\n}",
    );
    let main_path = write_program("generic-boundary-policy", &source);
    let checked = compile_to_checked(&main_path, None).expect("generic policy instance compiles");
    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let conformance = checked
        .typed
        .data_conformances()
        .iter()
        .find(|conformance| conformance.type_name.as_str() == "TickProvider")
        .expect("TickProvider conformance");
    let arguments = checked
        .typed
        .type_reference_table
        .type_reference_handles(conformance.arguments);
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed_instance(
        &checked.typed,
        tick,
        arguments,
    )
    .expect("generic Tick service schema");

    assert_eq!(schema.methods.len(), 1);
    assert!(schema.methods[0].calling_plan_fingerprint.is_some());
    assert_eq!(
        omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, tick)
            .expect("uninstantiated schema")
            .methods[0]
            .calling_plan_fingerprint,
        None,
        "a generic declaration is not itself a concrete ABI"
    );
}

#[test]
fn uninstantiated_generic_boundary_does_not_publish_an_abi() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {",
        "boundary trait Tick<C>: Calling<C> {",
    );
    let main_path = write_program("uninstantiated-generic-boundary", &source);
    let checked = compile_to_checked(&main_path, None).expect("generic declaration compiles");
    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, tick)
        .expect("generic declaration schema");

    assert_eq!(schema.methods[0].calling_plan_fingerprint, None);
}

#[test]
fn source_policy_receives_recursive_fixed_array_and_record_shapes() {
    let source = r#"
use omega::language::std::calling;

data Pair {
    left: f32;
    right: f32;
}

data RecursiveShapePolicy { }

machine RecursiveShapePolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 2 {
        true -> bytes(signature, signature.parameters[0])
        _ -> wrong()
    }

    state bytes(signature: BoundarySignature, root: i64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::FixedArray { element, length } -> bytes_array(signature, root, element, length)
            _ -> wrong()
        }
    }

    state bytes_array(signature: BoundarySignature, root: i64, element: i64, length: i64) -> BoundaryPlanResult {
        transition length == 16 && signature.shapes[root].byte_size == 16 && signature.shapes[root].alignment == 1 {
            true -> bytes_element(signature, element)
            _ -> wrong()
        }
    }

    state bytes_element(signature: BoundarySignature, element: i64) -> BoundaryPlanResult {
        transition signature.shapes[element].class {
            ValueClass::Integer -> pairs(signature, signature.parameters[1])
            _ -> wrong()
        }
    }

    state pairs(signature: BoundarySignature, root: i64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::FixedArray { element, length } -> pair_array(signature, root, element, length)
            _ -> wrong()
        }
    }

    state pair_array(signature: BoundarySignature, root: i64, element: i64, length: i64) -> BoundaryPlanResult {
        transition length == 2 && signature.shapes[root].byte_size == 16 && signature.shapes[root].alignment == 4 {
            true -> pair_record(signature, element)
            _ -> wrong()
        }
    }

    state pair_record(signature: BoundarySignature, root: i64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> pair_fields(signature, first_field, field_count)
            _ -> wrong()
        }
    }

    state pair_fields(signature: BoundarySignature, first: i64, count: i64) -> BoundaryPlanResult {
        transition count == 2 && signature.fields[first].byte_offset == 0 && signature.fields[first + 1].byte_offset == 4 {
            true -> first_float(signature, signature.fields[first].shape, signature.fields[first + 1].shape)
            _ -> wrong()
        }
    }

    state first_float(signature: BoundarySignature, first: i64, second: i64) -> BoundaryPlanResult {
        transition signature.shapes[first].class {
            ValueClass::Float -> second_float(signature, second)
            _ -> wrong()
        }
    }

    state second_float(signature: BoundarySignature, second: i64) -> BoundaryPlanResult {
        transition signature.shapes[second].class {
            ValueClass::Float -> observed()
            _ -> wrong()
        }
    }

    state observed() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "recursive fixed-array/record shape observed",
            },
        }
    }

    state wrong() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "recursive shape mismatch",
            },
        }
    }
}

boundary trait Probe: Calling<RecursiveShapePolicy> {
    machine inspect(bytes: [u8; 16], pairs: [Pair; 2]);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("recursive-shape", source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("the observing policy deliberately rejects after checking its input graph");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("recursive fixed-array/record shape observed"),
        "policy did not observe the expected recursive boundary graph:\n{rendered}"
    );
    assert!(!rendered.contains("recursive shape mismatch"), "{rendered}");
}
