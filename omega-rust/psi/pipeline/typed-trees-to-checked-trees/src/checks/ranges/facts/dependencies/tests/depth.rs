use super::initializer;
use crate::checks::ranges::RangeFacts;

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
    let program = super::typed_source(&format!(
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
    assert_eq!(facts.expression_dependencies.len(), 128);
}
