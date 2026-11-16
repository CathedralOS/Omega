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
    // The old enum reserved wire tag zero for a source evaluation receipt.
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
    typed_base_scoped(files, None)
}

fn constant_base(
    files: &[(&str, &str)],
) -> (
    std::sync::Arc<source::SourceMap>,
    Vec<source::SourceId>,
    symbol_resolved_trees_to_typed_trees::SeededTypingBase,
    crate::PreCheckEvaluation,
) {
    typed_base_scoped(
        files,
        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
    )
}

fn typed_base_scoped(
    files: &[(&str, &str)],
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
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
        let source_id = match package {
            Some(package) => {
                sources
                    .add_with_metadata(
                        std::path::Path::new("/constant-job").join(path),
                        (*text).to_owned(),
                        "/constant-job".into(),
                        Some(package),
                        source::SourceOrigin::User,
                    )
                    .source_id
            }
            None => {
                sources
                    .add(std::path::PathBuf::from(path), (*text).to_owned())
                    .source_id
            }
        };
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
fn an_unselected_applied_collection_read_does_not_stop_the_entry() {
    let (_, _, base, pending) = constant_base(&[
        ("main.omg", "machine entry() { }"),
        (
            "library.omg",
            r#"
data Shelf<T> { stored: [T; 2]; }
machine unread(shelf: &Shelf<u64>, index: u64) -> u64
requires index < 2
{ shelf.stored[index] + 0 }
"#,
        ),
    ]);
    let mut program = base.typed().clone();
    pending.evaluate(&mut program).unwrap();
    let prepared = crate::PreparedBuildMachineProgram::prepare(&program, None).unwrap();
    let target = || crate::InvocationTarget {
        name: None,
        layout_target: None,
        carrier_layout: None,
        address_model: None,
    };
    let entry = program.realized_machine_named("entry").unwrap().symbol;
    let value = prepared
        .entry(entry, entry, &[])
        .unwrap()
        .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
        .unwrap();
    assert_eq!(value, crate::BuildTimeValue::Unit);

    let read = program.realized_machine_named("unread").unwrap().symbol;
    let shelf = crate::BuildTimeValue::Struct {
        type_name: "Shelf".into(),
        fields: vec![(
            "stored".into(),
            crate::BuildTimeValue::Array(vec![
                crate::BuildTimeValue::Int(7),
                crate::BuildTimeValue::Int(9),
            ]),
        )],
    };
    let arguments = |index| [shelf.clone(), crate::BuildTimeValue::Int(index)];
    let value = prepared
        .entry(read, read, &arguments(1))
        .unwrap()
        .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
        .unwrap();
    assert_eq!(value, crate::BuildTimeValue::Int(9));

    let error = prepared
        .entry(read, read, &arguments(2))
        .err()
        .expect("the selected body still requires its index bound");
    assert!(
        error.contains("cannot prove requires contract for call unread") && error.contains("2 < 2"),
        "{error}"
    );
}

#[test]
fn a_proof_universe_level_is_not_an_ordinary_evaluation_request() {
    use typed_trees::deferred_refusals::DeferredRequest;
    use typed_trees::mathematical::MathematicalType;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::TypeReferenceNode;

    let (_, _, base, pending) = typed_base(&[(
        "program/main.omg",
        "let small(value: u64): core::Strict<0> = embed(value) <= 100;
machine holder() { let value: u32 = 0; }
",
    )]);
    let mut typed = base.typed().clone();
    crate::PreCheckEvaluation::defer_declared_requests(&mut typed).unwrap();
    assert!(typed.deferred_refusals.is_empty());
    assert!(pending.activate_sources(&typed).is_ok());

    let predicates = typed.mathematical_definitions();
    let (1, Some(predicate)) = (predicates.len(), predicates.first()) else {
        panic!("the written predicate");
    };
    let MathematicalType::Ordinary(result) = typed.mathematical_type(predicate.result) else {
        panic!("the written sort");
    };
    let TypeReferenceNode::Generic { arguments, .. } =
        typed.type_reference_table.type_reference(*result)
    else {
        panic!("the sort applied to its level");
    };
    let [level] = typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("the one universe level");
    };
    let level = *level;
    assert!(matches!(
        typed.type_reference_table.type_reference(level),
        TypeReferenceNode::ConstExpression(_)
    ));

    assert!(
        typed_trees::references::collect_program_references(&typed)
            .type_references
            .contains(&level)
    );

    // A shared type handle does not make its other occurrence a proof
    // level. The ordinary type request still belongs to its lexical holder.
    let holder = typed.realized_machine_named("holder").unwrap();
    let local = typed.machine_states(holder)[0]
        .statement_nodes
        .handles()
        .next()
        .unwrap();
    let holder = holder.symbol;
    let StatementNode::LocalData(binding) = typed.statement_table.statement_mut(local) else {
        panic!("the written local");
    };
    binding.type_reference = level;
    assert_eq!(
        crate::source_frontier::declared_jobs(&typed).unwrap(),
        [(holder, DeferredRequest::TypeExpressionEvaluation)]
    );
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
    assert_eq!(
        crate::PreCheckEvaluation::require_recorded_requests(&typed).unwrap_err(),
        error
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
    crate::PreCheckEvaluation::require_recorded_requests(&typed).unwrap();
    assert!(pending.activate_sources(&typed).is_ok());

    // Layout evaluation must leave a different unserved request in place.
    let mut prepared = typed.clone();
    prepared.deferred_refusals.clear();
    crate::evaluate_fixed_layout_applications(
        &mut prepared,
        &[],
        None,
        None,
        source::DependencyScope::Product,
        &mut Default::default(),
    )
    .unwrap();
    assert_eq!(prepared.deferred_refusals, typed.deferred_refusals);

    // The twin: the same program as written requests nothing.
    let mut plain = base.typed().clone();
    crate::PreCheckEvaluation::defer_declared_requests(&mut plain).unwrap();
    crate::evaluate_fixed_layout_applications(
        &mut plain,
        &[],
        None,
        None,
        source::DependencyScope::Product,
        &mut Default::default(),
    )
    .unwrap();
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
fn indexed_assignment_literals_retain_the_receivers_actual_type_application() {
    // The constructor must use the declared array element type, keeping
    // Cell<u32> separate from Cell<u64>. This checks typing metadata;
    // execution and establishment remain later checks.
    let (_, _, base, _) = typed_base(&[(
        "program/main.omg",
        r#"
data Cell<T> { value: T; }
data Store { small: [Cell<u32>; 2]; large: [Cell<u64>; 2]; }
machine fill(store: &mut Store) {
    store.small[0] = Cell { value: 7 };
    store.large[1] = Cell { value: 4294967296 };
}
"#,
    )]);
    let typed = base.typed();
    let machine = typed
        .realized_machine_named("fill")
        .expect("actual source machine");
    let assignments = typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| {
            typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(move |statement| match statement {
                    typed_trees::statement::StatementNode::Assignment(assignment) => {
                        Some((state, assignment))
                    }
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(assignments.len(), 2);
    let mut receiving_types = Vec::new();
    for ((state, assignment), (field_name, primitive)) in assignments.iter().zip([
        (
            "small",
            typed_trees::types::PrimitiveType::Integer(
                numerics::integer_carrier::FixedInteger::unsigned(32),
            ),
        ),
        ("large", typed_trees::types::PrimitiveType::U64),
    ]) {
        let declared = typed.state_parameters(state)[0].type_reference;
        let carrier = typed.type_reference_table.peeled_type_reference(declared);
        let store = typed
            .closed_data_definition(carrier)
            .expect("actual Store carrier");
        let field = typed
            .data_members(store)
            .iter()
            .filter_map(typed_trees::data::DataMember::as_field)
            .find(|field| field.name.as_str() == field_name)
            .expect("actual receiving field");
        let typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: receiving,
            ..
        } = typed
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            panic!("actual array field")
        };
        let receiving = *receiving;
        receiving_types.push(receiving);
        let application = typed
            .closed_data_definition(receiving)
            .expect("actual receiving application");
        assert_eq!(application.name.as_str(), "Cell");
        let required = typed
            .expression_table
            .construction_required_type_reference(assignment.value);
        let identity = |reference| {
            typed_trees::type_identity::applied_type_identity_in_region_scope(
                typed,
                reference,
                machine.symbol,
                machine.symbol,
                &[],
                &[],
            )
            .expect("the actual application has complete identity")
        };
        assert_eq!(identity(required), identity(receiving));
        assert_eq!(
            typed.type_reference_table.operand_lifetime_owner(required),
            machine.symbol
        );
        let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
            typed.expression_table.expression(assignment.value)
        else {
            panic!("actual authored constructor")
        };
        assert_eq!(literal.application_type, required);
        let [typed_trees::data::DataMember::Field(field)] = typed.data_members(application) else {
            panic!("the exact applied field roster")
        };
        assert_eq!(
            typed.primitive_type_reference(field.type_reference),
            Some(primitive)
        );
    }
    assert_ne!(receiving_types[0], receiving_types[1]);
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
fn an_owned_selected_program_is_narrowed_without_copying_its_typed_storage() {
    // A selected-source continuation owns its typed program once its bodies
    // are typed. Narrowing that program to the selected file reuses its
    // storage; narrowing a base another retained source still shares copies
    // it once and leaves the shared base whole. Both give the same program.
    let (sources, identities, base, _) = typed_base(&[
        (
            "program/main.omg",
            "data Pair { left: u64; right: u64; }\nmachine entry() { }\n",
        ),
        ("program/library.omg", "machine unreached() { }\n"),
    ]);
    let mut selected = sources.as_ref().clone();
    selected.retain_selected_sources(|file| file.source_id == identities[0]);
    let selected = std::sync::Arc::new(selected);
    let members = &base.typed().tables.data_members;
    assert!(!members.is_empty(), "the data type stores its fields");
    let storage = members.storage_slice().as_ptr();

    let retained = base.clone();
    let copied = retained
        .clone()
        .activate_sources(
            std::sync::Arc::clone(&selected),
            std::sync::Arc::clone(&sources),
            Vec::new(),
        )
        .expect("a shared base is narrowed from its copy");
    assert_ne!(
        copied.typed().tables.data_members.storage_slice().as_ptr(),
        storage
    );
    assert_eq!(machine_names(retained.typed()), ["entry", "unreached"]);
    drop(retained);

    let narrowed = base
        .activate_sources(selected, sources, Vec::new())
        .expect("an owned base is narrowed in place");
    assert_eq!(
        narrowed
            .typed()
            .tables
            .data_members
            .storage_slice()
            .as_ptr(),
        storage,
        "the owned program keeps its typed storage"
    );
    assert_eq!(machine_names(narrowed.typed()), ["entry"]);
    assert_eq!(narrowed.typed(), copied.typed());
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

trait RequestedConstants {
    fn evaluate_requested_constants(
        self,
        program: &mut typed_trees::TypedTrees,
    ) -> Result<crate::RecordedPreCheck, Vec<diagnostics::Diagnostic>>;
}
impl RequestedConstants for crate::PreCheckEvaluation {
    fn evaluate_requested_constants(
        self,
        program: &mut typed_trees::TypedTrees,
    ) -> Result<crate::RecordedPreCheck, Vec<diagnostics::Diagnostic>> {
        let mut recorded = self.evaluate_or_defer_recorded(program)?;
        let subject = program
            .const_declarations()
            .first()
            .and_then(|constant| {
                program
                    .symbols
                    .symbol_provenance_source_span(constant.symbol)
            })
            .and_then(|span| program.symbols.source_file(span))
            .and_then(|file| file.package_identity);
        crate::evaluate_constant_initializers(
            program,
            terminal_production::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[],
                subjects: &[subject],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            None,
            &mut recorded.accounting,
        )?;
        Ok(recorded)
    }
}

#[test]
fn a_machine_call_constant_retains_its_accepted_value_and_receipt() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const SCALE: u64 = opaque();
        machine opaque() -> u64 { 4u64 }
        machine answer() -> u64 { SCALE }
    "#,
    )]);
    let mut program = base.typed().clone();
    let recorded = pending
        .evaluate_requested_constants(&mut program)
        .expect("the actual constant invocation is accepted and evaluated");
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
    assert_eq!(recorded.accounting.receipts().len(), 1);
    let crate::PureEvaluationRecord::Invocation(receipt) = &recorded.accounting.receipts()[0]
    else {
        panic!("a source job retains its actual invocation receipt")
    };
    assert_eq!(receipt.admission().target(), None);
    assert!(receipt.usage().total_units() > 0);
    assert!(matches!(
        receipt.outcome(),
        crate::PureTerminalEvaluationOutcome::Materialized {
            value: crate::BuildTimeValue::Int(4),
            ..
        }
    ));
    let answer = program.realized_machine_named("answer").unwrap().symbol;
    let entry = crate::PreparedBuildMachineProgram::prepare(&program, None)
        .unwrap()
        .entry(answer, answer, &[])
        .unwrap();
    let mut meter = terminal_work::WorkMeter::unbounded();
    let value = entry
        .invoke(
            crate::InvocationTarget {
                name: None,
                layout_target: None,
                carrier_layout: None,
                address_model: None,
            },
            &mut meter,
        )
        .unwrap();
    assert_eq!(value, crate::BuildTimeValue::Int(4));
}

#[test]
fn a_computed_sum_constant_keeps_its_exact_case_and_payload_in_the_receipt() {
    use language_semantics::const_value::{CanonicalConstIdentity, DecodedCanonicalConstValue};

    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        data Policy [copy] {
            case Read(number: u64);
            case Write(number: u64);
        }
        const selected: Policy = choose();
        machine choose() -> Policy { Policy::Read { number: 7u64 } }
        machine answer() -> Policy { selected }
    "#,
    )]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
    let handle = program.roots.const_declarations.handles().next().unwrap();
    let symbol = program.tables.const_declarations.get(handle).symbol;
    let value = typed_trees::canonical_const_declaration_value(&program, symbol)
        .unwrap()
        .decode_encoding()
        .unwrap();
    let DecodedCanonicalConstValue::Variant {
        type_name,
        case_name,
        fields,
    } = value
    else {
        panic!("the accepted result must retain its case");
    };
    assert_eq!(type_name, "Policy");
    assert_eq!(case_name, "Read");
    let [
        (
            field_name,
            DecodedCanonicalConstValue::Integer {
                integer_type,
                value,
            },
        ),
    ] = fields.as_slice()
    else {
        panic!("the selected case must retain its number");
    };
    assert_eq!(field_name, "number");
    assert_eq!(*value, numerics::bignum::BigInt::from(7u64));

    let declaration = program.tables.const_declarations.get(handle);
    let underlying = program
        .unwrapped_type_reference(declaration.declared_type)
        .unwrap();
    let carrier = program
        .type_reference_table
        .type_reference(underlying)
        .as_named_symbol()
        .unwrap();
    let data = program.data_definition_by_symbol(carrier).unwrap();
    let wrong_case = program
        .data_members(data)
        .iter()
        .filter_map(|member| member.as_variant())
        .find(|case| case.name.as_str() == "Write")
        .unwrap();
    let mut altered = program.clone();
    let typed_trees::expression::ExpressionNode::StructLiteral(materialized) = altered
        .expression_table
        .expression_mut(declaration.materialized_initializer)
    else {
        panic!("the named constant must materialize to a case literal");
    };
    materialized.case_symbol = Some(wrong_case.symbol);
    materialized.case_name = Some(wrong_case.name.clone());
    assert_eq!(
        crate::validate_constant_evaluations(&altered, &recorded.accounting).unwrap_err(),
        "constant materialization differs from its invocation outcome"
    );

    let changed_number = vec![(
        field_name.clone(),
        DecodedCanonicalConstValue::Integer {
            integer_type: *integer_type,
            value: numerics::bignum::BigInt::from(8u64),
        },
    )];
    for (case_name, fields) in [("Write".to_owned(), fields), (case_name, changed_number)] {
        let changed = DecodedCanonicalConstValue::Variant {
            type_name: type_name.clone(),
            case_name,
            fields,
        };
        let encoding = CanonicalConstIdentity::from_declaration_value(&changed)
            .unwrap()
            .encoding;
        let mut altered = program.clone();
        altered
            .tables
            .const_declarations
            .get_mut(handle)
            .evaluated_value = Some(encoding.into());
        assert_eq!(
            crate::validate_constant_evaluations(&altered, &recorded.accounting).unwrap_err(),
            "constant value differs from its accepted invocation outcome"
        );
    }
}

#[test]
fn a_constant_invocation_must_establish_the_callee_requirement() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const SCALE: u64 = bounded(9u64);
        machine bounded(value: u64) -> u64 requires value <= 5 { value }
        machine answer() -> u64 { SCALE }
    "#,
    )]);
    let mut program = base.typed().clone();
    let errors = pending
        .evaluate_requested_constants(&mut program)
        .err()
        .expect("a representable result does not establish the invocation's premise");
    let error = diagnostics::join_messages(&errors);
    assert!(
        error.contains("requires")
            || error.contains("requirement")
            || error.contains("precondition"),
        "{error}"
    );
    assert!(program.const_declarations()[0].evaluated_value.is_none());
}

#[test]
fn constant_dependencies_are_evaluated_before_their_callers() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const EXTRA: u64 = LAST + 1u64;
        const LAST: u64 = produce();
        const FIRST: u64 = seed();
        machine produce() -> u64 { FIRST + 1u64 }
        machine seed() -> u64 { 4u64 }
        machine answer() -> u64 { EXTRA }
    "#,
    )]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
    assert_eq!(recorded.accounting.receipts().len(), 3);
    let last = program
        .const_declarations()
        .iter()
        .find(|row| program.symbols.name(row.symbol) == "LAST")
        .unwrap();
    let value = typed_trees::canonical_const_declaration_value(&program, last.symbol).unwrap();
    assert!(matches!(
        value.decode_encoding(),
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. }) if value == numerics::bignum::BigInt::from(5)
    ));
    let extra = program
        .const_declarations()
        .iter()
        .find(|row| program.symbols.name(row.symbol) == "EXTRA")
        .unwrap();
    let value = typed_trees::canonical_const_declaration_value(&program, extra.symbol).unwrap();
    assert!(matches!(
        value.decode_encoding(),
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. }) if value == numerics::bignum::BigInt::from(6)
    ));
}

#[test]
fn a_computed_record_array_receipt_checks_each_named_use() {
    use typed_trees::expression::ExpressionNode;

    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        data Ticket [copy] { number: u32; }
        const TICKETS: [Ticket; 2] = make();
        machine make() -> [Ticket; 2] {
            [Ticket { number: 3u32 }, Ticket { number: 5u32 }]
        }
        machine answer() -> u32 { TICKETS[1u64].number }
    "#,
    )]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();

    let declaration = &program.const_declarations()[0];
    let crate::PureEvaluationRecord::Invocation(receipt) = &recorded.accounting.receipts()[0]
    else {
        panic!("the constant must execute its original initializer");
    };
    let original = receipt.request_source();
    let use_handle = typed_trees::references::collect_program_references(original)
        .expressions
        .into_iter()
        .find(|expression| {
            original
                .expression_table
                .name_path_node(*expression)
                .and_then(|path| path.resolved_symbol())
                == Some(declaration.symbol)
        })
        .expect("the invocation source retains its named use");
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(use_handle)
    else {
        panic!("the named use must carry the evaluated array");
    };
    let mut fields = program
        .expression_table
        .expression_handles(*elements)
        .iter()
        .map(|element| {
            let ExpressionNode::StructLiteral(record) =
                program.expression_table.expression(*element)
            else {
                panic!("the snapshot must preserve the element's record carrier");
            };
            program.expression_table.struct_fields(record.fields)[0].value
        });
    let first = fields.next().unwrap();
    let second = fields.next().unwrap();
    let wrong = program.expression_table.expression(first).clone();
    *program.expression_table.expression_mut(second) = wrong;
    assert_eq!(
        crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap_err(),
        "constant materialization differs from its invocation outcome"
    );
}

#[test]
fn a_constant_receipt_cannot_be_swapped_rewritten_or_reused_after_source_changes() {
    let text = r#"
        const FIRST: u64 = first();
        const SECOND: u64 = second();
        machine first() -> u64 { 4u64 }
        machine second() -> u64 { 5u64 }
        machine use_values() -> u64 { FIRST + SECOND }
    "#;
    let (_, _, base, pending) = constant_base(&[("main.omg", text)]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
    let handles = program
        .roots
        .const_declarations
        .handles()
        .collect::<Vec<_>>();
    let mut changed_use = program.clone();
    let first = program.const_declarations()[0].symbol;
    let crate::PureEvaluationRecord::Invocation(first_receipt) = &recorded.accounting.receipts()[0]
    else {
        panic!("actual invocation")
    };
    let original = first_receipt.request_source();
    let expression = typed_trees::references::collect_program_references(original)
        .expressions
        .into_iter()
        .find(|expression| {
            original
                .expression_table
                .name_path_node(*expression)
                .and_then(|path| path.resolved_symbol())
                == Some(first)
        })
        .expect("the source retains its named use");
    *changed_use.expression_table.expression_mut(expression) =
        typed_trees::expression::ExpressionNode::Boolean(false);
    assert!(crate::validate_constant_evaluations(&changed_use, &recorded.accounting).is_err());
    let mut changed = program.clone();
    changed
        .tables
        .const_declarations
        .get_mut(handles[0])
        .evaluated_value = Some(
        language_semantics::const_value::CanonicalConstIdentity::integer(
            numerics::literals::LandedIntegerType::U64,
            99,
        )
        .encoding
        .into(),
    );
    assert!(
        crate::validate_constant_evaluations(&changed, &recorded.accounting)
            .unwrap_err()
            .contains("outcome")
    );
    let mut swapped = program.clone();
    let other = swapped
        .tables
        .const_declarations
        .get(handles[1])
        .evaluation_entry;
    swapped
        .tables
        .const_declarations
        .get_mut(handles[0])
        .evaluation_entry = other;
    assert!(
        crate::validate_constant_evaluations(&swapped, &recorded.accounting)
            .unwrap_err()
            .contains("swapped")
    );
    assert!(
        crate::validate_constant_evaluations(
            &program,
            &crate::SourceCompletionAccounting::default()
        )
        .is_err()
    );
    let mut missing = program.clone();
    let declaration = missing.tables.const_declarations.get_mut(handles[0]);
    declaration.evaluation_entry = Default::default();
    declaration.evaluation_expression = Default::default();
    assert!(
        crate::validate_constant_evaluations(
            &missing,
            &crate::SourceCompletionAccounting::default()
        )
        .unwrap_err()
        .contains("lost its initializer")
    );
    let first = program.realized_machine_named("first").unwrap();
    let state = &program.machine_states(first)[0];
    let typed_trees::statement::StatementNode::Transition(transition) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("source returns a value")
    };
    let typed_trees::statement::TransitionTargetNode::Value(expression) =
        program.statement_table.transition_target(transition.target)
    else {
        panic!("source returns a value")
    };
    let expression = *expression;
    let replacement = program
        .tables
        .const_declarations
        .get(handles[1])
        .materialized_initializer;
    let value = program.expression_table.expression(replacement).clone();
    let mut stale = program.clone();
    *stale.expression_table.expression_mut(expression) = value;
    assert!(
        crate::validate_constant_evaluations(&stale, &recorded.accounting)
            .unwrap_err()
            .contains("stale")
    );
    let (_, _, other, pending) = constant_base(&[("main.omg", text)]);
    let mut other = other.typed().clone();
    pending.evaluate_or_defer_recorded(&mut other).unwrap();
    assert!(
        crate::validate_constant_evaluations(&other, &recorded.accounting)
            .unwrap_err()
            .contains("source owner")
    );
}

#[test]
fn constant_evaluation_keeps_the_callee_cast_location() {
    let callee = "
        domain u64::Delivered requires self >= 42u64 && self <= 42u64;
        machine make() -> u64 {
            let value: u64 in Delivered = 41u64 as +Delivered;
            value as -Delivered
        }
    ";
    let (_, ids, base, pending) = constant_base(&[
        ("main.omg", "const SAVED: u64 = make();"),
        ("callee.omg", callee),
    ]);
    let errors = pending
        .evaluate_requested_constants(&mut base.typed().clone())
        .err()
        .expect("the constant initializer calls a machine with a false cast");
    let [error] = errors.as_slice() else {
        panic!("expected the cast refusal, got {errors:?}");
    };
    assert!(
        error.message.starts_with(
            "constant `SAVED` evaluation: cannot prove the value of this cast is in the domain it adds in machine `make` state `entry` local `value`: domain `u64::Delivered` requires `self >= 42 && self <= 42`"
        ),
        "{error}"
    );
    let cast = error.source_span.expect("the refusal retains the cast");
    assert_eq!(cast.source_id, ids[1]);
    assert_eq!(cast.span.start, callee.find("+Delivered").unwrap());
}

#[test]
fn a_constant_cannot_acquire_a_target_from_an_untagged_caller() {
    let source = r#"
        const VALUE: u64 = answer();
        machine answer() -> u64 { selected() }
        linux_x86_64 machine selected() -> u64 { 7u64 }
    "#;
    let (_, ids, base, pending) = constant_base(&[("main.omg", source)]);
    let mut program = base.typed().clone();
    let errors = pending
        .evaluate_requested_constants(&mut program)
        .err()
        .expect("no caller chose this target body");
    let error = diagnostics::join_messages(&errors);
    assert!(error.contains("target"), "{error}");
    let declaration = &program.const_declarations()[0];
    let site = errors[0]
        .source_span
        .expect("the target selection refusal identifies the selected declaration");
    let callee = "selected";
    let callee_start = source
        .rfind(callee)
        .expect("fixture declares the target body");
    assert_eq!(site.source_id, ids[0]);
    assert_eq!(
        site.span.start..site.span.end,
        callee_start..callee_start + callee.len()
    );
    assert!(declaration.evaluated_value.is_none());
}

#[test]
fn constant_evaluation_obeys_a_helpers_written_service_ceiling() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        boundary trait Requests { machine observe() reaches none crashes none; }
        machine helper() -> u64 reaches Requests { 7u64 }
        const VALUE: u64 = helper();
    "#,
    )]);
    let mut program = base.typed().clone();
    let errors = pending
        .evaluate_requested_constants(&mut program)
        .err()
        .expect("a pure body does not shrink a published service ceiling");
    let error = diagnostics::join_messages(&errors);
    assert!(
        error.contains("service") || error.contains("reach"),
        "{error}"
    );
    assert!(program.const_declarations()[0].evaluated_value.is_none());
}

#[test]
fn an_unowned_constant_request_cannot_bypass_declaration_admission() {
    let (_, _, base, pending) = typed_base(&[(
        "main.omg",
        "const VALUE: u64 = answer(); machine answer() -> u64 { 7u64 }",
    )]);
    let mut program = base.typed().clone();
    let error = pending
        .evaluate_requested_constants(&mut program)
        .err()
        .expect("an unowned user source has no package execution custody");
    assert!(diagnostics::join_messages(&error).contains("without reconciled package custody"));
    assert!(program.const_declarations()[0].evaluated_value.is_none());
}

#[test]
fn constant_snapshots_preserve_boolean_and_unsigned_high_bits() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const FLAG: bool = flag();
        const HIGH: u64 = high();
        machine flag() -> bool { true }
        machine high() -> u64 { 0xFFFF_FFFF_FFFF_FFFFu64 }
    "#,
    )]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
    for constant in program.const_declarations() {
        let value = typed_trees::canonical_const_declaration_value(&program, constant.symbol)
            .unwrap()
            .decode_encoding()
            .unwrap();
        match program.symbols.name(constant.symbol) {
            "FLAG" => assert_eq!(
                value,
                language_semantics::const_value::DecodedCanonicalConstValue::Boolean(true)
            ),
            "HIGH" => assert!(
                matches!(value, language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. }
                if value == numerics::bignum::BigInt::from(u64::MAX))
            ),
            _ => panic!("only authored constants are inspected"),
        }
    }
}

#[test]
fn a_complete_subject_with_machine_constants_publishes_its_actual_module() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const FIRST: u64 = first();
        const SECOND: u64 = second();
        machine first() -> u64 { 4u64 }
        machine second() -> u64 { 5u64 }
        machine main() -> u64 { FIRST + SECOND }
    "#,
    )]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
    let main = program.realized_machine_named("main").unwrap().symbol;
    let (source, _) = typed_trees_to_typed_trees::prepare_typed_trees(
        program,
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .unwrap();
    let accepted = terminal_production::AcceptedTerminalSource::judge(
        std::sync::Arc::new(source),
        terminal_production::TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &[main],
            subjects: &[semantic_vocabulary::PackageKeyIdentity::from_digest(
                [1; 32],
            )],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap();
    let produced = accepted
        .produce(
            Default::default(),
            &mut terminal_production::UnmeasuredProduction,
        )
        .unwrap();
    assert_eq!(
        produced.view().module(),
        &accepted.closure().elaboration.module
    );
    let program = terminal_interpreter::TerminalProgram::borrow(produced.view());
    let mut instance = program
        .instantiate(terminal_interpreter::TerminalInstantiation::default())
        .unwrap();
    let mut meter = terminal_work::WorkMeter::unbounded();
    let terminal_interpreter::TerminalExecutionStatus::Complete(
        terminal_interpreter::TerminalExecutionResult::Scalar(value),
    ) = instance.invoke(&mut meter).unwrap()
    else {
        panic!("the published program returns the computed constants")
    };
    assert_eq!(
        crate::terminal_values::scalar_result(&program, value).unwrap(),
        crate::BuildTimeValue::Int(9)
    );
    let foreign =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 13)
            .unwrap();
    let symbolic = semantic_vocabulary::IntegerType::parameterized(
        semantic_vocabulary::IntegerSign::Unsigned,
        semantic_vocabulary::WidthParameter {
            owner: semantic_vocabulary::MachineId::new(1).unwrap(),
            value: semantic_vocabulary::ValueId::new(1).unwrap(),
        },
    );
    for scalar_type in [foreign, symbolic] {
        assert!(
            crate::terminal_values::scalar_argument(
                &program,
                semantic_vocabulary::ScalarType::Integer(scalar_type),
                &crate::BuildTimeValue::Int(9),
            )
            .is_err(),
            "an input type must belong to the actual published owner"
        );
        assert!(
            crate::terminal_values::scalar_result(
                &program,
                terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(9),
                }
            )
            .is_err(),
            "a returned scalar cannot acquire an unrelated or symbolic width"
        );
    }
}

#[test]
fn an_unread_machine_constant_is_pending_until_the_actual_scope_demands_it() {
    for body in [
        "machine helper() -> u64 { selected() } linux_x86_64 machine selected() -> u64 { 7u64 }",
        "boundary trait Requests { machine observe() reaches none crashes none; } machine helper() -> u64 reaches Requests { 7u64 }",
    ] {
        let text = format!(
            "const VALUE: u64 = helper(); machine main() -> u64 {{ 0u64 }} machine demanded() -> u64 {{ VALUE }} {body}"
        );
        let (_, _, base, pending) = constant_base(&[("main.omg", &text)]);
        let mut program = base.typed().clone();
        let recorded = pending.evaluate_or_defer_recorded(&mut program).unwrap();
        assert!(recorded.accounting.receipts().is_empty());
        let main = program.realized_machine_named("main").unwrap().symbol;
        let request = terminal_production::TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &[main],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        };
        let mut accounting = crate::SourceCompletionAccounting::default();
        crate::evaluate_constant_initializers(&mut program, request, None, &mut accounting)
            .unwrap();
        assert!(accounting.receipts().is_empty());
        assert!(program.const_declarations()[0].evaluated_value.is_none());
        crate::validate_constant_evaluations(&program, &accounting).unwrap();
        let demanded = program.realized_machine_named("demanded").unwrap().symbol;
        for requested in [
            terminal_production::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[demanded],
                ..request
            },
            terminal_production::TerminalSourceCompilation {
                subjects: &[semantic_vocabulary::PackageKeyIdentity::from_digest(
                    [1; 32],
                )],
                ..request
            },
        ] {
            let error = crate::evaluate_constant_initializers(
                &mut program.clone(),
                requested,
                None,
                &mut crate::SourceCompletionAccounting::default(),
            )
            .unwrap_err();
            let diagnostic = diagnostics::join_messages(&error);
            assert!(
                diagnostic.contains("target")
                    || diagnostic.contains("service")
                    || diagnostic.contains("reach"),
                "{diagnostic}"
            );
        }
    }
}

#[test]
fn a_constant_receipt_authenticates_ordered_argument_storage() {
    for (arguments, replacement, expected) in [("1u64", "2u64", 1i64), ("1u64, 2u64", "2u64", 1i64)]
    {
        let text = if arguments.contains(',') {
            format!(
                "const VALUE:u64=f(); machine f()->u64 {{ g({arguments}) }} machine g(a:u64,b:u64)->u64 {{ a }} machine spare()->u64 {{ {replacement} }}"
            )
        } else {
            format!(
                "const VALUE:u64=f(); machine f()->u64 {{ g({arguments}) }} machine g(a:u64)->u64 {{ a }} machine spare()->u64 {{ {replacement} }}"
            )
        };
        let (_, _, base, pending) = constant_base(&[("main.omg", &text)]);
        let mut program = base.typed().clone();
        let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
        crate::validate_constant_evaluations(&program, &recorded.accounting).unwrap();
        let crate::PureEvaluationRecord::Invocation(receipt) = &recorded.accounting.receipts()[0]
        else {
            panic!("actual invocation")
        };
        assert!(
            matches!(receipt.outcome(), crate::PureTerminalEvaluationOutcome::Materialized { value: crate::BuildTimeValue::Int(value), .. } if *value==expected)
        );
        let g = program.realized_machine_named("g").unwrap().symbol;
        let references = typed_trees::references::collect_program_references(&program);
        let (expression, call) = references
            .expressions
            .iter()
            .find_map(|expression| {
                let typed_trees::expression::ExpressionNode::Call(call) =
                    program.expression_table.expression(*expression)
                else {
                    return None;
                };
                program
                    .machine_by_symbol(call.target_symbol)
                    .or_else(|| program.machine_holding_state(call.target_symbol))
                    .is_some_and(|machine| machine.symbol == g)
                    .then(|| (*expression, call.clone()))
            })
            .expect("the actual call to g");
        let old = program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let two = references.expressions.iter().find(|expression| {
            matches!(program.expression_table.expression(**expression), typed_trees::expression::ExpressionNode::Integer(literal) if literal.value_u64()==Some(2))
        }).copied().unwrap();
        program
            .expression_table
            .set_expression_handle_at_offset(call.arguments, 0, two);
        if old.len() == 2 {
            program
                .expression_table
                .set_expression_handle_at_offset(call.arguments, 1, old[0]);
        }
        assert_eq!(
            program.expression_table.expression(expression),
            &typed_trees::expression::ExpressionNode::Call(call)
        );
        assert!(
            crate::validate_constant_evaluations(&program, &recorded.accounting)
                .unwrap_err()
                .contains("input storage")
        );
    }
}

#[test]
fn a_later_constant_admission_failure_keeps_the_work_of_earlier_jobs() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const FIRST:u64 = first();
        const SECOND:u64 = bounded(9u64);
        machine first()->u64 { 4u64 }
        machine bounded(value:u64)->u64 requires value <= 5 { value }
    "#,
    )]);
    let mut program = base.typed().clone();
    pending.evaluate_or_defer_recorded(&mut program).unwrap();
    let mut accounting = crate::SourceCompletionAccounting::default();
    let error = crate::evaluate_constant_initializers(
        &mut program,
        terminal_production::TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &[],
            subjects: &[semantic_vocabulary::PackageKeyIdentity::from_digest(
                [1; 32],
            )],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        None,
        &mut accounting,
    )
    .unwrap_err();
    assert!(diagnostics::join_messages(&error).contains("requires"));
    assert_eq!(accounting.receipts().len(), 1);
    let crate::PureEvaluationRecord::Invocation(receipt) = &accounting.receipts()[0] else {
        panic!("actual invocation")
    };
    assert!(receipt.usage().total_units() > 0);
    assert!(matches!(
        receipt.outcome(),
        crate::PureTerminalEvaluationOutcome::Materialized {
            value: crate::BuildTimeValue::Int(4),
            ..
        }
    ));
    accounting.validate_charges().unwrap();
    assert!(program.const_declarations()[0].evaluated_value.is_some());
    assert!(program.const_declarations()[1].evaluated_value.is_none());
}

#[test]
fn an_exhausted_constant_job_keeps_its_failed_invocation_usage() {
    let mut text = String::from("machine leaf() {}\n");
    let mut previous = String::from("leaf");
    for level in 0..18 {
        let name = format!("branch_{level}");
        text.push_str(&format!(
            "machine {name}() {{ {previous}(); {previous}(); }}\n"
        ));
        previous = name;
    }
    text.push_str(&format!(
        "machine answer()->u64 {{ {previous}(); 4u64 }} const VALUE:u64=answer();"
    ));
    let (_, _, base, pending) = constant_base(&[("main.omg", &text)]);
    let mut program = base.typed().clone();
    pending.evaluate_or_defer_recorded(&mut program).unwrap();
    let mut accounting = crate::SourceCompletionAccounting::default();
    let error = crate::evaluate_constant_initializers(
        &mut program,
        terminal_production::TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &[],
            subjects: &[semantic_vocabulary::PackageKeyIdentity::from_digest(
                [1; 32],
            )],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        None,
        &mut accounting,
    )
    .unwrap_err();
    assert_eq!(
        accounting.receipts().len(),
        1,
        "{}",
        diagnostics::join_messages(&error)
    );
    let crate::PureEvaluationRecord::Invocation(receipt) = &accounting.receipts()[0] else {
        panic!("actual invocation")
    };
    assert_eq!(
        receipt.usage().total_units(),
        crate::evaluation_accounting::PURE_PSI_WORK_BUDGET,
    );
    let crate::PureTerminalEvaluationOutcome::Failed { reason } = receipt.outcome() else {
        panic!("exhausted invocation returned a value")
    };
    assert!(reason.contains("BudgetExhausted(Work)"), "{reason}");
    accounting.validate_charges().unwrap();
    assert!(program.const_declarations()[0].evaluated_value.is_none());
}

#[test]
fn shared_account_joins_retain_one_record_per_actual_invocation() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        "const VALUE:u64=answer(); machine answer()->u64 { 4u64 }",
    )]);
    let mut program = base.typed().clone();
    let recorded = pending.evaluate_requested_constants(&mut program).unwrap();
    let mut accounting = recorded.accounting.clone();
    accounting.append(recorded.accounting.clone());
    assert_eq!(accounting.receipts().len(), 1);
    let requester = program.const_declarations()[0].symbol;
    let answer = program.realized_machine_named("answer").unwrap().symbol;
    let prepared = crate::PreparedBuildMachineProgram::prepare(&program, None).unwrap();
    let entry = prepared.entry(requester, answer, &[]).unwrap();
    let mut separate = crate::SourceCompletionAccounting::default();
    for _ in 0..2 {
        entry
            .invoke_recorded(
                crate::InvocationTarget {
                    name: None,
                    layout_target: None,
                    carrier_layout: None,
                    address_model: None,
                },
                &mut separate,
            )
            .unwrap();
    }
    assert_eq!(
        separate.receipts().len(),
        2,
        "equal invocations are genuinely separate executions"
    );
    let mut projected = recorded.accounting.clone();
    projected.append(separate);
    accounting.append(projected.clone());
    accounting.append(projected.clone());
    assert_eq!(accounting.receipts().len(), 3);
    for (actual, expected) in accounting.receipts().iter().zip(projected.receipts()) {
        let (
            crate::PureEvaluationRecord::Invocation(actual),
            crate::PureEvaluationRecord::Invocation(expected),
        ) = (actual, expected)
        else {
            panic!("actual invocations")
        };
        assert!(
            std::sync::Arc::ptr_eq(actual, expected),
            "joins preserve the incoming order after retained occurrences"
        );
    }
    accounting.validate_charges().unwrap();
}

/// A program whose data type `Storage` holds a `Buffer<{argument}>`, with
/// `leaf` returning `{leaf}`. `outer` reads `leaf` through a type applied at
/// `leaf()`, so evaluating `outer()` waits for that application. `demanded`
/// names `Storage`; `main` names none of it.
fn const_argument_program(declarations: &str, leaf: &str, argument: &str) -> String {
    format!(
        "data Buffer<const Capacity: u64> {{ slots: [u8; Capacity]; }}
        data Storage {{ buffer: Buffer<{argument}>; }}
        {declarations}
        machine leaf() -> u64 {{ {leaf} }}
        machine outer() -> u64 {{ let buffer: Buffer<leaf()>; leaf() }}
        machine demanded(storage: &Storage) -> u64 {{ 0u64 }}
        machine main() -> u64 {{ 0u64 }}"
    )
}

/// Types `text`, records its evaluation requests on their declarations, and
/// evaluates the const arguments the machine `entry` demands.
fn evaluate_const_arguments_of(
    text: &str,
    entry: &str,
) -> (
    typed_trees::TypedTrees,
    crate::SourceCompletionAccounting,
    Result<(), Vec<diagnostics::Diagnostic>>,
) {
    let (_, _, base, pending) = constant_base(&[("main.omg", text)]);
    let mut program = base.typed().clone();
    pending.evaluate_or_defer_recorded(&mut program).unwrap();
    let entry = program.realized_machine_named(entry).unwrap().symbol;
    let mut accounting = crate::SourceCompletionAccounting::default();
    let result = crate::evaluate_const_arguments(
        &mut program,
        terminal_production::TerminalSourceCompilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        None,
        None,
        &mut accounting,
    );
    (program, accounting, result)
}

/// The applications of `Buffer` written in the source, each with its one
/// argument.
fn buffer_applications(
    program: &typed_trees::TypedTrees,
) -> Vec<(
    typed_trees::types::TypeReferenceHandle,
    typed_trees::types::TypeReferenceHandle,
)> {
    use typed_trees::types::TypeReferenceNode;
    let buffer = program
        .data_definition_by_name_str("Buffer")
        .expect("the Buffer declaration")
        .symbol;
    program
        .type_reference_table
        .type_reference_rows()
        .filter_map(|(reference, node)| match node {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if *base_symbol == buffer => Some((
                reference,
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)[0],
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn a_const_argument_written_as_a_call_becomes_the_literal_of_its_value() {
    use typed_trees::data::DataMember;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    let text = const_argument_program("", "4u64", "outer()");
    let (program, accounting, result) = evaluate_const_arguments_of(&text, "demanded");
    result.unwrap();
    // One invocation per argument: `leaf()` for the type in `outer`, then
    // `outer()`, which reads that type.
    assert_eq!(accounting.receipts().len(), 2);
    accounting.validate_charges().unwrap();
    let applications = buffer_applications(&program);
    assert_eq!(applications.len(), 2);
    for (application, argument) in applications {
        // The argument is the node typing writes for the literal `4`.
        assert!(
            matches!(
                program.type_reference_table.type_reference(argument),
                TypeReferenceNode::Named { symbol, name }
                    if !symbol.is_valid() && name.as_str() == "4"
            ),
            "{:?}",
            program.type_reference_table.type_reference(argument)
        );
        // And the applied type is closed, with an array of four bytes.
        let view = program
            .closed_data_definition(application)
            .expect("a closed application");
        let [DataMember::Field(slots)] = program.data_members(view) else {
            panic!("the one field")
        };
        assert!(matches!(
            program
                .type_reference_table
                .type_reference(slots.type_reference),
            TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(4),
                ..
            }
        ));
    }
    // Neither holder is refused any more for an unevaluated type.
    for holder in [
        program
            .data_definition_by_name_str("Storage")
            .unwrap()
            .symbol,
        program.realized_machine_named("outer").unwrap().symbol,
    ] {
        assert!(program.deferred_refusal(holder).is_none());
    }
}

#[test]
fn a_closed_array_extent_uses_its_checked_value_when_demanded() {
    use typed_trees::data::DataMember;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    let source = "machine extent() -> u64 { 4u64 }
        data Packet { bytes: [u8; extent()]; }
        machine reader(packet: &Packet) -> u64 { 0u64 }
        machine unrelated() -> u64 { 0u64 }";
    for (entry, expected_calls) in [("reader", 1), ("unrelated", 0)] {
        let (program, accounting, result) = evaluate_const_arguments_of(source, entry);
        result.unwrap();
        assert_eq!(accounting.receipts().len(), expected_calls);
        accounting.validate_charges().unwrap();
        let packet = program.data_definition_by_name_str("Packet").unwrap();
        let [DataMember::Field(bytes)] = program.data_members(packet) else {
            panic!("the original array field");
        };
        let node = program
            .type_reference_table
            .type_reference(bytes.type_reference);
        if expected_calls == 1 {
            assert!(
                matches!(
                    node,
                    TypeReferenceNode::FixedArray {
                        length: FixedArrayLength::Literal(4),
                        ..
                    }
                ),
                "{node:?}"
            );
            assert!(program.deferred_refusal(packet.symbol).is_none());
        } else {
            assert!(
                matches!(
                    node,
                    TypeReferenceNode::FixedArray {
                        length: FixedArrayLength::Expression(_),
                        ..
                    }
                ),
                "{node:?}"
            );
            assert!(program.deferred_refusal(packet.symbol).is_some());
        }
    }
}

#[test]
fn an_evaluated_array_extent_must_be_a_nonnegative_length() {
    let source = "machine extent() -> i64 { 0i64 - 1i64 }
        data Packet { bytes: [u8; extent()]; }
        machine reader(packet: &Packet) -> u64 { 0u64 }";
    let (_, accounting, result) = evaluate_const_arguments_of(source, "reader");
    let errors = result.expect_err("the evaluated extent is negative");
    assert_eq!(accounting.receipts().len(), 1);
    assert!(
        errors.iter().any(|error| {
            error.message.contains("computed array extent")
                && error.message.contains("not a closed nonnegative integer")
        }),
        "{errors:?}"
    );
}

#[test]
fn a_const_argument_no_demanded_declaration_holds_is_left_as_written() {
    use typed_trees::types::TypeReferenceNode;
    let text = const_argument_program("", "4u64", "outer()");
    let (program, accounting, result) = evaluate_const_arguments_of(&text, "main");
    result.unwrap();
    assert!(accounting.receipts().is_empty());
    for (_, argument) in buffer_applications(&program) {
        assert!(matches!(
            program.type_reference_table.type_reference(argument),
            TypeReferenceNode::ConstExpression(_)
        ));
    }
    // Its holder keeps the refusal a reader raises if it is demanded later.
    let storage = program
        .data_definition_by_name_str("Storage")
        .unwrap()
        .symbol;
    assert!(program.deferred_refusal(storage).is_some());
}

#[test]
fn a_const_argument_call_that_cannot_be_an_argument_is_an_error_at_the_call() {
    for (declarations, argument, expected) in [
        // A negative value is no array length.
        (
            "data Signed<const Capacity: i64> { slots: [u8; Capacity]; }
             data Below { buffer: Signed<below()>; }
             machine below() -> i64 { 0i64 - 1i64 }
             machine reads(below: &Below) -> u64 { 0u64 }",
            "outer()",
            &["an applied array length is not a closed nonnegative integer"][..],
        ),
        // A machine that reaches a runtime service has no compile-time value.
        (
            "boundary trait Clock { machine now() -> u64 reaches none crashes none; }
             machine observed() -> u64 reaches Clock { Clock::now() }
             machine reads(storage: &Storage) -> u64 { 0u64 }",
            "observed()",
            &["target", "service", "reach"][..],
        ),
        // A value computed through the type it is an argument of.
        (
            "machine own() -> u64 { let buffer: Buffer<own()>; 4u64 }
             machine reads(storage: &Storage) -> u64 { own() }",
            "outer()",
            &["has no first step"][..],
        ),
    ] {
        let text = const_argument_program(declarations, "4u64", argument);
        let (_, _, result) = evaluate_const_arguments_of(&text, "reads");
        let diagnostic = diagnostics::join_messages(&result.unwrap_err());
        assert!(
            diagnostic.contains("const argument for `Capacity` of")
                && expected.iter().any(|part| diagnostic.contains(part)),
            "{diagnostic}"
        );
    }
}

#[test]
fn equal_const_indices_keep_the_same_record_type_through_build_invocation() {
    use crate::BuildTimeValue;
    use typed_trees::data::DataMember;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    for (declarations, argument, receipts) in [
        ("", "4", 0),
        ("", "(2 + 2)", 0),
        ("const CAPACITY: u64 = 4u64;", "CAPACITY", 0),
        ("", "outer()", 2),
    ] {
        let text = const_argument_program(declarations, "4u64", argument);
        let (library, main) = text
            .rsplit_once("machine main()")
            .expect("the original main and library declarations");
        let main = format!("machine main(){main}");
        let (_, _, base, pending) = constant_base(&[("main.omg", &main), ("library.omg", library)]);
        for (root, expected_receipts) in [("main", 0), ("demanded", receipts)] {
            let mut program = base.typed().clone();
            pending
                .clone()
                .evaluate_or_defer_recorded(&mut program)
                .unwrap();
            let entry = program.realized_machine_named(root).unwrap().symbol;
            let request = terminal_production::TerminalSourceCompilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            };
            let mut accounting = crate::SourceCompletionAccounting::default();
            crate::evaluate_const_arguments(&mut program, request, None, None, &mut accounting)
                .unwrap_or_else(|error| panic!("{argument}, {root}: {error:?}"));
            crate::evaluate_constant_initializers(&mut program, request, None, &mut accounting)
                .unwrap();
            assert_eq!(
                accounting.receipts().len(),
                expected_receipts,
                "{argument}, {root}"
            );
            accounting.validate_charges().unwrap();
            let prepared = crate::PreparedBuildMachineProgram::prepare(&program, None).unwrap();
            let invoke = |arguments: &[BuildTimeValue]| {
                prepared
                    .entry(entry, entry, arguments)
                    .and_then(|invocation| {
                        invocation.invoke(
                            crate::InvocationTarget {
                                name: None,
                                layout_target: None,
                                carrier_layout: None,
                                address_model: None,
                            },
                            &mut terminal_work::WorkMeter::unbounded(),
                        )
                    })
            };
            if root == "main" {
                assert_eq!(invoke(&[]), Ok(BuildTimeValue::Int(0)), "{argument}");
                continue;
            }

            let storage = program.data_definition_by_name_str("Storage").unwrap();
            let [DataMember::Field(buffer)] = program.data_members(storage) else {
                panic!("Storage retains its single Buffer field");
            };
            let TypeReferenceNode::Generic { arguments, .. } = program
                .type_reference_table
                .type_reference(buffer.type_reference)
            else {
                panic!("Storage retains its applied Buffer field");
            };
            let [index] = program
                .type_reference_table
                .type_reference_handles(*arguments)
            else {
                panic!("Buffer retains its single Capacity argument");
            };
            assert!(
                matches!(
                    program.type_reference_table.type_reference(*index),
                    TypeReferenceNode::Named { symbol, name }
                        if !symbol.is_valid() && name.as_str() == "4"
                ),
                "{argument}"
            );
            if argument == "CAPACITY" {
                let constants = program.const_declarations();
                let (1, Some(constant)) = (constants.len(), constants.first()) else {
                    panic!("one selected const declaration");
                };
                let origins = program
                    .type_reference_table
                    .constant_origins(*index)
                    .unwrap();
                assert!(
                    origins
                        .iter()
                        .any(|origin| origin.declaration == constant.symbol)
                );
            }
            let view = program
                .closed_data_definition(buffer.type_reference)
                .expect("the record argument has an exact closed application");
            let [DataMember::Field(slots)] = program.data_members(view) else {
                panic!("Buffer retains its single array field");
            };
            assert!(matches!(
                program
                    .type_reference_table
                    .type_reference(slots.type_reference),
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(4),
                    ..
                }
            ));

            let input = |buffer: &str, length| BuildTimeValue::Struct {
                type_name: "Storage".into(),
                fields: vec![(
                    "buffer".into(),
                    BuildTimeValue::Struct {
                        type_name: buffer.into(),
                        fields: vec![(
                            "slots".into(),
                            BuildTimeValue::Array(vec![BuildTimeValue::Int(0); length]),
                        )],
                    },
                )],
            };
            assert_eq!(
                invoke(&[input("Buffer", 4)]),
                Ok(BuildTimeValue::Int(0)),
                "{argument}"
            );
            assert_eq!(
                invoke(&[input("Buffer", 3)]).unwrap_err(),
                "constant argument `storage`: array argument differs from its declared extent",
                "{argument}"
            );
            assert_eq!(
                invoke(&[input("OtherBuffer", 4)]).unwrap_err(),
                "constant argument `storage`: invocation record names another declaration",
                "{argument}"
            );
        }
    }
}

#[test]
fn deriving_numbered_record_metadata_does_not_request_wire_evaluation() {
    let (_, _, base, pending) = constant_base(&[
        ("main.omg", "machine entry() { }"),
        (
            "library.omg",
            r#"
data Numbered { #0 count: u64; }
machine read() -> u64 {
    let item: Numbered = Numbered { count: 9u64 };
    item.count
}
"#,
        ),
    ]);
    let mut program = base.typed().clone();
    let schemas = program.wire_schemas();
    let (1, Some(schema)) = (schemas.len(), schemas.first()) else {
        panic!("ordinary numbered data derives its schema");
    };
    assert!(schema.encoding.is_none());
    let schema = schema.symbol;
    pending.evaluate(&mut program).unwrap();
    crate::PreCheckEvaluation::evaluate_extension_or_defer(Vec::new(), 0, &mut program).unwrap();
    let prepared = crate::PreparedBuildMachineProgram::prepare(&program, None).unwrap();
    let target = || crate::InvocationTarget {
        name: None,
        layout_target: None,
        carrier_layout: None,
        address_model: None,
    };
    for (name, expected) in [
        ("entry", crate::BuildTimeValue::Unit),
        ("read", crate::BuildTimeValue::Int(9)),
    ] {
        let machine = program.realized_machine_named(name).unwrap().symbol;
        let actual = prepared
            .entry(machine, machine, &[])
            .unwrap()
            .invoke(target(), &mut terminal_work::WorkMeter::unbounded())
            .unwrap();
        assert_eq!(actual, expected);
    }

    program.record_wire_schema_plan(schema, [], [], false);
    let error = crate::PreCheckEvaluation::defer_declared_requests(&mut program).unwrap_err();
    assert_eq!(
        error[0].message,
        "no product route yet: source wire-schema evaluation"
    );
}
