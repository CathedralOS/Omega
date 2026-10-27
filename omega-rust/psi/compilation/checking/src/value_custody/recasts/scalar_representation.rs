use symbols::SymbolHandle;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MutableScalarRepresentationFacts {
    pub(super) atomic: bool,
    pub(super) domains: Vec<SymbolHandle>,
    pub(super) values: ScalarRepresentationSet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ScalarRepresentationSet {
    /// Inclusive bit-pattern intervals in ascending unsigned order. Integer
    /// ranges may split at signed zero; an unconstrained scalar is the one
    /// full-width interval.
    ExactBitPatterns(Vec<(u64, u64)>),
    /// A closed numeric interval over one exact float carrier, excluding NaN.
    /// This deliberately does not pretend that numeric endpoints enumerate
    /// IEEE bit patterns: it may imply another interval only on the same
    /// primitive. An unbounded side is the matching infinity, which every
    /// ordered comparison still admits.
    FloatInterval {
        primitive: PrimitiveType,
        minimum: FloatEndpoint,
        maximum: FloatEndpoint,
    },
}

/// One exact float interval endpoint, held as the bits of its `f64` value so
/// interval identity is structural. Every `f32` value widens to `f64` exactly,
/// so one representation serves both carriers. NaN is never an endpoint and
/// zero is unsigned: `-0.0` and `0.0` bound the same numeric set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FloatEndpoint(u64);

impl FloatEndpoint {
    const NEGATIVE_INFINITY: Self = Self(f64::NEG_INFINITY.to_bits());
    const POSITIVE_INFINITY: Self = Self(f64::INFINITY.to_bits());

    fn new(value: f64) -> Option<Self> {
        if value.is_nan() {
            return None;
        }
        Some(Self(if value == 0.0 { 0.0f64 } else { value }.to_bits()))
    }

    /// An integer endpoint the carrier represents exactly. A bound the format
    /// would round names a different numeric set, so it stays unsupported.
    fn from_integer(primitive: PrimitiveType, value: i64) -> Option<Self> {
        const EXACT_IN_F64: i64 = 1 << 53;
        if !(-EXACT_IN_F64..=EXACT_IN_F64).contains(&value) {
            return None;
        }
        let widened = value as f64;
        if primitive == PrimitiveType::F32 && f64::from(widened as f32) != widened {
            return None;
        }
        Self::new(widened)
    }

    fn value(self) -> f64 {
        f64::from_bits(self.0)
    }

    fn maximum(self, other: Self) -> Self {
        if other.value() > self.value() {
            other
        } else {
            self
        }
    }

    fn minimum(self, other: Self) -> Self {
        if other.value() < self.value() {
            other
        } else {
            self
        }
    }
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
    program: &dyn crate::CheckingInput,
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
                        typed_trees::types::TypeConstraintNode::Domain(domain)
                            if domain.symbol.is_valid() =>
                        {
                            if !domains.contains(&domain.symbol) {
                                domains.push(domain.symbol);
                            }
                        }
                        typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {}
                        typed_trees::types::TypeConstraintNode::Range {
                            minimum,
                            maximum,
                            end_inclusive,
                        } => {
                            let minimum =
                                crate::closed_integer_range_bound(program, *minimum)?.to_i64()?;
                            let maximum = crate::closed_integer_range_maximum(
                                program,
                                *maximum,
                                *end_inclusive,
                            )?
                            .to_i64()?;
                            range = Some(match range {
                                Some((existing_minimum, existing_maximum)) => {
                                    (existing_minimum.max(minimum), existing_maximum.min(maximum))
                                }
                                None => (minimum, maximum),
                            });
                        }
                        typed_trees::types::TypeConstraintNode::Domain(_)
                        | typed_trees::types::TypeConstraintNode::Named(_) => {
                            return None;
                        }
                    }
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Named { symbol, name }
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
                        minimum: FloatEndpoint::from_integer(primitive, minimum)?,
                        maximum: FloatEndpoint::from_integer(primitive, maximum)?,
                    },
                    (PrimitiveType::F32 | PrimitiveType::F64, Some(_)) => return None,
                    (_, Some(range)) => ScalarRepresentationSet::ExactBitPatterns(
                        integer_range_bit_patterns(primitive, range)?,
                    ),
                    (_, None) => ScalarRepresentationSet::ExactBitPatterns(
                        full_scalar_bit_patterns(primitive),
                    ),
                };
                let atomic = program
                    .symbols
                    .builtin_type_atom(*symbol)
                    .is_some_and(|atom| atom.is_atomic());
                return Some(MutableScalarRepresentationFacts {
                    atomic,
                    domains,
                    values,
                });
            }
            _ => return None,
        }
    }
}

/// The physical representations of an IntegerAt leaf are the logical values
/// that its selected encoding can store, mapped to that encoding's bit width.
/// Comparing the unencoded carrier would confuse an i64 stored in one signed
/// byte with an eight-byte referent. Declaration facts still restrict the set;
/// concrete semantic writes keep their separate fitting obligation.
pub(super) fn encode_stored_integer_facts(
    facts: &mut MutableScalarRepresentationFacts,
    primitive: PrimitiveType,
    width: u16,
    interpretation: layout_plans::IntegerInterpretation,
) -> Option<()> {
    let carrier_width = u16::try_from(primitive.scalar_byte_size()?.checked_mul(8)?).ok()?;
    if !primitive.accepts_integer_literal() || width == 0 || width > carrier_width {
        return None;
    }
    let ScalarRepresentationSet::ExactBitPatterns(values) = &mut facts.values else {
        return None;
    };
    if width == 64 {
        // Layout admission already requires total decoding into the logical
        // carrier. At full width its bit coordinates are unchanged; a signed
        // carrier with an unsigned 64-bit encoding was rejected at admission.
        return Some(());
    }
    let mask = (1u64 << width) - 1;
    let (minimum, maximum) = match interpretation {
        layout_plans::IntegerInterpretation::Signed => {
            let half = 1i64 << (width - 1);
            (-half, half - 1)
        }
        layout_plans::IntegerInterpretation::Unsigned => (0, i64::try_from(mask).ok()?),
    };
    let admitted = integer_range_bit_patterns(primitive, (minimum, maximum))?;
    let fitting = intersect_bit_pattern_intervals(std::mem::take(values), &admitted);
    *values = normalize_bit_pattern_intervals(
        fitting
            .into_iter()
            .map(|(lower, upper)| (lower & mask, upper & mask))
            .collect(),
    );
    Some(())
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

/// Narrow a representation set by a place's `where`-fact value interval, so a
/// constraint spelled on the data declaration compares identically to one
/// spelled on the type itself. Either endpoint may be open; the side that
/// stays open imposes nothing and skips narrowing on it.
pub(super) fn narrow_to_integer_interval(
    facts: &mut MutableScalarRepresentationFacts,
    primitive: PrimitiveType,
    minimum: Option<i64>,
    maximum: Option<i64>,
) {
    let (Some(minimum), Some(maximum)) = (minimum, maximum) else {
        return;
    };
    if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
        // Narrowing only removes values, so an endpoint the carrier cannot
        // represent exactly skips the narrowing rather than inventing a fact.
        if let (Some(minimum), Some(maximum)) = (
            FloatEndpoint::from_integer(primitive, minimum),
            FloatEndpoint::from_integer(primitive, maximum),
        ) {
            narrow_to_float_interval(facts, primitive, minimum, maximum);
        }
        return;
    }
    if let ScalarRepresentationSet::ExactBitPatterns(values) = &mut facts.values
        && let Some(interval) = integer_range_bit_patterns(primitive, (minimum, maximum))
    {
        *values = intersect_bit_pattern_intervals(std::mem::take(values), &interval);
    }
}

/// Narrow a float leaf to a closed numeric interval. A float type with no
/// attached range carries its full bit set; an interval on it is the same
/// fact a float range states.
fn narrow_to_float_interval(
    facts: &mut MutableScalarRepresentationFacts,
    primitive: PrimitiveType,
    minimum: FloatEndpoint,
    maximum: FloatEndpoint,
) {
    match &mut facts.values {
        ScalarRepresentationSet::ExactBitPatterns(_) => {
            facts.values = ScalarRepresentationSet::FloatInterval {
                primitive,
                minimum,
                maximum,
            };
        }
        ScalarRepresentationSet::FloatInterval {
            minimum: facts_minimum,
            maximum: facts_maximum,
            ..
        } => {
            *facts_minimum = facts_minimum.maximum(minimum);
            *facts_maximum = facts_maximum.minimum(maximum);
        }
    }
}

/// Consume complete field-validity rows without reopening predicate bodies.
/// Integer envelopes become exact stored bit sets. IEEE literal comparisons
/// keep their selected meaning and format; unsupported bounds fence the record.
pub(super) fn narrow_by_field_representation_bounds(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    facts: &mut MutableScalarRepresentationFacts,
    primitive: PrimitiveType,
    bounds: &[crate::proof_contracts::default_domains::FieldRepresentationBound],
) -> Option<()> {
    use crate::proof_contracts::default_domains::FieldRepresentationBound;
    use typed_trees::expression::BinaryOperator;
    if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
        let mut minimum = FloatEndpoint::NEGATIVE_INFINITY;
        let mut maximum = FloatEndpoint::POSITIVE_INFINITY;
        for bound in bounds {
            let FieldRepresentationBound::FloatLiteral(bound) = bound else {
                return None;
            };
            let meaning =
                crate::proof_contracts::default_domains::float_field_literal_bound_meaning(
                    program,
                    definition,
                    bound.conjunct,
                )?;
            let value = FloatEndpoint::new(meaning.to_f64())?;
            match bound.operator {
                BinaryOperator::LessOrEqual => maximum = maximum.minimum(value),
                BinaryOperator::GreaterOrEqual => minimum = minimum.maximum(value),
                BinaryOperator::Equal => {
                    minimum = minimum.maximum(value);
                    maximum = maximum.minimum(value);
                }
                _ => return None,
            }
        }
        if minimum.value() > maximum.value() {
            return None;
        }
        narrow_to_float_interval(facts, primitive, minimum, maximum);
        return Some(());
    }
    if !primitive.accepts_integer_literal() {
        return None;
    }
    let ScalarRepresentationSet::ExactBitPatterns(values) = &mut facts.values else {
        return None;
    };
    for bound in bounds {
        let FieldRepresentationBound::Integer {
            minimum, maximum, ..
        } = bound
        else {
            return None;
        };
        let (carrier_minimum, carrier_maximum) = primitive_integer_extent(primitive)?;
        let minimum = minimum.unwrap_or(carrier_minimum).max(carrier_minimum);
        let maximum = maximum.unwrap_or(carrier_maximum).min(carrier_maximum);
        let interval = integer_range_bit_patterns(primitive, (minimum, maximum))?;
        *values = intersect_bit_pattern_intervals(std::mem::take(values), &interval);
    }
    if values.is_empty() {
        return None;
    }
    *values = normalize_bit_pattern_intervals(std::mem::take(values));
    Some(())
}

/// The inclusive value extent of an integer carrier, for one-sided bounds.
/// `u64` values above `i64::MAX` have no `i64` spelling, so a one-sided `u64`
/// bound stays unsupported rather than truncating the extent.
fn primitive_integer_extent(primitive: PrimitiveType) -> Option<(i64, i64)> {
    let bit_count = primitive.scalar_byte_size()? * 8;
    if primitive.is_signed_integer() {
        if bit_count == 64 {
            return Some((i64::MIN, i64::MAX));
        }
        let half = 1i64 << (bit_count - 1);
        return Some((-half, half - 1));
    }
    if bit_count >= 64 {
        return None;
    }
    Some((0, (1i64 << bit_count) - 1))
}

fn intersect_bit_pattern_intervals(
    source: Vec<(u64, u64)>,
    interval: &[(u64, u64)],
) -> Vec<(u64, u64)> {
    let mut intersected = Vec::new();
    for (low, high) in source {
        for &(interval_low, interval_high) in interval {
            let low = low.max(interval_low);
            let high = high.min(interval_high);
            if low <= high {
                intersected.push((low, high));
            }
        }
    }
    intersected
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
    program: &dyn crate::CheckingInput,
    source: &MutableScalarRepresentationFacts,
    target: &MutableScalarRepresentationFacts,
) -> bool {
    if source.values != target.values {
        return false;
    }
    let implies = |sources: &[SymbolHandle], targets: &[SymbolHandle]| {
        targets.iter().all(|target| {
            sources.iter().any(|source| {
                typed_trees::domain::declared_domain_implies(program, *source, *target)
            })
        })
    };
    implies(&source.domains, &target.domains) && implies(&target.domains, &source.domains)
}

pub(super) fn scalar_representation_facts_imply(
    program: &dyn crate::CheckingInput,
    source: &MutableScalarRepresentationFacts,
    target: &MutableScalarRepresentationFacts,
) -> bool {
    // Shared views preserve the observation protocol. Atomic targets can
    // also write, so their validity obligation is two-way despite `&`.
    // Exclusive views use the independent equivalence judgment above.
    if source.atomic != target.atomic {
        return false;
    }
    if target.atomic {
        return mutable_scalar_representation_facts_equivalent(program, source, target);
    }
    let domains_imply = target.domains.iter().all(|target| {
        source
            .domains
            .iter()
            .any(|source| typed_trees::domain::declared_domain_implies(program, *source, *target))
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
                && target_minimum.value() <= source_minimum.value()
                && source_maximum.value() <= target_maximum.value()
        }
        (
            ScalarRepresentationSet::FloatInterval { primitive, .. },
            ScalarRepresentationSet::ExactBitPatterns(target),
        ) => target == &full_scalar_bit_patterns(*primitive),
        _ => false,
    }
}
