use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    executable_name, fs, interpret, pass_canary, retained_float_differential_result_identity,
};
use compiler::CheckedCompileRequest;

#[test]
fn build_runtime_float_semantics_twins_agree() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.semantic-edge-twins.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "one zero-argument semantic machine at build time and runtime",
        "binary32/binary64 nearest-even ties",
        "subnormal underflow and finite overflow",
        "signed zero, infinity, NaN, and partial comparisons",
        "minimum/maximum, classification, and square root",
        "directed arithmetic and directed FMA",
        "fused versus separately rounded multiply-add",
    ];
    // Provider-plan identity now retains exact package provenance for every
    // schema, requirement owner, provider type, and origin. Builtin float
    // providers carry the explicit unbound identity; the companion stability
    // invariant lives with the float plan suite.
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xc8d6_5187_ebc2_db51;

    let canary = pass_canary(fixture_roster::FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("float semantic twins should compile and evaluate their array length");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine: name } =
            &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_intrinsics.len(),
        56,
        "{DIFFERENTIAL_SUITE_ID} must bind every operation/format edge used by the twin"
    );
    assert_eq!(
        selected_plan_identities.len(),
        56,
        "{DIFFERENTIAL_SUITE_ID} must retain one exact plan per selected intrinsic"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "the runtime half of the float semantic twins should interpret"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "build-time and runtime f32/f64 edge families should agree"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-float-semantic-edge-twins-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float semantic twins should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float semantic twins should run natively");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native runtime must agree with the build-time twin; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let scratch = std::env::temp_dir().join(format!(
        "omega-float-semantic-edge-twins-linux-arm64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let source_dir = scratch.join("src");
    fs::create_dir_all(&source_dir).expect("semantic-edge cross-target source directory");
    fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
        .expect("copy semantic-edge twin canary");
    let standard_library = crate::repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        source_dir.join("build.omg"),
        format!(
            "\
             machine build(builder: &mut Build) {{\n\
                 builder.application(\"float-semantic-edge-twins\");\n\
                 builder.depend(Source::Path {{\n        location: \"{standard_library}\"\n    }});\n\
                 builder.roots.bind(linux_arm64::ProgramEntry, Main::main);\n\
             }}\n"
        ),
    )
    .expect("write semantic-edge build source");
    compile(CanaryCompileSpec {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(scratch.join("out")),
        target_name: Some("linux_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("float semantic twins should compile for linux_arm64: {diagnostics:#?}")
    });
    let _ = fs::remove_dir_all(&scratch);

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, build/runtime edge corpus, interpreter/native results, and cross-target build before refreshing the retained identity"
    );
}

#[test]
fn linux_arm64_float_semantic_edge_twin_retains_artifact_evidence() {
    const SUITE_ID: &str = "omega.float.hardware.linux_arm64.semantic-edge-twin.v1";
    const COVERAGE: &[&str] = &[
        "one zero-argument semantic machine at build time and runtime",
        "binary32/binary64 nearest-even ties",
        "subnormal underflow and finite overflow",
        "signed zero, infinity, NaN, and partial comparisons",
        "minimum/maximum, classification, and square root",
        "directed arithmetic and directed FMA",
        "fused versus separately rounded multiply-add",
    ];
    const EXPECTED_PLAN_COUNT: usize = 56;
    const EXPECTED_BUILD_ARTIFACT_IDENTITY: u64 = 0xfc80_03e9_bf11_3370;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    const EXPECTED_HOSTED_EXECUTION_IDENTITY: u64 = 0xf9bd_4b4f_c0cb_1bbb;

    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let canary = pass_canary(fixture_roster::FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS);
    let scratch = std::env::temp_dir().join(format!(
        "omega-linux-arm64-float-semantic-edge-twin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let source_dir = scratch.join("src");
    fs::create_dir_all(&source_dir).expect("Linux AArch64 semantic-edge source directory");
    fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
        .expect("copy Linux AArch64 semantic-edge twin canary");
    let standard_library = crate::repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        source_dir.join("build.omg"),
        format!(
            "\
             machine build(builder: &mut Build) {{\n\
                 builder.application(\"linux-arm64-float-semantic-edge-twin\");\n\
                 builder.depend(Source::Path {{\n        location: \"{standard_library}\"\n    }});\n\
                 builder.roots.bind(linux_arm64::ProgramEntry, Main::main);\n\
             }}\n"
        ),
    )
    .expect("write Linux AArch64 semantic-edge build source");
    let main_path = source_dir.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("linux_arm64"),
    ))
    .expect("Linux AArch64 float twin should compile and evaluate its array length");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(machine.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(selected_intrinsics.len(), EXPECTED_PLAN_COUNT);
    assert_eq!(selected_plan_identities.len(), EXPECTED_PLAN_COUNT);
    assert!(
        selected_intrinsics
            .iter()
            .any(|intrinsic| intrinsic.contains("fused_multiply_add"))
    );
    assert!(
        selected_intrinsics
            .iter()
            .any(|intrinsic| intrinsic.contains("toward_"))
    );

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let first_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("first-build")),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float twin should produce an exact linux_arm64 image");
    let first_path = first_build
        .checked_native_executable_path()
        .expect("linux_arm64 cross-build must retain its executable receipt");
    let image_bytes = fs::read(first_path).expect("read retained linux_arm64 image bytes");
    assert!(!image_bytes.is_empty());
    assert_eq!(image_bytes.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(
        image_bytes.get(18..20),
        Some([0xb7, 0x00].as_slice()),
        "retained ELF image must name the AArch64 machine"
    );

    let second_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("second-build")),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float twin should reproduce its exact linux_arm64 image");
    let second_bytes = fs::read(
        second_build
            .checked_native_executable_path()
            .expect("second linux_arm64 cross-build must retain its executable receipt"),
    )
    .expect("read second linux_arm64 image bytes");
    assert_eq!(image_bytes, second_bytes);

    let mut build_artifact_identity = 0xcbf29ce484222325_u64;
    retain(&mut build_artifact_identity, SUITE_ID.as_bytes());
    retain(
        &mut build_artifact_identity,
        b"evidence:cross-build-exact-image",
    );
    for category in COVERAGE {
        retain(&mut build_artifact_identity, category.as_bytes());
    }
    for intrinsic in &selected_intrinsics {
        retain(&mut build_artifact_identity, intrinsic.as_bytes());
    }
    for identity in &selected_plan_identities {
        retain(&mut build_artifact_identity, &identity.to_le_bytes());
    }
    retain(
        &mut build_artifact_identity,
        &interpreted.exit_code.to_le_bytes(),
    );
    retain(&mut build_artifact_identity, &interpreted.stdout);
    retain(&mut build_artifact_identity, &interpreted.stderr);
    retain(&mut build_artifact_identity, b"target-profile:linux_arm64");
    retain(&mut build_artifact_identity, b"architecture:aarch64");
    retain(&mut build_artifact_identity, b"object-format:elf");
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_arm64().pointer_size as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_arm64().pointer_alignment as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(&main_path).expect("read retained float semantic-edge source bytes"),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(source_dir.join("build.omg")).expect("read retained Linux AArch64 build binding"),
    );
    retain(&mut build_artifact_identity, &image_bytes);
    assert_eq!(
        build_artifact_identity, EXPECTED_BUILD_ARTIFACT_IDENTITY,
        "{SUITE_ID} build/artifact identity changed ({build_artifact_identity:#018x}); validate the exact plans, interpreter result, target binding, and reproducible image before refreshing it",
    );

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        let output = Command::new(first_path)
            .output()
            .expect("hosted linux_arm64 float twin should execute its retained image");
        assert_eq!(output.status.code(), Some(70));
        let mut execution_identity = build_artifact_identity;
        retain(&mut execution_identity, b"evidence:hosted-native-execution");
        retain(
            &mut execution_identity,
            &output.status.code().unwrap_or_default().to_le_bytes(),
        );
        retain(&mut execution_identity, &output.stdout);
        retain(&mut execution_identity, &output.stderr);
        assert_eq!(
            execution_identity, EXPECTED_HOSTED_EXECUTION_IDENTITY,
            "{SUITE_ID} hosted execution identity changed ({execution_identity:#018x})",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn linux_x64_baseline_float_semantic_edge_twin_retains_artifact_evidence() {
    const SUITE_ID: &str = "omega.float.hardware.linux_x64.baseline-semantic-edge-twin.v1";
    const COVERAGE: &[&str] = &[
        "one zero-argument baseline semantic machine at build time and runtime",
        "binary32/binary64 nearest-even add/subtract/multiply/divide",
        "subnormal underflow and finite overflow",
        "signed zero, infinities, NaNs, and partial comparisons",
        "minimum/maximum, classify/predicates, square root, and negate",
        "separately rounded multiply-then-add",
    ];
    const EXPECTED_PLAN_COUNT: usize = 36;
    const EXPECTED_BUILD_ARTIFACT_IDENTITY: u64 = 0xa237_8240_2355_e2c9;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    const EXPECTED_HOSTED_EXECUTION_IDENTITY: u64 = 0x895e_d190_164e_b67e;

    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let canary = pass_canary(fixture_roster::FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS_X86_BASELINE);
    let main_path = canary.join("main.omg");
    let scratch = std::env::temp_dir().join(format!(
        "omega-x86-baseline-float-semantic-edge-twin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("linux_x86_64"),
    ))
    .expect("baseline x86 float twin should compile and evaluate its array length");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(machine.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(selected_intrinsics.len(), EXPECTED_PLAN_COUNT);
    assert_eq!(selected_plan_identities.len(), EXPECTED_PLAN_COUNT);
    assert!(selected_intrinsics.iter().all(|intrinsic| {
        !intrinsic.contains("toward_") && !intrinsic.contains("fused_multiply_add")
    }));

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let first_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("first-build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline x86 float twin should produce an exact linux_x64 image");
    let first_path = first_build
        .checked_native_executable_path()
        .expect("linux_x64 cross-build must retain its executable receipt");
    let image_bytes = fs::read(first_path).expect("read retained linux_x64 image bytes");
    assert!(!image_bytes.is_empty());
    assert_eq!(image_bytes.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(
        image_bytes.get(18..20),
        Some([0x3e, 0x00].as_slice()),
        "retained ELF image must name the x86-64 machine"
    );

    let second_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("second-build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline x86 float twin should reproduce its exact linux_x64 image");
    let second_bytes = fs::read(
        second_build
            .checked_native_executable_path()
            .expect("second linux_x64 cross-build must retain its executable receipt"),
    )
    .expect("read second linux_x64 image bytes");
    assert_eq!(image_bytes, second_bytes);

    let mut build_artifact_identity = 0xcbf29ce484222325_u64;
    retain(&mut build_artifact_identity, SUITE_ID.as_bytes());
    retain(
        &mut build_artifact_identity,
        b"evidence:cross-build-exact-image",
    );
    for category in COVERAGE {
        retain(&mut build_artifact_identity, category.as_bytes());
    }
    for intrinsic in &selected_intrinsics {
        retain(&mut build_artifact_identity, intrinsic.as_bytes());
    }
    for identity in &selected_plan_identities {
        retain(&mut build_artifact_identity, &identity.to_le_bytes());
    }
    retain(
        &mut build_artifact_identity,
        &interpreted.exit_code.to_le_bytes(),
    );
    retain(&mut build_artifact_identity, &interpreted.stdout);
    retain(&mut build_artifact_identity, &interpreted.stderr);
    retain(&mut build_artifact_identity, b"target-profile:linux_x86_64");
    retain(&mut build_artifact_identity, b"architecture:x86_64");
    retain(&mut build_artifact_identity, b"object-format:elf");
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_x64().pointer_size as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_x64().pointer_alignment as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(&main_path).expect("read retained x86 baseline source bytes"),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(canary.join("build.omg")).expect("read retained linux_x64 build binding"),
    );
    retain(&mut build_artifact_identity, &image_bytes);
    assert_eq!(
        build_artifact_identity, EXPECTED_BUILD_ARTIFACT_IDENTITY,
        "{SUITE_ID} build/artifact identity changed ({build_artifact_identity:#018x}); validate the exact baseline plans, interpreter result, target binding, and reproducible image before refreshing it",
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let output = Command::new(first_path)
            .output()
            .expect("hosted linux_x64 baseline twin should execute its retained image");
        assert_eq!(output.status.code(), Some(70));
        let mut execution_identity = build_artifact_identity;
        retain(&mut execution_identity, b"evidence:hosted-native-execution");
        retain(
            &mut execution_identity,
            &output.status.code().unwrap_or_default().to_le_bytes(),
        );
        retain(&mut execution_identity, &output.stdout);
        retain(&mut execution_identity, &output.stderr);
        assert_eq!(
            execution_identity, EXPECTED_HOSTED_EXECUTION_IDENTITY,
            "{SUITE_ID} hosted execution identity changed ({execution_identity:#018x})",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn windows_x64_baseline_float_semantic_edge_twin_retains_artifact_evidence() {
    const SUITE_ID: &str = "omega.float.hardware.windows_x64.baseline-semantic-edge-twin.v1";
    const COVERAGE: &[&str] = &[
        "one zero-argument baseline semantic machine at build time and runtime",
        "binary32/binary64 nearest-even add/subtract/multiply/divide",
        "subnormal underflow and finite overflow",
        "signed zero, infinities, NaNs, and partial comparisons",
        "minimum/maximum, classify/predicates, square root, and negate",
        "separately rounded multiply-then-add",
    ];
    const EXPECTED_PLAN_COUNT: usize = 36;
    const EXPECTED_BUILD_ARTIFACT_IDENTITY: u64 = 0x0551_4042_2e6f_f7b1;
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    const EXPECTED_HOSTED_EXECUTION_IDENTITY: u64 = 0xa36f_0003_a672_28d6;

    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let canary = pass_canary(fixture_roster::FLOAT_BUILD_RUNTIME_SEMANTICS_TWINS_WINDOWS_X64);
    let main_path = canary.join("main.omg");
    let scratch = std::env::temp_dir().join(format!(
        "omega-windows-x64-baseline-float-semantic-edge-twin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some("windows_x86_64"),
    ))
    .expect("baseline Windows x64 float twin should compile and evaluate its array length");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(machine.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(selected_intrinsics.len(), EXPECTED_PLAN_COUNT);
    assert_eq!(selected_plan_identities.len(), EXPECTED_PLAN_COUNT);
    assert!(selected_intrinsics.iter().all(|intrinsic| {
        !intrinsic.contains("toward_") && !intrinsic.contains("fused_multiply_add")
    }));

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let first_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("first-build")),
        target_name: Some("windows_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline Windows x64 float twin should produce an exact windows_x64 image");
    let first_path = first_build
        .checked_native_executable_path()
        .expect("windows_x64 cross-build must retain its executable receipt");
    let image_bytes = fs::read(first_path).expect("read retained windows_x64 image bytes");
    assert_eq!(image_bytes.get(..2), Some(b"MZ".as_slice()));
    let pe_offset = u32::from_le_bytes(
        image_bytes
            .get(0x3c..0x40)
            .expect("PE image must retain its DOS e_lfanew field")
            .try_into()
            .expect("PE e_lfanew field has an exact four-byte width"),
    ) as usize;
    assert_eq!(
        image_bytes.get(pe_offset..pe_offset + 4),
        Some(b"PE\0\0".as_slice()),
        "retained image must carry the PE signature at e_lfanew"
    );
    assert_eq!(
        image_bytes.get(pe_offset + 4..pe_offset + 6),
        Some([0x64, 0x86].as_slice()),
        "retained PE image must name the AMD64 machine"
    );

    let second_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("second-build")),
        target_name: Some("windows_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline Windows x64 float twin should reproduce its exact windows_x64 image");
    let second_bytes = fs::read(
        second_build
            .checked_native_executable_path()
            .expect("second windows_x64 cross-build must retain its executable receipt"),
    )
    .expect("read second windows_x64 image bytes");
    assert_eq!(image_bytes, second_bytes);

    let mut build_artifact_identity = 0xcbf29ce484222325_u64;
    retain(&mut build_artifact_identity, SUITE_ID.as_bytes());
    retain(
        &mut build_artifact_identity,
        b"evidence:cross-build-exact-image",
    );
    for category in COVERAGE {
        retain(&mut build_artifact_identity, category.as_bytes());
    }
    for intrinsic in &selected_intrinsics {
        retain(&mut build_artifact_identity, intrinsic.as_bytes());
    }
    for identity in &selected_plan_identities {
        retain(&mut build_artifact_identity, &identity.to_le_bytes());
    }
    retain(
        &mut build_artifact_identity,
        &interpreted.exit_code.to_le_bytes(),
    );
    retain(&mut build_artifact_identity, &interpreted.stdout);
    retain(&mut build_artifact_identity, &interpreted.stderr);
    retain(
        &mut build_artifact_identity,
        b"target-profile:windows_x86_64",
    );
    retain(&mut build_artifact_identity, b"architecture:x86_64");
    retain(&mut build_artifact_identity, b"object-format:coff");
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::windows_x64().pointer_size as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::windows_x64().pointer_alignment as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(&main_path).expect("read retained Windows x64 baseline source bytes"),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(canary.join("build.omg")).expect("read retained windows_x64 build binding"),
    );
    retain(&mut build_artifact_identity, &image_bytes);
    assert_eq!(
        build_artifact_identity, EXPECTED_BUILD_ARTIFACT_IDENTITY,
        "{SUITE_ID} build/artifact identity changed ({build_artifact_identity:#018x}); validate the exact baseline plans, interpreter result, target binding, and reproducible image before refreshing it",
    );

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let output = Command::new(first_path)
            .output()
            .expect("hosted windows_x64 baseline twin should execute its retained image");
        assert_eq!(output.status.code(), Some(70));
        let mut execution_identity = build_artifact_identity;
        retain(&mut execution_identity, b"evidence:hosted-native-execution");
        retain(
            &mut execution_identity,
            &output.status.code().unwrap_or_default().to_le_bytes(),
        );
        retain(&mut execution_identity, &output.stdout);
        retain(&mut execution_identity, &output.stderr);
        assert_eq!(
            execution_identity, EXPECTED_HOSTED_EXECUTION_IDENTITY,
            "{SUITE_ID} hosted execution identity changed ({execution_identity:#018x})",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}
