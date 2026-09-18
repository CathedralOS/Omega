//! Real process entry must provision the receiver; the test supplies no pointer,
//! storage grant, interpreter arguments, or replacement native entry stub.

use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, CompileReport, Path, PathBuf, compile,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, fs, pass_canary,
    repo_root, unique_no_output_build_dir,
};
use compiler::CheckedCompileRequest;
struct HostedProject(PathBuf);

impl Drop for HostedProject {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "retained failed hosted receiver observations: {}",
                self.0.display()
            );
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

enum ReceiverObservation {
    ScalarMutation,
    BorrowedRecordCopy,
    BorrowedResultSnapshot,
    LocalBorrowedResultSnapshot,
    SignedWrappingRemainder,
    IndexedPrimitiveArray,
    SumAndRecordArray,
}

fn compile_and_run_hosted_receiver(
    explicit_exit: bool,
    bound_service: bool,
    observation: ReceiverObservation,
) {
    let directory = unique_no_output_build_dir();
    fs::create_dir(&directory).expect("create exclusively owned hosted-entry project");
    let project = HostedProject(directory);
    let standard_library = repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        project.0.join("build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.application("hosted-receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    builder.select_provider<Console, ConsoleNativeProvider>();
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}}
"#
        ),
    )
    .expect("write authored target, entry, and provider selection");
    let completion = if explicit_exit {
        "self.console.exit_process(37);"
    } else {
        ""
    };
    let console_type = if bound_service {
        "Service<Console> in Bound"
    } else {
        "Console"
    };
    let observed_value = match observation {
        ReceiverObservation::IndexedPrimitiveArray => "self.bytes[255] as i32",
        _ => "self.value",
    };
    let extra_machines = match observation {
        ReceiverObservation::IndexedPrimitiveArray => {
            "machine set_array_byte(bytes: &mut [u8; 256]) { bytes[255] = 65; }"
        }
        ReceiverObservation::SumAndRecordArray => {
            "data Pair { first: i32; second: i32; }
             data Event { case Loud(gain: i32); case Quiet; }
             data Mixed { flag: i32; case Flat; case Raised(level: i32); }"
        }
        _ => "",
    };
    let (extra_fields, initialization, entry_condition, observation_condition, receiver_bytes) =
        match observation {
        ReceiverObservation::ScalarMutation => ("", "self.value = 65;", "self.value == 0", "self.value == 65", 260),
        ReceiverObservation::IndexedPrimitiveArray => (
            "",
            "self.bytes[255] = 64; let saved: u8 = self.bytes[255]; set_array_byte(&mut self.bytes);",
            "self.value == 0",
            "saved == 64 && self.bytes[255] == 65 && self.bytes[254] == 0",
            260,
        ),
        ReceiverObservation::BorrowedRecordCopy => (
            "source: Counter; destination: Counter;",
            "self.source.value = 65; copy_counter(&self.source, &mut self.destination); self.value = self.destination.value;",
            "self.value == 0",
            "self.value == 65",
            268,
        ),
        ReceiverObservation::BorrowedResultSnapshot => (
            "source: Counter;",
            "self.source.value = 65; let saved: i32 = self.source.read(); self.source.write(66); let current: i32 = self.source.read(); self.value = saved;",
            "self.value == 0",
            "saved == 65 && current == 66",
            264,
        ),
        ReceiverObservation::LocalBorrowedResultSnapshot => (
            "",
            "let mut counter: Counter = Counter::new(65); let saved: i32 = counter.read(); counter.write(66); let current: i32 = counter.read(); self.value = saved;",
            "self.value == 0",
            "saved == 65 && current == 66",
            260,
        ),
        ReceiverObservation::SumAndRecordArray => (
            "pair: Pair; grid: [Pair; 2]; tags: [Event; 2]; event: Event; mixed: Mixed;",
            "self.pair.second = 63; self.grid[1].second = 63; self.value = 65;",
            "self.value == 0 && self.pair.first == 0 && self.pair.second == 0
             && self.event in Event::Loud
             && self.tags[0] in Event::Loud && self.tags[1] in Event::Loud",
            "self.value == 65 && self.pair.second == 63",
            320,
        ),
        ReceiverObservation::SignedWrappingRemainder => (
            "dividend: i64 in Wrapping; divisor: i64 in Wrapping;
             first: i64 in Wrapping; second: i64 in Wrapping;
             third: i64 in Wrapping; self_remainder: i64 in Wrapping;
             small: i8 in Wrapping; small_divisor: i8 in Wrapping; small_remainder: i8 in Wrapping;
             medium: i16 in Wrapping; medium_divisor: i16 in Wrapping; medium_remainder: i16 in Wrapping;
             word: i32 in Wrapping; word_divisor: i32 in Wrapping; word_remainder: i32 in Wrapping;",
            "self.dividend = -7; self.divisor = 3;
             self.first = self.dividend % self.divisor;
             self.self_remainder = self.dividend % self.dividend;
             self.dividend = 7; self.divisor = -3;
             self.second = self.dividend % self.divisor;
             self.dividend = -9223372036854775808; self.divisor = -1;
             self.third = self.dividend % self.divisor;
             self.small = -7; self.small_divisor = 3;
             self.small_remainder = self.small % self.small_divisor;
             self.medium = -32768; self.medium_divisor = -1;
             self.medium_remainder = self.medium % self.medium_divisor;
             self.word = -7; self.word_divisor = 3;
             self.word_remainder = self.word % self.word_divisor;
             self.value = 65;",
            "self.value == 0",
            "self.first == -1 && self.second == 1 && self.third == 0 && self.self_remainder == 0
             && self.small_remainder == -1 && self.medium_remainder == 0 && self.word_remainder == -1",
            336,
        ),
    };
    fs::write(
        project.0.join("main.omg"),
        format!(
            r#"use omega_language_std::console;
use omega::language::core::service;

data Counter {{ value: i32; }}
machine copy_counter(source: &Counter, destination: &mut Counter) {{
    destination.value = source.value;
}}
machine Counter::read(&self) -> i32 {{ self.value }}
machine Counter::write(&mut self, value: i32) {{ self.value = value; }}
machine Counter::new(value: i32) -> Counter {{ Counter {{ value: value }} }}
{extra_machines}

data Main {{
    value: i32;
    bytes: [u8; 256];
    {extra_fields}
    console: {console_type};
}}

machine Main::main(&mut self) reaches Console {{
    transition {entry_condition} {{
        true -> initialized()
        false -> failed()
    }}
    state initialized(&mut self) {{
        {initialization}
        transition {observation_condition} {{
            true -> observed()
            false -> failed()
        }}
    }}
    state observed(&mut self) {{
        self.console.write_byte({observed_value});
        {completion}
    }}
    state failed(&mut self) {{
        self.console.write_byte(70);
    }}
}}
"#
        ),
    )
    .expect("write receiver storage and Fused Console customer");
    let result = compile(CanaryCompileSpec {
        root_path: project.0.join("main.omg"),
        build_dir: Some(project.0.join("build")),
        target_name: Some("macos_arm64".into()),
        product: CanaryCompileProduct::NativeArtifact,
    });
    if !bound_service {
        let diagnostics = result.expect_err("a bare interface field supplies no Bound occurrence");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "macOS hosted receiver bridge lost exact contract, storage, or entry custody"
                )),
            "unexpected missing-establishment rejection: {diagnostics:#?}"
        );
        return;
    }
    let report = result.unwrap_or_else(|diagnostics| {
        panic!("authored hosted receiver must produce its executable: {diagnostics:#?}")
    });
    let receiver = report
        .retained_native_artifact()
        .unwrap()
        .object()
        .hosted_receiver_binding()
        .expect("retain exact provisioned receiver");
    assert_eq!(
        receiver.receiver_byte_count(),
        receiver_bytes,
        "scalar and fixed array both occupy the image-backed receiver"
    );
    if !explicit_exit {
        assert_hosted_binding_replay_rejects_corruption(&report);
    }
    let report = report
        .publish_retained_native_artifact(&project.0.join("build"))
        .expect("publish exact admitted native artifact after corruption controls");
    let executable = report
        .checked_native_executable_path()
        .expect("exact executable publication receipt");
    let bytes = fs::read(executable).expect("read published signed Mach-O");
    assert_eq!(bytes.get(..4), Some([0xcf, 0xfa, 0xed, 0xfe].as_slice()));

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // Execute the published bytes without a host wrapper or re-signing.
        let output = Command::new(executable)
            .output()
            .expect("execute authored hosted receiver process");
        assert_eq!(
            output.status.code(),
            Some(if explicit_exit { 37 } else { 0 }),
            "unexpected process completion: {output:?}"
        );
        assert_eq!(
            output.stdout, b"A",
            "receiver must begin at zero and retain its write"
        );
        assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    eprintln!(
        "SKIP: hosted receiver runtime requires macOS AArch64; source cross-emission checked"
    );
}

fn assert_hosted_binding_replay_rejects_corruption(report: &CompileReport) {
    let native = report
        .retained_native_artifact()
        .expect("retain admitted native object");
    let object = native.object();
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
}

#[test]
fn hosted_receiver_normal_return_provisions_zii_storage_and_fused_console() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::ScalarMutation);
}

#[test]
fn hosted_receiver_explicit_process_exit_preserves_its_distinct_outcome() {
    compile_and_run_hosted_receiver(true, true, ReceiverObservation::ScalarMutation);
}

#[test]
fn hosted_receiver_rejects_bare_interface_without_bound_establishment() {
    compile_and_run_hosted_receiver(false, false, ReceiverObservation::ScalarMutation);
}

#[test]
fn hosted_receiver_observes_copy_between_disjoint_borrowed_records() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::BorrowedRecordCopy);
}

#[test]
fn hosted_receiver_keeps_scalar_call_results_across_later_mutation() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::BorrowedResultSnapshot);
}

#[test]
fn hosted_receiver_keeps_local_scalar_call_results_across_later_mutation() {
    compile_and_run_hosted_receiver(
        false,
        true,
        ReceiverObservation::LocalBorrowedResultSnapshot,
    );
}

#[test]
fn hosted_receiver_signed_wrapping_remainder_preserves_sign_and_overflow_policy() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::SignedWrappingRemainder);
}

#[test]
fn hosted_receiver_indexed_primitive_storage_survives_state_transition() {
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::IndexedPrimitiveArray);
}

#[test]
fn hosted_receiver_provisions_record_arrays_and_the_zero_tag_sum_case() {
    // A receiver carrying nested records, structural arrays, a closed sum,
    // and a mixed declaration provisions all of them under the same ZII
    // contract: record fields, element payloads, common fields, and the
    // first declared case's payload begin established at zero. The entry
    // transition observes `self.event in Event::Loud` — `Loud` is declared
    // first, so the zero tag selects its zeroed payload case — before any
    // write, and nested/indexed scalar stores survive the bridge round trip.
    compile_and_run_hosted_receiver(false, true, ReceiverObservation::SumAndRecordArray);
}

#[test]
fn hosted_receiver_provisions_ieee_float_leaves_for_constant_stores() {
    // The authored fixture declares `f64` and `f32` fields beside the Bound
    // Console carrier and runs the `runtime_float_constant_store_exit` store
    // sequence. The bridge zero-fills the receiver, and all-zero bits are the
    // exact positive `0.0` of both IEEE formats, so the float leaves are
    // zero-valid storage: before this admission the same program was refused
    // by the bridge's storage-shape guard. Unit plans admit no float guards,
    // so the witness is the provisioned receiver executing every store to
    // exit 70.
    let canary = pass_canary(fixture_roster::EXPRESSIONS_RUNTIME_FLOAT_RECEIVER_STORAGE_EXIT);
    let build_dir = unique_no_output_build_dir();
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .unwrap_or_else(|diagnostics| {
            panic!("IEEE float receiver leaves must publish natively: {diagnostics:#?}")
        });
    let executable = compilation
        .checked_native_executable_path()
        .expect("float receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("execute the float receiver canary");
    assert_eq!(
        output.status.code(),
        Some(70),
        "the provisioned f64/f32 receiver leaves must execute every constant store (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn hosted_erased_receiver_preserves_source_cleanup_eligibility() {
    for (nominal_cleanup, bound_service) in [(false, false), (false, true), (true, false)] {
        let directory = unique_no_output_build_dir();
        fs::create_dir(&directory).expect("create owned erased-receiver project");
        let project = HostedProject(directory);
        let standard_library = repo_root()
            .join("source/library/std")
            .to_string_lossy()
            .replace('\\', "/");
        let provider = if bound_service {
            "builder.select_provider<Console, ConsoleNativeProvider>();"
        } else {
            ""
        };
        fs::write(
            project.0.join("build.omg"),
            format!(
                r#"
machine build(builder: &mut Build) {{
    builder.application("erased-receiver");
    builder.depend(Source::Path {{ location: "{standard_library}" }});
    {provider}
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}}
"#
            ),
        )
        .unwrap();
        let cleanup = if nominal_cleanup {
            "machine Main::drop(&mut self) { Helper::finish(); }"
        } else {
            ""
        };
        let service_field = if bound_service {
            "console: Service<Console> in Bound;"
        } else {
            ""
        };
        fs::write(
            project.0.join("main.omg"),
            format!(
                r#"
use omega_language_std::console;
use omega::language::core::service;
data Helper {{}}
machine Helper::finish() {{}}
data Main {{ value: i32; {service_field} }}
{cleanup}
machine Main::main(&mut self) {{}}
"#
            ),
        )
        .unwrap();
        if bound_service {
            assert_erased_service_settlement_requires_its_source_row(&project.0.join("main.omg"));
        }
        let result = compile(CanaryCompileSpec {
            root_path: project.0.join("main.omg"),
            build_dir: Some(project.0.join("build")),
            target_name: Some("macos_arm64".into()),
            product: CanaryCompileProduct::NativeArtifact,
        });
        if nominal_cleanup {
            let Err(diagnostics) = result else {
                panic!("an unused receiver cannot silently lose its nominal cleanup");
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("no executable nominal cleanup")),
                "{diagnostics:?}"
            );
            continue;
        }
        let report = result.expect("trivial unused receiver still produces a native executable");
        assert!(
            report
                .retained_native_artifact()
                .unwrap()
                .object()
                .hosted_receiver_binding()
                .is_none(),
            "checked erasure requires no physical receiver argument"
        );
        let report = report
            .publish_retained_native_artifact(&project.0.join("build"))
            .unwrap();
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let output = Command::new(report.checked_native_executable_path().unwrap())
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert!(
                output.stdout.is_empty() && output.stderr.is_empty(),
                "{output:?}"
            );
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = report;
            eprintln!(
                "SKIP: erased receiver runtime requires macOS AArch64; native product checked"
            );
        }
    }
}

fn assert_erased_service_settlement_requires_its_source_row(root: &Path) {
    let checked =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(root, Some("macos_arm64")))
            .expect("check the actual core Bound service and selected provider");
    let selected = checked.selected_program_entry().unwrap();
    let source = selected.source_signature();
    let produced = terminal_production::TerminalProductionRequest::for_machine_symbol(
        &checked,
        source.machine_symbol(),
    )
    .produce_program_entry(source.identity().bytes())
    .unwrap();
    let eligible = produced.receipt().receiver_eligibility().unwrap();
    assert!(matches!(
        eligible.projection(),
        terminal_psi::CheckedProgramEntryReceiverProjection::Erased { .. }
    ));
    assert_eq!(eligible.fused_service_fields().len(), 1);
    assert_eq!(
        eligible.fused_service_fields()[0].field_identity(),
        "console"
    );
    let calling_plans = selected.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    let rows = selected.fused_service_establishments();
    assert_eq!(rows.len(), 1);
    native_realization::validate_native_program_entry_settlement(
        produced.artifact(),
        produced.receipt(),
        native_realization::NativeProgramEntrySettlement::new(source, calling_plans, rows),
        target::NativeTarget::macos_arm64(),
    )
    .expect("exact source service roster independently settles");
    assert_eq!(
        native_realization::validate_native_program_entry_settlement(
            produced.artifact(),
            produced.receipt(),
            native_realization::NativeProgramEntrySettlement::new(source, calling_plans, &[]),
            target::NativeTarget::macos_arm64(),
        ),
        Err(native_realization::NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift),
        "removing the unused Bound service row must not bypass establishment",
    );
}
