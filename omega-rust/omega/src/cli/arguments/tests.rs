use super::*;

fn invocation(arguments: &[&str]) -> Invocation {
    parse(arguments.iter().map(OsString::from))
        .unwrap_or_else(|error| panic!("{arguments:?}: {error}"))
}

#[test]
fn compilation_keeps_the_first_argument_and_path_values() {
    let Invocation::Compile(request) = invocation(&[
        "--check",
        "--build-dir",
        "build directory",
        "source file.omg",
    ]) else {
        panic!("expected compile request");
    };
    assert!(request.check_only);
    assert_eq!(request.build_dir, Some(PathBuf::from("build directory")));
    assert_eq!(request.root_path, PathBuf::from("source file.omg"));
    for path in ["install.omg", "update.omg", "run.omg", "audit.omg"] {
        assert!(
            matches!(invocation(&[path]), Invocation::Compile(request) if request.root_path == std::path::Path::new(path))
        );
    }
}

#[test]
fn execution_and_inspection_receive_complete_requests() {
    let Invocation::Run(request) = invocation(&["run", "--both", "--keep", "main.omg"]) else {
        panic!("expected run request");
    };
    assert!(request.both && request.keep);
    assert_eq!(request.main_path, PathBuf::from("main.omg"));
    let Invocation::InspectTerminal(request) =
        invocation(&["inspect-terminal", "--machine", "Root::main", "main.omg"])
    else {
        panic!("expected inspection request");
    };
    assert_eq!(request.machine, "Root::main");
    assert_eq!(request.root_path, PathBuf::from("main.omg"));
}

#[test]
fn package_requests_preserve_the_operation_and_acquisition_policy() {
    let Invocation::Package { command, options } = invocation(&[
        "install",
        "../dependency",
        "--offline",
        "--project",
        "project",
    ]) else {
        panic!("expected install request");
    };
    assert!(matches!(command, PackageCommand::Install { source, .. } if source == "../dependency"));
    assert!(options.offline);
    assert_eq!(options.project_root, PathBuf::from("project"));
    assert!(matches!(
        invocation(&["update", "--discard-review"]),
        Invocation::Package {
            command: PackageCommand::DiscardReview,
            ..
        }
    ));
}

#[test]
fn audit_and_sample_routes_are_selected_without_executing_them() {
    let Invocation::AuditSource(request) =
        invocation(&["audit", "source", "--kind", "local", "../dependency"])
    else {
        panic!("expected source inspection");
    };
    assert_eq!(request.locator, "../dependency");
    let Invocation::AuditPackages(request) =
        invocation(&["audit", "packages", "--details", "--offline"])
    else {
        panic!("expected package inspection");
    };
    assert!(request.details && request.offline);
    assert!(
        matches!(invocation(&["refresh-samples"]), Invocation::RefreshSamples(root) if root == std::path::Path::new("samples"))
    );
    assert!(
        matches!(invocation(&["refresh-samples", "my samples"]), Invocation::RefreshSamples(root) if root == std::path::Path::new("my samples"))
    );
}

#[test]
fn help_is_distinct_from_an_invalid_invocation() {
    for arguments in [
        vec!["install", "--help"],
        vec!["update", "--help"],
        vec!["audit", "packages", "--help"],
    ] {
        assert!(matches!(invocation(&arguments), Invocation::Help(_)));
    }
    for arguments in [
        vec![],
        vec!["install"],
        vec!["update", "--rev", "main"],
        vec!["audit"],
        vec!["audit", "unknown"],
        vec!["run"],
        vec!["run", "--offline", "main.omg"],
        vec!["inspect-terminal", "main.omg"],
    ] {
        assert!(
            parse(arguments.iter().map(OsString::from)).is_err(),
            "{arguments:?}"
        );
    }
}
