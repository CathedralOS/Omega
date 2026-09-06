use super::*;
use typed_trees::statement::StatementNode;

fn bounds(
    declaration: &str,
    carrier: &str,
    update: &str,
    lower: Option<i64>,
    upper: Option<i64>,
) -> Option<(Option<i64>, Option<i64>)> {
    let source = format!(
        "{declaration}
        data Counter {{ value: {carrier}; }}
        machine Counter::update(&mut self) {{ self.value = {update}; }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("monotonic_counter.omg".into(), source.clone())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let [machine] = program.machines() else {
        panic!("one attached update machine");
    };
    let state = &program.machine_states(machine)[0];
    let assignment = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Assignment(assignment) => Some(assignment),
            _ => None,
        })
        .expect("the authored field assignment");
    let target = crate::exact_self_field(&program, machine, assignment.target)
        .expect("assignment target retains its exact attached field");
    let ExpressionNode::Binary(binary) = program.expression_table.expression(assignment.value)
    else {
        panic!("the authored binary update");
    };
    let counter = [binary.left, binary.right]
        .into_iter()
        .find_map(|operand| crate::exact_self_field(&program, machine, operand))
        .expect("one operand is the same attached counter");
    assert!(target.symbol.is_valid());
    assert_eq!(target.symbol, counter.symbol);
    builtin_monotonic_integer_update_bounds(
        &program,
        machine,
        state,
        assignment.value,
        lower,
        upper,
    )
}

#[test]
fn signed_wrapping_increment_uses_the_independent_upper_bound_and_carrier_floor() {
    for update in ["self.value + 1", "1 + self.value"] {
        assert_eq!(
            bounds("", "i32 in Wrapping", update, None, Some(3)),
            Some((Some(i64::from(i32::MIN) + 1), Some(4)))
        );
    }
}

#[test]
fn signed_wrapping_increment_rejects_a_possible_wrap() {
    assert_eq!(
        bounds(
            "",
            "i32 in Wrapping",
            "self.value + 2147483647",
            Some(2),
            Some(2)
        ),
        None
    );
    assert_eq!(
        bounds("", "i32 in Wrapping", "self.value + 1", Some(0), None),
        None
    );
}

#[test]
fn unsigned_wrapping_decrement_uses_the_independent_lower_bound() {
    assert_eq!(
        bounds("", "u32 in Wrapping", "self.value - 1", Some(1), None),
        Some((Some(0), Some(i64::from(u32::MAX) - 1)))
    );
    assert_eq!(
        bounds("", "u32 in Wrapping", "self.value - 3", Some(2), Some(2)),
        None
    );
}

#[test]
fn u64_wrapping_does_not_invent_a_finite_upper_endpoint() {
    assert_eq!(
        bounds("", "u64 in Wrapping", "self.value + 1", Some(1), None),
        None
    );
    assert_eq!(
        bounds("", "u64 in Wrapping", "self.value - 1", Some(1), None),
        Some((Some(0), None))
    );
    assert_eq!(
        bounds("", "u64 in Wrapping", "self.value - 1", Some(0), None),
        None
    );
}

#[test]
fn saturating_updates_clamp_instead_of_intersecting_away_the_result() {
    assert_eq!(
        bounds(
            "",
            "i8 in Saturating",
            "self.value + 1",
            Some(127),
            Some(127)
        ),
        Some((Some(127), Some(127)))
    );
    assert_eq!(
        bounds(
            "",
            "i8 in Saturating",
            "self.value - 1",
            Some(-128),
            Some(-128)
        ),
        Some((Some(-128), Some(-128)))
    );
}

#[test]
fn exact_and_trapping_updates_describe_only_normal_completion() {
    for carrier in ["i8", "i8 in Trapping"] {
        assert_eq!(
            bounds("", carrier, "self.value + 1", Some(126), Some(127)),
            Some((Some(127), Some(127)))
        );
        assert_eq!(
            bounds("", carrier, "self.value - 1", Some(-128), Some(-127)),
            Some((Some(-128), Some(-128)))
        );
        assert_eq!(
            bounds("", carrier, "self.value + 1", Some(127), Some(127)),
            None
        );
        assert_eq!(
            bounds("", carrier, "self.value - 1", Some(-128), Some(-128)),
            None
        );
    }
}

#[test]
fn only_the_actual_builtin_update_has_primitive_monotonicity() {
    assert_eq!(
        bounds(
            "operator + i32::custom(left: i32 in Wrapping, right: i32 in Wrapping) -> i32 in Wrapping;",
            "i32 in Wrapping",
            "self.value + 1",
            Some(0),
            Some(3)
        ),
        None
    );
    assert_eq!(
        bounds(
            "operator + f64::unrelated(left: f64, right: f64) -> f64;",
            "i32 in Wrapping",
            "self.value + 1",
            Some(0),
            Some(3)
        ),
        Some((Some(1), Some(4)))
    );
}

#[test]
fn contradictory_input_bounds_do_not_authorize_an_update() {
    assert_eq!(
        bounds("", "i32 in Wrapping", "self.value + 1", Some(4), Some(3)),
        None
    );
}
