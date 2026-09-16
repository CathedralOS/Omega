use super::{ExpressionHandle, ExpressionNode, State, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::flow::CanonicalPlace;
use typed_trees::statement::StatementNode;

mod atomics;
mod borrows;
pub(crate) mod calls;
mod indexes;

fn typed_source(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize dependency fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn initializer(program: &TypedTrees, state: &State) -> ExpressionHandle {
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
        .expect("computed local")
}

fn parameter_place(program: &TypedTrees, state: &State, name: &str) -> CanonicalPlace {
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
        .expect("parameter");
    CanonicalPlace {
        root: facts::PlaceRoot::Symbol(parameter.symbol),
        segments: Vec::new(),
    }
}

#[test]
fn retention_requires_complete_disjoint_writes_for_every_operand() {
    let program = typed_source(
        "machine window(left: i64, right: i64, unrelated: i64) {
        let cut: i64 = left - right;
    }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [("left", false), ("right", false), ("unrelated", true)] {
        let writes = [parameter_place(&program, state, name)];
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .contains(&label),
            survives,
            "write to {name}"
        );
    }
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, None)
            .is_empty()
    );
    for root in [
        facts::PlaceRoot::Unknown,
        facts::PlaceRoot::Symbol(SymbolHandle::invalid()),
        facts::PlaceRoot::Expression(expression),
    ] {
        let writes = [CanonicalPlace {
            root,
            segments: Vec::new(),
        }];
        assert!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&writes))
                .is_empty()
        );
    }
}

#[test]
fn identical_labels_cannot_choose_between_different_typed_reads() {
    let program = typed_source(
        "machine first(original: i64, unrelated: i64) {
        let cut: i64 = original - 1;
    }
    machine second(original: i64, unrelated: i64) {
        let cut: i64 = original - 1;
    }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let first_expression = initializer(&program, first_state);
    let second_expression = initializer(&program, second_state);
    assert_ne!(first_expression, second_expression);
    assert_eq!(
        program.expression_table.display_name(first_expression),
        program.expression_table.display_name(second_expression)
    );
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, first, first_state, first_expression);
    let writes = [parameter_place(&program, first_state, "unrelated")];
    assert_eq!(
        facts
            .preserved_expression_labels(&program, first, first_state, Some(&writes))
            .len(),
        1
    );
    // Repeated registration of the same exact meaning is harmless.
    facts.record_expression_dependencies(&program, first, first_state, first_expression);
    assert_eq!(facts.expression_dependencies.len(), 1);
    facts.record_expression_dependencies(&program, second, second_state, second_expression);
    assert!(
        facts
            .preserved_expression_labels(&program, first, first_state, Some(&writes))
            .is_empty()
    );
}

#[test]
fn an_incoming_expression_cannot_borrow_the_current_states_parameter_names() {
    let program = typed_source(
        "machine window(original: i64) {
        let cut: i64 = original - 1;
        transition { _ -> next(original) }
        state next(original: i64) {
            let cut: i64 = original - 1;
        }
    }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let incoming = initializer(&program, &states[0]);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, &states[1], incoming);
    assert!(
        facts
            .expression_dependencies
            .iter()
            .all(|row| row.reads.is_none())
    );
    assert!(
        facts
            .preserved_expression_labels(&program, machine, &states[1], Some(&[]))
            .is_empty()
    );
}

#[test]
fn calls_and_call_selectors_do_not_claim_an_argument_only_read_set() {
    for expression in ["compute(original)", "items[compute(original)]"] {
        let program = typed_source(&format!(
            "machine compute(original: u64) -> u64 {{ original }}
            machine window(original: u64, items: &[u64; 4]) {{
                let cut: u64 = {expression};
            }}"
        ));
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
        assert_eq!(facts.expression_dependencies.len(), 1);
        assert!(facts.expression_dependencies[0].reads.is_none());
        assert!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&[]))
                .is_empty()
        );
    }
}

/// A literal operand contributes no caller storage: only the parameter read
/// remains, so a fact over `left - 2.5` survives writes the subtraction
/// cannot observe.
#[test]
fn a_literal_operand_reads_nothing_but_keeps_the_expression_completable() {
    let program = typed_source(
        "machine window(left: f64, unrelated: f64) {
        let cut: f64 = left - 2.5;
    }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("float subtraction reads");
    assert_eq!(
        reads.as_slice(),
        [parameter_place(&program, state, "left")].as_slice()
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [("left", false), ("unrelated", true)] {
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

/// Array and struct literals evaluate every operand where the literal
/// appears, so their footprint is the union of element and field-value
/// reads — nothing more and nothing less.
#[test]
fn compound_literals_read_each_evaluated_operand() {
    for (declaration, statement) in [
        ("", "let cut: [i64; 2] = [left, right];"),
        (
            "data Pair { a: i64; b: i64; }",
            "let cut: Pair = Pair { a: left, b: right };",
        ),
    ] {
        let program = typed_source(&format!(
            "{declaration}
            machine window(left: i64, right: i64, unrelated: i64) {{
                {statement}
            }}"
        ));
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let expression = initializer(&program, state);
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .unwrap_or_else(|| panic!("literal reads for {statement}"));
        assert_eq!(
            reads.as_slice(),
            [
                parameter_place(&program, state, "left"),
                parameter_place(&program, state, "right"),
            ]
            .as_slice(),
            "{statement}"
        );
        let label = program.expression_table.display_name(expression);
        for (name, survives) in [("left", false), ("right", false), ("unrelated", true)] {
            let writes = [parameter_place(&program, state, name)];
            assert_eq!(
                facts
                    .preserved_expression_labels(&program, machine, state, Some(&writes))
                    .contains(&label),
                survives,
                "{statement}: write to {name}"
            );
        }
    }
}

/// A match reads its subject, each compared pattern value, and whichever
/// arm's value runs. The union footprint is conservative: a write to any
/// possibly-read place retires the match's facts even when that arm might
/// not have been selected.
#[test]
fn a_match_reads_its_subject_patterns_and_arm_values() {
    let program = typed_source(
        "machine window(flag: i64, left: i64, right: i64, unrelated: i64) {
        let cut: i64 = match flag { 0 -> left, _ -> right };
    }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("match reads");
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "flag"),
            parameter_place(&program, state, "left"),
            parameter_place(&program, state, "right"),
        ]
        .as_slice()
    );
    let label = program.expression_table.display_name(expression);
    for (name, survives) in [
        ("flag", false),
        ("left", false),
        ("right", false),
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
fn missing_typed_place_identities_cannot_be_recovered_from_display_names() {
    for source in [
        "machine window(original: i64) { let cut: i64 = original - 1; }",
        "data Host { original: i64; }
         machine Host::window(&self) { let cut: i64 = self.original - 1; }",
    ] {
        let mut program = typed_source(source);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let expression = initializer(&program, state);
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            !facts
                .preserved_expression_labels(&program, machine, state, Some(&[]))
                .is_empty()
        );
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            panic!("arithmetic fixture");
        };
        let operand = binary.left;
        match program.expression_table.expression_mut(operand) {
            ExpressionNode::Name(path) => {
                path.head_symbol = SymbolHandle::invalid();
                path.symbol = SymbolHandle::invalid();
                path.member_symbols = arena::HandleSpan::empty();
            }
            ExpressionNode::Member(member) => member.member_symbol = SymbolHandle::invalid(),
            _ => panic!("a typed parameter or member operand"),
        }
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        assert!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&[]))
                .is_empty(),
            "missing identity was recovered by spelling: {source}"
        );
    }
}
