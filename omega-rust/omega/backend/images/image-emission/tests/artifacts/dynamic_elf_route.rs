//! The dynamic ELF publication route selects its writer from the object's own
//! normalized imports and replays the emitted custody independently. Until now
//! this route was only exercised end-to-end by
//! `compiler/tests/source_evaluated_native_realization/linux_dynamic_realization.rs`;
//! these tests pin the route boundary inside the owning crate: an
//! import-bearing ELF object cannot fall back to the installable direct
//! writer, a non-import object cannot be forced into dynamic custody, a
//! rejected request returns its consumed interpreter, and independent replay
//! rejects substituted object or direct-image custody.

use super::{WriteExitProvider, linux_foreign_call_plan, port_effect_plan};
use image_emission::{
    ExecutableImageEmissionRequest, RequestedExecutableImage, RequestedExecutableImageError,
    build_object_artifact, emit_executable_image, emit_requested_executable_image,
    validate_requested_dynamic_elf_image, validate_requested_executable_image,
};
use target::{NativeTarget, TargetProfile};

fn interpreter() -> target::NormalizedElfInterpreterPlan {
    target::normalize_elf_interpreter_plan(
        b"/lib64/ld-linux-x86-64.so.2".to_vec(),
        TargetProfile::LinuxX64,
    )
    .expect("canonical Linux x86-64 interpreter")
}

fn linux_import_artifact(provider_identity: u64) -> image_emission::ObjectArtifact {
    build_object_artifact(&linux_foreign_call_plan(&WriteExitProvider(
        provider_identity,
    )))
    .expect("import-bearing Linux object")
}

#[test]
fn import_bearing_elf_object_selects_dynamic_route_and_replays_exact_custody() {
    let artifact = linux_import_artifact(91);
    assert_eq!(
        artifact.object().layout.normalized_imports.len(),
        1,
        "one versioned ELF import selects the dynamic route",
    );
    let image = emit_requested_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter()),
    )
    .expect("import-bearing ELF object selects the dynamic writer");
    let RequestedExecutableImage::DynamicElf(dynamic) = &image else {
        panic!("import-bearing ELF object must produce dynamic custody");
    };
    assert_eq!(dynamic.artifact(), &artifact);
    assert!(dynamic.output().bytes.starts_with(b"\x7fELF"));
    assert_eq!(dynamic.output().final_image_imports, 1);
    validate_requested_executable_image(&artifact, &image)
        .expect("requested image replays against its exact object");
    validate_requested_dynamic_elf_image(&artifact, dynamic)
        .expect("the dynamic branch replays independently");
}

#[test]
fn import_bearing_elf_object_fails_closed_on_direct_or_substituted_custody() {
    let artifact = linux_import_artifact(91);

    // The direct writer cannot absorb an import-bearing ELF object: the
    // request fails closed and names the target plus consumed subsystem.
    let error =
        emit_requested_executable_image(&artifact, ExecutableImageEmissionRequest::direct(3))
            .expect_err("import-bearing ELF object must refuse the direct writer");
    assert!(matches!(
        *error,
        RequestedExecutableImageError::MissingDynamicElfInterpreter {
            target,
            subsystem: 3,
            ..
        } if target == NativeTarget::linux_x64()
    ));
    assert!(
        error
            .diagnostic()
            .message
            .contains("requires an exact normalized interpreter"),
    );

    // A direct image emitted for a non-import object cannot be validated as
    // this object's custody either.
    let direct_object = build_object_artifact(&port_effect_plan(&WriteExitProvider(970)))
        .expect("non-import Linux object");
    assert!(direct_object.object().layout.normalized_imports.is_empty());
    let direct_image = emit_executable_image(&direct_object, 3).expect("non-import direct image");
    let error = validate_requested_executable_image(
        &artifact,
        &RequestedExecutableImage::Direct(direct_image),
    )
    .expect_err("a direct image cannot cover an import-bearing ELF object");
    assert!(
        error
            .message
            .contains("import-bearing ELF object was substituted into direct image custody"),
    );

    // A dynamic image emitted for a different import-bearing object does not
    // retain this object; replay binds the exact source artifact.
    let donor = linux_import_artifact(97);
    let donor_image = emit_requested_executable_image(
        &donor,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter()),
    )
    .expect("donor dynamic image");
    let RequestedExecutableImage::DynamicElf(donor_dynamic) = donor_image else {
        unreachable!("the donor object retains one normalized ELF import")
    };
    assert_ne!(donor_dynamic.artifact(), &artifact);
    let error = validate_requested_dynamic_elf_image(&artifact, &donor_dynamic)
        .expect_err("dynamic custody does not follow a substituted object");
    assert!(
        error
            .message
            .contains("does not retain the exact source object artifact"),
    );
}

#[test]
fn non_import_object_rejects_dynamic_route_and_recovers_interpreter() {
    let artifact = build_object_artifact(&port_effect_plan(&WriteExitProvider(970)))
        .expect("non-import Linux object");
    assert!(artifact.object().layout.normalized_imports.is_empty());
    let interpreter = interpreter();
    let expected = interpreter.clone();
    let error = emit_requested_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter),
    )
    .expect_err("non-import ELF object must refuse the dynamic writer");
    assert!(matches!(
        *error,
        RequestedExecutableImageError::UnexpectedDynamicElfInterpreter { .. }
    ));
    assert!(
        error
            .diagnostic()
            .message
            .contains("cannot select the dynamic writer without normalized ELF imports"),
    );
    let recovered = error
        .into_unexpected_interpreter()
        .expect("the rejected request retains exact interpreter custody");
    assert_eq!(recovered, expected);
}

#[test]
fn interpreter_selection_is_bound_into_emitted_dynamic_bytes() {
    let artifact = linux_import_artifact(91);
    let glibc = emit_requested_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter()),
    )
    .expect("glibc dynamic image");
    let musl_interpreter = target::normalize_elf_interpreter_plan(
        b"/lib/ld-musl-x86_64.so.1".to_vec(),
        TargetProfile::LinuxX64,
    )
    .expect("musl interpreter");
    let musl = emit_requested_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(musl_interpreter),
    )
    .expect("musl dynamic image");
    assert_ne!(
        glibc.output().bytes,
        musl.output().bytes,
        "the consumed interpreter is bound into the emitted image bytes",
    );
}
