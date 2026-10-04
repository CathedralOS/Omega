//! Spatial borrow judgments over captured places and their access kinds.
//! Bounds are evaluated once per comparison and reused by overlap and
//! containment. Stated premises supply only relations available at this use;
//! unknown bounds remain conservative. No successful judgment is serialized
//! for the checker to reconstruct later.

mod indexes;
mod premises;
mod segments;

pub(super) use self::premises::append_call_premises;
pub(super) use self::premises::{StatedOrderingPremise, stated_ordering_premises};
use self::segments::place_segments_compatibility;

pub(super) fn captured_place_compatibility<'p>(
    program: &'p typed_trees::TypedTrees,
    left: &checked_trees::CapturedPlace,
    left_access: &checked_trees::BorrowAccessKind,
    right: &checked_trees::CapturedPlace,
    right_access: &checked_trees::BorrowAccessKind,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> checked_trees::CapturedPlaceCompatibility {
    let receiver_namespace = receiver_namespace_places(program, left, right);
    let (left_place, right_place) = receiver_namespace
        .as_ref()
        .map_or((left, right), |(left, right)| (left, right));
    let roots_valid = left_place.root_symbol.is_valid() && right_place.root_symbol.is_valid();
    let same_root = roots_valid && left_place.root_symbol == right_place.root_symbol;
    let (segments_may_overlap, containment) = if same_root {
        place_segments_compatibility(
            program,
            &left_place.segments,
            &right_place.segments,
            premises,
            bound_lookup,
        )
    } else {
        (false, checked_trees::CapturedPlaceContainment::None)
    };
    let disjoint = roots_valid && (!same_root || !segments_may_overlap);
    let shares_dependent_fact = same_root
        && place_segments_share_dependent_fact(
            program,
            left_place.root_symbol,
            &left_place.segments,
            &right_place.segments,
        );
    let both_shared = matches!(left_access, checked_trees::BorrowAccessKind::Read)
        && matches!(right_access, checked_trees::BorrowAccessKind::Read);

    checked_trees::CapturedPlaceCompatibility {
        left: left.clone(),
        right: right.clone(),
        disjoint,
        containment,
        non_interfering: both_shared || (disjoint && !shares_dependent_fact),
    }
}

/// Borrow access places capture a direct `self.field` at the attached field's
/// own root, while a bare `self` operand captures the machine root: the whole
/// receiver. When one place is rooted at an attached machine and the other at
/// one of that machine's attached fields, re-root the field place under the
/// machine so both compare in the receiver's namespace. Any other pair keeps
/// its own roots.
fn receiver_namespace_places(
    program: &typed_trees::TypedTrees,
    left: &checked_trees::CapturedPlace,
    right: &checked_trees::CapturedPlace,
) -> Option<(checked_trees::CapturedPlace, checked_trees::CapturedPlace)> {
    if !left.root_symbol.is_valid()
        || !right.root_symbol.is_valid()
        || left.root_symbol == right.root_symbol
    {
        return None;
    }
    if let Some(right) = attached_field_place_under_receiver(program, left, right) {
        return Some((left.clone(), right));
    }
    attached_field_place_under_receiver(program, right, left).map(|left| (left, right.clone()))
}

fn attached_field_place_under_receiver(
    program: &typed_trees::TypedTrees,
    receiver: &checked_trees::CapturedPlace,
    field_place: &checked_trees::CapturedPlace,
) -> Option<checked_trees::CapturedPlace> {
    let machine = program.machine_by_symbol(receiver.root_symbol)?;
    let field = validation::exact_attached_field(
        program,
        machine,
        field_place.root_symbol,
        program.symbols.name(field_place.root_symbol),
    )?;
    let mut segments = Vec::with_capacity(field_place.segments.len() + 1);
    segments.push(facts::PlaceSegment::Field {
        symbol: field.symbol,
    });
    segments.extend_from_slice(&field_place.segments);
    Some(checked_trees::CapturedPlace {
        root_symbol: receiver.root_symbol,
        segments,
    })
}

/// Re-roots a canonical write/access place against the loan's root exactly as
/// `canonical_place_loan_compatibility` judges the pair. Returns `None` for
/// roots that cannot name a symbol place at all.
pub(super) fn canonical_place_for_loan(
    place: &crate::flow::CanonicalPlace,
    loan: &checked_trees::BorrowLoanFact,
) -> Option<checked_trees::CapturedPlace> {
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                Some(checked_trees::CapturedPlace {
                    root_symbol: symbol,
                    segments: place.segments.clone(),
                })
            } else {
                match place.segments.split_first() {
                    Some((
                        facts::PlaceSegment::Field {
                            symbol: field_symbol,
                        },
                        remaining,
                    )) if *field_symbol == loan.root_symbol => Some(checked_trees::CapturedPlace {
                        root_symbol: loan.root_symbol,
                        segments: remaining.to_vec(),
                    }),
                    Some((
                        facts::PlaceSegment::Case { .. },
                        [
                            facts::PlaceSegment::Field {
                                symbol: field_symbol,
                            },
                            remaining @ ..,
                        ],
                    )) if *field_symbol == loan.root_symbol => Some(checked_trees::CapturedPlace {
                        root_symbol: loan.root_symbol,
                        segments: remaining.to_vec(),
                    }),
                    _ => Some(checked_trees::CapturedPlace {
                        root_symbol: symbol,
                        segments: place.segments.clone(),
                    }),
                }
            }
        }
        facts::PlaceRoot::Unknown
        | facts::PlaceRoot::Expression(_)
        | facts::PlaceRoot::TypeReference(_) => None,
    }
}

pub(super) fn canonical_place_loan_compatibility<'p>(
    program: &'p typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &checked_trees::BorrowLoanFact,
    borrow: &checked_trees::BorrowFacts,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> checked_trees::CapturedPlaceCompatibility {
    let right = captured_loan_place(borrow, loan);
    let Some(left) = canonical_place_for_loan(place, loan) else {
        return checked_trees::CapturedPlaceCompatibility {
            right,
            ..Default::default()
        };
    };
    captured_place_compatibility(
        program,
        &left,
        &checked_trees::BorrowAccessKind::Mutable,
        &right,
        &loan.kind,
        premises,
        bound_lookup,
    )
}

fn captured_loan_place(
    borrow: &checked_trees::BorrowFacts,
    loan: &checked_trees::BorrowLoanFact,
) -> checked_trees::CapturedPlace {
    checked_trees::CapturedPlace {
        root_symbol: loan.root_symbol,
        segments: borrow.loan_segments(loan).to_vec(),
    }
}

/// A borrow of one field participating in a dependent-data fact pins the
/// sibling fields named by that SAME fact. A direct structural overlap is not
/// required: mutating the sibling would invalidate the relation that makes the
/// borrowed projection meaningful while it remains live.
fn place_segments_share_dependent_fact(
    program: &typed_trees::TypedTrees,
    root_symbol: symbols::SymbolHandle,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let divergence = left
        .iter()
        .zip(right)
        .position(|(left, right)| left != right);
    let Some(divergence) = divergence else {
        return false;
    };
    let (
        facts::PlaceSegment::Field { symbol: left_field },
        facts::PlaceSegment::Field {
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
        if let facts::PlaceSegment::Field { symbol } = segment {
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
            typed_trees::domain::ProofFact::Expression(expression) => {
                expression_mentions_field(program, *expression, left_name)
                    && expression_mentions_field(program, *expression, right_name)
            }
            typed_trees::domain::ProofFact::Membership(_) => false,
            typed_trees::domain::ProofFact::Proposition(_) => false,
        })
}

fn data_field_name<'a>(
    program: &'a typed_trees::TypedTrees,
    definition: &'a typed_trees::data::DataDefinition,
    symbol: symbols::SymbolHandle,
) -> Option<&'a str> {
    program.data_members(definition).iter().find_map(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == symbol).then_some(field.name.as_str())
    })
}

fn expression_mentions_field(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    field: &str,
) -> bool {
    use typed_trees::expression::ExpressionNode;
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

pub(super) fn borrow_loan_compatibility<'p>(
    program: &'p typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
    left: &checked_trees::BorrowLoanFact,
    right: &checked_trees::BorrowLoanFact,
    premises: &[StatedOrderingPremise],
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> checked_trees::CapturedPlaceCompatibility {
    captured_place_compatibility(
        program,
        &captured_loan_place(&facts.borrow, left),
        &left.kind,
        &captured_loan_place(&facts.borrow, right),
        &right.kind,
        premises,
        bound_lookup,
    )
}
