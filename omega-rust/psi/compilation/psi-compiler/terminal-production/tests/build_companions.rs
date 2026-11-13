//! A Build program's calls of the toolchain protocols are published with the
//! rows its host reads beside them. Each test types a Build program from
//! source through the ordinary lexer, parser, resolver and typer, has the
//! lowered-form checker judge it, and reads what production publishes.

use language_semantics::declaration_selection::{BuildSelectionRequest, RootBindingOperand};
use psi_terminal_production::{
    AcceptedTerminalSource, ProducedTerminalProgram, TerminalBuildBindingOperation,
    TerminalBuildRequestOperation, TerminalProductionCustody, UnmeasuredProduction,
};
use source::DependencyScope;
use std::sync::Arc;

/// The part of the toolchain Build prelude these programs use, declared the
/// way the prelude declares it: in a toolchain source named `<build-prelude>`.
/// Each operation also states `crashes none`, which the real prelude does
/// not yet: the checker refuses a boundary call with no published crash
/// contract, and that refusal is not what these tests are about.
const PRELUDE: &str = r#"
public data Build {
    marker: u64;
}
public boundary trait BuildBindingOperations {
    machine prepare(builder: &Build) -> u64 reaches none crashes none;
    machine bind(activation: u64) reaches none crashes none;
}
public boundary trait BuildRequestOperations {
    machine artifact_only(builder: &Build) reaches none crashes none;
}
public boundary trait Probe {
    machine poke(value: u64) reaches none crashes none;
}
"#;

fn package() -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap()
}

fn typed(build: &str) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let mut syntax = None;
    let mut ids = Vec::new();
    for (path, text, origin) in [
        ("package/build.omg", build, source::SourceOrigin::User),
        ("<build-prelude>", PRELUDE, source::SourceOrigin::Toolchain),
    ] {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                "package".into(),
                Some(package()),
                origin,
                source::SourceResolutionStratum::Base,
                DependencyScope::Build,
            )
            .source_id;
        ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            None => {
                syntax =
                    Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap());
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    // The Build script sees the prelude's `Build` as the compiler binds it.
    request.top_level_bindings = vec![symbols::SourceScopedTopLevelBinding::new(
        ids[0], ids[1], "Build",
    )];
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn accepted(build: &str) -> Result<AcceptedTerminalSource, Vec<diagnostics::Diagnostic>> {
    let typed = typed(build);
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .unwrap()
        .symbol;
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    AcceptedTerminalSource::judge(
        Arc::new(frozen),
        structure_elaboration::Compilation {
            entries: &[entry],
            subjects: &[Some(package())],
            purpose: DependencyScope::Build,
            selected: &[],
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn produced(build: &str) -> Result<ProducedTerminalProgram, Vec<diagnostics::Diagnostic>> {
    accepted(build)
        .unwrap_or_else(|refused| panic!("{refused:?}"))
        .produce(
            TerminalProductionCustody::default(),
            &mut UnmeasuredProduction,
        )
}

/// The boundary each `(machine, operation, boundary)` row names, read back from the
/// published artifact: the row is about a call the artifact really makes.
fn published_boundary_calls(
    program: &ProducedTerminalProgram,
) -> Vec<(
    semantic_vocabulary::MachineId,
    semantic_vocabulary::OperationId,
    semantic_vocabulary::BoundaryMachineId,
)> {
    program
        .view()
        .module()
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .operations()
                .filter_map(move |operation| match &operation.kind {
                    terminal_psi::OperationKind::BoundaryCall { boundary, .. } => {
                        Some((machine.id, operation.id, *boundary))
                    }
                    _ => None,
                })
        })
        .collect()
}

const ONE_BINDING: &str = r#"
machine build(builder: &mut Build) {
    BuildRequestOperations::artifact_only(builder);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
"#;

#[test]
fn a_root_binding_is_published_with_the_entry_it_names() {
    let program = produced(ONE_BINDING).unwrap();
    let view = program.view();
    let emitted = published_boundary_calls(&program);
    assert_eq!(emitted.len(), 3, "the request, the prepare and the bind");

    let calls = program.build_bindings().calls_for(view).unwrap();
    assert_eq!(
        calls.iter().map(|call| call.action).collect::<Vec<_>>(),
        [
            TerminalBuildBindingOperation::Prepare,
            TerminalBuildBindingOperation::Bind
        ]
    );
    for call in calls.iter() {
        assert!(emitted.contains(&(call.machine, call.operation, call.boundary)));
        let BuildSelectionRequest::Entry(binding) = &call.request else {
            panic!("{:?}", call.request)
        };
        assert_eq!(binding.slot, "linux_x86_64::ProgramEntry");
        assert_eq!(
            binding.implementation,
            RootBindingOperand::AuthoredPath("main".into())
        );
        // A root binding names its entry by path; it resolves nothing from
        // the selecting machine's own scope.
        assert_eq!(call.selecting_machine, None);
        assert!(call.static_environment.is_none());
        assert!(call.native_locator.is_none());
    }
    // The two calls of one selection are different operations.
    assert_ne!(
        (calls[0].operation, calls[0].boundary),
        (calls[1].operation, calls[1].boundary)
    );

    let requests = program.build_requests().calls_for(view).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].action,
        TerminalBuildRequestOperation::ArtifactOnly
    );
    assert!(emitted.contains(&(
        requests[0].machine,
        requests[0].operation,
        requests[0].boundary
    )));
    assert!(
        program
            .build_bindings()
            .declarations_for(view)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn each_binding_of_one_machine_keeps_its_own_entry() {
    let program = produced(
        r#"
machine build(builder: &mut Build) {
    Build::bind(builder, linux_x86_64::ProgramEntry, first);
    Build::bind(builder, windows_x86_64::ProgramEntry, second);
}
"#,
    )
    .unwrap();
    let calls = program.build_bindings().calls_for(program.view()).unwrap();
    let named: Vec<_> = calls
        .iter()
        .map(|call| {
            let BuildSelectionRequest::Entry(binding) = &call.request else {
                panic!("{:?}", call.request)
            };
            (call.action, binding.slot.as_str())
        })
        .collect();
    assert_eq!(
        named,
        [
            (
                TerminalBuildBindingOperation::Prepare,
                "linux_x86_64::ProgramEntry"
            ),
            (
                TerminalBuildBindingOperation::Bind,
                "linux_x86_64::ProgramEntry"
            ),
            (
                TerminalBuildBindingOperation::Prepare,
                "windows_x86_64::ProgramEntry"
            ),
            (
                TerminalBuildBindingOperation::Bind,
                "windows_x86_64::ProgramEntry"
            ),
        ]
    );
}

#[test]
fn a_build_with_no_selection_publishes_no_binding_row() {
    let program = produced(
        r#"
machine build(builder: &mut Build) {
    BuildRequestOperations::artifact_only(builder);
}
"#,
    )
    .unwrap();
    let view = program.view();
    assert!(program.build_bindings().calls_for(view).unwrap().is_empty());
    assert_eq!(program.build_requests().calls_for(view).unwrap().len(), 1);
}

/// Helpers can bind different entries at the same local operation number.
/// Their emitted machine keeps the host requests distinct.
#[test]
fn two_machines_keep_distinct_bindings_at_the_same_operation_number() {
    let program = produced(
        r#"
machine build(builder: &mut Build) {
    first(builder);
    second(builder);
}
machine first(builder: &mut Build) {
    Build::bind(builder, linux_x86_64::ProgramEntry, one);
}
machine second(builder: &mut Build) {
    Build::bind(builder, windows_x86_64::ProgramEntry, two);
}
"#,
    )
    .unwrap();
    let calls = program.build_bindings().calls_for(program.view()).unwrap();
    let emitted = published_boundary_calls(&program);
    assert_eq!(calls.len(), 4);
    for call in calls.iter() {
        assert!(emitted.contains(&(call.machine, call.operation, call.boundary)));
        let BuildSelectionRequest::Entry(binding) = &call.request else {
            panic!("{:?}", call.request);
        };
        let RootBindingOperand::AuthoredPath(entry) = &binding.implementation else {
            panic!("{:?}", binding.implementation);
        };
        let expected = match entry.as_str() {
            "one" => "linux_x86_64::ProgramEntry",
            "two" => "windows_x86_64::ProgramEntry",
            other => panic!("unexpected selected entry {other}"),
        };
        assert_eq!(binding.slot, expected);
    }
    let bindings: Vec<_> = calls
        .iter()
        .filter(|call| call.action == TerminalBuildBindingOperation::Bind)
        .collect();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].operation, bindings[1].operation);
    assert_eq!(bindings[0].boundary, bindings[1].boundary);
    assert_ne!(bindings[0].machine, bindings[1].machine);
}

/// Identical request operations still retain each helper's exact call identity.
#[test]
fn two_machines_that_make_the_same_request_keep_their_own_rows() {
    let program = produced(
        r#"
machine build(builder: &mut Build) {
    first(builder);
    second(builder);
}
machine first(builder: &mut Build) {
    BuildRequestOperations::artifact_only(builder);
}
machine second(builder: &mut Build) {
    BuildRequestOperations::artifact_only(builder);
}
"#,
    )
    .unwrap();
    let requests = program.build_requests().calls_for(program.view()).unwrap();
    assert_eq!(published_boundary_calls(&program).len(), 2);
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].operation, requests[1].operation);
    assert_eq!(requests[0].boundary, requests[1].boundary);
    assert_ne!(requests[0].machine, requests[1].machine);
    let emitted = published_boundary_calls(&program);
    for request in requests.iter() {
        assert!(emitted.contains(&(request.machine, request.operation, request.boundary)));
    }
}

/// Build admission bounds an activation's authority by what its accepted
/// program asks of the host. A call of the toolchain's protocols is inside
/// that bound; a call of any other boundary is outside it, and gets no row.
/// `Probe` is declared in the prelude's own source and is still outside: a
/// protocol is one of the four the toolchain names, not anything it ships.
#[test]
fn a_boundary_outside_the_build_protocols_is_reported_and_given_no_row() {
    let inside = accepted(ONE_BINDING).unwrap();
    let asked = inside.build_protocol_use().unwrap();
    assert!(!asked.reaches_outside);
    assert!(!asked.filesystem);

    let outside = accepted(
        r#"
machine build(builder: &mut Build) {
    BuildRequestOperations::artifact_only(builder);
    Probe::poke(7);
}
"#,
    )
    .unwrap();
    assert!(outside.build_protocol_use().unwrap().reaches_outside);
    let program = outside
        .produce(
            TerminalProductionCustody::default(),
            &mut UnmeasuredProduction,
        )
        .unwrap();
    assert_eq!(published_boundary_calls(&program).len(), 2);
    assert_eq!(
        program
            .build_requests()
            .calls_for(program.view())
            .unwrap()
            .len(),
        1
    );
}
