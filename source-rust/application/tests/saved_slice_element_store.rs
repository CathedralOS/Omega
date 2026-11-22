//! A saved Psi file must be readable without the compiler run that wrote it.
//! `apply_room` copies exits from one room to another through two slices. Each
//! `out_exits[i] = room_exits[i]` stores through a borrowed slice element, and
//! the next copy still needs to know that the source room kept its ASCII
//! commands. The reader has to work that out from the saved file alone.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use terminal_codec::{CanonicalTerminalArtifact, ClosedTerminalModule, CodecError};
use terminal_psi::TerminalModule;

fn omega(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.current_dir(directory).stdin(Stdio::null());
    command
}

fn report(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

/// Compile the fixture in one process and keep only the Psi file it saved.
/// The source directory is deleted before anything reads the file back.
fn saved_artifact(scratch: &Path) -> PathBuf {
    let source = scratch.join("source");
    std::fs::create_dir(&source).unwrap();
    std::fs::copy(
        support::repository_root()
            .join("source-rust/application/tests/fixtures/ownership/run/slice_element_store")
            .join("main.omg"),
        source.join("main.omg"),
    )
    .unwrap();
    let saved = scratch.join("saved");
    std::fs::create_dir(&saved).unwrap();
    let artifact = saved.join("rooms.psi");
    let emitted = omega(&source)
        .arg("--emit-psi")
        .arg(&artifact)
        .arg("main.omg")
        .output()
        .expect("compile the room copy");
    assert!(emitted.status.success(), "{}", report(&emitted));
    std::fs::remove_dir_all(&source).unwrap();
    artifact
}

/// Change one fact in the saved program and read the result back.
fn reopen_with(
    artifact: &Path,
    change: impl FnOnce(&mut TerminalModule),
) -> Result<ClosedTerminalModule, CodecError> {
    let bytes = std::fs::read(artifact).unwrap();
    let saved = CanonicalTerminalArtifact::from_bytes(&bytes).expect("the saved file reopens");
    let mut module = saved.semantic_module().clone();
    change(&mut module);
    let encoded = ClosedTerminalModule::new(module).expect("the changed program still encodes");
    ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes())
}

#[test]
fn a_saved_copy_between_two_slices_reopens_without_its_source() {
    let scratch = TemporaryDirectory::create("saved-slice-element-store", false).unwrap();
    let artifact = saved_artifact(scratch.path());
    let reopened = omega(artifact.parent().unwrap())
        .args(["psi", "inspect", "--artifact", "rooms.psi"])
        .output()
        .expect("read the saved file back");
    assert!(reopened.status.success(), "{}", report(&reopened));
}

#[test]
fn the_copied_room_runs_natively_and_in_psi() {
    let project = support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership/run/slice_element_store");
    let scratch = TemporaryDirectory::create("slice-element-store-run", false).unwrap();
    std::fs::copy(project.join("main.omg"), scratch.path().join("main.omg")).unwrap();
    let ran = omega(scratch.path())
        .args(["run", "--both", "main.omg"])
        .output()
        .expect("run the room copy on both engines");
    let ran = report(&ran);
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(ran.lines().any(|line| line == receipt), "{ran}");
    }
}

/// The copies claim ASCII commands because the source room's exits hold them.
/// A saved file that drops that one parameter fact must be refused where the
/// source slice is formed.
#[test]
fn a_saved_copy_is_refused_when_the_source_room_loses_its_ascii_fact() {
    let scratch = TemporaryDirectory::create("saved-slice-element-store", false).unwrap();
    let artifact = saved_artifact(scratch.path());
    assert!(reopen_with(&artifact, |_| {}).is_ok());
    let refused = reopen_with(&artifact, |module| {
        let apply_room = module
            .machines
            .iter_mut()
            .find(|machine| {
                machine
                    .declaration
                    .as_ref()
                    .and_then(|declaration| module.identities.text(declaration.callable))
                    .is_some_and(|identity| identity.contains("apply_room"))
            })
            .expect("the saved program keeps apply_room");
        let room = &mut apply_room.contract.structural_parameters[0];
        assert_eq!(room.projected_qualifications.len(), 1);
        room.projected_qualifications.clear();
    });
    let Err(CodecError::MalformedOperationFoundation {
        machine, reason, ..
    }) = refused
    else {
        panic!("{:?}", refused.map(|_| ()));
    };
    assert!(machine.contains("apply_room"), "{machine}");
    assert_eq!(
        reason,
        "view element memberships do not hold for every actual source element"
    );
}
