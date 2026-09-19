use super::checked_source;
use checked_trees::{BorrowCompatibilityDerivation, BorrowCompatibilityPremiseSource};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionNode};

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
fn domain_membership_certifies_disjoint_window_write_and_call() {
    let mut checked = checked_source(DOMAIN_WINDOW);
    assert!(
        checked
            .facts
            .borrow
            .mutation_certificates
            .iter()
            .any(|(_, certificate)| certificate.derivation
                == checked_trees::BorrowCompatibilityDerivation::Premised)
    );
    assert!(
        checked
            .facts
            .borrow
            .call_compatibility_certificates
            .iter()
            .any(|(_, certificate)| certificate.derivation
                == checked_trees::BorrowCompatibilityDerivation::Premised)
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("domain-established borrow evidence replays");
}

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
        let mut checked = checked_source(&DOMAIN_WINDOW.replace("self >= 2", predicate));
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
    let checked = checked_source(&source);
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

#[test]
fn membership_can_separate_loans_but_cannot_license_contained_exclusive_loans() {
    let source = DOMAIN_WINDOW.replace(
        "self.items[0] = 3;\n        take(&mut self.items[1]);",
        "let outside: &mut i32 = &mut self.items[1]; outside = 3;",
    );
    let mut checked = checked_source(&source);
    assert!(
        checked
            .facts
            .borrow
            .compatibility_certificates
            .iter()
            .any(|(_, certificate)| certificate.derivation
                == BorrowCompatibilityDerivation::Premised
                && certificate.conclusion.disjoint)
    );
    crate::checks::check_checked_facts_recording(&checked.typed, &mut checked.facts)
        .expect("formation replays the same domain predicate");
    assert_conflict(&source.replace("&mut self.items[1]", "&mut self.items[split_point]"));
}

#[test]
fn unrelated_weak_disjunctive_or_absent_membership_cannot_separate_places() {
    for predicate in [
        "self >= 1",
        "self != 1",
        "self >= 2 || self == 0",
        "!(self < 2 && self > 4)",
    ] {
        assert_conflict(&DOMAIN_WINDOW.replace("self >= 2", predicate));
    }
    assert_conflict(&DOMAIN_WINDOW.replace("requires split_point in u64::Upper;", ""));
    assert_conflict(&DOMAIN_WINDOW.replace("self.items[1]", "self.items[split_point]"));
    assert_conflict(
        &DOMAIN_WINDOW
            .replace(
                "split_point: u64 [0..=4]",
                "split_point: u64 [0..=4], other: u64 [0..=4]",
            )
            .replace("requires split_point in", "requires other in"),
    );
}

#[test]
fn mutable_membership_subject_needs_preservation_evidence() {
    assert_conflict(&DOMAIN_WINDOW.replace("split_point: u64", "mut split_point: u64"));
}

fn assert_conflict(source: &str) {
    use crate::tests::{
        Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
    };
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize domain control");
    let syntax = parse_syntax_trees(&tokens).expect("parse domain control");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve domain control");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type domain control");
    let diagnostics = crate::lower_typed_trees(typed).expect_err("unproven separation rejects");
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
fn replay_rejects_changed_domain_predicate() {
    let mut checked = checked_source(DOMAIN_WINDOW);
    let expression = domain_predicate(&checked);
    let ExpressionNode::Binary(binary) = checked.typed.expression_table.expression_mut(expression)
    else {
        panic!("comparison");
    };
    binary.operator = BinaryOperator::LessOrEqual;
    assert_replay_rejects(&mut checked);
}

#[test]
fn replay_rejects_changed_membership_subject_or_domain() {
    for change_domain in [false, true] {
        let mut checked = checked_source(DOMAIN_WINDOW);
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
fn replay_rejects_retargeted_or_missing_domain_tokens() {
    for change in 0..5 {
        let mut checked = checked_source(DOMAIN_WINDOW);
        let handle = checked
            .facts
            .borrow
            .mutation_certificates
            .iter()
            .find_map(|(handle, certificate)| (!certificate.premises.is_empty()).then_some(handle))
            .expect("premised mutation");
        let certificate = checked.facts.borrow.mutation_certificates.get_mut(handle);
        if change == 0 {
            certificate.premises.clear();
        } else {
            let premise = &mut certificate.premises[0];
            let BorrowCompatibilityPremiseSource::RequiresDomain {
                membership,
                domain,
                predicate,
            } = &mut premise.source
            else {
                panic!("domain token");
            };
            match change {
                1 => *membership = arena::Handle::invalid(),
                2 => *domain = symbols::SymbolHandle::invalid(),
                3 => *predicate = arena::Handle::invalid(),
                4 => premise.right = premise.left,
                _ => unreachable!(),
            }
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
    let mut checked = checked_source(&source);
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

#[test]
fn replay_rejects_authored_ordering_and_boolean_wrapper_meanings() {
    for (carrier, spelling, predicate) in [
        (
            "u64",
            language_core::OperatorSpelling::GreaterEqual,
            "self >= 2",
        ),
        (
            "bool",
            language_core::OperatorSpelling::Equal,
            "(self >= 2) == true",
        ),
    ] {
        let source = format!(
            "operator Quantity::compare(left: {carrier}, right: {carrier}) -> bool; {}",
            DOMAIN_WINDOW.replace("self >= 2", predicate),
        );
        let mut checked = checked_source(&source);
        let operator = checked.typed.roots.operators.start();
        checked.typed.tables.operators.get_mut(operator).spelling = Some(spelling);
        assert_replay_rejects(&mut checked);
    }
}

#[test]
fn replay_rejects_changed_domain_carrier_and_instance_identity() {
    for change_carrier in [false, true] {
        let mut checked = checked_source(DOMAIN_WINDOW);
        let (handle, _) = checked
            .typed
            .tables
            .domain_definitions
            .iter()
            .find(|(_, domain)| domain.name.as_str().ends_with("::Upper"))
            .expect("domain owner");
        if change_carrier {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| checked.typed.symbols.name(machine.symbol) == "take")
                .expect("take");
            let state = &checked.typed.machine_states(machine)[0];
            let parameter = checked.typed.state_parameters(state)[0].type_reference;
            checked
                .typed
                .tables
                .domain_definitions
                .get_mut(handle)
                .target_type = parameter;
        } else {
            checked
                .typed
                .tables
                .domain_definitions
                .get_mut(handle)
                .semantic_id = language_semantics::SemanticDomainId::NULL;
        }
        assert_replay_rejects(&mut checked);
    }
}

#[test]
fn replay_rejects_jointly_missing_domain_identities() {
    let mut checked = checked_source(DOMAIN_WINDOW);
    let handle = checked
        .typed
        .proof_facts
        .iter()
        .find_map(|(handle, fact)| matches!(fact, ProofFact::Membership(_)).then_some(handle))
        .expect("membership");
    let ProofFact::Membership(membership) = checked.typed.proof_facts.get_mut(handle) else {
        panic!("membership");
    };
    let domain_symbol = membership.domain_symbol;
    membership.semantic_domain = language_semantics::SemanticDomainId::NULL;
    let domain_handle = checked
        .typed
        .tables
        .domain_definitions
        .iter()
        .find_map(|(handle, domain)| (domain.symbol == domain_symbol).then_some(handle))
        .expect("domain");
    checked
        .typed
        .tables
        .domain_definitions
        .get_mut(domain_handle)
        .semantic_id = language_semantics::SemanticDomainId::NULL;
    assert_replay_rejects(&mut checked);
}

#[test]
fn replay_rejects_foreign_carrier_symbol_behind_builtin_spelling() {
    let mut checked = checked_source(DOMAIN_WINDOW);
    let domain = checked
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("::Upper"))
        .expect("domain");
    let carrier = domain.target_type;
    let foreign_symbol = checked.typed.data_definitions()[0].symbol;
    let typed_trees::types::TypeReferenceNode::Named { name, .. } = checked
        .typed
        .type_reference_table
        .type_reference(carrier)
        .clone()
    else {
        panic!("named carrier");
    };
    checked.typed.type_reference_table.substitute_node(
        carrier,
        typed_trees::types::TypeReferenceNode::Named {
            name,
            symbol: foreign_symbol,
        },
    );
    assert_replay_rejects(&mut checked);
}

#[test]
fn domain_subject_carrier_comes_from_the_binding_not_its_display_name() {
    let source = DOMAIN_WINDOW
        .replace(
            "split_point: u64 [0..=4]",
            "split_point: u64 [0..=4], other: i32",
        )
        .replace(
            "requires split_point in u64::Upper;",
            "requires split_point in u64::Upper; requires other == other;",
        );
    let mut checked = checked_source(&source);
    let original = checked
        .typed
        .proof_facts
        .iter()
        .find_map(|(_, fact)| match fact {
            ProofFact::Membership(membership) => Some(membership.value),
            _ => None,
        })
        .expect("membership subject");
    let ExpressionNode::Name(original_path) = checked.typed.expression_table.expression(original)
    else {
        panic!("original name");
    };
    let display_members = original_path.members;
    let foreign_path = checked
        .typed
        .expression_table
        .iter_expressions()
        .find_map(|(_, node)| match node {
            ExpressionNode::Name(path) if checked.typed.symbols.name(path.symbol) == "other" => {
                Some(*path)
            }
            _ => None,
        })
        .expect("other binding");
    let mut forged = foreign_path;
    forged.members = display_members;
    let expression = checked
        .typed
        .expression_table
        .insert(ExpressionNode::Name(forged));
    let domain = checked
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("::Upper"))
        .expect("domain");
    assert!(
        !validation::has_exact_integer_domain_subject(&checked.typed, domain, expression),
        "a retained i32 binding cannot become the displayed u64 parameter"
    );
}
