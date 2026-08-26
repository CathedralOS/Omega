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
    assert!(row.evidence_term.is_none());
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

#[test]
fn outcome_specific_named_and_unnamed_rows_discharge_on_matching_exit() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> {
            selected: ready();
            true;
        }
        {
            selected = incoming;
            Outcome::Success
        }
        "#,
    );
    let checked = lower_typed_trees(typed).expect("matching exit discharges guarded rows");
    let rows = checked
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .map(|(_, row)| row)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.evidence_term.is_some()));
    assert!(rows.iter().any(|row| row.evidence_term.is_none()));
    assert_eq!(checked.facts.proof.evidence_forwardings.iter().count(), 1);
}

#[test]
fn outcome_specific_named_row_checks_evidence_after_result_substitution() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        data Outcome { case Success; case Failure; }
        proposition accepted(value: Outcome) evidence Evidence;
        machine choose() -> Outcome
        requires incoming: accepted(Outcome::Success)
        ensures Outcome::Success -> { selected: accepted(result); }
        {
            selected = incoming;
            Outcome::Success
        }
        "#,
    );
    lower_typed_trees(typed)
        .expect("the named source exactly inhabits the concretely substituted guarantee");
}

#[test]
fn outcome_specific_named_row_rejects_wrong_substituted_evidence() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        data Outcome { case Success; case Failure; }
        proposition accepted(value: Outcome) evidence Evidence;
        machine choose() -> Outcome
        requires incoming: accepted(Outcome::Failure)
        ensures Outcome::Success -> { selected: accepted(result); }
        {
            selected = incoming;
            Outcome::Success
        }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the assigned term must match the concrete qualifying result");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not inhabit its guarantee after substituting the concrete result")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn outcome_specific_named_row_is_not_required_on_other_case() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        { Outcome::Failure }
        "#,
    );
    lower_typed_trees(typed).expect("nonmatching result has no guarded evidence lane");
}

#[test]
fn outcome_specific_named_row_requires_one_matching_assignment() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        { Outcome::Success }
        "#,
    );
    let diagnostics =
        lower_typed_trees(typed).expect_err("matching result must assign guarded named evidence");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "outcome-specific evidence `selected` is not definitely assigned on the matching ordinary exit"
    )), "unexpected diagnostics: {diagnostics:?}");
}

#[test]
fn outcome_specific_named_row_rejects_assignment_on_other_case() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        {
            selected = incoming;
            Outcome::Failure
        }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("nonmatching result must not assign guarded named evidence");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "outcome-specific evidence `selected` is assigned on a nonmatching ordinary exit"
            )),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn outcome_specific_named_row_rejects_duplicate_matching_assignment() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        {
            selected = incoming;
            selected = incoming;
            Outcome::Success
        }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a guarded output is assigned exactly once on a matching path");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("named ensures evidence `selected` is assigned more than once")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn outcome_specific_unnamed_row_substitutes_concrete_result() {
    let typed = parse_typed_trees(
        r#"
        data Outcome { case Success; case Failure; }
        proposition accepted(value: Outcome);
        machine choose() -> Outcome
        requires accepted(Outcome::Success)
        ensures Outcome::Success -> { accepted(result); }
        { Outcome::Success }
        "#,
    );
    lower_typed_trees(typed).expect("matching fact proves the substituted result proposition");
}

#[test]
fn outcome_specific_unnamed_row_substitutes_payload_constructor() {
    let typed = parse_typed_trees(
        r#"
        data Outcome { case Success(value: i32); case Failure; }
        proposition accepted(value: Outcome);
        machine choose() -> Outcome
        requires accepted(Outcome::Success { value: 7 })
        ensures Outcome::Success -> { accepted(result); }
        { Outcome::Success { value: 7 } }
        "#,
    );
    lower_typed_trees(typed)
        .expect("the full concrete payload constructor participates in result substitution");
}

#[test]
fn outcome_specific_unnamed_row_rejects_missing_matching_proof() {
    let typed = parse_typed_trees(
        r#"
        data Outcome { case Success; case Failure; }
        proposition accepted(value: Outcome);
        machine choose() -> Outcome
        ensures Outcome::Success -> { accepted(result); }
        { Outcome::Success }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("matching result must establish its guarded proposition");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove outcome-specific guarantee on the matching ordinary exit")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn outcome_specific_assignment_must_cover_every_qualifying_join_input() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose(left: bool) -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        {
            transition left {
                true -> assigned()
                false -> omitted()
            }
            state assigned() {
                selected = incoming;
                transition { _ -> joined() }
            }
            state omitted() { transition { _ -> joined() } }
            state joined() { Outcome::Success }
        }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("one predecessor cannot establish a guarded term for the whole join");
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
        "outcome-specific evidence `selected` is not definitely assigned on the matching ordinary exit through choose::joined"
    )), "unexpected diagnostics: {diagnostics:?}");
}

#[test]
fn outcome_specific_assignment_on_all_join_inputs_passes() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose(left: bool) -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        {
            transition left {
                true -> first()
                false -> second()
            }
            state first() {
                selected = incoming;
                transition { _ -> joined() }
            }
            state second() {
                selected = incoming;
                transition { _ -> joined() }
            }
            state joined() { Outcome::Success }
        }
        "#,
    );
    lower_typed_trees(typed).expect("every qualifying predecessor establishes the guarded term");
}

#[test]
fn outcome_specific_rows_need_no_lane_on_crash_exit() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose() -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); false; }
        crashes Abort
        { crash Abort; }
        "#,
    );
    lower_typed_trees(typed).expect("a crash exit has no result or guarded proof lane");
}

#[test]
fn outcome_specific_rows_reject_unclassified_dynamic_result() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        data Outcome { case Success; case Failure; }
        machine choose(value: Outcome) -> Outcome
        requires incoming: ready()
        ensures Outcome::Success -> { selected: ready(); }
        { value }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a dynamic sum result cannot silently count as a nonmatching case");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot classify the ordinary result case")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}
