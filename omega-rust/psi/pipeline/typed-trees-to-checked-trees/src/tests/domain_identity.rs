use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

fn check(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("checked lowering should succeed")
}

fn specializations_for<'program>(
    checked: &'program checked_trees::CheckedTrees,
    machine_name: &str,
) -> Vec<&'program typed_trees::typed_trees::MachineSpecialization> {
    checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == machine_name
            })
        })
        .collect()
}

#[test]
fn reordered_domain_conjunctions_share_one_specialization() {
    let checked = check(
        r#"
        domain i32::Alpha;
        domain i32::Beta;
        data Main {}

        machine inspect<T [copy]>(value: T) {}

        machine caller(
            first: i32 in Alpha & Beta,
            second: i32 in Beta & Alpha
        ) {
            inspect(first);
            inspect(second);
        }

        machine Main::run(&mut self) {}
        "#,
    );

    let specializations = specializations_for(&checked, "inspect");
    assert_eq!(
        specializations.len(),
        1,
        "commutative domain conjunctions are one monomorphization key"
    );
}

#[test]
fn distinct_domains_with_the_same_term_count_do_not_share_a_specialization() {
    let checked = check(
        r#"
        domain i32::Alpha;
        domain i32::Beta;
        data Main {}

        machine inspect<T [copy]>(value: T) {}

        machine caller(first: i32 in Alpha, second: i32 in Beta) {
            inspect(first);
            inspect(second);
        }

        machine Main::run(&mut self) {}
        "#,
    );

    let specializations = specializations_for(&checked, "inspect");
    assert_eq!(
        specializations.len(),
        2,
        "domain contents, not a diagnostic constraint count, own specialization identity"
    );
    assert_ne!(
        specializations[0].report_fingerprint, specializations[1].report_fingerprint,
        "distinct normalized domain expressions must publish distinct specialization fingerprints"
    );
}
