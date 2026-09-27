use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

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
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
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
    let diagnostics = match lower_typed_trees(typed_program(&source), &CheckingRequest::settled()) {
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
    let diagnostics = match lower_typed_trees(typed_program(&source), &CheckingRequest::settled()) {
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
