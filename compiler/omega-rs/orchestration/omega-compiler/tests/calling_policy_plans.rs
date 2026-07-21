use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use omega_compiler::{compile_to_checked, evaluate_calling_policy_plan};
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
