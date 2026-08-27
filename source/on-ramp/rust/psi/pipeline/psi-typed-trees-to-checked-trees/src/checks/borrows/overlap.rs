mod indexes;
mod segments;

use self::segments::{place_segments_containment, place_segments_may_overlap};

pub(super) fn captured_place_compatibility(
    program: &psi_typed_trees::TypedTrees,
    left: &psi_checked_trees::CapturedPlace,
    left_access: &psi_checked_trees::BorrowAccessKind,
    right: &psi_checked_trees::CapturedPlace,
    right_access: &psi_checked_trees::BorrowAccessKind,
) -> psi_checked_trees::CapturedPlaceCompatibility {
    let roots_valid = left.root_symbol.is_valid() && right.root_symbol.is_valid();
    let same_root = roots_valid && left.root_symbol == right.root_symbol;
    let disjoint = roots_valid
        && (!same_root || !place_segments_may_overlap(program, &left.segments, &right.segments));
    let containment = if same_root {
        place_segments_containment(&left.segments, &right.segments)
    } else {
        psi_checked_trees::CapturedPlaceContainment::None
    };
    let shares_dependent_fact = same_root
        && place_segments_share_dependent_fact(
            program,
            left.root_symbol,
            &left.segments,
            &right.segments,
        );
    let both_shared = matches!(left_access, psi_checked_trees::BorrowAccessKind::Read)
        && matches!(right_access, psi_checked_trees::BorrowAccessKind::Read);

    psi_checked_trees::CapturedPlaceCompatibility {
        left: left.clone(),
        right: right.clone(),
        disjoint,
        containment,
        non_interfering: both_shared || (disjoint && !shares_dependent_fact),
    }
}

pub(super) fn canonical_place_loan_compatibility(
    program: &psi_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &psi_checked_trees::BorrowLoanFact,
    borrow: &psi_checked_trees::BorrowFacts,
) -> psi_checked_trees::CapturedPlaceCompatibility {
    let right = captured_loan_place(borrow, loan);
    let left = match place.root {
        psi_facts::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                Some(psi_checked_trees::CapturedPlace {
                    root_symbol: symbol,
                    segments: place.segments.clone(),
                })
            } else {
                match place.segments.split_first() {
                    Some((
                        psi_facts::PlaceSegment::Field {
                            symbol: field_symbol,
                        },
                        remaining,
                    )) if *field_symbol == loan.root_symbol => {
                        Some(psi_checked_trees::CapturedPlace {
                            root_symbol: loan.root_symbol,
                            segments: remaining.to_vec(),
                        })
                    }
                    Some((
                        psi_facts::PlaceSegment::Case { .. },
                        [
                            psi_facts::PlaceSegment::Field {
                                symbol: field_symbol,
                            },
                            remaining @ ..,
                        ],
                    )) if *field_symbol == loan.root_symbol => {
                        Some(psi_checked_trees::CapturedPlace {
                            root_symbol: loan.root_symbol,
                            segments: remaining.to_vec(),
                        })
                    }
                    _ => Some(psi_checked_trees::CapturedPlace {
                        root_symbol: symbol,
                        segments: place.segments.clone(),
                    }),
                }
            }
        }
        psi_facts::PlaceRoot::Unknown
        | psi_facts::PlaceRoot::Expression(_)
        | psi_facts::PlaceRoot::TypeReference(_) => None,
    };
    let Some(left) = left else {
        return psi_checked_trees::CapturedPlaceCompatibility {
            right,
            ..Default::default()
        };
    };
    captured_place_compatibility(
        program,
        &left,
        &psi_checked_trees::BorrowAccessKind::Mutable,
        &right,
        &loan.kind,
    )
}

fn captured_access_place(
    borrow: &psi_checked_trees::BorrowFacts,
    access: &psi_checked_trees::BorrowArgumentAccessFact,
) -> psi_checked_trees::CapturedPlace {
    psi_checked_trees::CapturedPlace {
        root_symbol: access.root_symbol,
        segments: borrow.access_segments(access).to_vec(),
    }
}

fn captured_loan_place(
    borrow: &psi_checked_trees::BorrowFacts,
    loan: &psi_checked_trees::BorrowLoanFact,
) -> psi_checked_trees::CapturedPlace {
    psi_checked_trees::CapturedPlace {
        root_symbol: loan.root_symbol,
        segments: borrow.loan_segments(loan).to_vec(),
    }
}

/// A borrow of one field participating in a dependent-data fact pins the
/// sibling fields named by that SAME fact. A direct structural overlap is not
/// required: mutating the sibling would invalidate the relation that makes the
/// borrowed projection meaningful while it remains live.
fn place_segments_share_dependent_fact(
    program: &psi_typed_trees::TypedTrees,
    root_symbol: psi_symbols::SymbolHandle,
    left: &[psi_facts::PlaceSegment],
    right: &[psi_facts::PlaceSegment],
) -> bool {
    let divergence = left
        .iter()
        .zip(right)
        .position(|(left, right)| left != right);
    let Some(divergence) = divergence else {
        return false;
    };
    let (
        psi_facts::PlaceSegment::Field { symbol: left_field },
        psi_facts::PlaceSegment::Field {
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
        if let psi_facts::PlaceSegment::Field { symbol } = segment {
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
            psi_typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_field(program, *expression, left_name)
                    && expression_mentions_field(program, *expression, right_name)
            }
            psi_typed_trees::domain::ProofFact::Membership(_) => false,
            psi_typed_trees::domain::ProofFact::Proposition(_) => false,
        })
}

fn data_field_name<'a>(
    program: &'a psi_typed_trees::TypedTrees,
    definition: &'a psi_typed_trees::data::DataDefinition,
    symbol: psi_symbols::SymbolHandle,
) -> Option<&'a str> {
    program.data_members(definition).iter().find_map(|member| {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == symbol).then_some(field.name.as_str())
    })
}

fn expression_mentions_field(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    field: &str,
) -> bool {
    use psi_typed_trees::expression::ExpressionNode;
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

pub(super) fn borrow_access_compatibility(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    left: &psi_checked_trees::BorrowArgumentAccessFact,
    right: &psi_checked_trees::BorrowArgumentAccessFact,
) -> psi_checked_trees::CapturedPlaceCompatibility {
    captured_place_compatibility(
        program,
        &captured_access_place(&facts.borrow, left),
        &left.kind,
        &captured_access_place(&facts.borrow, right),
        &right.kind,
    )
}

pub(super) fn borrow_access_loan_compatibility(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    access: &psi_checked_trees::BorrowArgumentAccessFact,
    loan: &psi_checked_trees::BorrowLoanFact,
) -> psi_checked_trees::CapturedPlaceCompatibility {
    captured_place_compatibility(
        program,
        &captured_access_place(&facts.borrow, access),
        &access.kind,
        &captured_loan_place(&facts.borrow, loan),
        &loan.kind,
    )
}

pub(super) fn borrow_loan_compatibility(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    left: &psi_checked_trees::BorrowLoanFact,
    right: &psi_checked_trees::BorrowLoanFact,
) -> psi_checked_trees::CapturedPlaceCompatibility {
    captured_place_compatibility(
        program,
        &captured_loan_place(&facts.borrow, left),
        &left.kind,
        &captured_loan_place(&facts.borrow, right),
        &right.kind,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_trees::{BorrowAccessKind, CapturedPlace, CapturedPlaceContainment};

    fn symbol(index: u32) -> psi_symbols::SymbolHandle {
        psi_symbols::SymbolHandle::from_arena_index(index)
    }

    fn field(symbol: psi_symbols::SymbolHandle) -> psi_facts::PlaceSegment {
        psi_facts::PlaceSegment::Field { symbol }
    }

    #[test]
    fn structural_verdicts_preserve_exact_identity_and_direction() {
        let program = psi_typed_trees::TypedTrees::default();
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
        );
        assert_eq!(
            reversed.containment,
            CapturedPlaceContainment::RightContainsLeft
        );
    }

    #[test]
    fn shared_reads_are_noninterfering_without_manufacturing_disjointness() {
        let program = psi_typed_trees::TypedTrees::default();
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
        );
        assert!(!conflicting.non_interfering);
    }

    #[test]
    fn invalid_and_runtime_indexed_places_do_not_gain_spatial_verdicts() {
        let mut program = psi_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(psi_checked_trees::expression::ExpressionNode::Boolean(true));
        let indexed = CapturedPlace {
            root_symbol: symbol(1),
            segments: vec![psi_facts::PlaceSegment::Index { expression }],
        };
        let indexed_compatibility = captured_place_compatibility(
            &program,
            &indexed,
            &BorrowAccessKind::Mutable,
            &indexed,
            &BorrowAccessKind::Mutable,
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
        );
        assert!(!invalid_compatibility.disjoint);
        assert_eq!(
            invalid_compatibility.containment,
            CapturedPlaceContainment::None
        );
        assert!(!invalid_compatibility.non_interfering);
    }
}
