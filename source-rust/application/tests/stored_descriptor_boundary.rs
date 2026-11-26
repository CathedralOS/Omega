mod support;

use std::path::Path;
use std::process::{Command, Stdio};
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
    StructuralAccess,
};

#[test]
fn a_checked_boundary_returns_its_stored_descriptor() {
    let project =
        support::repository_root().join("source-rust/application/tests/fixtures/ownership/run/stored_descriptor_boundary");
    let run = |options: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .arg("run")
            .args(options)
            .arg("build.omg")
            .current_dir(&project)
            .stdin(Stdio::null())
            .output()
            .expect("run the original boundary call");
        let report = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{report}");
        assert!(output.stdout.is_empty(), "{output:?}");
        report.into_owned()
    };
    let psi = run(&["--engine", "psi"]);
    assert!(psi.lines().any(|line| line == "Psi exit 0"), "{psi}");
    let report = run(&["--engine", "native", "--keep"]);
    assert!(
        report.lines().any(|line| line == "native exit 0"),
        "{report}"
    );
    let kept = report
        .lines()
        .find_map(|line| line.strip_prefix("build dir kept: "))
        .expect("keep the saved Psi artifact");
    let bytes = std::fs::read(Path::new(kept).join("program.oterm")).unwrap();
    let reopened = CanonicalTerminalArtifact::from_bytes(&bytes).expect("reopen the saved product");
    let module = reopened.view().module();
    let requirement = module
        .interfaces.boundary_machines
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
