use super::{ExpressionHandle, ExpressionNode, SymbolHandle, TypedTrees};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::dependencies::tests::initializer;
use crate::checks::ranges::facts::dependencies::tests::parameter_place;
use crate::tests::front_end::typed_program;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

fn window(program: &TypedTrees) -> (&Machine, &State) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("window"))
        .expect("window");
    (machine, &program.machine_states(machine)[0])
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

/// Record the `let cut = ..` initializer's dependencies with the statement
/// index the statement checker would have set, returning its display label.
fn record_label(
    facts: &mut RangeFacts,
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> String {
    let expression = initializer(program, state);
    facts.statement_index = statement_index_of(program, state, "cut");
    facts.record_expression_dependencies(program, machine, state, expression);
    program.expression_table.display_name(expression)
}

/// A member spelling that resolves to no declared field of the receiver's
/// type has no honest identity: dropping the binder's symbol alone is not
/// enough while the contextual walk still finds `Pair::a`, so the drifted
/// name must be unresolvable on `Pair`.
#[test]
fn a_temporary_member_with_unresolved_identity_stays_incomplete() {
    let mut program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine window(left: i64, right: i64, unrelated: i64) {
            let cut: i64 = Pair { a: left, b: right }.a;
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
    facts.bound_program = Some(&program);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// A `match` collection recovers no element type the range checker's own
/// reader can stand on, so builtin index meaning cannot be established and
/// the temporary collection keeps an incomplete read set — the same boundary
/// the match-receiver member keeps.
#[test]
fn a_match_collection_index_stays_incomplete() {
    let program = typed_program(
        "machine window(flag: i64, left: &[i64; 4], right: &[i64; 4], index: u64, unrelated: i64) {
            let cut: i64 = match flag { 0 -> left, _ -> right }[index];
        }",
    );
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// Arms whose declared result types resolve to different declarations give
/// the member no single honest identity — `left` and `right` both carry a
/// field spelled `a`, but `.a` cannot name one declared field across the
/// dispatch.
#[test]
fn a_match_receiver_member_with_disagreeing_arm_leaves_stays_incomplete() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        data Other { a: i64; }
        machine window(flag: i64, left: Pair, right: Other, unrelated: i64) {
            let cut: i64 = match flag { 0 -> left, _ -> right }.a;
        }",
    );
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// An arm whose declared result type cannot be recovered — a borrow has no
/// by-value result type — leaves the member identity unproven, so the read
/// set stays incomplete rather than guessing at the surviving peer.
#[test]
fn a_match_receiver_member_with_an_untyped_arm_stays_incomplete() {
    let program = typed_program(
        "data Pair { a: i64; b: i64; }
        machine window(flag: i64, left: Pair, right: Pair, unrelated: i64) {
            let cut: i64 = match flag { 0 -> left, _ -> &right }.a;
        }",
    );
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// A deeper member chain through a `match` receiver names no storage either:
/// `match .. .inner.a` still reads exactly the dispatch footprint. The outer
/// member's identity is recovered the same way the direct match member's is —
/// the arms agree on `Outer`, `inner` is its declared field, and `a` is a
/// declared field of `inner`'s declared type — so each hop resolves on the
/// previous hop's declaration leaf without a place position.
#[test]
fn a_match_receiver_member_chain_reads_the_match_footprint() {
    let program = typed_program(
        "data Inner { a: i64; b: i64; }
        data Outer { inner: Inner; other: i64; }
        machine window(flag: i64, left: Outer, right: Outer, unrelated: i64) {
            let cut: i64 = match flag { 0 -> left, _ -> right }.inner.a;
        }",
    );
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    let label = record_label(&mut facts, &program, machine, state);
    let reads = facts.expression_dependencies[0]
        .reads
        .as_ref()
        .expect("match-receiver member-chain footprint");
    assert_eq!(
        reads.as_slice(),
        [
            parameter_place(&program, state, "flag"),
            parameter_place(&program, state, "left"),
            parameter_place(&program, state, "right"),
        ]
        .as_slice(),
        "{reads:?}"
    );
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

/// An intermediate hop that names no field of the arms' agreed declaration
/// breaks the chain: `match .. .missing.a` cannot establish which declaration
/// `.a` belongs to, so the read set stays incomplete.
#[test]
fn a_match_receiver_member_chain_with_an_unresolved_hop_stays_incomplete() {
    let mut program = typed_program(
        "data Inner { a: i64; b: i64; }
        data Outer { inner: Inner; other: i64; }
        machine window(flag: i64, left: Outer, right: Outer, unrelated: i64) {
            let cut: i64 = match flag { 0 -> left, _ -> right }.inner.a;
        }",
    );
    let expression = {
        let (_, state) = window(&program);
        initializer(&program, state)
    };
    let ExpressionNode::Member(outer) = program.expression_table.expression(expression) else {
        panic!("member-chain fixture")
    };
    let receiver = outer.receiver;
    let ExpressionNode::Member(inner) = program.expression_table.expression_mut(receiver) else {
        panic!("member-chain receiver fixture")
    };
    inner.member = "missing".into();
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    record_label(&mut facts, &program, machine, state);
    assert!(facts.expression_dependencies[0].reads.is_none());
}

/// The outermost member chain inside `expression` carrying a case-qualified
/// hop — the operand spelling `seed_binary_guard_facts` hands the recorder
/// for a desugared destructure guard's comparison.
fn case_member_operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => (member.case_variant.is_some()
            || case_member_operand(program, member.receiver).is_some())
        .then_some(expression),
        ExpressionNode::Binary(binary) => case_member_operand(program, binary.left)
            .or_else(|| case_member_operand(program, binary.right)),
        ExpressionNode::Unary(unary) => case_member_operand(program, unary.operand),
        _ => None,
    }
}

/// The transition `When` guard in `state`, with the statement index the
/// checker would have set for it.
fn case_guard(program: &TypedTrees, state: &State) -> (usize, ExpressionHandle) {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(statement_index, statement)| match statement {
            StatementNode::Transition(transition) => match transition.guard {
                typed_trees::statement::TransitionGuardNode::When(guard) => {
                    Some((statement_index, guard))
                }
                _ => None,
            },
            _ => None,
        })
        .expect("guarded transition")
}

/// A member above a case-qualified receiver whose own field fails to resolve
/// keeps the whole chain incomplete: `inner@Strike` names no such payload
/// field, so `.v` above it has no receiver type to stand on.
#[test]
fn a_member_above_an_unresolved_case_hop_stays_incomplete() {
    let mut program = typed_program(
        "data Inner { v: i64; }
        data Attack {
            case Idle;
            case Strike(inner: Inner, bonus: i64);
        }
        data Main { attack: Attack; }
        machine Main::window(&mut self, unrelated: i64) {
            transition self.attack {
                Attack::Strike { inner, bonus } if inner.v == 10 -> done(bonus)
                _ -> done(0)
            }
            state done(&mut self, power: i64) { let z: i64 = power; }
        }",
    );
    let (statement_index, operand) = {
        let (_, state) = window(&program);
        let (statement_index, guard) = case_guard(&program, state);
        let operand = case_member_operand(&program, guard).expect("case-qualified operand");
        (statement_index, operand)
    };
    let ExpressionNode::Member(outer) = program.expression_table.expression(operand) else {
        panic!("member fixture")
    };
    let receiver = outer.receiver;
    let ExpressionNode::Member(inner) = program.expression_table.expression_mut(receiver) else {
        panic!("case-member receiver fixture")
    };
    inner.member = "missing".into();
    let (machine, state) = window(&program);
    let mut facts = RangeFacts::new(&[]);
    facts.bound_program = Some(&program);
    facts.statement_index = statement_index;
    facts.record_expression_dependencies(&program, machine, state, operand);
    assert!(facts.expression_dependencies[0].reads.is_none());
}
