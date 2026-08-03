use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, EntryStack, MachineState, MachineStateSet,
    Preemption, ValueShape,
};
use omega_compiler::{
    compile_to_checked, evaluate_calling_policy_plan, selected_external_root_entry_fact_bindings,
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
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
use omega::language::core::interrupt;

data X86InterruptPolicy { }

machine X86InterruptPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 {
        true -> accept(signature, signature.parameters[0])
        _ -> reject()
    }

    state accept(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition root < 256 {
            true -> build(signature, root)
            _ -> reject()
        }
    }

    state build(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.convention = CallingConvention::SystemVAMD64;
        output.call.parameter_count = 1;
        output.call.parameters[0].shape.class = AbiValueClass::Integer;
        output.call.parameters[0].shape.byte_size = signature.shapes[root].byte_size;
        output.call.parameters[0].shape.alignment = signature.shapes[root].alignment;
        output.call.parameters[0].location_count = 1;
        output.call.parameters[0].locations[0] = ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: signature.shapes[root].byte_size,
            alignment: 8
        };
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

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "TimerRoot requires exactly one acknowledgement",
            },
        }
    }
}

data MaskProvider { }
MaskProvider satisfies InterruptMaskControl;

machine MaskProvider::save_and_mask(&mut self) -> InterruptMaskGuard in Active
    satisfies InterruptMaskControl::save_and_mask
    via Binding::CompilerIntrinsic("InterruptMaskControl::save_and_mask");

boundary trait LookalikeMaskControl {
    machine save(&mut self) -> InterruptMaskGuard in Active;
}

data LookalikeMaskProvider { }
LookalikeMaskProvider satisfies LookalikeMaskControl;

machine LookalikeMaskProvider::save(&mut self) -> InterruptMaskGuard in Active
    satisfies LookalikeMaskControl::save
    via Binding::CompilerIntrinsic("LookalikeMaskControl::save");

boundary trait TimerRoot: InterruptEntry + Calling<X86InterruptPolicy> {
}

boundary trait LookalikeEntry: Calling<X86InterruptPolicy> {
    machine enter(acknowledgement: InterruptAcknowledgement in Pending)
    reaches PortIo;
}

data TimerProvider { }
TimerProvider satisfies TimerRoot;

machine TimerProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)
    satisfies InterruptEntry::enter
    reaches PortIo
{
    acknowledgement.complete();
}

data LookalikeEntryProvider { }
LookalikeEntryProvider satisfies LookalikeEntry;

machine LookalikeEntryProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)
    satisfies LookalikeEntry::enter
    reaches PortIo
{
    acknowledgement.complete();
}

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
        &CallSignature {
            parameters: vec![ValueShape::integer(40, 8)],
            result: None,
        },
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
        .plan_by_name("TimerProvider::satisfies::TimerRoot")
        .expect("selected TimerRoot provider plan");
    let mask_plan = checked
        .selected_provider_plans()
        .plan_by_name("MaskProvider::satisfies::InterruptMaskControl")
        .expect("selected interrupt-mask provider plan");
    let [mask_save] = mask_plan.schema.methods.as_slice() else {
        panic!("mask provider must publish one save-and-mask requirement");
    };
    let [active] = mask_save.result_claims.as_slice() else {
        panic!("mask transition must publish one structured Active result claim");
    };
    assert_eq!(active.domain, "InterruptMaskGuard::Active");
    assert_eq!(
        active.effective_carry,
        omega_core::semantics::CarryPolicy::STRICT
    );
    let mut missing_active = mask_plan.clone();
    missing_active.schema.methods[0].result_claims.clear();
    assert_ne!(
        mask_plan.identity_fingerprint(),
        missing_active.identity_fingerprint(),
        "the mask provider receipt must bind its routed Active result claim"
    );
    let mask_selection = selected_external_root_provider_plan(
        checked.selected_provider_plans(),
        "InterruptMaskControl",
    )
    .expect("mask transition bridge should retain its selected provider plan");
    let runtime_active = mask_selection
        .result_claims(&mask_save.requirement_identity)
        .expect("exact Active result claim should lower into the runtime receipt contract");
    let [runtime_active] = runtime_active.as_slice() else {
        panic!("runtime mask bridge must retain one Active claim");
    };
    assert_eq!(runtime_active.provider_plan, mask_selection.identity);
    assert_eq!(
        runtime_active.requirement_identity,
        mask_save.requirement_identity
    );
    assert_eq!(runtime_active.domain, "InterruptMaskGuard::Active");
    let lookalike_plan = checked
        .selected_provider_plans()
        .plan_by_name("LookalikeMaskProvider::satisfies::LookalikeMaskControl")
        .expect("look-alike mask provider plan");
    let [lookalike_save] = lookalike_plan.schema.methods.as_slice() else {
        panic!("look-alike provider must publish one requirement");
    };
    assert!(
        lookalike_save.result_claims.is_empty(),
        "a requirement not named by the domain route must not publish Active issuance authority"
    );
    assert_eq!(selected.rows.len(), 1);
    let [entry] = selected.schema.methods.as_slice() else {
        panic!("TimerRoot must inherit one exact core entry requirement");
    };
    assert_eq!(entry.name, "enter");
    assert_eq!(entry.requirement_owner, "InterruptEntry");
    assert!(entry.requirement_identity.contains("InterruptEntry"));
    let [acknowledgement] = entry.parameter_type_identities.as_slice() else {
        panic!("timer root must bind its acknowledgement parameter identity");
    };
    assert!(acknowledgement.contains("InterruptAcknowledgement"));
    assert!(acknowledgement.contains("Pending"));
    let [pending] = entry.entry_claims.as_slice() else {
        panic!("timer root must publish one structured accepted authority claim");
    };
    assert_eq!(pending.parameter_index, 0);
    assert_eq!(pending.domain, "InterruptAcknowledgement::Pending");
    assert_eq!(
        pending.effective_carry,
        omega_core::semantics::CarryPolicy::STRICT
    );
    assert_eq!(
        pending.authority_flow,
        omega_effects::provider_plan::ServiceEntryAuthorityFlow::Accepts
    );
    let mut weakened = selected.clone();
    weakened.schema.methods[0].parameter_type_identities[0] = "InterruptAcknowledgement".to_owned();
    assert_ne!(
        selected.identity_fingerprint(),
        weakened.identity_fingerprint(),
        "the provider-plan identity carried into external-root admission must drift if Pending issuance is removed"
    );
    let mut unreported = selected.clone();
    unreported.schema.methods[0].entry_claims.clear();
    assert_ne!(
        selected.identity_fingerprint(),
        unreported.identity_fingerprint(),
        "the provider-plan receipt must bind the compiler-owned accepted-authority row"
    );
    let root_selection =
        selected_external_root_provider_plan(checked.selected_provider_plans(), "TimerRoot")
            .expect("external-root bridge should retain the qualified timer schema");
    assert_eq!(
        root_selection.identity.normalized_identity(),
        selected.identity_fingerprint()
    );
    assert_eq!(
        root_selection.schema.methods[0].parameter_type_identities,
        selected.schema.methods[0].parameter_type_identities,
        "the root bridge must carry the exact qualified source signature beside its receipt identity"
    );
    assert_eq!(
        root_selection.schema.methods[0].entry_claims, selected.schema.methods[0].entry_claims,
        "the root bridge must carry the structured accepted claim and strict carry policy beside its receipt identity"
    );
    let runtime_claims = root_selection
        .entry_claims(&entry.requirement_identity)
        .expect("exact timer entry claims should lower into the runtime ledger");
    let [runtime_pending] = runtime_claims.as_slice() else {
        panic!("runtime root bridge must retain one Pending claim");
    };
    assert_eq!(runtime_pending.parameter_index, 0);
    assert_eq!(runtime_pending.domain, "InterruptAcknowledgement::Pending");
    assert_eq!(
        runtime_pending.effective_carry,
        omega_core::semantics::CarryPolicy::STRICT
    );
    let entry_fact_bindings = selected_external_root_entry_fact_bindings(
        &checked,
        checked.selected_provider_plans(),
        "TimerRoot",
    )
    .expect("installed root must bind its accepted claim to one checked entry fact");
    let [entry_fact] = entry_fact_bindings.as_slice() else {
        panic!("TimerRoot must bind one checked Pending parameter fact");
    };
    assert_eq!(entry_fact.provider_plan(), root_selection.identity);
    assert_eq!(
        entry_fact.requirement_identity(),
        entry.requirement_identity
    );
    assert_eq!(entry_fact.parameter_index(), 0);
    assert_eq!(entry_fact.domain(), "InterruptAcknowledgement::Pending");
    assert_eq!(
        entry_fact.parameter_symbol(),
        checked.typed.state_parameters(
            checked
                .typed
                .machine_states(
                    checked
                        .typed
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str() == "TimerProvider::enter")
                        .expect("timer adapter"),
                )
                .first()
                .expect("timer entry state"),
        )[0]
        .symbol
    );
    assert_eq!(
        checked
            .facts
            .semantic
            .facts
            .get(entry_fact.checked_fact())
            .evidence
            .origin,
        omega_core::semantics::QualificationEvidenceOrigin::Propagated,
        "the checked adapter fact remains an ordinary parameter precondition until occurrence admission"
    );
    let mut drifted_checked = checked.clone();
    drifted_checked
        .facts
        .semantic
        .facts
        .get_mut(entry_fact.checked_fact())
        .evidence
        .origin = omega_core::semantics::QualificationEvidenceOrigin::CheckedTransformation;
    assert!(
        selected_external_root_entry_fact_bindings(
            &drifted_checked,
            drifted_checked.selected_provider_plans(),
            "TimerRoot",
        )
        .expect_err("a non-precondition fact must not satisfy installed-root entry binding")
        .0
        .contains("maps to 0 checked entry facts")
    );
    let lookalike_entry_plan = checked
        .selected_provider_plans()
        .plan_by_name("LookalikeEntryProvider::satisfies::LookalikeEntry")
        .expect("look-alike entry provider plan");
    let [lookalike_entry] = lookalike_entry_plan.schema.methods.as_slice() else {
        panic!("look-alike entry provider must publish one requirement");
    };
    assert!(
        lookalike_entry.entry_claims.is_empty(),
        "a qualified parameter whose requirement is not named by the domain route must remain an ordinary precondition"
    );
    assert!(
        selected_external_root_entry_fact_bindings(
            &checked,
            checked.selected_provider_plans(),
            "LookalikeEntry",
        )
        .expect("a look-alike root has no routed entry bindings")
        .is_empty()
    );
    assert_eq!(
        selected_external_root_provider_plan_id(checked.selected_provider_plans(), "TimerRoot")
            .expect("external-root bridge should retain the selected timer plan")
            .normalized_identity(),
        selected.identity_fingerprint()
    );
    let qualification = omega_visualizations::qualification_evidence_manifest_json(
        &checked,
        checked.selected_provider_plans(),
    );
    assert!(qualification.contains("\"boundary_authority_flow\": ["));
    assert!(qualification.contains("\"flow\": \"accepts\""));
    assert!(qualification.contains("\"flow\": \"returns\""));
    assert!(qualification.contains("\"boundary\": \"TimerRoot\""));
    assert!(
        qualification.contains("\"requirement\": \"InterruptEntry::enter\""),
        "qualification artifact must retain the inherited semantic requirement:\n{qualification}"
    );
    assert!(qualification.contains("\"parameter_index\": 0"));
    assert!(qualification.contains("\"domain\": \"InterruptAcknowledgement::Pending\""));
    assert!(qualification.contains("\"domain\": \"InterruptMaskGuard::Active\""));
    assert!(qualification.contains(
        "\"effective_carry\": {\"suspension\": \"forbidden\", \"cpu\": \"same\", \"thread\": \"same\", \"address\": \"stable\"}"
    ));
    assert!(qualification.contains(&format!(
        "\"receipt_identity\": \"0x{:016x}\"",
        selected.identity_fingerprint()
    )));
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
        .expect("typed semantic identity for the published boundary contract");
    assert_ne!(retained.fingerprint, 0);
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
fn full_width_unsigned_calling_values_are_not_reinterpreted_as_signed() {
    let source = r#"
use omega::language::std::calling;

data FullWidthPolicy { }
machine FullWidthPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.stack_alignment = 18446744073709551615;
    BoundaryPlanResult::Accepted { plan: output }
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("full-width-value", source);
    let checked =
        compile_to_checked(&main_path, None).expect("full-width u64 policy should compile");
    let error = evaluate_calling_policy_plan(
        &checked.typed,
        "FullWidthPolicy::plan",
        &CallSignature::default(),
    )
    .expect_err("the normalized u16 alignment must reject a full-width source value");

    assert!(
        error.contains("stack_alignment 18446744073709551615 is outside u16 range"),
        "unexpected diagnostic: {error}"
    );
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
        "boundary trait Tick<C>: Calling<C> {\n    machine tick(&mut self);\n}\n\ndata TickProvider { count: i64; }\nTickProvider satisfies Tick<NoResultPolicy>;\nmachine TickProvider::tick(&mut self) satisfies Tick<NoResultPolicy>::tick {\n    self.count = 1;\n}",
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

    state bytes(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::FixedArray { element, length } -> bytes_array(signature, root, element, length)
            _ -> wrong()
        }
    }

    state bytes_array(signature: BoundarySignature, root: u64, element: u64, length: u64) -> BoundaryPlanResult {
        transition length == 16 && signature.shapes[root].byte_size == 16 && signature.shapes[root].alignment == 1 {
            true -> bytes_element(signature, element)
            _ -> wrong()
        }
    }

    state bytes_element(signature: BoundarySignature, element: u64) -> BoundaryPlanResult {
        transition signature.shapes[element].class {
            ValueClass::Integer -> pairs(signature, signature.parameters[1])
            _ -> wrong()
        }
    }

    state pairs(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::FixedArray { element, length } -> pair_array(signature, root, element, length)
            _ -> wrong()
        }
    }

    state pair_array(signature: BoundarySignature, root: u64, element: u64, length: u64) -> BoundaryPlanResult {
        transition length == 2 && signature.shapes[root].byte_size == 16 && signature.shapes[root].alignment == 4 {
            true -> pair_record(signature, element)
            _ -> wrong()
        }
    }

    state pair_record(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> pair_fields(signature, first_field, field_count)
            _ -> wrong()
        }
    }

    state pair_fields(signature: BoundarySignature, first: u64, count: u64) -> BoundaryPlanResult {
        transition count == 2 && signature.fields[first].byte_offset == 0 && signature.fields[first + 1].byte_offset == 4 {
            true -> first_float(signature, signature.fields[first].shape, signature.fields[first + 1].shape)
            _ -> wrong()
        }
    }

    state first_float(signature: BoundarySignature, first: u64, second: u64) -> BoundaryPlanResult {
        transition signature.shapes[first].class {
            ValueClass::Float -> second_float(signature, second)
            _ -> wrong()
        }
    }

    state second_float(signature: BoundarySignature, second: u64) -> BoundaryPlanResult {
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
