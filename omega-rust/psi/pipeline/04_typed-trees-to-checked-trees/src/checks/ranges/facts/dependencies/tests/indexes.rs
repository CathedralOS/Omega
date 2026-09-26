use super::{ExpressionHandle, ExpressionNode, StatementNode, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::checks::ranges::facts::dependencies::tests::selected_operator_facts;
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use typed_trees::machine::Machine;
use typed_trees::state::State;

fn index_source(declaration: &str, selector: &str) -> TypedTrees {
    typed_program(&format!(
        "{declaration}
        machine window(items: &[i64; 4], selectors: &[u64; 4], index: u64, unrelated: u64) {{
            let cut: i64 = items[{selector}];
        }}"
    ))
}

fn window(program: &TypedTrees) -> (&Machine, &State) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "window")
        .expect("window");
    (machine, &program.machine_states(machine)[0])
}

#[test]
fn missing_selector_expression_or_symbol_does_not_establish_complete_reads() {
    for remove_expression in [false, true] {
        let mut program = index_source("", "index");
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let expression = initializer(&program, state);
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            panic!("index fixture");
        };
        let selector = indexed.index;
        if remove_expression {
            let ExpressionNode::Indexed(indexed) =
                program.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            indexed.index = ExpressionHandle::invalid();
        } else {
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(selector)
            else {
                panic!("named selector")
            };
            path.head_symbol = SymbolHandle::invalid();
            path.symbol = SymbolHandle::invalid();
            path.member_symbols = arena::HandleSpan::empty();
        }
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.bound_program = Some(&program);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(facts.expression_dependencies[0].reads.is_none());
    }
}

#[test]
fn a_second_use_row_disagreeing_with_the_selection_is_inconsistent_custody() {
    let program = index_source(
        "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;",
        "index",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut operators = selected_operator_facts(&program);
    let mut duplicate = *operators
        .uses
        .iter()
        .find_map(|(_, row)| (row.expression == expression).then_some(row))
        .expect("checked use row");
    duplicate.status = checked_trees::CheckedOperatorResolutionStatus::Ambiguous;
    operators.uses.append(duplicate);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

fn statement_index_of(program: &TypedTrees, state: &State, name: &str) -> usize {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.name.as_str() == name)
        })
        .expect("named local statement")
}

#[test]
fn a_selected_index_operator_with_an_open_range_stays_incomplete() {
    let program = typed_program(
        "boundary operator [..] Slice::window(items: &[i64], start: u64, end: u64) -> i64;
        machine window(items: &[i64; 4], low: u64, unrelated: u64) {
            let cut: i64 = items[low..];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// Constant bounds collapse the window to resolved `start..end` coordinates:
/// the footprint is the window place alone, and an element write outside the
/// extent is proven disjoint instead of retiring the window's facts.
#[test]
fn a_constant_range_window_keeps_its_exact_extent() {
    let program = typed_program(
        "machine window(items: &[i64; 4], unrelated: u64) {
            let cut: &[i64] = items[0..2];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("constant window reads");
    let mut window = parameter_place(&program, state, "items");
    window
        .segments
        .push(facts::PlaceSegment::FixedRange { start: 0, end: 2 });
    assert_eq!(reads.as_slice(), [window].as_slice());
    let label = program.expression_table.display_name(expression);
    let write_at = |index: usize| {
        let mut place = parameter_place(&program, state, "items");
        place
            .segments
            .push(facts::PlaceSegment::FixedIndex { index });
        place
    };
    let write_range = |start: usize, end: usize| {
        let mut place = parameter_place(&program, state, "items");
        place
            .segments
            .push(facts::PlaceSegment::FixedRange { start, end });
        place
    };
    for (write, survives) in [
        (write_at(0), false),
        (write_at(3), true),
        (write_range(1, 4), false),
        // An adjacent window is disjoint, not overlapping.
        (write_range(2, 3), true),
        (parameter_place(&program, state, "items"), false),
        (parameter_place(&program, state, "unrelated"), true),
    ] {
        assert_eq!(
            facts
                .preserved_expression_labels(
                    &program,
                    machine,
                    state,
                    Some(std::slice::from_ref(&write)),
                )
                .contains(&label),
            survives,
            "write to {write:?}"
        );
    }
}

fn arithmetic_window_source(selector: &str) -> TypedTrees {
    typed_program(&format!(
        "operator + u64::custom(left: u64, right: u64) -> u64;
        machine window(items: &[i64; 4], low: u64, step: u64, high: u64, unrelated: u64) {{
            let cut: &[i64] = items[{selector}];
        }}"
    ))
}

/// A point selector with authored arithmetic over a local leaf is not
/// hoisted, so the application stays inline in the builtin element place.
/// Its reads are the operands plus the element place, and the place algebra
/// keeps the non-constant selector conservative: a fixed-element write
/// still retires the facts because the selected declaration's value is
/// unknown.
#[test]
fn a_selected_arithmetic_point_selector_reads_its_operands_and_stays_conservative() {
    let program = typed_program(
        "operator + u64::custom(left: u64, right: u64) -> u64;
        machine window(items: &[i64; 4], low: u64, unrelated: u64) {
            let offset: u64 = 1u64;
            let cut: i64 = items[low + offset];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let offset = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "offset" => {
                Some(CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(local.symbol),
                    segments: Vec::new(),
                })
            }
            _ => None,
        })
        .expect("offset local");
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("index fixture")
    };
    assert!(matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Binary(_)
    ));
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.checked_operators = Some(&operators);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("selected arithmetic selector reads");
    let mut element = parameter_place(&program, state, "items");
    element.segments.push(facts::PlaceSegment::Index {
        expression: indexed.index,
    });
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "low"),
            offset.clone(),
            element,
        ]
        .as_slice()
    );
    let label = program.expression_table.display_name(expression);
    let mut fixed_element = parameter_place(&program, state, "items");
    fixed_element
        .segments
        .push(facts::PlaceSegment::FixedIndex { index: 3 });
    for (write, survives) in [
        (parameter_place(&program, state, "items"), false),
        (fixed_element, false),
        (parameter_place(&program, state, "low"), false),
        (offset, false),
        (parameter_place(&program, state, "unrelated"), true),
    ] {
        assert_eq!(
            facts
                .preserved_expression_labels(
                    &program,
                    machine,
                    state,
                    Some(std::slice::from_ref(&write)),
                )
                .contains(&label),
            survives,
            "write to {write:?}"
        );
    }
}

/// A constant-shaped authored application (`1u64 + 0u64`) is refused
/// independently of custody: the place algebra folds it syntactically to
/// builtin arithmetic's coordinate, which the selected declaration never
/// established. Production records no use row for literal-only operands,
/// so the row is fabricated from a resolved donor occurrence to prove that
/// the fold guard, not missing custody, keeps the point selector and the
/// window bound incomplete.
#[test]
fn a_constant_shaped_selected_arithmetic_application_stays_incomplete() {
    for (declared, selector) in [("i64", "1u64 + 0u64"), ("&[i64]", "1u64 + 0u64..high")] {
        let program = typed_program(&format!(
            "operator + u64::custom(left: u64, right: u64) -> u64;
            machine window(items: &[i64; 4], low: u64, step: u64, high: u64) {{
                let cut: {declared} = items[{selector}];
                let donor: u64 = low + step;
            }}"
        ));
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        assert_eq!(statement_index_of(&program, state, "cut"), 0);
        let application = match program.expression_table.expression(expression) {
            ExpressionNode::Indexed(indexed) => {
                match program.expression_table.expression(indexed.index) {
                    ExpressionNode::Range(range) => range.start,
                    _ => indexed.index,
                }
            }
            _ => panic!("index fixture"),
        };
        assert!(
            program
                .expression_table
                .constant_integer_value(application)
                .is_some(),
            "{selector}: the application is constant-shaped"
        );
        let donor = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == "donor" => {
                    Some(local.initial_value)
                }
                _ => None,
            })
            .expect("donor local");
        let mut operators = selected_operator_facts(&program);
        let mut fabricated = *operators
            .uses
            .iter()
            .find_map(|(_, row)| {
                (row.expression == donor
                    && row.status == checked_trees::CheckedOperatorResolutionStatus::Resolved)
                    .then_some(row)
            })
            .expect("resolved donor row");
        fabricated.expression = application;
        let checked_trees::CheckedValueOrigin::StateStatement {
            statement_index, ..
        } = &mut fabricated.origin
        else {
            panic!("statement-scoped donor")
        };
        *statement_index = 0;
        operators.uses.append(fabricated);
        let mut facts = RangeFacts::new(&[]);
        facts.bound_program = Some(&program);
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            facts.expression_dependencies[0].reads.is_none(),
            "{selector}: a folded authored application claimed a footprint"
        );
    }
}

/// A builtin operator over an authored application keeps recursing: the
/// outer `*` has builtin meaning on its own node, so only the inner `+`
/// needs custody, and the window reads every leaf operand.
#[test]
fn a_builtin_operator_over_a_selected_application_reads_every_leaf_operand() {
    let program = arithmetic_window_source("(low + step) * 2u64..high");
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("mixed builtin and selected arithmetic reads");
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("window fixture")
    };
    let mut window = parameter_place(&program, state, "items");
    window.segments.push(facts::PlaceSegment::Index {
        expression: indexed.index,
    });
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "low"),
            parameter_place(&program, state, "step"),
            parameter_place(&program, state, "high"),
            window,
        ]
        .as_slice()
    );
}

/// A contract-scope occurrence has no statement use custody, so an authored
/// arithmetic selector in `requires` stays incomplete exactly like a
/// selected `[]` there.
#[test]
fn a_requires_scope_selected_arithmetic_selector_has_no_statement_use_custody() {
    let program = typed_program(
        "operator + u64::custom(left: u64, right: u64) -> u64;
        machine window(items: &[i64; 4], low: u64, step: u64)
        requires 0 <= items[low + step]; {}",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let contract = &program.machine_contracts(machine)[0];
    let typed_trees::domain::ProofFact::Expression(guard) =
        program.proof_facts.span_or_empty(contract.facts)[0]
    else {
        panic!("expression contract")
    };
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        panic!("bound comparison")
    };
    let expression = binary.right;
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(
        facts.expression_dependencies[0].reads.is_none(),
        "a contract-scope arithmetic occurrence invented statement use custody"
    );
}

/// The authored application standing under a cast or unary wrapper in a
/// selector or bound position, for tests that inspect or corrupt its custody.
fn wrapped_selector_application(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("index fixture")
    };
    let ExpressionNode::Cast(cast) = program.expression_table.expression(indexed.index) else {
        panic!("cast-wrapped selector")
    };
    cast.value
}

/// A cast or unary wrapper has one immediate runtime child and binds no
/// overloadable token, so the operand gate keeps walking through it to the
/// authored application below: the wrapped selector reads exactly the operands
/// the selected declaration receives, plus the element or window place.
#[test]
fn a_wrapped_authored_arithmetic_selector_reads_every_operand() {
    for (declared, selector, operands) in [
        ("i64", "(low + step) as u64", &["low", "step"][..]),
        ("i64", "~(low + step)", &["low", "step"][..]),
        ("i64", "(low + step) as u64 as u64", &["low", "step"][..]),
        (
            "&[i64]",
            "(low + step) as u64..high",
            &["low", "step", "high"][..],
        ),
        ("&[i64]", "low..(low + step) as u64", &["low", "step"][..]),
    ] {
        let program = typed_program(&format!(
            "operator + u64::custom(left: u64, right: u64) -> u64;
            machine window(items: &[i64; 4], low: u64, step: u64, high: u64, unrelated: u64) {{
                let cut: {declared} = items[{selector}];
            }}"
        ));
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let operators = selected_operator_facts(&program);
        let mut facts = RangeFacts::new(&[]);
        facts.bound_program = Some(&program);
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .unwrap_or_else(|| panic!("{selector}: a wrapped authored arithmetic footprint"));
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            panic!("index fixture")
        };
        let mut selected = parameter_place(&program, state, "items");
        selected.segments.push(facts::PlaceSegment::Index {
            expression: indexed.index,
        });
        let expected: Vec<CanonicalPlace> = operands
            .iter()
            .map(|name| parameter_place(&program, state, name))
            .chain(std::iter::once(selected))
            .collect();
        assert_eq!(reads.as_slice(), expected.as_slice(), "{selector}");
        let label = program.expression_table.display_name(expression);
        for name in operands.iter().copied().chain(["items"]) {
            let writes = [parameter_place(&program, state, name)];
            assert!(
                !facts
                    .preserved_expression_labels(&program, machine, state, Some(&writes))
                    .contains(&label),
                "{selector}: a write to {name} must retire the premise"
            );
        }
        let writes = [parameter_place(&program, state, "unrelated")];
        assert!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            "{selector}: a disjoint write must preserve it"
        );
    }
}

/// The wrapper admits nothing on its own. A cast over a family the gate still
/// refuses stays incomplete: an authored comparison is not an arithmetic
/// spelling, a constant-shaped authored application is folded syntactically by
/// the place algebra before the selected declaration can establish a
/// coordinate, and drifted custody proves nothing about which declaration ran.
#[test]
fn a_wrapper_over_a_refused_operand_family_stays_incomplete() {
    for (source, drift) in [
        (
            "operator < u64::compare(left: u64, right: u64) -> bool;
            machine window(items: &[i64; 4], low: u64, step: u64) {
                let cut: i64 = items[(low < step) as u64];
            }",
            false,
        ),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;
            machine window(items: &[i64; 4], low: u64, step: u64) {
                let cut: i64 = items[(1u64 + 0u64) as u64];
            }",
            false,
        ),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;
            machine window(items: &[i64; 4], low: u64, step: u64) {
                let cut: i64 = items[(low + step) as u64];
            }",
            true,
        ),
    ] {
        let program = typed_program(source);
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let mut operators = selected_operator_facts(&program);
        if drift {
            let application = wrapped_selector_application(&program, expression);
            let handle = operators
                .uses
                .iter()
                .find_map(|(handle, row)| (row.expression == application).then_some(handle))
                .expect("checked use row for the wrapped application");
            operators.uses.get_mut(handle).status =
                checked_trees::CheckedOperatorResolutionStatus::Ambiguous;
        }
        let mut facts = RangeFacts::new(&[]);
        facts.bound_program = Some(&program);
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            facts.expression_dependencies[0].reads.is_none(),
            "a wrapper admitted a refused operand family: {source}"
        );
    }
}
