use std::path::Path;
use std::process::{Command, Stdio};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
    StructuralAccess,
};

#[test]
fn a_checked_boundary_returns_its_stored_descriptor() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/run/ownership/stored_descriptor_boundary");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "--keep", "build.omg"])
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .expect("run the original boundary call");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.status.success(), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
    let kept = report
        .lines()
        .find_map(|line| line.strip_prefix("build dir kept: "))
        .expect("keep the saved Psi artifact");
    let bytes = std::fs::read(Path::new(kept).join("program.oterm")).unwrap();
    let reopened = CanonicalTerminalArtifact::from_bytes(&bytes).expect("reopen the saved product");
    let module = reopened.view().module();
    let requirement = module
        .boundary_machines
        .iter()
        .find(|boundary| {
            module
                .identities
                .text(boundary.identity)
                .is_some_and(|identity| identity.contains("Probe::retained"))
        })
        .expect("the saved product retains the exact boundary requirement");
    assert_eq!(
        requirement.reference_outputs,
        [BoundaryReferenceOutput {
            destination: BoundaryReferenceDestination::Parameter(0),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
            dependencies: vec![BoundaryLifetimeDependency::Stored {
                argument_index: 0,
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
        }],
    );
    std::fs::remove_dir_all(kept).expect("remove the build directory kept by this test");
}
