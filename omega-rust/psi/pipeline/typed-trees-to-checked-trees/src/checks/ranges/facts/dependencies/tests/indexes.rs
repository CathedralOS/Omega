use super::{ExpressionHandle, ExpressionNode, StatementNode, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::checks::ranges::facts::dependencies::tests::selected_operator_facts;
use crate::checks::ranges::facts::dependencies::tests::typed_source;
use crate::flow::CanonicalPlace;
use typed_trees::machine::Machine;
use typed_trees::state::State;

fn index_source(declaration: &str, selector: &str) -> TypedTrees {
    typed_source(&format!(
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
fn indexed_reads_retain_element_coordinates_and_each_selector_dependency() {
    for selector in ["1", "index", "selectors[index]"] {
        let program = typed_source(&format!(
            "machine window(items: &[i64; 4], selectors: &[u64; 4], index: u64, unrelated: u64)
            requires 0 <= items[{selector}]; {{}}"
        ));
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
        let label = program.expression_table.display_name(expression);
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .expect("complete indexed reads");
        assert_eq!(
            reads.len(),
            match selector {
                "1" => 1,
                "index" => 2,
                _ => 3,
            }
        );
        let mut element = parameter_place(&program, state, "items");
        element
            .segments
            .push(facts::PlaceSegment::FixedIndex { index: 0 });
        for (write, survives) in [
            (element, selector == "1"),
            (parameter_place(&program, state, "items"), false),
            (parameter_place(&program, state, "index"), selector == "1"),
            (
                parameter_place(&program, state, "selectors"),
                selector != "selectors[index]",
            ),
            (parameter_place(&program, state, "unrelated"), true),
        ] {
            assert_eq!(
                facts
                    .preserved_expression_labels(
                        &program,
                        machine,
                        state,
                        Some(std::slice::from_ref(&write))
                    )
                    .contains(&label),
                survives,
                "{selector}: {write:?}"
            );
        }
    }
}

#[test]
fn selector_constant_folding_requires_the_selected_builtin_arithmetic() {
    for (declaration, complete) in [
        ("", true),
        (
            "operator + f64::unrelated(left: f64, right: f64) -> f64;",
            true,
        ),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            false,
        ),
    ] {
        let program = index_source(declaration, "1u64 + 0u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "window")
            .expect("window");
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(
            &program,
            machine,
            state,
            initializer(&program, state),
        );
        assert_eq!(
            facts.expression_dependencies[0].reads.is_some(),
            complete,
            "{declaration}"
        );
    }
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
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(facts.expression_dependencies[0].reads.is_none());
    }
}

#[test]
fn authored_index_operators_do_not_claim_builtin_element_reads() {
    for (declaration, complete) in [
        ("", true),
        (
            "boundary operator [] Slice::other(items: &[u64], index: u64) -> u64;",
            true,
        ),
        (
            "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;",
            false,
        ),
    ] {
        let program = index_source(declaration, "0u64");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "window")
            .expect("window");
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(
            &program,
            machine,
            state,
            initializer(&program, state),
        );
        assert_eq!(
            facts.expression_dependencies[0].reads.is_some(),
            complete,
            "{declaration}"
        );
    }
}

#[test]
fn a_selected_index_operator_reads_exactly_its_checked_operands() {
    let program = index_source(
        "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;",
        "index",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("selected index operand reads");
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "items"),
            parameter_place(&program, state, "index"),
        ]
        .as_slice()
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [
        ("items", false),
        ("index", false),
        ("selectors", true),
        ("unrelated", true),
    ] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

#[test]
fn a_selected_range_operator_reads_its_window_operands() {
    let program = typed_source(
        "boundary operator [..] Slice::window(items: &[i64], start: u64, end: u64) -> i64;
        machine window(items: &[i64; 4], low: u64, high: u64, unrelated: u64) {
            let cut: i64 = items[low..high];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("selected range operand reads");
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "items"),
            parameter_place(&program, state, "low"),
            parameter_place(&program, state, "high"),
        ]
        .as_slice()
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [
        ("items", false),
        ("low", false),
        ("high", false),
        ("unrelated", true),
    ] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

#[test]
fn a_selected_index_operator_needs_stable_checked_custody() {
    let declaration = "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;";
    for mutate in [
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.status = checked_trees::CheckedOperatorResolutionStatus::Missing;
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.status = checked_trees::CheckedOperatorResolutionStatus::Ambiguous;
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.selected_operator_symbol = SymbolHandle::invalid();
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.candidate_count += 1;
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.spelling = language_core::operator_spelling::OperatorSpelling::Range;
        },
    ] {
        let program = index_source(declaration, "index");
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let mut operators = selected_operator_facts(&program);
        let handle = operators
            .uses
            .iter()
            .find_map(|(handle, row)| (row.expression == expression).then_some(handle))
            .expect("checked use row");
        mutate(operators.uses.get_mut(handle));
        let mut facts = RangeFacts::new(&[]);
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            facts.expression_dependencies[0].reads.is_none(),
            "drifted selection custody still claimed a footprint"
        );
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

/// A computed index operand is hoisted to an immutable local before the
/// application runs, so the operand read is the captured value's own
/// identity: writes to the selector's source storage cannot rewrite a value
/// that was already frozen.
#[test]
fn a_selected_index_operator_reads_the_captured_selector_value() {
    let program = index_source(
        "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;",
        "selectors[index]",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let statement_index = statement_index_of(&program, state, "cut");
    let capture = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) => Some(CanonicalPlace {
                root: facts::PlaceRoot::Symbol(local.symbol),
                segments: Vec::new(),
            }),
            _ => None,
        })
        .expect("hoisted selector capture");
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("index fixture")
    };
    assert!(matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Name(_)
    ));
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("selected index operand reads");
    assert_eq!(
        reads.as_slice(),
        [parameter_place(&program, state, "items"), capture].as_slice()
    );
    let label = program.expression_table.display_name(expression);
    // The captured operand is already a value: writes to the selector's
    // source storage or the original index parameter cannot rewrite it.
    for (name, survives) in [
        ("items", false),
        ("index", true),
        ("selectors", true),
        ("unrelated", true),
    ] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

/// A selected application nested under a builtin index is also hoisted, so
/// each occurrence keeps its own checked custody: corrupting the inner
/// selection's evidence retires only the inner footprint, while the outer
/// element read observes the frozen capture.
#[test]
fn a_selected_index_operand_keeps_independent_custody_from_the_outer_read() {
    let declaration = "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;";
    for corrupt_inner in [false, true] {
        let program = typed_source(&format!(
            "{declaration}
            machine window(items: &[i64; 4], index: u64, unrelated: u64) {{
                let cut: i64 = items[items[index]];
            }}"
        ));
        let (machine, state) = window(&program);
        let outer_statement = statement_index_of(&program, state, "cut");
        let (capture, inner_expression) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(outer_statement)
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) => Some((
                    CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(local.symbol),
                        segments: Vec::new(),
                    },
                    local.initial_value,
                )),
                _ => None,
            })
            .expect("hoisted inner application");
        let outer_expression = initializer(&program, state);
        let ExpressionNode::Indexed(outer) = program.expression_table.expression(outer_expression)
        else {
            panic!("index fixture")
        };
        let mut operators = selected_operator_facts(&program);
        if corrupt_inner {
            let handle = operators
                .uses
                .iter()
                .find_map(|(handle, row)| (row.expression == inner_expression).then_some(handle))
                .expect("inner checked use row");
            operators.uses.get_mut(handle).status =
                checked_trees::CheckedOperatorResolutionStatus::Ambiguous;
        }
        let mut facts = RangeFacts::new(&[]);
        facts.checked_operators = Some(&operators);
        facts.statement_index = 0;
        facts.record_expression_dependencies(&program, machine, state, inner_expression);
        facts.statement_index = outer_statement;
        facts.record_expression_dependencies(&program, machine, state, outer_expression);

        let inner_reads = &facts.expression_dependencies[0].reads;
        let outer_reads = &facts.expression_dependencies[1].reads;
        // The inner application keeps its own custody; the outer builtin
        // element read depends only on the frozen capture and the
        // collection.
        let mut outer_element = parameter_place(&program, state, "items");
        outer_element.segments.push(facts::PlaceSegment::Index {
            expression: outer.index,
        });
        assert_eq!(
            outer_reads.as_deref(),
            Some([capture.clone(), outer_element].as_slice()),
            "corrupt_inner={corrupt_inner}"
        );
        if corrupt_inner {
            assert!(inner_reads.is_none());
        } else {
            assert_eq!(
                inner_reads.as_deref(),
                Some(
                    [
                        parameter_place(&program, state, "items"),
                        parameter_place(&program, state, "index"),
                    ]
                    .as_slice()
                )
            );
        }
        // A write to the original index parameter retires the inner
        // application's facts but leaves the outer element read, which saw
        // only the frozen capture.
        let writes = [parameter_place(&program, state, "index")];
        let preserved = facts.preserved_expression_labels(&program, machine, state, Some(&writes));
        let inner_label = program.expression_table.display_name(inner_expression);
        let outer_label = program.expression_table.display_name(outer_expression);
        assert!(
            !preserved.contains(&inner_label),
            "corrupt_inner={corrupt_inner}"
        );
        assert!(
            preserved.contains(&outer_label),
            "the outer element read outlives the inner operand's source: corrupt_inner={corrupt_inner}"
        );
    }
}

#[test]
fn a_selected_index_operator_with_an_open_range_stays_incomplete() {
    let program = typed_source(
        "boundary operator [..] Slice::window(items: &[i64], start: u64, end: u64) -> i64;
        machine window(items: &[i64; 4], low: u64, unrelated: u64) {
            let cut: i64 = items[low..];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

#[test]
fn a_requires_scope_selected_operator_has_no_statement_use_custody() {
    let program = typed_source(
        "boundary operator [] Slice::custom(items: &[i64], index: u64) -> i64;
        machine window(items: &[i64; 4], index: u64)
        requires 0 <= items[index]; {}",
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
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(
        facts.expression_dependencies[0].reads.is_none(),
        "a contract-scope occurrence invented statement use custody"
    );
}

#[test]
fn dynamic_contract_reads_retain_current_parameter_identities() {
    let program = typed_source(
        "machine window(original: &mut [i64; 2], mut index: u64 [0..=1])
        requires 0 <= original[index] && original[index] <= 4; {
            let mut unrelated: i64 = 0; unrelated = 1;
        }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let mut facts = RangeFacts::new(&[]);
    crate::checks::ranges::requirements::seed_state_requires(&program, &mut facts, machine, state);
    let rows = facts
        .expression_dependencies
        .iter()
        .map(|row| {
            (
                &row.label,
                &row.reads,
                program.expression_table.expression(row.expression),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        facts
            .expression_dependencies
            .iter()
            .filter(|row| row.label == "original[index]")
            .all(|row| row.reads.is_some()),
        "{rows:#?}"
    );
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&[]))
            .contains(&"original[index]".to_owned()),
        "{rows:#?}"
    );
}

#[test]
fn a_reference_read_below_an_index_is_not_an_integer_snapshot() {
    let program = typed_source(
        "data Cell { value: &i64; }
        machine window(items: &[Cell; 2]) {
        let cut: &i64 = items[0].value;
    }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "cut" => Some(local),
            _ => None,
        })
        .expect("reference copy");
    let mut facts = RangeFacts::new(&[]);
    facts.prove_index_upper_bound(program.expression_table.display_name(expression), 5);
    facts.alias_integer_place_value(&program, machine, state, expression, local.symbol, "cut");
    assert!(!facts.index_upper_bound_is_proven("cut", 5));
}

/// A builtin `collection[a..b]` window is a place: its complete footprint is
/// each present bound's operand reads plus the window place itself. Writes
/// to the collection or either bound retire its facts; unrelated storage
/// cannot touch them.
#[test]
fn a_builtin_range_window_reads_its_collection_and_both_bounds() {
    let program = typed_source(
        "machine window(items: &[i64; 4], low: u64, high: u64, unrelated: u64) {
            let cut: &[i64] = items[low..high];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("builtin window reads");
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
            parameter_place(&program, state, "high"),
            window,
        ]
        .as_slice()
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [
        ("items", false),
        ("low", false),
        ("high", false),
        ("unrelated", true),
    ] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

/// Constant bounds collapse the window to resolved `start..end` coordinates:
/// the footprint is the window place alone, and an element write outside the
/// extent is proven disjoint instead of retiring the window's facts.
#[test]
fn a_constant_range_window_keeps_its_exact_extent() {
    let program = typed_source(
        "machine window(items: &[i64; 4], unrelated: u64) {
            let cut: &[i64] = items[0..2];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
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

/// An omitted open bound reads nothing: `a..`, `..b`, and `..` keep only the
/// bounds the syntax actually evaluates, while the window place still tracks
/// the collection.
#[test]
fn an_open_builtin_window_reads_only_its_present_bounds() {
    for (selector, bounds) in [
        ("low..", vec!["low"]),
        ("..high", vec!["high"]),
        ("..", Vec::new()),
    ] {
        let program = typed_source(&format!(
            "machine window(items: &[i64; 4], low: u64, high: u64, unrelated: u64) {{
                let cut: &[i64] = items[{selector}];
            }}"
        ));
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .unwrap_or_else(|| panic!("open window {selector} reads"));
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            panic!("window fixture")
        };
        let mut window = parameter_place(&program, state, "items");
        window.segments.push(facts::PlaceSegment::Index {
            expression: indexed.index,
        });
        let expected: Vec<CanonicalPlace> = bounds
            .iter()
            .map(|name| parameter_place(&program, state, name))
            .chain(std::iter::once(window))
            .collect();
        assert_eq!(reads.as_slice(), expected.as_slice(), "{selector}");
        let label = program.expression_table.display_name(expression);
        for (name, survives) in [
            ("items", false),
            ("low", !bounds.contains(&"low")),
            ("high", !bounds.contains(&"high")),
            ("unrelated", true),
        ] {
            let writes = [parameter_place(&program, state, name)];
            assert_eq!(
                facts
                    .preserved_expression_labels(&program, machine, state, Some(&writes))
                    .contains(&label),
                survives,
                "{selector}: write to {name}"
            );
        }
    }
}

/// A window bound that selects authored arithmetic is call-shaped inside the
/// operand position: without checked operator custody the operand scan
/// cannot describe its reads, so the window stays incomplete instead of
/// claiming only the bound's visible places.
#[test]
fn a_window_bound_with_authored_arithmetic_and_no_checked_custody_stays_incomplete() {
    let program = typed_source(
        "operator + u64::custom(left: u64, right: u64) -> u64;
        machine window(items: &[i64; 4], low: u64, high: u64) {
            let cut: &[i64] = items[low + 0u64..high];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

fn arithmetic_window_source(selector: &str) -> TypedTrees {
    typed_source(&format!(
        "operator + u64::custom(left: u64, right: u64) -> u64;
        machine window(items: &[i64; 4], low: u64, step: u64, high: u64, unrelated: u64) {{
            let cut: &[i64] = items[{selector}];
        }}"
    ))
}

/// The authored `+` application inside a builtin window bound, for tests
/// that corrupt its checked custody.
fn window_start_bound(program: &TypedTrees, expression: ExpressionHandle) -> ExpressionHandle {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        panic!("window fixture")
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        panic!("range selector")
    };
    range.start
}

/// With the exact checked operator-use row at this statement, an authored
/// arithmetic window bound is a selected application whose footprint is the
/// operands the declaration receives: the window reads the collection place
/// plus each bound's operands, survives writes the bound cannot observe,
/// and is retired by a write to any operand or to the collection.
#[test]
fn a_selected_arithmetic_window_bound_reads_its_checked_operands() {
    let program = arithmetic_window_source("low + step..high");
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let operators = selected_operator_facts(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("selected arithmetic window reads");
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
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [
        ("items", false),
        ("low", false),
        ("step", false),
        ("high", false),
        ("unrelated", true),
    ] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
}

/// A point selector with authored arithmetic over a local leaf is not
/// hoisted, so the application stays inline in the builtin element place.
/// Its reads are the operands plus the element place, and the place algebra
/// keeps the non-constant selector conservative: a fixed-element write
/// still retires the facts because the selected declaration's value is
/// unknown.
#[test]
fn a_selected_arithmetic_point_selector_reads_its_operands_and_stays_conservative() {
    let program = typed_source(
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
        let program = typed_source(&format!(
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
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            facts.expression_dependencies[0].reads.is_none(),
            "{selector}: a folded authored application claimed a footprint"
        );
    }
}

/// Drifted custody on the arithmetic application — a missing or ambiguous
/// status, an invalid selection, a candidate roster that no longer matches,
/// or a foreign spelling — leaves the window incomplete.
#[test]
fn a_selected_arithmetic_bound_needs_stable_checked_custody() {
    for mutate in [
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.status = checked_trees::CheckedOperatorResolutionStatus::Missing;
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.status = checked_trees::CheckedOperatorResolutionStatus::Ambiguous;
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.selected_operator_symbol = SymbolHandle::invalid();
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.candidate_count += 1;
        },
        |row: &mut checked_trees::CheckedOperatorUseFact| {
            row.spelling = language_core::operator_spelling::OperatorSpelling::Subtract;
        },
    ] {
        let program = arithmetic_window_source("low + step..high");
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let application = window_start_bound(&program, expression);
        let mut operators = selected_operator_facts(&program);
        let handle = operators
            .uses
            .iter()
            .find_map(|(handle, row)| (row.expression == application).then_some(handle))
            .expect("checked use row for the arithmetic bound");
        mutate(operators.uses.get_mut(handle));
        let mut facts = RangeFacts::new(&[]);
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            facts.expression_dependencies[0].reads.is_none(),
            "drifted arithmetic custody still claimed a footprint"
        );
    }
}

/// Each authored application inside a bound keeps its own custody: a nested
/// `(low + step) + high` reads every leaf operand once when both rows are
/// stable, and corrupting only the inner row leaves the whole window
/// incomplete.
#[test]
fn nested_selected_arithmetic_bounds_each_keep_their_own_custody() {
    for corrupt_inner in [false, true] {
        let program = arithmetic_window_source("low + step + high..high");
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let outer = window_start_bound(&program, expression);
        let ExpressionNode::Binary(binary) = program.expression_table.expression(outer) else {
            panic!("nested arithmetic bound")
        };
        let inner = binary.left;
        assert!(matches!(
            program.expression_table.expression(inner),
            ExpressionNode::Binary(_)
        ));
        let mut operators = selected_operator_facts(&program);
        if corrupt_inner {
            let handle = operators
                .uses
                .iter()
                .find_map(|(handle, row)| (row.expression == inner).then_some(handle))
                .expect("inner checked use row");
            operators.uses.get_mut(handle).status =
                checked_trees::CheckedOperatorResolutionStatus::Ambiguous;
        }
        let mut facts = RangeFacts::new(&[]);
        facts.checked_operators = Some(&operators);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = &facts.expression_dependencies[0].reads;
        if corrupt_inner {
            assert!(
                reads.is_none(),
                "an unproven inner application was admitted"
            );
            continue;
        }
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
        else {
            panic!("window fixture")
        };
        let mut window = parameter_place(&program, state, "items");
        window.segments.push(facts::PlaceSegment::Index {
            expression: indexed.index,
        });
        assert_eq!(
            reads.as_deref(),
            Some(
                [
                    parameter_place(&program, state, "low"),
                    parameter_place(&program, state, "step"),
                    parameter_place(&program, state, "high"),
                    window,
                ]
                .as_slice()
            )
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
    let program = typed_source(
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
    facts.checked_operators = Some(&operators);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(
        facts.expression_dependencies[0].reads.is_none(),
        "a contract-scope arithmetic occurrence invented statement use custody"
    );
}
