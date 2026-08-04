mod indexes;
mod segments;

use self::segments::place_segments_may_overlap;

pub(super) fn canonical_place_overlaps_loan(
    program: &psi_typed_trees::TypedTrees,
    place: &crate::flow::CanonicalPlace,
    loan: &psi_checked_trees::BorrowLoanFact,
    borrow: &psi_checked_trees::BorrowFacts,
) -> bool {
    match place.root {
        psi_facts::PlaceRoot::Symbol(symbol) => {
            if symbol == loan.root_symbol {
                return place_segments_may_overlap(
                    program,
                    &place.segments,
                    borrow.loan_segments(loan),
                ) || place_segments_share_dependent_fact(
                    program,
                    symbol,
                    &place.segments,
                    borrow.loan_segments(loan),
                );
            }

            match place.segments.split_first() {
                Some((
                    psi_facts::PlaceSegment::Field {
                        symbol: field_symbol,
                    },
                    remaining,
                )) if *field_symbol == loan.root_symbol => {
                    place_segments_may_overlap(program, remaining, borrow.loan_segments(loan))
                        || place_segments_share_dependent_fact(
                            program,
                            loan.root_symbol,
                            remaining,
                            borrow.loan_segments(loan),
                        )
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
                    place_segments_may_overlap(program, remaining, borrow.loan_segments(loan))
                        || place_segments_share_dependent_fact(
                            program,
                            loan.root_symbol,
                            remaining,
                            borrow.loan_segments(loan),
                        )
                }
                _ => false,
            }
        }
        psi_facts::PlaceRoot::Unknown
        | psi_facts::PlaceRoot::Expression(_)
        | psi_facts::PlaceRoot::TypeReference(_) => false,
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
        ExpressionNode::Mutable(inner) => expression_mentions_field(program, *inner, field),
        _ => false,
    }
}

pub(super) fn borrow_accesses_overlap(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    left: &psi_checked_trees::BorrowArgumentAccessFact,
    right: &psi_checked_trees::BorrowArgumentAccessFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(left),
            facts.borrow.access_segments(right),
        )
}

pub(super) fn borrow_access_overlaps_loan(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    access: &psi_checked_trees::BorrowArgumentAccessFact,
    loan: &psi_checked_trees::BorrowLoanFact,
) -> bool {
    access.root_symbol == loan.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.access_segments(access),
            facts.borrow.loan_segments(loan),
        )
}

pub(super) fn borrow_loan_overlaps_loan(
    program: &psi_typed_trees::TypedTrees,
    facts: &psi_checked_trees::CheckFacts,
    left: &psi_checked_trees::BorrowLoanFact,
    right: &psi_checked_trees::BorrowLoanFact,
) -> bool {
    left.root_symbol == right.root_symbol
        && place_segments_may_overlap(
            program,
            facts.borrow.loan_segments(left),
            facts.borrow.loan_segments(right),
        )
}
