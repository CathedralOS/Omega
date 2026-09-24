use super::parameter_place;
use crate::checks::ranges::facts::RangeFacts;
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::{TypedTrees, machine::Machine, state::State};

fn atomic_source(body: &str) -> TypedTrees {
    typed_program(&format!(
        "data Host {{
            counter: AtomicU32;
            other: AtomicU32;
            unrelated: i64;
        }}
        machine Host::window(&mut self, index: u64, delta: u32, expected: u32, next: u32) {{
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

fn local_place(program: &TypedTrees, state: &State, name: &str) -> CanonicalPlace {
    let symbol = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == name => Some(local.symbol),
            _ => None,
        })
        .expect("named local");
    CanonicalPlace {
        root: facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    }
}

/// The assignment statement carrying a writing atomic, with its statement
/// index, resident-place target, and carrier value.
fn atomic_assignment(
    program: &TypedTrees,
    state: &State,
) -> (usize, ExpressionHandle, ExpressionHandle) {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match statement {
            StatementNode::Assignment(assignment) => {
                Some((index, assignment.target, assignment.value))
            }
            _ => None,
        })
        .expect("atomic assignment")
}

// The place an atomic's resident expression names, normalized the same way
// `collect_reads` normalizes it. For writing axes the resident place is the
// carrier assignment's `target`; for a load it is `atomic.value`.
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

/// `place.store(v, ordering)` is a complete footprint once the carrier
/// supplies the resident place and the stored operand's reads complete. A
/// literal operand contributes nothing beyond the resident place.
#[test]
fn atomic_store_reads_the_resident_place_and_stored_operand() {
    let program = atomic_source(
        "self.counter.store(delta, NoOrdering);
        let probe: i64 = self.unrelated;",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (statement_index, target, value) = atomic_assignment(&program, state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        panic!("atomic store")
    };
    assert!(matches!(
        atomic.ordering,
        language_core::atomic::AtomicOrderingPlan::Store(_)
    ));
    assert!(!atomic.result.is_valid());
    let counter = canonical_resident_place(&program, machine, state, target);
    let delta = parameter_place(&program, state, "delta");
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert_eq!(
        facts.expression_dependencies[0].reads.as_deref(),
        Some([counter.clone(), delta.clone()].as_slice())
    );
    let label = program.expression_table.display_name(value);
    for (write, survives) in [
        // The resident place and the stored operand are both read; a write to
        // either retires the label. Disjoint writes cannot reach them.
        (counter, false),
        (delta, false),
        (parameter_place(&program, state, "index"), true),
        (local_place(&program, state, "probe"), true),
    ] {
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&[write]))
                .contains(&label),
            survives
        );
    }
    // An empty write report no longer retires the label: the admitted
    // footprint is what a write must be proven disjoint from.
    assert!(
        facts
            .preserved_expression_labels(&program, machine, state, Some(&[]))
            .contains(&label)
    );
}

#[test]
fn atomic_store_of_a_literal_reads_only_the_resident_place() {
    let program = atomic_source("self.counter.store(9, NoOrdering);");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (statement_index, target, value) = atomic_assignment(&program, state);
    let counter = canonical_resident_place(&program, machine, state, target);
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert_eq!(
        facts.expression_dependencies[0].reads.as_deref(),
        Some(std::slice::from_ref(&counter))
    );
}

/// The stored operand's reads join the footprint recursively: a nested load
/// reads its own resident place, so a write to it retires the store's label.
#[test]
fn atomic_store_of_a_nested_load_reads_the_loaded_place_too() {
    let program = atomic_source("self.counter.store(self.other.load(NoOrdering), NoOrdering);");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (statement_index, target, value) = atomic_assignment(&program, state);
    let counter = canonical_resident_place(&program, machine, state, target);
    let ExpressionNode::Atomic(store) = program.expression_table.expression(value) else {
        panic!("atomic store")
    };
    let ExpressionNode::Atomic(load) = program.expression_table.expression(store.value) else {
        panic!("nested load operand")
    };
    let other = canonical_resident_place(&program, machine, state, load.value);
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("store reads");
    assert_eq!(reads.as_slice(), [counter, other.clone()].as_slice());
    let label = program.expression_table.display_name(value);
    assert!(
        !facts
            .preserved_expression_labels(&program, machine, state, Some(&[other]))
            .contains(&label)
    );
}

/// `let r = place.swap(v, ord)` reads the resident place and the replacement
/// operand, and its result destination is part of the carrier's footprint: a
/// write landing on `r` can never slip past the recorded label.
#[test]
fn atomic_swap_reads_resident_replacement_and_result_destination() {
    let program = atomic_source("let prior: u32 = self.counter.swap(delta, NoOrdering);");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (statement_index, target, value) = atomic_assignment(&program, state);
    assert_eq!(
        statement_index, 1,
        "the desugar reserves the result local first"
    );
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        panic!("atomic swap")
    };
    assert!(matches!(
        atomic.ordering,
        language_core::atomic::AtomicOrderingPlan::Swap(_)
    ));
    let counter = canonical_resident_place(&program, machine, state, target);
    let delta = parameter_place(&program, state, "delta");
    let prior = local_place(&program, state, "prior");
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert_eq!(
        facts.expression_dependencies[0].reads.as_deref(),
        Some([counter.clone(), delta.clone(), prior.clone()].as_slice())
    );
    let label = program.expression_table.display_name(value);
    for (write, survives) in [
        (counter, false),
        (delta, false),
        (prior, false),
        (parameter_place(&program, state, "index"), true),
    ] {
        assert_eq!(
            facts
                .preserved_expression_labels(&program, machine, state, Some(&[write]))
                .contains(&label),
            survives
        );
    }
}

/// `let r = place.fetch_add(d, ord)` carries the update model `r + d`. The
/// model's left operand is the observed-prior placeholder pinned to `r` —
/// the resident place already covers that read — so the footprint is the
/// resident place, the fetch operand, and the result destination.
#[test]
fn atomic_fetch_reads_the_update_model_not_the_prior_placeholder() {
    let program = atomic_source("let prior: u32 = self.counter.fetch_add(delta, NoOrdering);");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (statement_index, target, value) = atomic_assignment(&program, state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        panic!("atomic fetch")
    };
    assert!(matches!(
        atomic.ordering,
        language_core::atomic::AtomicOrderingPlan::ReadModifyWrite(_)
    ));
    let ExpressionNode::Binary(update) = program.expression_table.expression(atomic.value) else {
        panic!("fetch update model")
    };
    // The model's left operand is the `prior` placeholder — the same local
    // `result` names — never an operand read.
    for placeholder in [update.left, atomic.result] {
        let ExpressionNode::Name(path) = program.expression_table.expression(placeholder) else {
            panic!("prior placeholder")
        };
        let [member] = program.expression_table.name_path_members(path.members) else {
            panic!("single-member prior name")
        };
        assert_eq!(member.as_str(), "prior");
    }
    let counter = canonical_resident_place(&program, machine, state, target);
    let delta = parameter_place(&program, state, "delta");
    let prior = local_place(&program, state, "prior");
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert_eq!(
        facts.expression_dependencies[0].reads.as_deref(),
        Some([counter, delta, prior].as_slice())
    );
}

/// A decisive compare-exchange reads the resident place plus the expected and
/// replacement operands of the exact prior-shaped update model, and owns its
/// result destination.
#[test]
fn atomic_compare_exchange_reads_expected_and_replacement_operands() {
    let program = atomic_source(
        "let prior: u32 = self.counter.compare_exchange(expected, next, NoOrdering, NoOrdering);",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (statement_index, target, value) = atomic_assignment(&program, state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        panic!("atomic compare-exchange")
    };
    assert!(matches!(
        atomic.ordering,
        language_core::atomic::AtomicOrderingPlan::CompareExchange { .. }
    ));
    let counter = canonical_resident_place(&program, machine, state, target);
    let expected = parameter_place(&program, state, "expected");
    let next = parameter_place(&program, state, "next");
    let prior = local_place(&program, state, "prior");
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert_eq!(
        facts.expression_dependencies[0].reads.as_deref(),
        Some([counter.clone(), expected, next, prior].as_slice())
    );
    let label = program.expression_table.display_name(value);
    assert!(
        !facts
            .preserved_expression_labels(&program, machine, state, Some(&[counter]))
            .contains(&label)
    );
}

/// The admitted footprint is evidence, not syntax: any axis whose carrier,
/// ordering, custody, result destination, or update model cannot be proven
/// stays incomplete rather than claiming the resident place alone.
#[test]
fn writing_atomic_axes_without_a_complete_footprint_stay_incomplete() {
    use language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};

    // A stored operand whose own reads cannot be proven keeps the store
    // incomplete: the resident place alone is not the footprint.
    let program = typed_program(
        "data Host { counter: AtomicU32; }
        machine compute(value: u32) -> u32 { value }
        machine Host::window(&mut self, delta: u32) {
            self.counter.store(compute(delta), NoOrdering);
        }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() != "compute")
        .expect("window");
    let state = &program.machine_states(machine)[0];
    let (statement_index, _, value) = atomic_assignment(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // The carrier at this statement index supplies the resident place; an
    // atomic recorded against a different statement has none.
    let program = atomic_source(
        "self.counter.store(9, NoOrdering);
        let probe: i64 = self.unrelated;",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (_, _, value) = atomic_assignment(&program, state);
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = 1;
    facts.record_expression_dependencies(&program, machine, state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A store carrying a result destination is not the store axis's shape.
    let mut program = atomic_source("self.counter.store(9, NoOrdering);");
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic store")
    };
    atomic.result = value;
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // An ordering the operation cannot mean is not a legal plan.
    let mut program = atomic_source("self.counter.store(9, NoOrdering);");
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic store")
    };
    atomic.ordering = AtomicOrderingPlan::Store(MemoryOrdering::Receive);
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // The observing single-attempt custody cannot carry a scalar store.
    let mut program = atomic_source("self.counter.store(9, NoOrdering);");
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic store")
    };
    atomic.result_custody =
        language_core::atomic::AtomicExpressionResultCustody::ObservingCompareExchangeOnce(
            language_core::atomic::AtomicCompareExchangeOnceResultCustody::CANONICAL,
        );
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A swap without a result destination cannot pin the displaced prior.
    let mut program = atomic_source("let prior: u32 = self.counter.swap(delta, NoOrdering);");
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic swap")
    };
    atomic.result = ExpressionHandle::invalid();
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A result destination that names non-local storage is not the carrier's
    // same-symbol result local.
    let mut program = atomic_source(
        "let prior: u32 = self.counter.swap(delta, NoOrdering);
        let probe: i64 = self.unrelated;",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let member = local_initializer(&program, &state, "probe");
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic swap")
    };
    atomic.result = member;
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A result destination naming a local not yet current at the carrier is
    // a foreign destination, not the reserved result slot.
    let mut program = atomic_source(
        "let prior: u32 = self.counter.swap(delta, NoOrdering);
        let later: u32 = 7;
        let probe: u32 = later;",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let later = local_initializer(&program, &state, "probe");
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic swap")
    };
    atomic.result = later;
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A fetch model whose left operand is not the result placeholder is a
    // substituted update, not `prior OP operand`.
    let mut program = atomic_source("let prior: u32 = self.counter.fetch_add(delta, NoOrdering);");
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        panic!("atomic fetch")
    };
    let update = atomic.value;
    let ExpressionNode::Binary(update) = program.expression_table.expression_mut(update) else {
        panic!("fetch update model")
    };
    update.left = update.right;
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // The single-attempt observing form's failure path can be an uncommitted
    // attempt — no scalar prior footprint exists to admit.
    let mut program = atomic_source(
        "let prior: u32 = self.counter.compare_exchange(expected, next, NoOrdering, NoOrdering);",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic compare-exchange")
    };
    atomic.ordering = AtomicOrderingPlan::CompareExchangeOnce {
        success: MemoryOrdering::NoOrdering,
        failure: MemoryOrdering::NoOrdering,
    };
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A failure ordering that cannot legally pair with the success ordering
    // is not a compare-exchange plan at all.
    let mut program = atomic_source(
        "let prior: u32 = self.counter.compare_exchange(expected, next, NoOrdering, NoOrdering);",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression_mut(value) else {
        panic!("atomic compare-exchange")
    };
    atomic.ordering = AtomicOrderingPlan::CompareExchange {
        success: MemoryOrdering::Publish,
        failure: MemoryOrdering::Receive,
    };
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());

    // A compare-exchange model whose `prior` add-operand is replaced by the
    // expected operand is a substituted update, not the carrier's shape.
    let mut program = atomic_source(
        "let prior: u32 = self.counter.compare_exchange(expected, next, NoOrdering, NoOrdering);",
    );
    let machine = program.machines()[0].clone();
    let state = program.machine_states(&machine)[0].clone();
    let (statement_index, _, value) = atomic_assignment(&program, &state);
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(value) else {
        panic!("atomic compare-exchange")
    };
    let model = atomic.value;
    let ExpressionNode::Binary(sum) = program.expression_table.expression(model) else {
        panic!("compare-exchange model")
    };
    let ExpressionNode::Binary(product) = program.expression_table.expression(sum.right) else {
        panic!("compare-exchange product")
    };
    let ExpressionNode::Binary(equal) = program.expression_table.expression(product.left) else {
        panic!("compare-exchange equality")
    };
    let expected_operand = equal.right;
    let ExpressionNode::Binary(sum) = program.expression_table.expression_mut(model) else {
        panic!("compare-exchange model")
    };
    sum.left = expected_operand;
    let mut facts = RangeFacts::new(&[]);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, &machine, &state, value);
    assert!(facts.expression_dependencies[0].reads.is_none());
}
