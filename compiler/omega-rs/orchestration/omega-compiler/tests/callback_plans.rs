use omega_compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-callback-plan-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create callback-plan test directory");
    let main_path = directory.join("main.omg");
    fs::write(&main_path, source).expect("write callback-plan test program");
    fs::write(
        directory.join("build.omg"),
        r#"
machine build(b: &mut Build) {
    b.accept_boundary<Registrar>();
    b.select_provider<Registrar, RegistrarProvider>();
}
"#,
    )
    .expect("write callback-plan build program");
    main_path
}

const CALLBACK_PROGRAM: &str = r#"
use omega::language::std::calling;

data CallbackPolicy { }
CallbackPolicyCallingPolicy: CallbackPolicy satisfies CallingPolicy;

machine CallbackPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 0 && !signature.has_result {
        true -> accept()
        _ -> reject()
    }

    state accept() -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.stack_alignment = 16;
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection {
                reason: "callback signature must be () -> unit",
            },
        }
    }
}

boundary trait Callback: Calling<CallbackPolicy> {
    machine call();
}

data Application { }
boundary machine Application::dispatch()
    satisfies Callback::call
{
    let marker: u64 = 0;
}

boundary trait Registrar {
    machine install<machine Selected>(&self)
    where machine Selected();
    ensures true;
}

data RegistrarProvider { }

machine RegistrarProvider::install<machine Selected>(&self)
where machine Selected();
satisfies Registrar::install
{
    let marker: u64 = 0;
}

data Main {
    registrar: &Registrar;
}

machine Main::main(&self) {
    self.registrar.install<Application::dispatch>();
}
"#;

#[test]
fn selected_static_boundary_machine_retains_exact_address_free_callback_plan() {
    let main_path = write_program("exact", CALLBACK_PROGRAM);
    let checked = compile_to_checked(&main_path, None).expect("callback program should compile");
    let [binding] = checked.callback_bindings().bindings() else {
        panic!("one contextual callback binding should be retained");
    };

    let callback_trait = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == binding.callback_trait)
        .expect("retained callback trait symbol");
    let callback_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == binding.callback_machine)
        .expect("retained callback machine symbol");

    assert_eq!(callback_trait.name.as_str(), "Callback");
    assert_eq!(callback_machine.name.as_str(), "Application::dispatch");
    assert_eq!(binding.boundary_entry_plan.call.stack_alignment, 16);
    assert_ne!(binding.calling_plan_fingerprint, 0);
    assert_ne!(binding.specialization_fingerprint, 0);
}

#[test]
fn ordinary_static_machine_selection_does_not_become_a_callback() {
    let source = CALLBACK_PROGRAM.replace(
        "self.registrar.install<Application::dispatch>();",
        "// no registration selection",
    );
    let main_path = write_program("unselected", &source);
    let checked = compile_to_checked(&main_path, None).expect("unselected program should compile");

    assert!(checked.callback_bindings().bindings().is_empty());
}

#[test]
fn one_registration_operation_cannot_select_multiple_callback_machines() {
    let source = CALLBACK_PROGRAM
        .replace(
            "data Application { }",
            r#"data Application { }
data OtherApplication { }
boundary machine OtherApplication::dispatch()
    satisfies Callback::call
{
    let marker: u64 = 0;
}"#,
        )
        .replace(
            "    ensures true;\n}",
            r#"    ensures true;

    machine install_pair<machine First, machine Second>(&self)
    where machine First();
    where machine Second();
    ensures true;
}"#,
        )
        .replace(
            "data Main {",
            r#"machine RegistrarProvider::install_pair<machine First, machine Second>(&self)
where machine First();
where machine Second();
satisfies Registrar::install_pair
{
    let marker: u64 = 0;
}

data Main {"#,
        )
        .replace(
            "self.registrar.install<Application::dispatch>();",
            "self.registrar.install_pair<Application::dispatch, OtherApplication::dispatch>();",
        );
    let main_path = write_program("multiple", &source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("multiple callback selections must fail closed");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("selects 2 static callback machines"),
        "unexpected diagnostics:\n{rendered}"
    );
}
