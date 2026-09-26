use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn accepts(source: &str) {
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects_uncovered(source: &str) {
    let diagnostics =
        match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
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

#[test]
fn common_disjunctive_entry_consequences_cover_direct_and_call_routes() {
    for (requirement, route) in [
        ("(a && b) || (a && c)", "a"),
        ("(a && b) || ((a && c) || (a && d))", "a"),
        ("((a && b) || (a && c)) && ((d && b) || (d && c))", "a && d"),
        ("(!a && b) || (c && !a)", "a == false"),
        ("!((a || b) && (a || c))", "!a"),
        ("((a && b) || (a && c)) == true", "a"),
    ] {
        for body in ["crash Trap;", "trigger()"] {
            accepts(&format!(
                "machine trigger() -> bool\ncrashes Trap\n{{ crash Trap; }}\n\
                 machine value(mut a: bool, b: bool, c: bool, d: bool) -> bool\n\
                 requires {requirement}\ncrashes Trap {route}\n{{ a = false; {body} }}",
            ));
        }
    }
    accepts(
        "machine value(mut a: bool, b: bool, c: bool) -> bool\n\
         requires (a && b) || (a && c)\ncrashes Trap a\n\
         { a = false; transition { _ -> failed(false) }\n\
           state failed(a: bool) -> bool { crash Trap; } }",
    );
}
