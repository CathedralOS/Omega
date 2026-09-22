use super::{ExpressionNode, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use typed_trees::machine::Machine;
use typed_trees::state::State;

fn window(program: &TypedTrees) -> (&Machine, &State) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "window")
        .expect("window");
    (machine, &program.machine_states(machine)[0])
}

fn field_symbol(program: &TypedTrees, data_name: &str, field_name: &str) -> SymbolHandle {
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == data_name)
        .expect(data_name);
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect(field_name)
}

fn field_place(
    program: &TypedTrees,
    state: &State,
    parameter: &str,
    fields: &[(&str, &str)],
) -> CanonicalPlace {
    let mut place = parameter_place(program, state, parameter);
    for (data, field) in fields {
        place.segments.push(facts::PlaceSegment::Field {
            symbol: field_symbol(program, data, field),
        });
    }
    place
}

/// An explicit `&`/`&mut` borrow inside a place chain addresses exactly the
/// target's place: `(&pair).a` is `pair.a` wearing a reference the canonical
/// place already peels. The complete footprint is the projected place plus
/// whatever its own selectors read — nothing more.
#[test]
fn a_borrowed_member_receiver_reads_its_projected_place() {
    for borrow in ["&pair", "&mut pair"] {
        let program = typed_program(&format!(
            "data Pair {{ a: i64; b: i64; }}
            machine window(mut pair: Pair, unrelated: i64) {{
                let cut: i64 = ({borrow}).a;
            }}"
        ));
        let (machine, state) = window(&program);
        let expression = initializer(&program, state);
        let mut facts = RangeFacts::new(&[]);
        facts.record_expression_dependencies(&program, machine, state, expression);
        let reads = facts.expression_dependencies[0]
            .reads
            .as_ref()
            .unwrap_or_else(|| panic!("borrowed member footprint: {borrow}"));
        assert_eq!(
            reads.as_slice(),
            [field_place(&program, state, "pair", &[("Pair", "a")])].as_slice(),
            "{borrow}"
        );
        let label = program.expression_table.display_name(expression);
        for (write, survives) in [
            (
                field_place(&program, state, "pair", &[("Pair", "a")]),
                false,
            ),
            (field_place(&program, state, "pair", &[("Pair", "b")]), true),
            (parameter_place(&program, state, "pair"), false),
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
                "{borrow}: {write:?}"
            );
        }
    }
}

/// The same peel applies when the borrow is a builtin index collection:
/// `(&items)[i]` reads the element place `items[i]` plus the selector
/// operand `i`.
#[test]
fn a_borrowed_index_collection_reads_the_element_place() {
    let program = typed_program(
        "machine window(items: &[i64; 4], index: u64, unrelated: u64) {
            let cut: i64 = (&items)[index];
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("borrowed collection footprint");
    let mut element = parameter_place(&program, state, "items");
    element.segments.push(facts::PlaceSegment::Index {
        expression: {
            let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression)
            else {
                panic!("indexed fixture")
            };
            indexed.index
        },
    });
    assert_eq!(
        reads.as_slice(),
        [parameter_place(&program, state, "index"), element.clone(),].as_slice(),
        "{reads:?}"
    );
    let label = program.expression_table.display_name(expression);
    for (write, survives) in [
        (parameter_place(&program, state, "items"), false),
        (parameter_place(&program, state, "index"), false),
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
            "{write:?}"
        );
    }
}

/// A borrow chain nested inside another operand keeps each operand's reads:
/// the borrowed member contributes its projected place, the sibling operand
/// its own.
#[test]
fn a_borrowed_member_inside_a_compound_operand_reads_each_side() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine window(pair: Pair, offset: i64, unrelated: i64) {
            let cut: i64 = (&pair).a + offset;
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("compound borrowed-member footprint");
    assert_eq!(
        reads.as_slice(),
        [
            field_place(&program, state, "pair", &[("Pair", "a")]),
            parameter_place(&program, state, "offset"),
        ]
        .as_slice(),
        "{reads:?}"
    );
}

/// Borrowing a temporary is not a place read: `(&compute()).a` cannot name
/// the call result's storage, so the read set stays incomplete rather than
/// borrowing the callee's operand footprint for a field the caller cannot
/// see.
#[test]
fn a_borrow_of_a_call_result_stays_incomplete() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine compute() -> Pair { Pair { a: 0, b: 0 } }
        machine window(pair: Pair, unrelated: i64) {
            let cut: i64 = (&compute()).a;
        }",
    );
    let (machine, state) = window(&program);
    let expression = initializer(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// A member whose name resolves to no field of the receiver type cannot name
/// its projected place: clearing the node-carried symbol alone is not enough
/// (the contextual member walk still finds `Pair::a`), so the drifted
/// spelling must be unresolvable on the receiver's declaration.
#[test]
fn an_unresolved_member_on_a_borrowed_receiver_stays_incomplete() {
    let mut program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine window(pair: Pair, unrelated: i64) {
            let cut: i64 = (&pair).a;
        }",
    );
    let expression = {
        let (_, state) = window(&program);
        initializer(&program, state)
    };
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        panic!("member fixture")
    };
    member.member_symbol = SymbolHandle::invalid();
    member.member = "missing".into();
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// The peel reaches `self` storage: `(&self.pair).a` reads the attached
/// field place, so a sibling-field write preserves the premise while a
/// write to the projected field or the whole receiver retires it.
#[test]
fn a_borrowed_self_member_reads_the_attached_field_place() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        data Main { pair: Pair; unrelated: i64; }
        machine Main::window(&self, offset: i64) -> i64 {
            let cut: i64 = (&self.pair).a;
            cut
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
        .expect("borrowed self-member footprint");
    let self_parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .expect("self parameter");
    let mut expected = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(self_parameter.symbol),
        segments: Vec::new(),
    };
    expected.segments.push(facts::PlaceSegment::Field {
        symbol: field_symbol(&program, "Main", "pair"),
    });
    expected.segments.push(facts::PlaceSegment::Field {
        symbol: field_symbol(&program, "Pair", "a"),
    });
    assert_eq!(reads.as_slice(), [expected].as_slice(), "{reads:?}");
    let label = program.expression_table.display_name(expression);
    // Writes are recorded through the machine root: `normalized_event_place_root`
    // re-bases the self parameter onto it for the overlap comparison.
    let machine_write = |fields: &[(&str, &str)]| {
        let mut place = CanonicalPlace {
            root: facts::PlaceRoot::Symbol(machine.symbol),
            segments: Vec::new(),
        };
        for (data, field) in fields {
            place.segments.push(facts::PlaceSegment::Field {
                symbol: field_symbol(&program, data, field),
            });
        }
        place
    };
    for (write, survives) in [
        (machine_write(&[("Main", "pair"), ("Pair", "a")]), false),
        (machine_write(&[("Main", "pair")]), false),
        (machine_write(&[("Main", "pair"), ("Pair", "b")]), true),
        (machine_write(&[("Main", "unrelated")]), true),
        (parameter_place(&program, state, "offset"), true),
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
            "{write:?}"
        );
    }
}

/// A borrow chain of a place that is not current storage at this statement
/// has no statement-use custody: the same incoming-expression floor a direct
/// member keeps applies through the peel.
#[test]
fn a_borrowed_member_of_foreign_storage_stays_incomplete() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine window(pair: Pair) {
            let cut: i64 = (&pair).a;
            transition { _ -> next(pair) }
            state next(pair: Pair) {
                let cut: i64 = (&pair).a;
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
}
