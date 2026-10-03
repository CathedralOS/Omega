use super::{PhysicalClockOperation, bindings, selected_bindings};
use crate::temporary_directory::TemporaryDirectory;
use checked_interpreter::{BuildMachineEntry, InterpretOptions};
use effects::provider_plan::ProviderBinding;
use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};

#[test]
fn physical_waits_require_the_complete_selected_locator() {
    for (library, symbol, expected) in [
        (
            b"/usr/lib/libSystem.B.dylib".as_slice(),
            b"_poll".as_slice(),
            Some(PhysicalClockOperation::DarwinPoll),
        ),
        (b"/usr/local/lib/libpoll.dylib", b"_poll", None),
        (b"/usr/lib/libSystem.B.dylib", b"_unlink", None),
    ] {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: library.to_vec(),
                symbol: symbol.to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .expect("valid Mach-O locator");
        assert_eq!(PhysicalClockOperation::from_locator(&locator), expected);
    }
    let sleep = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"Sleep".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid PE locator");
    assert_eq!(
        PhysicalClockOperation::from_locator(&sleep),
        Some(PhysicalClockOperation::WindowsSleep)
    );
    assert_eq!(
        PhysicalClockOperation::WindowsSleep.operation(),
        checked_interpreter::VirtualClockOperation::Wait {
            maximum_milliseconds: i64::from(u32::MAX) - 1,
            result: checked_interpreter::VirtualClockResult::Unit,
        }
    );
}

#[test]
fn selected_import_receipts_bind_renamed_waits_without_granting_unselected_providers() {
    let directory =
        TemporaryDirectory::create("virtual-clock-selection", false).expect("test directory");
    let root = directory.path().join("main.omg");
    std::fs::write(
        &root,
        r#"
use omega::language::core::external_binding;
pub boundary trait Pacing { machine mark() reaches Pacing; }
data Wait { }
data Selected { }
data Unselected { }
boundary requirement Wait::pause(pointer: u64, count: u64, duration: i32) -> i32 reaches Pacing;
machine wait_binding() -> ForeignBinding<26, 5, 0> {
    ForeignBinding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "/usr/lib/libSystem.B.dylib", symbol: "_poll",
        },
    }
}

machine other_binding() -> ForeignBinding<26, 7, 0> {
    ForeignBinding::DllImport {
        import: DllImport::MachODylibSymbol {
            install_name: "/usr/lib/libSystem.B.dylib", symbol: "_usleep",
        },
    }
}
machine Selected::wait(pointer: u64, count: u64, duration: i32) -> i32
    satisfies Wait::pause via wait_binding();
machine Unselected::wait(pointer: u64, count: u64, duration: i32) -> i32
    satisfies Wait::pause via other_binding();
machine main() reaches Pacing { _ = Wait::pause(123, 0, 40); }
"#,
    )
    .expect("source fixture");
    std::fs::write(
        directory.path().join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    Build::application(builder, "virtual_clock_selection");
    builder.select_provider<Wait::pause, Selected>();
    builder.roots.bind(macos_arm64::ProgramEntry, main);
}
"#,
    )
    .expect("build fixture");
    let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
        &root,
        Some("macos_arm64"),
    ))
    .unwrap_or_else(|diagnostics| panic!("checked clock fixture: {diagnostics:?}"));
    let waits = bindings(&checked).expect("exact selected wait");
    assert_eq!(waits.len(), 1);
    let outcome = checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions {
            virtual_clock_bindings: waits,
            ..InterpretOptions::default()
        },
    );
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.exit_code, 0);
    let without_binding = checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert!(
        without_binding.error.is_some(),
        "the declaration alone grants no virtual wait"
    );

    let execution = checked.execution();
    let evaluated = &checked.providers().evaluated_via_bindings;
    let mut external_rows = checked.providers().external_binding_rows.clone();
    let physical_wait = external_rows
        .iter_mut()
        .find(|external| {
            matches!(&external.binding, calling_conventions::ExternalBindingKind::Import { locator }
            if PhysicalClockOperation::from_locator(locator).is_some())
        })
        .expect("selected external wait");
    physical_wait.boundary_entry_plan = None;
    assert!(
        selected_bindings(
            &checked,
            execution.selected_target_profile,
            &execution.selected_provider_plan_facts,
            &execution.selected_provider_provenance,
            evaluated,
            &external_rows,
        )
        .is_err(),
        "a foreign locator cannot replace its physical calling-plan evidence"
    );
    assert!(
        selected_bindings(
            &checked,
            Some(TargetProfile::WindowsX64),
            &execution.selected_provider_plan_facts,
            &execution.selected_provider_provenance,
            evaluated,
            &checked.providers().external_binding_rows,
        )
        .is_err(),
        "a matching locator cannot cross target selection"
    );

    let mut provenance = execution.selected_provider_provenance.clone();
    let retained = provenance.iter_mut().find(|retained| retained.plan.rows.iter().any(|row| {
        matches!(&row.binding, ProviderBinding::Import { evaluated } if PhysicalClockOperation::from_locator(evaluated.locator()).is_some())
    })).expect("selected import provenance");
    let alternate = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Unselected::wait")
        .expect("unselected candidate survives for receipt review");
    retained.provider.row_realizations[0] = alternate.symbol;
    assert!(
        selected_bindings(
            &checked,
            execution.selected_target_profile,
            &execution.selected_provider_plan_facts,
            &provenance,
            evaluated,
            &checked.providers().external_binding_rows,
        )
        .is_err(),
        "another checked provider cannot borrow a selected import receipt"
    );
    let mut provenance = execution.selected_provider_provenance.clone();
    provenance.push(provenance[0].clone());
    assert!(
        selected_bindings(
            &checked,
            execution.selected_target_profile,
            &execution.selected_provider_plan_facts,
            &provenance,
            evaluated,
            &checked.providers().external_binding_rows,
        )
        .is_err(),
        "duplicate provenance cannot create extra authority"
    );
}

#[test]
fn windows_sleep_uses_the_selected_import_and_declines_unknown_replacements() {
    for (export, duration, supported, completes) in [
        ("Sleep", 0_u32, true, true),
        ("Sleep", u32::MAX - 1, true, true),
        ("Sleep", u32::MAX, true, false),
        ("BeepX", 40, false, false),
    ] {
        let directory =
            TemporaryDirectory::create("virtual-clock-windows", false).expect("test directory");
        let root = directory.path().join("main.omg");
        std::fs::write(
            &root,
            format!(
                r#"
use omega::language::core::external_binding;
pub boundary trait Pacing {{ machine sleep(duration: u32) reaches Pacing; }}
data Host {{ }}
machine wait_binding() -> ForeignBinding<12, 5, 0> {{
    ForeignBinding::DllImport {{
        import: DllImport::PeByName {{ library: "kernel32.dll", export: "{export}" }},
    }}
}}
machine Host::wait(duration: u32) satisfies Pacing::sleep via wait_binding();
machine main() reaches Pacing {{ Pacing::sleep({duration}); }}
"#
            ),
        )
        .expect("Windows source fixture");
        std::fs::write(
            directory.path().join("build.omg"),
            r#"
machine build(builder: &mut Build) {
    Build::application(builder, "virtual_clock_windows");
    builder.select_provider<Pacing, Host>();
    builder.roots.bind(windows_x86_64::ProgramEntry, main);
}
"#,
        )
        .expect("Windows build fixture");
        let checked = compiler::compile_to_checked(compiler::CheckedCompileRequest::new(
            &root,
            Some("windows_x86_64"),
        ))
        .unwrap_or_else(|diagnostics| panic!("checked Windows clock fixture: {diagnostics:?}"));
        let waits = bindings(&checked).expect("selected imports are well formed");
        assert_eq!(waits.len(), usize::from(supported));
        let outcome = checked_interpreter::interpret_entry(
            &checked,
            BuildMachineEntry::Name("main"),
            &[],
            InterpretOptions {
                virtual_clock_bindings: waits,
                ..InterpretOptions::default()
            },
        );
        if completes {
            assert!(outcome.error.is_none(), "{:?}", outcome.error);
            assert_eq!(outcome.exit_code, 0);
        } else {
            assert!(
                outcome.error.is_some(),
                "an unknown import or indefinite duration cannot become a finite virtual wait"
            );
        }
    }
}
