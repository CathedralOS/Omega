//! Borrow compatibility judgments: whether two captured places are disjoint,
//! whether one contains the other, and whether accesses through them can
//! coexist.
//!
//! Every judgment ends in one of two functions:
//! `captured_place_compatibility_with_selector_snapshot` forms a new
//! judgment, and `captured_place_compatibility_from_selector_snapshot`
//! replays a retained one. Places with different valid roots are disjoint.
//! Places with the same root are compared segment by segment (`segments`),
//! and index segments by their evaluated bounds (`indexes`), using stated
//! ordering premises (`premises`) where the bounds alone do not decide. Two
//! reads never interfere; otherwise the places must be disjoint and must not
//! name two sibling fields that one data `where` fact relates.
//!
//! A new judgment records the evaluated index bounds (the selector snapshot)
//! and the premises it consumed; the borrow checks keep both in their
//! certificates. A replay re-evaluates the pair against those records and
//! returns `CompatibilityReplayDrift` when a snapshot row or premise no
//! longer matches.
//!
//! The public wrappers adapt their inputs: the `captured_place_*` forms take
//! two captured places, the `borrow_loan_*` forms two loan rows, and the
//! `canonical_place_loan_*` forms a mutated place re-rooted against a loan
//! (`canonical_place_for_loan`). `borrows`, `statements` and `calls` call
//! them. `stated_ordering_premises` collects the premises a state starts
//! with from its `requires` and incoming guards, and `append_call_premises`
//! adds the call `ensures` available at one statement's entry.

mod indexes;
mod premises;
mod segments;

#[cfg(test)]
mod unresolved_identities;

pub(super) use self::indexes::CompatibilityReplayDrift;
use self::indexes::SelectorSessionClosure;
pub(super) use self::premises::append_call_premises;
pub(super) use self::premises::{StatedOrderingPremise, stated_ordering_premises};
use self::segments::{
    place_segments_compatibility_from_snapshot, place_segments_compatibility_with_snapshot,
};

pub(super) struct CapturedPlaceCompatibilityEvidence {
    pub compatibility: crate::checked_trees::CapturedPlaceCompatibility,
    pub selector_snapshot: Vec<crate::checked_trees::BorrowCompatibilitySelectorSnapshot>,
    /// Exact stated requires tokens the judgment consumed, in consult order.
    /// Empty for a purely structural derivation.
    pub premises: Vec<crate::checked_trees::BorrowCompatibilityPremise>,
}

pub(super) fn captured_place_compatibility<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    left: &crate::checked_trees::CapturedPlace,
    left_access: &crate::checked_trees::BorrowAccessKind,
    right: &crate::checked_trees::CapturedPlace,
    right_access: &crate::checked_trees::BorrowAccessKind,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> crate::checked_trees::CapturedPlaceCompatibility {
    captured_place_compatibility_with_selector_snapshot(
        program,
        left,
        left_access,
        right,
        right_access,
        premises,
        bound_lookup,
    )
    .compatibility
}

pub(super) fn captured_place_compatibility_with_selector_snapshot<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    left: &crate::checked_trees::CapturedPlace,
    left_access: &crate::checked_trees::BorrowAccessKind,
    right: &crate::checked_trees::CapturedPlace,
    right_access: &crate::checked_trees::BorrowAccessKind,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> CapturedPlaceCompatibilityEvidence {
    let roots_valid = left.root_symbol.is_valid() && right.root_symbol.is_valid();
    let same_root = roots_valid && left.root_symbol == right.root_symbol;
    let (segments_may_overlap, containment, closure) = if same_root {
        place_segments_compatibility_with_snapshot(
            program,
            &left.segments,
            &right.segments,
            premises,
            bound_lookup,
        )
    } else {
        (
            false,
            crate::checked_trees::CapturedPlaceContainment::None,
            SelectorSessionClosure {
                snapshot: Vec::new(),
                premises: Vec::new(),
            },
        )
    };
    let disjoint = roots_valid && (!same_root || !segments_may_overlap);
    let shares_dependent_fact = same_root
        && place_segments_share_dependent_fact(
            program,
            left.root_symbol,
            &left.segments,
            &right.segments,
        );
    let both_shared = matches!(left_access, crate::checked_trees::BorrowAccessKind::Read)
        && matches!(right_access, crate::checked_trees::BorrowAccessKind::Read);

    CapturedPlaceCompatibilityEvidence {
        compatibility: crate::checked_trees::CapturedPlaceCompatibility {
            left: left.clone(),
            right: right.clone(),
            disjoint,
            containment,
            non_interfering: both_shared || (disjoint && !shares_dependent_fact),
        },
        selector_snapshot: closure.snapshot,
        premises: closure.premises,
    }
}

fn captured_place_compatibility_from_selector_snapshot<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    left: &crate::checked_trees::CapturedPlace,
    left_access: &crate::checked_trees::BorrowAccessKind,
    right: &crate::checked_trees::CapturedPlace,
    right_access: &crate::checked_trees::BorrowAccessKind,
    selector_snapshot: &[crate::checked_trees::BorrowCompatibilitySelectorSnapshot],
    premises: &[StatedOrderingPremise],
    recorded_premises: &[crate::checked_trees::BorrowCompatibilityPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> Result<crate::checked_trees::CapturedPlaceCompatibility, CompatibilityReplayDrift> {
    let roots_valid = left.root_symbol.is_valid() && right.root_symbol.is_valid();
    let same_root = roots_valid && left.root_symbol == right.root_symbol;
    let (segments_may_overlap, containment) = if same_root {
        place_segments_compatibility_from_snapshot(
            program,
            &left.segments,
            &right.segments,
            selector_snapshot,
            premises,
            recorded_premises,
            bound_lookup,
        )?
    } else {
        if !selector_snapshot.is_empty() {
            return Err(CompatibilityReplayDrift::SelectorSnapshot);
        }
        if !recorded_premises.is_empty() {
            return Err(CompatibilityReplayDrift::Premise);
        }
        (false, crate::checked_trees::CapturedPlaceContainment::None)
    };
    let disjoint = roots_valid && (!same_root || !segments_may_overlap);
    let shares_dependent_fact = same_root
        && place_segments_share_dependent_fact(
            program,
            left.root_symbol,
            &left.segments,
            &right.segments,
        );
    let both_shared = matches!(left_access, crate::checked_trees::BorrowAccessKind::Read)
        && matches!(right_access, crate::checked_trees::BorrowAccessKind::Read);
    Ok(crate::checked_trees::CapturedPlaceCompatibility {
        left: left.clone(),
        right: right.clone(),
        disjoint,
        containment,
        non_interfering: both_shared || (disjoint && !shares_dependent_fact),
    })
}

/// Re-roots a canonical write/access place against the loan's root exactly as
/// `canonical_place_loan_compatibility` judges the pair. Returns `None` for
/// roots that cannot name a symbol place at all.
pub(super) fn canonical_place_for_loan(
    place: &crate::flow::CanonicalPlace,
    loan: &crate::checked_trees::BorrowLoanFact,
) -> Option<crate::checked_trees::CapturedPlace> {
    match place.root {
        crate::fact_plan::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                Some(crate::checked_trees::CapturedPlace {
                    root_symbol: symbol,
                    segments: place.segments.clone(),
                })
            } else {
                match place.segments.split_first() {
                    Some((
                        crate::fact_plan::PlaceSegment::Field {
                            symbol: field_symbol,
                        },
                        remaining,
                    )) if *field_symbol == loan.root_symbol => {
                        Some(crate::checked_trees::CapturedPlace {
                            root_symbol: loan.root_symbol,
                            segments: remaining.to_vec(),
                        })
                    }
                    Some((
                        crate::fact_plan::PlaceSegment::Case { .. },
                        [
                            crate::fact_plan::PlaceSegment::Field {
                                symbol: field_symbol,
                            },
                            remaining @ ..,
                        ],
                    )) if *field_symbol == loan.root_symbol => {
                        Some(crate::checked_trees::CapturedPlace {
                            root_symbol: loan.root_symbol,
                            segments: remaining.to_vec(),
                        })
                    }
                    _ => Some(crate::checked_trees::CapturedPlace {
                        root_symbol: symbol,
                        segments: place.segments.clone(),
                    }),
                }
            }
        }
        crate::fact_plan::PlaceRoot::Unknown
        | crate::fact_plan::PlaceRoot::Expression(_)
        | crate::fact_plan::PlaceRoot::TypeReference(_) => None,
    }
}

/// Capture-session compatibility for one statement-mutated place against an
/// active loan: the same left join and spatial judgment admission uses, with
/// the selector snapshot and consumed premise tokens retained for replay.
pub(super) fn canonical_place_loan_compatibility_with_selector_snapshot<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &crate::checked_trees::BorrowLoanFact,
    borrow: &crate::checked_trees::BorrowFacts,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> CapturedPlaceCompatibilityEvidence {
    let right = captured_loan_place(borrow, loan);
    let Some(left) = canonical_place_for_loan(place, loan) else {
        return CapturedPlaceCompatibilityEvidence {
            compatibility: crate::checked_trees::CapturedPlaceCompatibility {
                right,
                ..Default::default()
            },
            selector_snapshot: Vec::new(),
            premises: Vec::new(),
        };
    };
    captured_place_compatibility_with_selector_snapshot(
        program,
        &left,
        &crate::checked_trees::BorrowAccessKind::Mutable,
        &right,
        &loan.kind,
        premises,
        bound_lookup,
    )
}

pub(super) fn canonical_place_loan_compatibility<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &crate::checked_trees::BorrowLoanFact,
    borrow: &crate::checked_trees::BorrowFacts,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> crate::checked_trees::CapturedPlaceCompatibility {
    canonical_place_loan_compatibility_with_selector_snapshot(
        program,
        place,
        loan,
        borrow,
        premises,
        bound_lookup,
    )
    .compatibility
}

/// Replays one judged captured-place/loan pair from its frozen selector
/// snapshot. The left side arrives already re-rooted exactly as the retained
/// certificate judged it; the right side is re-captured from the loan row.
pub(super) fn captured_place_loan_compatibility_from_selector_snapshot<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    left: &crate::checked_trees::CapturedPlace,
    left_access: &crate::checked_trees::BorrowAccessKind,
    right: &crate::checked_trees::BorrowLoanFact,
    right_access: &crate::checked_trees::BorrowAccessKind,
    borrow: &crate::checked_trees::BorrowFacts,
    selector_snapshot: &[crate::checked_trees::BorrowCompatibilitySelectorSnapshot],
    premises: &[StatedOrderingPremise],
    recorded_premises: &[crate::checked_trees::BorrowCompatibilityPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> Result<crate::checked_trees::CapturedPlaceCompatibility, CompatibilityReplayDrift> {
    captured_place_compatibility_from_selector_snapshot(
        program,
        left,
        left_access,
        &captured_loan_place(borrow, right),
        right_access,
        selector_snapshot,
        premises,
        recorded_premises,
        bound_lookup,
    )
}

fn captured_loan_place(
    borrow: &crate::checked_trees::BorrowFacts,
    loan: &crate::checked_trees::BorrowLoanFact,
) -> crate::checked_trees::CapturedPlace {
    crate::checked_trees::CapturedPlace {
        root_symbol: loan.root_symbol,
        segments: borrow.loan_segments(loan).to_vec(),
    }
}

/// A borrow of one field participating in a dependent-data fact pins the
/// sibling fields named by that SAME fact. A direct structural overlap is not
/// required: mutating the sibling would invalidate the relation that makes the
/// borrowed projection meaningful while it remains live.
fn place_segments_share_dependent_fact(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    root_symbol: symbols::SymbolHandle,
    left: &[crate::fact_plan::PlaceSegment],
    right: &[crate::fact_plan::PlaceSegment],
) -> bool {
    let divergence = left
        .iter()
        .zip(right)
        .position(|(left, right)| left != right);
    let Some(divergence) = divergence else {
        return false;
    };
    let (
        crate::fact_plan::PlaceSegment::Field { symbol: left_field },
        crate::fact_plan::PlaceSegment::Field {
            symbol: right_field,
        },
    ) = (left[divergence], right[divergence])
    else {
        return false;
    };

    let Some(mut parent_type) = crate::flow::symbol_type_symbol(program, root_symbol) else {
        return false;
    };
    for segment in &left[..divergence] {
        if let crate::fact_plan::PlaceSegment::Field { symbol } = segment {
            let Some(next_type) = crate::flow::symbol_type_symbol(program, *symbol) else {
                return false;
            };
            parent_type = next_type;
        }
    }
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == parent_type)
    else {
        return false;
    };
    let Some(left_name) = data_field_name(program, definition, left_field) else {
        return false;
    };
    let Some(right_name) = data_field_name(program, definition, right_field) else {
        return false;
    };

    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .any(|fact| match fact {
            symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Expression(
                expression,
            ) => {
                expression_mentions_field(program, *expression, left_name)
                    && expression_mentions_field(program, *expression, right_name)
            }
            symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Membership(_) => {
                false
            }
            symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Proposition(
                _,
            ) => false,
        })
}

fn data_field_name<'a>(
    program: &'a symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    definition: &'a symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition,
    symbol: symbols::SymbolHandle,
) -> Option<&'a str> {
    program.data_members(definition).iter().find_map(|member| {
        let symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Field(field) =
            member
        else {
            return None;
        };
        (field.symbol == symbol).then_some(field.name.as_str())
    })
}

fn expression_mentions_field(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    field: &str,
) -> bool {
    use symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode;
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == field),
        ExpressionNode::Binary(binary) => {
            expression_mentions_field(program, binary.left, field)
                || expression_mentions_field(program, binary.right, field)
        }
        ExpressionNode::Member(member) => {
            member.member.as_str() == field
                || expression_mentions_field(program, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => expression_mentions_field(program, inner.target, field),
        _ => false,
    }
}

pub(super) fn borrow_loan_compatibility_with_selector_snapshot<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &crate::checked_trees::CheckFacts,
    left: &crate::checked_trees::BorrowLoanFact,
    right: &crate::checked_trees::BorrowLoanFact,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> CapturedPlaceCompatibilityEvidence {
    captured_place_compatibility_with_selector_snapshot(
        program,
        &captured_loan_place(&facts.borrow, left),
        &left.kind,
        &captured_loan_place(&facts.borrow, right),
        &right.kind,
        premises,
        bound_lookup,
    )
}

pub(super) fn borrow_loan_compatibility_from_selector_snapshot<'p>(
    program: &'p symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &crate::checked_trees::CheckFacts,
    left: &crate::checked_trees::BorrowLoanFact,
    left_access: &crate::checked_trees::BorrowAccessKind,
    right: &crate::checked_trees::BorrowLoanFact,
    right_access: &crate::checked_trees::BorrowAccessKind,
    selector_snapshot: &[crate::checked_trees::BorrowCompatibilitySelectorSnapshot],
    premises: &[StatedOrderingPremise],
    recorded_premises: &[crate::checked_trees::BorrowCompatibilityPremise],
    bound_lookup: &mut Option<crate::validation::ImmutableBoundLookup<'p>>,
) -> Result<crate::checked_trees::CapturedPlaceCompatibility, CompatibilityReplayDrift> {
    captured_place_compatibility_from_selector_snapshot(
        program,
        &captured_loan_place(&facts.borrow, left),
        left_access,
        &captured_loan_place(&facts.borrow, right),
        right_access,
        selector_snapshot,
        premises,
        recorded_premises,
        bound_lookup,
    )
}

#[cfg(test)]
mod tests {
    use crate::checked_trees::{BorrowAccessKind, CapturedPlace, CapturedPlaceContainment};
    use crate::checks::borrows::overlap::captured_place_compatibility;

    fn symbol(index: u32) -> symbols::SymbolHandle {
        symbols::SymbolHandle::from_arena_index(index)
    }

    fn field(symbol: symbols::SymbolHandle) -> crate::fact_plan::PlaceSegment {
        crate::fact_plan::PlaceSegment::Field { symbol }
    }

    #[test]
    fn structural_verdicts_preserve_exact_identity_and_direction() {
        let program = symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default();
        let root = symbol(1);
        let left_field = symbol(2);
        let right_field = symbol(3);
        let whole = CapturedPlace {
            root_symbol: root,
            segments: Vec::new(),
        };
        let left = CapturedPlace {
            root_symbol: root,
            segments: vec![field(left_field)],
        };
        let right = CapturedPlace {
            root_symbol: root,
            segments: vec![field(right_field)],
        };

        let siblings = captured_place_compatibility(
            &program,
            &left,
            &BorrowAccessKind::Mutable,
            &right,
            &BorrowAccessKind::Mutable,
            &[],
            &mut None,
        );
        assert!(siblings.disjoint);
        assert!(siblings.non_interfering);
        assert_eq!(siblings.containment, CapturedPlaceContainment::None);
        assert_eq!(siblings.left, left);
        assert_eq!(siblings.right, right);

        let contained = captured_place_compatibility(
            &program,
            &whole,
            &BorrowAccessKind::Mutable,
            &left,
            &BorrowAccessKind::Read,
            &[],
            &mut None,
        );
        assert!(!contained.disjoint);
        assert!(!contained.non_interfering);
        assert_eq!(
            contained.containment,
            CapturedPlaceContainment::LeftContainsRight
        );

        let reversed = captured_place_compatibility(
            &program,
            &left,
            &BorrowAccessKind::Read,
            &whole,
            &BorrowAccessKind::Mutable,
            &[],
            &mut None,
        );
        assert_eq!(
            reversed.containment,
            CapturedPlaceContainment::RightContainsLeft
        );
    }

    #[test]
    fn shared_reads_are_noninterfering_without_manufacturing_disjointness() {
        let program = symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default();
        let place = CapturedPlace {
            root_symbol: symbol(1),
            segments: vec![field(symbol(2))],
        };
        let compatibility = captured_place_compatibility(
            &program,
            &place,
            &BorrowAccessKind::Read,
            &place,
            &BorrowAccessKind::Read,
            &[],
            &mut None,
        );

        assert!(!compatibility.disjoint);
        assert_eq!(compatibility.containment, CapturedPlaceContainment::Same);
        assert!(compatibility.non_interfering);

        let conflicting = captured_place_compatibility(
            &program,
            &place,
            &BorrowAccessKind::Read,
            &place,
            &BorrowAccessKind::WriteOnly,
            &[],
            &mut None,
        );
        assert!(!conflicting.non_interfering);
    }

    #[test]
    fn invalid_and_runtime_indexed_places_do_not_gain_spatial_verdicts() {
        let mut program = symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees::default();
        let expression = program.expression_table.insert(
            crate::checked_trees::expression::ExpressionNode::Boolean(true),
        );
        let indexed = CapturedPlace {
            root_symbol: symbol(1),
            segments: vec![crate::fact_plan::PlaceSegment::Index { expression }],
        };
        let indexed_compatibility = captured_place_compatibility(
            &program,
            &indexed,
            &BorrowAccessKind::Mutable,
            &indexed,
            &BorrowAccessKind::Mutable,
            &[],
            &mut None,
        );
        assert!(!indexed_compatibility.disjoint);
        assert_eq!(
            indexed_compatibility.containment,
            CapturedPlaceContainment::None
        );
        assert!(!indexed_compatibility.non_interfering);

        let invalid = CapturedPlace::default();
        let invalid_compatibility = captured_place_compatibility(
            &program,
            &invalid,
            &BorrowAccessKind::Mutable,
            &indexed,
            &BorrowAccessKind::Mutable,
            &[],
            &mut None,
        );
        assert!(!invalid_compatibility.disjoint);
        assert_eq!(
            invalid_compatibility.containment,
            CapturedPlaceContainment::None
        );
        assert!(!invalid_compatibility.non_interfering);
    }
}
