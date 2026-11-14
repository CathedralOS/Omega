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
            psi_terminal_production::TerminalSourceCompilation {
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
    let mut meter = psi_work::WorkMeter::unbounded();
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
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value: 5, .. })
    ));
    let extra = program
        .const_declarations()
        .iter()
        .find(|row| program.symbols.name(row.symbol) == "EXTRA")
        .unwrap();
    let value = typed_trees::canonical_const_declaration_value(&program, extra.symbol).unwrap();
    assert!(matches!(
        value.decode_encoding(),
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value: 6, .. })
    ));
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
        language_semantics::const_value::CanonicalConstIdentity::integer("u64", 99)
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
fn a_constant_cannot_acquire_a_target_from_an_untagged_caller() {
    let (_, _, base, pending) = constant_base(&[(
        "main.omg",
        r#"
        const VALUE: u64 = answer();
        machine answer() -> u64 { selected() }
        linux_x86_64 machine selected() -> u64 { 7u64 }
    "#,
    )]);
    let mut program = base.typed().clone();
    let errors = pending
        .evaluate_requested_constants(&mut program)
        .err()
        .expect("no caller chose this target body");
    let error = diagnostics::join_messages(&errors);
    assert!(error.contains("target"), "{error}");
    assert!(program.const_declarations()[0].evaluated_value.is_none());
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
                if value == i128::from(u64::MAX))
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
    let accepted = psi_terminal_production::AcceptedTerminalSource::judge(
        std::sync::Arc::new(source),
        psi_terminal_production::TerminalSourceCompilation {
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
            &mut psi_terminal_production::UnmeasuredProduction,
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
    let mut meter = psi_work::WorkMeter::unbounded();
    let terminal_interpreter::TerminalExecutionStatus::Complete(
        terminal_interpreter::TerminalExecutionResult::Scalar(value),
    ) = instance.invoke(&mut meter).unwrap()
    else {
        panic!("the published program returns the computed constants")
    };
    assert_eq!(
        crate::terminal_values::scalar_result(value).unwrap(),
        crate::BuildTimeValue::Int(9)
    );
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
        let request = psi_terminal_production::TerminalSourceCompilation {
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
            psi_terminal_production::TerminalSourceCompilation {
                entries: &[demanded],
                ..request
            },
            psi_terminal_production::TerminalSourceCompilation {
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
        psi_terminal_production::TerminalSourceCompilation {
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
        psi_terminal_production::TerminalSourceCompilation {
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
    assert!(receipt.usage().total_units() > 0);
    assert!(matches!(
        receipt.outcome(),
        crate::PureTerminalEvaluationOutcome::Failed { .. }
    ));
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
