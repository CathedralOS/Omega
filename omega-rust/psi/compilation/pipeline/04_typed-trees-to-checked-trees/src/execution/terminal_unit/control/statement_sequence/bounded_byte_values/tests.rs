//! A bounded byte local constructed from a literal establishes the declared
//! carrier and exact declared qualifications; raw arrays keep their own producer.
use crate::tests::front_end::checked_program_result;
use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};

const DOMAIN: &str = "domain [u8; 8]::Utf8(bytes) requires valid_utf8(bytes);";

fn checked(source: &str) -> CheckedTrees {
    checked_program_result(&format!("{DOMAIN}\n{source}"))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn machine_named(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

/// Every literal establishment the machine's single-body plan retains, as
/// (statement, carrier identity, octets).
fn literal_establishments(checked: &CheckedTrees, name: &str) -> Vec<(u32, String, Vec<u8>)> {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(checked, name))
        .map(|plan| {
            plan.operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishByteSequence {
                        result, value, ..
                    } => Some((
                        result.statement_index,
                        result.type_identity.clone(),
                        value.literal_bytes()?.to_vec(),
                    )),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A literal shorter than the capacity keeps its own octets: the plan carries
/// "150", not eight bytes, and names the same carrier identity a borrowed
/// `&[u8; 8] in Utf8` formal registers, which a later call loan must match.
#[test]
fn a_shorter_literal_establishes_the_declared_bounded_carrier() {
    let checked = checked(
        r#"
        machine hold() {
            let text: [u8; 8] in Utf8 = "150";
        }
        machine peek(text: &[u8; 8] in Utf8) {
        }
    "#,
    );
    let formal = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "peek"))
        .expect("a borrowed bounded formal plans")
        .structural_parameters[0]
        .type_identity
        .clone();
    assert_eq!(
        literal_establishments(&checked, "hold"),
        vec![(0, formal, b"150".to_vec())]
    );
}

/// A raw `[u8; N]` remains an exact-width fixed array; a mutable bounded local
/// starts with the same exact literal establishment as an immutable owner.
#[test]
fn mutable_bounded_literals_keep_their_declared_qualification_roster() {
    let checked = checked(
        r#"
        machine raw() {
            let bytes: [u8; 3] = "abc";
        }
        machine replaceable() {
            let mut text: [u8; 8] in Utf8 = "150";
        }
    "#,
    );
    assert!(literal_establishments(&checked, "raw").is_empty());
    let literals = literal_establishments(&checked, "replaceable");
    assert_eq!(literals.len(), 1);
    assert_eq!(literals[0].2, b"150");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "replaceable"))
        .unwrap();
    let qualifications = plan
        .operations
        .iter()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishByteSequence { qualifications, .. } => {
                Some(qualifications)
            }
            _ => None,
        })
        .expect("one bounded literal establishment among the local's operations");
    let utf8 = checked
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("Utf8"))
        .expect("declared Utf8");
    assert_eq!(qualifications, &[utf8.semantic_id]);
}

#[test]
fn bounded_byte_domain_family_keeps_distinct_carriers_and_rejects_pair_conflicts() {
    let checked = checked(
        r#"
        domain [u8; 10]::Utf8(bytes) requires valid_utf8(bytes);
        machine hold() {
            let digits: [u8; 10] in Utf8 = "0123456789";
            let mut line: [u8; 8] in Utf8 = "ab";
            let other: [u8; 8] in Utf8 = "cd";
        }
    "#,
    );
    let literals = literal_establishments(&checked, "hold");
    assert_eq!(
        literals.len(),
        3,
        "both capacities establish without changing the source"
    );
    assert_ne!(literals[0].1, literals[1].1);
    assert_eq!(literals[1].1, literals[2].1);
    let domains = &checked.facts.flow.terminal_unit_effects.structural_domains;
    assert_eq!(domains.len(), 2, "repeated exact carrier is deduplicated");
    assert_eq!(domains[0].domain, domains[1].domain);
    assert_eq!(
        domains[0].identity, domains[1].identity,
        "the family identity is unchanged"
    );
    assert_ne!(
        domains[0].carrier_type_identity,
        domains[1].carrier_type_identity
    );
    for literal in &literals {
        assert!(
            domains
                .iter()
                .any(|domain| domain.carrier_type_identity == literal.1)
        );
    }

    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        unreachable!()
    };
    let mut shapes = crate::execution::terminal_unit::types::ShapeCollector::new(&checked.typed);
    let qualifications = crate::execution::terminal_unit::types::parameter_qualifications(
        &checked.typed,
        &mut shapes,
        local.type_reference,
        &[],
    )
    .unwrap();
    assert_eq!(qualifications, vec![domains[0].domain]);
    shapes.domains[0].byte_predicate = None;
    assert!(
        shapes
            .add_domain(qualifications[0], local.type_reference, &[])
            .is_none(),
        "a conflicting description of the exact family/carrier pair must reject"
    );
}

#[test]
fn bounded_byte_domain_catalog_uses_the_selected_carrier_declaration() {
    let checked = checked(
        r#"
        domain [u8; 10]::Utf8(bytes) requires valid_utf8(bytes);
        machine hold() {
            let digits: [u8; 10] in Utf8 = "0123456789";
            let text: [u8; 8] in Utf8 = "ab";
        }
    "#,
    );
    let literals = literal_establishments(&checked, "hold");
    assert_eq!(literals.len(), 2);
    let domains = &checked.facts.flow.terminal_unit_effects.structural_domains;
    for (literal, predicate) in literals.iter().zip([
        language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8,
        language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8,
    ]) {
        let domain = domains
            .iter()
            .find(|domain| domain.carrier_type_identity == literal.1)
            .unwrap();
        assert_eq!(
            domain.byte_predicate,
            Some(predicate),
            "the family does not replace the selected declaration"
        );
    }
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        unreachable!()
    };
    let carrier = local.type_reference;
    let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
        checked.type_reference_table.type_reference(carrier)
    else {
        unreachable!()
    };
    let [typed_trees::types::TypeConstraintNode::Domain(constraint)] =
        checked.type_reference_table.constraints(*constraints)
    else {
        unreachable!()
    };
    for ambiguous in [false, true] {
        let mut program = checked.typed.clone();
        let mut forged = constraint.clone();
        if !ambiguous {
            forged.symbol = symbols::SymbolHandle::invalid();
        }
        let mut rows = vec![typed_trees::types::TypeConstraintNode::Domain(
            forged.clone(),
        )];
        if ambiguous {
            rows.push(typed_trees::types::TypeConstraintNode::Domain(forged));
        }
        let forged_constraints = program.type_reference_table.insert_constraints(rows);
        program
            .type_reference_table
            .set_constraint_span(carrier, forged_constraints);
        let mut shapes = crate::execution::terminal_unit::types::ShapeCollector::new(&program);
        assert!(
            shapes
                .add_domain(constraint.semantic_id, carrier, &[])
                .is_none(),
            "an invalid or ambiguous authored declaration cannot fall back to its semantic family"
        );
    }
}
