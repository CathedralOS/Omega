//! L3 -> L4 handoff for a guarded machine declared by another package.
//!
//! L3's checked-library mode is crate-private, and a source package links only
//! the toolchain package. The public route for a client and its library is one
//! resolved stage with a requester-local dependency, typed by `type_stage` and
//! prepared by the public preparation pass. L3 then publishes the client's call
//! with the route and guard from the library's own tree. L4 must run that guard
//! with the client's operands before admitting the invocation.
use omega_preparation::{CheckedPreparationEvidence, PreparationAnswers, PreparedValue, prepare_typing_pass};
use omega_resolver::{Dependency, ParsedSource, StageSource, resolve_stage};
use omega_typer::{CrashCause, TypeError, TypedTrees, TypingProgress, type_stage};

const ADMISSION: &str = "static evaluation cannot admit this invocation: the selected Trap route remains possible for the supplied arguments";

fn producer(guard: &str) -> String {
    format!("public machine guarded(x:u8)->u8 reaches none crashes Trap {guard} terminates {{x}}")
}

/// Resolve and type both packages together. Only the client may name `geo`.
fn typed<'text>(
    library: &'text str,
    client: &'text str,
) -> TypedTrees<'text> {
    let texts = [("geometry", "geometry.omg", library), ("client", "client.omg", client)];
    let syntax: Vec<_> = texts
        .iter()
        .map(|&(_, _, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = texts
        .iter()
        .zip(&syntax)
        .map(|(&(package, logical_path, text), (tokens, tree))| StageSource {
            package,
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let dependencies = [Dependency {
        requester: "client",
        alias: "geo",
        package: "geometry",
    }];
    let resolved = resolve_stage(&sources, &dependencies).unwrap_or_else(|error| panic!("{error:?}"));
    type_stage(&resolved).unwrap_or_else(|error| panic!("{error:?}"))
}

/// Run the public preparation pass over the completed typed stage.
fn prepare(typed: TypedTrees<'_>) -> Result<Vec<(String, PreparedValue)>, TypeError> {
    let progress = TypingProgress::Complete(typed);
    let pass = prepare_typing_pass(&progress, &[], None, CheckedPreparationEvidence::default())?;
    let constants = match pass.answers {
        PreparationAnswers::Frozen { constants, .. } => constants,
        other => panic!("a completed stage freezes its constants, found {other:?}"),
    };
    let mut constants: Vec<_> = constants
        .into_iter()
        .map(|(declaration, value)| (declaration.path.to_string(), value))
        .collect();
    constants.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(constants)
}

#[test]
fn a_client_constant_runs_the_library_guard_with_its_own_operand() {
    let library = producer("x == 0");
    let client = "const SAFE:u8=geo\\guarded(3u8);";
    let stage = typed(&library, client);

    // A constant initializer selects a machine. Its crash contract names the guard
    // in the library's tree, and the client supplies the operand for that guard.
    let call = stage
        .calls
        .iter()
        .find(|call| call.site.package == "client" && call.machine.path == "guarded")
        .expect("the client initializer selects the library machine");
    assert_eq!(call.machine.package, "geometry");
    let signature = stage.signatures.iter().find(|signature| signature.machine == call.machine).unwrap();
    let route = stage.crash_routes.iter().find(|route| route.contract == signature.site).unwrap();
    assert_eq!(route.cause, CrashCause::Trap);
    let guard = route.guard.as_ref().unwrap();
    assert_eq!((guard.package.as_str(), guard.file.as_str()), ("geometry", "geometry.omg"));
    assert_eq!(guard.span.slice(library.as_bytes()), Some(b"x == 0".as_slice()));

    // L4: the guard is false for 3, so the invocation is admitted and its result frozen.
    spec_corpus::observe_handoff!(
        "guarded-library-calls-and-forwarded-callbacks/consumer",
        assert_eq!(prepare(stage).unwrap(), [("SAFE".to_owned(), PreparedValue::Integer(3u8.into()))])
    );

    // The guard holds for 0, so the same client call is refused at the client's call site.
    let refused = "const SAFE:u8=geo\\guarded(0u8);";
    let error = prepare(typed(&library, refused)).unwrap_err();
    assert_eq!(error.message, ADMISSION);
    assert_eq!((error.package.as_str(), error.file.as_str()), ("client", "client.omg"));
    assert_eq!(error.span.slice(refused.as_bytes()), Some(b"geo\\guarded(0u8)".as_slice()));

    // Changing only the library guard changes which client operand is admitted.
    // L4 therefore read the guard from the library, not a property of the call.
    let moved = producer("x == 3");
    let error = prepare(typed(&moved, client)).unwrap_err();
    assert_eq!(error.message, ADMISSION);
    assert_eq!(error.span.slice(client.as_bytes()), Some(b"geo\\guarded(3u8)".as_slice()));
    assert_eq!(
        prepare(typed(&moved, refused)).unwrap(),
        [("SAFE".to_owned(), PreparedValue::Integer(0u8.into()))]
    );
}
