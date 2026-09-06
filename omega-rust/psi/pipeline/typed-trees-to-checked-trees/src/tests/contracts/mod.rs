use super::*;

mod anonymous_integer_exits;
mod assembly;
mod assigned_values;
mod bounded_arguments;
mod bounded_returns;
mod byte_sequences;
mod closed_boolean_calls;
mod contract_entailment_certificates;
mod fact_call_projections;
mod indexing;
mod instantiation;
mod nominal_parameter_fields;
mod proof_obligations;
mod propositions;
mod qualification_evidence;
mod resultless_laws;
mod scalar_exits;
mod state_entry_contracts;
mod total_specification_arithmetic;
mod transition_exits;

fn parse_typed_trees(source: &str) -> typed_trees::TypedTrees {
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
            typed_trees::data::DataMember::Variant(variant)
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

#[test]
fn outcome_specific_selected_term_is_available_in_matching_caller_arm() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }

        machine produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); }
        { selected = ConcreteEvidence; Outcome::Success }

        machine caller() {
            transition produce() {
                Outcome::Success { ; selected: local } -> consume(; local)
                Outcome::Failure { } -> {}
            }
            state consume() requires needed: ready() {}
        }
        "#,
    );
    let checked = lower_typed_trees(typed).expect("selected guarded term should bind in its arm");
    let arm = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .next()
        .map(|(_, arm)| arm)
        .expect("one matching caller arm");
    assert_eq!(arm.rows.len(), 1);
    let selected = arm.rows[0]
        .selected_term
        .expect("selected caller-local term");
    assert_eq!(
        checked.facts.proof.evidence_terms.get(selected).name,
        "local"
    );
}

#[test]
fn outcome_specific_selected_term_is_available_from_saved_immutable_call() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        proposition accepted(value: Outcome) evidence Evidence;

        machine produce() -> Outcome
        requires incoming: accepted(Outcome::Success)
        ensures Outcome::Success -> { selected: accepted(result); }
        { selected = incoming; Outcome::Success }

        machine caller()
        requires seed: accepted(Outcome::Success)
        {
            let saved: Outcome = produce(; seed);
            transition saved {
                Outcome::Success { ; selected: local } -> consume(saved; local)
                Outcome::Failure { } -> {}
            }
            state consume(value: Outcome) requires needed: accepted(value) {}
        }
        "#,
    );
    let checked =
        lower_typed_trees(typed).expect("saved immutable call should retain its selected term");
    let arm = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .next()
        .map(|(_, arm)| arm)
        .expect("one matching caller arm");
    assert!(arm.result_call_statement_index < arm.statement_index);
    let caller_state = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .and_then(|machine| checked.machine_states(machine).first())
        .expect("caller entry state");
    let typed_trees::statement::StatementNode::LocalData(saved) = &checked
        .statement_table
        .statements(caller_state.statement_nodes)[arm.result_call_statement_index]
    else {
        panic!("saved call statement must remain a local")
    };
    assert_eq!(saved.name.as_str(), "saved");
    let validity = &arm.rows[0].validity;
    assert_eq!(validity.result_occurrence, arm.result_expression);
    assert_eq!(validity.referenced_occurrences.len(), 1);
    let interface_scope = validity
        .evidence_interface_scope
        .as_ref()
        .expect("witness-bearing proposition retains its interface scope");
    assert_eq!(
        interface_scope.retained_occurrences,
        validity.referenced_occurrences
    );
    assert!(interface_scope.reference_regions.is_empty());
    assert!(
        checked
            .facts
            .semantic
            .contexts_at_point(facts::ProgramPoint::Statement {
                machine_symbol: arm.caller_machine_symbol,
                state_symbol: arm.caller_state_symbol,
                statement_index: arm.statement_index,
            })
            .flat_map(|context| context.facts())
            .any(|fact| matches!(
                fact.place,
                facts::FactPlace::Place(place)
                    if checked.facts.semantic.places.get(place).root
                        == facts::PlaceRoot::Symbol(saved.symbol)
            )),
        "the saved result occurrence must root the guarded validity context"
    );
    let selected = arm.rows[0]
        .selected_term
        .expect("selected caller-local term");
    assert_eq!(
        checked.facts.proof.evidence_terms.get(selected).name,
        "local"
    );
}

#[test]
fn outcome_specific_indexed_validity_retains_collection_and_index() {
    let checked = lower_typed_trees(parse_typed_trees(
        r#"
        trait Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        proposition accepted(result_value: Outcome, observed: i32) evidence Evidence;

        machine produce(items: &[i32], index: u64) -> Outcome
        requires incoming: accepted(Outcome::Success, items[index])
        ensures Outcome::Success -> { selected: accepted(result, items[index]); }
        { selected = incoming; Outcome::Success }

        machine caller(items: &[i32], index: u64)
        requires seed: accepted(Outcome::Success, items[index])
        {
            let saved: Outcome = produce(items, index; seed);
            transition saved {
                Outcome::Success { ; selected: local } -> consume(items, index, saved; local)
                Outcome::Failure { } -> {}
            }
            state consume(items: &[i32], index: u64, value: Outcome)
            requires needed: accepted(value, items[index]) {}
        }
        "#,
    ))
    .expect("indexed guarded fact should retain both the indexed place and selector value");
    let arm = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .next()
        .map(|(_, arm)| arm)
        .expect("one matching caller arm");
    assert_eq!(
        arm.rows[0].validity.referenced_occurrences.len(),
        3,
        "result, indexed collection place, and index selector are distinct occurrences"
    );
    let root_names = checked
        .facts
        .semantic
        .contexts_at_point(facts::ProgramPoint::Statement {
            machine_symbol: arm.caller_machine_symbol,
            state_symbol: arm.caller_state_symbol,
            statement_index: arm.statement_index,
        })
        .flat_map(|context| context.facts())
        .filter_map(|fact| match fact.place {
            facts::FactPlace::Place(place) => match checked.facts.semantic.places.get(place).root {
                facts::PlaceRoot::Symbol(symbol) => {
                    Some(crate::labels::symbol_name(&checked, symbol))
                }
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(root_names.iter().any(|name| name == "items"));
    assert!(root_names.iter().any(|name| name == "index"));
}

#[test]
fn outcome_specific_omitted_named_and_unnamed_rows_are_fact_only_in_matching_arm() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }

        machine produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); true; }
        { selected = ConcreteEvidence; Outcome::Success }

        machine caller() {
            transition produce() {
                Outcome::Success { ; } -> consume()
                Outcome::Failure { } -> {}
            }
            state consume() requires ready() {}
        }
        "#,
    );
    let checked = lower_typed_trees(typed)
        .expect("all matching guarded facts should publish without selected terms");
    let arm = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .next()
        .map(|(_, arm)| arm)
        .expect("one matching caller arm");
    assert_eq!(arm.rows.len(), 2);
    assert!(arm.rows.iter().all(|row| row.selected_term.is_none()));
    assert!(
        arm.rows
            .iter()
            .all(|row| row.validity.result_occurrence == arm.result_expression)
    );
    assert!(arm.rows[0].validity.evidence_interface_scope.is_some());
    assert!(arm.rows[1].validity.evidence_interface_scope.is_none());
}

#[test]
fn outcome_specific_validity_invalidates_only_on_referenced_writes() {
    for (mutated_field, should_invalidate) in [("value", true), ("other", false)] {
        let source = format!(
            r#"
            trait Evidence {{}}
            data Record {{ value: i32; other: i32; }}
            data Outcome [copy] {{ case Success; case Failure; }}
            proposition accepted(result_value: Outcome, observed: i32) evidence Evidence;

            machine produce(observed: i32) -> Outcome
            requires incoming: accepted(Outcome::Success, observed)
            ensures Outcome::Success -> {{ selected: accepted(result, observed); }}
            {{ selected = incoming; Outcome::Success }}

            machine caller(record: &mut Record)
            requires seed: accepted(Outcome::Success, record.value)
            {{
                let saved: Outcome = produce(record.value; seed);
                transition saved {{
                    Outcome::Success {{ ; selected: local }} -> consume(record, saved; local)
                    Outcome::Failure {{ }} -> {{}}
                }}
                state consume(target: &mut Record, value: Outcome)
                requires needed: accepted(value, target.value)
                {{ target.{mutated_field} = 7; }}
            }}
            "#,
        );
        let checked = lower_typed_trees(parse_typed_trees(&source))
            .expect("guarded evidence should satisfy the target before its write");
        let arm = checked
            .facts
            .proof
            .outcome_specific_arms
            .iter()
            .next()
            .map(|(_, arm)| arm)
            .expect("one matching caller arm");
        assert_eq!(
            arm.rows[0].validity.referenced_occurrences.len(),
            2,
            "result and observed parameter are structural occurrences"
        );
        let guarantee = checked
            .facts
            .proof
            .outcome_specific_guarantees
            .get(arm.rows[0].guarantee)
            .fact;
        let consume = checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .find(|state| state.name.as_str() == "consume")
            .expect("consume state");
        let call = checked
            .facts
            .flow
            .control
            .calls
            .iter()
            .find_map(|(_, call)| (call.target_symbol == consume.symbol).then_some(call))
            .expect("consume call flow");
        let invalidated = checked
            .facts
            .flow
            .invalidations
            .events
            .span_or_empty(call.invalidations)
            .iter()
            .any(|event| {
                matches!(
                    checked.facts.semantic.facts.get(event.fact).payload,
                    facts::FactPayload::ContractPropositionApplication { fact, .. }
                        if fact == guarantee
                )
            });
        assert_eq!(
            invalidated, should_invalidate,
            "mutation of `{mutated_field}` must follow exact referenced-place overlap"
        );
    }
}

#[test]
fn outcome_specific_validity_contexts_do_not_couple_independent_rows() {
    let checked = lower_typed_trees(parse_typed_trees(
        r#"
        trait Evidence {}
        data Record { value: i32; other: i32; }
        data Outcome [copy] { case Success; case Failure; }
        proposition left_accepted(result_value: Outcome, observed: i32) evidence Evidence;
        proposition right_accepted(result_value: Outcome, observed: i32) evidence Evidence;

        machine produce(left: i32, right: i32) -> Outcome
        requires left_in: left_accepted(Outcome::Success, left)
        requires right_in: right_accepted(Outcome::Success, right)
        ensures Outcome::Success -> {
            selected_left: left_accepted(result, left);
            selected_right: right_accepted(result, right);
        }
        {
            selected_left = left_in;
            selected_right = right_in;
            Outcome::Success
        }

        machine caller(record: &mut Record)
        requires left_seed: left_accepted(Outcome::Success, record.value)
        requires right_seed: right_accepted(Outcome::Success, record.other)
        {
            let saved: Outcome = produce(record.value, record.other; left_seed, right_seed);
            transition saved {
                Outcome::Success {
                    ; selected_left: local_left, selected_right: local_right
                } -> consume(record, saved; local_left, local_right)
                Outcome::Failure { } -> {}
            }
            state consume(target: &mut Record, value: Outcome)
            requires left_needed: left_accepted(value, target.value)
            requires right_needed: right_accepted(value, target.other)
            { target.value = 7; }
        }
        "#,
    ))
    .expect("independent guarded rows should satisfy the target before its write");
    let arm = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .next()
        .map(|(_, arm)| arm)
        .expect("one matching caller arm");
    assert_eq!(arm.rows.len(), 2);
    let guarantee_facts = arm
        .rows
        .iter()
        .map(|row| {
            checked
                .facts
                .proof
                .outcome_specific_guarantees
                .get(row.guarantee)
                .fact
        })
        .collect::<Vec<_>>();
    let consume = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.name.as_str() == "consume")
        .expect("consume state");
    let call = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(_, call)| (call.target_symbol == consume.symbol).then_some(call))
        .expect("consume call flow");
    let invalidated = checked
        .facts
        .flow
        .invalidations
        .events
        .span_or_empty(call.invalidations)
        .iter()
        .filter_map(
            |event| match checked.facts.semantic.facts.get(event.fact).payload {
                facts::FactPayload::ContractPropositionApplication { fact, .. }
                    if guarantee_facts.contains(&fact) =>
                {
                    Some(fact)
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert!(invalidated.contains(&guarantee_facts[0]));
    assert!(
        !invalidated.contains(&guarantee_facts[1]),
        "a write to the left row dependency must not invalidate the independent right row"
    );
}

#[test]
fn outcome_specific_fact_and_term_do_not_leak_to_sibling_arm() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        ConcreteEvidence: satisfies Evidence {}
        data Outcome [copy] { case Success; case Failure; }

        machine produce() -> Outcome
        ensures Outcome::Success -> { selected: ready(); }
        { selected = ConcreteEvidence; Outcome::Success }

        machine caller() {
            transition produce() {
                Outcome::Success { ; } -> {}
                Outcome::Failure { } -> consume()
            }
            state consume() requires ready() {}
        }
        "#,
    );
    let checked = lower_typed_trees(typed).expect("sibling arm remains independently checkable");
    let arms = checked
        .facts
        .proof
        .outcome_specific_arms
        .iter()
        .map(|(_, arm)| arm)
        .collect::<Vec<_>>();
    assert_eq!(
        arms.len(),
        1,
        "only the matching Success arm publishes rows"
    );
    let arm = arms[0];
    let guarantee = checked
        .facts
        .proof
        .outcome_specific_guarantees
        .get(arm.rows[0].guarantee)
        .fact;
    let caller_state = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "caller")
        .and_then(|machine| checked.machine_states(machine).first())
        .expect("caller entry state");
    let sibling = checked
        .statement_table
        .statements(caller_state.statement_nodes)
        .iter()
        .enumerate()
        .filter(|(_, statement)| {
            matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            )
        })
        .map(|(index, _)| index)
        .find(|index| *index != arm.statement_index)
        .expect("Failure sibling transition");
    assert!(
        checked
            .facts
            .semantic
            .contexts_at_point(facts::ProgramPoint::Statement {
                machine_symbol: arm.caller_machine_symbol,
                state_symbol: arm.caller_state_symbol,
                statement_index: sibling,
            })
            .all(|context| context.facts().all(|fact| !matches!(
                fact.payload,
                facts::FactPayload::ContractPropositionApplication { fact, .. }
                    if fact == guarantee
            ))),
        "matching-case guarantee must not be materialized at the sibling coordinate"
    );
}

#[test]
fn outcome_specific_selected_term_does_not_bind_in_a_sibling_arm() {
    let typed = parse_typed_trees(
        r#"
        trait Evidence {}
        data Outcome [copy] { case Success; case Failure; }
        proposition accepted(value: Outcome) evidence Evidence;

        machine produce() -> Outcome
        requires incoming: accepted(Outcome::Success)
        ensures Outcome::Success -> { selected: accepted(result); }
        { selected = incoming; Outcome::Success }

        machine caller()
        requires seed: accepted(Outcome::Success)
        {
            let saved: Outcome = produce(; seed);
            transition saved {
                Outcome::Success { ; selected: local } -> {}
                Outcome::Failure { } -> consume(saved; local)
            }
            state consume(value: Outcome) requires needed: accepted(value) {}
        }
        "#,
    );
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a selected term must not enter the sibling arm's proof namespace");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("local")
                && (diagnostic.message.contains("proof") || diagnostic.message.contains("evidence"))
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn outcome_specific_selector_rejects_wrong_case_and_noncall_origin() {
    for (source, expected) in [
        (
            r#"
            trait Evidence {}
            proposition ready() evidence Evidence;
            ConcreteEvidence: satisfies Evidence {}
            data Outcome [copy] { case Success; case Failure; }
            machine produce() -> Outcome
            ensures Outcome::Success -> { selected: ready(); }
            { selected = ConcreteEvidence; Outcome::Success }
            machine caller() {
                transition produce() {
                    Outcome::Success { } -> {}
                    Outcome::Failure { ; selected: local } -> {}
                }
            }
            "#,
            "is not a named guarantee of the matching result case",
        ),
        (
            r#"
            data Outcome { case Success; case Failure; }
            machine caller(value: Outcome) {
                transition value {
                    Outcome::Success { ; selected: local } -> {}
                    Outcome::Failure { } -> {}
                }
            }
            "#,
            "limited to one unambiguous direct call captured in an immutable local",
        ),
    ] {
        let typed = parse_typed_trees(source);
        let diagnostics = lower_typed_trees(typed).expect_err("invalid arm source must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }
}
mod boundary_exit_facts;
mod copied_fields;
