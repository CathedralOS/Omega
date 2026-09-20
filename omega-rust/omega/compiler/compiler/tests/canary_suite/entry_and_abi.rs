//! Fixtures shared by the program entry and ABI canaries.

#[path = "entry_and_abi/aarch64_entry_abi.rs"]
mod aarch64_entry_abi;
#[path = "entry_and_abi/efi_entry_abi.rs"]
mod efi_entry_abi;
#[path = "../fixture_rosters/entry_and_abi.rs"]
pub(super) mod fixture_roster;
#[path = "entry_and_abi/hosted_receiver.rs"]
mod hosted_receiver;
#[path = "entry_and_abi/hosted_receiver_linux.rs"]
mod hosted_receiver_linux;
#[path = "entry_and_abi/hosted_receiver_linux_arm64.rs"]
mod hosted_receiver_linux_arm64;
#[path = "entry_and_abi/hosted_receiver_windows.rs"]
mod hosted_receiver_windows;
#[path = "entry_and_abi/pass_canary_coverage.rs"]
mod pass_canary_coverage;
#[path = "entry_and_abi/program_entries_and_image_validation.rs"]
mod program_entries_and_image_validation;
#[path = "entry_and_abi/runtime_canaries_and_efi_handoff.rs"]
mod runtime_canaries_and_efi_handoff;
#[path = "entry_and_abi/sysv_entry_abi.rs"]
mod sysv_entry_abi;
#[path = "entry_and_abi/uefi_loaded_image_layout.rs"]
mod uefi_loaded_image_layout;

use crate::{Command, CompileReport, Path, fs};

fn write_cross_target_application_build(source_dir: &Path) {
    fs::write(
        source_dir.join("build.omg"),
        "machine build(builder: &mut Build) { builder.application(\"cross-target-canary\"); }\n",
    )
    .expect("write target-independent application build source");
}

fn assert_native_exit_code(
    report: &CompileReport,
    expected: i32,
    fixture: &str,
    expectation: &str,
) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{expectation}; expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
