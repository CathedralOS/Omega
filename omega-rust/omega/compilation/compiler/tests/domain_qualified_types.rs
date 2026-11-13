//! The lowered-form checker reads a type that carries a declared domain,
//! `T in D`, as the carrier `T` and the set of domains on it
//! (`wiki/spec/language/domains.md`, "Literals and expected types": "the
//! supplied expression's static type must carry exactly the domains the
//! destination declares"). A value keeps its domains through a parameter, a
//! call and a result; a cast entry keeps or removes one by name; nothing
//! else attaches or drops one.
//!
//! One program pins the accepted forms from source. Each twin changes one
//! machine and is not accepted.

use lowered_discharge::RouteVerdict;

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const DOMAINS: &str = "\
domain i32::Km;
machine carry(distance: i32 in Km) -> i32 in Km {
    distance
}
machine main() { }
";

/// A qualified value passes to a parameter and a result of its exact type,
/// `=Km` keeps the domain and `-Km` removes it.
const ACCEPTED: &str = "\
machine relay(distance: i32 in Km) -> i32 in Km {
    carry(distance)
}
machine kept(distance: i32 in Km) -> i32 in Km {
    distance as =Km
}
machine removed(distance: i32 in Km) -> i32 {
    distance as -Km
}
machine compared(left: i32 in Km, right: i32 in Km) -> bool {
    (left as -Km) > (right as -Km)
}
";

/// The verdict of each judged part of `DOMAINS` followed by `machines`,
/// staged as a standalone `main.omg`: one word, and what a refusal names.
fn verdicts(name: &str, machines: &str) -> Vec<String> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root =
        std::env::temp_dir().join(format!("omega-domain-types-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&root).expect("staging directory");
    let main = root.join("main.omg");
    std::fs::write(&main, format!("{DOMAINS}{machines}")).expect("staged source");
    let frontier = compiler::CompilationSession::default().typed_frontier_without_build(
        compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
        None,
    );
    let _ = std::fs::remove_dir_all(&root);
    let frontier = frontier.unwrap_or_else(|failure| {
        panic!(
            "{name}: Psi 00 through 04 must complete: {:?}",
            failure.into_diagnostics()
        )
    });
    lowered_discharge::judge_compilation(
        &frontier.typed,
        structure_elaboration::Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    )
    .iter()
    .map(|verdict| match verdict {
        Ok(RouteVerdict::Accepted(_)) => "accepted".to_owned(),
        Ok(RouteVerdict::Rejected { .. }) => "rejected".to_owned(),
        Ok(RouteVerdict::Incomplete(_)) => "incomplete".to_owned(),
        Ok(RouteVerdict::FallBack(refusal)) => format!("not-supported: {refusal:?}"),
        Err(defect) => format!("defect: {defect:?}"),
    })
    .collect()
}

#[test]
fn a_value_keeps_its_domain_through_parameters_results_and_entries() {
    assert_eq!(verdicts("accepted", ACCEPTED), ["accepted"]);
}

/// The one verdict of a twin, which must not be an acceptance and must name
/// `expected`.
fn refused_with(name: &str, machines: &str, expected: &str) {
    let verdicts = verdicts(name, machines);
    assert!(
        verdicts.len() == 1 && verdicts[0].contains(expected),
        "{name}: {verdicts:#?}"
    );
}

#[test]
fn a_plain_argument_for_a_qualified_parameter_is_not_accepted() {
    refused_with(
        "plain_argument",
        "machine wrong(distance: i32) -> i32 in Km {\n    carry(distance)\n}\n",
        "typing argument types",
    );
}

#[test]
fn a_plain_value_for_a_qualified_result_is_not_accepted() {
    refused_with(
        "plain_result",
        "machine wrong(distance: i32) -> i32 in Km {\n    distance\n}\n",
        "typing arm value type",
    );
}

#[test]
fn a_qualified_value_for_a_plain_result_is_not_accepted() {
    refused_with(
        "qualified_result",
        "machine wrong(distance: i32 in Km) -> i32 {\n    distance\n}\n",
        "typing arm value type",
    );
}

#[test]
fn a_builtin_comparison_of_qualified_operands_is_not_accepted() {
    refused_with(
        "qualified_operands",
        "machine wrong(left: i32 in Km, right: i32 in Km) -> bool {\n    left > right\n}\n",
        "operator on an operand that is not a plain or policy-qualified scalar",
    );
}

/// Whether a machine may add a domain is the next judgment the checker
/// makes; until it does, every addition is refused by name.
#[test]
fn adding_a_declared_domain_is_refused_by_name() {
    refused_with(
        "added",
        "machine wrong(distance: i32) -> i32 in Km {\n    distance as +Km\n}\n",
        "cast that adds a declared domain",
    );
}
