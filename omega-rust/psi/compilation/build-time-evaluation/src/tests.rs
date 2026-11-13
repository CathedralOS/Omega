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
fn a_declared_constant_nobody_reads_requests_no_evaluation() {
    // The constant has no recorded value, and nothing in the program names
    // it. Carrying the declaration needs no evaluator.
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .unwrap();
    let (_, pending) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_trees::TypedTrees::default();
    typed.push_const_declaration(Default::default());
    assert!(typed.const_declarations()[0].evaluated_value.is_none());
    let before = typed.clone();
    let record = pending
        .evaluate_or_defer_recorded(&mut typed)
        .expect("a declaration alone is not an evaluation request");
    assert!(typed == before);
    assert!(record.pending.is_none());
    assert!(record.accounting.receipts().is_empty());
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

#[test]
fn an_evaluation_request_in_a_declaration_refuses_that_declaration_only() {
    use typed_trees::deferred_refusals::DeferredRequest;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    // `holder` declares a local whose type is an array with a computed
    // extent, which needs an evaluator that does not exist. `served` asks
    // for nothing.
    let (_, _, base, pending) = typed_base(&[(
        "program/main.omg",
        "machine holder() {
    let count: u32 = 0;
}
machine served() { }
",
    )]);
    let mut typed = base.typed().clone();
    let holder = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "holder")
        .expect("the holder machine");
    let (holder, state) = (holder.symbol, typed.machine_states(holder)[0].clone());
    let local = state.statement_nodes.handles().next().expect("the local");
    let StatementNode::LocalData(binding) = typed.statement_table.statement(local).clone() else {
        panic!("a typed local")
    };
    let extent = typed
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: binding.type_reference,
            length: FixedArrayLength::Expression(binding.type_reference),
        });
    let StatementNode::LocalData(binding) = typed.statement_table.statement_mut(local) else {
        panic!("a typed local")
    };
    binding.type_reference = extent;

    // A reader of the program refuses the request while nothing carries it.
    let error = match pending.activate_sources(&typed) {
        Ok(_) => panic!("an unrecorded request must not pass"),
        Err(error) => error,
    };
    assert!(
        error[0].message.contains("declaration `holder`")
            && error[0].message.contains("computed extent"),
        "{error:?}"
    );
    // Once the request is recorded on its declaration the program goes on,
    // and only that declaration is refused.
    crate::PreCheckEvaluation::defer_declared_requests(&mut typed).unwrap();
    assert_eq!(
        typed
            .deferred_refusals
            .iter()
            .map(|row| (row.declaration, row.request))
            .collect::<Vec<_>>(),
        [(holder, DeferredRequest::ArrayExtentEvaluation)]
    );
    assert!(pending.activate_sources(&typed).is_ok());

    // The step that would evaluate layout policies has no evaluator
    // either: it records the same requests and stops nothing.
    let mut prepared = typed.clone();
    prepared.deferred_refusals.clear();
    crate::evaluate_fixed_layout_applications(&mut prepared, &[], None).unwrap();
    assert_eq!(prepared.deferred_refusals, typed.deferred_refusals);

    // The twin: the same program as written requests nothing.
    let mut plain = base.typed().clone();
    crate::PreCheckEvaluation::defer_declared_requests(&mut plain).unwrap();
    crate::evaluate_fixed_layout_applications(&mut plain, &[], None).unwrap();
    assert!(plain.deferred_refusals.is_empty());
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
    // Nothing in the base reads its constant either, so the base owes no
    // evaluation for it.
    pending
        .evaluate(&mut whole)
        .expect("an unread constant is not an evaluation request");
}

/// Forget the value resolution recorded for the program's only constant,
/// leaving the declaration as one whose initializer nobody has evaluated.
fn forget_recorded_value(typed: &mut typed_trees::TypedTrees) {
    let mut declaration = typed.const_declarations()[0].clone();
    assert!(declaration.evaluated_value.take().is_some());
    typed.roots.const_declarations = Default::default();
    typed.push_const_declaration(declaration);
    assert_eq!(typed.const_declarations().len(), 1);
}

#[test]
fn an_activated_program_may_keep_a_constant_it_never_reads() {
    // The selected file declares the constant itself, so activation keeps
    // it. No machine reads it, so the program asks for no evaluation, whether
    // or not resolution recorded the constant's value.
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
    let continuation = pending
        .activate_sources(activated.typed())
        .expect("a kept constant nobody reads is not an evaluation request");
    let mut program = activated.typed().clone();
    forget_recorded_value(&mut program);
    continuation
        .evaluate(&mut program)
        .expect("an unevaluated constant nobody reads is not a request either");
}

#[test]
fn a_read_of_an_unevaluated_constant_is_refused_by_name() {
    // The machine reads the constant. With the value resolution recorded,
    // the read needs no evaluator. Without a recorded value the same read is
    // refused, naming the constant and pointing at the read; the initializer
    // text is not taken as the value.
    let text = "const LIMIT: u32 = 7;
machine entry() {
    let read: u32 = LIMIT;
}
";
    let (_, _, base, pending) = typed_base(&[("program/main.omg", text)]);
    let mut evaluated = base.typed().clone();
    assert_eq!(evaluated.const_declarations().len(), 1);
    pending
        .clone()
        .evaluate(&mut evaluated)
        .expect("resolution recorded this literal constant's value");

    let mut unevaluated = base.typed().clone();
    forget_recorded_value(&mut unevaluated);
    let before = unevaluated.clone();
    let errors = pending
        .evaluate(&mut unevaluated)
        .expect_err("no stage evaluated the constant this machine reads");
    assert!(unevaluated == before);
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .contains("source constant evaluation: `LIMIT` is used"),
        "{}",
        errors[0].message
    );
    let read = text.find("LIMIT;").expect("the read");
    let span = errors[0]
        .source_span
        .expect("the refusal points at the read");
    assert_eq!(span.span.start, read);
}
