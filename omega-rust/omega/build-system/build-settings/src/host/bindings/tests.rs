//! Source-produced helper calls reach the real Build host with separate sites.
//! The test supplies a host-owned receiver and calls the actual host adapter;
//! it does not run ordinary Build activation or the interpreted source body.

use super::*;
use terminal_interpreter::{TerminalEffectWait, TerminalScalarValue};

const PRELUDE: &str = r#"
public data Build { marker: u64; }
public boundary trait BuildBindingOperations {
    machine prepare(builder: &Build) -> u64 reaches none crashes none;
    machine bind(activation: u64) reaches none crashes none;
}
"#;

const BUILD: &str = r#"
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

fn produce(build: &str) -> psi_compiler::ProducedTerminalProgram {
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap();
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
                Some(package),
                origin,
                source::SourceResolutionStratum::Base,
                source::DependencyScope::Build,
            )
            .source_id;
        ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            None => {
                syntax =
                    Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap())
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = vec![symbols::SourceScopedTopLevelBinding::new(
        ids[0], ids[1], "Build",
    )];
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .unwrap()
        .symbol;
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    let accepted = psi_compiler::AcceptedTerminalSource::judge(
        Arc::new(frozen),
        psi_compiler::TerminalSourceCompilation {
            entries: &[entry],
            subjects: &[Some(package)],
            purpose: source::DependencyScope::Build,
            selected: &[],
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    accepted
        .produce(Default::default(), &mut psi_compiler::UnmeasuredProduction)
        .unwrap()
}

fn effect(
    program: &psi_compiler::ProducedTerminalProgram,
    call: &TerminalBuildBindingCall,
    arguments: Vec<TerminalScalarValue>,
    structural_arguments: Vec<TerminalStructuralValue>,
) -> TerminalEffect {
    let boundary = program
        .view()
        .module()
        .boundary_machine(call.site.boundary)
        .unwrap();
    let block = program
        .view()
        .module()
        .machine(call.site.machine)
        .unwrap()
        .blocks
        .iter()
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| operation.id == call.site.operation)
        })
        .expect("the actual emitted call's block")
        .id;
    TerminalEffect::BoundaryCall {
        site: TerminalEffectWait {
            program: program.view().custody_identity(),
            machine: call.site.machine,
            block,
            operation: call.site.operation,
            boundary: call.site.boundary,
        },
        arguments,
        byte_sequence_arguments: vec![None; structural_arguments.len()],
        structural_arguments,
        completion_receipts: Vec::new(),
        result: boundary.result.clone(),
    }
}

#[test]
fn source_helper_calls_authenticate_their_site_artifact_receiver_and_ticket() {
    let program = produce(BUILD);
    let view = program.view();
    let calls = program.build_bindings().calls_for(view).unwrap();
    assert_eq!(calls.len(), 4);
    let first = &calls[0];
    let structural_type = view
        .module()
        .boundary_machine(first.site.boundary)
        .unwrap()
        .structural_parameters[0]
        .structural_type;
    let receiver = TerminalStructuralValue::minted(1, structural_type, Vec::new());
    let host =
        TerminalBuildBindingHost::new(view, program.build_bindings(), receiver.clone(), None)
            .unwrap();

    let signature =
        TerminalImportSignature::from(view.module().boundary_machine(first.site.boundary).unwrap());
    let mut adapter = host.adapter(signature);
    let mut wrong_machine = effect(&program, first, Vec::new(), vec![receiver.clone()]);
    let TerminalEffect::BoundaryCall { site, .. } = &mut wrong_machine else {
        unreachable!()
    };
    site.machine = match view.module().entry.unwrap() {
        terminal_psi::TargetMachineCallable::Machine(machine) => machine,
        _ => panic!("source entry is a machine"),
    };
    assert_ne!(site.machine, first.site.machine);
    assert!(
        adapter
            .handle_effect_result(&wrong_machine)
            .unwrap_err()
            .reason
            .contains("no source-issued designated operands")
    );

    let other = produce(BUILD);
    assert_eq!(view.identity(), other.view().identity());
    assert_ne!(view.custody_identity(), other.view().custody_identity());
    let mut wrong_artifact = effect(&program, first, Vec::new(), vec![receiver.clone()]);
    let TerminalEffect::BoundaryCall { site, .. } = &mut wrong_artifact else {
        unreachable!()
    };
    site.program = other.view().custody_identity();
    assert!(
        adapter
            .handle_effect_result(&wrong_artifact)
            .unwrap_err()
            .reason
            .contains("another accepted artifact")
    );

    let mut detached = receiver.clone();
    detached.opaque_identity += 1;
    assert!(
        adapter
            .handle_effect_result(&effect(&program, first, Vec::new(), vec![detached]))
            .unwrap_err()
            .reason
            .contains("original compiler-issued Build receiver")
    );
    drop(adapter);

    let mut previous: Option<psi_compiler::BuildCallSite> = None;
    let (pairs, remainder) = calls.as_chunks::<2>();
    assert!(remainder.is_empty());
    for [prepare, commit] in pairs {
        assert_eq!(prepare.action, TerminalBuildBindingOperation::Prepare);
        assert_eq!(commit.action, TerminalBuildBindingOperation::Bind);
        if let Some(previous) = previous {
            assert_ne!(previous.machine, prepare.site.machine);
            assert_eq!(previous.operation, prepare.site.operation);
            assert_eq!(previous.boundary, prepare.site.boundary);
        }
        previous = Some(prepare.site);
        let mut prepare_adapter = host.adapter(TerminalImportSignature::from(
            view.module()
                .boundary_machine(prepare.site.boundary)
                .unwrap(),
        ));
        let TerminalEffectResult::Scalar(ticket) = prepare_adapter
            .handle_effect_result(&effect(
                &program,
                prepare,
                Vec::new(),
                vec![receiver.clone()],
            ))
            .unwrap()
        else {
            panic!("prepare returns its activation ticket")
        };
        drop(prepare_adapter);
        let mut commit_adapter = host.adapter(TerminalImportSignature::from(
            view.module()
                .boundary_machine(commit.site.boundary)
                .unwrap(),
        ));
        let call = effect(&program, commit, vec![ticket], Vec::new());
        assert_eq!(
            commit_adapter.handle_effect_result(&call).unwrap(),
            TerminalEffectResult::Unit
        );
        assert!(
            commit_adapter
                .handle_effect_result(&call)
                .unwrap_err()
                .reason
                .contains("consumed or belongs to another request")
        );
    }
    let selected = host.take_executed().unwrap();
    let selected = selected
        .iter()
        .map(|selection| {
            let ExecutedBuildSelection::Entry(binding) = selection else {
                panic!("entry selection")
            };
            (binding.slot.as_str(), &binding.implementation)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        selected,
        [
            (
                "linux_x86_64::ProgramEntry",
                &RootBindingSelection::AuthoredPath("one".into())
            ),
            (
                "windows_x86_64::ProgramEntry",
                &RootBindingSelection::AuthoredPath("two".into())
            ),
        ]
    );
}
