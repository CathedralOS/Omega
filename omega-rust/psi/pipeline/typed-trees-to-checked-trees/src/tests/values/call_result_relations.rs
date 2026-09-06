use super::*;

const COST: &str = r#"
    machine cost(level: u32 [1..=10]) -> u32 [15..=60] {
        10 + level * 5
    }
"#;

const FIELD_COST: &str = r#"
    machine cost(level: u32 [0..=10]) -> u32 [10..=60] {
        10 + level * 5
    }
"#;

const MAYBE_COST: &str = r#"
    machine maybe_cost(level: u32 [0..=10]) -> u32 [10..=60]
        crashes Abort
    {
        transition level == 0 {
            true -> failed()
            false -> (10 + level * 5)
        }
        state failed() -> u32 [10..=60] { crash Abort; }
    }
"#;

fn accepts(source: &str) {
    lower_typed_trees(typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects_subtraction(source: &str) {
    let diagnostics = match lower_typed_trees(typed_trees(source)) {
        Ok(_) => panic!("an invalid call-result relation authorized subtraction: {source}"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("may overflow")),
        "the source must reach arithmetic checking: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn repeated_pure_call_satisfies_the_guarded_subtraction_task_source() {
    accepts(&format!(
        r#"
        {COST}
        data Main {{}}
        machine Main::main(&mut self, level: u32 [1..=10], xp: u32 [0..=1000])
            -> u32 [0..=1000]
        {{
            transition xp >= cost(level) {{
                true -> spend(level, xp)
                false -> (xp)
            }}
            state spend(&mut self, level: u32 [1..=10], xp: u32 [0..=1000])
                -> u32 [0..=1000]
                requires xp >= cost(level)
            {{ xp - cost(level) }}
        }}
        "#
    ));
}

#[test]
fn repeated_pure_call_relation_follows_renamed_reordered_state_arguments() {
    // No target Requires: this must transport the established guard itself.
    accepts(&format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], xp: u32 [0..=1000]) -> u32 [0..=1000] {{
            transition xp >= cost(level) {{
                true -> debit(xp, level)
                false -> (xp)
            }}
            state debit(available: u32 [0..=1000], rank: u32 [1..=10]) -> u32 [0..=1000] {{
                available - cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn named_state_call_result_requirement_is_an_independent_body_assumption() {
    // The unconditional delivery is established by the caller's entry Requires;
    // the target requirement names its own parameter telescope.
    accepts(&format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], xp: u32 [0..=1000]) -> u32 [0..=1000]
            requires xp >= cost(level)
        {{
            transition {{ _ -> debit(xp, level) }}
            state debit(available: u32 [0..=1000], rank: u32 [1..=10]) -> u32 [0..=1000]
                requires available >= cost(rank)
            {{ available - cost(rank) }}
        }}
        "#
    ));
}

#[test]
fn changed_call_arguments_do_not_inherit_the_guard_relation() {
    rejects_subtraction(&format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], other: u32 [1..=10], xp: u32 [0..=1000]) -> u32 {{
            transition xp >= cost(level) {{ true -> debit(other, xp) false -> (xp) }}
            state debit(rank: u32 [1..=10], available: u32 [0..=1000]) -> u32 {{
                available - cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn same_spelled_nonentry_parameters_do_not_reuse_machine_entry_call_relations() {
    rejects_subtraction(&format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], other: u32 [1..=10], xp: u32 [0..=1000]) -> u32
            requires xp >= cost(level)
        {{
            transition {{ _ -> debit(other, xp) }}
            state debit(level: u32 [1..=10], xp: u32 [0..=1000]) -> u32 {{
                xp - cost(level)
            }}
        }}
        "#
    ));
}

#[test]
fn same_spelled_nonentry_places_do_not_reuse_machine_entry_relations() {
    // Adjacent legacy dependent_relations fallback must not authorize the new
    // call-result rule through a display-name-only entry bridge.
    rejects_subtraction(
        r#"
        machine spend(left: u32, right: u32) -> u32 requires left >= right {
            transition { _ -> debit(right, left) }
            state debit(left: u32, right: u32) -> u32 { left - right }
        }
        "#,
    );
}

#[test]
fn mutating_a_call_input_invalidates_its_repeated_result_relation() {
    rejects_subtraction(&format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], xp: u32 [0..=1000]) -> u32 {{
            transition xp >= cost(level) {{ true -> debit(level, xp) false -> (xp) }}
            state debit(mut rank: u32 [1..=10], available: u32 [0..=1000]) -> u32 {{
                rank = 10;
                available - cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn effectful_repeated_calls_do_not_mint_a_stable_result_relation() {
    rejects_subtraction(&format!(
        r#"
        {FIELD_COST}
        data Main {{ observed: u32 [0..=10]; }}
        machine Main::observe_cost(&mut self, level: u32 [0..=10]) -> u32 [10..=60] {{
            self.observed = level;
            cost(level)
        }}
        machine Main::spend(&mut self, level: u32 [0..=10], xp: u32 [0..=1000]) -> u32 {{
            self.observed = level;
            transition xp >= self.observe_cost(level) {{
                true -> debit(level, xp)
                false -> (xp)
            }}
            state debit(&mut self, rank: u32 [0..=10], available: u32 [0..=1000]) -> u32 {{
                available - self.observe_cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn distinct_same_signature_callees_do_not_share_a_result_relation() {
    rejects_subtraction(&format!(
        r#"
        {COST}
        machine other_cost(level: u32 [1..=10]) -> u32 [15..=60] {{ 60 }}
        machine spend(level: u32 [1..=10], xp: u32 [0..=1000]) -> u32 {{
            transition xp >= cost(level) {{ true -> debit(level, xp) false -> (xp) }}
            state debit(rank: u32 [1..=10], available: u32 [0..=1000]) -> u32 {{
                available - other_cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn disjoint_self_field_write_preserves_a_pure_call_input_relation() {
    accepts(&format!(
        r#"
        {FIELD_COST}
        data Main {{ level: u32 [0..=10]; marker: u32; }}
        machine Main::spend(&mut self, initial_level: u32 [0..=10], xp: u32 [0..=1000]) -> u32 [0..=1000] {{
            self.level = initial_level;
            transition xp >= cost(self.level) {{ true -> debit(xp) false -> (xp) }}
            state debit(&mut self, available: u32 [0..=1000]) -> u32 [0..=1000] {{
                self.marker = 0;
                available - cost(self.level)
            }}
        }}
        "#
    ));
}

#[test]
fn an_unguarded_second_predecessor_removes_a_call_result_relation() {
    rejects_subtraction(&format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], xp: u32 [0..=1000], gate: bool) -> u32 {{
            transition gate {{ true -> guarded(level, xp) false -> debit(level, xp) }}
            state guarded(rank: u32 [1..=10], available: u32 [0..=1000]) -> u32 {{
                transition available >= cost(rank) {{
                    true -> debit(rank, available)
                    false -> (available)
                }}
            }}
            state debit(rank: u32 [1..=10], available: u32 [0..=1000]) -> u32 {{
                available - cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn saved_scalar_call_input_survives_a_later_mutating_state_argument() {
    // Explicitly capture the scalar before the call: source lowering currently
    // hoists nested argument calls before the jump's complete argument list.
    // The saved value is independent of the subsequently changed field.
    accepts(&format!(
        r#"
        {FIELD_COST}
        data Main {{ level: u32 [0..=10]; }}
        machine overwrite(value: &mut u32 [0..=10]) -> u32 {{ value = 10; 0 }}
        machine Main::spend(&mut self, initial_level: u32 [0..=10], xp: u32 [0..=1000]) -> u32 [0..=1000] {{
            self.level = initial_level;
            let saved: u32 [0..=10] = self.level;
            transition xp >= cost(saved) {{
                true -> debit(saved, xp, overwrite(&mut self.level))
                false -> (xp)
            }}
            state debit(rank: u32 [0..=10], available: u32 [0..=1000], ignored: u32)
                -> u32 [0..=1000]
            {{ available - cost(rank) }}
        }}
        "#
    ));
}

#[test]
fn changed_field_argument_is_not_the_pre_mutation_call_input_snapshot() {
    rejects_subtraction(&format!(
        r#"
        {FIELD_COST}
        data Main {{ level: u32 [0..=10]; }}
        machine overwrite(value: &mut u32 [0..=10]) -> u32 {{ value = 10; 0 }}
        machine Main::spend(&mut self, initial_level: u32 [0..=10], xp: u32 [0..=1000]) -> u32 {{
            self.level = initial_level;
            transition xp >= cost(self.level) {{
                true -> debit(overwrite(&mut self.level), self.level, xp)
                false -> (xp)
            }}
            state debit(ignored: u32, rank: u32 [0..=10], available: u32 [0..=1000]) -> u32 {{
                available - cost(rank)
            }}
        }}
        "#
    ));
}

#[test]
fn unguarded_arrival_must_establish_the_named_state_call_result_requirement() {
    let source = format!(
        r#"
        {COST}
        machine spend(level: u32 [1..=10], xp: u32 [0..=1000]) -> u32 [0..=1000] {{
            transition {{ _ -> debit(xp, level) }}
            state debit(available: u32 [0..=1000], rank: u32 [1..=10]) -> u32 [0..=1000]
                requires available >= cost(rank)
            {{ available - cost(rank) }}
        }}
        "#
    );
    let diagnostics = match lower_typed_trees(typed_trees(&source)) {
        Ok(_) => panic!("an unguarded arrival cannot establish the target requirement"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call debit")
        }),
        "{source}\n{diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("may overflow")),
        "the body must check under its own requirement even when an arrival fails: {diagnostics:#?}"
    );
}

#[test]
fn a_crashing_scalar_call_is_not_a_total_requires_term() {
    let source = format!(
        r#"
        {MAYBE_COST}
        machine spend(level: u32 [0..=10], xp: u32 [0..=1000]) -> u32
            requires xp >= maybe_cost(level)
            crashes Abort
        {{ xp - maybe_cost(level) }}
        "#
    );
    let diagnostics = match lower_typed_trees(typed_trees(&source)) {
        Ok(_) => panic!("a potentially crashing call cannot denote a total requirement term"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("not denotational")
                || diagnostic.message.contains("not total")
        }),
        "the contract must reject call totality, not an unrelated arithmetic bound: {source}\n{diagnostics:#?}"
    );
}

#[test]
fn runtime_repeated_call_relation_is_conditional_on_normal_return() {
    // Unlike a contract term, the executable guard may terminate with Abort.
    // On its normal-return branch the same pure inputs give the same result;
    // the enclosing machine separately publishes the possible crash.
    accepts(&format!(
        r#"
        {MAYBE_COST}
        machine spend(level: u32 [0..=10], xp: u32 [0..=1000]) -> u32 [0..=1000]
            crashes Abort
        {{
            transition xp >= maybe_cost(level) {{
                true -> debit(level, xp)
                false -> (xp)
            }}
            state debit(rank: u32 [0..=10], available: u32 [0..=1000]) -> u32 [0..=1000] {{
                available - maybe_cost(rank)
            }}
        }}
        "#
    ));
}
