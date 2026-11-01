//! Pure integer premise ordering shared by checking clients.
//!
//! Clients establish availability, exact storage versions, selected builtin
//! meaning and formal/actual bindings before constructing these local records.
//! The oracle only composes those premises: it cannot form a loan, extend a
//! lifetime, create authority or treat a mutable storage name as value evidence.
//! Checked offset arithmetic and the existing query budget remain conservative.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegerBound {
    Integer(i64),
    /// Typed builtin runtime element value under one immutable formal frame.
    /// This is neither a structural place key nor evidence of an index bound.
    EntryRead(crate::ImmutableEntryReadIdentity),
    Symbol {
        symbol: SymbolHandle,
        offset: i64,
    },
    /// The value currently stored under one mutable storage symbol. The
    /// storage name is a query coordinate, never positive evidence: storage
    /// bounds never order structurally (the storage may hold a different
    /// occurrence at each statement), so stated evidence can only claim them
    /// in a scope where the establishment binding is version-pinned.
    Storage {
        symbol: SymbolHandle,
    },
    /// The value at one projected place under an immutable storage symbol
    /// (`pair.first`, `pair.items[2]`): a frozen identity of the bound
    /// value's place, equal only to the same projection path of the same
    /// symbol. Projections whose path carries unresolved segments stay
    /// outside the vocabulary.
    Projected {
        symbol: SymbolHandle,
        segments: Vec<facts::PlaceSegment>,
    },
    /// The value currently stored under one projected place of mutable
    /// storage (`mut_pair.first`, `self.pivot[2]`). Like `Storage` it is a
    /// query coordinate, never positive evidence, and needs the same
    /// version-pin evidence before stated premises can claim it.
    StorageProjected {
        symbol: SymbolHandle,
        segments: Vec<facts::PlaceSegment>,
    },
    /// `first + second + offset` over two distinct immutable symbols in
    /// canonical arena order; `offset` may be zero because a two-symbol sum
    /// has no simpler spelling.
    SymbolSum {
        first: SymbolHandle,
        second: SymbolHandle,
        offset: i64,
    },
    /// The value produced by one exact call occurrence, named by its
    /// expression handle plus the resolved segment path projected through
    /// it (`choose(seed).first` carries `[Field{first}]`; a bare result
    /// carries the empty path). A call result carries no place identity,
    /// so a nested call's `ensures` binds the occurrence itself — the
    /// intra-statement fact context for guarantees produced and consumed
    /// within one statement. Equal only to the same occurrence at the same
    /// projected path; a call result never shifts or carries a storage
    /// coordinate.
    CallResult {
        expression: ExpressionHandle,
        segments: Vec<facts::PlaceSegment>,
    },
}

/// Integer ordering relations; disequality separates points, not windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerOrderingRelation {
    LessOrEqual,
    StrictlyBefore,
    Equal,
    NotEqual,
}

/// Normalized immutable relation and its reconstructed query transport.
/// Transport is local checking state, never serialized proof authority.
#[derive(Debug, Clone)]
pub struct StatedOrderingPremise {
    pub relation: IntegerOrderingRelation,
    pub left: IntegerBound,
    pub right: IntegerBound,
    pub parameter_arguments: Option<Vec<(symbols::SymbolHandle, symbols::SymbolHandle)>>,
}

impl StatedOrderingPremise {
    /// Candidate endpoints grant nothing: the query owner must independently
    /// prove their equality in its current transported namespace before use.
    pub fn equality_candidates(&self) -> Option<[IntegerBound; 2]> {
        matches!(self.relation, IntegerOrderingRelation::Equal)
            .then(|| [self.left.clone(), self.right.clone()])
    }

    /// The caller has established equality of the detached values at their
    /// actual evaluation epochs, not equality of current storage coordinates.
    pub fn captured_equality(left: IntegerBound, right: IntegerBound) -> Self {
        Self {
            relation: IntegerOrderingRelation::Equal,
            left,
            right,
            parameter_arguments: None,
        }
    }
}

pub fn prove_integer_ordering(
    premises: &[StatedOrderingPremise],
    left: IntegerBound,
    relation: IntegerOrderingRelation,
    right: IntegerBound,
) -> bool {
    // Reserve the whole former single/two-premise search unchanged, then
    // bound only the additional equality exploration. Exhaustion proves
    // nothing; it cannot turn a failed compatibility query into authority.
    const EQUALITY_CONSULTATIONS: usize = 4096;
    let mut remaining = premises
        .len()
        .saturating_mul(premises.len())
        .saturating_add(EQUALITY_CONSULTATIONS);
    if prove_from_premises(
        premises,
        left.clone(),
        relation,
        right.clone(),
        &mut remaining,
    ) {
        return true;
    }
    let mut candidates = Vec::new();
    for candidate in premises
        .iter()
        .filter_map(StatedOrderingPremise::equality_candidates)
        .flatten()
    {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    let mut pending = vec![(left, right)];
    let mut visited = Vec::new();
    while let Some((left, right)) = pending.pop() {
        if visited.contains(&(left.clone(), right.clone())) {
            continue;
        }
        visited.push((left.clone(), right.clone()));
        for candidate in &candidates {
            for (subject, next) in [
                (&left, (candidate.clone(), right.clone())),
                (&right, (left.clone(), candidate.clone())),
            ] {
                if subject == candidate || visited.contains(&next) || pending.contains(&next) {
                    continue;
                }
                if !prove_from_premises(
                    premises,
                    subject.clone(),
                    IntegerOrderingRelation::Equal,
                    candidate.clone(),
                    &mut remaining,
                ) {
                    if remaining == 0 {
                        return false;
                    }
                    continue;
                }
                if prove_from_premises(
                    premises,
                    next.0.clone(),
                    relation,
                    next.1.clone(),
                    &mut remaining,
                ) {
                    return true;
                }
                if remaining == 0 {
                    return false;
                }
                pending.push(next);
            }
        }
    }
    false
}

fn prove_from_premises(
    premises: &[StatedOrderingPremise],
    left: IntegerBound,
    relation: IntegerOrderingRelation,
    right: IntegerBound,
    remaining: &mut usize,
) -> bool {
    for premise in premises {
        if *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        if premise_proves(premise, left.clone(), relation, right.clone()) {
            return true;
        }
    }
    // No single premise names both bounds; a two-premise chain through
    // one shared middle bound may still derive the relation.
    for first_index in 0..premises.len() {
        for second_index in 0..premises.len() {
            if first_index == second_index {
                continue;
            }
            if *remaining == 0 {
                return false;
            }
            *remaining -= 1;
            if premise_chain_proves(
                &premises[first_index],
                &premises[second_index],
                left.clone(),
                relation,
                right.clone(),
            ) {
                return true;
            }
        }
    }
    false
}

/// Whether `premise` proves `left <query> right`.
///
/// The query shifts against the premise's endpoints only within one symbol's
/// constant-offset line or the integer line; unrelated bound pairs stay
/// unproven. Equality and disequality are symmetric, so their endpoints are
/// consulted in both orientations.
pub fn premise_proves(
    premise: &StatedOrderingPremise,
    left: IntegerBound,
    query: IntegerOrderingRelation,
    right: IntegerBound,
) -> bool {
    let (Some(left), Some(right)) = (
        transport_query_bound(premise, left),
        transport_query_bound(premise, right),
    ) else {
        return false;
    };
    premise_orientation_proves(
        premise.left.clone(),
        premise.relation,
        premise.right.clone(),
        left.clone(),
        query,
        right.clone(),
    ) || (matches!(
        premise.relation,
        IntegerOrderingRelation::Equal | IntegerOrderingRelation::NotEqual
    ) && premise_orientation_proves(
        premise.right.clone(),
        premise.relation,
        premise.left.clone(),
        left,
        query,
        right,
    ))
}

fn transport_query_bound(
    premise: &StatedOrderingPremise,
    bound: IntegerBound,
) -> Option<IntegerBound> {
    let Some(bindings) = &premise.parameter_arguments else {
        return Some(bound);
    };
    let argument = |symbol| {
        bindings
            .iter()
            .find_map(|(parameter, argument)| (*parameter == symbol).then_some(*argument))
    };
    Some(match bound {
        IntegerBound::Integer(_) => bound,
        // An entry read cannot cross a different parameter namespace without
        // the owning typed read/telescope substitution.
        IntegerBound::EntryRead(_) => return None,
        IntegerBound::Symbol { symbol, offset } => IntegerBound::Symbol {
            symbol: argument(symbol)?,
            offset,
        },
        IntegerBound::Storage { symbol } => IntegerBound::Storage {
            symbol: argument(symbol)?,
        },
        IntegerBound::Projected { symbol, segments } => IntegerBound::Projected {
            symbol: argument(symbol)?,
            segments,
        },
        IntegerBound::StorageProjected { symbol, segments } => IntegerBound::StorageProjected {
            symbol: argument(symbol)?,
            segments,
        },
        // A call-occurrence bound carries no parameter symbol to
        // transport; the occurrence identity crosses scopes unchanged.
        IntegerBound::CallResult { .. } => bound,
        IntegerBound::SymbolSum {
            first,
            second,
            offset,
        } => {
            let first = argument(first)?;
            let second = argument(second)?;
            // Two aliases of one argument mean coefficient two, not one.
            // That coefficient is outside the current normalized vocabulary.
            if first == second {
                return None;
            }
            let (first, second) = if (first.arena_index(), first.generation())
                < (second.arena_index(), second.generation())
            {
                (first, second)
            } else {
                (second, first)
            };
            IntegerBound::SymbolSum {
                first,
                second,
                offset,
            }
        }
    })
}

/// Whether `premise_left <premise> premise_right` proves
/// `left <query> right`, with `left = premise_left + d1` and
/// `right = premise_right + d2` under the constant-offset algebra.
fn premise_orientation_proves(
    premise_left: IntegerBound,
    premise: IntegerOrderingRelation,
    premise_right: IntegerBound,
    left: IntegerBound,
    query: IntegerOrderingRelation,
    right: IntegerBound,
) -> bool {
    let (Some(left_shift), Some(right_shift)) = (
        bound_shift(left, premise_left),
        bound_shift(right, premise_right),
    ) else {
        return false;
    };
    use IntegerOrderingRelation as Relation;
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

/// Whether `first` and `second` transitively prove `left <query> right`
/// through one shared middle bound: `first` supplies `a r1 m1`, `second`
/// supplies `m2 r2 b`, and the pair composes when `m1 <= m2` on a common term
/// line. A strictly separating middle (`m1 < m2`) makes the composed relation
/// strict; at an equal middle the relation is strict when either leg is
/// strict and loose when both are loose, with `==` absorbing a leg outright.
/// `!=` transports only across an equality leg with an exact shared middle;
/// other relations or a shifted middle could reorder or collapse its endpoints. Each query endpoint transports through its own premise's
/// parameter bindings, so a guard-scope premise may chain with a state-scope
/// one. The composed outer endpoints still participate in the ordinary
/// constant-offset algebra, so `i < n && n <= j` proves `i < j + 1`.
pub fn premise_chain_proves(
    first: &StatedOrderingPremise,
    second: &StatedOrderingPremise,
    left: IntegerBound,
    query: IntegerOrderingRelation,
    right: IntegerBound,
) -> bool {
    first_legs(first).into_iter().flatten().any(|first_leg| {
        second_legs(second).into_iter().flatten().any(|second_leg| {
            chained_legs_prove(
                first_leg.clone(),
                second_leg,
                first,
                second,
                left.clone(),
                query,
                right.clone(),
            )
        })
    })
}

/// One usable orientation of a premise in a chain. The first leg is
/// `outer <relation> middle`; the second is `middle <relation> outer`.
#[derive(Clone)]
struct ChainLeg {
    outer: IntegerBound,
    relation: IntegerOrderingRelation,
    middle: IntegerBound,
}

/// Orientations of `premise` usable as the left leg of a chain: the shared
/// middle is its right endpoint. Equality and disequality are symmetric.
fn legs(
    premise: &StatedOrderingPremise,
    outer: &IntegerBound,
    middle: &IntegerBound,
) -> [Option<ChainLeg>; 2] {
    let stored = Some(ChainLeg {
        outer: outer.clone(),
        relation: premise.relation,
        middle: middle.clone(),
    });
    let flipped = matches!(
        premise.relation,
        IntegerOrderingRelation::Equal | IntegerOrderingRelation::NotEqual
    )
    .then_some(ChainLeg {
        outer: middle.clone(),
        relation: premise.relation,
        middle: outer.clone(),
    });
    [stored, flipped]
}

fn first_legs(premise: &StatedOrderingPremise) -> [Option<ChainLeg>; 2] {
    legs(premise, &premise.left, &premise.right)
}

fn second_legs(premise: &StatedOrderingPremise) -> [Option<ChainLeg>; 2] {
    legs(premise, &premise.right, &premise.left)
}

fn chained_legs_prove(
    first_leg: ChainLeg,
    second_leg: ChainLeg,
    first: &StatedOrderingPremise,
    second: &StatedOrderingPremise,
    left: IntegerBound,
    query: IntegerOrderingRelation,
    right: IntegerBound,
) -> bool {
    use IntegerOrderingRelation as Relation;
    let transports_disequality = match (first_leg.relation, second_leg.relation) {
        (Relation::NotEqual, Relation::Equal) | (Relation::Equal, Relation::NotEqual) => true,
        (Relation::NotEqual, _) | (_, Relation::NotEqual) => return false,
        _ => false,
    };
    // The legs meet only when the first middle provably precedes or equals
    // the second on one shared term line.
    let Some(middle_gap) = bound_shift(second_leg.middle, first_leg.middle) else {
        return false;
    };
    if middle_gap < 0 || (transports_disequality && middle_gap != 0) {
        return false;
    }
    // Disequality has no ordering direction. An available exact equation may
    // replace one endpoint, but a loose/strict leg or a shifted shared middle
    // could collapse the two points. Keep this substitution distinct from
    // ordinary ordered transitivity below.
    let relation = if transports_disequality {
        Relation::NotEqual
    } else if middle_gap > 0 {
        Relation::StrictlyBefore
    } else {
        match (first_leg.relation, second_leg.relation) {
            (Relation::Equal, relation) | (relation, Relation::Equal) => relation,
            (Relation::StrictlyBefore, _) | (_, Relation::StrictlyBefore) => {
                Relation::StrictlyBefore
            }
            (Relation::LessOrEqual, Relation::LessOrEqual) => Relation::LessOrEqual,
            _ => return false,
        }
    };
    let (Some(left), Some(right)) = (
        transport_query_bound(first, left),
        transport_query_bound(second, right),
    ) else {
        return false;
    };
    premise_orientation_proves(
        first_leg.outer,
        relation,
        second_leg.outer,
        left,
        query,
        right,
    )
}

/// `value - base` when both bounds sit on one symbol's offset line, on one
/// canonical two-symbol sum's offset line, or both are integers. Distinct
/// term sets stay unrelated, never negative evidence.
fn bound_shift(value: IntegerBound, base: IntegerBound) -> Option<i64> {
    match (value, base) {
        (IntegerBound::EntryRead(value), IntegerBound::EntryRead(base)) if value == base => Some(0),
        (IntegerBound::Integer(value), IntegerBound::Integer(base)) => value.checked_sub(base),
        (
            IntegerBound::Symbol {
                symbol: value_symbol,
                offset: value_offset,
            },
            IntegerBound::Symbol {
                symbol: base_symbol,
                offset: base_offset,
            },
        ) if value_symbol == base_symbol => value_offset.checked_sub(base_offset),
        // Two storage rows in one premise evaluation name the same pinned
        // occurrence: the minting binding verified the storage's version at
        // this scope, so the rows share one zero-offset line.
        (
            IntegerBound::Storage {
                symbol: value_symbol,
            },
            IntegerBound::Storage {
                symbol: base_symbol,
            },
        ) if value_symbol == base_symbol => Some(0),
        // One projected member place of the same symbol sits on the
        // zero-offset line of that projection: `pair.first >= 2` shifts the
        // query's own `pair.first` by zero.
        (
            IntegerBound::Projected {
                symbol: value_symbol,
                segments: value_segments,
            },
            IntegerBound::Projected {
                symbol: base_symbol,
                segments: base_segments,
            },
        ) if value_symbol == base_symbol && value_segments == base_segments => Some(0),
        (
            IntegerBound::StorageProjected {
                symbol: value_symbol,
                segments: value_segments,
            },
            IntegerBound::StorageProjected {
                symbol: base_symbol,
                segments: base_segments,
            },
        ) if value_symbol == base_symbol && value_segments == base_segments => Some(0),
        (
            IntegerBound::SymbolSum {
                first: value_first,
                second: value_second,
                offset: value_offset,
            },
            IntegerBound::SymbolSum {
                first: base_first,
                second: base_second,
                offset: base_offset,
            },
        ) if value_first == base_first && value_second == base_second => {
            value_offset.checked_sub(base_offset)
        }
        // The same call occurrence at the same projected path names one
        // produced value: a premise minted at the call sits on the query's
        // zero-offset line.
        (
            IntegerBound::CallResult {
                expression: value_expression,
                segments: value_segments,
            },
            IntegerBound::CallResult {
                expression: base_expression,
                segments: base_segments,
            },
        ) if value_expression == base_expression && value_segments == base_segments => Some(0),
        _ => None,
    }
}

pub fn integer_bound_terms_equal(left: IntegerBound, right: IntegerBound) -> bool {
    match (left, right) {
        (IntegerBound::EntryRead(left), IntegerBound::EntryRead(right)) => left == right,
        (IntegerBound::Integer(_), IntegerBound::Integer(_)) => true,
        (
            IntegerBound::Symbol {
                symbol: left_symbol,
                ..
            },
            IntegerBound::Symbol {
                symbol: right_symbol,
                ..
            },
        ) => left_symbol == right_symbol,
        (
            IntegerBound::SymbolSum {
                first: left_first,
                second: left_second,
                ..
            },
            IntegerBound::SymbolSum {
                first: right_first,
                second: right_second,
                ..
            },
        ) => left_first == right_first && left_second == right_second,
        (
            IntegerBound::Projected {
                symbol: left_symbol,
                segments: left_segments,
            },
            IntegerBound::Projected {
                symbol: right_symbol,
                segments: right_segments,
            },
        ) => left_symbol == right_symbol && left_segments == right_segments,
        (
            IntegerBound::CallResult {
                expression: left_expression,
                segments: left_segments,
            },
            IntegerBound::CallResult {
                expression: right_expression,
                segments: right_segments,
            },
        ) => left_expression == right_expression && left_segments == right_segments,
        _ => false,
    }
}

pub fn integer_bound_offset(bound: IntegerBound) -> Option<i64> {
    match bound {
        IntegerBound::Integer(value) => Some(value),
        IntegerBound::EntryRead(_) => Some(0),
        IntegerBound::Symbol { offset, .. } => Some(offset),
        IntegerBound::Storage { .. } => Some(0),
        IntegerBound::Projected { .. } | IntegerBound::StorageProjected { .. } => Some(0),
        IntegerBound::CallResult { .. } => Some(0),
        IntegerBound::SymbolSum { offset, .. } => Some(offset),
    }
}

/// Publish only consequences of the caller's live strict/non-strict orderings
/// and already established collection upper bounds. Subjects and collections
/// are opaque equality identities supplied by that caller; this finite closure
/// cannot create new subjects or survive a caller's write invalidation.
pub fn propagate_integer_upper_bounds<Subject: Clone + Eq, Collection: Clone + Eq>(
    orderings: &[(Subject, Subject, bool)],
    indexes: &mut Vec<(Collection, Subject)>,
    range_bounds: &mut Vec<(Collection, Subject)>,
) {
    loop {
        let mut grown = false;
        for (lower, upper, strict) in orderings {
            for (collection, _) in indexes
                .clone()
                .into_iter()
                .filter(|(_, index)| index == upper)
            {
                let consequence = (collection, lower.clone());
                if !indexes.contains(&consequence) {
                    indexes.push(consequence);
                    grown = true;
                }
            }
            for (collection, _) in range_bounds
                .clone()
                .into_iter()
                .filter(|(_, bound)| bound == upper)
            {
                let consequence = (collection, lower.clone());
                if *strict {
                    if !indexes.contains(&consequence) {
                        indexes.push(consequence);
                        grown = true;
                    }
                } else if !range_bounds.contains(&consequence) {
                    range_bounds.push(consequence);
                    grown = true;
                }
            }
        }
        if !grown {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_premise_oracle_requires_each_exact_relation() {
        let mut symbols = symbols::SymbolTableBuilder::new();
        let mut subject = |name| IntegerBound::Symbol {
            symbol: symbols.insert_root(
                symbols::SymbolKind::Root,
                symbols::SymbolNameRef::Static(name),
            ),
            offset: 0,
        };
        let left = subject("left");
        let middle = subject("middle");
        let right = subject("right");
        let premise = |left, relation, right| StatedOrderingPremise {
            left,
            relation,
            right,
            parameter_arguments: None,
        };
        let first = premise(
            left.clone(),
            IntegerOrderingRelation::StrictlyBefore,
            middle.clone(),
        );
        let second = premise(
            middle.clone(),
            IntegerOrderingRelation::LessOrEqual,
            right.clone(),
        );
        assert!(prove_integer_ordering(
            &[first.clone(), second],
            left.clone(),
            IntegerOrderingRelation::StrictlyBefore,
            right.clone()
        ));
        assert!(!prove_integer_ordering(
            &[first],
            left.clone(),
            IntegerOrderingRelation::StrictlyBefore,
            right.clone()
        ));

        let different = premise(
            left.clone(),
            IntegerOrderingRelation::NotEqual,
            middle.clone(),
        );
        let equal = premise(
            middle.clone(),
            IntegerOrderingRelation::Equal,
            right.clone(),
        );
        assert!(prove_integer_ordering(
            &[different.clone(), equal.clone()],
            left.clone(),
            IntegerOrderingRelation::NotEqual,
            right.clone()
        ));
        assert!(!prove_integer_ordering(
            &[different.clone(), equal],
            left.clone(),
            IntegerOrderingRelation::StrictlyBefore,
            right.clone()
        ));
        let loose = premise(middle, IntegerOrderingRelation::LessOrEqual, right.clone());
        assert!(!prove_integer_ordering(
            &[different, loose],
            left,
            IntegerOrderingRelation::NotEqual,
            right
        ));
    }

    #[test]
    fn integer_upper_bound_closure_keeps_strictness_and_missing_links() {
        let mut indexes = Vec::new();
        let mut ranges = vec![("items", "end")];
        propagate_integer_upper_bounds(
            &[("first", "middle", false), ("middle", "end", true)],
            &mut indexes,
            &mut ranges,
        );
        assert!(indexes.contains(&("items", "first")));
        assert!(indexes.contains(&("items", "middle")));

        let mut missing_indexes = Vec::new();
        let mut missing_ranges = vec![("items", "end")];
        propagate_integer_upper_bounds(
            &[("first", "middle", false)],
            &mut missing_indexes,
            &mut missing_ranges,
        );
        assert!(missing_indexes.is_empty());
        assert_eq!(missing_ranges, vec![("items", "end")]);

        let mut loose_indexes = Vec::new();
        let mut loose_ranges = vec![("items", "end")];
        propagate_integer_upper_bounds(
            &[("first", "end", false)],
            &mut loose_indexes,
            &mut loose_ranges,
        );
        assert!(loose_indexes.is_empty());
        assert!(loose_ranges.contains(&("items", "first")));
    }
}
