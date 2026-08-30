use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, EntryStack, MachineState, MachineStateSet,
    Preemption, ValueShape,
};
use omega_compiler::compile_to_checked;
use omega_provider_planning::calling_policy_plans::evaluate_calling_policy_plan;
use omega_provider_planning::plans::{
    selected_external_root_entry_fact_bindings, selected_external_root_provider_plan,
    selected_external_root_provider_plan_id,
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
        .plan_by_report_fingerprint(identity.normalized_identity())
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
fn direct_callback_parameter_is_interleaved_without_a_source_runtime_argument() {
    let main_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
        .join("source/library/std/tests/direct_callback_parameter.omg");
    let checked = compile_to_checked(&main_path, Some("windows_x64"))
        .expect("target closure should place the declared direct callback parameter");
    let registrar = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str().ends_with("HookRegistrar"))
        .expect("HookRegistrar boundary trait");
    let install = checked
        .typed
        .trait_machine_signatures(registrar)
        .iter()
        .find(|signature| signature.name.as_str() == "install")
        .expect("HookRegistrar::install requirement");
    assert_eq!(
        checked.typed.state_signature_parameters(install).len(),
        2,
        "the native-only callback must not become a source runtime parameter"
    );
    let [callback] = install.native_callback_parameters.as_slice() else {
        panic!("one exact native-only callback declaration must survive typed lowering");
    };
    assert_eq!(callback.name.as_str(), "procedure");
    assert_eq!(callback.binder.as_str(), "Handler");
    assert_eq!(callback.native_ordinal, 1);
}

#[test]
fn direct_callback_parameter_requires_a_bodyless_boundary_requirement() {
    let source = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(5)
            .unwrap()
            .join("source/library/std/tests/direct_callback_parameter.omg"),
    )
    .unwrap()
    .replace("boundary trait HookRegistrar", "trait HookRegistrar");
    let main_path = write_program("direct-callback-nonboundary", &source);
    let diagnostics = compile_to_checked(&main_path, Some("windows_x64"))
        .expect_err("a non-boundary trait cannot declare a native callback parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("permitted only on a bodyless boundary-trait requirement")
    }));
}

#[test]
fn direct_callback_parameter_requires_its_exact_nominal_binder() {
    let source = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(5)
            .unwrap()
            .join("source/library/std/tests/direct_callback_parameter.omg"),
    )
    .unwrap()
    .replace(
        "native callback procedure from Handler",
        "native callback procedure from Missing",
    );
    let main_path = write_program("direct-callback-missing-binder", &source);
    let diagnostics = compile_to_checked(&main_path, Some("windows_x64"))
        .expect_err("a direct callback cannot infer or invent its binder");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown machine binder `Missing`")
    }));
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
#[ignore = "TASKS_PACKAGE_MANAGER OPAQUE-BY-VALUE-BOUNDARY-ABI: implementation pending"]
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
        schema.methods[0].calling_plan_report_fingerprint,
        Some(validated.contract_report_fingerprint())
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
        mask_plan.report_fingerprint(),
        missing_active.report_fingerprint(),
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
        selected.report_fingerprint(),
        weakened.report_fingerprint(),
        "the provider-plan identity carried into external-root admission must drift if Pending issuance is removed"
    );
    let mut unreported = selected.clone();
    unreported.schema.methods[0].entry_claims.clear();
    assert_ne!(
        selected.report_fingerprint(),
        unreported.report_fingerprint(),
        "the provider-plan receipt must bind the compiler-owned accepted-authority row"
    );
    let root_selection =
        selected_external_root_provider_plan(checked.selected_provider_plans(), "TimerRoot")
            .expect("external-root bridge should retain the qualified timer schema");
    assert_eq!(
        root_selection.identity.normalized_identity(),
        selected.report_fingerprint()
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
        selected.report_fingerprint()
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
        selected.report_fingerprint()
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
        selected.report_fingerprint()
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
    assert_ne!(validated.contract_report_fingerprint(), 0);

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
        schema.methods[0].calling_plan_report_fingerprint,
        Some(validated.contract_report_fingerprint())
    );
    let retained = checked
        .typed
        .boundary_calling_plans
        .iter()
        .find(|identity| identity.report_fingerprint == validated.contract_report_fingerprint())
        .expect("typed semantic identity for the published boundary contract");
    assert_ne!(retained.report_fingerprint, 0);
    assert_eq!(
        retained.commitment.as_bytes(),
        validated.contract_commitment_digest()
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
            .calling_plan_report_fingerprint
            .expect("evaluated calling identity")
    };

    assert_eq!(fingerprint("FirstPolicy"), fingerprint("RenamedPolicy"));
}

#[test]
fn calling_policy_evaluates_distinct_same_named_requirement_overloads() {
    let source = r#"
use omega::language::std::calling;

data OneParameterPolicy { }
OneParameterPolicyCallingPolicy: OneParameterPolicy satisfies CallingPolicy;

machine OneParameterPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.convention = CallingConvention::MicrosoftX64;
    output.call.parameter_count = 1;
    output.call.parameters[0].shape.class = AbiValueClass::Integer;
    output.call.parameters[0].shape.byte_size = signature.shapes[0].byte_size;
    output.call.parameters[0].shape.alignment = signature.shapes[0].alignment;
    output.call.parameters[0].location_count = 1;
    output.call.parameters[0].locations[0] = ValueLocation::Register {
        register: MachineRegister::X86Rcx,
        value_byte_offset: 0,
        byte_size: signature.shapes[0].byte_size,
    };
    output.call.stack_alignment = 16;
    output.call.shadow_bytes = 32;
    output.call.entry_control = EntryControl::CallReturn;
    BoundaryPlanResult::Accepted { plan: output }
}

boundary trait OverloadedEntry: Calling<OneParameterPolicy> {
    machine enter(value: u64);
    machine enter(value: i64);
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("same-named-requirement-overloads", source);
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
    assert!(methods[0].calling_plan_report_fingerprint.is_some());
    assert!(methods[0].calling_plan_commitment.is_some());
    assert!(methods[1].calling_plan_report_fingerprint.is_some());
    assert!(methods[1].calling_plan_commitment.is_some());

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
    assert!(schema.methods[0].calling_plan_report_fingerprint.is_some());
    assert!(schema.methods[0].calling_plan_commitment.is_some());
    assert_eq!(
        omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, tick)
            .expect("uninstantiated schema")
            .methods[0]
            .calling_plan_report_fingerprint,
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

    assert_eq!(schema.methods[0].calling_plan_report_fingerprint, None);
    assert_eq!(schema.methods[0].calling_plan_commitment, None);
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
