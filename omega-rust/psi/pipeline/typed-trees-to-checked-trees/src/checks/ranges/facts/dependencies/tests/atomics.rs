use super::{parameter_place, typed_source};
use crate::checks::ranges::facts::RangeFacts;
use crate::flow::CanonicalPlace;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::{TypedTrees, machine::Machine, state::State};

fn atomic_source(body: &str) -> TypedTrees {
    typed_source(&format!(
        "data Host {{
            counter: AtomicU32;
            unrelated: i64;
        }}
        machine Host::window(&mut self, index: u64) {{
            {body}
        }}"
    ))
}

fn local_initializer(program: &TypedTrees, state: &State, name: &str) -> ExpressionHandle {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == name => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .expect("named local")
}

// The place a load reads is the canonical place of its resident `value`
// expression, normalized the same way `collect_reads` normalizes it.
fn canonical_resident_place(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> CanonicalPlace {
    let mut place =
        crate::flow::canonical_place_from_expression_in_state(program, state.symbol, 0, expression)
            .expect("canonical resident place");
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
    place
}

#[test]
fn atomic_load_reads_are_exactly_the_resident_place() {
    let program = atomic_source(
        "let cut: u32 = self.counter.load(NoOrdering);
        let probe: i64 = self.unrelated;",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let expression = local_initializer(&program, state, "cut");
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
        panic!("atomic load")
    };
    let counter = canonical_resident_place(&program, machine, state, atomic.value);
    let unrelated = canonical_resident_place(
        &program,
        machine,
        state,
        local_initializer(&program, state, "probe"),
    );
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, expression);
    assert_eq!(
        facts.expression_dependencies[0].reads.as_deref(),
        Some(std::slice::from_ref(&counter))
    );
    let label = program.expression_table.display_name(expression);
    for (write, survives) in [
        // A write to the resident place retires the label; a write to a
        // sibling field or an unrelated parameter cannot reach it. A write
        // to `self` itself covers the cell, so it also retires the label.
        (counter.clone(), false),
        (unrelated, true),
        (parameter_place(&program, state, "index"), true),
        (
            CanonicalPlace {
                root: counter.root,
                segments: Vec::new(),
            },
            false,
        ),
    ] {
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&[write]))
                .contains(&label),
            survives
        );
    }
}

#[test]
fn writing_atomic_axes_have_no_place_footprint() {
    let program = atomic_source(
        "self.counter.store(9, NoOrdering);
        let cut: i64 = self.unrelated;",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let assignment_value = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Assignment(assignment) => Some(assignment.value),
            _ => None,
        })
        .expect("store assignment");
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment_value)
    else {
        panic!("atomic store")
    };
    assert!(matches!(
        atomic.ordering,
        language_core::atomic::AtomicOrderingPlan::Store(_)
    ));
    let mut facts = RangeFacts::new(&[]);
    facts.record_expression_dependencies(&program, machine, state, assignment_value);
    // The resident place the store writes is not its read footprint, so the
    // label can never survive a write report, not even an empty one.
    assert!(facts.expression_dependencies[0].reads.is_none());
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&[]))
            .is_empty()
    );
}
