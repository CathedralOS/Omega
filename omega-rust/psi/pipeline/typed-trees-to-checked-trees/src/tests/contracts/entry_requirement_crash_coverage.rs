use super::*;

fn accepts(source: &str) {
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects_uncovered(source: &str) {
    let diagnostics = match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => panic!("an entry requirement cannot establish this route: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("uncovered")),
        "{diagnostics:#?}"
    );
}

#[test]
fn boolean_entry_requirements_cover_matching_direct_crash_sites() {
    for (parameters, requirement, route) in [
        ("flag: bool", "flag", "flag"),
        ("mut flag: bool", "!flag", "!flag"),
        ("flag: bool, other: bool", "flag && other", "flag"),
        ("flag: bool, other: bool", "!(flag || other)", "!other"),
    ] {
        accepts(&format!(
            "machine value({parameters}) -> bool\nrequires {requirement}\ncrashes Trap {route}\n{{ crash Trap; }}"
        ));
    }
}

#[test]
fn entry_requirement_snapshots_survive_writes_and_named_state_rebinding() {
    accepts(
        "machine value(mut flag: bool) -> bool\nrequires flag\ncrashes Trap flag\n{ flag = false; transition { _ -> failed(false) } state failed(flag: bool) -> bool { crash Trap; } }",
    );
}

#[test]
fn entry_requirements_cover_matching_call_routes() {
    accepts(
        "machine trigger() -> bool\ncrashes Trap\n{ crash Trap; }\nmachine value(flag: bool) -> bool\nrequires flag\ncrashes Trap flag\n{ trigger() }",
    );
}

#[test]
fn entry_requirements_do_not_change_cause_formal_or_disjunction() {
    for source in [
        "machine value(flag: bool) -> bool\nrequires flag\ncrashes Abort flag\n{ crash Trap; }",
        "machine value(flag: bool, other: bool) -> bool\nrequires flag\ncrashes Trap other\n{ crash Trap; }",
        "machine value(flag: bool, other: bool) -> bool\nrequires (flag || other)\ncrashes Trap flag\n{ crash Trap; }",
        "machine value(mut flag: bool) -> bool\nrequires !flag\ncrashes Trap flag\n{ flag = true; crash Trap; }",
        "machine value(mut flag: bool) -> bool\nrequires !flag\ncrashes Trap flag\n{ flag = true; transition flag { true -> failed() false -> false } state failed() -> bool { crash Trap; } }",
    ] {
        rejects_uncovered(source);
    }
}

#[test]
fn named_state_requirements_are_not_ambient_invocation_facts() {
    rejects_uncovered(
        "machine value(flag: bool) -> bool\ncrashes Trap flag\n{ transition { _ -> failed(true) } state failed(flag: bool) -> bool\nrequires flag\n{ crash Trap; } }",
    );
}

#[test]
fn entry_polarity_covers_equivalent_builtin_boolean_route_spellings() {
    for (requirement, route) in [
        ("!flag", "flag == false"),
        ("!flag", "false == flag"),
        ("!flag", "flag != true"),
        ("!flag", "!(flag == true)"),
        ("flag", "!flag == false"),
        ("flag", "!!flag == true"),
        ("!(flag || other)", "other == false"),
    ] {
        for call in [false, true] {
            let body = if call { "trigger()" } else { "crash Trap;" };
            accepts(&format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine value(mut flag: bool, other: bool) -> bool\nrequires {requirement}\n\
                 crashes Trap {route}\n{{ flag = true; {body} }}",
            ));
        }
    }
}

#[test]
fn equivalent_route_spelling_needs_the_complete_exact_entry_fact() {
    for (requirement, route) in [
        ("flag", "flag == false"),
        ("!other", "flag == false"),
        ("!flag", "flag == true"),
        ("!flag", "(flag == false) && other"),
        ("!flag", "flag == other"),
    ] {
        rejects_uncovered(&format!(
            "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
             machine value(mut flag: bool, other: bool) -> bool\nrequires {requirement}\n\
             crashes Trap {route}\n{{ flag = false; trigger() }}",
        ));
    }
    rejects_uncovered(
        "machine value(mut flag: bool) -> bool\ncrashes Trap flag == false\n\
         { flag = false; transition flag { false -> failed() true -> true }\n\
           state failed() -> bool { crash Trap; } }",
    );
}
