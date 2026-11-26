//! A saved Psi file must be readable without the compiler run that wrote it.
//! `apply_room` copies exits from one room to another through two slices. Each
//! `out_exits[i] = room_exits[i]` stores through a borrowed slice element, and
//! the next copy still needs to know that the source room kept its ASCII
//! commands. The reader has to work that out from the saved file alone.

#[path = "support/project.rs"]
mod project;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::{Path, PathBuf};
use std::process::Output;
use terminal_codec::{CanonicalTerminalArtifact, ClosedTerminalModule, CodecError};
use terminal_psi::{OperationKind, OperationResult, TerminalMachine, TerminalModule};

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
    let emitted = project::omega(
        &source,
        &["--emit-psi", artifact.to_str().unwrap(), "main.omg"],
    );
    assert!(emitted.status.success(), "{}", report(&emitted));
    std::fs::remove_dir_all(&source).unwrap();
    artifact
}

/// The saved `apply_room`, where the tests below remove one fact at a time.
fn apply_room(module: &mut TerminalModule) -> &mut TerminalMachine {
    let identities = &module.identities;
    module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .declaration
                .as_ref()
                .and_then(|declaration| identities.text(declaration.callable))
                .is_some_and(|identity| identity.contains("apply_room"))
        })
        .expect("the saved program keeps apply_room")
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
    let reopened = project::omega(
        artifact.parent().unwrap(),
        &["psi", "inspect", "--artifact", "rooms.psi"],
    );
    assert!(reopened.status.success(), "{}", report(&reopened));
}

#[test]
fn the_copied_room_runs_natively_and_in_psi() {
    let project = support::repository_root()
        .join("source-rust/application/tests/fixtures/ownership/run/slice_element_store");
    let scratch = TemporaryDirectory::create("slice-element-store-run", false).unwrap();
    std::fs::copy(project.join("main.omg"), scratch.path().join("main.omg")).unwrap();
    for (engine, receipt) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let ran = project::omega(scratch.path(), &["run", "--engine", engine, "main.omg"]);
        let ran = report(&ran);
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
        let room = &mut apply_room(module).contract.structural_parameters[0];
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

/// The reader cannot tell which exit `out_exits[0] = ...` wrote. It may keep
/// treating every exit of the destination as holding ASCII commands only
/// because each stored exit has to bring that fact with it. A saved file
/// whose first copied exit no longer claims ASCII commands must be refused
/// at the store through the slice element.
#[test]
fn a_saved_copy_is_refused_when_the_stored_exit_loses_its_ascii_fact() {
    let scratch = TemporaryDirectory::create("saved-slice-element-store", false).unwrap();
    let artifact = saved_artifact(scratch.path());
    let refused = reopen_with(&artifact, |module| {
        let copied = apply_room(module)
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::StructuralLeafCopy { .. }))
            .expect("apply_room copies an exit out of the source slice");
        let OperationResult::Structural(exit) = &mut copied.result else {
            panic!("{:?}", copied.result);
        };
        assert_eq!(exit.projected_qualifications.len(), 1);
        exit.projected_qualifications.clear();
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
        "structural field store does not restore the selected memberships"
    );
}

/// The twin fixture borrows the source slice from the destination room, so
/// `out_exits[0] = room_exits[0]` would write storage that `room_exits` still
/// reads. The compiler must refuse it and save nothing: the reader of a saved
/// file checks the facts in it and does not check loans.
#[test]
fn the_compiler_refuses_a_source_slice_that_borrows_the_destination_room() {
    let scratch = TemporaryDirectory::create("slice-element-store-overlap", false).unwrap();
    std::fs::copy(
        support::repository_root()
            .join("source-rust/application/tests/fixtures/borrows/fail/slice_element_store")
            .join("main.omg"),
        scratch.path().join("main.omg"),
    )
    .unwrap();
    let artifact = scratch.path().join("rooms.psi");
    let refused = project::omega(
        scratch.path(),
        &["--emit-psi", artifact.to_str().unwrap(), "main.omg"],
    );
    let refusal = report(&refused);
    assert!(!refused.status.success(), "{refusal}");
    for expected in [
        "the successor would borrow one place as both mutable and read-only",
        "in machine `apply_room` (conflicting loans)",
        "main.omg:9:14",
    ] {
        assert!(refusal.contains(expected), "{refusal}");
    }
    assert!(!artifact.exists(), "{refusal}");
}

/// The compiler's refusal above is the only one. Pointing the saved source
/// slice at the destination room writes the same overlap into the saved
/// program. No fact the reader checks changes, so it reopens the file. This
/// test records that boundary; it does not ask for the overlap to be legal.
#[test]
fn the_reader_reopens_a_saved_copy_whose_source_slice_borrows_the_destination() {
    let scratch = TemporaryDirectory::create("saved-slice-element-store", false).unwrap();
    let artifact = saved_artifact(scratch.path());
    let reopened = reopen_with(&artifact, |module| {
        let machine = apply_room(module);
        let out_room = machine.contract.structural_parameters[1].place;
        let room_exits = machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::EstablishElementView { source, .. } => Some(source),
                _ => None,
            })
            .expect("apply_room forms the shared source slice");
        assert_ne!(room_exits.place, out_room);
        room_exits.place = out_room;
    });
    assert!(reopened.is_ok(), "{:?}", reopened.map(|_| ()));
}
