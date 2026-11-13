//! A Build program's calls of the toolchain protocols are published with the
//! rows its host reads beside them. Each test types a Build program from
//! source through the ordinary lexer, parser, resolver and typer, has the
//! lowered-form checker judge it, and reads what production publishes.

use language_semantics::declaration_selection::{BuildSelectionRequest, RootBindingOperand};
use psi_terminal_production::{
    AcceptedTerminalSource, BuildCallSite, ProducedTerminalProgram, TerminalBuildBindingOperation,
    TerminalBuildRequestOperation, TerminalProductionCustody, UnmeasuredProduction,
};
use source::DependencyScope;
use std::sync::Arc;

/// The part of the toolchain Build prelude these programs use, declared the
/// way the prelude declares it: in a toolchain source named `<build-prelude>`.
/// Each operation states `crashes none`, as the prelude's do: the checker
/// refuses a boundary call with no published crash contract.
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
    accepted_for(build, terminal_codec::TerminalPublication::Invocation)
}

fn accepted_for(
    build: &str,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<diagnostics::Diagnostic>> {
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
        publication,
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

/// Every boundary call the published artifact makes, as the site a host
/// row names: a row is about a call the artifact really makes.
fn published_boundary_calls(program: &ProducedTerminalProgram) -> Vec<BuildCallSite> {
    let mut calls = Vec::new();
    for machine in &program.view().module().machines {
        for operation in machine.operations() {
            if let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind {
                calls.push(BuildCallSite {
                    machine: machine.id,
                    operation: operation.id,
                    boundary: *boundary,
                });
            }
        }
    }
    calls
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
        assert!(emitted.contains(&call.site));
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
    assert_ne!(calls[0].site, calls[1].site);

    let requests = program.build_requests().calls_for(view).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].action,
        TerminalBuildRequestOperation::ArtifactOnly
    );
    assert!(emitted.contains(&requests[0].site));
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

const TWO_HELPERS: &str = r#"
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
"#;

/// Operations are numbered within each machine, so two helpers that each
/// bind an entry make their calls at the same operation numbers of the same
/// boundaries. Each call still has its own row, told apart by its machine,
/// and each row keeps the entry its own helper named.
#[test]
fn two_machines_that_bind_at_the_same_operation_number_keep_their_own_rows() {
    let program = produced(TWO_HELPERS).unwrap();
    let calls = program.build_bindings().calls_for(program.view()).unwrap();
    let emitted = published_boundary_calls(&program);
    let binds: Vec<_> = calls
        .iter()
        .filter(|call| call.action == TerminalBuildBindingOperation::Bind)
        .map(|call| {
            let BuildSelectionRequest::Entry(binding) = &call.request else {
                panic!("{:?}", call.request)
            };
            assert!(emitted.contains(&call.site));
            (call.site, binding.slot.as_str())
        })
        .collect();
    let [(first, first_slot), (second, second_slot)] = binds.as_slice() else {
        panic!("{binds:?}")
    };
    assert_eq!(
        [*first_slot, *second_slot],
        ["linux_x86_64::ProgramEntry", "windows_x86_64::ProgramEntry"]
    );
    // The collision the machine resolves: without it the two rows would be
    // one key with two different entries.
    assert_eq!(
        (first.operation, first.boundary),
        (second.operation, second.boundary)
    );
    assert_ne!(first.machine, second.machine);
}

/// The failing twin: a row is for one machine's call. The other helper's
/// call at the same operation number and boundary is not that row's call,
/// so a host that looks a call up by its site cannot be handed the wrong
/// entry.
#[test]
fn a_row_does_not_answer_for_the_same_operation_of_another_machine() {
    let program = produced(TWO_HELPERS).unwrap();
    let calls = program.build_bindings().calls_for(program.view()).unwrap();
    for site in published_boundary_calls(&program) {
        let rows: Vec<_> = calls.iter().filter(|call| call.site == site).collect();
        assert_eq!(rows.len(), 1, "{site:?}");
    }
}

/// A request row is per call too: two machines that make the same request
/// at the same operation number publish two rows.
#[test]
fn two_machines_that_make_the_same_request_each_publish_its_row() {
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
    let sites: Vec<_> = requests.iter().map(|request| request.site).collect();
    assert_eq!(sites, published_boundary_calls(&program));
    assert_eq!(sites.len(), 2);
    assert_ne!(sites[0].machine, sites[1].machine);
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

/// A saved artifact's code digest does not authenticate separately decoded
/// host actions. A valid site with a different action or protocol family must
/// not acquire authority; even an unmodified saved Build row needs its source
/// correspondence rejoined. Fresh accepted production remains available.
#[test]
fn saved_build_rows_do_not_authorize_changed_actions_or_protocol_families() {
    use compiler_storage::{Record, RecordLimits, RecordWriter};
    use psi_terminal_production::{TerminalBuildQueryCall, TerminalBuildRequestCall};

    let program = accepted_for(ONE_BINDING, terminal_codec::TerminalPublication::Portable)
        .unwrap()
        .produce(
            TerminalProductionCustody::default(),
            &mut UnmeasuredProduction,
        )
        .unwrap();
    let refused = compiler_storage::encode(&program, RecordLimits::default()).unwrap_err();
    assert!(
        refused.to_string().contains(
            "saved Build companions require their accepted source protocol correspondence"
        ),
        "{refused}"
    );
    let view = program.view();
    let original = program.build_requests().calls_for(view).unwrap()[0].clone();
    let binding = &program.build_bindings().calls_for(view).unwrap()[0];
    let mut different_action = original.clone();
    different_action.action = TerminalBuildRequestOperation::Application;
    let mut different_family = original.clone();
    different_family.site = binding.site;
    for request in [original, different_action, different_family] {
        // Write one altered companion beside the real accepted artifact,
        // using exactly the existing private record layout. No new evidence
        // or source protocol declaration is supplied by these bytes.
        let mut output = RecordWriter::new(RecordLimits::default());
        output.record(|output| {
            2_u32.write(output)?;
            program.artifact().unwrap().to_bytes().write(output)?;
            Arc::<[psi_terminal_production::TerminalBuildBindingCall]>::from([]).write(output)?;
            Arc::<[Arc<language_semantics::declaration_selection::ProductDeclarationRequest>]>::from([]).write(output)?;
            Arc::<[TerminalBuildQueryCall]>::from([]).write(output)?;
            Arc::<[language_semantics::declaration_selection::NativeBindingFormation]>::from([]).write(output)?;
            Arc::<[TerminalBuildRequestCall]>::from([request]).write(output)?;
            Arc::<[psi_terminal_production::TerminalBuildFilesystemCall]>::from([]).write(output)
        }).unwrap();
        let refused = compiler_storage::decode::<ProducedTerminalProgram>(
            &output.finish(),
            RecordLimits::default(),
        )
        .unwrap_err();
        assert!(
            refused.to_string().contains(
                "saved Build companions require their accepted source protocol correspondence"
            ),
            "{refused}"
        );
    }
}

/// Portable artifacts without host actions still use the normal persistence
/// path, without a fabricated protocol witness.
#[test]
fn a_portable_artifact_without_build_companions_still_round_trips() {
    let program = accepted_for(
        "machine build(builder: &mut Build) {}",
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap()
    .produce(
        TerminalProductionCustody::default(),
        &mut UnmeasuredProduction,
    )
    .unwrap();
    let bytes =
        compiler_storage::encode(&program, compiler_storage::RecordLimits::default()).unwrap();
    let restored = compiler_storage::decode::<ProducedTerminalProgram>(
        &bytes,
        compiler_storage::RecordLimits::default(),
    )
    .unwrap();
    assert_eq!(
        restored.view().custody_identity(),
        program.view().custody_identity()
    );
    assert!(
        restored
            .build_bindings()
            .calls_for(restored.view())
            .unwrap()
            .is_empty()
    );
    assert!(
        restored
            .build_requests()
            .calls_for(restored.view())
            .unwrap()
            .is_empty()
    );
}
