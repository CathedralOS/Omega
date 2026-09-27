use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

mod anonymous_array_landing;
mod anonymous_integer_exits;
mod anonymous_rational_arguments;
mod anonymous_rational_exits;
mod anonymous_rational_floats;
mod anonymous_record_landing;
mod anonymous_window_types;
mod assembly;
mod assigned_values;
mod boolean_call_results;
mod bounded_arguments;
mod bounded_returns;
mod byte_sequences;
mod call_ceilings;
mod call_result_guarantees;
mod closed_boolean_calls;
mod contract_entailment_certificates;
mod cyclic_header_invariants;
mod element_fields;
mod entry_requirement_crash_coverage;
mod fact_call_projections;
mod field_domain_writes;
mod indexing;
mod instantiation;
mod mutable_parameter_results;
mod nominal_parameter_fields;
mod proof_call_requirements;
mod proof_obligations;
mod propositions;
mod qualification_evidence;
mod result_case_membership;
mod resultless_laws;
mod scalar_exits;
mod scalar_result_bounds;
mod setter_requires;
mod state_entry_contracts;
mod state_field_values;
mod subslice_call_bounds;
mod total_specification_arithmetic;
mod transition_exits;
mod where_field_bounds;

fn parse_typed_trees(source: &str) -> typed_trees::TypedTrees {
    // The source loader supplies these canonical core declarations in real
    // compilations. This single-source unit harness installs the same service
    // identities directly so checked-asm rows exercise normalized reach.
    let source =
        format!("boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}");
    typed_program(&source)
}

/// Same prelude as `parse_typed_trees`, with the toolchain `core/binding.omg`
/// resident so fixtures can spell the intrinsic `Binding<R>` carrier, plus the
/// settled fused-service erasure authorizations
/// `bind_fixture_fused_service_erasures` supplies — without one an authored
/// `Binding<R>` field stays unshaped and the machine's unit plan fails closed.
fn parse_typed_trees_with_service(source: &str) -> typed_trees::TypedTrees {
    let mut typed = crate::tests::front_end::typed_program_with_core_service(&format!(
        "boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}"
    ));
    crate::tests::bind_fixture_fused_service_erasures(&mut typed);
    typed
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
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("nonmatching result has no guarded evidence lane");
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
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
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("every qualifying predecessor establishes the guarded term");
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("a dynamic sum result cannot silently count as a nonmatching case");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot classify the ordinary result case")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn outcome_specific_indexed_validity_retains_collection_and_index() {
    let checked = lower_typed_trees(
        parse_typed_trees(
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
        ),
        &CheckingRequest::settled(),
    )
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
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
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
fn outcome_specific_validity_contexts_do_not_couple_independent_rows() {
    let checked = lower_typed_trees(
        parse_typed_trees(
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
        ),
        &CheckingRequest::settled(),
    )
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
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
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
        let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
            .expect_err("invalid arm source must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }
}
mod boundary_exit_facts;

#[test]
fn outcome_evidence_retains_constructor_payload_values_and_field_associations() {
    for (incoming, result, accepted) in [
        (
            "Outcome::Success { left: 1u32, right: 2u32 }",
            "Outcome::Success { right: 2u32, left: 1u32 }",
            true,
        ),
        (
            "Outcome::Success { left: 1u32, right: 2u32 }",
            "Outcome::Success { left: 2u32, right: 1u32 }",
            false,
        ),
        (
            "Outcome::Success { left: 1u32, right: 2u32 }",
            "Outcome::Success { left: 1u32, right: 3u32 }",
            false,
        ),
    ] {
        let typed = parse_typed_trees(&format!(
            r#"
            trait Evidence {{}}
            data Outcome {{ case Success(left: u32, right: u32); case Failure; }}
            proposition accepted(value: Outcome) evidence Evidence;
            machine choose() -> Outcome
            requires incoming: accepted({incoming})
            ensures Outcome::Success -> {{ selected: accepted(result); }}
            {{ selected = incoming; {result} }}
        "#
        ));
        let result = lower_typed_trees(typed, &CheckingRequest::settled());
        if accepted {
            result.expect("same constructor field values in a different authored order");
        } else {
            let diagnostics = result
                .expect_err("different constructor field values cannot supply result evidence");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(
                        "does not inhabit its guarantee after substituting the concrete result"
                    )),
                "{diagnostics:?}"
            );
        }
    }
}
