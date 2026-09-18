//! Fixtures shared by the calling policy plan tests: written programs,
//! the repository root and retained interrupt representations.

#[path = "calling_policy_plans/calling_vocabulary_and_callbacks.rs"]
mod calling_vocabulary_and_callbacks;
#[path = "calling_policy_plans/linux_entry.rs"]
mod linux_entry;
#[path = "calling_policy_plans/macos_entry.rs"]
mod macos_entry;
#[path = "calling_policy_plans/opaque_boundaries.rs"]
mod opaque_boundaries;
#[path = "calling_policy_plans/policy_evaluation.rs"]
mod policy_evaluation;
#[path = "calling_policy_plans/windows_entry.rs"]
mod windows_entry;

use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use provider_planning::selected_external_root_provider_plan_id;

use std::fs;
use std::path::PathBuf;

use semantic_vocabulary::PackageKeyIdentity;

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

fn write_project(name: &str, source: &str, build: &str) -> PathBuf {
    let main = write_program(name, source);
    fs::write(
        main.parent().expect("project directory").join("build.omg"),
        build,
    )
    .expect("write calling-policy build file");
    main
}

fn compile_project_negative(name: &str, source: &str, build: &str) -> String {
    compile_to_checked(CheckedCompileRequest::new(
        &write_project(name, source, build),
        None,
    ))
    .expect_err("negative opaque representation project must reject")
    .iter()
    .map(|diagnostic| diagnostic.message.as_str())
    .collect::<Vec<_>>()
    .join("\n")
}

fn compile_std_negative(name: &str, source: &str) -> String {
    let (path, package_inputs) = write_callback_package(name, source);
    let result = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&path, Some("windows_x86_64"))
    });
    result
        .expect_err("negative callback source canary must reject")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn repository_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("Omega repository root")
        .to_path_buf()
}

fn callback_fixture_source(name: &str) -> String {
    fs::read_to_string(
        repository_root()
            .join("source/library/std/tests")
            .join(name),
    )
    .unwrap_or_else(|error| panic!("read callback fixture `{name}`: {error}"))
}

fn standard_library_root() -> PathBuf {
    repository_root().join("source/library/std")
}

/// Compose a callback fixture package that takes the standard library as an
/// ordinary dependency.
///
/// The Windows x86-64 program-entry slot owns a closed physical-contract
/// package (`targets/windows_x86_64/entry.omg`), so every target-selected
/// compilation seeds that authored contract, and the contract declares its
/// calling vocabulary through the bundled `std::calling` module. A fixture that
/// also copied `calling.omg` into its own package root declared that vocabulary
/// twice in one program — 32 `duplicate data`/`duplicate trait` diagnostics
/// before any callback behavior was reached. Binding the standard library as a
/// dependency instead makes it the single supplier of both the contract and the
/// vocabulary, and the fixture source imports `omega_language_std::calling`; a
/// package-aware source may not spell `omega::language::std::...` directly, so
/// the requester-local alias is the only admissible spelling.
fn write_callback_package(name: &str, source: &str) -> (PathBuf, PackageCompilationInputs) {
    let directory = std::env::temp_dir().join(format!(
        "omega-calling-policy-package-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create callback package fixture");
    let main = directory.join("main.omg");
    fs::write(
        &main,
        source.replacen("use calling;", "use omega_language_std::calling;", 1),
    )
    .expect("write callback package source");

    let package = PackageKeyIdentity::from_digest([73; 32])
        .expect("nonzero callback fixture package identity");
    let standard_library = PackageKeyIdentity::from_digest([77; 32])
        .expect("nonzero standard-library package identity");
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![
            PackageSourceBinding::new(package, "calling-policy-fixture", directory),
            PackageSourceBinding::new(
                standard_library,
                "omega-language-std",
                standard_library_root(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            package,
            "omega_language_std",
            standard_library,
        )],
    )
    .expect("callback fixture package graph");
    (main, inputs)
}

fn selected_plan_for_external_root<'a>(
    facts: &'a effects::SelectedProviderPlanFacts,
    trait_name: &str,
) -> &'a effects::provider_plan::ProviderPlan {
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

use omega::language::core::layout;
use calling;

boundary trait WindowProcedure {
    machine call(message: u64) -> u64;
}

boundary trait UnusedProcedure {
    machine call(message: u64) -> u64;
}

data Spread {}

WndClassWindowProcedureSlot:
    Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;

SecondaryWndClassWindowProcedureSlot:
    Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;

UnusedWindowProcedureSlot:
    Spread satisfies PrivateCallbackSlot<UnusedProcedure::call>;

machine Spread::plan(&mut self, schema: Schema) -> Plan {
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 },
    };
    let plan: Plan = Plan {
        entries: owned_entries,
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

const INTERRUPT_POLICY: &str = r#"
use omega::language::std::calling;
use omega::language::core::interrupt;

data PicAckCarrier {
    physical_root: u64;
    execution: u64;
    invocation: u64;
    policy: u64;
    acknowledgement: u64;
}

PicAckRepresentation:
    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;

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
            true -> size(signature, root)
            _ -> reject()
        }
    }

    state size(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].byte_size == 40 {
            true -> alignment(signature, root)
            _ -> reject()
        }
    }

    state alignment(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition signature.shapes[root].alignment == 8 {
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

const INTERRUPT_REPRESENTATION_BUILD: &str = r#"
machine build(builder: &mut Build) {
    builder.application("interrupt-entry");
    builder.select_representation<
        InterruptAcknowledgement,
        PicAckRepresentation
    >();
}
"#;

const INTERRUPT_OPAQUE_RESULT_POLICY: &str = r#"
data InterruptResultPolicy { }
InterruptResultPolicyCallingPolicy: InterruptResultPolicy satisfies CallingPolicy;

machine InterruptResultPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 0 && signature.has_result {
        true -> accept(signature, signature.result)
        _ -> reject()
    }

    state accept(signature: BoundarySignature, result: u64) -> BoundaryPlanResult {
        transition result < 256 {
            true -> build(signature, result)
            _ -> reject()
        }
    }

    state build(signature: BoundarySignature, result: u64) -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.convention = CallingConvention::SystemVAMD64;
        output.call.has_result = true;
        output.call.result.shape.class = AbiValueClass::Integer;
        output.call.result.shape.byte_size = signature.shapes[result].byte_size;
        output.call.result.shape.alignment = signature.shapes[result].alignment;
        output.call.result.location_count = 1;
        output.call.result.locations[0] = ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register {
                register: MachineRegister::X86Rdi,
            },
            has_copy: false,
            copy_stack_byte_offset: 0,
            byte_size: signature.shapes[result].byte_size,
            alignment: signature.shapes[result].alignment,
        };
        output.call.stack_alignment = 16;
        output.call.entry_control = EntryControl::CallReturn;
        output.state.initial_regime = MachineRegime::X86Long64;
        output.state.stack = EntryStack::ProviderSelected;
        output.state.preemption = Preemption::NotApplicable;
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "opaque result policy requires exactly one result",
            },
        }
    }
}

boundary trait InterruptResult: Calling<InterruptResultPolicy> {
    machine issue() -> InterruptAcknowledgement;
}
"#;

fn interrupt_envelope_policy(fields: &str, extra_declarations: &str) -> String {
    let lookalike_requirement = format!(
        "data InterruptEnvelope {{\n{fields}\n}}\n\nboundary trait LookalikeEntry: Calling<X86InterruptPolicy> {{\n    machine enter(envelope: InterruptEnvelope)\n    reaches PortIo;\n}}"
    );
    let representation_declarations = format!(
        "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;\n\n{extra_declarations}"
    );
    INTERRUPT_POLICY
        .replace(
            "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
            &representation_declarations,
        )
        .replace(
            "boundary trait LookalikeEntry: Calling<X86InterruptPolicy> {\n    machine enter(acknowledgement: InterruptAcknowledgement in Pending)\n    reaches PortIo;\n}",
            &lookalike_requirement,
        )
        .replace(
            "data LookalikeEntryProvider { }\nLookalikeEntryProviderLookalikeEntry: LookalikeEntryProvider satisfies LookalikeEntry;\n\nmachine LookalikeEntryProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)\n    satisfies LookalikeEntry::enter\n    reaches PortIo\n{\n    acknowledgement.complete();\n}\n\n",
            "",
        )
        .replace(
            "signature.shapes[root].byte_size == 40",
            "signature.shapes[root].byte_size == 40 || signature.shapes[root].byte_size == 80",
        )
}

fn retained_interrupt_representation(
    checked: &compiler::CheckedCompilation,
) -> &representation_planning::OpaqueRepresentationSelection {
    let [selection] = checked.opaque_representation_selections() else {
        panic!("one exact opaque-representation selection")
    };
    let opaque = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "InterruptAcknowledgement")
        .expect("exact opaque declaration");
    let carrier = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "PicAckCarrier")
        .expect("exact representation carrier");
    let conformance = checked
        .conformances()
        .iter()
        .find(|definition| {
            checked
                .symbols
                .display_path(definition.symbol, "::")
                .ends_with("PicAckRepresentation")
        })
        .expect("exact representation conformance");
    let build = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("authoritative build machine");

    assert_eq!(selection.opaque(), opaque.symbol);
    assert_eq!(selection.carrier(), carrier.symbol);
    assert_eq!(selection.application().declaration, conformance.symbol);
    assert_eq!(
        selection.application().subject_identity.as_deref(),
        Some("PicAckCarrier")
    );
    assert!(!selection.application().commitment.is_zero());
    assert_eq!(selection.selecting_machine(), build.symbol);
    let selecting_source = checked
        .symbols
        .source_file(selection.source_span())
        .expect("selection must retain authored source custody");
    assert_eq!(
        selecting_source
            .path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("build.omg")
    );
    assert!(selection.source_span().span.start < selection.source_span().span.end);
    selection
}

const FOREIGN_OPAQUE_SOURCE: &str = r#"
use omega::language::core::external_binding;
use omega::language::core::representation;

pub boundary data ForeignToken;

data ForeignTokenCarrier {
    low: u64;
    high: u64;
}

ForeignTokenRepresentation:
    ForeignTokenCarrier satisfies OpaqueRepresentation<ForeignToken>;

boundary trait ForeignChannel {
    machine deliver(token: ForeignToken);
}

windows_x86_64 machine deliver_binding() -> Binding<7, 7, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "foreign",
            export: "deliver",
        },
    }
}

machine deliver_leaf(token: ForeignToken)
    satisfies ForeignChannel::deliver
    via deliver_binding();

data Main { channel: ForeignChannel; }
machine Main::main(&mut self) { }
"#;

const FOREIGN_OPAQUE_BUILD: &str = r#"
machine build(builder: &mut Build) {
    builder.application("foreign-channel");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.select_representation<ForeignToken, ForeignTokenRepresentation>();
}
"#;
