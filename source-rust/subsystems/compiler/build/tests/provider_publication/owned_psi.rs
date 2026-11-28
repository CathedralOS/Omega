//! Swap two independently built suppliers of the same bundled requirement.
use super::*;
use psi_authorization::AuthorizedTerminalArtifact;
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue};
use terminal_runtime::embedding::OwnedPsiProvider;

fn supplier(label: &str, value: i32, export: bool) -> (AuthorizedTerminalArtifact, Vec<u8>) {
    let tmp =
        platform_custody::temporary_directory::TemporaryDirectory::create(label, false).unwrap();
    let path = tmp.path().join("build.omg");
    let choose = if export {
        "Build::export_provider::<|streams|::SandboxInput, |app|::Input>(b);"
    } else {
        "Build::select_provider::<|streams|::SandboxInput, |app|::Input>(b, linux_x86_64);"
    };
    std::fs::write(
        &path,
        format!(
            r#"use main.omg as app;
use {{omega}}/language/core/sandbox/streams.omg as streams;
machine build(b: @mutable BuildPlan) {{
    b.product = BuildProduct::Package {{ name: "{label}" }};
    {choose}
}}"#
        ),
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("main.omg"),
        format!(
            r#"use {{omega}}/language/core/sandbox/streams.omg as streams;
public Input satisfies |streams|::SandboxInput {{
    machine read_byte() -> i32 reaches none crashes none {{ {value}i32 }}
}}"#
        ),
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (stage, inputs) = run_project_build(&path, &[target]);
    let product = stage
        .settle_dependency(inputs.clone())
        .unwrap()
        .open_product(inputs)
        .unwrap();
    let stage = product.product_stage(&[target]).unwrap();
    let produced = source_compiler::compile_for_target(stage.source(), target.target_name(), None)
        .unwrap()
        .into_parts()
        .0;
    let desc =
        native_artifact::describe_portable_component(&produced, &Default::default()).unwrap();
    let received = CanonicalTerminalArtifact::from_bytes(&produced.to_bytes()).unwrap();
    let trust = HostProducerTrust::accept("checked fixture supplier");
    (
        authorize_terminal_artifact(Arc::new(received), Some(&trust)).unwrap(),
        desc,
    )
}

#[test]
fn failed_replacement_preserves_the_current_installed_export() {
    let (editor, private_description) = supplier("editor", 7, false);
    let (quiet, quiet_description) = supplier("quiet", 29, true);
    let (loud, loud_description) = supplier("loud", 31, true);
    let module = editor.artifact().semantic_module();
    let [fixed] = module.interfaces.provider_selections.as_slice() else {
        panic!("one static selection")
    };
    let schema = module
        .service_schema(fixed.schema_declaration, fixed.schema_application)
        .unwrap();
    let [method] = schema.surface.methods.as_slice() else {
        panic!("one operation")
    };
    let slot = module
        .interfaces
        .provider_candidates
        .iter()
        .find(|c| {
            c.provider_identity == fixed.provider_identity
                && module.identities.text(c.source_requirement_identity)
                    == Some(schema.surface.identity_text(method.requirement_identity))
        })
        .unwrap()
        .boundary;
    let load = |image, desc: &[u8]| {
        OwnedPsiProvider::load(
            &editor,
            fixed.schema_declaration,
            fixed.schema_application,
            image,
            desc,
            terminal_interpreter::TerminalRuntimeSupplyRequest {
                native_code_layouts: None,
                x86_flags: None,
                memory_ordering: None,
                entry: None,
                installation: None,
                host_imports: &[],
                external_completions: &[],
                target_machines: None,
                target_layouts: None,
            },
        )
    };
    let read = |preview: &OwnedPsiProvider| {
        let member = preview.member(&editor, slot).unwrap();
        let mut call = member.start(Default::default()).unwrap();
        let result = call
            .invoke(&mut terminal_work::WorkMeter::with_budget(1000))
            .unwrap();
        let TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Signed(value),
                ..
            },
        )) = result
        else {
            panic!("expected a byte from the selected image, got {result:?}")
        };
        value
    };
    let mut preview = load(quiet, &quiet_description).unwrap();
    assert_eq!(read(&preview), 29);
    let mismatch = load(loud.clone(), &quiet_description)
        .err()
        .expect("wrong image");
    assert!(
        mismatch.contains("is not the admitted subject"),
        "{mismatch}"
    );
    let not_exported = load(editor.clone(), &private_description)
        .err()
        .expect("private record");
    assert!(
        not_exported.contains("does not export the requested closed provider record"),
        "{not_exported}"
    );
    assert!(
        preview.member(&loud, slot).is_none(),
        "a slot belongs to its receiver"
    );
    assert_eq!(read(&preview), 29);
    preview = load(loud, &loud_description).unwrap();
    assert_eq!(read(&preview), 31);
}
