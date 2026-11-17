//! Consequences of complete fixed-array defaults, without replacing those
//! defaults. For example, bounds on every byte establish ASCII; a UTF-8
//! disjunction supplies no such bound. The ordinary storage checker still
//! enforces initialization, all other clauses and restoration windows.

use semantic_vocabulary::{
    ByteSequencePredicate, CanonicalStructuralPathSegment, ContentPlaceVersion, IntegerSign,
    IntegerValue, PlaceId, Proposition, ScalarTerm, StructuralPlaceKind,
};
use terminal_psi::{
    ArrayExtent, ByteSequencePredicateSet, StructuralAccess, StructuralArgument,
    StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(super) fn argument_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<ByteSequencePredicateSet> {
    if argument.access == StructuralAccess::WriteOnlyBorrow {
        return None;
    }
    let root = machine.structural_place_type(argument.place)?;
    if matches!(
        machine.structural_place(argument.place)?.kind,
        StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. }
    ) && crate::arrival_parameter(machine, argument.place)?.access
        == StructuralAccess::WriteOnlyBorrow
    {
        return None;
    }
    let carrier =
        crate::runtime_structural_path_tip(module.structural_types.iter(), root, &argument.path)?;
    let StructuralTypeShape::FixedArray {
        length: ArrayExtent::Literal(length),
        ..
    } = carrier.shape
    else {
        return None;
    };
    if !crate::byte_predicate_carrier(module.structural_types.iter(), carrier.id) {
        return None;
    }
    let defaults = crate::structural_qualifications::DefaultRequirements::new(
        &module.structural_types,
        &module.structural_domains,
        machine,
    )
    .ok()?;
    let clauses = defaults.storage(argument.place, &argument.path).ok()?;
    let clauses = clauses
        .iter()
        .map(|required| defaults.clause(required))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some(consequences(length, &clauses))
}

fn consequences(length: u64, clauses: &[Proposition]) -> ByteSequencePredicateSet {
    let subject = PlaceId::new(1).expect("self is nonzero");
    let mut bounds = Vec::<(u64, u8, u8)>::new();
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    let mut pending = clauses.iter().collect::<Vec<_>>();
    let field = |term: &ScalarTerm| {
        let ScalarTerm::IntegerField {
            version: ContentPlaceVersion::Current,
            root,
            path,
            scalar_type,
        } = term
        else {
            return None;
        };
        if *root != subject
            || scalar_type.sign() != IntegerSign::Unsigned
            || scalar_type.bits() != Some(8)
        {
            return None;
        }
        match path.as_slice() {
            [CanonicalStructuralPathSegment::FixedIndex(index)] if *index < length => Some(*index),
            _ => None,
        }
    };
    let literal = |term: &ScalarTerm| {
        let ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } = term
        else {
            return None;
        };
        (scalar_type.sign() == IntegerSign::Unsigned && scalar_type.bits() == Some(8))
            .then(|| u8::try_from(*value).ok())
            .flatten()
    };
    while let Some(clause) = pending.pop() {
        let (left, right, equality) = match clause {
            Proposition::Conjunction(parts) => {
                pending.extend(parts);
                continue;
            }
            Proposition::LessOrEqual(left, right) => (left, right, false),
            Proposition::Equal(left, right) => (left, right, true),
            _ => continue,
        };
        if equality {
            let asserted = match (left, right) {
                (
                    ScalarTerm::ByteSequencePredicate {
                        predicate,
                        root,
                        path,
                    },
                    ScalarTerm::Boolean(true),
                )
                | (
                    ScalarTerm::Boolean(true),
                    ScalarTerm::ByteSequencePredicate {
                        predicate,
                        root,
                        path,
                    },
                ) if *root == subject && path.is_empty() => Some(*predicate),
                _ => None,
            };
            if let Some(predicate) = asserted {
                predicates.insert(predicate);
            }
        }
        if let (Some(index), Some(value)) = (field(left), literal(right)) {
            bounds.push((index, if equality { value } else { 0 }, value));
        } else if let (Some(value), Some(index)) = (literal(left), field(right)) {
            bounds.push((index, value, if equality { value } else { 255 }));
        }
    }
    // Storage follows the supplied clause roster, never the largest received
    // array extent. Missing cells contribute no predicate.
    bounds.sort_unstable_by_key(|row| row.0);
    let mut merged = Vec::<(u64, u8, u8)>::with_capacity(bounds.len());
    for (index, low, high) in bounds {
        if let Some(previous) = merged.last_mut().filter(|row| row.0 == index) {
            previous.1 = previous.1.max(low);
            previous.2 = previous.2.min(high);
        } else {
            merged.push((index, low, high));
        }
    }
    let complete = u64::try_from(merged.len()).ok() == Some(length)
        && merged
            .iter()
            .enumerate()
            .all(|(position, row)| u64::try_from(position).ok() == Some(row.0));
    if length > 0 {
        predicates.insert(ByteSequencePredicate::NonEmpty);
    }
    if complete && merged.iter().all(|row| row.2 <= 127) {
        predicates.insert(ByteSequencePredicate::AsciiOnly);
        predicates.insert(ByteSequencePredicate::ValidUtf8);
    }
    if complete && merged.iter().all(|row| row.1 >= 1) {
        predicates.insert(ByteSequencePredicate::NoNul);
    }
    predicates
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::IntegerType;

    fn bound(index: u64, high: u8) -> Proposition {
        let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        Proposition::LessOrEqual(
            ScalarTerm::IntegerField {
                version: ContentPlaceVersion::Current,
                root: PlaceId::new(1).unwrap(),
                path: vec![CanonicalStructuralPathSegment::FixedIndex(index)],
                scalar_type: byte,
            },
            ScalarTerm::integer(byte, IntegerValue::Unsigned(u128::from(high))).unwrap(),
        )
    }

    #[test]
    fn only_required_bounds_on_every_cell_establish_ascii() {
        let clauses = vec![Proposition::Conjunction(vec![bound(0, 126), bound(1, 127)])];
        let expected = consequences(2, &clauses);
        assert!(
            expected
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::AsciiOnly)
        );
        assert!(
            expected
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
        );
        assert!(
            !expected
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::NoNul)
        );
        for clauses in [
            vec![bound(0, 127)],
            vec![bound(0, 127), bound(1, 128)],
            vec![Proposition::Disjunction(vec![bound(0, 127), bound(1, 127)])],
            vec![Proposition::Implication {
                premise: Box::new(bound(0, 127)),
                conclusion: Box::new(bound(1, 127)),
            }],
        ] {
            assert!(
                !consequences(2, &clauses)
                    .predicates()
                    .any(|predicate| predicate == ByteSequencePredicate::AsciiOnly)
            );
        }
        let asserted = |root| {
            Proposition::Equal(
                ScalarTerm::byte_sequence_predicate(
                    ByteSequencePredicate::AsciiOnly,
                    root,
                    Vec::new(),
                ),
                ScalarTerm::Boolean(true),
            )
        };
        assert!(
            consequences(2, &[asserted(PlaceId::new(1).unwrap())])
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::AsciiOnly)
        );
        assert!(
            !consequences(2, &[asserted(PlaceId::new(2).unwrap())])
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::AsciiOnly)
        );
        // A received extent cannot cause extent-sized scratch allocation.
        assert!(
            !consequences(u64::MAX, &clauses)
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::AsciiOnly)
        );
    }
}
