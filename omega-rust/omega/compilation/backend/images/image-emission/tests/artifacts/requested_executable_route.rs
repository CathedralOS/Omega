//! The requested-executable router boundary: an object's own normalized ELF
//! imports select its writer, and each arm's custody replays independently.
//! Until now this route was only exercised end-to-end by
//! `compiler/tests/source_evaluated_native_realization/linux_dynamic_realization.rs`;
//! these tests pin the route boundary inside the owning crate: an
//! import-bearing ELF object cannot fall back to the installable direct
//! writer, a non-import object cannot be forced into dynamic custody, a
//! rejected request returns its consumed interpreter, the direct arm binds
//! the subsystem only into PE output and a code-signature identifier only
//! into Mach-O bytes, the production bridge rejoins admitted bytes to their
//! exact object, and independent replay rejects substituted object or
//! direct-image custody.

use super::{WriteExitProvider, linux_foreign_call_plan, port_effect_plan, two_function_plan};
use image_emission::{
    ExecutableImageEmissionRequest, ImageApplicationMetadataError, RequestedExecutableImage,
    RequestedExecutableImageError, build_object_artifact, emit_admitted_dynamic_elf_image,
    emit_direct_executable_image, emit_dynamic_elf_image, emit_executable_image,
    validate_dynamic_elf_image_emission, validate_executable_image,
    validate_requested_dynamic_elf_image,
};
use target::{ApplicationIdentifier, HostedApplicationIntent, NativeTarget, TargetProfile};

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
    let image = emit_executable_image(
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
    validate_executable_image(&artifact, &image)
        .expect("requested image replays against its exact object");
    validate_requested_dynamic_elf_image(&artifact, dynamic)
        .expect("the dynamic branch replays independently");
}

#[test]
fn import_bearing_elf_object_fails_closed_on_direct_or_substituted_custody() {
    let artifact = linux_import_artifact(91);

    // The direct writer cannot absorb an import-bearing ELF object: the
    // request fails closed and names the target plus consumed subsystem.
    let error = emit_executable_image(&artifact, ExecutableImageEmissionRequest::direct(3))
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
    let direct_image =
        emit_direct_executable_image(&direct_object, 3).expect("non-import direct image");
    let error =
        validate_executable_image(&artifact, &RequestedExecutableImage::Direct(direct_image))
            .expect_err("a direct image cannot cover an import-bearing ELF object");
    assert!(
        error
            .message
            .contains("import-bearing ELF object was substituted into direct image custody"),
    );

    // A dynamic image emitted for a different import-bearing object does not
    // retain this object; replay binds the exact source artifact.
    let donor = linux_import_artifact(97);
    let donor_image = emit_executable_image(
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
    let error = emit_executable_image(
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
    let glibc = emit_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter()),
    )
    .expect("glibc dynamic image");
    let musl_interpreter = target::normalize_elf_interpreter_plan(
        b"/lib/ld-musl-x86_64.so.1".to_vec(),
        TargetProfile::LinuxX64,
    )
    .expect("musl interpreter");
    let musl = emit_executable_image(
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

/// The exact `PT_INTERP` segment contents of one ELF64 image, `None` when the
/// image has no interpreter program header. Parsed from the emitted bytes, so
/// the check sees what a Linux loader would read, not a writer-side record.
fn pt_interp(bytes: &[u8]) -> Option<Vec<u8>> {
    const PT_INTERP: u32 = 3;
    let read_u16 =
        |offset: usize| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
    let read_u32 =
        |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let read_u64 =
        |offset: usize| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    assert!(
        bytes.starts_with(b"\x7fELF\x02\x01"),
        "ELF64 little-endian image"
    );
    let program_headers = read_u64(0x20) as usize;
    let entry_size = usize::from(read_u16(0x36));
    let count = usize::from(read_u16(0x38));
    (0..count).find_map(|index| {
        let header = program_headers + index * entry_size;
        (read_u32(header) == PT_INTERP).then(|| {
            let offset = read_u64(header + 8) as usize;
            let size = read_u64(header + 32) as usize;
            bytes[offset..offset + size].to_vec()
        })
    })
}

#[test]
fn build_loader_settles_import_bearing_elf_output_into_dynamic_pt_interp() {
    let artifact = linux_import_artifact(91);
    let musl = target::normalize_elf_interpreter_plan(
        b"/lib/ld-musl-x86_64.so.1".to_vec(),
        TargetProfile::LinuxX64,
    )
    .expect("musl interpreter");
    for (loader, expected) in [
        (
            target::declared_elf_interpreter_default(TargetProfile::LinuxX64)
                .expect("Linux x86-64 environment default"),
            b"/lib64/ld-linux-x86-64.so.2\0".as_slice(),
        ),
        (musl, b"/lib/ld-musl-x86_64.so.1\0".as_slice()),
    ] {
        let request = ExecutableImageEmissionRequest::direct(3)
            .settle_elf_loader(&artifact, Some(&loader))
            .expect("an import-bearing ELF object accepts the configured loader");
        let ExecutableImageEmissionRequest::DynamicElf { interpreter } = &request else {
            panic!("an import-bearing ELF object settles into a dynamic request");
        };
        assert_eq!(interpreter, &loader);
        let image = emit_executable_image(&artifact, request).expect("dynamic image");
        let RequestedExecutableImage::DynamicElf(dynamic) = &image else {
            panic!("the settled request selects dynamic custody");
        };
        assert_eq!(
            pt_interp(&dynamic.output().bytes).as_deref(),
            Some(expected),
            "PT_INTERP carries exactly the configured loader and its terminator",
        );
        validate_executable_image(&artifact, &image).expect("dynamic image replays");
    }
}

#[test]
fn build_loader_never_makes_static_output_dynamic() {
    let artifact = build_object_artifact(&port_effect_plan(&WriteExitProvider(970)))
        .expect("non-import Linux object");
    let loader = target::declared_elf_interpreter_default(TargetProfile::LinuxX64)
        .expect("Linux x86-64 environment default");
    let request = ExecutableImageEmissionRequest::direct(3)
        .settle_elf_loader(&artifact, Some(&loader))
        .expect("a static object keeps its direct request");
    assert!(matches!(
        request,
        ExecutableImageEmissionRequest::Direct { subsystem: 3, .. }
    ));
    let image = emit_executable_image(&artifact, request).expect("static image");
    let RequestedExecutableImage::Direct(direct) = &image else {
        panic!("a loader alone must not select dynamic linking");
    };
    assert_eq!(
        pt_interp(&direct.output().bytes),
        None,
        "static output has no PT_INTERP"
    );

    // Non-ELF objects ignore the loader configuration the same way.
    let windows = windows_object();
    assert!(matches!(
        ExecutableImageEmissionRequest::direct(3)
            .settle_elf_loader(&windows, Some(&loader))
            .expect("a COFF object keeps its direct request"),
        ExecutableImageEmissionRequest::Direct { subsystem: 3, .. }
    ));
}

#[test]
fn import_bearing_elf_output_without_loader_rejects_as_missing_configuration() {
    let artifact = linux_import_artifact(91);
    let error = ExecutableImageEmissionRequest::direct(3)
        .settle_elf_loader(&artifact, None)
        .expect_err("dynamic output without a loader lacks output configuration");
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
            .contains("missing output configuration"),
        "unexpected diagnostic: {}",
        error.diagnostic().message,
    );

    // A loader for another target is refused with its custody intact rather
    // than written into this object's PT_INTERP.
    let arm64 = target::declared_elf_interpreter_default(TargetProfile::LinuxArm64)
        .expect("Linux ARM64 environment default");
    let error = ExecutableImageEmissionRequest::direct(3)
        .settle_elf_loader(&artifact, Some(&arm64))
        .expect_err("a foreign-target loader cannot serve this object");
    assert_eq!(error.into_unexpected_interpreter(), Some(arm64));
}

fn windows_object() -> image_emission::ObjectArtifact {
    let mut plan = two_function_plan();
    plan.target = NativeTarget::windows_x64();
    build_object_artifact(&plan).expect("COFF object")
}

fn macos_object() -> image_emission::ObjectArtifact {
    let mut plan = two_function_plan();
    plan.target = NativeTarget::macos_arm64();
    for function in &mut plan.functions {
        function.bytes = [0x5280_00e0u32, 0xd65f_03c0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
    }
    build_object_artifact(&plan).expect("Mach-O object")
}

#[test]
fn non_import_elf_object_selects_direct_route_and_replays_exact_custody() {
    let artifact = build_object_artifact(&port_effect_plan(&WriteExitProvider(970)))
        .expect("non-import Linux object");
    assert!(artifact.object().layout.normalized_imports.is_empty());
    let image = emit_executable_image(&artifact, ExecutableImageEmissionRequest::direct(3))
        .expect("non-import ELF object selects the direct writer");
    let RequestedExecutableImage::Direct(direct) = &image else {
        panic!("non-import ELF object must produce direct custody");
    };
    assert!(direct.output().bytes.starts_with(b"\x7fELF"));
    assert_eq!(
        direct.subsystem(),
        None,
        "ELF custody carries no subsystem fact",
    );
    validate_executable_image(&artifact, &image)
        .expect("the requested direct image replays against its exact object");

    // The router still binds custody at this layer: a direct image emitted
    // for one object cannot be rejoined to a different object.
    let other = build_object_artifact(&two_function_plan()).expect("other Linux object");
    let error = validate_executable_image(&other, &image)
        .expect_err("a direct image does not cover a substituted object");
    assert!(
        error
            .message
            .contains("different semantic or evidence identity"),
    );
}

#[test]
fn direct_request_binds_subsystem_only_into_pe_output() {
    let artifact = windows_object();
    assert!(artifact.object().layout.normalized_imports.is_empty());
    let image = emit_executable_image(&artifact, ExecutableImageEmissionRequest::direct(3))
        .expect("COFF object selects the direct writer");
    let RequestedExecutableImage::Direct(direct) = &image else {
        panic!("a COFF object must produce direct custody");
    };
    assert!(direct.output().bytes.starts_with(b"MZ"));
    assert_eq!(direct.subsystem(), Some(3));
    validate_executable_image(&artifact, &image)
        .expect("the PE image replays against its exact object");

    let other = emit_executable_image(&artifact, ExecutableImageEmissionRequest::direct(5))
        .expect("a second subsystem emits");
    assert_ne!(
        direct.output().bytes,
        other.output().bytes,
        "the consumed subsystem is bound into the emitted image bytes",
    );
}

#[test]
fn direct_request_binds_code_signature_identifier_into_macho_output() {
    let artifact = macos_object();
    let identifier = ApplicationIdentifier::new(b"com.example.receiver").unwrap();
    let mut request = ExecutableImageEmissionRequest::direct(0);
    request
        .settle_application_metadata(
            artifact.target(),
            Some(HostedApplicationIntent::Gui),
            0,
            Some(&identifier),
        )
        .expect("application identity is inherited before signing");
    let image = emit_executable_image(&artifact, request).expect("signed Mach-O image");
    let RequestedExecutableImage::Direct(direct) = &image else {
        panic!("a Mach-O object must produce direct custody");
    };
    assert_eq!(
        image_macho::code_signature_identifier(&direct.output().bytes).as_deref(),
        Some("com.example.receiver"),
        "the consumed identifier is embedded verbatim in the CodeDirectory",
    );

    // Without a bound identity the writer falls back to the validated
    // executable leaf as the ad-hoc label.
    let unsigned = emit_executable_image(&artifact, ExecutableImageEmissionRequest::direct(0))
        .expect("unsigned Mach-O image");
    assert_eq!(
        image_macho::code_signature_identifier(&unsigned.output().bytes).as_deref(),
        Some("omega-program"),
    );
    assert_ne!(
        direct.output().bytes,
        unsigned.output().bytes,
        "the bound signing identity changes the emitted image bytes",
    );
    validate_executable_image(&artifact, &image)
        .expect("the signed image replays against its exact object");
}

#[test]
fn application_metadata_rejects_substituted_subsystem_and_identifier() {
    let identifier = ApplicationIdentifier::new(b"com.example.receiver").unwrap();
    let mut request = ExecutableImageEmissionRequest::direct(3)
        .with_code_signature_identifier(Some("com.example.other".to_owned()));
    assert_eq!(
        request.settle_application_metadata(
            NativeTarget::macos_arm64(),
            Some(HostedApplicationIntent::Gui),
            2,
            Some(&identifier),
        ),
        Err(ImageApplicationMetadataError::SubsystemMismatch),
        "loader metadata is checked before the signing identity",
    );
    assert_eq!(
        request.settle_application_metadata(
            NativeTarget::macos_arm64(),
            Some(HostedApplicationIntent::Gui),
            3,
            Some(&identifier),
        ),
        Err(ImageApplicationMetadataError::IdentifierMismatch),
    );
    assert_eq!(
        request.code_signature_identifier(),
        Some("com.example.other")
    );
    let mut exact = ExecutableImageEmissionRequest::direct(3)
        .with_code_signature_identifier(Some(identifier.as_str().to_owned()));
    exact
        .settle_application_metadata(
            NativeTarget::macos_arm64(),
            Some(HostedApplicationIntent::Gui),
            3,
            Some(&identifier),
        )
        .expect("an explicit exact identity remains valid");
}

#[test]
fn application_metadata_requires_identity_only_for_macho_gui() {
    for target in [NativeTarget::macos_arm64(), NativeTarget::macos_x64()] {
        let mut request = ExecutableImageEmissionRequest::direct(2);
        assert_eq!(
            request.settle_application_metadata(
                target,
                Some(HostedApplicationIntent::Gui),
                2,
                None
            ),
            Err(ImageApplicationMetadataError::MissingGuiIdentifier),
        );
        for intent in [None, Some(HostedApplicationIntent::Console)] {
            request
                .settle_application_metadata(target, intent, 2, None)
                .expect("a raw PE GUI word does not imply hosted GUI intent");
            assert_eq!(request.code_signature_identifier(), None);
        }
        request = request.with_code_signature_identifier(Some("com.example.consumer".to_owned()));
        request
            .settle_application_metadata(target, Some(HostedApplicationIntent::Gui), 2, None)
            .expect("a consumer may supply an identity when the retained application omitted it");
    }
    for target in [NativeTarget::windows_x64(), NativeTarget::linux_x64()] {
        ExecutableImageEmissionRequest::direct(2)
            .settle_application_metadata(target, Some(HostedApplicationIntent::Gui), 2, None)
            .expect("non-Mach-O GUI output needs no signing identity");
    }
}

#[test]
fn application_metadata_preserves_dynamic_interpreter_on_success_and_refusal() {
    let expected = interpreter();
    let identifier = ApplicationIdentifier::new(b"com.example.receiver").unwrap();
    let mut request = ExecutableImageEmissionRequest::dynamic_elf(expected.clone());
    request
        .settle_application_metadata(
            NativeTarget::linux_x64(),
            Some(HostedApplicationIntent::Gui),
            2,
            Some(&identifier),
        )
        .expect("dynamic ELF has no subsystem or signing metadata to substitute");
    assert_eq!(request.code_signature_identifier(), None);
    assert_eq!(
        request.settle_application_metadata(
            NativeTarget::macos_arm64(),
            Some(HostedApplicationIntent::Gui),
            2,
            Some(&identifier),
        ),
        Err(ImageApplicationMetadataError::MissingGuiIdentifier),
    );
    let ExecutableImageEmissionRequest::DynamicElf { interpreter } = request else {
        panic!("metadata settlement cannot change the image request variant");
    };
    assert_eq!(
        interpreter, expected,
        "refusal retains exact loader custody"
    );
}

#[test]
fn empty_code_signature_identifier_fails_closed() {
    let artifact = macos_object();
    let error = emit_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::direct(0)
            .with_code_signature_identifier(Some(String::new())),
    )
    .expect_err("an empty signing identity must not fall back to the leaf");
    assert!(matches!(*error, RequestedExecutableImageError::Direct(_)));
    assert!(
        error
            .diagnostic()
            .message
            .contains("code signature identifier is empty"),
    );
}

#[test]
fn dynamic_elf_request_discards_the_code_signature_identifier() {
    let request = ExecutableImageEmissionRequest::dynamic_elf(interpreter())
        .with_code_signature_identifier(Some("com.example.ignored".to_string()));
    assert_eq!(
        request.code_signature_identifier(),
        None,
        "a dynamic request carries no Mach-O signing identity",
    );
    let artifact = linux_import_artifact(91);
    let decorated = emit_executable_image(&artifact, request)
        .expect("dynamic image with a discarded identifier");
    let plain = emit_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter()),
    )
    .expect("undecorated dynamic image");
    assert_eq!(
        decorated.output().bytes,
        plain.output().bytes,
        "a discarded identity cannot perturb the admitted dynamic bytes",
    );
}

#[test]
fn dynamic_elf_custody_replay_requires_exact_normalized_imports() {
    let artifact = linux_import_artifact(91);
    let image = emit_executable_image(
        &artifact,
        ExecutableImageEmissionRequest::dynamic_elf(interpreter()),
    )
    .expect("dynamic image");
    let RequestedExecutableImage::DynamicElf(dynamic) = &image else {
        unreachable!("the import-bearing object retains its normalized import")
    };
    let plain = build_object_artifact(&port_effect_plan(&WriteExitProvider(970)))
        .expect("non-import Linux object");
    let error = validate_requested_dynamic_elf_image(&plain, dynamic)
        .expect_err("a non-import artifact cannot host dynamic ELF custody");
    assert!(
        error
            .message
            .contains("requires exact normalized ELF imports"),
    );
}

#[test]
fn admitted_dynamic_elf_bridge_rejoins_and_replays_exact_custody() {
    let artifact = linux_import_artifact(91);
    // The complete owner chain is also a direct public entry: it consumes the
    // interpreter and returns admitted-byte custody without the router's
    // artifact-binding wrapper.
    let emission = emit_dynamic_elf_image(&artifact, interpreter())
        .expect("the dynamic ELF owner chain emits admitted custody");
    assert!(emission.output().bytes.starts_with(b"\x7fELF"));
    validate_dynamic_elf_image_emission(&artifact, &emission)
        .expect("the production bridge replays independently");
    let expected_bytes = emission.output().bytes.clone();

    // The production bridge rejoins admitted bytes to the emitted-output
    // surface byte-exactly; the result replays the same custody.
    let rejoined = emit_admitted_dynamic_elf_image(&artifact, emission.into_admitted())
        .expect("admitted bytes rejoin their exact object");
    assert_eq!(rejoined.output().bytes, expected_bytes);
    validate_dynamic_elf_image_emission(&artifact, &rejoined)
        .expect("the rejoined emission replays the same custody");
}

#[test]
fn admitted_dynamic_elf_bridge_rejects_a_substituted_object_and_keeps_custody() {
    let artifact = linux_import_artifact(91);
    let emission = emit_dynamic_elf_image(&artifact, interpreter()).expect("dynamic emission");
    let expected_bytes = emission.output().bytes.clone();
    let substituted = build_object_artifact(&port_effect_plan(&WriteExitProvider(970)))
        .expect("non-import Linux object");

    // Independent replay does not follow a substituted object: the replayed
    // image is rebuilt from the supplied artifact, so foreign custody rejects.
    assert!(
        validate_dynamic_elf_image_emission(&substituted, &emission).is_err(),
        "admitted custody does not follow a substituted object",
    );

    // The production bridge fails closed the same way, and its rejection
    // retains the admitted owner so orchestration can retry or inspect it.
    let error = emit_admitted_dynamic_elf_image(&substituted, emission.into_admitted())
        .expect_err("the bridge rejects admitted bytes against a foreign object");
    let (recovered, _diagnostic) = error.into_parts();
    let rejoined = emit_admitted_dynamic_elf_image(&artifact, recovered)
        .expect("the retained admitted bytes still join their exact object");
    assert_eq!(
        rejoined.output().bytes,
        expected_bytes,
        "rejection retained the admitted bytes byte-exactly",
    );
}
