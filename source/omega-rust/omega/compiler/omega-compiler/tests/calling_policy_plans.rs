use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use omega_compiler::{compile_to_checked, compile_to_checked_with_packages};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use omega_provider_planning::calling_policy_plans::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryValueClass, evaluate_calling_policy_plan,
};
use omega_provider_planning::plans::{
    selected_external_root_entry_fact_bindings, selected_external_root_provider_plan,
    selected_external_root_provider_plan_id,
};

use std::fs;
use std::path::PathBuf;

use psi_core::PackageKeyIdentity;

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
    compile_to_checked(&write_project(name, source, build), None)
        .expect_err("negative opaque representation project must reject")
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn compile_std_negative(name: &str, source: &str) -> String {
    let (path, package_inputs) = write_callback_package(name, source);
    let result = compile_to_checked_with_packages(&path, Some("windows_x86_64"), package_inputs);
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
        .nth(5)
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

fn write_callback_package(name: &str, source: &str) -> (PathBuf, PackageCompilationInputs) {
    let directory = std::env::temp_dir().join(format!(
        "omega-calling-policy-package-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create callback package fixture");
    fs::copy(
        repository_root().join("source/library/std/calling.omg"),
        directory.join("calling.omg"),
    )
    .expect("copy package-local calling vocabulary");
    let main = directory.join("main.omg");
    fs::write(&main, source).expect("write callback package source");

    let package = PackageKeyIdentity::from_digest([73; 32])
        .expect("nonzero callback fixture package identity");
    let inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "calling-policy-fixture",
            directory,
        )],
        Vec::new(),
    )
    .expect("callback fixture package graph");
    (main, inputs)
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
target windows_x86_64 {
}

use omega::language::core::layout;
use calling;

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
    let source = callback_fixture_source("callback_materialization_closure.omg");
    assert_eq!(
        source,
        CALLBACK_MATERIALIZATION_POLICY.trim_start_matches('\n'),
        "the source canary and its readable test fixture must remain identical"
    );
    let (main_path, package_inputs) =
        write_callback_package("materialization-closure", CALLBACK_MATERIALIZATION_POLICY);
    compile_to_checked_with_packages(&main_path, Some("windows_x86_64"), package_inputs)
        .expect("target-selected registrar should consume both exact closed layout demands");
}

#[test]
fn direct_callback_parameter_is_interleaved_without_a_source_runtime_argument() {
    let source = callback_fixture_source("direct_callback_parameter.omg");
    let (main_path, package_inputs) = write_callback_package("direct-callback", &source);
    let checked =
        compile_to_checked_with_packages(&main_path, Some("windows_x86_64"), package_inputs)
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
    let [nominal_use] = checked.facts.nominal_machine_uses.uses.as_slice() else {
        panic!("the actual registrar occurrence must retain one selected callback use");
    };
    assert!(matches!(
        nominal_use.site,
        psi_checked_trees::NominalMachineUseSite::Statement(_)
    ));
    let [placement] = checked.callback_placements() else {
        panic!("the actual registrar occurrence must bind one callback placement");
    };
    assert_eq!(placement.site, nominal_use.site);
    assert_eq!(placement.registration_operation, install.symbol);
    let materialization = placement
        .private_materialization
        .as_ref()
        .expect("the direct callback must retain its target-closed native parameter");
    assert!(matches!(
        materialization.destination,
        omega_calling_conventions::NativePlace::Parameter(_)
    ));
    assert_eq!(
        placement.boundary_entry_plan.call.parameters.len(),
        1,
        "the selected callback's inbound ABI stays distinct from the registrar ABI",
    );
    assert_eq!(
        materialization
            .registrar_boundary_entry_plan
            .call
            .parameters
            .len(),
        3
    );
    let application = materialization
        .direct_registrar_parameter_application
        .as_ref()
        .expect("direct callback retains one exact target-closed telescope row");
    let omega_calling_conventions::NativePlace::Parameter(destination) =
        &materialization.destination
    else {
        unreachable!();
    };
    assert_eq!(application.parameter, *destination);
    assert_eq!(application.native_ordinal, 1);
    assert_eq!(
        application.shape,
        omega_calling_conventions::ValueShape::integer(8, 8)
    );
    assert_eq!(
        application.placement,
        materialization
            .registrar_boundary_entry_plan
            .call
            .parameters[1]
    );

    let mutations: [(
        &str,
        fn(&mut omega_calling_conventions::NativeParameterApplication),
    ); 3] = [
        (
            "identity",
            |application: &mut omega_calling_conventions::NativeParameterApplication| {
                application.parameter =
                    omega_calling_conventions::NativeParameterId::new(0xdead).unwrap();
            },
        ),
        ("ordinal", |application| application.native_ordinal = 0),
        ("shape", |application| {
            application.shape = omega_calling_conventions::ValueShape::float(8);
        }),
    ];
    for (label, mutate) in mutations {
        let mut drifted = placement.clone();
        let application = drifted
            .private_materialization
            .as_mut()
            .unwrap()
            .direct_registrar_parameter_application
            .as_mut()
            .unwrap();
        mutate(application);
        assert!(
            omega_backend_plan::validate_bound_nominal_callback_placement(&drifted).is_err(),
            "{label} drift must reject independently",
        );
    }
    let mut missing_application = placement.clone();
    missing_application
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application = None;
    assert!(
        omega_backend_plan::validate_bound_nominal_callback_placement(&missing_application)
            .is_err(),
        "a direct destination cannot lose its target-closed application",
    );
    let mut placement_drift = placement.clone();
    placement_drift
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application
        .as_mut()
        .unwrap()
        .placement = materialization
        .registrar_boundary_entry_plan
        .call
        .parameters[0]
        .clone();
    assert!(
        omega_backend_plan::validate_bound_nominal_callback_placement(&placement_drift).is_err(),
        "physical placement substitution must reject independently",
    );

    let requirement_identity = checked
        .typed
        .normalized_hermetic_symbol_identity(install.symbol)
        .expect("exact registrar requirement identity");
    let mut stale_v1_parameter = placement.clone();
    stale_v1_parameter
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application
        .as_mut()
        .unwrap()
        .parameter =
        omega_calling_conventions::callback_native_parameter_id(&requirement_identity, 1);
    assert!(
        omega_backend_plan::validate_bound_nominal_callback_placement(&stale_v1_parameter).is_err(),
        "the retired ordinal-derived v1 parameter identity must not substitute for v2",
    );

    let mut invented_parameter = placement.clone();
    let invented = omega_calling_conventions::NativeParameterId::new(0xbeef).unwrap();
    let materialization = invented_parameter.private_materialization.as_mut().unwrap();
    materialization.destination = omega_calling_conventions::NativePlace::Parameter(invented);
    materialization
        .direct_registrar_parameter_application
        .as_mut()
        .unwrap()
        .parameter = invented;
    assert!(
        omega_backend_plan::validate_bound_nominal_callback_placement(&invented_parameter).is_err(),
        "a locally coherent policy-created parameter must not enter the declared telescope",
    );

    let mut wrong_binder = placement.clone();
    wrong_binder
        .private_materialization
        .as_mut()
        .unwrap()
        .binder = omega_calling_conventions::StaticMachineBinderId::new(0xcafe).unwrap();
    assert!(
        omega_backend_plan::validate_bound_nominal_callback_placement(&wrong_binder).is_err(),
        "a different binder identity must not retain the direct application",
    );

    let mut wrong_requirement = placement.clone();
    wrong_requirement
        .private_materialization
        .as_mut()
        .unwrap()
        .requirement = omega_calling_conventions::CallbackRequirementId::new(0xfade).unwrap();
    assert!(
        omega_backend_plan::validate_bound_nominal_callback_placement(&wrong_requirement).is_err(),
        "a different callback requirement must not retain the direct application",
    );
}

#[test]
fn opaque_movement_retains_native_ordinal_after_direct_callback_insertion() {
    let source = callback_fixture_source("direct_callback_parameter.omg")
        .replace(
            "use calling;",
            "use calling;\nuse omega::language::core::representation;\n\npub boundary data CallbackToken;\npub data CallbackTokenCarrier { value: u64; }\npub CallbackTokenRepresentation:\n    CallbackTokenCarrier satisfies OpaqueRepresentation<CallbackToken>;",
        )
        .replace("module: u64", "module: CallbackToken")
        .replace(
            "data Main { }\nmachine Main::main(&mut self) {\n    HookRegistrar::install<HookProvider::call>(1u64, 2u64);\n}",
            "data Main { }\nmachine Main::register(&mut self, module: CallbackToken) {\n    HookRegistrar::install<HookProvider::call>(1u64, module);\n}\nmachine Main::main(&mut self) { }",
        );
    let (main_path, package_inputs) =
        write_callback_package("opaque-direct-callback-ordinal", &source);
    fs::write(
        main_path
            .parent()
            .expect("callback package directory")
            .join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("opaque-direct-callback-ordinal");
    builder.select_representation<CallbackToken, CallbackTokenRepresentation>();
}
"#,
    )
    .expect("write callback opaque-representation selection");
    let checked =
        compile_to_checked_with_packages(&main_path, Some("windows_x86_64"), package_inputs)
            .expect("opaque registrar parameter should close around the direct callback");
    let opaque = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "CallbackToken")
        .expect("exact callback-token opaque declaration");
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            realization
                .materialized_signature
                .opaque_representation_uses()
                .iter()
                .any(|representation| representation.opaque() == opaque.symbol)
                && realization
                    .replayed_validated_application()
                    .is_ok_and(|(validated, _, _)| validated.plan().call.parameters.len() == 3)
        })
        .expect("one closed registrar signature with its compiler-inserted callback");
    let representation = realization
        .materialized_signature
        .opaque_representation_uses()
        .iter()
        .find(|representation| representation.opaque() == opaque.symbol)
        .expect("one exact callback-token occurrence");
    let (validated, _, _) = realization
        .replayed_validated_application()
        .expect("callback-interleaved opaque plan should replay exactly");
    let movement = realization
        .materialized_signature
        .opaque_representation_movement(representation, &validated)
        .expect("opaque occurrence must rejoin after direct callback insertion");
    assert!(matches!(
        movement.role(),
        BoundaryOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 1,
            native_ordinal: 2,
        }
    ));
    assert!(movement.path().is_empty());
    assert_eq!(movement.placement(), &validated.plan().call.parameters[2]);
    assert!(matches!(
        movement.placement().locations.as_slice(),
        [omega_calling_conventions::ValueLocation::Register {
            register: omega_calling_conventions::MachineRegister::X86R8,
            ..
        }]
    ));
    let _ = fs::remove_dir_all(
        main_path
            .parent()
            .expect("temporary callback package directory"),
    );
}

#[test]
fn direct_callback_parameter_requires_a_bodyless_boundary_requirement() {
    let source = callback_fixture_source("direct_callback_parameter.omg")
        .replace("boundary trait HookRegistrar", "trait HookRegistrar");
    let (main_path, package_inputs) =
        write_callback_package("direct-callback-nonboundary", &source);
    let diagnostics =
        compile_to_checked_with_packages(&main_path, Some("windows_x86_64"), package_inputs)
            .expect_err("a non-boundary trait cannot declare a native callback parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("permitted only on a bodyless boundary-trait requirement")
    }));
}

#[test]
fn direct_callback_parameter_requires_its_exact_nominal_binder() {
    let source = callback_fixture_source("direct_callback_parameter.omg").replace(
        "native callback procedure from Handler",
        "native callback procedure from Missing",
    );
    let (main_path, package_inputs) =
        write_callback_package("direct-callback-missing-binder", &source);
    let diagnostics =
        compile_to_checked_with_packages(&main_path, Some("windows_x86_64"), package_inputs)
            .expect_err("a direct callback cannot infer or invent its binder");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown machine binder `Missing`")
    }));
}

#[test]
fn direct_callback_parameter_rejects_inferred_duplicate_and_unconstrained_binders() {
    let source = callback_fixture_source("direct_callback_parameter.omg");
    for (name, mutated, expected) in [
        (
            "inferred-binder",
            source.replace(
                "native callback procedure from Handler",
                "native callback procedure",
            ),
            "from",
        ),
        (
            "duplicate-name",
            source.replace(
                "native callback procedure from Handler,\n        module",
                "native callback procedure from Handler,\n        native callback procedure from Handler,\n        module",
            ),
            "native callback parameter `procedure` is declared more than once",
        ),
        (
            "duplicate-binder",
            source.replace(
                "native callback procedure from Handler,\n        module",
                "native callback procedure from Handler,\n        native callback backup from Handler,\n        module",
            ),
            "is assigned to more than one declared native callback parameter",
        ),
        (
            "unconstrained-binder",
            source.replace(
                "    )\n    where machine Handler satisfies HookProcedure::call;",
                "    );",
            ),
            "requires an authored declaration-site contract",
        ),
    ] {
        let rendered = compile_std_negative(name, &mutated);
        assert!(
            rendered.contains(expected),
            "{name} produced unexpected diagnostics:\n{rendered}",
        );
    }
}

#[test]
fn authored_addr_parameter_cannot_substitute_for_a_native_callback_declaration() {
    let source = callback_fixture_source("direct_callback_parameter.omg")
        .replace("native callback procedure from Handler", "procedure: addr");
    let rendered = compile_std_negative("authored-addr", &source);
    assert!(
        rendered.contains("invalid direct callback telescope"),
        "authored addr substitution produced unexpected diagnostics:\n{rendered}",
    );
}

#[test]
fn callback_private_materialization_requires_an_explicit_cited_demand() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "    let placed: Plan =\n        Plan::place_private<WndClassWindowProcedureSlot>(plan, 8);\n    Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "    plan",
    );
    let rendered = compile_std_negative("callback-uncited-demand", &source);

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
    let rendered = compile_std_negative("callback-wrong-layout", &source);

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
    let rendered = compile_std_negative("callback-ambiguous-requirement", &source);

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
    checked: &omega_compiler::CheckedCompilation,
) -> &omega_representation_planning::OpaqueRepresentationSelection {
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

#[test]
fn source_interrupt_policy_publishes_and_selects_the_complete_entry_plan() {
    let main_path = write_project(
        "interrupt-entry",
        INTERRUPT_POLICY,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(&main_path, None).expect("interrupt policy should compile");
    let restore = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptMaskGuard::restore")
        .expect("checked-in core restore requirement");
    assert!(restore.is_public);
    assert_eq!(
        restore.supply_mode,
        psi_language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    assert!(!restore.body_is_present);
    assert!(!restore.service_reach_is_installation_bound);
    let restore_snapshot = checked
        .typed
        .snapshot()
        .roots
        .machines
        .into_iter()
        .find(|machine| machine.name == "InterruptMaskGuard::restore")
        .expect("restore requirement snapshot");
    assert_eq!(restore_snapshot.service_reach, ["MachineControl"]);
    let [restore_contract] = restore_snapshot.contracts.as_slice() else {
        panic!("restore requirement must retain only its Active precondition")
    };
    assert_eq!(restore_contract.kind, "requires");
    let [psi_typed_trees::snapshot::ProofFactSnapshot::Membership { domain, value, .. }] =
        restore_contract.facts.as_slice()
    else {
        panic!("restore requirement must retain one membership precondition")
    };
    assert_eq!(domain, &["InterruptMaskGuard", "Active"]);
    assert!(matches!(
        value,
        psi_typed_trees::snapshot::ExpressionSnapshot::Name { path }
            if path == &["self"]
    ));
    let complete = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("checked-in core completion requirement");
    assert!(complete.is_public);
    assert_eq!(
        complete.supply_mode,
        psi_language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    assert!(!complete.body_is_present);
    assert!(!complete.service_reach_is_installation_bound);
    let complete_snapshot = checked
        .typed
        .snapshot()
        .roots
        .machines
        .into_iter()
        .find(|machine| machine.name == "InterruptAcknowledgement::complete")
        .expect("completion requirement snapshot");
    assert_eq!(complete_snapshot.service_reach, ["PortIo"]);
    let [complete_contract] = complete_snapshot.contracts.as_slice() else {
        panic!("completion requirement must retain only its Pending precondition")
    };
    assert_eq!(complete_contract.kind, "requires");
    let [psi_typed_trees::snapshot::ProofFactSnapshot::Membership { domain, value, .. }] =
        complete_contract.facts.as_slice()
    else {
        panic!("completion requirement must retain one membership precondition")
    };
    assert_eq!(domain, &["InterruptAcknowledgement", "Pending"]);
    assert!(matches!(
        value,
        psi_typed_trees::snapshot::ExpressionSnapshot::Name { path }
            if path == &["self"]
    ));
    let selection = retained_interrupt_representation(&checked);
    let timer_entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "TimerProvider::enter")
        .expect("timer adapter");
    let timer_entry = checked
        .typed
        .machine_states(timer_entry)
        .first()
        .expect("timer entry state");
    let acknowledgement = checked
        .typed
        .state_parameters(timer_entry)
        .first()
        .expect("timer acknowledgement parameter");
    let acknowledgement_layout = omega_layout::layout_type_reference(
        &checked,
        omega_target::NativeTarget::host(),
        checked.opaque_representation_selections(),
        acknowledgement.type_reference,
    )
    .expect("selected opaque representation must supply general by-value layout");
    assert_eq!(acknowledgement_layout.size, 40);
    assert_eq!(acknowledgement_layout.alignment, 8);
    let demanded_realizations = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| {
            realization
                .materialized_signature
                .opaque_representation_uses()
                .iter()
                .any(|use_| use_.opaque() == selection.opaque())
        })
        .collect::<Vec<_>>();
    assert!(
        !demanded_realizations.is_empty(),
        "the by-value boundary crossing must retain its exact representation use"
    );
    assert!(demanded_realizations.iter().all(|realization| {
        realization
            .materialized_signature
            .opaque_representation_uses()
            .iter()
            .filter(|use_| use_.opaque() == selection.opaque())
            .all(|use_| {
                use_.conformance() == selection.application().declaration
                    && use_.carrier() == selection.carrier()
                    && usize::from(use_.shape_root())
                        < realization.materialized_signature.shapes().len()
                    && use_.application_report_fingerprint()
                        == selection.application().report_fingerprint
                    && use_.conformance_application_commitment()
                        == selection.application().commitment.as_bytes()
                    && use_.representation_schema_version() == selection.schema_version()
                    && use_.origin() == selection.origin()
                    && use_.lifecycle() == selection.lifecycle()
                    && use_.copy_disposition() == selection.copy_disposition()
                    && use_.selected_application_commitment()
                        == selection.selected_application_commitment()
                    && use_.rederived_selected_application_commitment()
                        == use_.selected_application_commitment()
            })
            && realization.replayed_validated_application().is_ok_and(
                |(_, report_fingerprint, commitment)| {
                    report_fingerprint == realization.report_fingerprint
                        && commitment == realization.commitment
                },
            )
    }));
    for realization in &demanded_realizations {
        let (validated, _, _) = realization
            .replayed_validated_application()
            .expect("opaque boundary use must replay its exact validated plan");
        for representation in realization
            .materialized_signature
            .opaque_representation_uses()
            .iter()
            .filter(|use_| use_.opaque() == selection.opaque())
        {
            let movement = realization
                .materialized_signature
                .opaque_representation_movement(representation, &validated)
                .expect("opaque shape node must rejoin one exact ABI placement");
            assert!(matches!(
                movement.role(),
                omega_provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole::Parameter { .. }
            ));
            assert_eq!(movement.placement().shape.byte_size, 40);
            assert_eq!(movement.placement().shape.alignment, 8);
            assert!(!movement.placement().locations.is_empty());
        }
    }

    let timer = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "TimerRoot")
        .expect("TimerRoot boundary trait");
    let schema = omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, timer)
        .expect("TimerRoot service schema");
    assert!(schema.methods[0].calling_plan_report_fingerprint.is_some());
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
fn selected_opaque_representation_supplies_nested_general_layout() {
    let source = INTERRUPT_POLICY.replace(
        "data Main { }",
        "data InterruptEnvelope { acknowledgement: InterruptAcknowledgement }\n\ndata Main { }",
    );
    let main_path = write_project(
        "interrupt-nested-layout",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("selected opaque representation should close nested layout");
    let layouts = omega_layout::build_layout_plan(
        &checked,
        omega_target::NativeTarget::host(),
        checked.opaque_representation_selections(),
    )
    .expect("general layout should consume the selected carrier");
    let envelope = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "InterruptEnvelope")
        .expect("nested opaque envelope layout");
    assert_eq!(envelope.layout.size, 40);
    assert_eq!(envelope.layout.alignment, 8);
    let omega_layout::DataShape::Record { fields } = envelope.shape else {
        panic!("interrupt envelope should remain a semantic record")
    };
    let [acknowledgement] = layouts.fields.span_or_empty(fields) else {
        panic!("interrupt envelope should retain one semantic field")
    };
    assert_eq!(
        acknowledgement.type_name.as_ref(),
        "InterruptAcknowledgement"
    );
    assert_eq!(acknowledgement.layout.size, 40);
    assert_eq!(acknowledgement.layout.alignment, 8);
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn opaque_result_rejoins_its_exact_result_placement() {
    let source = format!("{INTERRUPT_POLICY}\n{INTERRUPT_OPAQUE_RESULT_POLICY}");
    let main_path = write_project(
        "interrupt-opaque-result-movement",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked =
        compile_to_checked(&main_path, None).expect("opaque result policy should compile");
    let selection = retained_interrupt_representation(&checked);
    let mut result_movements = 0;

    for realization in checked.boundary_calling_plan_realizations() {
        let (validated, _, _) = realization
            .replayed_validated_application()
            .expect("opaque result plan should replay exactly");
        for representation in realization
            .materialized_signature
            .opaque_representation_uses()
            .iter()
            .filter(|representation| representation.opaque() == selection.opaque())
        {
            let movement = realization
                .materialized_signature
                .opaque_representation_movement(representation, &validated)
                .expect("opaque result must rejoin one exact result placement");
            if movement.role() != BoundaryOpaqueRepresentationMovementRole::Result {
                continue;
            }
            result_movements += 1;
            assert!(movement.path().is_empty());
            assert_eq!(
                realization.materialized_signature.result(),
                Some(representation.shape_root())
            );
            assert_eq!(movement.placement().shape.byte_size, 40);
            assert_eq!(movement.placement().shape.alignment, 8);
            assert!(matches!(
                movement.placement().locations.as_slice(),
                [omega_calling_conventions::ValueLocation::Indirect { .. }]
            ));
        }
    }

    assert_eq!(
        result_movements, 1,
        "the authored result boundary should retain one opaque result movement"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn nested_opaque_path_ignores_an_identically_shaped_ordinary_field() {
    let source = interrupt_envelope_policy(
        "    ordinary: PicAckCarrier;\n    acknowledgement: InterruptAcknowledgement;",
        "",
    );
    let main_path = write_project(
        "interrupt-nested-opaque-movement",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("nested opaque representation policy should compile");
    let selection = retained_interrupt_representation(&checked);
    let mut nested_movements = 0;

    for realization in checked.boundary_calling_plan_realizations() {
        let (validated, _, _) = realization
            .replayed_validated_application()
            .expect("nested opaque plan should replay exactly");
        for representation in realization
            .materialized_signature
            .opaque_representation_uses()
            .iter()
            .filter(|representation| representation.opaque() == selection.opaque())
        {
            let movement = realization
                .materialized_signature
                .opaque_representation_movement(representation, &validated)
                .expect("nested opaque must rejoin one exact parameter placement");
            if movement.path()
                != [BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 1 }]
            {
                continue;
            }
            nested_movements += 1;
            assert!(matches!(
                movement.role(),
                BoundaryOpaqueRepresentationMovementRole::Parameter {
                    formal_ordinal: 0,
                    native_ordinal: 0,
                }
            ));
            assert_eq!(movement.placement().shape.byte_size, 80);
            assert_eq!(movement.placement().shape.alignment, 8);

            let [parameter_root] = realization.materialized_signature.parameters() else {
                panic!("nested boundary should retain one semantic parameter")
            };
            let root = realization.materialized_signature.shapes()[usize::from(*parameter_root)];
            let BoundaryValueClass::Record {
                first_field,
                field_count: 2,
            } = root.class()
            else {
                panic!("nested boundary parameter should remain a two-field record")
            };
            let fields = &realization.materialized_signature.fields()
                [usize::from(first_field)..usize::from(first_field) + 2];
            let ordinary_root = fields[0].shape();
            let opaque_root = fields[1].shape();
            assert_eq!(opaque_root, representation.shape_root());
            assert_ne!(ordinary_root, representation.shape_root());
            let ordinary = realization.materialized_signature.shapes()[usize::from(ordinary_root)];
            let opaque = realization.materialized_signature.shapes()[usize::from(opaque_root)];
            assert_eq!(ordinary.byte_size(), opaque.byte_size());
            assert_eq!(ordinary.alignment(), opaque.alignment());
            assert_eq!(
                realization
                    .materialized_signature
                    .opaque_representation_uses()
                    .iter()
                    .filter(|candidate| candidate.opaque() == selection.opaque())
                    .count(),
                1,
                "the equal ordinary carrier field must not be marked as opaque"
            );
        }
    }

    assert_eq!(
        nested_movements, 1,
        "the authored envelope should retain one exact nested opaque movement"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn repeated_opaque_values_rejoin_distinct_equal_layout_occurrences() {
    let source = interrupt_envelope_policy(
        "    first: InterruptAcknowledgement;\n    second: InterruptAcknowledgement;",
        "",
    );
    let main_path = write_project(
        "interrupt-repeated-opaque-movement",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("repeated opaque representation policy should compile");
    let selection = retained_interrupt_representation(&checked);
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            realization
                .materialized_signature
                .opaque_representation_uses()
                .iter()
                .filter(|representation| representation.opaque() == selection.opaque())
                .count()
                == 2
        })
        .expect("one boundary signature with two exact opaque occurrences");
    let (validated, _, _) = realization
        .replayed_validated_application()
        .expect("repeated opaque plan should replay exactly");
    let uses = realization
        .materialized_signature
        .opaque_representation_uses()
        .iter()
        .filter(|representation| representation.opaque() == selection.opaque())
        .collect::<Vec<_>>();
    let [first, second] = uses.as_slice() else {
        panic!("two exact repeated opaque markers")
    };
    assert_ne!(first.shape_root(), second.shape_root());
    let first_shape = realization.materialized_signature.shapes()[usize::from(first.shape_root())];
    let second_shape =
        realization.materialized_signature.shapes()[usize::from(second.shape_root())];
    assert_eq!(first_shape.byte_size(), second_shape.byte_size());
    assert_eq!(first_shape.alignment(), second_shape.alignment());

    let movements = uses
        .iter()
        .map(|representation| {
            realization
                .materialized_signature
                .opaque_representation_movement(representation, &validated)
                .expect("each repeated marker must rejoin its own occurrence")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        movements
            .iter()
            .map(|movement| movement.path())
            .collect::<Vec<_>>(),
        [
            &[BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 0 }][..],
            &[BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 1 }][..],
        ]
    );
    assert!(movements.iter().all(|movement| {
        matches!(
            movement.role(),
            BoundaryOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal: 0,
                native_ordinal: 0,
            }
        ) && movement.placement().shape.byte_size == 80
    }));
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn distinct_opaque_values_with_equal_layout_retain_distinct_nominal_markers() {
    let source = interrupt_envelope_policy(
        "    acknowledgement: InterruptAcknowledgement;\n    shadow: ShadowAcknowledgement;",
        "pub boundary data ShadowAcknowledgement;\n\ndata ShadowAckCarrier {\n    physical_root: u64;\n    execution: u64;\n    invocation: u64;\n    policy: u64;\n    acknowledgement: u64;\n}\n\nShadowAckRepresentation:\n    ShadowAckCarrier satisfies OpaqueRepresentation<ShadowAcknowledgement>;",
    );
    let build = INTERRUPT_REPRESENTATION_BUILD.replace(
        "    >();",
        "    >();\n    builder.select_representation<\n        ShadowAcknowledgement,\n        ShadowAckRepresentation\n    >();",
    );
    let main_path = write_project("interrupt-equal-opaque-movement", &source, &build);
    let checked = compile_to_checked(&main_path, None)
        .expect("equal-layout opaque representation policy should compile");
    let opaque_symbols = ["InterruptAcknowledgement", "ShadowAcknowledgement"].map(|name| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("exact `{name}` opaque declaration"))
            .symbol
    });
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            opaque_symbols.iter().all(|opaque| {
                realization
                    .materialized_signature
                    .opaque_representation_uses()
                    .iter()
                    .any(|representation| representation.opaque() == *opaque)
            })
        })
        .expect("one boundary signature carrying both opaque declarations");
    let (validated, _, _) = realization
        .replayed_validated_application()
        .expect("equal-layout opaque plan should replay exactly");
    let uses = opaque_symbols.map(|opaque| {
        realization
            .materialized_signature
            .opaque_representation_uses()
            .iter()
            .find(|representation| representation.opaque() == opaque)
            .expect("one exact nominal opaque marker")
    });
    assert_ne!(uses[0].opaque(), uses[1].opaque());
    assert_ne!(uses[0].carrier(), uses[1].carrier());
    assert_ne!(uses[0].shape_root(), uses[1].shape_root());
    let shapes = uses.map(|representation| {
        realization.materialized_signature.shapes()[usize::from(representation.shape_root())]
    });
    assert_eq!(shapes[0].byte_size(), shapes[1].byte_size());
    assert_eq!(shapes[0].alignment(), shapes[1].alignment());
    let movements = uses.map(|representation| {
        realization
            .materialized_signature
            .opaque_representation_movement(representation, &validated)
            .expect("equal layout must not substitute one nominal marker for another")
    });
    assert_eq!(
        movements[0].path(),
        [BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 0 }]
    );
    assert_eq!(
        movements[1].path(),
        [BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 1 }]
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn opaque_by_value_boundary_rejects_without_build_selection() {
    let rendered = compile_project_negative(
        "interrupt-missing-representation",
        INTERRUPT_POLICY,
        "machine build(builder: &mut Build) { builder.application(\"interrupt-entry\"); }",
    );
    assert!(
        rendered.contains("InterruptAcknowledgement")
            && rendered.contains("authoritative build selects no exact"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn reference_only_opaque_boundary_retains_unused_selection_without_demanding_one() {
    let source = INTERRUPT_POLICY
        .replace(
            "boundary trait TimerRoot: InterruptEntry + Calling<X86InterruptPolicy> {",
            "boundary trait TimerRoot: Calling<X86InterruptPolicy> {\n    machine inspect(acknowledgement: &InterruptAcknowledgement);",
        )
        .replace("signature.shapes[root].byte_size == 40", "signature.shapes[root].byte_size == 8")
        .replace(
            "machine enter(acknowledgement: InterruptAcknowledgement in Pending)\n    reaches PortIo;",
            "machine enter(acknowledgement: &InterruptAcknowledgement);",
        )
        .replace(
            "machine TimerProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)\n    satisfies InterruptEntry::enter\n    reaches PortIo\n{\n    acknowledgement.complete();\n}",
            "machine TimerProvider::inspect(acknowledgement: &InterruptAcknowledgement)\n    satisfies TimerRoot::inspect\n{\n}",
        )
        .replace(
            "machine LookalikeEntryProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)\n    satisfies LookalikeEntry::enter\n    reaches PortIo\n{\n    acknowledgement.complete();\n}",
            "machine LookalikeEntryProvider::enter(acknowledgement: &InterruptAcknowledgement)\n    satisfies LookalikeEntry::enter\n{\n}",
        )
        .replace(
            "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
            "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;\n\nAlternatePicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
        );
    let unselected = write_project(
        "interrupt-reference-only-unselected-representation",
        &source,
        "machine build(builder: &mut Build) { builder.application(\"interrupt-entry\"); }",
    );
    let unselected = compile_to_checked(&unselected, None)
        .expect("a reference-only opaque pointee must not demand representation closure");
    assert!(unselected.opaque_representation_selections().is_empty());
    let inspect = unselected
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "TimerProvider::inspect")
        .expect("reference-only timer adapter");
    let inspect = unselected
        .typed
        .machine_states(inspect)
        .first()
        .expect("reference-only entry state");
    let acknowledgement = unselected
        .typed
        .state_parameters(inspect)
        .first()
        .expect("reference-only acknowledgement");
    let reference_layout = omega_layout::layout_type_reference(
        &unselected,
        omega_target::NativeTarget::host(),
        unselected.opaque_representation_selections(),
        acknowledgement.type_reference,
    )
    .expect("reference layout must not demand an opaque representation");
    assert_eq!(
        reference_layout.size,
        omega_target::NativeTarget::host().pointer_size
    );
    assert_eq!(
        reference_layout.alignment,
        omega_target::NativeTarget::host().pointer_alignment
    );
    let psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } = unselected
        .typed
        .type_reference_table
        .type_reference(acknowledgement.type_reference)
    else {
        panic!("reference-only acknowledgement should retain its referee")
    };
    let diagnostic = omega_layout::layout_type_reference(
        &unselected,
        omega_target::NativeTarget::host(),
        unselected.opaque_representation_selections(),
        *referee,
    )
    .expect_err("the same opaque requested by value must require a selection");
    assert!(
        diagnostic
            .message
            .contains("requires one exact representation")
    );

    let selected = write_project(
        "interrupt-reference-only-selected-representation",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let selected = compile_to_checked(&selected, None)
        .expect("an unused valid selection remains activation policy");
    let selected_application = retained_interrupt_representation(&selected).application();
    assert!(
        selected
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature
                .opaque_representation_uses()
                .is_empty())
    );

    let alternate_build = INTERRUPT_REPRESENTATION_BUILD
        .replace("PicAckRepresentation", "AlternatePicAckRepresentation");
    let alternate = write_project(
        "interrupt-reference-only-alternate-representation",
        &source,
        &alternate_build,
    );
    let alternate = compile_to_checked(&alternate, None)
        .expect("an alternate unused valid selection remains activation policy");
    let [alternate_selection] = alternate.opaque_representation_selections() else {
        panic!("one alternate unused opaque-representation selection")
    };
    assert_eq!(
        selected.selected_provider_plans(),
        alternate.selected_provider_plans(),
        "an unused representation selection must not change calling-plan settlement"
    );
    assert_ne!(
        selected_application.commitment,
        alternate_selection.application().commitment,
        "unused selection custody must retain the exact closed application identity"
    );
    assert_ne!(
        selected, alternate,
        "unused opaque-representation policy must participate in checked semantic equality"
    );
}

#[test]
fn opaque_representation_rejects_duplicate_build_selection() {
    let duplicate = INTERRUPT_REPRESENTATION_BUILD.replace(
        "    >();",
        "    >();\n    builder.select_representation<InterruptAcknowledgement, PicAckRepresentation>();",
    );
    let rendered = compile_project_negative(
        "interrupt-duplicate-representation",
        INTERRUPT_POLICY,
        &duplicate,
    );
    assert!(
        rendered.contains("selects opaque representation") && rendered.contains("more than once"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn opaque_representation_rejects_conformance_for_another_opaque_type() {
    let source = INTERRUPT_POLICY.replace(
        "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
        "pub boundary data OtherAcknowledgement [linear];\n\nPicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<OtherAcknowledgement>;",
    );
    let rendered = compile_project_negative(
        "interrupt-mismatched-representation",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    assert!(
        rendered.contains("represents `OtherAcknowledgement`")
            && rendered.contains("InterruptAcknowledgement"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn opaque_representation_rejects_a_trait_lookalike() {
    let source = INTERRUPT_POLICY.replace(
        "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
        "trait RepresentationLookalike<Opaque> { }\n\nPicAckRepresentation:\n    PicAckCarrier satisfies RepresentationLookalike<InterruptAcknowledgement>;",
    );
    let rendered = compile_project_negative(
        "interrupt-lookalike-representation",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    assert!(
        rendered.contains("does not satisfy the exact compiler-owned")
            && rendered.contains("OpaqueRepresentation"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn changing_the_selected_opaque_conformance_reissues_the_calling_application() {
    fn timer_calling_application(checked: &omega_compiler::CheckedCompilation) -> u64 {
        let timer = checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "TimerRoot")
            .expect("TimerRoot boundary trait");
        omega_effects::provider_plan::ServiceSchema::from_typed(&checked.typed, timer)
            .expect("TimerRoot service schema")
            .methods[0]
            .calling_plan_report_fingerprint
            .expect("TimerRoot calling application")
    }

    let first = compile_to_checked(
        &write_project(
            "interrupt-representation-identity-first",
            INTERRUPT_POLICY,
            INTERRUPT_REPRESENTATION_BUILD,
        ),
        None,
    )
    .expect("first representation application");
    let alternate_source =
        INTERRUPT_POLICY.replace("PicAckRepresentation:", "AlternatePicAckRepresentation:");
    let alternate_build = INTERRUPT_REPRESENTATION_BUILD
        .replace("PicAckRepresentation", "AlternatePicAckRepresentation");
    let alternate = compile_to_checked(
        &write_project(
            "interrupt-representation-identity-alternate",
            &alternate_source,
            &alternate_build,
        ),
        None,
    )
    .expect("alternate representation application");

    let first_selection = retained_interrupt_representation(&first);
    let [alternate_selection] = alternate.opaque_representation_selections() else {
        panic!("one alternate opaque-representation selection")
    };
    assert_ne!(
        first_selection.application().commitment,
        alternate_selection.application().commitment,
        "the retained closed application commitment must change with its named conformance"
    );
    assert_ne!(
        timer_calling_application(&first),
        timer_calling_application(&alternate),
        "an exact named-conformance substitution must reissue the calling application even when carrier shape is unchanged"
    );
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
    let application_report = schema.methods[0]
        .calling_plan_report_fingerprint
        .expect("boundary method must publish its complete calling-plan application");
    assert_ne!(
        application_report,
        validated.contract_report_fingerprint(),
        "the retained application must bind target and semantic signature beside the accepted policy output"
    );
    let retained = checked
        .typed
        .boundary_calling_plans
        .iter()
        .find(|identity| identity.report_fingerprint == application_report)
        .expect("typed semantic identity for the published boundary contract");
    assert_ne!(retained.report_fingerprint, 0);
    assert_ne!(
        retained.commitment.as_bytes(),
        validated.contract_commitment_digest(),
        "raw policy acceptance is not the complete target-closed application"
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
