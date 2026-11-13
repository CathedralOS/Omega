use crate::{
    BuildTimeEvaluationRequest, BuildTimeSources, PureEvaluationRecord, evaluate_pre_resolution,
};

#[test]
fn an_empty_frontier_preserves_its_input_without_execution_receipts() {
    let original = syntax_trees::SyntaxTrees::new(source::SourceId(17));
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: original.clone(),
        source_context: BuildTimeSources::default(),
    })
    .expect("no source normalization work");
    let (returned, pending) = evaluated.into_syntax_and_pre_check();
    assert!(returned == original);
    let limits = compiler_storage::RecordLimits::default();
    let bytes = compiler_storage::encode(&pending, limits).expect("actual empty continuation");
    let mut restored: crate::PreCheckEvaluation =
        compiler_storage::decode(&bytes, limits).expect("current source-input schema");
    let mut typed = typed_trees::TypedTrees::default();
    restored
        .rejoin_source_authority(&typed, None)
        .expect("known absent authority needs no invented source map");
    let pending = restored;
    let before = typed.clone();
    let record = pending
        .evaluate_or_defer_recorded(&mut typed)
        .expect("no typed work");
    assert!(typed == before);
    assert!(record.pending.is_none());
    assert!(record.accounting.receipts().is_empty());
}

#[test]
fn nonempty_syntax_stays_exact_until_typed_requests_are_known() {
    let mut original = syntax_trees::SyntaxTrees::new(source::SourceId(17));
    original.push_root_item(syntax_trees::item::Item::Module(Default::default()));
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: original.clone(),
        source_context: BuildTimeSources::default(),
    })
    .expect("syntax is retained, not evaluated or accepted here");
    let (returned, _) = evaluated.into_syntax_and_pre_check();
    assert!(returned == original);
    assert_eq!(returned.root_item_count(), 1);
}

#[test]
fn ordinary_declarations_are_retained_without_claiming_execution() {
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .expect("empty syntax");
    let (_, pending) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_trees::TypedTrees::default();
    typed.push_data_definition(Default::default());
    let before = typed.clone();
    let record = pending
        .evaluate_or_defer_recorded(&mut typed)
        .expect("the declaration alone requests no evaluation");
    assert!(typed == before);
    assert!(record.pending.is_none());
    assert!(record.accounting.receipts().is_empty());
}

#[test]
fn a_real_constant_request_refuses_without_erasing_its_input() {
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .unwrap();
    let (_, pending) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_trees::TypedTrees::default();
    typed.push_const_declaration(Default::default());
    let before = typed.clone();
    let errors = pending
        .evaluate(&mut typed)
        .expect_err("no constant producer has run");
    assert!(errors[0].message.contains("source constant evaluation"));
    assert!(typed == before);
}

#[test]
fn orphan_result_rows_do_not_make_a_source_frontier_empty() {
    let mut closure = typed_trees::TypedTrees::default();
    closure
        .constant_recipe_closure
        .target_bodies
        .push(Default::default());
    // The legacy convenience predicate sees only values; admission must not
    // ignore retained target selection or receipt dimensions.
    assert!(closure.constant_recipe_closure.is_empty());
    let mut placements = typed_trees::TypedTrees::default();
    placements.wire_placements.insert(Default::default());
    let mut encoding = typed_trees::TypedTrees::default();
    encoding.wire_encode_obligations.insert(Default::default());
    let mut proved = typed_trees::TypedTrees::default();
    proved.retain_proven_type_contexts(vec![typed_trees::ProvenTypeContext {
        expression: Default::default(),
        equations: Vec::new(),
        reachable: true,
        case_value: Some(true),
        admitted_cases: None,
    }]);
    for mut typed in [closure, placements, encoding, proved] {
        let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
            syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
            source_context: BuildTimeSources::default(),
        })
        .unwrap();
        let (_, pending) = evaluated.into_syntax_and_pre_check();
        let before = typed.clone();
        assert!(pending.evaluate(&mut typed).is_err());
        assert!(typed == before);
    }
}

#[test]
fn ordinary_contract_sources_reach_typed_preparation_without_being_evaluated() {
    // These existing fixtures include an allowed branch computation and one
    // whose arithmetic promise is unproved. Preparation retains each source;
    // only the later checker judges its body.
    for (path, text) in [
        (
            "tests/fixtures/pass/contracts/consistent_branches_quiet_exit/main.omg",
            include_str!(
                "../../../../../tests/fixtures/pass/contracts/consistent_branches_quiet_exit/main.omg"
            ),
        ),
        (
            "tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction/main.omg",
            include_str!(
                "../../../../../tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction/main.omg"
            ),
        ),
    ] {
        let mut sources = source::SourceMap::from_files(Vec::new());
        let source_id = sources
            .add(std::path::PathBuf::from(path), text.to_owned())
            .source_id;
        let sources = std::sync::Arc::new(sources);
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("existing source lexes");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("existing source parses");
        let original = syntax.clone();
        let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
            syntax_trees: syntax,
            source_context: BuildTimeSources {
                sources: Some(std::sync::Arc::clone(&sources)),
                ..BuildTimeSources::default()
            },
        })
        .expect("ordinary source is retained until requests are typed");
        let (syntax, pending) = evaluated.into_syntax_and_pre_check();
        assert!(syntax == original);
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax,
                sources: Some(sources),
                top_level_bindings: Vec::new(),
                retained_base: None,
            },
        )
        .expect("source resolves through the ordinary resolver");
        let mut typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("source reaches the ordinary typed frontier");
        assert!(typed.machines().len() >= 2);
        let before = typed.clone();
        let record = pending
            .evaluate_or_defer_recorded(&mut typed)
            .expect("these bodies request no source evaluation");
        assert!(typed == before);
        assert!(record.pending.is_none());
        assert!(record.accounting.receipts().is_empty());
    }
}

#[test]
fn retired_tree_receipts_cannot_be_decoded_as_canonical_scalar_receipts() {
    let limits = compiler_storage::RecordLimits::default();
    // The old enum reserved wire tag zero for a source/checked-tree receipt.
    // No old tree or supposed proof is constructed to test that boundary.
    let bytes = compiler_storage::encode(&0_u32, limits).expect("retired tag");
    let error = compiler_storage::decode::<PureEvaluationRecord>(&bytes, limits)
        .expect_err("retired source evaluation must not become canonical evidence");
    assert!(
        error
            .to_string()
            .contains("retired tree-evaluation receipt")
    );
}

/// Type `files` together as one retained base; the continuation is the
/// base's pending evaluation.
fn typed_base(
    files: &[(&str, &str)],
) -> (
    std::sync::Arc<source::SourceMap>,
    Vec<source::SourceId>,
    symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    crate::PreCheckEvaluation,
) {
    let mut sources = source::SourceMap::from_files(Vec::new());
    let mut identities = Vec::new();
    let mut syntax: Option<syntax_trees::SyntaxTrees> = None;
    for (path, text) in files {
        let source_id = sources
            .add(std::path::PathBuf::from(path), (*text).to_owned())
            .source_id;
        identities.push(source_id);
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("source lexes");
        let page = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
            .expect("source parses");
        match &mut syntax {
            Some(syntax) => syntax.extend_from(&page),
            None => syntax = Some(page),
        }
    }
    let sources = std::sync::Arc::new(sources);
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax.expect("at least one source"),
        source_context: BuildTimeSources {
            sources: Some(std::sync::Arc::clone(&sources)),
            ..BuildTimeSources::default()
        },
    })
    .expect("source is retained until its requests are typed");
    let (syntax, pending) = evaluated.into_syntax_and_pre_check();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::clone(&sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        },
    )
    .expect("sources resolve");
    let base =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees_to_seeded_base(resolved)
            .expect("sources reach the typed frontier");
    (sources, identities, base, pending)
}

fn machine_names(typed: &typed_trees::TypedTrees) -> Vec<&str> {
    let mut names = typed
        .machines()
        .iter()
        .map(|machine| machine.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    names
}

#[test]
fn an_activated_program_keeps_only_its_selected_sources() {
    // A retained base holds two files and the program being compiled
    // selects the first. The second file's machine and constant leave the
    // program, and the constant's evaluation request stays with the base.
    let (sources, identities, base, pending) = typed_base(&[
        (
            "program/main.omg",
            "machine entry() {
    local()
}
machine local() { }
",
        ),
        (
            "program/library.omg",
            "machine unreached() { }
const UNREACHED: u32 = 7;
",
        ),
    ]);
    assert_eq!(machine_names(base.typed()), ["entry", "local", "unreached"]);
    assert_eq!(base.typed().const_declarations().len(), 1);

    let mut selected = sources.as_ref().clone();
    selected.retain_selected_sources(|file| file.source_id == identities[0]);
    let selected = std::sync::Arc::new(selected);
    let mut whole = base.typed().clone();
    let activated = base
        .activate_sources(
            std::sync::Arc::clone(&selected),
            std::sync::Arc::clone(&sources),
            Vec::new(),
        )
        .expect("the selected program is narrowed out of its base");
    assert_eq!(machine_names(activated.typed()), ["entry", "local"]);
    assert!(activated.typed().const_declarations().is_empty());
    assert_eq!(
        activated
            .resolved()
            .roots
            .machines
            .iter()
            .map(|root| activated.resolved().symbols.name(root.symbol))
            .collect::<Vec<_>>(),
        ["entry", "local"],
        "a later continuation extends the same narrowed program"
    );

    let continuation = pending
        .activate_sources(activated.typed())
        .expect("the activated program asks for no evaluation");
    let mut program = activated.typed().clone();
    continuation
        .evaluate(&mut program)
        .expect("the activated program has no evaluation request");
    // The base still owes its constant: no evaluator has run it.
    let errors = pending
        .evaluate(&mut whole)
        .expect_err("the base's constant request is not erased");
    assert!(errors[0].message.contains("source constant evaluation"));
}

#[test]
fn an_activated_program_still_owes_the_constants_it_keeps() {
    // The selected file declares the constant itself, so activation keeps it
    // and the continuation refuses: nothing has evaluated it.
    let (sources, identities, base, pending) = typed_base(&[
        (
            "program/main.omg",
            "const KEPT: u32 = 7;\nmachine entry() { }\n",
        ),
        ("program/library.omg", "machine unreached() { }\n"),
    ]);
    let mut selected = sources.as_ref().clone();
    selected.retain_selected_sources(|file| file.source_id == identities[0]);
    let activated = base
        .activate_sources(std::sync::Arc::new(selected), sources, Vec::new())
        .expect("the selected program is narrowed out of its base");
    assert_eq!(machine_names(activated.typed()), ["entry"]);
    assert_eq!(activated.typed().const_declarations().len(), 1);
    let errors = pending
        .activate_sources(activated.typed())
        .err()
        .expect("a kept constant is an unserved evaluation request");
    assert!(errors[0].message.contains("source constant evaluation"));
}
