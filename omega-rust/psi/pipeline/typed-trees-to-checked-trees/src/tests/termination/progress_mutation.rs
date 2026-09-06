use super::*;
use language_semantics::TerminationGuarantee;

fn fixture(statements: &str) -> checked_trees::CheckedTrees {
    fixture_with_contract(statements, true, false)
}

fn fixture_with_contract(
    statements: &str,
    requires_original: bool,
    published: bool,
) -> checked_trees::CheckedTrees {
    fixture_with_extra(statements, requires_original, published, "")
}

fn fixture_with_extra(
    statements: &str,
    requires_original: bool,
    published: bool,
    extra: &str,
) -> checked_trees::CheckedTrees {
    let body = format!("{statements}\ntransition {{ _ -> wait_context(context) }}");
    fixture_with_body(&body, requires_original, published, extra)
}

fn fixture_with_body(
    body: &str,
    requires_original: bool,
    published: bool,
    extra: &str,
) -> checked_trees::CheckedTrees {
    check_source(&fixture_source(body, requires_original, published, extra))
}

fn fixture_source(body: &str, requires_original: bool, published: bool, extra: &str) -> String {
    let original_requirement = if requires_original {
        "requires context.scheduler in WeakFair"
    } else {
        ""
    };
    let publication = if published { "terminates;" } else { "" };
    let visibility = if published { "pub " } else { "" };
    format!(
        r#"
        data Main {{}}
        machine Main::run(&mut self) {{}}
        pub data SchedulerHandle {{}}
        pub data Context {{ scheduler: SchedulerHandle; counter: u64; }}
        pub domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        pub boundary trait SchedulerAdmission {{
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }}
        pub machine wait_context(context: &Context)
        requires context.scheduler in WeakFair
        terminates;
        -> u64 {{ 0 }}
        {visibility}machine replace(context: &mut Context, replacement: &Context) -> u64
        {original_requirement}
        requires replacement.scheduler in WeakFair
        {publication}
        {{
            {body}
        }}
        {extra}
        "#
    )
}

fn check_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize progress mutation");
    let syntax = parse_syntax_trees(&tokens).expect("parse progress mutation");
    let resolved = lower_syntax_trees(&syntax).expect("resolve progress mutation");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type progress mutation");
    lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("check progress mutation: {diagnostics:#?}"))
}

fn assert_subject(statements: &str, expected_parameter: &str) {
    let program = fixture(statements);
    assert_subjects(&program, &[expected_parameter]);
}

fn assert_subjects(program: &checked_trees::CheckedTrees, expected_parameters: &[&str]) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "replace")
        .unwrap();
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .unwrap();
    let TerminationGuarantee::Terminates { premises } = &plan.checked_summary else {
        panic!("exact structural assignment must retain a checked progress summary");
    };
    assert_eq!(
        premises.len(),
        expected_parameters.len(),
        "only actually used schedulers supply progress premises: {premises:#?}"
    );
    for expected_parameter in expected_parameters {
        let parameter = program
            .state_parameters(&program.machine_states(machine)[0])
            .iter()
            .find(|parameter| parameter.name.as_str() == *expected_parameter)
            .unwrap();
        let premise = premises.iter().find(|premise| premise.subject.root == parameter.symbol)
            .unwrap_or_else(|| panic!("progress must follow {expected_parameter}, not the overwritten slot: {premises:#?}"));
        assert_eq!(premise.subject.projections.len(), 1);
        assert_eq!(
            program
                .symbols
                .display_path(premise.subject.projections[0], "::"),
            "Context::scheduler"
        );
    }
}

#[test]
fn unchanged_progress_subject_uses_the_original_parameter() {
    assert_subject("", "context");
}

#[test]
fn disjoint_store_preserves_the_original_progress_subject() {
    assert_subject("context.counter = 1;", "context");
}

#[test]
fn replaced_field_uses_the_replacement_progress_subject() {
    assert_subject("context.scheduler = replacement.scheduler;", "replacement");
}

#[test]
fn calls_before_and_after_replacement_keep_both_exact_subjects() {
    let program = fixture("_ = wait_context(context); context.scheduler = replacement.scheduler;");
    assert_subjects(&program, &["context", "replacement"]);
}

#[test]
fn saved_original_value_restores_the_original_progress_subject() {
    assert_subject(
        "let original: SchedulerHandle = context.scheduler; context.scheduler = replacement.scheduler; context.scheduler = original;",
        "context",
    );
}

#[test]
fn published_replacement_requires_only_the_replacement_premise() {
    let program = fixture_with_contract("context.scheduler = replacement.scheduler;", false, true);
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn disjoint_helper_write_preserves_the_exact_progress_subject() {
    let program = fixture_with_extra(
        "_ = increment_counter(context);",
        true,
        false,
        "machine increment_counter(context: &mut Context) -> u64 { context.counter = 1; 0 }",
    );
    assert_subjects(&program, &["context"]);
}

#[test]
fn an_overlapping_helper_write_cannot_preserve_entry_identity() {
    let program = fixture_with_extra(
        "_ = overwrite(context, replacement);",
        true,
        false,
        "pub machine overwrite(context: &mut Context, replacement: &Context) -> u64
         requires replacement.scheduler in WeakFair
         ensures context.scheduler in WeakFair
         terminates;
         { context.scheduler = replacement.scheduler; 0 }",
    );
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}

#[test]
fn copied_field_carries_its_origin_into_a_named_state() {
    let program = fixture_with_body(
        "let copied: SchedulerHandle = replacement.scheduler;
         transition { _ -> waiting(copied) }
         state waiting(scheduler: SchedulerHandle) -> u64
         requires scheduler in WeakFair
         { wait_scheduler(scheduler) }",
        true,
        false,
        "pub machine wait_scheduler(scheduler: SchedulerHandle) -> u64
         requires scheduler in WeakFair
         terminates;
         { 0 }",
    );
    assert_subjects(&program, &["replacement"]);
}

#[test]
fn an_owned_copy_keeps_the_source_value_from_before_a_later_store() {
    for body in [
        "let original: SchedulerHandle = context.scheduler;
         context.scheduler = replacement.scheduler;
         replacement.scheduler = original;",
        "let saved: SchedulerHandle = replacement.scheduler;
         replacement.scheduler = context.scheduler;
         context.scheduler = saved;",
    ] {
        let source = fixture_source(
            &format!("{body}\ntransition {{ _ -> wait_context(context) }}"),
            true,
            false,
            "",
        )
        .replace(
            "replacement: &Context) -> u64",
            "replacement: &mut Context) -> u64",
        );
        assert_subjects(&check_source(&source), &["replacement"]);
    }
}

#[test]
fn an_owned_copy_keeps_the_source_value_from_before_a_later_call() {
    let source = fixture_source(
        "let saved: SchedulerHandle = replacement.scheduler;
         _ = overwrite(replacement, context);
         context.scheduler = saved;
         transition { _ -> wait_context(context) }",
        true,
        false,
        "machine overwrite(destination: &mut Context, source: &Context) -> u64
         requires source.scheduler in WeakFair
         ensures destination.scheduler in WeakFair
         { destination.scheduler = source.scheduler; 0 }",
    )
    .replace(
        "replacement: &Context) -> u64",
        "replacement: &mut Context) -> u64",
    );
    assert_subjects(&check_source(&source), &["replacement"]);
}

#[test]
fn a_changed_field_before_a_named_transition_cannot_reuse_entry_identity() {
    let program = fixture_with_body(
        "context.scheduler = replacement.scheduler;
         transition { _ -> waiting(context) }
         state waiting(selected: &Context) -> u64
         requires selected.scheduler in WeakFair
         { wait_context(selected) }",
        true,
        false,
        "",
    );
    let plan = program
        .facts
        .termination
        .for_machine(symbol_of_checked(&program, "replace"))
        .unwrap();
    // Per-field arrival substitution is still needed before this can derive
    // replacement.scheduler. Root identity cannot stand in for those contents.
    assert_eq!(plan.checked_summary, TerminationGuarantee::NoGuarantee);
}
