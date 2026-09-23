//! Checked hosted program entry must provision the authored receiver under
//! exact custody all the way from source to the published process image.
//!
//! This is the differential suite's leg of the canary `entry_and_abi::
//! hosted_receiver*` contract: an authored `Binding<Console>`-carrying
//! receiver compiles through checked trees, retains its binding row on the
//! admitted native object, replays independently against corruption, and —
//! on the one host whose produced ELF can execute here — is run natively and
//! witnessed at exit status and stdout. A bare interface spelling of the
//! same field must refuse before any binding is admitted.
//!
//! The project, its package graph and its two acceptance rows are authored
//! by `common/hosted_receiver_project.rs`, which the `hosted_receiver`
//! target includes too; only these two witnesses live here.

#[path = "common/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "common/hosted_receiver_project.rs"]
mod hosted_receiver_project;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::process::Command;

use compiler::{CheckedCompileRequest, compile_to_checked};
use hosted_receiver_project::{
    author_hosted_receiver_project, compile_hosted_receiver, hosted_receiver_package_inputs,
    project_directory,
};

#[test]
fn hosted_receiver_checked_entry_provisions_and_executes_on_linux_x86_64() {
    let project = project_directory("linux");
    let root = author_hosted_receiver_project(&project, false, true);
    let report = compile_hosted_receiver(&root, &project);
    let receiver = report
        .retained_native_artifact()
        .expect("retain admitted native object")
        .object()
        .hosted_receiver_binding()
        .expect("retain exact provisioned receiver");
    assert_eq!(
        receiver.receiver_byte_count(),
        260,
        "scalar and fixed array both occupy the image-backed receiver"
    );

    // Independent replay from the published parts rejects custody drift in
    // every receiver row the checked entry established.
    let object = report
        .retained_native_artifact()
        .expect("retain admitted native object")
        .object();
    let fragments = object
        .fragment_source_for_test()
        .expect("retain physical fragment source");
    image_emission::validate_function_fragment_object_artifact(fragments, object)
        .expect("complete bound object must independently replay");
    let mut receiver_size = object.clone();
    *receiver_size
        .hosted_receiver_byte_count_mut_for_test()
        .unwrap() += 1;
    let mut stack_demand = object.clone();
    *stack_demand
        .hosted_receiver_stack_ceiling_mut_for_test()
        .unwrap() += 16;
    let mut receiver_source = object.clone();
    let source = receiver_source
        .hosted_receiver_source_mut_for_test()
        .unwrap();
    *source = program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        source.target_slot(),
        source.machine_symbol(),
        source.state_symbol(),
        source.machine_name().into(),
        source.state_name().into(),
        source.normalized_callable_identity().into(),
        program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
        source.visible_parameters().to_vec(),
    )
    .expect("free declaration is valid alone, but cannot replace this receiver occurrence");
    for (name, corrupted) in [
        ("receiver size", receiver_size),
        ("stack demand", stack_demand),
        ("source receiver", receiver_source),
    ] {
        assert!(
            image_emission::validate_function_fragment_object_artifact(fragments, &corrupted)
                .is_err(),
            "independent fragment replay must reject changed {name}"
        );
    }

    let report = report
        .publish_retained_native_artifact(&project.join("build"))
        .expect("publish exact admitted native artifact after corruption controls");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = std::fs::read(executable).expect("read published ELF");
    assert_eq!(
        bytes.get(..4),
        Some([0x7f, 0x45, 0x4c, 0x46].as_slice()),
        "the published hosted receiver must be an ELF image"
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // The kernel arrives at the emitted ELF entry point through the
        // exact hosted bridge: a working bridge is the only way the receiver
        // reaches the semantic continuation.
        let output = Command::new(executable)
            .output()
            .expect("execute authored Linux hosted receiver process");
        assert_eq!(
            output.status.code(),
            Some(0),
            "unexpected process completion: {output:?}"
        );
        assert_eq!(
            output.stdout, b"A",
            "receiver must begin at zero and retain its write"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    eprintln!(
        "SKIP: hosted receiver runtime requires Linux x86-64; source compile, custody replay, and image publication still asserted"
    );
}

#[test]
fn hosted_receiver_checked_entry_rejects_bare_interface_field() {
    let project = project_directory("bare");
    let root = author_hosted_receiver_project(&project, false, false);
    let inputs = hosted_receiver_package_inputs(&project);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root, Some("linux_x86_64"))
    })
    .expect_err("a bare interface field is not a service carrier");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("the intrinsic `Binding<R>` carrier is the only service value spelling")),
        "unexpected bare-carrier rejection: {diagnostics:#?}"
    );
}
