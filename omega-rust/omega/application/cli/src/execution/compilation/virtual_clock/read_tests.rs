use super::{bindings, selected_bindings};
use crate::temporary_directory::TemporaryDirectory;
use checked_interpreter::{BuildMachineEntry, InterpretOptions};
use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};

fn clock_program(read: &str, wait: &str, clock_id: u32) -> String {
    format!(
        r#"
use omega::language::core::external_binding;
pub boundary trait Clock {{
    machine read(clock_id: u32) -> u64;
    machine pause(pointer: u64, count: u64, milliseconds: i32) -> i32;
}}
pub boundary trait Console {{ machine exit_process(code: i32); }}
machine exit_binding() -> ForeignBinding<26, 5, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::MachODylibSymbol {{
            install_name: "/usr/lib/libSystem.B.dylib", symbol: "_exit",
        }},
    }}
}}
macos_arm64 machine selected_exit(code: i32)
    satisfies Console::exit_process via exit_binding();
machine read_binding() -> ForeignBinding<26, 22, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::MachODylibSymbol {{
            install_name: "/usr/lib/libSystem.B.dylib",
            symbol: "_clock_gettime_nsec_np",
        }},
    }}
}}
machine wait_binding() -> ForeignBinding<26, 5, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::MachODylibSymbol {{
            install_name: "/usr/lib/libSystem.B.dylib", symbol: "_poll",
        }},
    }}
}}
macos_arm64 machine selected_read(clock_id: u32) -> u64
    satisfies Clock::read via read_binding();
macos_arm64 machine selected_wait(pointer: u64, count: u64, milliseconds: i32) -> i32
    satisfies Clock::pause via wait_binding();
machine main() reaches Clock + Console {{
    let before: u64 = {read}({clock_id});
    let before_again: u64 = {read}({clock_id});
    let wait_result: i32 = {wait}(0, 0, 40);
    let after: u64 = {read}({clock_id});
    let after_again: u64 = {read}({clock_id});
    transition before == 0 && before_again == 0 && wait_result == 0
        && after == 40000000 && after_again == after {{
        true -> finish(70)
        _ -> finish(1)
    }}
    state finish(code: i32) {{ Console::exit_process(code); }}
}}
"#
    )
}

fn checked_clock(source: &str) -> (TemporaryDirectory, compiler::CheckedCompilation) {
    let directory =
        TemporaryDirectory::create("virtual-clock-read", false).expect("test directory");
    let root = directory.path().join("main.omg");
    std::fs::write(&root, source).expect("clock source fixture");
    std::fs::write(
        directory.path().join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    Build::application(builder, "virtual_clock_read");
    builder.roots.bind(macos_arm64::ProgramEntry, main);
}
"#,
    )
    .expect("clock build fixture");
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
        &root,
        Some("macos_arm64"),
    ))
    .unwrap_or_else(|diagnostics| panic!("checked clock read fixture: {diagnostics:?}"));
    (directory, checked)
}

#[test]
fn selected_clock_reads_observe_waits_without_advancing_time() {
    for (read, wait) in [
        ("Clock::read", "Clock::pause"),
        ("selected_read", "selected_wait"),
    ] {
        let (_directory, checked) = checked_clock(&clock_program(read, wait, 6));
        let clocks = bindings(&checked).expect("exact selected read and wait");
        assert_eq!(clocks.len(), 2);
        let outcome = checked_interpreter::interpret_entry(
            &checked,
            BuildMachineEntry::Name("main"),
            &[],
            InterpretOptions {
                virtual_clock_bindings: clocks,
                ..InterpretOptions::default()
            },
        );
        assert!(outcome.error.is_none(), "{read}: {:?}", outcome.error);
        assert_eq!(outcome.exit_code, 70, "{read}");
    }
}

#[test]
fn selected_clock_reads_decline_other_clock_ids() {
    let (_directory, checked) = checked_clock(&clock_program("selected_read", "selected_wait", 0));
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions {
            virtual_clock_bindings: bindings(&checked).expect("selected clock"),
            ..InterpretOptions::default()
        },
    );
    assert!(
        outcome.error.is_some(),
        "a selected monotonic reader cannot grant a different clock"
    );
}

#[test]
fn clock_reads_require_selected_locator_and_calling_evidence() {
    let (_directory, checked) = checked_clock(&clock_program("selected_read", "selected_wait", 6));
    let without_binding = checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert!(
        without_binding.error.is_some(),
        "a bodyless declaration alone cannot grant a virtual clock read"
    );

    let mut external_rows = checked.providers().external_binding_rows.clone();
    let read = external_rows
        .iter_mut()
        .find(|row| {
            matches!(&row.binding,
                calling_conventions::ExternalBindingKind::Import { locator }
                if matches!(locator.locator(), ForeignLocatorCandidate::MachODylibSymbol { symbol, .. }
                    if symbol == b"_clock_gettime_nsec_np"))
        })
        .expect("selected physical read");
    read.binding = calling_conventions::ExternalBindingKind::Import {
        locator: normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/local/lib/libClock.dylib".to_vec(),
                symbol: b"_clock_gettime_nsec_np".to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .expect("well-formed substituted locator"),
    };
    let execution = checked.execution();
    assert!(
        selected_bindings(
            &checked,
            execution.selected_target_profile,
            &execution.selected_provider_plan_facts,
            &execution.selected_provider_provenance,
            &checked.providers().evaluated_via_bindings,
            &external_rows,
        )
        .is_err(),
        "an unrelated library cannot reuse the selected clock receipt"
    );
}
