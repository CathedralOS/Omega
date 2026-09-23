use super::{
    Fixture, admit_import, admit_imports, terminal_authority_permission_policy,
    terminal_authority_policy,
};
use compiler::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
#[cfg(windows)]
use std::fs;
use task_plans::SameStackContributionAdmissionReceiptId;

#[test]
fn retained_x86_fma_and_source_evaluated_import_stop_at_fma_transport() {
    // FMA occurrences admit and settle upstream (see a2t's
    // validate_ieee_float_fma_settlements), but emit_realization_object has no
    // transport for them in the common instruction pipeline and rejects by
    // contract. This pin holds the rejection at that declared stage; the
    // nested MXCSR custody composition legs restore once provider transport
    // lands.
    let fixture = Fixture::new_windows_x86_fma();
    let retained = fixture.compile_terminal();
    let admission = admit_import(
        &retained,
        SameStackContributionAdmissionReceiptId::from_normalized_identity(0x5846_4d41_0005)
            .unwrap(),
    );
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let diagnostics = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &[SourceEvaluatedImportSettlement::new(
                    &admission.execution,
                    &admission.same_stack,
                )],
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .expect_err("FMA occurrences have no provider transport on the common instruction pipeline");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "FMA provider transport is not implemented in the common instruction pipeline",
        )
    }));
}

#[test]
fn windows_catalog_dll_case_variants_preserve_imports_and_execute() {
    for (case_name, library) in [
        ("lower", "kernel32.dll"),
        ("catalog", "Kernel32.dll"),
        ("upper-stem", "KERNEL32.dll"),
        ("upper", "KERNEL32.DLL"),
    ] {
        let source = format!(
            r#"use omega::language::core::service;
use omega::language::core::external_binding;

pub boundary trait WindowsCalls {{
    machine find_close(handle: i64) -> i32;
    machine exit(code: i32);
}}
windows_x86_64 machine find_close_binding() -> ForeignBinding<12, 9, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::PeByName {{ library: "{library}", export: "FindClose" }},
    }}
}}
windows_x86_64 machine exit_binding() -> ForeignBinding<12, 11, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::PeByName {{ library: "Kernel32.dll", export: "ExitProcess" }},
    }}
}}
machine find_close_leaf(handle: i64) -> i32 satisfies WindowsCalls::find_close via find_close_binding();
machine exit_leaf(code: i32) satisfies WindowsCalls::exit via exit_binding();
data Main {{ windows: Binding<WindowsCalls>; }}
machine Main::main(&mut self) reaches WindowsCalls {{
    let result: i32 = self.windows.find_close(0);
    self.windows.exit(result);
}}
"#
        );
        let fixture = Fixture::with_source(
            &format!("dll-case-{case_name}"),
            "windows_x86_64",
            &source,
            r#"machine build(builder: &mut Build) {
    builder.application("windows-dll-case");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
}
"#,
        );
        let retained = fixture.compile_terminal();
        let admissions = admit_imports(&retained, 0x4341_5345_0001);
        assert_eq!(
            admissions.len(),
            2,
            "{library}: both import leaves require custody"
        );
        let settlements = admissions
            .iter()
            .map(|admission| {
                SourceEvaluatedImportSettlement::new(&admission.execution, &admission.same_stack)
            })
            .collect::<Vec<_>>();
        let policy = terminal_authority_policy(&retained);
        let permission_policy = terminal_authority_permission_policy(&retained);
        let artifact = {
            let image_request = native_realization::ExecutableImageEmissionRequest::direct(
                retained
                    .native_realization_proposal()
                    .expect("native proposal")
                    .subsystem(),
            );
            realize_retained_native_artifact(
                retained,
                RetainedNativeRealizationRequest {
                    profile: &proof_admission::AdmissionProfile::default(),
                    optimization_selections:
                        &optimization_core::PostTerminalOptimizationSelections::default(),
                    terminal_authority_policy: policy,
                    accepted_package_terminal_authority_permission_policy:
                        native_realization::current_terminal_authority_permission_policy(),
                    terminal_authority_permission_policy: Some(permission_policy),
                    image_request,
                    imports: &settlements,
                },
            )
            .map(|artifact| match artifact {
                native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
                native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                    panic!("direct image request returned dynamic ELF custody")
                }
            })
            .map_err(|(_, diagnostics)| diagnostics)
        }
        .unwrap_or_else(|diagnostics| panic!("{library}: {diagnostics:#?}"));
        artifact
            .validate()
            .expect("DLL case artifact independently replays");
        assert_eq!(artifact.image().output().final_image_imports, 2);
        assert_eq!(artifact.image().output().final_image_relocations, 2);
        #[cfg(windows)]
        {
            let executable = fixture.root.join("dll-case.exe");
            fs::write(&executable, &artifact.image().output().bytes)
                .expect("write validated PE image");
            let mut child = std::process::Command::new(executable)
                .spawn()
                .expect("run DLL case image");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let status = loop {
                if let Some(status) = child.try_wait().expect("poll DLL case image") {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    child.kill().expect("stop stalled DLL case image");
                    child.wait().expect("reap stalled DLL case image");
                    panic!("{library}: native execution timed out");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert_eq!(
                status.code(),
                Some(0),
                "{library}: FindClose(0) result must reach ExitProcess"
            );
        }
        #[cfg(not(windows))]
        eprintln!("SKIP {library} native execution: host is not Windows");
    }
}

#[test]
fn windows_evaluated_u32_result_reaches_a_later_pe_import_through_exact_home_custody() {
    let fixture = Fixture::new_windows_u32_result_chain();
    let retained = fixture.compile_terminal();
    let admissions = admit_imports(&retained, 0x5749_4e52_0001);
    assert_eq!(
        admissions.len(),
        2,
        "both evaluated PE leaves require custody"
    );
    let settlements = admissions
        .iter()
        .map(|admission| {
            SourceEvaluatedImportSettlement::new(&admission.execution, &admission.same_stack)
        })
        .collect::<Vec<_>>();
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let artifact = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &settlements,
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .unwrap_or_else(|diagnostics| {
        panic!("Windows evaluated result chain should realize: {diagnostics:#?}")
    });

    artifact
        .validate()
        .expect("Windows evaluated result artifact independently replays");
    assert_eq!(artifact.target(), target::NativeTarget::windows_x64());
    // The fragment publication route seals object-level foreign-call custody
    // empty; the bound executable image carries the projected rows.
    let [producer, consumer] = artifact.image().foreign_calls() else {
        panic!("the source result chain must retain two PE calls")
    };
    let result = producer
        .scalar_result
        .as_ref()
        .expect("GetCurrentProcessId retains its exact u32 result home");
    let [argument] = consumer.scalar_arguments.as_slice() else {
        panic!("Sleep retains the preceding result as its sole argument")
    };
    assert_eq!(
        argument.source,
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(result.home)
    );
    assert_eq!(
        result.home.scalar_type,
        semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
                .unwrap()
        )
    );
    for (call, export) in [
        (producer, b"GetCurrentProcessId".as_slice()),
        (consumer, b"Sleep".as_slice()),
    ] {
        let target::ForeignLocatorCandidate::PeByName {
            library,
            export: name,
        } = call.locator.locator()
        else {
            panic!("the custody row must retain its PE import locator")
        };
        assert_eq!(library.as_slice(), b"kernel32.dll");
        assert_eq!(name.as_slice(), export);
    }
    assert_eq!(artifact.image().output().format, "pe64-x86_64-executable");
}

#[test]
fn windows_evaluated_result_rejects_cross_wired_same_stack_custody() {
    let fixture = Fixture::new_windows_u32_result_chain();
    let retained = fixture.compile_terminal();
    let admissions = admit_imports(&retained, 0x5749_4e52_1001);
    let [first, second] = admissions.as_slice() else {
        panic!("the Windows result chain must retain exactly two import admissions")
    };
    let cross_wired = [
        SourceEvaluatedImportSettlement::new(&first.execution, &second.same_stack),
        SourceEvaluatedImportSettlement::new(&second.execution, &first.same_stack),
    ];
    let policy = terminal_authority_policy(&retained);
    let permission_policy = terminal_authority_permission_policy(&retained);
    let diagnostics = {
        let image_request = native_realization::ExecutableImageEmissionRequest::direct(
            retained
                .native_realization_proposal()
                .expect("native proposal")
                .subsystem(),
        );
        realize_retained_native_artifact(
            retained,
            RetainedNativeRealizationRequest {
                profile: &proof_admission::AdmissionProfile::default(),
                optimization_selections:
                    &optimization_core::PostTerminalOptimizationSelections::default(),
                terminal_authority_policy: policy,
                accepted_package_terminal_authority_permission_policy:
                    native_realization::current_terminal_authority_permission_policy(),
                terminal_authority_permission_policy: Some(permission_policy),
                image_request,
                imports: &cross_wired,
            },
        )
        .map(|artifact| match artifact {
            native_realization::RequestedNativeArtifact::Direct(artifact) => artifact,
            native_realization::RequestedNativeArtifact::DynamicElf(_) => {
                panic!("direct image request returned dynamic ELF custody")
            }
        })
        .map_err(|(_, diagnostics)| diagnostics)
    }
    .expect_err("same-stack custody from the sibling PE leaf cannot authorize this result chain");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not carry same-stack custody for the exact selected provider row")
    }));
}
