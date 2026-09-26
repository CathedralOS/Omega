use crate::tests::front_end::checked_program_result;
use language_semantics::TerminationGuarantee;

mod aliases;
mod arrivals;

const CONTEXT_FIXTURE: &str = r#"
    data Main {}
    machine Main::run(&mut self) {}
    // Copyable subject metadata lets these fixtures test snapshot lineage;
    // copying still cannot establish a missing or invalidated progress profile.
    pub data SchedulerHandle [copy] {}
    pub data Context { scheduler: SchedulerHandle; counter: u64; }
    pub domain SchedulerHandle::WeakFair
    satisfies ProgressProfile
    established by SchedulerAdmission::grant;
    pub boundary trait SchedulerAdmission {
        machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
    }
    pub machine wait_context(context: &Context)
    requires context.scheduler in WeakFair
    terminates;
    -> u64 { 0 }
"#;

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
        {CONTEXT_FIXTURE}
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
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("check progress mutation: {diagnostics:#?}"))
}

fn assert_unproved_tail_requirement(source: &str) {
    let diagnostics = match checked_program_result(source) {
        Ok(_) => panic!("an exact progress origin cannot substitute for a proven call requirement"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract for call wait_context")),
        "{diagnostics:#?}"
    );
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
