use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MutableScalarRepresentationFacts {
    pub(super) domains: Vec<SymbolHandle>,
    pub(super) values: ScalarRepresentationSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ScalarRepresentationSet {
    /// Inclusive bit-pattern intervals in ascending unsigned order. Integer
    /// ranges may split at signed zero; an unconstrained scalar is the one
    /// full-width interval.
    ExactBitPatterns(Vec<(u64, u64)>),
    /// A numeric interval over one exact float carrier. This deliberately does
    /// not pretend that numeric endpoints enumerate IEEE bit patterns: it may
    /// imply another interval only on the same primitive.
    FloatInterval {
        primitive: PrimitiveType,
        minimum: i64,
        maximum: i64,
    },
}

/// The normalized representation facts carried by one scalar type reference.
///
/// Arithmetic policy changes how expressions compute, not which bit patterns
/// are established values, so it contributes no representation fact here. A
/// constant integer range is normalized into its exact two's-complement
/// bit-pattern set. Float ranges retain their primitive and numeric interval:
/// same-carrier interval inclusion is sound, while cross-carrier relations
/// remain fenced because numeric ranges do not enumerate IEEE representations.
/// Legacy named constraints remain fenced.
pub(super) fn mutable_scalar_representation_facts(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<MutableScalarRepresentationFacts> {
    let mut domains = Vec::new();
    let mut range: Option<(i64, i64)> = None;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                type_reference = *referee;
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        psi_typed_trees::types::TypeConstraintNode::Domain(domain)
                            if domain.symbol.is_valid() =>
                        {
                            if !domains.contains(&domain.symbol) {
                                domains.push(domain.symbol);
                            }
                        }
                        psi_typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                        psi_typed_trees::types::TypeConstraintNode::Range { minimum, maximum } => {
                            let minimum =
                                crate::arithmetic_domains::literal_i64(program, *minimum)?;
                            let maximum =
                                crate::arithmetic_domains::literal_i64(program, *maximum)?;
                            range = Some(match range {
                                Some((existing_minimum, existing_maximum)) => {
                                    (existing_minimum.max(minimum), existing_maximum.min(maximum))
                                }
                                None => (minimum, maximum),
                            });
                        }
                        psi_typed_trees::types::TypeConstraintNode::Domain(_)
                        | psi_typed_trees::types::TypeConstraintNode::Named(_) => {
                            return None;
                        }
                    }
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Named { name, .. }
                if PrimitiveType::from_name(name.as_str()).is_some() =>
            {
                let primitive = PrimitiveType::from_name(name.as_str())?;
                let values = match (primitive, range) {
                    (PrimitiveType::Bool, None) => {
                        ScalarRepresentationSet::ExactBitPatterns(vec![(0, 1)])
                    }
                    (PrimitiveType::Bool, Some(_)) => return None,
                    (
                        primitive @ (PrimitiveType::F32 | PrimitiveType::F64),
                        Some((minimum, maximum)),
                    ) if minimum <= maximum => ScalarRepresentationSet::FloatInterval {
                        primitive,
                        minimum,
                        maximum,
                    },
                    (PrimitiveType::F32 | PrimitiveType::F64, Some(_)) => return None,
                    (_, Some(range)) => ScalarRepresentationSet::ExactBitPatterns(
                        integer_range_bit_patterns(primitive, range)?,
                    ),
                    (_, None) => ScalarRepresentationSet::ExactBitPatterns(
                        full_scalar_bit_patterns(primitive),
                    ),
                };
                return Some(MutableScalarRepresentationFacts { domains, values });
            }
            _ => return None,
        }
    }
}

pub(super) fn full_scalar_bit_patterns(primitive: PrimitiveType) -> Vec<(u64, u64)> {
    let bit_count = primitive
        .scalar_byte_size()
        .expect("scalar primitive must have a byte size")
        * 8;
    let maximum = if bit_count == 64 {
        u64::MAX
    } else {
        (1u64 << bit_count) - 1
    };
    vec![(0, maximum)]
}

/// Normalize one inclusive integer value interval into the exact set of stored
/// bit patterns. Signed negative values occupy the high unsigned interval; a
/// range crossing zero therefore becomes two intervals. This makes
/// `i32 [0..=100]` representation-equivalent to `u32 [0..=100]`, while
/// `i32 [-1..=99]` is correctly distinct despite having the same cardinality.
fn integer_range_bit_patterns(
    primitive: PrimitiveType,
    (minimum, maximum): (i64, i64),
) -> Option<Vec<(u64, u64)>> {
    if minimum > maximum || !primitive.accepts_integer_literal() {
        return None;
    }
    let bit_count = primitive.scalar_byte_size()? * 8;
    let mask = if bit_count == 64 {
        u64::MAX
    } else {
        (1u64 << bit_count) - 1
    };
    let signed = primitive.is_signed_integer();
    if signed {
        let (primitive_minimum, primitive_maximum) = if bit_count == 64 {
            (i64::MIN, i64::MAX)
        } else {
            let half = 1i64 << (bit_count - 1);
            (-half, half - 1)
        };
        if minimum < primitive_minimum || maximum > primitive_maximum {
            return None;
        }
        let bits = |value: i64| (value as u64) & mask;
        return Some(normalize_bit_pattern_intervals(
            if maximum < 0 || minimum >= 0 {
                vec![(bits(minimum), bits(maximum))]
            } else {
                vec![(0, bits(maximum)), (bits(minimum), mask)]
            },
        ));
    }

    if minimum < 0 || (bit_count < 64 && maximum as u64 > mask) {
        return None;
    }
    Some(vec![(minimum as u64, maximum as u64)])
}

/// Canonicalize an exact representation set so equality depends on the bits it
/// denotes, not on how a source interval happened to partition them. This is
/// load-bearing for a full signed range: `i8 [-128..=127]` initially produces
/// `[0,127] + [128,255]`, which is the same set as unconstrained `u8`.
fn normalize_bit_pattern_intervals(mut intervals: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    intervals.sort_unstable_by_key(|&(low, high)| (low, high));
    let mut normalized: Vec<(u64, u64)> = Vec::with_capacity(intervals.len());
    for (low, high) in intervals {
        if let Some((_, previous_high)) = normalized.last_mut()
            && low <= previous_high.saturating_add(1)
        {
            *previous_high = (*previous_high).max(high);
        } else {
            normalized.push((low, high));
        }
    }
    normalized
}

/// Mutable aliases are safe only when arbitrary writes accepted through either
/// view remain established through the other. Domain conjunctions therefore
/// owe implication in both directions, and their normalized bit-pattern sets
/// must be identical. The normalized domain graph already accounts for shared
/// semantic identities and explicit membership chains.
pub(super) fn mutable_scalar_representation_facts_equivalent(
    program: &TypedTrees,
    source: &MutableScalarRepresentationFacts,
    target: &MutableScalarRepresentationFacts,
) -> bool {
    if source.values != target.values {
        return false;
    }
    let implies = |sources: &[SymbolHandle], targets: &[SymbolHandle]| {
        targets.iter().all(|target| {
            sources.iter().any(|source| {
                psi_typed_trees::domain::declared_domain_implies(program, *source, *target)
            })
        })
    };
    implies(&source.domains, &target.domains) && implies(&target.domains, &source.domains)
}

pub(super) fn scalar_representation_facts_imply(
    program: &TypedTrees,
    source: &MutableScalarRepresentationFacts,
    target: &MutableScalarRepresentationFacts,
) -> bool {
    let domains_imply = target.domains.iter().all(|target| {
        source.domains.iter().any(|source| {
            psi_typed_trees::domain::declared_domain_implies(program, *source, *target)
        })
    });
    domains_imply && scalar_representation_set_implies(&source.values, &target.values)
}

fn scalar_representation_set_implies(
    source: &ScalarRepresentationSet,
    target: &ScalarRepresentationSet,
) -> bool {
    match (source, target) {
        (
            ScalarRepresentationSet::ExactBitPatterns(source),
            ScalarRepresentationSet::ExactBitPatterns(target),
        ) => source.iter().all(|(source_low, source_high)| {
            target.iter().any(|(target_low, target_high)| {
                target_low <= source_low && source_high <= target_high
            })
        }),
        (
            ScalarRepresentationSet::FloatInterval {
                primitive: source_primitive,
                minimum: source_minimum,
                maximum: source_maximum,
            },
            ScalarRepresentationSet::FloatInterval {
                primitive: target_primitive,
                minimum: target_minimum,
                maximum: target_maximum,
            },
        ) => {
            source_primitive == target_primitive
                && target_minimum <= source_minimum
                && source_maximum <= target_maximum
        }
        (
            ScalarRepresentationSet::FloatInterval { primitive, .. },
            ScalarRepresentationSet::ExactBitPatterns(target),
        ) => target == &full_scalar_bit_patterns(*primitive),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MutableScalarRepresentationFacts, ScalarRepresentationSet, integer_range_bit_patterns,
        mutable_scalar_representation_facts_equivalent, scalar_representation_facts_imply,
    };
    use psi_typed_trees::types::PrimitiveType;

    #[test]
    fn signed_negative_ranges_normalize_to_high_unsigned_patterns() {
        assert_eq!(
            integer_range_bit_patterns(PrimitiveType::I8, (-4, -1)),
            Some(vec![(252, 255)])
        );
        assert_eq!(
            integer_range_bit_patterns(PrimitiveType::U8, (252, 255)),
            Some(vec![(252, 255)])
        );
    }

    #[test]
    fn signed_ranges_crossing_zero_split_without_inventing_the_gap() {
        assert_eq!(
            integer_range_bit_patterns(PrimitiveType::I8, (-2, 2)),
            Some(vec![(0, 2), (254, 255)])
        );
    }

    #[test]
    fn full_signed_ranges_canonicalize_to_the_carriers_complete_bit_set() {
        assert_eq!(
            integer_range_bit_patterns(PrimitiveType::I8, (-128, 127)),
            Some(vec![(0, 255)])
        );
        assert_eq!(
            integer_range_bit_patterns(PrimitiveType::I16, (-32_768, 32_767)),
            Some(vec![(0, 65_535)])
        );
    }

    #[test]
    fn bool_representation_may_weaken_but_only_equal_sets_alias_mutably() {
        let program = psi_typed_trees::TypedTrees::default();
        let boolean = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::ExactBitPatterns(vec![(0, 1)]),
        };
        let bounded_byte = boolean.clone();
        let unconstrained_byte = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::ExactBitPatterns(vec![(0, 255)]),
        };

        assert!(scalar_representation_facts_imply(
            &program,
            &boolean,
            &unconstrained_byte
        ));
        assert!(!scalar_representation_facts_imply(
            &program,
            &unconstrained_byte,
            &boolean
        ));
        assert!(mutable_scalar_representation_facts_equivalent(
            &program,
            &boolean,
            &bounded_byte
        ));
        assert!(!mutable_scalar_representation_facts_equivalent(
            &program,
            &boolean,
            &unconstrained_byte
        ));
    }

    #[test]
    fn float_intervals_imply_only_same_carrier_supersets() {
        let program = psi_typed_trees::TypedTrees::default();
        let narrow = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::FloatInterval {
                primitive: PrimitiveType::F32,
                minimum: 0,
                maximum: 1,
            },
        };
        let wide = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::FloatInterval {
                primitive: PrimitiveType::F32,
                minimum: -1,
                maximum: 2,
            },
        };
        let other_carrier = MutableScalarRepresentationFacts {
            domains: Vec::new(),
            values: ScalarRepresentationSet::FloatInterval {
                primitive: PrimitiveType::F64,
                minimum: -1,
                maximum: 2,
            },
        };

        assert!(scalar_representation_facts_imply(&program, &narrow, &wide));
        assert!(!scalar_representation_facts_imply(&program, &wide, &narrow));
        assert!(!scalar_representation_facts_imply(
            &program,
            &narrow,
            &other_carrier
        ));
        assert!(mutable_scalar_representation_facts_equivalent(
            &program, &narrow, &narrow
        ));
    }
}
