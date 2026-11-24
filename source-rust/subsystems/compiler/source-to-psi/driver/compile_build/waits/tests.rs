use super::*;
use syntax_trees_to_symbol_resolved_trees::resolution::{ResolutionRequest, resolve};

const SOURCE: SourceId = SourceId(0);
const PROGRAM: &str = r#"
use kit;
data Build {}
machine Build::prepare_dependency(builder: @mutable Build) suspends {}
machine build(builder: @mutable Build) suspends {
    let before: u32 = 1;
    suspend Build::prepare_dependency(builder);
    let seven: u32 = TEST();
    let after: u32 = before;
}
"#;

fn select(
    text: &str,
    imports: &[PendingBuildImport],
) -> (
    SyntaxTrees,
    EntryPlan,
    Vec<crate::source_state::ParkedBuildLocals>,
) {
    select_after(text, imports, None)
}

fn select_after(
    text: &str,
    imports: &[PendingBuildImport],
    previous: Option<&BuildStage>,
) -> (
    SyntaxTrees,
    EntryPlan,
    Vec<crate::source_state::ParkedBuildLocals>,
) {
    let tokens = crate::test_support::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    let mut syntax =
        crate::test_support::frontend_fixture::parse_syntax_trees_with_id(SOURCE, &tokens).unwrap();
    let start = text.find("machine build").unwrap() + "machine ".len();
    let entry = SourceSpan::new(SOURCE, Span::new(start, start + 5));
    let mut plan = EntryPlan::new(&mut syntax, entry, previous, !imports.is_empty()).unwrap();
    let resolved = resolve_program(syntax.clone(), text);
    let parked = plan.select(&resolved, imports).unwrap();
    plan.project(&mut syntax);
    (syntax, plan, parked)
}

fn resolve_program(
    syntax: SyntaxTrees,
    text: &str,
) -> symbol_resolved_trees::source_resolved::SymbolResolvedTrees {
    let mut sources = crate::source_inputs::SourceMap::default();
    assert_eq!(
        sources
            .add_with_metadata(
                "app/build.omg".into(),
                text.into(),
                "app".into(),
                None,
                source_admission::SourceOrigin::User
            )
            .source_id,
        SOURCE
    );
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .unwrap()
}

#[test]
fn selected_wait_keeps_the_original_declaration_needed_by_its_suffix() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (syntax, plan, parked) = select(&PROGRAM.replace("TEST", r"kit\seven"), &imports);
    let site = plan.stage.outgoing_parked_preparation().unwrap();
    assert_eq!(&PROGRAM[site.start..site.end], "prepare_dependency");
    assert_eq!(
        syntax
            .items
            .state(plan.calls[plan.truncate[0]].state)
            .statements
            .len(),
        2
    );
    let [parked] = parked.as_slice() else {
        panic!("one projected declaration")
    };
    assert_eq!(parked.declarations.len(), 1);
    let span = parked.declarations[0];
    assert_eq!(&PROGRAM[span.start..span.end], "before");
}

#[test]
fn unrelated_missing_names_do_not_select_a_preparation() {
    let pending = PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    };
    for (name, imports) in [
        (r"kitchen\seven", vec![pending.clone()]),
        ("typo", vec![pending]),
        (
            r"kit\seven",
            vec![PendingBuildImport {
                source: SourceId(9),
                span: Span::default(),
                target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
            }],
        ),
    ] {
        let (_, plan, parked) = select(&PROGRAM.replace("TEST", name), &imports);
        assert_eq!(plan.stage.outgoing_parked_preparation(), None);
        assert!(parked.is_empty());
        assert!(plan.truncate.is_empty());
    }
}

#[test]
fn a_later_call_demands_its_helpers_import_in_the_callers_stage() {
    let program = PROGRAM.replace("TEST", "consume")
        + r"
machine consume() -> u32 { kit\seven() }";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, parked) = select(&program, &imports);
    assert!(plan.stage.outgoing_parked_preparation().is_some());
    assert_eq!(parked[0].declarations.len(), 1);
    let [delayed] = parked[0].deferred_subjects.as_slice() else {
        panic!("only the called machine needs the unavailable package");
    };
    assert_eq!(&program[delayed.span.start..delayed.span.end], "consume");
}

#[test]
fn only_a_file_qualifier_requests_a_pending_import_named_like_a_local_value() {
    let program = PROGRAM
        .replace("use kit;", "use {kit}/tools.omg as kit;")
        .replace("    let seven", "    let kit: u32 = 1;\n    let seven");
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::File {
            root: Some("kit".into()),
            path: "tools.omg".into(),
            alias: "kit".into(),
        },
    }];
    // Preparing the package can supply its machine. It cannot supply a method on the local value.
    for (call, waits) in [("kit.seven", false), ("|kit|::seven", true)] {
        let (_, plan, parked) = select(&program.replace("TEST", call), &imports);
        assert_eq!(
            plan.stage.outgoing_parked_preparation().is_some(),
            waits,
            "{call}"
        );
        assert_eq!(!parked.is_empty(), waits, "{call}");
    }
}

#[test]
fn a_helper_preparation_keeps_its_actual_caller_and_return_body() {
    let program = PROGRAM
        .replace(
            "    suspend Build::prepare_dependency(builder);",
            "    suspend ready(builder);",
        )
        .replace("TEST", r"kit\seven")
        + "\nmachine ready(builder: @mutable Build) suspends { suspend Build::prepare_dependency(builder); }";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, parked) = select(&program, &imports);
    let path = plan.stage.outgoing_preparation_path().unwrap();
    assert_eq!(
        path.iter()
            .map(|site| &program[site.call.start..site.call.end])
            .collect::<Vec<_>>(),
        ["ready", "prepare_dependency"]
    );
    assert_eq!(parked.len(), 1);
    let span = parked[0].declarations[0];
    assert_eq!(&program[span.start..span.end], "before");
}

#[test]
fn a_wait_inside_the_helper_keeps_the_helpers_post_wait_local() {
    let program = PROGRAM
        .replace(
            "    suspend Build::prepare_dependency(builder);",
            "    suspend ready(builder);",
        )
        .replace("TEST()", "7")
        + r"
machine ready(builder: @mutable Build) suspends {
    let held: u32 = 1;
    suspend Build::prepare_dependency(builder);
    let next: u32 = kit\seven();
    let after: u32 = held;
}";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, parked) = select(&program, &imports);
    assert_eq!(plan.stage.outgoing_preparation_path().unwrap().len(), 2);
    let [helper] = parked.as_slice() else {
        panic!("only the helper reads a pending import");
    };
    assert_eq!(
        &program[helper.entry.span.start..helper.entry.span.end],
        "ready"
    );
    assert_eq!(helper.declarations.len(), 1);
    let span = helper.declarations[0];
    assert_eq!(&program[span.start..span.end], "held");
}

#[test]
fn a_wait_matches_the_resolved_target_with_its_static_qualifier() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, _) = select(&PROGRAM.replace("TEST()", "7"), &imports);
    let start = PROGRAM.find("suspend Build").unwrap() + "suspend ".len();
    let end = start + "Build::prepare_dependency".len();
    let selected = plan.call_at(plan.entry, Span::new(start, end)).unwrap();
    assert!(plan.calls[selected].prepares);
}

#[test]
fn a_helper_unreachable_from_build_does_not_request_a_dependency() {
    let program = PROGRAM.replace("TEST()", "7")
        + r"
machine unused() -> u32 { kit\seven() }";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, parked) = select(&program, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_none());
    assert!(parked.is_empty());
}

#[test]
fn missing_imports_point_to_each_authored_occurrence() {
    let program = crate::source_state::TypedTrees::default();
    let import = |source| PendingBuildImport {
        source,
        span: Span::new(4, 7),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    };
    let imports = [import(SOURCE), import(SourceId(8))];
    let (_, finished, _) = select(&PROGRAM.replace("TEST()", "7"), &imports);
    let errors = finished
        .stage
        .check_imports(&program, &imports)
        .unwrap_err();
    assert_eq!(errors.len(), imports.len());
    for (error, import) in errors.iter().zip(&imports) {
        assert_eq!(error.severity, diagnostics::DiagnosticSeverity::Error);
        assert_eq!(
            error.source_span,
            Some(SourceSpan::new(import.source, import.span))
        );
        assert_eq!(error.message, "Build import `kit` has no supplied source");
    }
}

#[test]
fn a_stage_keeps_its_borrowed_formal_for_the_mounted_call() {
    let program = PROGRAM.replace("TEST()", r"kit\seven(builder)");
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, _, parked) = select(&program, &imports);
    let [entry] = parked.as_slice() else {
        panic!("one projected entry")
    };
    let names: Vec<_> = entry
        .declarations
        .iter()
        .map(|span| &program[span.start..span.end])
        .collect();
    assert_eq!(names, ["builder", "before"]);
}

#[test]
fn a_conditional_preparation_retains_both_caller_arms() {
    let program = r"
use kit;
data Build {}
machine Build::prepare_dependency(builder: @mutable Build) suspends {}
machine build(builder: @mutable Build, choosing: bool) suspends {
    transition choosing {
        true -> suspend ready(builder),
        _ -> skipped()
    }
    machine skipped() {}
    machine ready(builder: @mutable Build) suspends {
        let saved: u32 = 3;
        suspend Build::prepare_dependency(builder);
        let value: u32 = kit\seven();
        let seen: u32 = saved;
    }
}";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (syntax, plan, parked) = select(program, &imports);
    let [caller, leaf] = plan.stage.outgoing_preparation_path().unwrap() else {
        panic!("one conditional call and one wait")
    };
    assert_eq!(&program[caller.call.start..caller.call.end], "ready");
    assert!(!caller.projects_suffix);
    assert!(leaf.projects_suffix);
    assert!(plan.stage.can_finish_before_preparation());
    let entry = syntax
        .root_items()
        .find_map(|item| {
            item.as_machine()
                .filter(|machine| machine.name.as_str(&syntax) == "build")
        })
        .unwrap();
    let state = syntax
        .items
        .state(syntax.items.state_handles(entry.states)[0]);
    let choices = syntax
        .items
        .statements(state.statements)
        .iter()
        .filter_map(|&statement| match syntax.statements.statement(statement) {
            StatementNode::Transition(arm) => Some(arm),
            _ => None,
        })
        .map(|arm| {
            let syntax_trees::statement::TransitionTargetNode::Named(expression) =
                syntax.statements.transition_target(arm.target)
            else {
                panic!("authored call")
            };
            syntax.expressions.call(*expression).target.as_str(&syntax)
        })
        .collect::<Vec<_>>();
    assert_eq!(choices, ["ready", "skipped"]);
    assert_eq!(parked.len(), 1);
    let only = &plan.calls[plan.truncate[0]];
    assert_eq!(
        &program[only.owner.span.start..only.owner.span.end],
        "ready"
    );
}

#[test]
fn a_generated_entry_query_in_a_bind_operand_requests_its_import() {
    let program = PROGRAM.replace(
        "let seven: u32 = TEST();",
        r#"Build::bind(builder, linux_x86_64\ProgramEntry, kit\select(builder, "ProgramEntry"));"#,
    );
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, _) = select(&program, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_some());
}

#[test]
fn a_missing_type_in_the_suffix_is_a_dependency_use() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let program = PROGRAM.replace("let seven: u32 = TEST();", r"let seven: kit\Answer = 7;");
    let (_, plan, _) = select(&program, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_some());
    let before = program.replace("let before: u32", r"let before: kit\Answer");
    let (_, plan, _) = select(&before, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_none());
}

#[test]
fn a_called_helpers_contract_is_a_dependency_use_before_its_body() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let program = PROGRAM.replace("TEST()", "consume()")
        + r"
machine consume() -> u32 requires kit\permitted() { 7u32 }";
    let (_, plan, _) = select(&program, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_some());
    let early = program.replace("let before: u32 = 1", "let before: u32 = consume()");
    let (_, plan, _) = select(&early, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_none());
}

#[test]
fn a_wait_in_a_sibling_arm_does_not_supply_another_arms_import() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let program = r"
use kit;
data Build {}
machine Build::prepare_dependency(builder: @mutable Build) suspends {}
machine build(builder: @mutable Build, choosing: bool) suspends {
    transition choosing {
        true -> suspend ready(builder),
        _ -> finish()
    }
    machine ready(builder: @mutable Build) suspends { suspend Build::prepare_dependency(builder); }
    machine finish() { let result: u32 = kit\seven(); }
}";
    let (_, plan, _) = select(program, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_none());
}

#[test]
fn a_value_returning_helper_keeps_its_authored_completion_at_a_nested_wait() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::new(5, 8),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let program = PROGRAM
        .replace(
            "suspend Build::prepare_dependency(builder);",
            "let saved: u32 = suspend ready(builder);",
        )
        .replace("TEST()", r"kit\seven(saved)")
        + r"
machine ready(builder: @mutable Build) -> u32 suspends {
    suspend Build::prepare_dependency(builder);
    19u32
}";
    let (syntax, plan, parked) = select(&program, &imports);
    let [caller, leaf] = plan.stage.outgoing_preparation_path().unwrap() else {
        panic!("helper and caller")
    };
    assert!(caller.projects_suffix);
    assert!(!leaf.projects_suffix);
    assert_eq!(parked.len(), 1);
    let ready = syntax
        .root_items()
        .find_map(|item| {
            item.as_machine()
                .filter(|machine| machine.name.as_str(&syntax) == "ready")
        })
        .unwrap();
    let state = syntax
        .items
        .state(syntax.items.state_handles(ready.states)[0]);
    let last = syntax.items.statements(state.statements).last().unwrap();
    let StatementNode::Transition(arm) = syntax.statements.statement(*last) else {
        panic!("ordinary return")
    };
    let syntax_trees::statement::TransitionTargetNode::Value(value) =
        syntax.statements.transition_target(arm.target)
    else {
        panic!("authored return value")
    };
    let span = syntax.expressions.source_span(*value).span;
    assert_eq!(&program[span.start..span.end], "19u32");
}

#[test]
fn separate_dispatch_arms_prepare_only_their_own_missing_scopes() {
    let program = r"
use left;
use right;
data Build {}
machine Build::prepare_dependency(builder: @mutable Build) suspends {}
machine build(builder: @mutable Build, choosing: bool) suspends {
    transition choosing {
        true -> suspend first(builder),
        _ -> suspend second(builder)
    }
    machine first(builder: @mutable Build) suspends {
        suspend Build::prepare_dependency(builder);
        let seen: u32 = left\answer();
    }
    machine second(builder: @mutable Build) suspends {
        suspend Build::prepare_dependency(builder);
        let seen: u32 = right\answer();
    }
}";
    let imports = ["left", "right"].map(|path| PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec![path.into()]),
    });
    let (_, initial, parked) = select(program, &imports);
    assert_eq!(parked.len(), 2);
    assert!(!initial.stage.can_finish_before_preparation());
    let choices = initial.stage.outgoing_preparation_paths();
    assert_eq!(choices.len(), 2);
    assert_eq!(
        choices
            .iter()
            .map(|path| {
                assert_eq!(path.len(), 2);
                assert!(!path[0].projects_suffix && path[1].projects_suffix);
                &program[path[0].call.start..path[0].call.end]
            })
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    for (selected, remaining) in [(0, 1), (1, 0)] {
        let mut actual = initial.stage.clone();
        actual.commit_preparation(selected).unwrap();
        let (_, next, _) = select_after(
            program,
            std::slice::from_ref(&imports[remaining]),
            Some(&actual),
        );
        assert!(next.stage.can_finish_before_preparation());
        assert_eq!(
            next.stage.incoming_preparation_path().unwrap(),
            choices[selected]
        );
        assert_eq!(
            next.stage.outgoing_preparation_path().unwrap(),
            choices[remaining]
        );
    }
    let uncovered = program.replace(
        "suspend Build::prepare_dependency(builder);\n        let seen: u32 = right",
        "let seen: u32 = right",
    );
    assert_ne!(program, uncovered);
    assert!(
        select(&uncovered, &imports)
            .1
            .stage
            .outgoing_preparation_paths()
            .is_empty()
    );
}

#[test]
fn a_record_constructor_alone_requests_the_missing_type() {
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let program = PROGRAM.replace(
        "let seven: u32 = TEST();",
        r"let item = kit\Score { value: 7i32 };",
    );
    let (_, plan, _) = select(&program, &imports);
    assert!(plan.stage.outgoing_preparation_path().is_some());
    assert!(!plan.stage.can_finish_before_preparation());
    let early = program.replace(
        "let before: u32 = 1;",
        r"let before = kit\Score { value: 1i32 };",
    );
    assert!(
        select(&early, &imports)
            .1
            .stage
            .outgoing_preparation_paths()
            .is_empty()
    );
}

#[test]
fn a_shared_use_requires_a_preparation_on_each_returning_arm() {
    let program = r"
use kit;
data Build {}
machine Build::prepare_dependency(builder: @mutable Build) suspends {}
machine build(builder: @mutable Build, choosing: bool) suspends {
    suspend choose(builder, choosing);
    let must_be_available: u32 = kit\answer();
}
machine choose(builder: @mutable Build, choosing: bool) suspends {
    transition choosing { true -> suspend first(builder), _ -> skip() }
    machine first(builder: @mutable Build) suspends {
        suspend Build::prepare_dependency(builder);
        let seen: u32 = kit\answer();
    }
    machine skip() {}
}";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (_, plan, parked) = select(program, &imports);
    assert!(plan.stage.outgoing_preparation_paths().is_empty());
    assert!(parked.is_empty());
}

#[test]
fn a_projected_value_helper_keeps_its_real_result_type_and_exact_open_call() {
    let text = r"
use kit;
data Build {}
machine Build::prepare_dependency(builder: @mutable Build) suspends {}
machine build(builder: @mutable Build) suspends { let saved: u32 = suspend ready(builder); }
machine ready(builder: @mutable Build) -> u32 suspends {
    suspend Build::prepare_dependency(builder);
    kit\answer()
}";
    let imports = [PendingBuildImport {
        source: SOURCE,
        span: Span::default(),
        target: crate::SourceImportTarget::Namespace(vec!["kit".into()]),
    }];
    let (syntax, plan, parked) = select(text, &imports);
    let [open] = parked.as_slice() else {
        panic!("only the actual value-producing body is open")
    };
    let path = plan.stage.outgoing_preparation_path().unwrap();
    assert_eq!(path.len(), 2);
    assert!(!path[0].projects_suffix && path[1].projects_suffix);
    assert_eq!(
        open.pending_completion_at,
        SourceSpan::new(SOURCE, path[1].call)
    );
    assert_eq!(
        &text[open.pending_completion_at.span.start..open.pending_completion_at.span.end],
        "prepare_dependency"
    );
    let typed = symbol_resolved_trees_to_typed_trees::typing::lower_symbol_resolved_trees(
        &resolve_program(syntax, text),
    )
    .unwrap();
    let ready = typed
        .machines()
        .iter()
        .find(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(open.entry))
        .unwrap();
    let entry = &typed.machine_states(ready)[0];
    assert_eq!(typed.symbols.symbol_source_span(entry.symbol), open.state);
    assert!(entry.return_type.is_valid());
    assert_eq!(typed.display_type_reference(entry.return_type), "u32");

    // The same shape without a pending use is an ordinary missing return.
    let unfinished = text.replace(r"    kit\answer()", "");
    assert_ne!(unfinished, text);
    let (_, plan, rows) = select(&unfinished, &imports);
    assert!(plan.stage.outgoing_preparation_paths().is_empty() && rows.is_empty());
}
