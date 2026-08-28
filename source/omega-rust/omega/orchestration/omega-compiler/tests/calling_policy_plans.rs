use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, EntryStack, MachineState, MachineStateSet,
    Preemption, ValueShape,
};
use omega_compiler::{
    CompileOptions, PROGRAM_STORAGE_INSTALLATION_ARTIFACT, ProgramStorageEntryBridgeError,
    ProgramStorageEntryInitialStorageAuthorityKind,
    ProgramStorageEntryRecordedWholeRootArgumentRecovery, ProgramStorageInstallationHandoffError,
    ProgramStorageRootInput, SelectedExternalRootProviderPlan, SelectedProgramStorageEntryPlan,
    bind_emitted_program_storage_entry_native_bridge,
    bind_program_storage_entry_emitted_whole_root_arguments, bind_program_storage_entry_plan,
    bind_program_storage_entry_whole_root_arguments,
    bind_program_storage_entry_whole_root_logical_values,
    bind_program_storage_entry_whole_root_operands,
    bind_recorded_program_storage_entry_whole_root_arguments, compile_to_checked,
    evaluate_calling_policy_plan, install_program_storage_entry_provider_invocation,
    plan_program_storage_entry_wrapper_caller_frame, program_storage_installation_record_json,
    reserve_program_storage_entry_outgoing_stack_frame, selected_external_root_entry_fact_bindings,
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
};

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    let requested_product = if options.write_output {
        omega_compiler::RequestedCompileProduct::InstalledOutput
    } else {
        omega_compiler::RequestedCompileProduct::Check
    };
    omega_compiler::compile(
        omega_compiler::CompileRequest::new(options).with_requested_product(requested_product),
    )
}

use omega_instruction_selection::derive_boundary_entry_storage;
use psi_extents::{
    AddressSpaceId, ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRights,
    ExtentRootGrant, MappingEraId,
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

fn compile_std_negative(name: &str, source: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .join("source/library/std/tests")
        .join(format!(".callback-{name}-{}.omg", std::process::id()));
    fs::write(&path, source).expect("write hermetic callback source canary");
    let result = compile_to_checked(&path, Some("windows_x64"));
    fs::remove_file(&path).expect("remove hermetic callback source canary");
    result
        .expect_err("negative callback source canary must reject")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn selected_plan_for_external_root<'a>(
    facts: &'a omega_effects::SelectedProviderPlanFacts,
    trait_name: &str,
) -> &'a omega_effects::provider_plan::ProviderPlan {
    let identity = selected_external_root_provider_plan_id(facts, trait_name)
        .unwrap_or_else(|error| panic!("selected `{trait_name}` provider plan: {error}"));
    facts
        .plan_by_identity(identity.normalized_identity())
        .unwrap_or_else(|| {
            panic!(
                "selected `{trait_name}` provider identity {:#018x} must address an exact retained plan",
                identity.normalized_identity()
            )
        })
}

const POLICY: &str = r#"
use omega::language::std::calling;

data NoResultPolicy { }
NoResultPolicyCallingPolicy: NoResultPolicy satisfies CallingPolicy;

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

const CALLBACK_MATERIALIZATION_POLICY: &str = r#"
target windows_x64 {
}

use omega::language::core::layout;
use omega::language::std::calling;

boundary trait WindowProcedure {
    machine call(message: u64) -> u64;
}

boundary trait UnusedProcedure {
    machine call(message: u64) -> u64;
}

data Spread {
    entries: [FieldEntry; 64];
}

WndClassWindowProcedureSlot:
    Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;

SecondaryWndClassWindowProcedureSlot:
    Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;

UnusedWindowProcedureSlot:
    Spread satisfies PrivateCallbackSlot<UnusedProcedure::call>;

machine Spread::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    let plan: Plan = Plan {
        entries: self.entries,
        entry_count: 1,
        size_fixed: 24,
        size_is_dynamic: false,
        align: 8,
    };
    let placed: Plan =
        Plan::place_private<WndClassWindowProcedureSlot>(plan, 8);
    Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)
}

data ForeignRecord {
    payload: u64;
}

data RegistrarPolicy { }
RegistrarPolicyCallingPolicy: RegistrarPolicy satisfies CallingPolicy;

machine RegistrarPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 {
        true -> select_catalog(signature)
        _ -> reject()
    }

    state select_catalog(signature: BoundarySignature) -> BoundaryPlanResult {
        transition signature.callback_demand_count == 0 {
            true -> check_root(signature, signature.parameters[0])
            _ -> select_two_demands(signature)
        }
    }

    state select_two_demands(signature: BoundarySignature) -> BoundaryPlanResult {
        transition signature.callback_demand_count == 2 {
            true -> check_root(signature, signature.parameters[0])
            _ -> reject()
        }
    }

    state check_root(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition root < 256 {
            true -> build(signature, root)
            _ -> reject()
        }
    }

    state build(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.convention = CallingConvention::MicrosoftX64;
        output.call.parameter_count = 1;
        output.call.parameters[0].shape.class = AbiValueClass::Integer;
        output.call.parameters[0].shape.byte_size = signature.shapes[root].byte_size;
        output.call.parameters[0].shape.alignment = signature.shapes[root].alignment;
        output.call.parameters[0].location_count = 1;
        output.call.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::X86Rcx,
            value_byte_offset: 0,
            byte_size: signature.shapes[root].byte_size,
        };
        output.call.stack_alignment = 16;
        output.call.shadow_bytes = 32;
        output.call.entry_control = EntryControl::CallReturn;
        output.state.initial_regime = MachineRegime::X86Long64;
        output.state.stack = EntryStack::ProviderSelected;
        output.state.preemption = Preemption::NotApplicable;
        output.call.callback_materialization_count = signature.callback_demand_count;
        transition signature.callback_demand_count == 2 {
            true -> bind_callback(signature, output)
            _ -> accept(output)
        }
    }

    state bind_callback(
        signature: BoundarySignature,
        output: BoundaryEntryPlan
    ) -> BoundaryPlanResult {
        let mut bound: BoundaryEntryPlan = output;
        bound.call.callback_materializations[0].binder = signature.callback_binders[0].binder;
        bound.call.callback_materializations[0].destination =
            signature.callback_demands[0].destination;
        bound.call.callback_materializations[1].binder = signature.callback_binders[1].binder;
        bound.call.callback_materializations[1].destination =
            signature.callback_demands[1].destination;
        BoundaryPlanResult::Accepted { plan: bound }
    }

    state accept(output: BoundaryEntryPlan) -> BoundaryPlanResult {
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection { reason: "invalid callback catalog" },
        }
    }
}

boundary trait WindowRegistrar: Calling<RegistrarPolicy> {
    machine register<machine Selected, machine SecondarySelected>(
        specification: &Spread<ForeignRecord>
    )
    where machine Selected satisfies WindowProcedure::call;
    where machine SecondarySelected satisfies WindowProcedure::call;
}

data Main { }
machine Main::main(&mut self) { }
"#;

#[test]
fn target_selected_callback_policy_consumes_two_closed_layout_demands() {
    let main_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .join("source/library/std/tests/callback_materialization_closure.omg");
    assert_eq!(
        fs::read_to_string(&main_path).unwrap(),
        CALLBACK_MATERIALIZATION_POLICY.trim_start_matches('\n'),
        "the source canary and its readable test fixture must remain identical"
    );
    compile_to_checked(&main_path, Some("windows_x64"))
        .expect("target-selected registrar should consume both exact closed layout demands");
}

#[test]
fn callback_private_materialization_requires_an_explicit_cited_demand() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "    let placed: Plan =\n        Plan::place_private<WndClassWindowProcedureSlot>(plan, 8);\n    Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "    plan",
    );
    let main_path = write_program("callback-uncited-demand", &source);
    let diagnostics = compile_to_checked(&main_path, Some("windows_x64"))
        .expect_err("a nominal callback binder cannot assume an uncited private demand");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("omits a nominal callback binder"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_a_foreign_layout_subject() {
    let source = CALLBACK_MATERIALIZATION_POLICY
        .replace(
            "data Spread {\n    entries: [FieldEntry; 64];\n}",
            "data Spread {\n    entries: [FieldEntry; 64];\n}\n\ndata OtherSpread {\n    entries: [FieldEntry; 64];\n}",
        )
        .replace(
            "Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;",
            "OtherSpread satisfies PrivateCallbackSlot<WindowProcedure::call>;",
        );
    let main_path = write_program("callback-wrong-layout", &source);
    let diagnostics = compile_to_checked(&main_path, Some("windows_x64"))
        .expect_err("a private slot cannot be cited by a foreign layout policy");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("active layout producer") && rendered.contains("OtherSpread"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_an_ambiguous_requirement_path() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "boundary trait WindowProcedure {\n    machine call(message: u64) -> u64;\n}",
        "boundary trait WindowProcedure {\n    machine call(message: u64) -> u64;\n    machine call(message: i64) -> u64;\n}",
    );
    let main_path = write_program("callback-ambiguous-requirement", &source);
    let diagnostics = compile_to_checked(&main_path, Some("windows_x64"))
        .expect_err("a signature-free callback requirement must resolve uniquely");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("overloads requirement `call`")
            && rendered.contains("WindowProcedure::call"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_the_wrong_callback_requirement() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "WndClassWindowProcedureSlot:\n    Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;",
        "WndClassWindowProcedureSlot:\n    Spread satisfies PrivateCallbackSlot<UnusedProcedure::call>;",
    );
    let rendered = compile_std_negative("wrong-requirement", &source);

    assert!(
        rendered.contains(
            "callback materialization binder and native-place demand require different callback contracts"
        ),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_a_raw_physical_offset_as_identity() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "        bound.call.callback_materializations[0].destination =\n            signature.callback_demands[0].destination;",
        "        let mut invented_path: [u64; 16];\n        invented_path[0] = 8;\n        bound.call.callback_materializations[0].destination = NativePlace::Field {\n            parameter: 8,\n            layout: 8,\n            field_path: invented_path,\n            field_path_count: 1,\n        };",
    );
    let rendered = compile_std_negative("raw-offset", &source);

    assert!(
        rendered.contains("does not name a declared private native-place demand"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_duplicate_source_placement() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "Plan::place_private<WndClassWindowProcedureSlot>(placed, 16)",
    );
    let rendered = compile_std_negative("duplicate-placement", &source);

    assert!(
        rendered.contains("duplicate private callback placement")
            || rendered.contains("more than once"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_overlapping_named_slots() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 8)",
    );
    let rendered = compile_std_negative("overlapping-slots", &source);

    assert!(
        rendered.contains("private callback slots") && rendered.contains("overlap"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_is_absent_from_semantic_projection() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "data ForeignRecord {",
        "machine Spread::read_private_slot(&mut self) -> u64 {\n    self.WndClassWindowProcedureSlot\n}\n\ndata ForeignRecord {",
    );
    let rendered = compile_std_negative("semantic-projection", &source);

    assert!(
        rendered.contains("Spread")
            && rendered.contains("has no field")
            && rendered.contains("WndClassWindowProcedureSlot"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_is_absent_from_semantic_assignment() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "data ForeignRecord {",
        "machine Spread::write_private_slot(&mut self) {\n    self.WndClassWindowProcedureSlot = 0;\n}\n\ndata ForeignRecord {",
    );
    let rendered = compile_std_negative("semantic-assignment", &source);

    assert!(
        rendered.contains("Spread")
            && rendered.contains("has no field")
            && rendered.contains("WndClassWindowProcedureSlot"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_a_machine_as_slot_identity() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "Plan::place_private<WndClassWindowProcedureSlot>(plan, 8)",
        "Plan::place_private<WindowProcedure::call>(plan, 8)",
    );
    let rendered = compile_std_negative("machine-as-slot", &source);

    assert!(
        rendered.contains(
            "static argument to `Plan::place_private` must resolve exactly to one named conformance"
        ),
        "unexpected diagnostics:\n{rendered}"
    );
}

const INTERRUPT_POLICY: &str = r#"
use omega::language::std::calling;
use omega::language::core::interrupt;

data X86InterruptPolicy { }
X86InterruptPolicyCallingPolicy: X86InterruptPolicy satisfies CallingPolicy;

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
MaskProviderInterruptMaskControl: MaskProvider satisfies InterruptMaskControl;

machine MaskProvider::save_and_mask(&mut self) -> InterruptMaskGuard in Active
    satisfies InterruptMaskControl::save_and_mask
    via Binding::CompilerIntrinsic;

boundary trait LookalikeMaskControl {
    machine save(&mut self) -> InterruptMaskGuard in Active;
}

data LookalikeMaskProvider { }
LookalikeMaskProviderLookalikeMaskControl: LookalikeMaskProvider satisfies LookalikeMaskControl;

machine LookalikeMaskProvider::save(&mut self) -> InterruptMaskGuard in Active
    satisfies LookalikeMaskControl::save
    via Binding::CompilerIntrinsic;

boundary trait TimerRoot: InterruptEntry + Calling<X86InterruptPolicy> {
}

boundary trait LookalikeEntry: Calling<X86InterruptPolicy> {
    machine enter(acknowledgement: InterruptAcknowledgement in Pending)
    reaches PortIo;
}

data TimerProvider { }
TimerProviderTimerRoot: TimerProvider satisfies TimerRoot;

machine TimerProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)
    satisfies InterruptEntry::enter
    reaches PortIo
{
    acknowledgement.complete();
}

data LookalikeEntryProvider { }
LookalikeEntryProviderLookalikeEntry: LookalikeEntryProvider satisfies LookalikeEntry;

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
    let selected = selected_plan_for_external_root(checked.selected_provider_plans(), "TimerRoot");
    assert_eq!(selected.name, "TimerProvider::satisfies::TimerRoot");
    let mask_plan =
        selected_plan_for_external_root(checked.selected_provider_plans(), "InterruptMaskControl");
    assert_eq!(
        mask_plan.name,
        "MaskProvider::satisfies::InterruptMaskControl"
    );
    let [mask_save] = mask_plan.schema.methods.as_slice() else {
        panic!("mask provider must publish one save-and-mask requirement");
    };
    let [active] = mask_save.result_claims.as_slice() else {
        panic!("mask transition must publish one structured Active result claim");
    };
    assert_eq!(active.domain, "InterruptMaskGuard::Active");
    assert_eq!(
        active.effective_carry,
        psi_language_semantics::CarryPolicy::STRICT
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
    let lookalike_plan =
        selected_plan_for_external_root(checked.selected_provider_plans(), "LookalikeMaskControl");
    assert_eq!(
        lookalike_plan.name,
        "LookalikeMaskProvider::satisfies::LookalikeMaskControl"
    );
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
    let entry_reach = checked
        .selected_provider_plans()
        .installation_reach_resolution(&entry.requirement_identity)
        .expect("the installed interrupt entry must resolve its bounded reach row");
    assert_eq!(
        entry_reach.upper_bound,
        ["MachineControl".to_owned(), "PortIo".to_owned()]
    );
    assert_eq!(
        entry_reach.resolved_row,
        ["PortIo".to_owned()],
        "the PIC-shaped test provider refines the conservative hardware ceiling to PortIo"
    );
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
        pending.predicate_body,
        psi_language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(
        pending.effective_carry,
        psi_language_semantics::CarryPolicy::STRICT
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
        psi_language_semantics::CarryPolicy::STRICT
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
        psi_language_semantics::QualificationEvidenceOrigin::Propagated,
        "the checked adapter fact remains an ordinary parameter precondition until occurrence admission"
    );
    let mut drifted_checked = checked.clone();
    drifted_checked
        .facts
        .semantic
        .facts
        .get_mut(entry_fact.checked_fact())
        .evidence
        .origin = psi_language_semantics::QualificationEvidenceOrigin::CheckedTransformation;
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
    let lookalike_entry_plan =
        selected_plan_for_external_root(checked.selected_provider_plans(), "LookalikeEntry");
    assert_eq!(
        lookalike_entry_plan.name,
        "LookalikeEntryProvider::satisfies::LookalikeEntry"
    );
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
    let json_identity = |identity: &str| identity.replace('\\', "\\\\").replace('"', "\\\"");
    assert!(qualification.contains(&format!(
        "\"requirement_identity\": \"{}\"",
        json_identity(&entry.requirement_identity)
    )));
    assert!(qualification.contains(&format!(
        "\"requirement_identity\": \"{}\"",
        json_identity(&mask_save.requirement_identity)
    )));
    let authority_flow = qualification
        .split_once("\"boundary_authority_flow\": [")
        .expect("qualification artifact publishes boundary authority rows")
        .1;
    let entry_identity = format!(
        "\"requirement_identity\": \"{}\"",
        json_identity(&entry.requirement_identity)
    );
    let entry_identity_offset = authority_flow
        .find(&entry_identity)
        .expect("authority rows retain the exact inherited entry identity");
    let entry_row_start = authority_flow[..entry_identity_offset]
        .rfind("\n    {")
        .expect("entry authority row begins before its identity");
    let entry_row_end = authority_flow[entry_identity_offset..]
        .find("\n    }")
        .map(|end| entry_identity_offset + end)
        .expect("entry authority row ends after its identity");
    let entry_row = &authority_flow[entry_row_start..entry_row_end];
    assert!(entry_row.contains("\"boundary\": \"TimerRoot\""));
    assert!(entry_row.contains("\"requirement_owner\": \"InterruptEntry\""));
    assert!(entry_row.contains(&format!(
        "\"receipt_identity\": \"0x{:016x}\"",
        selected.identity_fingerprint()
    )));
    assert!(
        !authority_flow.contains(&format!(
            "\"requirement_identity\": \"{}\"",
            json_identity(&lookalike_entry.requirement_identity)
        )),
        "the unrelated look-alike requirement must not acquire an authority row"
    );
    assert!(qualification.contains("\"predicate_body\": \"bodyless\""));
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
fn program_storage_entry_publishes_both_core_owned_root_positions() {
    let main_path = write_program(
        "program-storage-entry",
        include_str!(
            "../../../../../../tests/canaries/pass/build/uefi_program_entry_storage_roots/main.omg"
        ),
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("the core program-storage entry requirement should compile");
    let entry = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "UefiApplication")
        .expect("UEFI application entry trait");
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, entry)
        .expect("program-storage entry schema");
    let method = schema
        .methods
        .iter()
        .find(|method| method.requirement_owner == "ProgramStorageEntry")
        .expect("entry schema retains the semantic storage requirement");
    let physical = schema
        .methods
        .iter()
        .find(|method| method.requirement_owner == "UefiPhysicalEntry")
        .expect("entry schema retains the distinct physical UEFI requirement");
    assert_eq!(schema.methods.len(), 2);
    assert_eq!(physical.name, "enter");
    assert_eq!(physical.parameter_type_identities.len(), 2);
    assert!(physical.parameter_type_identities[0].contains("EfiImageHandle"));
    assert!(physical.parameter_type_identities[1].contains("EfiSystemTable"));
    assert!(
        physical
            .result_type_identity
            .as_deref()
            .is_some_and(|identity| identity.contains("EfiStatus"))
    );
    assert_ne!(physical.requirement_identity, method.requirement_identity);
    assert_eq!(method.requirement_owner, "ProgramStorageEntry");
    assert_eq!(method.name, "enter");
    assert_eq!(method.parameter_type_identities.len(), 2);
    for identity in &method.parameter_type_identities {
        assert!(
            identity.contains("Extent"),
            "missing Extent carrier: {identity}"
        );
        assert!(
            identity.contains("Granted"),
            "missing exact Granted qualification: {identity}"
        );
    }
    assert_eq!(method.entry_claims.len(), 2);
    assert_eq!(method.entry_claims[0].parameter_index, 0);
    assert_eq!(method.entry_claims[1].parameter_index, 1);
    assert!(
        method
            .entry_claims
            .iter()
            .all(|claim| claim.domain == "Extent::Granted")
    );
    assert!(
        method
            .entry_claims
            .iter()
            .all(|claim| claim.predicate_body.is_present())
    );
    let requirement_identity = method.requirement_identity.clone();

    let shape = ValueShape::integer(16, 8);
    let boundary = evaluate_calling_policy_plan(
        &checked.typed,
        "UefiX86_64::plan",
        &CallSignature {
            parameters: vec![shape, shape],
            result: None,
        },
    )
    .expect("UEFI program-entry plan");
    let storage =
        derive_boundary_entry_storage(boundary.plan(), &[(0, shape), (16, shape)], None, None)
            .expect("generated process-entry captures");
    let selected_schema = schema;
    let selected_provider = SelectedExternalRootProviderPlan {
        identity: omega_external_roots::ProviderPlanId::from_normalized_identity(90)
            .expect("selected provider identity"),
        schema: selected_schema.clone(),
    };
    let generic_error = selected_provider
        .entry_claims(&requirement_identity)
        .expect_err("predicate-bearing roots require their specialized installer");
    assert!(generic_error.0.contains("predicate obligations"));
    let hosted_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::MacosArm64.program_entry_slot(),
        selected_schema.clone(),
        requirement_identity.clone(),
    )
    .expect_err("hosted entries cannot claim source-visible program-storage roots");
    assert!(
        hosted_error
            .0
            .contains("exact program-storage entry contract")
    );
    let mut mismatched_schema = selected_schema.clone();
    mismatched_schema.trait_name = "OtherApplication".into();
    let mismatched_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        mismatched_schema,
        requirement_identity.clone(),
    )
    .expect_err("a target slot cannot accept a different boundary schema");
    assert!(mismatched_error.0.contains("requires boundary schema"));
    let empty_identity_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        selected_schema.clone(),
        String::new(),
    )
    .expect_err("an arrival requirement needs exact identity");
    assert!(
        empty_identity_error
            .0
            .contains("no exact arrival requirement identity")
    );
    let wrong_identity_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        selected_schema.clone(),
        format!("{requirement_identity}#lookalike"),
    )
    .expect_err("a lookalike requirement identity cannot select the arrival slot");
    assert!(
        wrong_identity_error
            .0
            .contains("0 copies of exact arrival requirement")
    );
    let mut duplicate_schema = selected_schema.clone();
    duplicate_schema
        .methods
        .push(duplicate_schema.methods[0].clone());
    let duplicate_identity_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        duplicate_schema,
        requirement_identity.clone(),
    )
    .expect_err("duplicate exact arrival identities must reject");
    assert!(
        duplicate_identity_error
            .0
            .contains("2 copies of exact arrival requirement")
    );
    let mut owner_drift_schema = selected_schema.clone();
    owner_drift_schema.methods[0].requirement_owner = "LookalikeOwner".into();
    let owner_drift_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        owner_drift_schema,
        requirement_identity.clone(),
    )
    .expect_err("the exact identity cannot launder readable owner drift");
    assert!(
        owner_drift_error
            .0
            .contains("drifted from `ProgramStorageEntry::enter`")
    );
    let mut name_drift_schema = selected_schema.clone();
    name_drift_schema.methods[0].name = "lookalike_enter".into();
    let name_drift_error = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        name_drift_schema,
        requirement_identity.clone(),
    )
    .expect_err("the exact identity cannot launder readable method drift");
    assert!(
        name_drift_error
            .0
            .contains("drifted from `ProgramStorageEntry::enter`")
    );
    let mut overloaded_schema = selected_schema;
    let mut same_name_overload = overloaded_schema.methods[0].clone();
    same_name_overload.requirement_identity = format!("{requirement_identity}#other-overload");
    overloaded_schema.methods.push(same_name_overload);
    let selected = SelectedProgramStorageEntryPlan::from_target_slot(
        omega_target::TargetProfile::UefiX64.program_entry_slot(),
        overloaded_schema,
        requirement_identity.clone(),
    )
    .expect("selected target root slot");
    assert_eq!(selected.requirement_identity(), requirement_identity);
    let binding = bind_program_storage_entry_plan(&selected, &boundary, &storage)
        .expect("stable storage positions should bind to selected ABI captures");
    assert_eq!(binding.root_slot(), selected.root_slot());
    assert_eq!(binding.image().parameter_index(), 0);
    assert_eq!(binding.initial_storage().parameter_index(), 1);
    assert_eq!(
        binding.image().placement(),
        &boundary.plan().call.parameters[0]
    );
    assert_eq!(
        binding.initial_storage().placement(),
        &boundary.plan().call.parameters[1]
    );

    let artifact_directory = main_path
        .parent()
        .expect("temporary policy directory")
        .join("completed-installation");
    let (image_input, provider_issuance) =
        root_input_for_provider_invocation(91, 0x1000, 0x800, 90, 901);
    let (storage_input, _) = root_input_for_provider_invocation(92, u64::MAX, 2, 90, 901);
    let failed = install_program_storage_entry_provider_invocation(
        &artifact_directory,
        binding,
        &selected_provider,
        provider_issuance,
        image_input,
        storage_input,
    )
    .expect_err("both no-wrap predicates must precede either fact import");
    let ProgramStorageInstallationHandoffError::Rejected(failed) = failed else {
        panic!("invalid geometry must reject before record emission")
    };
    assert!(failed.diagnostic().0.contains("initial-storage"));
    let (binding, image, storage_input) = failed.into_parts();
    let recorded = install_program_storage_entry_provider_invocation(
        &artifact_directory,
        binding,
        &selected_provider,
        provider_issuance,
        image,
        storage_input.with_geometry(0x8000, 0x2000),
    )
    .expect("returned grants remain usable after rejected geometry");
    let installed = recorded
        .into_roots()
        .expect("free entry installation has no receiver activation");
    assert_eq!(
        (installed.image().base(), installed.image().length()),
        (0x1000, 0x800)
    );
    assert_eq!(
        (
            installed
                .initial_storage()
                .expect("free entry retains whole initial storage")
                .base(),
            installed
                .initial_storage()
                .expect("free entry retains whole initial storage")
                .length()
        ),
        (0x8000, 0x2000)
    );
    let installation = installed.installation_record();
    assert_eq!(installation.binding().root_slot(), selected.root_slot());
    assert_eq!(
        (installation.image().base(), installation.image().length()),
        (0x1000, 0x800)
    );
    assert_eq!(
        (
            installation.initial_storage().base(),
            installation.initial_storage().length()
        ),
        (0x8000, 0x2000)
    );
    assert_eq!(installation.image().end(), 0x1800);
    assert_eq!(installation.initial_storage().end(), 0xa000);
    assert_eq!(
        installation.image().address_space().normalized_identity(),
        100
    );
    assert!(installation.image().rights().is_empty());
    assert_eq!(installation.image().provenance().normalized_identity(), 101);
    assert_eq!(
        installation.image().mapping_era().normalized_identity(),
        102
    );
    let image_issuance = installation
        .image()
        .provider_issuance()
        .expect("program storage is provider issued");
    assert_eq!(image_issuance.issuance().normalized_identity(), 91 * 16 + 1);
    assert_eq!(image_issuance.backing().normalized_identity(), 91 * 16 + 2);
    assert_eq!(image_issuance.provider().normalized_identity(), 1090);
    assert_eq!(
        image_issuance.live_issuance_premise().normalized_identity(),
        91 * 16 + 4
    );
    assert_eq!(
        image_issuance.custody_root().normalized_identity(),
        91 * 16 + 5
    );
    assert_eq!(
        image_issuance.alias_class().normalized_identity(),
        91 * 16 + 6
    );
    assert_eq!(
        image_issuance.correspondence().normalized_identity(),
        91 * 16 + 7
    );
    assert_eq!(
        image_issuance.trust_provenance().normalized_identity(),
        91 * 16 + 8
    );
    let invocation = image_issuance.invocation();
    assert_eq!(invocation.provider_plan().normalized_identity(), 90);
    assert_eq!(invocation.invocation().normalized_identity(), 901);
    assert_eq!(
        invocation.establishment_route().normalized_identity(),
        91 * 16 + 11
    );
    assert_eq!(invocation.capacity().normalized_identity(), 91 * 16 + 12);
    assert_eq!(
        invocation.qualification().normalized_identity(),
        91 * 16 + 13
    );
    assert_eq!(
        installation.image().lineage_root().normalized_identity(),
        91
    );
    assert_eq!(
        installation
            .initial_storage()
            .lineage_root()
            .normalized_identity(),
        92
    );

    let provider_json = program_storage_installation_record_json(&installation);
    assert!(provider_json.contains("\"authority\": \"non_authoritative_audit_record\""));
    assert!(provider_json.contains("\"installation_status\": \"completed\""));
    assert!(provider_json.contains("\"role\": \"image\", \"parameter_index\": 0"));
    assert!(provider_json.contains("\"base\": \"0x0000000000001000\""));
    assert!(provider_json.contains("\"kind\": \"provider_issued\""));
    assert!(provider_json.contains("\"provider_plan\": \"0x000000000000005a\""));
    assert!(provider_json.contains("\"qualification\": \"0x00000000000005bd\""));

    let emitted =
        fs::read_to_string(artifact_directory.join(PROGRAM_STORAGE_INSTALLATION_ARTIFACT))
            .expect("read completed installation artifact");
    assert_eq!(emitted, provider_json);

    let receiver_binding = installation
        .binding()
        .clone()
        .with_checked_receiver_layout(
            "&mut Boot".into(),
            omega_layout::TypeLayout {
                size: 8,
                alignment: 8,
            },
        )
        .expect("checked receiver layout should attach to the selected entry");
    let receiver_artifact_directory = main_path
        .parent()
        .expect("temporary policy directory")
        .join("receiver-completed-installation");
    let (receiver_image, receiver_issuance) =
        root_input_for_provider_invocation(401, 0x4000, 0x400, 90, 905);
    let (receiver_storage, _) = root_input_for_provider_invocation(402, 0x8003, 11, 90, 905);
    let rejected = install_program_storage_entry_provider_invocation(
        &receiver_artifact_directory,
        receiver_binding,
        &selected_provider,
        receiver_issuance,
        receiver_image,
        receiver_storage,
    )
    .expect_err("receiver alignment and capacity must validate before grant consumption");
    let ProgramStorageInstallationHandoffError::Rejected(rejected) = rejected else {
        panic!("receiver capacity failure must precede record emission")
    };
    assert!(rejected.diagnostic().0.contains("cannot reserve"));
    let (receiver_binding, image_input, storage_input) = rejected.into_parts();
    let receiver_installation = install_program_storage_entry_provider_invocation(
        &receiver_artifact_directory,
        receiver_binding,
        &selected_provider,
        receiver_issuance,
        image_input,
        storage_input.with_geometry(0x8003, 0x20),
    )
    .expect("returned grants should install after receiver capacity is corrected");
    let receiver_record = receiver_installation.installation_record();
    let receiver = receiver_record
        .receiver()
        .expect("completed record should retain receiver placement");
    assert_eq!((receiver.base(), receiver.length()), (0x8008, 8));
    assert_eq!(receiver.initial_storage_offset(), 5);
    assert_eq!(
        receiver.lineage_root(),
        receiver_record.initial_storage().lineage_root()
    );
    let receiver_json = program_storage_installation_record_json(&receiver_record);
    assert!(receiver_json.contains("\"status\": \"reserved\""));
    assert!(receiver_json.contains("\"initialization\": \"bridge_required\""));
    assert!(receiver_json.contains("\"base\": \"0x0000000000008008\""));
    let receiver_release = receiver_installation
        .into_roots()
        .expect_err("receiver roots cannot bypass ZII activation");
    assert!(
        receiver_release
            .diagnostic()
            .0
            .contains("must be zeroed and activated")
    );
    let receiver_installation = receiver_release.into_installation();
    let mut receiver_bytes = [0xa5; 8];
    let wrong_mapping = receiver_installation
        .activate_receiver(0x8007, &mut receiver_bytes)
        .expect_err("receiver activation must bind the exact reserved base");
    assert!(wrong_mapping.diagnostic().0.contains("exactly cover"));
    assert_eq!(receiver_bytes, [0xa5; 8]);
    let receiver_installation = wrong_mapping.into_installation();
    let mut short_receiver_bytes = [0xa5; 7];
    let wrong_mapping = receiver_installation
        .activate_receiver(0x8008, &mut short_receiver_bytes)
        .expect_err("receiver activation must bind the exact reserved length");
    assert!(wrong_mapping.diagnostic().0.contains("exactly cover"));
    assert_eq!(short_receiver_bytes, [0xa5; 7]);
    let receiver_installation = wrong_mapping.into_installation();
    let mut receiver_bytes = [0xa5; 8];
    let mut activation = receiver_installation
        .activate_receiver(0x8008, &mut receiver_bytes)
        .expect("exact receiver mapping should construct one ZII activation");
    assert_eq!(
        (
            activation.placement().base(),
            activation.placement().length()
        ),
        (0x8008, 8)
    );
    assert_eq!(activation.receiver(), &[0; 8]);
    activation.receiver()[3] = 70;
    assert_eq!(activation.receiver()[3], 70);
    let receiver_roots = activation.finish();
    assert!(receiver_roots.initial_storage().is_none());
    let receiver_storage = receiver_roots
        .receiver_storage()
        .expect("nonzero receiver should hold a conserved initial-storage partition");
    assert_eq!(
        receiver_storage
            .before()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x8003, 5))
    );
    assert_eq!(
        receiver_storage
            .storage()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x8008, 8))
    );
    assert_eq!(
        receiver_storage
            .after()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x8010, 0x13))
    );
    let physical_binding = receiver_roots.binding().clone();
    let reserved_partition = receiver_roots
        .partition_initial_storage(0, 1)
        .expect_err("a receiver reservation must hide the original whole root");
    assert!(
        reserved_partition
            .diagnostic()
            .0
            .contains("already reserves")
    );

    let physical_artifact_directory = main_path
        .parent()
        .expect("temporary policy directory")
        .join("physical-provider-installation");
    let (unselected_image, unselected_issuance) =
        root_input_for_provider_invocation(507, 0x5000, 0x400, 90, 903);
    let (unselected_storage, _) = root_input_for_provider_invocation(508, 0x9003, 0x20, 90, 903);
    let unselected_provider = SelectedExternalRootProviderPlan {
        identity: omega_external_roots::ProviderPlanId::from_normalized_identity(91)
            .expect("different selected provider identity"),
        schema: selected_provider.schema.clone(),
    };
    let unselected = install_program_storage_entry_provider_invocation(
        &physical_artifact_directory,
        physical_binding.clone(),
        &unselected_provider,
        unselected_issuance,
        unselected_image,
        unselected_storage,
    )
    .expect_err("an issuance from an unselected provider plan must reject");
    let ProgramStorageInstallationHandoffError::Rejected(unselected) = unselected else {
        panic!("selected-provider mismatch must reject before record emission")
    };
    assert!(
        unselected
            .diagnostic()
            .0
            .contains("compiler-selected provider plan")
    );

    let mut drifted_schema = selected_provider.schema.clone();
    let drifted_method = drifted_schema
        .methods
        .iter_mut()
        .find(|method| method.requirement_identity == physical_binding.requirement_identity())
        .expect("selected provider implements the program-storage arrival requirement");
    drifted_method.calling_plan_fingerprint = Some(
        physical_binding
            .boundary_contract_fingerprint()
            .wrapping_add(1),
    );
    let drifted_provider = SelectedExternalRootProviderPlan {
        identity: selected_provider.identity,
        schema: drifted_schema,
    };
    let (drifted_image, drifted_issuance) =
        root_input_for_provider_invocation(509, 0x5000, 0x400, 90, 904);
    let (drifted_storage, _) = root_input_for_provider_invocation(510, 0x9003, 0x20, 90, 904);
    let drifted = install_program_storage_entry_provider_invocation(
        &physical_artifact_directory,
        physical_binding.clone(),
        &drifted_provider,
        drifted_issuance,
        drifted_image,
        drifted_storage,
    )
    .expect_err("calling-plan drift must reject the generated bridge binding");
    let ProgramStorageInstallationHandoffError::Rejected(drifted) = drifted else {
        panic!("calling-plan drift must reject before record emission")
    };
    assert!(
        drifted
            .diagnostic()
            .0
            .contains("calling plan does not match")
    );

    let (wrong_image, provider_issuance) =
        root_input_for_provider_invocation(501, 0x5000, 0x400, 90, 901);
    let (wrong_storage, _) = root_input_for_provider_invocation(502, 0x9003, 0x20, 90, 902);
    let wrong_invocation = install_program_storage_entry_provider_invocation(
        &physical_artifact_directory,
        physical_binding.clone(),
        &selected_provider,
        provider_issuance,
        wrong_image,
        wrong_storage,
    )
    .expect_err("both roots must belong to the same selected provider invocation");
    let ProgramStorageInstallationHandoffError::Rejected(wrong_invocation) = wrong_invocation
    else {
        panic!("provider invocation mismatch must reject before record emission")
    };
    assert!(
        wrong_invocation
            .diagnostic()
            .0
            .contains("selected root provider, plan, and invocation")
    );

    let (physical_image, provider_issuance) =
        root_input_for_provider_invocation(503, 0x5000, 0x400, 90, 901);
    let (physical_storage, _) = root_input_for_provider_invocation(504, 0x9003, 0x20, 90, 901);
    let bridge_without_provider = emitted_program_storage_bridge(physical_binding.clone(), None);
    let (unbound_image, unbound_issuance) =
        root_input_for_provider_invocation(511, 0x5000, 0x400, 90, 905);
    let (unbound_storage, _) = root_input_for_provider_invocation(512, 0x9003, 0x20, 90, 905);
    let mut unbound_receiver = [0xa5; 8];
    let unbound_executor_count = std::cell::Cell::new(0);
    let unbound = bridge_without_provider
        .dispatch_source_continuation_executor(
            &physical_artifact_directory,
            unbound_issuance,
            unbound_image,
            unbound_storage,
            0x9008,
            &mut unbound_receiver,
            |_| unbound_executor_count.set(unbound_executor_count.get() + 1),
        )
        .expect_err("a bridge without its selected provider cannot dispatch");
    assert_eq!(unbound_executor_count.get(), 0);
    let ProgramStorageEntryBridgeError::Installation(
        ProgramStorageInstallationHandoffError::Rejected(unbound),
    ) = unbound
    else {
        panic!("missing selected provider must reject before installation")
    };
    assert!(
        unbound
            .diagnostic()
            .0
            .contains("no retained selected physical provider")
    );
    let (_, unbound_image, unbound_storage) = unbound.into_parts();
    let _ = (unbound_image.into_grant(), unbound_storage.into_grant());
    assert_eq!(unbound_receiver, [0xa5; 8]);

    let physical_bridge =
        emitted_program_storage_bridge(physical_binding.clone(), Some(selected_provider.clone()));
    let (bad_geometry_image, bad_geometry_issuance) =
        root_input_for_provider_invocation(515, 0x5000, 0x400, 90, 907);
    let (bad_geometry_storage, _) = root_input_for_provider_invocation(516, u64::MAX, 2, 90, 907);
    let mut bad_geometry_receiver = [0xa5; 8];
    let bad_geometry_executor_count = std::cell::Cell::new(0);
    let bad_geometry = physical_bridge
        .dispatch_source_continuation_executor(
            &physical_artifact_directory,
            bad_geometry_issuance,
            bad_geometry_image,
            bad_geometry_storage,
            0x9008,
            &mut bad_geometry_receiver,
            |_| bad_geometry_executor_count.set(bad_geometry_executor_count.get() + 1),
        )
        .expect_err("invalid root geometry cannot dispatch");
    assert_eq!(bad_geometry_executor_count.get(), 0);
    let ProgramStorageEntryBridgeError::Installation(
        ProgramStorageInstallationHandoffError::Rejected(bad_geometry),
    ) = bad_geometry
    else {
        panic!("invalid geometry must reject before activation")
    };
    assert!(bad_geometry.diagnostic().0.contains("initial-storage"));
    let (_, bad_geometry_image, bad_geometry_storage) = bad_geometry.into_parts();
    let _ = (
        bad_geometry_image.into_grant(),
        bad_geometry_storage.into_grant(),
    );
    assert_eq!(bad_geometry_receiver, [0xa5; 8]);

    let (bad_mapping_image, bad_mapping_issuance) =
        root_input_for_provider_invocation(513, 0x5000, 0x400, 90, 906);
    let (bad_mapping_storage, _) = root_input_for_provider_invocation(514, 0x9003, 0x20, 90, 906);
    let mut bad_mapping_receiver = [0xa5; 8];
    let bad_mapping_executor_count = std::cell::Cell::new(0);
    let bad_mapping = physical_bridge
        .dispatch_source_continuation_executor(
            &physical_artifact_directory,
            bad_mapping_issuance,
            bad_mapping_image,
            bad_mapping_storage,
            0x9007,
            &mut bad_mapping_receiver,
            |_| bad_mapping_executor_count.set(bad_mapping_executor_count.get() + 1),
        )
        .expect_err("an inexact receiver mapping cannot dispatch");
    assert_eq!(bad_mapping_executor_count.get(), 0);
    let ProgramStorageEntryBridgeError::Activation(bad_mapping) = bad_mapping else {
        panic!("inexact mapped receiver must reject after recorded installation")
    };
    assert!(bad_mapping.diagnostic().0.contains("exactly cover"));
    assert_eq!(bad_mapping_receiver, [0xa5; 8]);
    let retained_installation = bad_mapping.into_installation();
    let attached = bind_recorded_program_storage_entry_whole_root_arguments(
        retained_installation,
        &physical_bridge,
    )
    .expect_err("an attached installation cannot enter the receiver-free argument carrier");
    assert!(attached.diagnostic().0.contains("attached program storage"));
    let ProgramStorageEntryRecordedWholeRootArgumentRecovery::RecordedInstallation(
        retained_installation,
    ) = attached.into_recovery()
    else {
        panic!("borrowed attached preflight must return the intact recorded installation")
    };
    assert!(retained_installation.roots().receiver_storage().is_some());

    let mut physical_receiver = [0xa5; 8];
    let expected_continuation = physical_bridge.continuation_key();
    let executor_count = std::cell::Cell::new(0);
    let dispatched = physical_bridge
        .dispatch_source_continuation_executor(
            &physical_artifact_directory,
            provider_issuance,
            physical_image,
            physical_storage,
            0x9008,
            &mut physical_receiver,
            |mut handoff| {
                executor_count.set(executor_count.get() + 1);
                assert_eq!(handoff.entry_symbol(), "program_storage_test_entry");
                assert_eq!(
                    (handoff.entry_text_offset(), handoff.entry_text_size()),
                    (32, 8)
                );
                assert_eq!(
                    handoff.entry_function_identity().source_key(),
                    Some(expected_continuation)
                );
                let transfer = handoff.wrapper_transfer();
                assert_eq!(
                    transfer
                        .wrapper_identity()
                        .program_storage_entry_continuation(),
                    Some(expected_continuation)
                );
                assert_eq!(
                    transfer.continuation_identity(),
                    omega_control_flow::MachineFunctionIdentity::source(expected_continuation)
                );
                assert_eq!(
                    transfer.roots()[0].role(),
                    omega_compiler::ProgramStorageEntryRootRole::Image
                );
                assert_eq!(
                    transfer.roots()[1].role(),
                    omega_compiler::ProgramStorageEntryRootRole::InitialStorage
                );
                assert!(matches!(
                    transfer.receiver(),
                    omega_compiler::ProgramStorageEntryWrapperReceiverTransfer::BorrowedActivationLoan(_)
                ));
                assert!(handoff.source_signature().is_none());
                assert!(handoff.continuation_abi().is_none());
                assert_eq!(handoff.continuation_key(), expected_continuation);
                assert_eq!(handoff.continuation_symbol(), "program_storage_test_entry");
                assert_eq!(
                    handoff.continuation_link_symbol(),
                    "program_storage_test_entry"
                );
                assert_eq!(
                    (
                        handoff.continuation_text_offset(),
                        handoff.continuation_text_size()
                    ),
                    (32, 8)
                );
                assert_eq!(
                    (
                        handoff.receiver_placement().base(),
                        handoff.receiver_placement().length(),
                    ),
                    (0x9008, 8)
                );
                assert!(handoff.continuation_receiver().is_none());
                assert_eq!(handoff.receiver(), &[0; 8]);
                handoff.receiver()[3] = 70;
                handoff.provider_invocation()
            },
        )
        .unwrap_or_else(|error| match error {
            ProgramStorageEntryBridgeError::Installation(error) => {
                panic!("physical provider installation should succeed: {error}")
            }
            ProgramStorageEntryBridgeError::Activation(error) => {
                panic!("physical receiver activation should succeed: {error}")
            }
            ProgramStorageEntryBridgeError::ContinuationReceiverBinding(error) => {
                panic!("physical receiver ABI check should succeed: {error}")
            }
        });
    assert_eq!(executor_count.get(), 1);
    assert_eq!(physical_receiver[3], 70);
    let (physical_roots, retained_invocation) = dispatched.into_parts();
    assert_eq!(retained_invocation.provider().normalized_identity(), 1090);
    assert_eq!(
        retained_invocation.provider_plan().normalized_identity(),
        90
    );
    assert_eq!(retained_invocation.invocation().normalized_identity(), 901);
    let physical_record =
        fs::read_to_string(physical_artifact_directory.join(PROGRAM_STORAGE_INSTALLATION_ARTIFACT))
            .expect("read physical-provider installation record");
    assert!(physical_record.contains("\"root_provider_invocation\": {"));
    assert!(physical_record.contains("\"provider\": \"0x0000000000000442\""));
    assert!(physical_record.contains("\"provider_plan\": \"0x000000000000005a\""));
    assert!(physical_record.contains("\"invocation\": \"0x0000000000000385\""));
    assert_eq!(
        physical_roots.provider_invocation(),
        Some(retained_invocation)
    );
    let physical_receiver_storage = physical_roots
        .receiver_storage()
        .expect("executor dispatch returns the conserved receiver partition");
    assert_eq!(
        physical_receiver_storage
            .before()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9003, 5))
    );
    assert_eq!(
        physical_receiver_storage
            .storage()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9008, 8))
    );
    assert_eq!(
        physical_receiver_storage
            .after()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9010, 0x13))
    );
    let attached_disposition = physical_roots
        .into_root_authority_disposition()
        .expect("installed receiver partition should retain an exact authority disposition");
    assert_eq!(
        attached_disposition.initial_storage_kind(),
        ProgramStorageEntryInitialStorageAuthorityKind::ReceiverPartitioned
    );
    assert!(attached_disposition.whole_initial_storage().is_none());
    assert_eq!(
        attached_disposition
            .residual_before()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9003, 5))
    );
    assert_eq!(
        attached_disposition
            .receiver_storage()
            .and_then(|receiver| receiver.storage())
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9008, 8))
    );
    assert_eq!(
        attached_disposition
            .residual_after()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x9010, 0x13))
    );
    let attached_error = attached_disposition
        .try_into_receiver_free_whole_roots()
        .expect_err("separated receiver residuals cannot become one whole Extent authority");
    assert!(
        attached_error
            .diagnostic()
            .0
            .contains("attached program storage")
    );
    let attached_disposition = attached_error.into_disposition();
    assert_eq!(
        attached_disposition
            .residual_before()
            .map(|extent| extent.length()),
        Some(5)
    );

    let static_view = installed
        .image_subextent(0x100, 0x80)
        .expect("static geometry stays a borrowed image view");
    assert_eq!(
        (static_view.loan().base(), static_view.loan().length()),
        (0x1100, 0x80)
    );
    assert_eq!(
        static_view.binding().requirement_identity(),
        installed.binding().requirement_identity()
    );
    drop(static_view);

    let failed = installed
        .partition_initial_storage(0x1f00, 0x200)
        .expect_err("an allocation cannot exceed initial storage");
    assert!(failed.diagnostic().0.contains("exceeds"));
    let installed = failed.into_roots();
    let partitioned = installed
        .partition_initial_storage(0x400, 0x800)
        .expect("owned allocation conserves both remainders");
    assert_eq!(
        partitioned
            .before()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x8000, 0x400))
    );
    assert_eq!(
        (
            partitioned.allocation().base(),
            partitioned.allocation().length()
        ),
        (0x8400, 0x800)
    );
    assert_eq!(
        partitioned
            .after()
            .map(|extent| (extent.base(), extent.length())),
        Some((0x8c00, 0x1400))
    );
    let restored = partitioned.rejoin();
    assert_eq!(
        (
            restored
                .initial_storage()
                .expect("rejoined allocation restores whole initial storage")
                .base(),
            restored
                .initial_storage()
                .expect("rejoined allocation restores whole initial storage")
                .length()
        ),
        (0x8000, 0x2000)
    );
    let free_disposition = restored
        .into_root_authority_disposition()
        .expect("rejoined receiver-free roots should retain whole authority");
    assert_eq!(
        free_disposition.initial_storage_kind(),
        ProgramStorageEntryInitialStorageAuthorityKind::Whole
    );
    assert!(free_disposition.receiver_storage().is_none());
    let free_roots = free_disposition
        .try_into_receiver_free_whole_roots()
        .expect("receiver-free installation owns two whole roots");
    assert_eq!(
        (free_roots.image().base(), free_roots.image().length()),
        (0x1000, 0x800)
    );
    assert_eq!(
        (
            free_roots.initial_storage().base(),
            free_roots.initial_storage().length()
        ),
        (0x8000, 0x2000)
    );
    let lower_level_bridge = emitted_program_storage_bridge(free_roots.binding().clone(), None);
    assert!(lower_level_bridge.wrapper_body_template().is_none());
    let missing_source =
        bind_program_storage_entry_whole_root_arguments(free_roots, &lower_level_bridge)
            .expect_err("a physical-plan-only bridge has no selected source ABI");
    assert!(
        missing_source
            .diagnostic()
            .0
            .contains("sealed selected source")
    );
    let free_roots = missing_source.into_authority();
    assert_eq!(free_roots.initial_storage().length(), 0x2000);
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn receiver_free_whole_root_authority_binds_exact_continuation_abi() {
    let directory = std::env::temp_dir().join(format!(
        "omega-free-program-storage-arguments-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create free program-storage project");
    let source = include_str!(
        "../../../../../../tests/canaries/pass/build/uefi_program_entry_storage_roots/main.omg"
    );
    let prefix = source
        .split_once("data Boot {")
        .expect("UEFI canary retains its Boot declaration")
        .0;
    fs::write(
        directory.join("main.omg"),
        format!(
            r#"{prefix}data Boot {{ }}

machine Boot::launch(
    image: Extent in Granted,
    initial_storage: Extent in Granted
) {{
    transition {{
        _ -> retain(image as Extent, initial_storage as Extent)
    }}

    state retain(image: Extent, initial_storage: Extent) {{
        transition {{
            _ -> retain(image, initial_storage)
        }}
    }}
}}
"#
        ),
    )
    .expect("write receiver-free source");
    fs::write(
        directory.join("build.omg"),
        r#"target uefi_x64 {
}

machine build(builder: &mut Build) {
    builder.application("receiver-free-whole-root-authority");
    builder.subsystem = Subsystem::EfiApplication;
    builder.freestanding = true;
    builder.roots.bind(uefi_x86_64::ProgramEntry, Boot::launch);
}
"#,
    )
    .expect("write receiver-free build root");
    let build_dir = directory.join("build");
    let report = compile(CompileOptions {
        root_path: directory.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: Some("uefi_x64".into()),
        write_output: true,
    })
    .expect("receiver-free UEFI entry should retain its source ABI");
    assert!(report.has_consistent_program_storage_entry_custody());
    assert_eq!(
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .output_path()
            .parent(),
        Some(build_dir.as_path()),
    );
    assert!(
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .has_consistent_installation_identity()
    );
    let bridge = report
        .program_storage_entry_bridge()
        .cloned()
        .expect("receiver-free UEFI entry bridge");
    assert_eq!(
        Some(bridge.binding().boundary_contract_fingerprint()),
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .boundary_contract_fingerprint(),
    );
    assert_eq!(
        bridge
            .emitted_wrapper_evidence()
            .expect("written bridge final evidence")
            .executable_inventory_fingerprint(),
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .inventory_fingerprint(),
    );
    assert_eq!(
        bridge
            .emitted_wrapper_evidence()
            .expect("written bridge final evidence")
            .compiler_text_validation()
            .derivation_fingerprint,
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .compiler_text_validation_fingerprint(),
    );
    assert_eq!(
        bridge
            .emitted_wrapper_evidence()
            .expect("written bridge final evidence")
            .compiler_function_validation()
            .evidence_fingerprint(),
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .compiler_function_validation_fingerprint(),
    );
    assert_eq!(
        bridge
            .emitted_wrapper_evidence()
            .expect("written bridge final evidence")
            .arrival()
            .boundary_contract_fingerprint(),
        report
            .executable_publication()
            .expect("written bridge publication receipt")
            .boundary_contract_fingerprint()
            .expect("program-storage publication boundary contract"),
    );
    let unwritten_report = compile(CompileOptions {
        root_path: directory.join("main.omg"),
        build_dir: Some(directory.join("unwritten-build")),
        target_name: Some("uefi_x64".into()),
        write_output: false,
    })
    .expect("the same receiver-free entry should compile without publishing an image");
    assert!(unwritten_report.has_consistent_program_storage_entry_custody());
    assert!(unwritten_report.executable_publication().is_none());
    let unwritten_bridge = unwritten_report
        .program_storage_entry_bridge()
        .cloned()
        .expect("unwritten receiver-free UEFI entry bridge");
    assert_eq!(unwritten_bridge.binding(), bridge.binding());
    assert!(unwritten_bridge.emitted_wrapper_evidence().is_none());
    assert!(matches!(
        bridge.continuation_abi().expect("source ABI").receiver(),
        omega_compiler::ProgramStorageEntryContinuationReceiverAbiPlan::Free
    ));
    let inbound = bridge
        .continuation_inbound()
        .expect("receiver-free source continuation must retain exact inbound realization");
    assert_eq!(
        inbound.continuation_identity(),
        omega_control_flow::MachineFunctionIdentity::source(bridge.continuation_key())
    );
    assert_eq!(inbound.continuation_symbol(), bridge.continuation_symbol());
    assert_eq!(
        inbound.continuation_text_range(),
        &(bridge.continuation_text_offset()
            ..bridge.continuation_text_offset() + bridge.continuation_text_size())
    );
    assert_eq!(inbound.call().result, None);
    let [image_inbound, storage_inbound] = inbound.arguments();
    assert_eq!(
        image_inbound.role(),
        omega_compiler::ProgramStorageEntryRootRole::Image
    );
    assert_eq!(image_inbound.visible_parameter_index(), 0);
    assert_eq!(image_inbound.call_parameter_index(), 0);
    assert_eq!(
        image_inbound.pointer(),
        omega_calling_conventions::IndirectPointerLocation::Register(
            omega_calling_conventions::MachineRegister::X86Rcx,
        )
    );
    assert_eq!(image_inbound.shape().byte_size, 16);
    assert_eq!(image_inbound.source_capture_write_range(), &(0..1));
    assert_eq!(
        storage_inbound.role(),
        omega_compiler::ProgramStorageEntryRootRole::InitialStorage
    );
    assert_eq!(storage_inbound.visible_parameter_index(), 1);
    assert_eq!(storage_inbound.call_parameter_index(), 1);
    assert_eq!(
        storage_inbound.pointer(),
        omega_calling_conventions::IndirectPointerLocation::Register(
            omega_calling_conventions::MachineRegister::X86Rdx,
        )
    );
    assert_eq!(storage_inbound.shape().byte_size, 16);
    assert_eq!(storage_inbound.source_capture_write_range(), &(1..2));
    let template = bridge
        .wrapper_body_template()
        .expect("receiver-free emitted bridge must retain its phase-alignment body template");
    let source_identity =
        omega_control_flow::MachineFunctionIdentity::source(bridge.continuation_key());
    let wrapper_identity =
        omega_control_flow::MachineFunctionIdentity::program_storage_entry_wrapper(
            bridge.continuation_key(),
        )
        .expect("selected source identity admits one generated wrapper identity");
    assert_eq!(template.wrapper_identity(), wrapper_identity);
    assert_eq!(template.continuation_identity(), source_identity);
    assert_eq!(template.continuation_symbol(), bridge.continuation_symbol());
    assert_eq!(
        template.continuation_text_range(),
        &(bridge.continuation_text_offset()
            ..bridge.continuation_text_offset() + bridge.continuation_text_size())
    );
    assert_eq!(
        template.wrapper_symbol(),
        omega_object_file::entry_symbol_name(omega_target::NativeTarget::uefi_x64())
    );
    assert_eq!(template.steps().len(), 11);
    assert!(matches!(
        &template.steps()[2],
        omega_compiler::ProgramStorageEntryWrapperBodyTemplateStep::CopyEntryIndirectU64ToOutgoingStack {
            role: omega_compiler::ProgramStorageEntryRootRole::Image,
            source_register: omega_calling_conventions::MachineRegister::X86Rcx,
            source_byte_offset: 0,
            stack_byte_offset: 32,
        }
    ));
    assert_eq!(
        &template.steps()[8],
        &omega_compiler::ProgramStorageEntryWrapperBodyTemplateStep::CallSourceContinuation {
            target: source_identity,
        }
    );
    assert_eq!(bridge.entry_function_identity(), wrapper_identity);
    assert_ne!(bridge.entry_function_identity(), source_identity);
    let emitted = bridge
        .emitted_wrapper_evidence()
        .expect("written receiver-free bridge must retain checked final-image evidence");
    assert_eq!(emitted.wrapper_identity(), wrapper_identity);
    assert_eq!(emitted.continuation_identity(), source_identity);
    assert_eq!(emitted.wrapper_symbol(), bridge.entry_symbol());
    assert_eq!(emitted.wrapper_section_offset(), bridge.entry_text_offset());
    assert_eq!(emitted.wrapper_byte_count(), bridge.entry_text_size());
    assert_eq!(
        emitted.continuation_symbol(),
        bridge.continuation_link_symbol()
    );
    assert_eq!(
        emitted.continuation_section_offset(),
        bridge.continuation_text_offset()
    );
    assert_eq!(
        emitted.continuation_byte_count(),
        bridge.continuation_text_size()
    );
    assert_eq!(emitted.final_call_bytes()[0], 0xe8);
    let arrival = emitted.arrival();
    assert_eq!(arrival.target(), omega_target::NativeTarget::uefi_x64());
    assert_eq!(arrival.wrapper_identity(), wrapper_identity);
    assert_eq!(
        arrival.boundary_contract_fingerprint(),
        bridge.binding().boundary_contract_fingerprint()
    );
    let [image_arrival, storage_arrival] = arrival.roots();
    assert_eq!(
        image_arrival.role(),
        omega_compiler::ProgramStorageEntryRootRole::Image
    );
    assert_eq!(image_arrival.arrival_parameter_index(), 0);
    assert_eq!(
        image_arrival.physical_arrival_placement(),
        bridge.wrapper_transfer().roots()[0].physical_arrival_placement()
    );
    assert_eq!(image_arrival.copies()[0].source_byte_offset(), 0);
    assert_eq!(
        image_arrival.copies()[1].caller_copy_stack_byte_offset(),
        40
    );
    assert_eq!(image_arrival.copies()[0].final_bytes().len(), 15);
    assert_eq!(
        storage_arrival.role(),
        omega_compiler::ProgramStorageEntryRootRole::InitialStorage
    );
    assert_eq!(storage_arrival.arrival_parameter_index(), 1);
    assert_eq!(storage_arrival.copies()[0].source_byte_offset(), 0);
    assert_eq!(
        storage_arrival.copies()[1].caller_copy_stack_byte_offset(),
        56
    );
    for root in arrival.roots() {
        assert!(
            root.copies()[0].section_byte_range().end
                <= root.copies()[1].section_byte_range().start
        );
        assert_ne!(
            root.copies()[0].selected_instruction_index(),
            root.copies()[1].selected_instruction_index()
        );
    }
    let expected_displacement = i32::try_from(emitted.continuation_section_offset()).unwrap()
        - i32::try_from(emitted.call_section_offset() + emitted.final_call_bytes().len()).unwrap();
    assert_eq!(
        &emitted.final_call_bytes()[1..],
        &expected_displacement.to_le_bytes()
    );
    assert!(emitted.wrapper_address() > emitted.continuation_address());
    assert_ne!(emitted.wrapper_byte_fingerprint(), 0);
    assert_ne!(emitted.continuation_byte_fingerprint(), 0);
    assert_ne!(emitted.compiler_text_validation().derivation_fingerprint, 0);
    assert_ne!(
        emitted
            .compiler_function_validation()
            .evidence_fingerprint(),
        0
    );
    assert_ne!(emitted.executable_inventory_fingerprint(), 0);
    let entry_manifest = fs::read_to_string(build_dir.join("10_program_storage_entry.json"))
        .expect("written bridge manifest");
    assert!(entry_manifest.contains("\"emitted_wrapper_evidence\": {"));
    assert!(entry_manifest.contains("\"final_call_bytes\": [232,"));
    assert!(entry_manifest.contains("\"semantic_wrapper_arrival\": {"));
    assert!(entry_manifest.contains("\"pointer_register\": \"X86Rcx\""));
    assert!(entry_manifest.contains("\"pointer_register\": \"X86Rdx\""));
    assert!(entry_manifest.contains("\"status\": \"pending_runtime_installation\""));
    let selected_provider = provider_for_program_storage_binding(bridge.binding());
    let (image, provider_issuance) =
        root_input_for_provider_invocation(701, 0x1000, 0x800, 90, 911);
    let (initial_storage, _) = root_input_for_provider_invocation(702, 0x8000, 0x2000, 90, 911);
    let installation = install_program_storage_entry_provider_invocation(
        &build_dir,
        bridge.binding().clone(),
        &selected_provider,
        provider_issuance,
        image,
        initial_storage,
    )
    .expect("install exact receiver-free roots");
    let alternate_source = fs::read_to_string(directory.join("main.omg"))
        .expect("read receiver-free source")
        .replace("Boot::launch(", "Boot::alternate(");
    fs::write(directory.join("main.omg"), alternate_source)
        .expect("write alternate receiver-free source");
    let alternate_build = fs::read_to_string(directory.join("build.omg"))
        .expect("read receiver-free build root")
        .replace("Boot::launch", "Boot::alternate");
    fs::write(directory.join("build.omg"), alternate_build)
        .expect("write alternate receiver-free build root");
    let other_bridge = compile(CompileOptions {
        root_path: directory.join("main.omg"),
        build_dir: Some(directory.join("alternate-build")),
        target_name: Some("uefi_x64".into()),
        write_output: false,
    })
    .expect("alternate receiver-free UEFI entry should compile")
    .program_storage_entry_bridge()
    .cloned()
    .expect("alternate receiver-free UEFI entry bridge");
    assert!(
        other_bridge.emitted_wrapper_evidence().is_none(),
        "an unwritten build must not claim final-image wrapper evidence"
    );
    let wrong_bridge =
        bind_recorded_program_storage_entry_whole_root_arguments(installation, &other_bridge)
            .expect_err("a recorded installation cannot bind another selected entry");
    assert!(
        wrong_bridge
            .diagnostic()
            .0
            .contains("exact program-storage bridge binding")
    );
    let ProgramStorageEntryRecordedWholeRootArgumentRecovery::RecordedInstallation(installation) =
        wrong_bridge.into_recovery()
    else {
        panic!("borrowed ABI preflight must return the intact recorded installation")
    };
    assert_eq!(installation.installation_record().image().base(), 0x1000);
    let carrier = bind_recorded_program_storage_entry_whole_root_arguments(installation, &bridge)
        .expect("the same recorded installation should retry against its exact bridge");
    let [image, initial_storage] = carrier.arguments();
    assert_eq!(
        image.role(),
        omega_compiler::ProgramStorageEntryRootRole::Image
    );
    assert_eq!(image.visible_parameter_index(), 0);
    assert_eq!(image.call_parameter_index(), 0);
    assert_eq!(
        initial_storage.role(),
        omega_compiler::ProgramStorageEntryRootRole::InitialStorage
    );
    assert_eq!(initial_storage.visible_parameter_index(), 1);
    assert_eq!(initial_storage.call_parameter_index(), 1);
    assert_eq!(
        image.placement(),
        &bridge.continuation_abi().unwrap().call().parameters[0]
    );
    assert_eq!(
        initial_storage.placement(),
        &bridge.continuation_abi().unwrap().call().parameters[1]
    );
    assert_eq!(
        (
            carrier
                .root_authority(omega_compiler::ProgramStorageEntryRootRole::Image)
                .base(),
            carrier
                .root_authority(omega_compiler::ProgramStorageEntryRootRole::InitialStorage)
                .length()
        ),
        (0x1000, 0x2000)
    );
    let missing_emitted =
        bind_program_storage_entry_emitted_whole_root_arguments(carrier, &unwritten_bridge)
            .expect_err("an unwritten bridge cannot bind installed roots to final wrapper bytes");
    assert!(
        missing_emitted
            .diagnostic()
            .0
            .contains("without final emitted-wrapper evidence")
    );
    let carrier = missing_emitted.into_arguments();
    let emitted_arguments =
        bind_program_storage_entry_emitted_whole_root_arguments(carrier, &bridge)
            .expect("installed roots should bind their exact final emitted wrapper");
    assert_eq!(
        emitted_arguments.emitted_wrapper().wrapper_identity(),
        bridge.entry_function_identity()
    );
    assert_eq!(
        emitted_arguments
            .emitted_wrapper()
            .arrival()
            .boundary_contract_fingerprint(),
        bridge.binding().boundary_contract_fingerprint()
    );
    let carrier = emitted_arguments.into_arguments();
    let values = bind_program_storage_entry_whole_root_logical_values(carrier)
        .expect("exact whole-root authorities should bind their logical Extent values");
    let [image_value, storage_value] = values.values();
    assert_eq!((image_value.base(), image_value.length()), (0x1000, 0x800));
    assert_eq!(
        (storage_value.base(), storage_value.length()),
        (0x8000, 0x2000)
    );
    for (index, value) in values.values().iter().enumerate() {
        assert_eq!(value.visible_parameter_index(), index);
        assert_eq!(value.call_parameter_index(), index);
        assert_eq!(
            value.layout().shape(),
            omega_calling_conventions::ValueShape::integer(16, 8)
        );
        let [base, length] = value.layout().fields();
        assert_eq!(base.byte_offset(), 0);
        assert_eq!(length.byte_offset(), 8);
    }
    let operands = bind_program_storage_entry_whole_root_operands(values)
        .expect("exact logical Extents should bind their indirect operand images");
    let [image_operand, storage_operand] = operands.operands();
    assert_eq!(
        image_operand.pointer(),
        omega_calling_conventions::IndirectPointerLocation::Register(
            omega_calling_conventions::MachineRegister::X86Rcx,
        )
    );
    assert_eq!(image_operand.caller_copy_byte_range(), 32..48);
    assert_eq!(
        image_operand.bytes(),
        &[
            0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ]
    );
    assert_eq!(
        storage_operand.pointer(),
        omega_calling_conventions::IndirectPointerLocation::Register(
            omega_calling_conventions::MachineRegister::X86Rdx,
        )
    );
    assert_eq!(storage_operand.caller_copy_byte_range(), 48..64);
    assert_eq!(
        storage_operand.bytes(),
        &[
            0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ]
    );
    assert_eq!(image_operand.byte_size(), 16);
    assert_eq!(storage_operand.alignment(), 8);
    assert_eq!(
        operands
            .logical_values()
            .arguments()
            .root_authority(omega_compiler::ProgramStorageEntryRootRole::Image)
            .base(),
        0x1000
    );
    let caller_frame = plan_program_storage_entry_wrapper_caller_frame(operands)
        .expect("exact operand images should plan one balanced wrapper caller frame");
    assert_eq!(caller_frame.shadow_byte_count(), 32);
    assert_eq!(caller_frame.outgoing_reservation_byte_count(), 72);
    assert_eq!(caller_frame.outgoing_release_byte_count(), 72);
    assert_eq!(caller_frame.pre_call_stack_alignment(), 16);
    use omega_compiler::ProgramStorageEntryWrapperCallerFrameStep::{
        BindCallerCopyAddress, WriteExtentWord,
    };
    let [
        WriteExtentWord {
            role: image_base_role,
            visible_parameter_index: image_base_visible,
            call_parameter_index: image_base_call,
            field: image_base_field,
            operand_byte_offset: image_base_operand_offset,
            stack_byte_offset: image_base_stack_offset,
            bytes: image_base_bytes,
        },
        WriteExtentWord {
            role: image_length_role,
            field: image_length_field,
            operand_byte_offset: image_length_operand_offset,
            stack_byte_offset: image_length_stack_offset,
            bytes: image_length_bytes,
            ..
        },
        WriteExtentWord {
            role: storage_base_role,
            field: storage_base_field,
            operand_byte_offset: storage_base_operand_offset,
            stack_byte_offset: storage_base_stack_offset,
            bytes: storage_base_bytes,
            ..
        },
        WriteExtentWord {
            role: storage_length_role,
            field: storage_length_field,
            operand_byte_offset: storage_length_operand_offset,
            stack_byte_offset: storage_length_stack_offset,
            bytes: storage_length_bytes,
            ..
        },
        BindCallerCopyAddress {
            role: image_address_role,
            register: image_address_register,
            caller_copy_stack_byte_offset: image_copy_offset,
            caller_copy_byte_count: image_copy_size,
            caller_copy_alignment: image_copy_alignment,
            ..
        },
        BindCallerCopyAddress {
            role: storage_address_role,
            register: storage_address_register,
            caller_copy_stack_byte_offset: storage_copy_offset,
            caller_copy_byte_count: storage_copy_size,
            caller_copy_alignment: storage_copy_alignment,
            ..
        },
    ] = caller_frame.steps()
    else {
        panic!("caller-frame recipe must retain four writes followed by two address bindings")
    };
    assert_eq!(
        (
            *image_base_role,
            *image_base_visible,
            *image_base_call,
            *image_base_field,
            *image_base_operand_offset,
            *image_base_stack_offset,
            *image_base_bytes,
        ),
        (
            omega_compiler::ProgramStorageEntryRootRole::Image,
            0,
            0,
            omega_compiler::ProgramEntrySourceExtentFieldRole::Base,
            0,
            32,
            0x1000_u64.to_le_bytes(),
        )
    );
    assert_eq!(
        (
            *image_length_role,
            *image_length_field,
            *image_length_operand_offset,
            *image_length_stack_offset,
            *image_length_bytes,
        ),
        (
            omega_compiler::ProgramStorageEntryRootRole::Image,
            omega_compiler::ProgramEntrySourceExtentFieldRole::Length,
            8,
            40,
            0x800_u64.to_le_bytes(),
        )
    );
    assert_eq!(
        (
            *storage_base_role,
            *storage_base_field,
            *storage_base_operand_offset,
            *storage_base_stack_offset,
            *storage_base_bytes,
        ),
        (
            omega_compiler::ProgramStorageEntryRootRole::InitialStorage,
            omega_compiler::ProgramEntrySourceExtentFieldRole::Base,
            0,
            48,
            0x8000_u64.to_le_bytes(),
        )
    );
    assert_eq!(
        (
            *storage_length_role,
            *storage_length_field,
            *storage_length_operand_offset,
            *storage_length_stack_offset,
            *storage_length_bytes,
        ),
        (
            omega_compiler::ProgramStorageEntryRootRole::InitialStorage,
            omega_compiler::ProgramEntrySourceExtentFieldRole::Length,
            8,
            56,
            0x2000_u64.to_le_bytes(),
        )
    );
    assert_eq!(
        (
            *image_address_role,
            *image_address_register,
            *image_copy_offset,
            *image_copy_size,
            *image_copy_alignment,
        ),
        (
            omega_compiler::ProgramStorageEntryRootRole::Image,
            omega_calling_conventions::MachineRegister::X86Rcx,
            32,
            16,
            8,
        )
    );
    assert_eq!(
        (
            *storage_address_role,
            *storage_address_register,
            *storage_copy_offset,
            *storage_copy_size,
            *storage_copy_alignment,
        ),
        (
            omega_compiler::ProgramStorageEntryRootRole::InitialStorage,
            omega_calling_conventions::MachineRegister::X86Rdx,
            48,
            16,
            8,
        )
    );
    assert_eq!(
        caller_frame
            .operands()
            .logical_values()
            .arguments()
            .root_authority(omega_compiler::ProgramStorageEntryRootRole::InitialStorage)
            .length(),
        0x2000
    );
    let reserved = reserve_program_storage_entry_outgoing_stack_frame(caller_frame)
        .expect("exact caller frame should yield sealed write authority");
    assert_eq!(reserved.frame_byte_count(), 72);
    assert_eq!(reserved.shadow_byte_range(), 0..32);
    assert_eq!(reserved.image_writable_byte_range(), 32..48);
    assert_eq!(reserved.initial_storage_writable_byte_range(), 48..64);
    assert_eq!(
        std::array::from_fn(|index| {
            let word = &reserved.words()[index];
            (word.stack_byte_offset(), word.value(), *word.bytes())
        }),
        [
            (32, 0x1000, 0x1000_u64.to_le_bytes()),
            (40, 0x800, 0x800_u64.to_le_bytes()),
            (48, 0x8000, 0x8000_u64.to_le_bytes()),
            (56, 0x2000, 0x2000_u64.to_le_bytes()),
        ]
    );
    let recovered_frame = reserved.into_caller_frame();
    assert_eq!(recovered_frame.outgoing_reservation_byte_count(), 72);
    assert_eq!(
        recovered_frame
            .operands()
            .logical_values()
            .arguments()
            .root_authority(omega_compiler::ProgramStorageEntryRootRole::Image)
            .base(),
        0x1000
    );
    let _ = fs::remove_dir_all(directory);
}

fn root_input_for_provider_invocation(
    lineage: u64,
    base: u64,
    length: u64,
    provider_plan: u64,
    invocation: u64,
) -> (ProgramStorageRootInput, psi_extents::ExtentProviderIssuance) {
    fn extent_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    let issuance_base = lineage * 16;
    let provider_issuance = psi_extents::ExtentProviderIssuance::from_normalized_identities([
        issuance_base + 1,
        issuance_base + 2,
        provider_plan + 1000,
        issuance_base + 4,
        issuance_base + 5,
        issuance_base + 6,
        issuance_base + 7,
        issuance_base + 8,
        provider_plan,
        invocation,
        issuance_base + 11,
        issuance_base + 12,
        issuance_base + 13,
    ])
    .expect("normalized physical-provider issuance");
    (
        ProgramStorageRootInput::new(
            ExtentRootGrant::from_admitted_provider(
                provider_issuance,
                extent_id(lineage, ExtentLineageId::from_normalized_identity),
                extent_id(100, AddressSpaceId::from_normalized_identity),
                ExtentRights::none(),
                extent_id(101, ExtentProvenanceId::from_normalized_identity),
                extent_id(102, MappingEraId::from_normalized_identity),
            ),
            base,
            length,
        ),
        provider_issuance,
    )
}

fn provider_for_program_storage_binding(
    binding: &omega_compiler::ProgramStorageEntryPlanBinding,
) -> SelectedExternalRootProviderPlan {
    let entry_claims = (0..2)
        .map(
            |parameter_index| omega_effects::provider_plan::ServiceEntryClaim {
                parameter_index,
                carrier_identity: "named(name(Extent))".into(),
                domain: "Extent::Granted".into(),
                predicate_body: psi_language_semantics::DomainPredicateBody::Present,
                effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                authority_flow: omega_effects::provider_plan::ServiceEntryAuthorityFlow::Accepts,
            },
        )
        .collect();
    SelectedExternalRootProviderPlan {
        identity: omega_external_roots::ProviderPlanId::from_normalized_identity(90)
            .expect("selected provider identity"),
        schema: omega_effects::provider_plan::ServiceSchema {
            trait_name: "UefiApplication".into(),
            trait_package_identity: None,
            methods: vec![omega_effects::provider_plan::ServiceMethod {
                name: "enter".into(),
                requirement_owner: "ProgramStorageEntry".into(),
                requirement_owner_package_identity: None,
                requirement_identity: binding.requirement_identity().into(),
                parameter_count: 2,
                parameter_type_identities: vec![
                    binding.image().parameter_type_identity().into(),
                    binding.initial_storage().parameter_type_identity().into(),
                ],
                entry_claims,
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: Vec::new(),
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_fingerprint: Some(binding.boundary_contract_fingerprint()),
            }],
        },
    }
}

fn emitted_program_storage_bridge(
    binding: omega_compiler::ProgramStorageEntryPlanBinding,
    selected_provider: Option<SelectedExternalRootProviderPlan>,
) -> omega_compiler::ProgramStorageEntryNativeBridgePlan {
    let continuation_key = omega_control_flow::StateKey {
        machine: psi_symbols::SymbolHandle::from_arena_index(1),
        state: psi_symbols::SymbolHandle::from_arena_index(2),
        segment_index: 0,
    };
    let mut object =
        omega_object_file::ObjectPlan::with_capacity(omega_target::NativeTarget::host(), 0, 1);
    let entry = object.layout.symbols.insert(omega_object_file::SymbolPlan {
        name: "program_storage_test_entry".into(),
        section: omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Text),
        offset: 32,
        size: 8,
        kind: omega_object_file::SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = entry;
    object
        .layout
        .function_symbols
        .insert(omega_object_file::FunctionSymbolPlan {
            identity: omega_control_flow::MachineFunctionIdentity::source(continuation_key),
            symbol: entry,
        });
    let mut encoded = omega_machine_bytes::EncodedMachinePlan::default();
    encoded
        .code
        .functions
        .insert(omega_machine_bytes::EncodedMachineFunction {
            symbol: std::sync::Arc::from("program_storage_test_entry"),
            identity: omega_control_flow::MachineFunctionIdentity::source(continuation_key),
            byte_offset: 32,
            byte_count: 8,
            instructions: psi_arena::HandleSpan::empty(),
        });
    let boundary_fingerprint = binding.boundary_contract_fingerprint();
    bind_emitted_program_storage_entry_native_bridge(
        binding,
        selected_provider,
        "synthetic_physical_entry".into(),
        &object,
        &encoded,
        continuation_key,
        Some(boundary_fingerprint),
        "Boot::launch".into(),
        "launch".into(),
    )
    .expect("synthetic emitted bridge should retain exact continuation identity")
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
FullWidthPolicyCallingPolicy: FullWidthPolicy satisfies CallingPolicy;
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
fn source_policy_receives_erased_stripped_nested_record_parameters_and_results() {
    let source = r#"
use omega::language::std::calling;

data Evidence { case Only; }
data Inner {
    byte: u8;
    proof [erased]: Evidence;
}
data Certified {
    head: u16;
    witness [erased]: Evidence;
    inner: Inner;
    tail: u32;
}

data ErasedRecordPolicy { }
ErasedRecordPolicyCallingPolicy: ErasedRecordPolicy satisfies CallingPolicy;

machine ErasedRecordPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 && signature.has_result {
        true -> outer(signature, signature.parameters[0], signature.result)
        _ -> wrong()
    }

    state outer(signature: BoundarySignature, root: u64, result: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> outer_fields(signature, result, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state outer_fields(
        signature: BoundarySignature,
        result: u64,
        root: u64,
        first: u64,
        count: u64
    ) -> BoundaryPlanResult {
        transition count == 3
            && signature.shapes[root].byte_size == 8
            && signature.shapes[root].alignment == 4
            && signature.fields[first].byte_offset == 0
            && signature.fields[first + 1].byte_offset == 2
            && signature.fields[first + 2].byte_offset == 4 {
            true -> inner(signature, result, signature.fields[first + 1].shape)
            _ -> wrong()
        }
    }

    state inner(signature: BoundarySignature, result: u64, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> inner_fields(signature, result, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state inner_fields(
        signature: BoundarySignature,
        result: u64,
        root: u64,
        first: u64,
        count: u64
    ) -> BoundaryPlanResult {
        transition count == 1
            && signature.shapes[root].byte_size == 1
            && signature.shapes[root].alignment == 1
            && signature.fields[first].byte_offset == 0 {
            true -> result_record(signature, result)
            _ -> wrong()
        }
    }

    state result_record(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> result_fields(signature, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state result_fields(
        signature: BoundarySignature,
        root: u64,
        first: u64,
        count: u64
    ) -> BoundaryPlanResult {
        transition count == 3
            && signature.shapes[root].byte_size == 8
            && signature.shapes[root].alignment == 4
            && signature.fields[first].byte_offset == 0
            && signature.fields[first + 1].byte_offset == 2
            && signature.fields[first + 2].byte_offset == 4 {
            true -> observed()
            _ -> wrong()
        }
    }

    state observed() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "erased-stripped nested records observed",
            },
        }
    }

    state wrong() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "erased-stripped record mismatch",
            },
        }
    }
}

boundary trait Probe: Calling<ErasedRecordPolicy> {
    machine inspect(value: Certified) -> Certified;
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("erased-boundary-record", source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("the observing policy deliberately rejects after checking its input graph");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("erased-stripped nested records observed"),
        "policy did not observe the erased-stripped recursive graph:\n{rendered}"
    );
    assert!(
        !rendered.contains("erased-stripped record mismatch"),
        "{rendered}"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn erased_case_data_remains_rejected_as_an_unclassified_sum_shape() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {\n    machine tick();\n}",
        "data Evidence { case Only; }\ndata Choice { case None; case Some(value: i32, proof [erased]: Evidence); }\n\nboundary trait Tick: Calling<NoResultPolicy> {\n    machine tick(value: Choice);\n}",
    );
    let main_path = write_program("erased-boundary-sum", &source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("case-bearing data has no public calling-policy graph shape yet");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("case data `Choice` is not yet classifiable as a boundary value"),
        "unexpected diagnostics:\n{rendered}"
    );
    assert!(
        !rendered.contains("erased-stripped ABI classification is not implemented yet"),
        "the retired blanket relevance fence should not mask the sum-shape diagnostic:\n{rendered}"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
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
fn calling_policy_evaluates_distinct_same_named_requirement_overloads() {
    let source = format!(
        "{INTERRUPT_POLICY}\n\nboundary trait OverloadedEntry: Calling<X86InterruptPolicy> {{\n    machine enter(value: u64);\n    machine enter(value: i64);\n}}\n"
    );
    let main_path = write_program("same-named-requirement-overloads", &source);
    let checked = compile_to_checked(&main_path, None)
        .expect("same-named exact requirement overloads should each evaluate their policy");
    let overloaded = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "OverloadedEntry")
        .expect("OverloadedEntry boundary trait");
    let schema =
        omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, overloaded)
            .expect("overloaded boundary service schema");
    let methods = schema
        .methods
        .iter()
        .filter(|method| method.name == "enter")
        .collect::<Vec<_>>();

    assert_eq!(methods.len(), 2);
    assert!(!methods[0].requirement_identity.is_empty());
    assert!(!methods[1].requirement_identity.is_empty());
    assert_ne!(
        methods[0].requirement_identity, methods[1].requirement_identity,
        "the readable method name is not an overload identity"
    );
    assert!(methods[0].calling_plan_fingerprint.is_some());
    assert!(methods[1].calling_plan_fingerprint.is_some());

    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn generic_boundary_conformance_selects_and_publishes_its_policy_instance() {
    let source = POLICY.replace(
        "boundary trait Tick: Calling<NoResultPolicy> {\n    machine tick();\n}",
        "boundary trait Tick<C>: Calling<C>\nwhere C satisfies CallingPolicy\n{\n    machine tick(&mut self);\n}\n\ndata TickProvider { count: i64; }\nTickProviderTick: TickProvider satisfies Tick<NoResultPolicy>;\nmachine TickProvider::tick(&mut self) satisfies Tick<NoResultPolicy>::tick {\n    self.count = 1;\n}",
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
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .carrier_name()
                .is_some_and(|carrier| carrier.as_str() == "TickProvider")
        })
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
        "boundary trait Tick<C>: Calling<C>\nwhere C satisfies CallingPolicy\n{",
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
RecursiveShapePolicyCallingPolicy: RecursiveShapePolicy satisfies CallingPolicy;

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

#[test]
fn source_policy_receives_stored_integer_physical_shape() {
    let source = r#"
use omega::language::std::calling;

data IntegerInterpretation { case Signed; case Unsigned; }
data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { key: u64; size: u64; align: u64; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64; }
data FieldPlan {
    case At(offset: u64);
    case IntegerAt(offset: u64, stored_width: u64, interpretation: IntegerInterpretation);
}
data FieldEntry { key: u64; placement: FieldPlan; }
data Plan {
    entries: [FieldEntry; 64];
    entry_count: u64;
    size_fixed: u64;
    size_is_dynamic: bool;
    align: u64;
}

data SignedByte { entries: [FieldEntry; 64]; }
machine SignedByte::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::IntegerAt {
            offset: 0,
            stored_width: 8,
            interpretation: IntegerInterpretation::Signed,
        },
    };
    Plan {
        entries: self.entries,
        entry_count: 1,
        size_fixed: 1,
        size_is_dynamic: false,
        align: 1,
    }
}

data PortableByte { value: i64; }

data StoredWidthPolicy { }
StoredWidthPolicyCallingPolicy: StoredWidthPolicy satisfies CallingPolicy;
machine StoredWidthPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 {
        true -> record(signature, signature.parameters[0])
        _ -> wrong()
    }

    state record(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].class {
            ValueClass::Record { first_field, field_count } -> field(signature, root, first_field, field_count)
            _ -> wrong()
        }
    }

    state field(signature: BoundarySignature, root: u64, first: u64, count: u64) -> BoundaryPlanResult {
        transition count == 1
            && signature.shapes[root].byte_size == 1
            && signature.shapes[root].alignment == 1
            && signature.fields[first].byte_offset == 0 {
            true -> scalar(signature, signature.fields[first].shape)
            _ -> wrong()
        }
    }

    state scalar(signature: BoundarySignature, field: u64) -> BoundaryPlanResult {
        transition signature.shapes[field].class {
            ValueClass::Integer -> scalar_width(signature, field)
            _ -> wrong()
        }
    }

    state scalar_width(signature: BoundarySignature, field: u64) -> BoundaryPlanResult {
        transition signature.shapes[field].byte_size == 1
            && signature.shapes[field].alignment == 1 {
            true -> observed()
            _ -> wrong()
        }
    }

    state observed() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "stored-integer physical shape observed",
            },
        }
    }

    state wrong() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "stored-integer shape mismatch",
            },
        }
    }
}

boundary trait Probe: Calling<StoredWidthPolicy> {
    machine inspect(value: SignedByte<PortableByte>);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("stored-integer-shape", source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("the observing policy deliberately rejects after checking the physical shape");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("stored-integer physical shape observed"),
        "policy did not observe the validated stored width:\n{rendered}"
    );
    assert!(
        !rendered.contains("stored-integer shape mismatch"),
        "{rendered}"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}
