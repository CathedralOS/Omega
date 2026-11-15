// `decide` first gives the prover and kernel only the facts that share a
// value with the goal, directly or through other facts, plus facts that name
// no value (`crate::connected`). These tests pin which facts that keeps.

use super::{id, number, u32_type};
use crate::{Hypotheses, connected};
use semantic_vocabulary::{IdTable, Proposition, ScalarTerm, ValueId};
use terminal_check_tables::KnowledgeId;

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(id::<ValueId>(raw), u32_type())
}

fn at_most(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(left, right)
}

/// Rows for `assumptions` then `axioms`, in that order.
fn hypotheses(assumptions: Vec<Proposition>, axioms: Vec<Proposition>) -> Hypotheses {
    let mut rows: IdTable<KnowledgeId, ()> = IdTable::default();
    Hypotheses {
        assumption_rows: assumptions.iter().map(|_| rows.push(())).collect(),
        axiom_rows: axioms.iter().map(|_| rows.push(())).collect(),
        assumptions,
        axioms,
    }
}

#[test]
fn a_fact_about_unrelated_values_is_left_out_and_a_chain_is_kept() {
    // Goal `v1 <= 10`. `v1 <= v2` and `v2 <= 10` reach it through `v2`;
    // `1 <= 2` names no value; `v3 <= 5` and `v4 <= v3` share nothing with it.
    let goal = at_most(value(1), number(10));
    let known = hypotheses(
        vec![
            at_most(value(1), value(2)),
            at_most(value(3), number(5)),
            at_most(number(1), number(2)),
        ],
        vec![at_most(value(4), value(3)), at_most(value(2), number(10))],
    );
    let narrowed = connected(&goal, &known).expect("unrelated facts are dropped");
    assert_eq!(
        narrowed.assumptions,
        [known.assumptions[0].clone(), known.assumptions[2].clone()]
    );
    assert_eq!(
        narrowed.assumption_rows,
        [known.assumption_rows[0], known.assumption_rows[2]]
    );
    assert_eq!(narrowed.axioms, [known.axioms[1].clone()]);
    assert_eq!(narrowed.axiom_rows, [known.axiom_rows[1]]);
}

#[test]
fn when_every_fact_is_connected_the_full_set_is_used() {
    let goal = at_most(value(1), number(10));
    let known = hypotheses(
        vec![at_most(value(1), value(2))],
        vec![at_most(value(2), value(3))],
    );
    assert!(connected(&goal, &known).is_none());
}
