use super::{State, StatementNode, TypedTrees, initializer};
use crate::checks::ranges::RangeFacts;
use crate::tests::front_end::typed_program;

/// The dependency walk carries an explicit depth bound (128) so an authored
/// expression tree deeper than the bound records the prefix it visited and
/// stops, rather than recursing to the bottom of the arena.
#[test]
fn dependency_recording_stops_at_the_depth_bound() {
    let terms = 200usize;
    let mut expression = "1".to_string();
    for _ in 1..terms {
        expression = format!("{expression} + 1");
    }
    let program = typed_program(&format!(
        "machine window(seed: i64) {{
        let cut: i64 = {expression};
        let live: i64 = seed;
    }}"
    ));
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert_eq!(
        facts.expression_dependencies.len(),
        super::super::EXPRESSION_WALK_DEPTH_BOUND
    );
}

/// The same bound gates the iterative leg: `integer_value_identity` unrolls
/// an immutable local-alias chain one hop per iteration, so a chain of
/// exactly the bound resolves and one link deeper stops unanswered rather
/// than walking to the leaf.
#[test]
fn local_alias_chain_unrolling_respects_the_depth_bound() {
    const BOUND: usize = super::super::EXPRESSION_WALK_DEPTH_BOUND;

    let chain = |links: usize| {
        let mut body = String::from("let a1: i64 = seed;\n");
        for i in 2..=links {
            body.push_str(&format!("let a{i}: i64 = a{};\n", i - 1));
        }
        body.push_str(&format!("let cut: i64 = a{links};\n"));
        typed_program(&format!("machine window(seed: i64) {{\n{body}}}"))
    };
    let seed_symbol = |program: &TypedTrees, state: &State| {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.name.as_str() == "seed")
            .expect("seed parameter")
            .symbol
    };
    let cut_expression = |program: &TypedTrees, state: &State| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == "cut" => {
                    Some(local.initial_value)
                }
                _ => None,
            })
            .expect("cut local")
    };

    // Exactly at the bound: the last iteration lands on the seed parameter.
    let program = chain(BOUND - 1);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    assert_eq!(
        super::super::captures::integer_value_identity(
            &program,
            state,
            cut_expression(&program, state),
        ),
        Some(seed_symbol(&program, state)),
    );

    // One link over: the walk exhausts the bound before reaching seed.
    let program = chain(BOUND);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    assert_eq!(
        super::super::captures::integer_value_identity(
            &program,
            state,
            cut_expression(&program, state),
        ),
        None,
    );
}
