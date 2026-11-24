use checked_program_to_psi as production;
use syntax_trees_to_symbol_resolved_trees::resolution;
use symbol_resolved_trees_to_typed_trees as typing;
use crate::driver::judge_lowered_source;
use typed_trees_to_prepared_typed_trees as preparation;
use super::*;

fn deferred(
    text: &str,
) -> (
    typing::SeededTypingBase,
    resolution::DeferredResolvedFrontier,
    Arc<crate::source_inputs::SourceMap>,
) {
    let mut sources = crate::source_inputs::SourceMap::default();
    let id = sources
        .add("completion/main.omg".into(), text.to_owned())
        .source_id;
    let sources = Arc::new(sources);
    let tokens = crate::test_support::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    let syntax = crate::test_support::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let frontier = resolution::pre_resolution::resolve_deferred_numeric_probe(
        resolution::ResolutionRequest {
            syntax,
            sources: Some(Arc::clone(&sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .unwrap();
    let typing =
        typing::lower_symbol_resolved_trees_to_seeded_base(frontier.shared_trees())
            .unwrap();
    (typing, frontier, sources)
}

#[test]
fn resolution_handoff_completes_each_batch_without_changing_its_predecessor() {
    let (mut typing, mut frontier, sources) = deferred(
        "machine first(value: u64) -> u64 { value } machine second(value: u64) -> u64 { value }",
    );
    frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let (activated, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
    frontier = activated;
    typing = typing.activate_body_completion(activation).unwrap();
    let root = frontier.trees().symbols.root();
    assert!(std::ptr::eq(
        typing.typed().symbols.get(root),
        frontier.trees().symbols.get(root),
    ));
    let pending: Vec<_> = frontier.pending_machines().collect();
    assert_eq!(pending.len(), 2);
    for selected in pending {
        frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let predecessor = typing.shared_resolved();
        let snapshot = predecessor.as_ref().clone();
        let completed = frontier.complete_machine_bodies(&[selected]).unwrap();
        (typing, frontier) = type_completed_bodies(typing, completed).unwrap();
        assert_eq!(predecessor.as_ref(), &snapshot);
        assert_eq!(typing.resolved(), frontier.trees());
    }
    assert_eq!(frontier.pending_machines().count(), 0);
    assert_eq!(typing.typed().machines.len(), 2);
}

#[test]
fn nonempty_selected_completion_retains_prior_refusal_rows() {
    use crate::source_state::deferred_refusals::DeferredRequest;

    let (mut typing, frontier, sources) = deferred(
        "machine length() -> u64 { 4 } \
         machine first() { let bytes: [u8; length()]; } \
         machine second() { let bytes: [u8; length()]; }",
    );
    let frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let (mut frontier, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
    typing = typing.activate_body_completion(activation).unwrap();
    let owner = |name: &str| {
        frontier
            .pending_machines()
            .find(|symbol| frontier.trees().symbols.name(*symbol) == name)
            .unwrap()
    };
    let first = owner("first");
    let second = owner("second");
    for selected in [first, second] {
        frontier = frontier
            .with_prepared_allocations(typing.prepare_allocations().unwrap())
            .unwrap();
        let predecessor = typing.clone();
        let completed = frontier.complete_machine_bodies(&[selected]).unwrap();
        (typing, frontier) = type_completed_bodies(typing, completed).unwrap();
        assert!(predecessor.typed().deferred_refusal(selected).is_none());
        let introduced: Vec<_> = typing.typed().deferred_refusals_of(selected).collect();
        assert!(introduced.iter().all(|row| row.declaration == selected));
        assert_eq!(
            introduced.iter().map(|row| row.request).collect::<Vec<_>>(),
            [
                DeferredRequest::TypeExpressionEvaluation,
                DeferredRequest::ArrayExtentEvaluation
            ]
        );
        PreCheckEvaluation::require_recorded_requests(typing.typed()).unwrap();
        for retained in &predecessor.typed().deferred_refusals {
            let successor = typing
                .typed()
                .deferred_refusals_of(retained.declaration)
                .find(|row| row.request == retained.request)
                .unwrap();
            assert!(std::ptr::eq(retained, successor));
        }
        assert_eq!(typing.resolved(), frontier.trees());
    }
    assert_eq!(typing.typed().deferred_refusals.len(), 4);
    assert_eq!(frontier.pending_machines().count(), 1);
}

#[test]
fn resolution_handoff_refuses_cross_paired_completion_and_activation() {
    let (typing, _, _) = deferred("machine first() -> u64 { 1 }");
    let (_, frontier, sources) = deferred("machine second() -> u64 { 2 }");
    let selected: Vec<_> = frontier.pending_machines().collect();
    let completed = frontier.clone().complete_machine_bodies(&selected).unwrap();
    let error = type_completed_bodies(typing.clone(), completed).unwrap_err();
    assert!(
        error.iter().any(|diagnostic| diagnostic
            .message
            .contains("exact retained resolved predecessor")),
        "{error:?}"
    );
    let (_, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
    let error = typing.activate_body_completion(activation).unwrap_err();
    assert!(
        error.iter().any(|diagnostic| diagnostic
            .message
            .contains("exact retained resolved predecessor")),
        "{error:?}"
    );
}

#[test]
fn completed_body_work_is_recorded_before_retained_source_activation() {
    use crate::source_state::deferred_refusals::DeferredRequest;

    let text = "machine length() -> u64 { 4 }\n\
                machine helper() { let bytes: [u8; length()]; }\n";
    let (mut typing, frontier, sources) = deferred(text);
    let pending = PreCheckEvaluation::new(None);
    PreCheckEvaluation::defer_declared_requests(typing.typed_mut()).unwrap();
    let frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let (frontier, activation) = frontier.activate_sources(sources, Vec::new()).unwrap();
    typing = typing.activate_body_completion(activation).unwrap();
    let frontier = frontier
        .with_prepared_allocations(typing.prepare_allocations().unwrap())
        .unwrap();
    let helper = frontier
        .pending_machines()
        .find(|symbol| frontier.trees().symbols.name(*symbol) == "helper")
        .unwrap();
    assert!(typing.typed().deferred_refusal(helper).is_none());
    let completed = frontier.complete_machine_bodies(&[helper]).unwrap();
    let (typing, _) = type_completed_bodies(typing, completed).unwrap();

    pending
        .activate_sources(typing.typed())
        .expect("new declaration-owned work is recorded before source activation");
    assert!(
        typing
            .typed()
            .deferred_refusals_of(helper)
            .any(|row| row.request == DeferredRequest::ArrayExtentEvaluation)
    );
    let unserved = typing.typed().deferred_refusal(helper).unwrap().request;
    let program = preparation::prepare_typed_trees(typing.typed().clone(), &[], &[]).unwrap();
    let refused = judge_lowered_source::judge_terminal_source(
        program,
        production::TerminalSourceCompilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[helper],
            subjects: &[],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect_err("recording the request cannot admit the helper that needs its evaluation");
    assert!(
        refused
            .iter()
            .any(|diagnostic| diagnostic.message.contains(unserved.construct())),
        "{refused:?}"
    );
}
