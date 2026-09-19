//! Explicit integer relations stated by the forming scope's `requires`
//! contracts.
//!
//! A premised compatibility derivation consumes the same establishment point
//! as the range checker: machine `requires` apply at the machine's entry
//! state and a state's own `requires` apply in that state. Each premise keeps
//! its exact `ContractProofFact` token and both operands normalized onto the
//! immutable-bound vocabulary the selector snapshot already uses, so replay
//! can re-derive the available set without trusting a serialized relation.
//! Premises prove bound ordering, equality or disequality the structural
//! judgment could not. Disequality distinguishes singleton elements without
//! choosing an ordering; it says nothing about overlap of wider windows.
//! No relation can form a loan, extend a lifetime, or widen access.

use checked_trees::{
    BorrowCompatibilityPremise, BorrowCompatibilityPremiseRelation, ContractProofFact,
    ContractProofFactKind, ContractProofFactOwner,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::indexes::{NormalizedBound, normalized_bound, selector_value};

/// One ordering relation an exact `requires` row states over normalized
/// immutable bounds. The `fact` handle is the premise's durable identity; the
/// normalized operands are what bound queries shift against.
#[derive(Debug, Clone, Copy)]
pub struct StatedOrderingPremise {
    fact: arena::Handle<ContractProofFact>,
    relation: BorrowCompatibilityPremiseRelation,
    left: NormalizedBound,
    right: NormalizedBound,
}

impl StatedOrderingPremise {
    /// The exact recorded identity of this premise in a `Premised`
    /// derivation, in the order the contract row stated the relation.
    pub fn token(&self) -> BorrowCompatibilityPremise {
        BorrowCompatibilityPremise {
            fact: self.fact,
            relation: self.relation,
            left: selector_value(self.left),
            right: selector_value(self.right),
        }
    }
}

/// Collect the ordering premises available at one formation scope.
///
/// Owner filtering mirrors `ranges::requirements::seed_state_requires`: a
/// machine-level `requires` belongs to the machine's entry state, while a
/// state-level `requires` belongs to its exact state. Inherited conformance
/// rows are excluded because the range establishment point reads only the
/// authored `machine_contracts`/`state_contracts` surfaces. Facts that do not
/// decompose into builtin integer comparisons over immutable bounds
/// contribute no premise.
pub fn stated_ordering_premises(
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine: &Machine,
    state: &State,
) -> Vec<StatedOrderingPremise> {
    let is_entry = state.symbol.is_valid()
        && program
            .machine_states(machine)
            .first()
            .is_some_and(|entry| entry.symbol == state.symbol);
    let mut premises = Vec::new();
    for (fact, row) in facts.proof.contract_facts.iter() {
        if row.kind != ContractProofFactKind::Requires || row.inherited_scope.is_some() {
            continue;
        }
        let owned = match row.owner {
            ContractProofFactOwner::Machine { machine_symbol } => {
                is_entry && machine_symbol == machine.symbol
            }
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } => machine_symbol == machine.symbol && state_symbol == state.symbol,
            _ => false,
        };
        if !owned {
            continue;
        }
        let typed_trees::domain::ProofFact::Expression(expression) =
            program.proof_facts.get(row.fact)
        else {
            continue;
        };
        decompose_premise_expression(program, machine, state, *expression, fact, &mut premises);
    }
    premises
}

/// Decompose one `requires` expression into atomic ordering premises.
///
/// Conjunctions contribute each conjunct independently; `>`/`>=` normalize to
/// the flipped `<`/`<=` premise so stored relations stay canonical. Every
/// node is gated by the same builtin-meaning check the range guard seeder
/// applies, so an overloaded comparison cannot masquerade as integer
/// ordering.
fn decompose_premise_expression(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    fact: arena::Handle<ContractProofFact>,
    premises: &mut Vec<StatedOrderingPremise>,
) {
    if !expression.is_valid()
        || !validation::has_builtin_decomposed_guard_meaning(
            program,
            machine,
            Some(state),
            expression,
        )
    {
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    use BorrowCompatibilityPremiseRelation as Relation;
    let (relation, left, right) = match binary.operator {
        BinaryOperator::And => {
            decompose_premise_expression(program, machine, state, binary.left, fact, premises);
            decompose_premise_expression(program, machine, state, binary.right, fact, premises);
            return;
        }
        BinaryOperator::Less => (Relation::StrictlyBefore, binary.left, binary.right),
        BinaryOperator::LessOrEqual => (Relation::LessOrEqual, binary.left, binary.right),
        BinaryOperator::Greater => (Relation::StrictlyBefore, binary.right, binary.left),
        BinaryOperator::GreaterOrEqual => (Relation::LessOrEqual, binary.right, binary.left),
        BinaryOperator::Equal => (Relation::Equal, binary.left, binary.right),
        BinaryOperator::NotEqual => (Relation::NotEqual, binary.left, binary.right),
        _ => return,
    };
    let (Some(left), Some(right)) = (
        normalized_bound(program, left),
        normalized_bound(program, right),
    ) else {
        return;
    };
    premises.push(StatedOrderingPremise {
        fact,
        relation,
        left,
        right,
    });
}

/// Whether `premise` proves `left <query> right`.
///
/// The query shifts against the premise's endpoints only within one symbol's
/// constant-offset line or the integer line; unrelated bound pairs stay
/// unproven. Equality and disequality are symmetric, so their endpoints are
/// consulted in both orientations.
pub fn premise_proves(
    premise: &StatedOrderingPremise,
    left: NormalizedBound,
    query: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> bool {
    premise_orientation_proves(
        premise.left,
        premise.relation,
        premise.right,
        left,
        query,
        right,
    ) || (matches!(
        premise.relation,
        BorrowCompatibilityPremiseRelation::Equal | BorrowCompatibilityPremiseRelation::NotEqual
    ) && premise_orientation_proves(
        premise.right,
        premise.relation,
        premise.left,
        left,
        query,
        right,
    ))
}

/// Whether `premise_left <premise> premise_right` proves
/// `left <query> right`, with `left = premise_left + d1` and
/// `right = premise_right + d2` under the constant-offset algebra.
fn premise_orientation_proves(
    premise_left: NormalizedBound,
    premise: BorrowCompatibilityPremiseRelation,
    premise_right: NormalizedBound,
    left: NormalizedBound,
    query: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> bool {
    let (Some(left_shift), Some(right_shift)) = (
        bound_shift(left, premise_left),
        bound_shift(right, premise_right),
    ) else {
        return false;
    };
    use BorrowCompatibilityPremiseRelation as Relation;
    match (premise, query) {
        // `L <= R` gives `L + d1 <= R + d2` when `d1 <= d2`, and the strict
        // `L + d1 < R + d2` when `d1 < d2`. One direction carries no equality.
        (Relation::LessOrEqual, Relation::LessOrEqual) => left_shift <= right_shift,
        (Relation::LessOrEqual, Relation::StrictlyBefore) => left_shift < right_shift,
        (Relation::LessOrEqual, Relation::Equal) => false,
        (Relation::LessOrEqual, Relation::NotEqual) => left_shift < right_shift,
        // `L < R` is `L + 1 <= R`, so `L + d1 <= R + d2` holds when
        // `d1 <= d2 + 1`, and `L + d1 < R + d2` already when `d1 <= d2`.
        (Relation::StrictlyBefore, Relation::LessOrEqual) => right_shift
            .checked_add(1)
            .is_none_or(|bound| left_shift <= bound),
        (Relation::StrictlyBefore, Relation::StrictlyBefore) => left_shift <= right_shift,
        (Relation::StrictlyBefore, Relation::Equal) => false,
        (Relation::StrictlyBefore, Relation::NotEqual) => left_shift <= right_shift,
        // `L == R` shifts either endpoint by the same constant in every
        // comparison direction.
        (Relation::Equal, Relation::LessOrEqual) => left_shift <= right_shift,
        (Relation::Equal, Relation::StrictlyBefore) => left_shift < right_shift,
        (Relation::Equal, Relation::Equal) => left_shift == right_shift,
        (Relation::Equal, Relation::NotEqual) => left_shift != right_shift,
        // Translation by the same mathematical integer preserves inequality.
        // Unequal shifts could collapse two distinct points onto one; `!=`
        // never determines which point precedes the other.
        (Relation::NotEqual, Relation::NotEqual) => left_shift == right_shift,
        (Relation::NotEqual, _) => false,
    }
}

/// A premise with no durable fact identity, for unit tests that exercise the
/// ordering consult without a contract-fact arena.
#[cfg(test)]
pub(super) fn ordering_premise(
    left: NormalizedBound,
    relation: BorrowCompatibilityPremiseRelation,
    right: NormalizedBound,
) -> StatedOrderingPremise {
    StatedOrderingPremise {
        fact: arena::Handle::invalid(),
        relation,
        left,
        right,
    }
}

/// `value - base` when both bounds sit on one symbol's offset line, on one
/// canonical two-symbol sum's offset line, or both are integers. Distinct
/// term sets stay unrelated, never negative evidence.
fn bound_shift(value: NormalizedBound, base: NormalizedBound) -> Option<i64> {
    match (value, base) {
        (NormalizedBound::Integer(value), NormalizedBound::Integer(base)) => {
            value.checked_sub(base)
        }
        (
            NormalizedBound::Symbol {
                symbol: value_symbol,
                offset: value_offset,
            },
            NormalizedBound::Symbol {
                symbol: base_symbol,
                offset: base_offset,
            },
        ) if value_symbol == base_symbol => value_offset.checked_sub(base_offset),
        (
            NormalizedBound::SymbolSum {
                first: value_first,
                second: value_second,
                offset: value_offset,
            },
            NormalizedBound::SymbolSum {
                first: base_first,
                second: base_second,
                offset: base_offset,
            },
        ) if value_first == base_first && value_second == base_second => {
            value_offset.checked_sub(base_offset)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::BorrowCompatibilityPremiseRelation;
    use crate::checks::borrows::overlap::StatedOrderingPremise;
    use crate::checks::borrows::overlap::premises::NormalizedBound;
    use crate::checks::borrows::overlap::premises::premise_proves;

    fn symbol(index: u32) -> symbols::SymbolHandle {
        symbols::SymbolHandle::from_arena_index(index)
    }

    fn integer(value: i64) -> NormalizedBound {
        NormalizedBound::Integer(value)
    }

    fn sym(index: u32, offset: i64) -> NormalizedBound {
        NormalizedBound::Symbol {
            symbol: symbol(index),
            offset,
        }
    }

    fn sum(first: u32, second: u32, offset: i64) -> NormalizedBound {
        NormalizedBound::SymbolSum {
            first: symbol(first),
            second: symbol(second),
            offset,
        }
    }

    fn premise(
        left: NormalizedBound,
        relation: BorrowCompatibilityPremiseRelation,
        right: NormalizedBound,
    ) -> StatedOrderingPremise {
        StatedOrderingPremise {
            fact: arena::Handle::invalid(),
            relation,
            left,
            right,
        }
    }

    use BorrowCompatibilityPremiseRelation as Relation;

    #[test]
    fn disequality_is_symmetric_and_requires_equal_translation() {
        let distinct = premise(sym(1, 0), Relation::NotEqual, sym(2, 0));
        for offset in [-2, 0, 3] {
            for (left, right) in [(1, 2), (2, 1)] {
                assert!(premise_proves(
                    &distinct,
                    sym(left, offset),
                    Relation::NotEqual,
                    sym(right, offset),
                ));
                for relation in [
                    Relation::LessOrEqual,
                    Relation::StrictlyBefore,
                    Relation::Equal,
                ] {
                    assert!(!premise_proves(
                        &distinct,
                        sym(left, offset),
                        relation,
                        sym(right, offset),
                    ));
                }
                assert!(!premise_proves(
                    &distinct,
                    sym(left, offset),
                    Relation::NotEqual,
                    sym(right, offset + 1),
                ));
            }
        }
        assert!(!premise_proves(
            &distinct,
            sym(1, 0),
            Relation::NotEqual,
            sym(3, 0)
        ));
    }

    #[test]
    fn shifted_relation_rules_hold_for_concrete_integer_interpretations() {
        fn holds(relation: Relation, left: i64, right: i64) -> bool {
            match relation {
                Relation::LessOrEqual => left <= right,
                Relation::StrictlyBefore => left < right,
                Relation::Equal => left == right,
                Relation::NotEqual => left != right,
            }
        }
        let relations = [
            Relation::LessOrEqual,
            Relation::StrictlyBefore,
            Relation::Equal,
            Relation::NotEqual,
        ];
        for relation in relations {
            let stated = premise(sym(1, 0), relation, sym(2, 0));
            for query in relations {
                for left_shift in -2..=2 {
                    for right_shift in -2..=2 {
                        if !premise_proves(&stated, sym(1, left_shift), query, sym(2, right_shift))
                        {
                            continue;
                        }
                        for left in -3..=3 {
                            for right in -3..=3 {
                                if holds(relation, left, right) {
                                    assert!(
                                        holds(query, left + left_shift, right + right_shift),
                                        "{left} {relation:?} {right}, shifts {left_shift}/{right_shift}, query {query:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn stated_less_or_equal_proves_shifted_window_boundaries() {
        let cut_le_last = premise(sym(1, 0), Relation::LessOrEqual, sym(2, 0));
        assert!(premise_proves(
            &cut_le_last,
            sym(1, 0),
            Relation::LessOrEqual,
            sym(2, 0)
        ));
        // `cut + 1 <= last` does not follow from `cut <= last`.
        assert!(!premise_proves(
            &cut_le_last,
            sym(1, 1),
            Relation::LessOrEqual,
            sym(2, 0)
        ));
        assert!(premise_proves(
            &cut_le_last,
            sym(1, 0),
            Relation::LessOrEqual,
            sym(2, 2)
        ));
        // Strict queries need a strict shift margin.
        assert!(premise_proves(
            &cut_le_last,
            sym(1, 0),
            Relation::StrictlyBefore,
            sym(2, 1)
        ));
        assert!(!premise_proves(
            &cut_le_last,
            sym(1, 0),
            Relation::StrictlyBefore,
            sym(2, 0)
        ));
        // Unrelated symbols and a bare `<=` never prove equality.
        assert!(!premise_proves(
            &cut_le_last,
            sym(1, 0),
            Relation::LessOrEqual,
            sym(3, 0)
        ));
        assert!(!premise_proves(
            &cut_le_last,
            sym(1, 0),
            Relation::Equal,
            sym(2, 0)
        ));
    }

    #[test]
    fn stated_strict_order_proves_one_step_of_non_strict_adjacency() {
        let cut_lt_last = premise(sym(1, 0), Relation::StrictlyBefore, sym(2, 0));
        // `cut < last` proves `cut + 1 <= last` but not `cut + 2 <= last`.
        assert!(premise_proves(
            &cut_lt_last,
            sym(1, 1),
            Relation::LessOrEqual,
            sym(2, 0)
        ));
        assert!(!premise_proves(
            &cut_lt_last,
            sym(1, 2),
            Relation::LessOrEqual,
            sym(2, 0)
        ));
        assert!(premise_proves(
            &cut_lt_last,
            sym(1, 0),
            Relation::StrictlyBefore,
            sym(2, 0)
        ));
        assert!(premise_proves(
            &cut_lt_last,
            sym(1, 1),
            Relation::StrictlyBefore,
            sym(2, 1)
        ));
    }

    #[test]
    fn stated_equality_proves_all_directions_in_both_orientations() {
        let cut_eq_last = premise(sym(1, 0), Relation::Equal, sym(2, 0));
        assert!(premise_proves(
            &cut_eq_last,
            sym(1, 0),
            Relation::Equal,
            sym(2, 0)
        ));
        // Equality is symmetric: `last + 1 == cut + 1` also follows.
        assert!(premise_proves(
            &cut_eq_last,
            sym(2, 1),
            Relation::Equal,
            sym(1, 1)
        ));
        assert!(premise_proves(
            &cut_eq_last,
            sym(1, 0),
            Relation::StrictlyBefore,
            sym(2, 1)
        ));
        assert!(premise_proves(
            &cut_eq_last,
            sym(1, 0),
            Relation::LessOrEqual,
            sym(2, 0)
        ));
        assert!(!premise_proves(
            &cut_eq_last,
            sym(1, 0),
            Relation::StrictlyBefore,
            sym(2, 0)
        ));
    }

    #[test]
    fn integer_premises_shift_on_the_plain_number_line() {
        let cut_at_most_four = premise(sym(1, 0), Relation::LessOrEqual, integer(4));
        assert!(premise_proves(
            &cut_at_most_four,
            sym(1, 0),
            Relation::LessOrEqual,
            integer(4)
        ));
        assert!(premise_proves(
            &cut_at_most_four,
            sym(1, -1),
            Relation::LessOrEqual,
            integer(4)
        ));
        assert!(!premise_proves(
            &cut_at_most_four,
            sym(1, 1),
            Relation::LessOrEqual,
            integer(4)
        ));
        let four_at_most_last = premise(integer(4), Relation::LessOrEqual, sym(2, 0));
        assert!(premise_proves(
            &four_at_most_last,
            integer(4),
            Relation::LessOrEqual,
            sym(2, 1)
        ));
        assert!(!premise_proves(
            &four_at_most_last,
            integer(4),
            Relation::LessOrEqual,
            sym(2, -1)
        ));
    }

    #[test]
    fn summed_premises_shift_on_their_own_offset_line() {
        // `i + j < cut` proves the same sum strictly before `cut`, plus its
        // constant shifts within the margin.
        let sum_lt_cut = premise(sum(1, 2, 0), Relation::StrictlyBefore, sym(3, 0));
        assert!(premise_proves(
            &sum_lt_cut,
            sum(1, 2, 0),
            Relation::StrictlyBefore,
            sym(3, 0)
        ));
        assert!(premise_proves(
            &sum_lt_cut,
            sum(1, 2, -1),
            Relation::LessOrEqual,
            sym(3, 0)
        ));
        // A shifted sum does not inherit the strict relation.
        assert!(!premise_proves(
            &sum_lt_cut,
            sum(1, 2, 1),
            Relation::StrictlyBefore,
            sym(3, 0)
        ));
        // A different term set stays unrelated even at the same offset.
        assert!(!premise_proves(
            &sum_lt_cut,
            sum(1, 4, 0),
            Relation::StrictlyBefore,
            sym(3, 0)
        ));
        assert!(!premise_proves(
            &sum_lt_cut,
            sym(1, 0),
            Relation::StrictlyBefore,
            sym(3, 0)
        ));
    }
}
