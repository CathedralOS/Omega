//! A package lock reads the exact-owner identity of every type in a retained
//! policy to check that its owners belong to the lock graph. This renderer
//! writes that text and `language_semantics` reads it back, and the two used to
//! drift apart unnoticed until a lock was built from a full compile. Each
//! witness here renders one form through the real renderer and reads the text,
//! and the node-kind tables below fail to compile when the renderer learns a
//! form that no witness covers.

mod fixture;
mod kinds;
mod witnesses;

use self::fixture::Fixture;
use self::kinds::{
    ActionKind, ConstraintKind, DomainKind, ExpressionKind, LengthKind, OperatorKind, PatternKind,
    TypeKind, action_kind, constraint_kind, domain_kind, expression_kind, length_kind,
    operator_kind, pattern_kind, type_kind,
};
use self::witnesses::{Witness, constraint_witnesses, expression_witnesses, type_witnesses};
use crate::expression::{ExpressionNode, MatchPattern};
use crate::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
};
use std::collections::BTreeSet;

#[derive(Default)]
struct Owners {
    seen: BTreeSet<[u8; 32]>,
    embedded: Vec<String>,
    depth: usize,
}

impl TypeIdentityPackageOwnerVisitor for Owners {
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
        self.depth += 1;
        Ok(())
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }
    fn reserve(&mut self, _: usize) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }
    fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
        self.seen.insert(digest);
        Ok(())
    }
    fn embedded_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError> {
        self.embedded.push(name.to_owned());
        Ok(())
    }
}

fn read(identity: &str) -> (Result<(), TypeIdentityVisitError>, Owners) {
    let mut owners = Owners::default();
    let result = visit_type_identity_package_owners(identity, &mut owners);
    assert_eq!(owners.depth, 0, "every entered node is left: {identity}");
    (result, owners)
}

/// Every witness is rendered and read, and the failures are reported together so
/// one run names each form that has drifted, not only the first.
fn check(fixture: &Fixture, witnesses: &[Witness]) {
    let mut failures = Vec::new();
    for witness in witnesses {
        let text = fixture.render(witness.reference, &witness.binders, &witness.substitutions);
        let (result, owners) = read(&text);
        let expected = witness
            .owners
            .iter()
            .map(|package| [*package; 32])
            .collect::<BTreeSet<_>>();
        if result != Ok(()) || owners.seen != expected {
            failures.push(format!(
                "{}: {result:?}, {} of {} package owners visited\n    {text}",
                witness.form,
                owners.seen.len(),
                expected.len(),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} forms do not read back:\n{}",
        failures.len(),
        witnesses.len(),
        failures.join("\n"),
    );
}

#[test]
fn every_type_form_the_renderer_writes_reads_back() {
    let mut fixture = Fixture::new();
    let witnesses = type_witnesses(&mut fixture);
    check(&fixture, &witnesses);
}

#[test]
fn every_constraint_form_the_renderer_writes_reads_back() {
    let mut fixture = Fixture::new();
    let witnesses = constraint_witnesses(&mut fixture);
    check(&fixture, &witnesses);
}

#[test]
fn every_index_form_the_renderer_writes_reads_back() {
    let mut fixture = Fixture::new();
    let witnesses = expression_witnesses(&mut fixture);
    check(&fixture, &witnesses);
}

/// A witness for each kind is not enough if a kind were only ever built and
/// never rendered, so the kinds are read off the nodes that the witnesses made.
#[test]
fn witnesses_reach_every_node_kind() {
    let mut fixture = Fixture::new();
    let mut witnesses = type_witnesses(&mut fixture);
    witnesses.extend(constraint_witnesses(&mut fixture));
    witnesses.extend(expression_witnesses(&mut fixture));
    check(&fixture, &witnesses);

    let table = &fixture.program.type_reference_table;
    let mut types = BTreeSet::new();
    let mut constraints = BTreeSet::new();
    let mut domains = BTreeSet::new();
    let mut lengths = BTreeSet::new();
    for (_, node) in table.type_reference_rows() {
        types.insert(type_kind(node));
        match node {
            TypeReferenceNode::Constrained {
                constraints: span, ..
            } => {
                for constraint in table.constraints(*span) {
                    constraints.insert(constraint_kind(constraint));
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        domains.insert(domain_kind(&domain.subject));
                    }
                }
            }
            TypeReferenceNode::FixedArray { length, .. } => {
                lengths.insert(length_kind(length));
            }
            _ => {}
        }
    }
    assert_eq!(types, TypeKind::ALL.iter().copied().collect());
    assert_eq!(constraints, ConstraintKind::ALL.iter().copied().collect());
    assert_eq!(domains, DomainKind::ALL.iter().copied().collect());
    assert_eq!(lengths, LengthKind::ALL.iter().copied().collect());

    let table = &fixture.program.expression_table;
    let mut expressions = BTreeSet::new();
    let mut operators = BTreeSet::new();
    let mut patterns = BTreeSet::new();
    let mut actions = BTreeSet::new();
    for (_, node) in table.iter_expressions() {
        expressions.insert(expression_kind(node));
        match node {
            ExpressionNode::Binary(binary) => {
                operators.insert(operator_kind(&binary.operator));
            }
            ExpressionNode::Match(dispatch) => {
                for arm in table.match_arms(dispatch.arms) {
                    patterns.insert(pattern_kind(&arm.pattern));
                    if let MatchPattern::Destructure(pattern) = &arm.pattern {
                        for field in table.match_fields(pattern.fields) {
                            actions.insert(action_kind(&field.action));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(expressions, ExpressionKind::ALL.iter().copied().collect());
    assert_eq!(operators, OperatorKind::ALL.iter().copied().collect());
    assert_eq!(patterns, PatternKind::ALL.iter().copied().collect());
    assert_eq!(actions, ActionKind::ALL.iter().copied().collect());
}

/// An invalid array extent writes a marker beside no failure flag, so only the
/// grammar stops it from entering a retained policy.
#[test]
fn an_unflagged_marker_is_refused_with_its_reason() {
    let mut fixture = Fixture::new();
    let not_a_recipe = fixture.named("Payload");
    let array = fixture.array(FixedArrayLength::Expression(not_a_recipe));
    let text = fixture.render(array, &[], &[]);
    assert_eq!(
        read(&text).0,
        Err(TypeIdentityVisitError::Unadmitted(
            language_semantics::type_identity::vocabulary::Unadmitted::InvalidIdentity
        )),
        "{text}"
    );
}
