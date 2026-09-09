use super::*;
use crate::flow::check_against_whole_pass as lower_typed_trees;

fn check(source: &str, accepted: bool, rejection: &str) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => assert!(accepted, "unproved contract accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection:?}: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn entry_self_transitions_reestablish_machine_preconditions() {
    for (replacement, accepted) in [("\"ok\"", true), ("[255, 0, 0, 0]", false)] {
        check(
            &format!(
                r#"
                domain [u8; 4]::Utf8 requires valid_utf8(self);
                machine rewrite(output: &mut [u8; 4])
                requires output in Utf8
                {{
                    output = {replacement};
                    transition {{ _ -> self }}
                }}
                "#
            ),
            accepted,
            "state arrival contract on self-transition",
        );
    }
}

#[test]
fn machine_entry_contract_follows_renamed_arguments_across_two_jumps() {
    check(
        r#"
        machine forwarding(items: &[u64], index: u64) -> u64
        requires index < items.len
        {
            transition { _ -> middle(items, index) }
            state middle(values: &[u64], position: u64) -> u64 {
                transition { _ -> read(values, position) }
            }
            state read(selected: &[u64], offset: u64) -> u64 { selected[offset] }
        }
    "#,
        true,
        "",
    );
}

#[test]
fn machine_entry_preconditions_remain_obligations_at_actual_calls() {
    for (argument, accepted) in [(0, true), (2, false)] {
        check(
            &format!(
                r#"
            machine restricted(index: u64) -> u64
            requires index < 2
            {{ index }}
            machine main() -> u64 {{ restricted({argument}) }}
        "#
            ),
            accepted,
            "requires",
        );
    }
}

#[test]
fn machine_entry_preconditions_remain_obligations_at_named_tail_calls() {
    for (argument, accepted) in [(0, true), (2, false)] {
        check(
            &format!(
                "machine restricted(index: u64) -> u64 requires index < 2 {{ index }}
                 machine main() -> u64 {{ transition {{ _ -> restricted({argument}) }} }}"
            ),
            accepted,
            "requires contract for call restricted",
        );
    }
}

#[test]
fn named_tail_calls_retain_machine_requirement_facts() {
    let typed = parse_typed_trees(
        "machine restricted(index: u64) -> u64 requires index < 2 { index }
         machine main() -> u64 { transition { _ -> restricted(2) } }",
    );
    let plan = proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &plan, &borrow);
    assert!(
        proof
            .contract_calls
            .iter()
            .any(|(_, call)| !call.requires.is_empty()),
        "named call retains its requirement roster"
    );
    let semantic = build_semantic_facts(&typed, &proof);
    assert!(
        semantic
            .contexts
            .iter()
            .any(|(_, context)| matches!(context.point, facts::ProgramPoint::CallRequires { .. })),
        "the roster reaches semantic call constraints"
    );
}

#[test]
fn tail_call_return_guarantees_do_not_leak_to_sibling_branches() {
    for (fallback, accepted) in [(7, true), (0, false)] {
        check(
            &format!(
                "machine produce() -> u64 ensures result == 7 {{ 7 }}
                 machine main(selected: bool) -> u64 ensures result == 7 {{
                     transition selected {{ true -> produce() false -> {fallback} }}
                 }}"
            ),
            accepted,
            "ensures",
        );
    }
}

#[test]
fn tail_calls_owe_the_selected_receivers_requirements() {
    for body in [
        "transition { _ -> other.restricted() }",
        "other.restricted()",
        "other.restricted(); 0",
    ] {
        for (requirement, accepted) in [("self.allowed", false), ("other.allowed", true)] {
            check(
                &format!(
                    "data Counter {{ allowed: bool; }}
                 machine Counter::restricted(&self) -> u64 requires self.allowed {{ 1 }}
                 machine Counter::main(&self, other: &Counter) -> u64
                 requires {requirement} {{ {body} }}"
                ),
                accepted,
                "requires contract for call restricted",
            );
        }
    }
}

#[test]
fn tail_call_requirements_use_live_guards_and_exact_actual_arguments() {
    for (argument, accepted) in [("position", true), ("other", false)] {
        check(
            &format!(
                "machine restricted(limit: u64, index: u64) -> u64
                 requires index < limit {{ index }}
                 machine main(bound: u64, position: u64, other: u64) -> u64 {{
                     transition position < bound {{
                         true -> restricted(bound, {argument})
                         false -> 0
                     }}
                 }}"
            ),
            accepted,
            "requires contract for call restricted",
        );
    }
}

#[test]
fn explicit_target_state_preconditions_remain_edge_obligations() {
    for (argument, accepted) in [(0, true), (2, false)] {
        check(
            &format!(
                r#"
            machine main() -> u64 {{
                transition {{ _ -> read({argument}) }}
                state read(position: u64) -> u64
                requires position < 2
                {{ position }}
            }}
        "#
            ),
            accepted,
            "requires contract",
        );
    }
}

#[test]
fn machine_entry_requires_are_not_internal_state_preconditions() {
    for parameter in ["entry", "unrelated"] {
        check(
            &format!(
                r#"
            machine main(entry: u64) -> u64
            requires entry == 7
            {{
                transition {{ _ -> finish(8) }}
                state finish({parameter}: u64) -> u64 {{ {parameter} }}
            }}
        "#
            ),
            true,
            "",
        );
    }
}

#[test]
fn normal_named_state_returns_still_owe_machine_ensures() {
    for (replacement, accepted) in [("\"okay\"", true), ("[255, 0, 0, 0]", false)] {
        check(
            &format!(
                r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine produce(output: &mut [u8; 4])
            ensures output in Utf8
            {{
                transition {{ _ -> finish(output) }}
                state finish(destination: &mut [u8; 4]) {{ destination = {replacement}; }}
            }}
        "#
            ),
            accepted,
            "ensures",
        );
    }
}

#[test]
fn machine_requires_semantic_assumptions_are_scoped_to_entry() {
    let typed = parse_typed_trees(
        r#"
        machine main(index: u64) -> u64
        requires index < 2
        {
            transition { _ -> finish(8) }
            state finish(index: u64) -> u64 { index }
        }
    "#,
    );
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("machine");
    let entry = typed
        .machine_states(machine)
        .first()
        .expect("entry state")
        .symbol;
    let point = facts::ProgramPoint::State {
        machine_symbol: machine.symbol,
        state_symbol: entry,
    };
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let belongs_to_machine = |fact: &facts::Fact| {
        matches!(
            fact.origin, facts::FactOrigin::MachineContract { machine_symbol }
                if machine_symbol == machine.symbol
        )
    };
    let assumptions: Vec<_> = semantic
        .facts
        .iter()
        .map(|(_, fact)| fact)
        .filter(|fact| belongs_to_machine(fact))
        .collect();
    assert!(
        !assumptions.is_empty(),
        "entry precondition must be retained"
    );
    assert!(
        assumptions.iter().all(|fact| fact.point == point),
        "machine requires are entry assumptions, not machine-wide state assumptions"
    );
    let contexts: Vec<_> = semantic
        .contexts
        .iter()
        .map(|(_, context)| context)
        .filter(|context| {
            semantic
                .context_view(context)
                .facts()
                .any(belongs_to_machine)
        })
        .collect();
    assert!(!contexts.is_empty(), "entry receives an assumption context");
    assert!(
        contexts.iter().all(|context| context.point == point),
        "internal state entry must not collect the machine precondition implicitly"
    );
}

#[test]
fn named_states_cannot_recover_entry_domains_after_mutating_calls() {
    for parameter in ["bytes", "destination"] {
        check(
            &format!(
                r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
            machine consume(bytes: &[u8; 4]) requires bytes in Utf8 {{}}
            machine main(bytes: &mut [u8; 4])
            requires bytes in Utf8
            {{
                corrupt(bytes);
                transition {{ _ -> finish(bytes) }}
                state finish({parameter}: &mut [u8; 4]) {{ consume({parameter}); }}
            }}
        "#
            ),
            false,
            "requires",
        );
    }
}

#[test]
fn internal_jumps_do_not_publish_machine_return_guarantees() {
    let typed = parse_typed_trees(
        r#"
        machine produce() -> u64
        ensures result == 7
        {
            transition { _ -> finish() }
            state finish() -> u64 { 7 }
        }
        machine consume() -> u64 { produce() }
    "#,
    );
    let producer = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "produce")
        .expect("producer")
        .symbol;
    let consumer = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("consumer")
        .symbol;
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    assert!(
        proof.contract_calls.iter().any(|(_, call)| {
            call.caller_machine_symbol == consumer
                && call.target_machine_symbol == producer
                && !call.ensures.is_empty()
        }),
        "an ordinary completed machine call supplies its ensures"
    );
    assert!(
        proof
            .contract_calls
            .iter()
            .filter(|(_, call)| call.caller_machine_symbol == producer)
            .all(|(_, call)| call.ensures.is_empty()),
        "an internal jump has not completed the machine and cannot supply its ensures"
    );
    let has_return_guarantee = |owner| {
        semantic.facts.iter().any(|(_, fact)| {
            matches!(fact.point, facts::ProgramPoint::CallEnsures { machine_symbol, .. }
            if machine_symbol == owner)
        })
    };
    assert!(
        has_return_guarantee(consumer),
        "ordinary call publishes live return evidence"
    );
    assert!(
        !has_return_guarantee(producer),
        "internal jump publishes no return evidence"
    );
}
