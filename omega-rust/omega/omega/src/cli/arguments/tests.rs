use super::{Invocation, OsString, parse};
fn invocation(arguments: &[&str]) -> Invocation {
    parse(arguments.iter().map(OsString::from))
        .unwrap_or_else(|error| panic!("{arguments:?}: {error}"))
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

/// The usage text is what every other entry point prints on a parse failure,
/// so the conventional spellings reach it rather than being read as a root
/// path or an unrecognized option.
#[test]
fn the_help_spellings_reach_the_usage_text() {
    for spelling in ["--help", "-h", "help"] {
        let Invocation::Help(usage) = invocation(&[spelling]) else {
            panic!("`{spelling}` should ask for usage");
        };
        assert!(
            usage.starts_with("usage: omega "),
            "`{spelling}` printed {usage}"
        );
    }
    // A file that merely begins with those letters is still a root path.
    assert!(
        matches!(invocation(&["help.omg"]), Invocation::Compile(request)
            if request.root_path == std::path::Path::new("help.omg"))
    );
}
