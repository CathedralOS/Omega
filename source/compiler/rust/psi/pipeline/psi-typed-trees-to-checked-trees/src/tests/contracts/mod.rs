use super::*;

mod assembly;
mod fact_call_projections;
mod indexing;
mod instantiation;
mod proof_obligations;
mod propositions;
mod qualification_evidence;
mod resultless_laws;
mod total_specification_arithmetic;

fn parse_typed_trees(source: &str) -> psi_typed_trees::TypedTrees {
    // The source loader supplies these canonical core declarations in real
    // compilations. This single-source unit harness installs the same service
    // identities directly so checked-asm rows exercise normalized reach.
    let source =
        format!("boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn outcome_specific_guarantee_reaches_separate_checked_carrier() {
    let typed = parse_typed_trees(
        r#"
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        ensures Outcome::Success -> { true; }
        { Outcome::Success }
        "#,
    );
    let outcome = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Outcome")
        .expect("Outcome data");
    let success = typed
        .data_members(outcome)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == "Success" =>
            {
                Some(variant)
            }
            _ => None,
        })
        .expect("Success case");
    let outcome_symbol = outcome.symbol;
    let success_symbol = success.symbol;
    let checked = lower_typed_trees(typed).expect("check guarded declaration stage");
    let mut rows = checked.facts.proof.outcome_specific_guarantees.iter();
    let (_, row) = rows.next().expect("one checked outcome-specific guarantee");
    assert!(
        rows.next().is_none(),
        "one checked outcome-specific guarantee"
    );
    assert_eq!(row.result_data, outcome_symbol);
    assert_eq!(row.result_case, success_symbol);
    assert!(row.public_selector.is_none());
    assert!(
        checked
            .facts
            .proof
            .contract_facts
            .iter()
            .all(|(_, fact)| { fact.fact != row.fact }),
        "guarded row must not enter the unconditional contract-fact lane"
    );
}
