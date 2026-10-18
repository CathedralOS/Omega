//! Spatial borrow judgments over captured places and their access kinds.
//! Bounds are evaluated once per comparison and reused by overlap and
//! containment. Stated premises supply only relations available at this use;
//! unknown bounds remain conservative. No successful judgment is serialized
//! for the checker to reconstruct later.

mod captured_values;
mod indexes;
mod premises;
mod segments;

pub(super) use self::premises::append_statement_premises;
pub(super) use self::premises::{StatedOrderingPremise, stated_ordering_premises};
use self::segments::place_segments_compatibility;

/// Produced evaluation facts shared by source debt and executable planning.
/// Borrow only these fields so recording cleanup invocations stays independent.
#[derive(Clone, Copy)]
pub(crate) struct StorageSelectorFacts<'facts> {
    pub flow: &'facts checked_trees::FlowFacts,
    pub semantic: &'facts facts::FactPlan,
}

/// Compare captured storage paths without assuming that current mutable
/// selector storage still holds an earlier value. Immutable values and exact
/// constants retain identity across statements; mutable query coordinates
/// cannot establish containment without version-pinned premises. Window debt
/// uses this same spatial judgment as loans, but supplies no current premises
/// as evidence about the opening occurrence. Detached selector copies obtain
/// equality only from their retained reaching values and write-prefix epochs;
/// both source debt and executable planning consume that same relation.
pub(crate) fn storage_path_compatibility(
    program: &dyn ::validation::CheckingInput,
    captures: StorageSelectorFacts<'_>,
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> (bool, checked_trees::CapturedPlaceContainment) {
    let lookup = validation::ImmutableBoundLookup::new(program);
    let premises = captured_values::equalities(program, captures, left, right, &lookup);
    place_segments_compatibility(program, left, right, &premises, &mut Some(lookup))
}

/// Cleanup must separate its selected receiver from moved storage in every
/// incoming occurrence. Only immutable captured-value relations supplement the
/// context-free spatial judgment; current mutable selector facts cannot do so.
pub(crate) fn captured_storage_paths_are_disjoint(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &checked_trees::FlowStateFact,
    contexts: &[(usize, Vec<facts::FactContextHandle>)],
    left: &[facts::PlaceSegment],
    right: &[facts::PlaceSegment],
) -> bool {
    let mut lookup = Some(validation::ImmutableBoundLookup::new(program));
    let captures = StorageSelectorFacts {
        flow: &facts.flow,
        semantic: &facts.semantic,
    };
    let Some(frames) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    !contexts.is_empty()
        && contexts.iter().all(|(statement, contexts)| {
            // One occurrence carries a conjunction of retained constraint
            // contexts (entry, stores, calls), not alternative executions.
            // Distinct incoming occurrences remain separate in the outer all.
            let bound_lookup =
                lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
            let mut premises =
                captured_values::equalities(program, captures, left, right, bound_lookup);
            premises.extend(self::premises::captured_call_premises(
                program,
                facts,
                state,
                *statement,
                contexts,
                &frames,
                bound_lookup,
            ));
            !place_segments_compatibility(program, left, right, &premises, &mut lookup).0
        })
}

pub(super) fn captured_place_compatibility<'p>(
    program: &'p dyn ::validation::CheckingInput,
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
    let roots_valid = left_place.root.is_valid() && right_place.root.is_valid();
    let same_root = roots_valid
        && left_place.root == right_place.root
        && left_place.incoming_reference == right_place.incoming_reference;
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
    // Different incoming permissions need not denote different bytes: shared
    // input descriptors may alias. The invocation admitted their permissions
    // together; keep that compatibility separate from a spatial conclusion.
    let independent_input_permissions = roots_valid
        && !same_root
        && (left_place.incoming_reference.is_valid() || right_place.incoming_reference.is_valid());
    let disjoint =
        roots_valid && !independent_input_permissions && (!same_root || !segments_may_overlap);
    let shares_dependent_fact = same_root
        && place_segments_share_dependent_fact(
            program,
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
        non_interfering: both_shared
            || independent_input_permissions
            || (disjoint && !shares_dependent_fact),
    }
}

/// Borrow access places capture a direct `self.field` at the attached field's
/// own root, while a bare `self` operand captures the machine root: the whole
/// receiver. When one place is rooted at an attached machine and the other at
/// one of that machine's attached fields, re-root the field place under the
/// machine so both compare in the receiver's namespace. Any other pair keeps
/// its own roots.
fn receiver_namespace_places(
    program: &dyn ::validation::CheckingInput,
    left: &checked_trees::CapturedPlace,
    right: &checked_trees::CapturedPlace,
) -> Option<(checked_trees::CapturedPlace, checked_trees::CapturedPlace)> {
    if !left.root.is_valid()
        || !right.root.is_valid()
        || left.incoming_reference.is_valid()
        || right.incoming_reference.is_valid()
        || left.root == right.root
    {
        return None;
    }
    if let Some(right) = attached_field_place_under_receiver(program, left, right) {
        return Some((left.clone(), right));
    }
    attached_field_place_under_receiver(program, right, left).map(|left| (left, right.clone()))
}

fn attached_field_place_under_receiver(
    program: &dyn ::validation::CheckingInput,
    receiver: &checked_trees::CapturedPlace,
    field_place: &checked_trees::CapturedPlace,
) -> Option<checked_trees::CapturedPlace> {
    let machine = program.machine_by_symbol(receiver.root.as_symbol()?)?;
    let field_symbol = field_place.root.as_symbol()?;
    let field = validation::exact_attached_field(
        program,
        machine,
        field_symbol,
        program.symbols.name(field_symbol),
    )?;
    let mut segments = Vec::with_capacity(field_place.segments.len() + 1);
    segments.push(facts::PlaceSegment::Field {
        symbol: field.symbol,
    });
    segments.extend_from_slice(&field_place.segments);
    Some(checked_trees::CapturedPlace {
        incoming_reference: Default::default(),
        root: receiver.root,
        segments,
    })
}

/// Re-roots a canonical write/access place against the loan's root exactly as
/// `canonical_place_loan_compatibility` judges the pair. Returns `None` for
/// roots that cannot identify storage at all.
pub(super) fn canonical_place_for_loan(
    place: &validation::CanonicalPlace,
    loan: &checked_trees::BorrowLoanFact,
) -> Option<checked_trees::CapturedPlace> {
    match place.root {
        facts::PlaceRoot::Symbol(symbol) => {
            if facts::PlaceRoot::Symbol(symbol) == loan.root {
                Some(checked_trees::CapturedPlace {
                    incoming_reference: Default::default(),
                    root: place.root,
                    segments: place.segments.clone(),
                })
            } else {
                match place.segments.split_first() {
                    Some((
                        facts::PlaceSegment::Field {
                            symbol: field_symbol,
                        },
                        remaining,
                    )) if facts::PlaceRoot::Symbol(*field_symbol) == loan.root => {
                        Some(checked_trees::CapturedPlace {
                            incoming_reference: Default::default(),
                            root: loan.root,
                            segments: remaining.to_vec(),
                        })
                    }
                    Some((
                        facts::PlaceSegment::Case { .. },
                        [
                            facts::PlaceSegment::Field {
                                symbol: field_symbol,
                            },
                            remaining @ ..,
                        ],
                    )) if facts::PlaceRoot::Symbol(*field_symbol) == loan.root => {
                        Some(checked_trees::CapturedPlace {
                            incoming_reference: Default::default(),
                            root: loan.root,
                            segments: remaining.to_vec(),
                        })
                    }
                    _ => Some(checked_trees::CapturedPlace {
                        incoming_reference: Default::default(),
                        root: place.root,
                        segments: place.segments.clone(),
                    }),
                }
            }
        }
        facts::PlaceRoot::Expression(_) => Some(checked_trees::CapturedPlace {
            incoming_reference: Default::default(),
            root: place.root,
            segments: place.segments.clone(),
        }),
        facts::PlaceRoot::Unknown | facts::PlaceRoot::TypeReference(_) => None,
    }
}

pub(super) fn canonical_place_loan_compatibility<'p>(
    program: &'p dyn ::validation::CheckingInput,
    place: &validation::CanonicalPlace,
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
        incoming_reference: loan.incoming_reference,
        root: loan.root,
        segments: borrow.loan_segments(loan).to_vec(),
    }
}

/// A borrow of one field participating in a dependent-data fact pins the
/// sibling fields named by that SAME fact. A direct structural overlap is not
/// required: mutating the sibling would invalidate the relation that makes the
/// borrowed projection meaningful while it remains live.
fn place_segments_share_dependent_fact(
    program: &dyn ::validation::CheckingInput,
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

    if !left_field.is_valid() || !right_field.is_valid() {
        return false;
    }
    // Resolved field identity names the declaring data owner even when the
    // place's root is an incoming referent, not the carrier holding its
    // descriptor. Looking up the root's type would check the carrier's facts
    // instead of the dependent relation governing these actual siblings.
    let parent = program.symbols.get(left_field).parent;
    if parent != program.symbols.get(right_field).parent {
        return false;
    }
    let Some(definition) = program.data_definition_by_symbol(parent) else {
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
    program: &'a dyn ::validation::CheckingInput,
    definition: &'a typed_trees::data::DataDefinition,
    symbol: symbols::SymbolHandle,
) -> Option<&'a str> {
    program
        .data_field(definition, symbol)
        .map(|field| field.name.as_str())
}

fn expression_mentions_field(
    program: &dyn ::validation::CheckingInput,
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
    program: &'p dyn ::validation::CheckingInput,
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
