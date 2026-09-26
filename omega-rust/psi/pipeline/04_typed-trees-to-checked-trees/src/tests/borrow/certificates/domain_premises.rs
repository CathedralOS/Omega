use crate::tests::front_end::{checked_program, checked_program_result};
use checked_trees::BorrowCompatibilityPremiseSource;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionNode;

const DOMAIN_WINDOW: &str = r#"
    domain u64::Upper requires self >= 2;
    data Main { items: [i32; 4]; }
    machine take(slot: &mut i32) { slot = 7; }
    machine Main::main(&mut self, split_point: u64 [0..=4]) -> u64
        requires split_point in u64::Upper;
    {
        let held: &mut [i32] = self.items[split_point..4];
        self.items[0] = 3;
        take(&mut self.items[1]);
        held.len
    }
"#;

#[test]
fn domain_predicate_boolean_structure_and_aliases_share_the_same_judgment() {
    for predicate in [
        "self > 1",
        "self == 2",
        "2 <= self",
        "self >= 2 && self <= 4",
        "!(self < 2 || self > 4)",
        "(self >= 2) == true",
        "false != (self >= 2)",
    ] {
        let mut checked = checked_program(&DOMAIN_WINDOW.replace("self >= 2", predicate));
        assert_domain_premises(&checked);
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect("Boolean decomposition preserves exact domain meaning");
    }
    let source = DOMAIN_WINDOW
        .replace("data Main", "domain u64::Alias = u64::Upper; data Main")
        .replace(
            "requires split_point in u64::Upper",
            "requires split_point in u64::Alias",
        );
    let checked = checked_program(&source);
    assert_domain_premises(&checked);
}

fn assert_domain_premises(checked: &checked_trees::CheckedTrees) {
    assert!(
        checked
            .facts
            .borrow
            .mutation_certificates
            .iter()
            .any(
                |(_, certificate)| certificate.premises.iter().any(|premise| matches!(
                    premise.source,
                    BorrowCompatibilityPremiseSource::RequiresDomain { .. }
                ))
            )
    );
    assert!(
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .any(
                |(_, certificate)| certificate.premises.iter().any(|premise| matches!(
                    premise.source,
                    BorrowCompatibilityPremiseSource::RequiresDomain { .. }
                ))
            )
    );
}

const PROJECTED_DOMAIN_WINDOW: &str = r#"
    domain u64::Upper requires self >= 2;
    data Pair { first: u64 [0..=4]; second: u64; }
    data Main { items: [i32; 4]; }
    machine Main::main(&mut self, pair: Pair) -> u64
        requires pair.first in u64::Upper;
    {
        let held: &mut [i32] = self.items[pair.first..4];
        self.items[0] = 3;
        held.len
    }
"#;

#[test]
fn projected_membership_subjects_reject_foreign_weakened_or_absent() {
    let source = PROJECTED_DOMAIN_WINDOW.replace(
        "requires pair.first in u64::Upper;",
        "requires pair.second in u64::Upper;",
    );
    assert_conflict(&source);
    for predicate in ["self >= 0", "self >= 2 || self == 0"] {
        assert_conflict(&PROJECTED_DOMAIN_WINDOW.replace("self >= 2", predicate));
    }
    assert_conflict(&PROJECTED_DOMAIN_WINDOW.replace("requires pair.first in u64::Upper;", ""));
    assert_conflict(&PROJECTED_DOMAIN_WINDOW.replace("pair: Pair", "mut pair: Pair"));
}

fn assert_conflict(source: &str) {
    let diagnostics = checked_program_result(source).expect_err("unproven separation rejects");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("while local borrow")
                || diagnostic.message.contains("is still active")
        ),
        "{diagnostics:?}"
    );
}

fn assert_replay_rejects(checked: &mut checked_trees::CheckedTrees) {
    let mutations = checked.facts.borrow.mutation_certificates.clone();
    let calls = checked.facts.borrow.call_compatibility_certificates.clone();
    let diagnostics =
        crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
            .expect_err("changed membership evidence must reject");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("premise tokens drifted")
                || diagnostic
                    .message
                    .contains("derivation drifted from its recorded premise ledger")
        ),
        "{diagnostics:?}"
    );
    assert_eq!(checked.facts.borrow.mutation_certificates, mutations);
    assert_eq!(checked.facts.borrow.call_compatibility_certificates, calls);
}

fn domain_predicate(
    checked: &checked_trees::CheckedTrees,
) -> typed_trees::expression::ExpressionHandle {
    let domain = checked
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("::Upper"))
        .expect("Upper declaration");
    let ProofFact::Expression(expression) = checked.typed.proof_facts.get(domain.facts.start())
    else {
        panic!("domain expression");
    };
    *expression
}

#[test]
fn replay_rejects_changed_membership_subject_or_domain() {
    for change_domain in [false, true] {
        let mut checked = checked_program(DOMAIN_WINDOW);
        let handle = checked
            .typed
            .proof_facts
            .iter()
            .find_map(|(handle, fact)| matches!(fact, ProofFact::Membership(_)).then_some(handle))
            .expect("required membership");
        let ProofFact::Membership(membership) = checked.typed.proof_facts.get_mut(handle) else {
            panic!("membership");
        };
        if change_domain {
            membership.domain_symbol = symbols::SymbolHandle::invalid();
        } else {
            membership.value = typed_trees::expression::ExpressionHandle::invalid();
        }
        assert_replay_rejects(&mut checked);
    }
}

#[test]
fn sibling_domain_self_cannot_be_grafted_into_the_membership_theory() {
    let source = DOMAIN_WINDOW.replace(
        "data Main",
        "domain u64::Sibling requires self >= 2; data Main",
    );
    let mut checked = checked_program(&source);
    let expression = domain_predicate(&checked);
    let sibling = checked
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("::Sibling"))
        .expect("sibling");
    let ProofFact::Expression(sibling_expression) =
        checked.typed.proof_facts.get(sibling.facts.start())
    else {
        panic!("sibling predicate");
    };
    let ExpressionNode::Binary(sibling_binary) = checked
        .typed
        .expression_table
        .expression(*sibling_expression)
    else {
        panic!("sibling comparison");
    };
    let foreign_self = sibling_binary.left;
    let ExpressionNode::Binary(binary) = checked.typed.expression_table.expression_mut(expression)
    else {
        panic!("comparison");
    };
    binary.left = foreign_self;
    assert_replay_rejects(&mut checked);
}
