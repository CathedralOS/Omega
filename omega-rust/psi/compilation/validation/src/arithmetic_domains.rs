/*
Source carrier and declared-range queries. Executable arithmetic, guards, writes
and return obligations are judged by the lowered-form checker.
*/

pub(crate) mod declared_float_facts {
    use crate::proof_contracts::default_domains::FieldRepresentationBound;
    use numerics::bignum::ExactFloat;
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Clone)]
    pub struct DeclaredFloatFieldBound {
        pub path: Vec<SymbolHandle>,
        pub primitive: PrimitiveType,
        pub operator: typed_trees::expression::BinaryOperator,
        pub value: f64,
    }

    impl DeclaredFloatFieldBound {
        pub fn predicate(&self) -> Option<facts::FloatPredicate> {
            let format = match self.primitive {
                PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
                PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
                _ => return None,
            };
            facts::FloatPredicate::comparison(
                format,
                self.operator,
                &ExactFloat::from_f64(self.value),
            )
        }
    }

    pub fn declared_float_field_bounds(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Vec<DeclaredFloatFieldBound> {
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program)
            && let Ok(cache) = memos.declared_float_field_bounds.lock()
            && let Some(rows) = cache.get(&reference)
        {
            return rows.clone();
        }
        let mut rows = Vec::new();
        collect(
            program,
            reference,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut rows,
        );
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program)
            && let Ok(mut cache) = memos.declared_float_field_bounds.lock()
        {
            cache.insert(reference, rows.clone());
        }
        rows
    }

    fn collect(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
        path: &mut Vec<SymbolHandle>,
        active: &mut Vec<SymbolHandle>,
        rows: &mut Vec<DeclaredFloatFieldBound>,
    ) {
        for _ in 0..program.type_reference_table.type_reference_count() {
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::WriteOnly,
                    ..
                } => return,
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => reference = *referee,
                _ => break,
            }
        }
        let Some(definition) =
            crate::value_custody::places::data_definition_for_type(program, reference)
        else {
            return;
        };
        if active.contains(&definition.symbol) {
            return;
        }
        active.push(definition.symbol);
        if let Some(bounds) =
            crate::proof_contracts::default_domains::data_requires_field_representation_bounds(
                program, definition,
            )
        {
            for bound in bounds {
                let FieldRepresentationBound::FloatLiteral(bound) = bound else {
                    continue;
                };
                let Some(field) = program.data_field(definition, bound.field) else {
                    continue;
                };
                let Some(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) =
                    program.primitive_type_reference(field.type_reference)
                else {
                    continue;
                };
                let Some(meaning) =
                    crate::proof_contracts::default_domains::float_field_literal_bound_meaning(
                        program,
                        definition,
                        bound.conjunct,
                    )
                else {
                    continue;
                };
                let value = meaning.to_f64();
                if !value.is_finite() {
                    continue;
                }
                let mut field_path = path.clone();
                field_path.push(field.symbol);
                rows.push(DeclaredFloatFieldBound {
                    path: field_path,
                    primitive,
                    operator: bound.operator,
                    value,
                });
            }
        }
        for member in program.data_members(definition) {
            if let DataMember::Field(field) = member {
                path.push(field.symbol);
                collect(program, field.type_reference, path, active, rows);
                path.pop();
            }
        }
        active.pop();
    }
}
pub use declared_float_facts::{DeclaredFloatFieldBound, declared_float_field_bounds};
pub(crate) mod declared_source_ranges {
    use super::range_constraints::range_constraint_interval;
    use super::{
        ArithmeticDomain, ExpressionHandle, Interval, Machine, State, TypeReferenceHandle,
    };
    use crate::value_custody::atomic_operations::is_atomic_type;

    pub(crate) fn enforced_declared_range(
        program: &dyn crate::CheckingInput,
        handle: TypeReferenceHandle,
    ) -> Option<Interval> {
        if is_atomic_type(program, handle)
            || program.arithmetic_domain_for_type_reference(handle) != ArithmeticDomain::Exact
        {
            return None;
        }
        range_constraint_interval(program, handle)
    }

    fn is_unresolved_bare_name(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(path) = program.expression_table.name_path_node(expression) else {
            return false;
        };
        let [first] = program.expression_table.name_path_members(path.members) else {
            return false;
        };
        !path.symbol.is_valid()
            && !path.head_symbol.is_valid()
            && language_semantics::const_value::boolean_literal_spelling(first.as_str()).is_none()
            && super::declared_place_type_raw(program, machine, Some(state), expression).is_none()
    }

    pub(crate) fn expression_reads_unresolved_local_initializer(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(state) = state else {
            return false;
        };
        let Some(path) = program.expression_table.name_path_node(expression) else {
            return false;
        };
        let [name] = program.expression_table.name_path_members(path.members) else {
            return false;
        };
        for statement in program.statement_table.statements(state.statement_nodes) {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            if local.name.as_str() == name.as_str() && local.initial_value.is_valid() {
                return is_unresolved_bare_name(program, machine, state, local.initial_value);
            }
        }
        false
    }
}

pub(crate) mod bigint_interval {
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BigIntInterval {
        pub low: Option<BigInt>,
        pub high: Option<BigInt>,
    }

    impl BigIntInterval {
        pub fn unbounded() -> Self {
            Self {
                low: None,
                high: None,
            }
        }

        pub fn constant(value: BigInt) -> Self {
            Self {
                low: Some(value.clone()),
                high: Some(value),
            }
        }

        pub fn intersect(&self, other: &Self) -> Self {
            Self {
                low: match (&self.low, &other.low) {
                    (Some(left), Some(right)) => Some(left.clone().max(right.clone())),
                    (Some(value), None) | (None, Some(value)) => Some(value.clone()),
                    (None, None) => None,
                },
                high: match (&self.high, &other.high) {
                    (Some(left), Some(right)) => Some(left.clone().min(right.clone())),
                    (Some(value), None) | (None, Some(value)) => Some(value.clone()),
                    (None, None) => None,
                },
            }
        }

        pub fn add(&self, other: &Self) -> Self {
            Self {
                low: match (&self.low, &other.low) {
                    (Some(a), Some(b)) => Some(a.add(b)),
                    _ => None,
                },
                high: match (&self.high, &other.high) {
                    (Some(a), Some(b)) => Some(a.add(b)),
                    _ => None,
                },
            }
        }

        pub fn subtract(&self, other: &Self) -> Self {
            Self {
                low: self
                    .low
                    .as_ref()
                    .zip(other.high.as_ref())
                    .map(|(left, right)| left.sub(right)),
                high: self
                    .high
                    .as_ref()
                    .zip(other.low.as_ref())
                    .map(|(left, right)| left.sub(right)),
            }
        }

        pub fn union(&self, other: &Self) -> Self {
            Self {
                low: self
                    .low
                    .as_ref()
                    .zip(other.low.as_ref())
                    .map(|(left, right)| left.clone().min(right.clone())),
                high: self
                    .high
                    .as_ref()
                    .zip(other.high.as_ref())
                    .map(|(left, right)| left.clone().max(right.clone())),
            }
        }

        pub fn excludes_zero(&self) -> bool {
            self.low.as_ref().is_some_and(|low| low > &BigInt::zero())
                || self.high.as_ref().is_some_and(BigInt::is_negative)
        }

        pub fn divide(&self, divisor: &Self) -> Self {
            let (dividend, divisor) = if divisor.high.as_ref().is_some_and(BigInt::is_negative) {
                let negative_one = BigInt::from_i64(-1);
                (self.scale(&negative_one), divisor.scale(&negative_one))
            } else {
                (self.clone(), divisor.clone())
            };
            if !divisor
                .low
                .as_ref()
                .is_some_and(|low| low > &BigInt::zero())
            {
                return Self::unbounded();
            }
            let endpoint = |numerator: &Option<BigInt>, lower: bool| {
                let numerator = numerator.as_ref()?;
                let denominator = if lower == numerator.is_negative() {
                    &divisor.low
                } else {
                    &divisor.high
                };
                match denominator {
                    Some(denominator) => {
                        numerator.div_rem(denominator).map(|(quotient, _)| quotient)
                    }
                    None => Some(BigInt::zero()),
                }
            };
            Self {
                low: endpoint(&dividend.low, true),
                high: endpoint(&dividend.high, false),
            }
        }

        pub fn remainder(&self, divisor: &Self) -> Self {
            let dividend = self.union(&Self::constant(BigInt::zero()));
            let Some(magnitude) = divisor
                .low
                .as_ref()
                .zip(divisor.high.as_ref())
                .map(|(low, high)| low.abs().max(high.abs()))
            else {
                return dividend;
            };
            if magnitude.is_zero() {
                return Self::unbounded();
            }
            let bound = magnitude.sub(&BigInt::from_i64(1));
            dividend.intersect(&Self {
                low: Some(bound.negate()),
                high: Some(bound),
            })
        }

        pub fn constant_remainder(&self, modulus: &BigInt, tight_interval: bool) -> Self {
            if modulus.is_zero() {
                return Self::unbounded();
            }
            if tight_interval
                && let (Some(low), Some(high)) = (&self.low, &self.high)
                && let (Some((low_quotient, low_remainder)), Some((high_quotient, high_remainder))) =
                    (low.div_rem(modulus), high.div_rem(modulus))
                && low_quotient == high_quotient
            {
                return Self {
                    low: Some(low_remainder.clone().min(high_remainder.clone())),
                    high: Some(low_remainder.max(high_remainder)),
                };
            }
            self.remainder(&Self::constant(modulus.clone()))
        }

        pub fn shift_right(&self, low_count: u64, high_count: u64) -> Self {
            if low_count > high_count {
                return Self::unbounded();
            }
            let low = self.low.as_ref().and_then(|value| {
                floor_shift_right(value, low_count)
                    .zip(floor_shift_right(value, high_count))
                    .map(|(low, high)| low.min(high))
            });
            let high = self.high.as_ref().and_then(|value| {
                floor_shift_right(value, low_count)
                    .zip(floor_shift_right(value, high_count))
                    .map(|(low, high)| low.max(high))
            });
            Self { low, high }
        }

        pub fn bitwise_and(&self, other: &Self, bits: u32, signed: bool) -> Self {
            let (minimum, maximum) = bitwise_carrier_bounds(bits, signed);
            let left_interval = self;
            let right_interval = other;
            let nonnegative =
                |interval: &Self| !interval.high.as_ref().is_some_and(BigInt::is_negative);
            let negative =
                |interval: &Self| !interval.low.as_ref().is_some_and(|low| !low.is_negative());
            let min_bound = |left: &Option<BigInt>, right: &Option<BigInt>| match (left, right) {
                (Some(left), Some(right)) => Some(left.clone().min(right.clone())),
                (Some(bound), None) | (None, Some(bound)) => Some(bound.clone()),
                (None, None) => None,
            };
            let mut low: Option<BigInt> = None;
            let mut high: Option<BigInt> = None;
            let mut fired = false;
            let mut extend = |quadrant_low: BigInt, quadrant_high: Option<BigInt>| {
                low = Some(match &low {
                    Some(current) => current.clone().min(quadrant_low.clone()),
                    None => quadrant_low,
                });
                high = match quadrant_high {
                    Some(bound) if !fired => Some(bound),
                    Some(bound) => high.clone().map(|current| current.max(bound)),
                    None => None,
                };
                fired = true;
            };
            let (left_nonnegative, left_negative) =
                (nonnegative(left_interval), negative(left_interval));
            let (right_nonnegative, right_negative) =
                (nonnegative(right_interval), negative(right_interval));
            if left_nonnegative && right_nonnegative {
                extend(
                    BigInt::zero(),
                    min_bound(&left_interval.high, &right_interval.high),
                );
            }
            if left_nonnegative && right_negative {
                extend(BigInt::zero(), left_interval.high.clone());
            }
            if left_negative && right_nonnegative {
                extend(BigInt::zero(), right_interval.high.clone());
            }
            if left_negative && right_negative {
                extend(
                    minimum.clone(),
                    min_bound(&left_interval.high, &right_interval.high),
                );
            }
            Self {
                low: low.map(|low| low.max(minimum.clone())).or(Some(minimum)),
                high: high.map(|high| high.min(maximum.clone())).or(Some(maximum)),
            }
        }

        pub fn from_comparison(
            operator: BinaryOperator,
            subject_on_left: bool,
            value: BigInt,
        ) -> Option<Self> {
            let operator = if subject_on_left {
                operator
            } else {
                operator.mirrored()
            };
            let one = BigInt::from_i64(1);
            let (low, high) = match operator {
                BinaryOperator::LessOrEqual => (None, Some(value)),
                BinaryOperator::Less => (None, Some(value.sub(&one))),
                BinaryOperator::GreaterOrEqual => (Some(value), None),
                BinaryOperator::Greater => (Some(value.add(&one)), None),
                BinaryOperator::Equal => (Some(value.clone()), Some(value)),
                _ => return None,
            };
            Some(Self { low, high })
        }

        pub fn from_refuted_comparison(
            operator: BinaryOperator,
            subject_on_left: bool,
            value: BigInt,
        ) -> Option<Self> {
            let refuted = match operator {
                BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
                BinaryOperator::LessOrEqual => BinaryOperator::Greater,
                BinaryOperator::Greater => BinaryOperator::LessOrEqual,
                BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
                _ => return None,
            };
            Self::from_comparison(refuted, subject_on_left, value)
        }

        pub fn refine_closed(&self, range: facts::IntegerRange) -> facts::IntegerRange {
            facts::IntegerRange {
                minimum: match &self.low {
                    Some(low) => range.minimum.max(low.clone()),
                    None => range.minimum,
                },
                maximum: match &self.high {
                    Some(high) => range.maximum.min(high.clone()),
                    None => range.maximum,
                },
            }
        }

        pub fn to_interval(&self) -> Option<super::Interval> {
            let end = |end: &Option<BigInt>| match end {
                Some(value) => value.to_i64().map(Some),
                None => Some(None),
            };
            Some(super::Interval {
                low: end(&self.low)?,
                high: end(&self.high)?,
            })
        }

        pub fn from_closed(range: facts::IntegerRange) -> Self {
            Self {
                low: Some(range.minimum),
                high: Some(range.maximum),
            }
        }

        pub fn into_closed(self) -> Option<facts::IntegerRange> {
            let minimum = self.low?;
            let maximum = self.high?;
            (minimum <= maximum).then_some(facts::IntegerRange { minimum, maximum })
        }

        pub fn scale(&self, factor: &BigInt) -> Self {
            if factor.is_zero() {
                return Self::constant(BigInt::zero());
            }
            let scaled_low = self.low.as_ref().map(|value| value.mul(factor));
            let scaled_high = self.high.as_ref().map(|value| value.mul(factor));
            if factor.is_negative() {
                Self {
                    low: scaled_high,
                    high: scaled_low,
                }
            } else {
                Self {
                    low: scaled_low,
                    high: scaled_high,
                }
            }
        }

        pub fn multiply(&self, other: &Self) -> Self {
            if let (Some(low), Some(high)) = (&self.low, &self.high)
                && low == high
            {
                return other.scale(low);
            }
            if let (Some(low), Some(high)) = (&other.low, &other.high)
                && low == high
            {
                return self.scale(low);
            }
            let (Some(self_low), Some(self_high), Some(other_low), Some(other_high)) =
                (&self.low, &self.high, &other.low, &other.high)
            else {
                return Self::unbounded();
            };
            let candidates = [
                self_low.mul(other_low),
                self_low.mul(other_high),
                self_high.mul(other_low),
                self_high.mul(other_high),
            ];
            Self {
                low: candidates.iter().min().cloned(),
                high: candidates.iter().max().cloned(),
            }
        }

        pub fn correlated_power(&self, power: u32) -> Self {
            if power == 0 {
                return Self::constant(BigInt::from_i64(1));
            }
            if power == 1 {
                return self.clone();
            }
            let (Some(low), Some(high)) = (&self.low, &self.high) else {
                return if power.is_multiple_of(2) {
                    Self {
                        low: Some(BigInt::zero()),
                        high: None,
                    }
                } else {
                    Self::unbounded()
                };
            };
            let corner_low = pow(low, power);
            let corner_high = pow(high, power);
            if power % 2 == 1 {
                return Self {
                    low: Some(corner_low),
                    high: Some(corner_high),
                };
            }
            let max_corner = corner_low.clone().max(corner_high.clone());
            let min_corner = if !low.is_negative() || high.is_negative() {
                corner_low.min(corner_high)
            } else {
                BigInt::zero()
            };
            Self {
                low: Some(min_corner),
                high: Some(max_corner),
            }
        }
    }

    fn pow(base: &BigInt, power: u32) -> BigInt {
        let mut result = BigInt::from_i64(1);
        for _ in 0..power {
            result = result.mul(base);
        }
        result
    }

    pub fn floor_shift_right(value: &BigInt, count: u64) -> Option<BigInt> {
        if count >= value.bit_length() as u64 {
            return Some(if value.is_negative() {
                BigInt::from_i64(-1)
            } else {
                BigInt::zero()
            });
        }
        let divisor = BigInt::from_i64(1).shl_bits(usize::try_from(count).ok()?);
        let (quotient, remainder) = value.div_rem(&divisor)?;
        Some(if remainder.is_negative() {
            quotient.sub(&BigInt::from_i64(1))
        } else {
            quotient
        })
    }

    pub fn bitwise_carrier_bounds(bits: u32, signed: bool) -> (BigInt, BigInt) {
        if signed {
            let half = BigInt::from_i64(1).shl_bits((bits - 1) as usize);
            (half.negate(), half.sub(&BigInt::from_i64(1)))
        } else {
            (
                BigInt::zero(),
                BigInt::from_i64(1)
                    .shl_bits(bits as usize)
                    .sub(&BigInt::from_i64(1)),
            )
        }
    }

    #[cfg(test)]
    mod tests {
        use super::BigIntInterval;
        use numerics::bignum::BigInt;
        use typed_trees::expression::BinaryOperator;

        fn ends(interval: Option<BigIntInterval>) -> Option<(Option<i64>, Option<i64>)> {
            interval.map(|interval| {
                (
                    interval.low.and_then(|low| low.to_i64()),
                    interval.high.and_then(|high| high.to_i64()),
                )
            })
        }

        #[test]
        fn comparisons_bound_the_subject_on_either_side() {
            let five = || BigInt::from_i64(5);
            let read =
                |operator, left| ends(BigIntInterval::from_comparison(operator, left, five()));
            assert_eq!(read(BinaryOperator::Less, true), Some((None, Some(4))));
            assert_eq!(
                read(BinaryOperator::LessOrEqual, true),
                Some((None, Some(5)))
            );
            assert_eq!(read(BinaryOperator::Greater, true), Some((Some(6), None)));
            assert_eq!(
                read(BinaryOperator::GreaterOrEqual, true),
                Some((Some(5), None))
            );
            assert_eq!(read(BinaryOperator::Equal, true), Some((Some(5), Some(5))));
            assert_eq!(read(BinaryOperator::Less, false), Some((Some(6), None)));
            assert_eq!(read(BinaryOperator::NotEqual, true), None);
            assert_eq!(read(BinaryOperator::Add, true), None);
        }

        #[test]
        fn refuted_orderings_bound_the_complement_and_equality_does_not() {
            let five = || BigInt::from_i64(5);
            let read = |operator| {
                ends(BigIntInterval::from_refuted_comparison(
                    operator,
                    true,
                    five(),
                ))
            };
            assert_eq!(read(BinaryOperator::Less), Some((Some(5), None)));
            assert_eq!(read(BinaryOperator::LessOrEqual), Some((Some(6), None)));
            assert_eq!(read(BinaryOperator::Greater), Some((None, Some(5))));
            assert_eq!(read(BinaryOperator::GreaterOrEqual), Some((None, Some(4))));
            assert_eq!(read(BinaryOperator::Equal), None);
            assert_eq!(read(BinaryOperator::NotEqual), None);
        }

        #[test]
        fn closed_refinement_keeps_a_crossed_meet() {
            let range = facts::IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(255),
            };
            let below =
                BigIntInterval::from_comparison(BinaryOperator::Less, true, BigInt::from_i64(0))
                    .unwrap();
            let refined = below.refine_closed(range);
            assert_eq!(refined.minimum, BigInt::from_i64(0));
            assert_eq!(refined.maximum, BigInt::from_i64(-1));
        }

        #[test]
        fn machine_width_view_refuses_an_end_outside_i64() {
            let past = BigIntInterval::from_comparison(
                BinaryOperator::Less,
                true,
                BigInt::from_i64(i64::MIN),
            )
            .unwrap();
            assert_eq!(past.to_interval(), None);
            let inside = BigIntInterval::from_comparison(
                BinaryOperator::GreaterOrEqual,
                true,
                BigInt::from_i64(3),
            )
            .unwrap()
            .to_interval()
            .unwrap();
            assert_eq!((inside.low, inside.high), (Some(3), None));
        }
    }
}
pub(crate) mod cast_ranges {

    pub fn plain_exact_widening_cast(
        program: &dyn crate::CheckingInput,
        cast: &typed_trees::expression::TableCastExpression,
    ) -> bool {
        unannotated_exact_cast(cast)
            && matches!(
                program
                    .type_reference_table
                    .type_reference(cast.target_type),
                typed_trees::types::TypeReferenceNode::Named { .. }
            )
    }

    pub fn unannotated_exact_cast(cast: &typed_trees::expression::TableCastExpression) -> bool {
        !cast.form.is_recast()
            && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
            && cast.semantic_domain.is_empty()
            && cast.semantic_domain_arguments.is_empty()
            && !cast.semantic_domain_symbol.is_valid()
            && !cast.semantic_domain_id.is_valid()
            && !(cast.result_type.is_valid() && cast.result_type != cast.target_type)
    }
}

pub(crate) mod domain_intervals {
    use crate::frozen_program::expression_table;
    use typed_trees::expression::ExpressionNode;

    pub fn declared_domain_predicate_bounds(
        program: &dyn crate::CheckingInput,
        domain: &typed_trees::types::DomainConstraint,
    ) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
        let interval = predicate_interval(program, domain)?;
        if interval.minimum.is_none() && interval.maximum.is_none() {
            return None;
        }
        let declaration = program.domain_definition(domain.symbol)?;
        let primitive =
            crate::value_custody::recasts::exact_primitive_type(program, declaration.target_type)?;
        let (minimum, maximum) =
            crate::proof_contracts::proof_embeddings::primitive_range(primitive)?;
        Some((
            interval.minimum.unwrap_or(minimum),
            interval.maximum.unwrap_or(maximum),
        ))
    }

    pub fn exact_declared_domain_interval(
        program: &dyn crate::CheckingInput,
        domain: &typed_trees::types::DomainConstraint,
    ) -> Option<(
        Option<numerics::bignum::BigInt>,
        Option<numerics::bignum::BigInt>,
    )> {
        let interval = predicate_interval(program, domain)?;
        let definition = program.domain_definition(domain.symbol)?;
        (interval.exact
            && definition.establishment_routes.is_empty()
            && (interval.minimum.is_some() || interval.maximum.is_some()))
        .then_some((interval.minimum, interval.maximum))
    }

    pub fn exact_declared_domain_carrier_interval(
        program: &dyn crate::CheckingInput,
        carrier: typed_trees::types::PrimitiveType,
        domain: &typed_trees::types::DomainConstraint,
    ) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
        let (minimum, maximum) = exact_declared_domain_interval(program, domain)?;
        let definition = typed_trees::domain::domain_by_symbol(program, domain.symbol)?;
        if crate::value_custody::recasts::exact_primitive_type(program, definition.target_type)?
            != carrier
        {
            return None;
        }
        let (carrier_minimum, carrier_maximum) =
            crate::proof_contracts::proof_embeddings::primitive_range(carrier)?;
        Some((
            minimum.map_or(carrier_minimum.clone(), |minimum| {
                minimum.max(carrier_minimum)
            }),
            maximum.map_or(carrier_maximum.clone(), |maximum| {
                maximum.min(carrier_maximum)
            }),
        ))
    }

    pub(crate) struct PredicateInterval {
        minimum: Option<numerics::bignum::BigInt>,
        maximum: Option<numerics::bignum::BigInt>,
        exact: bool,
    }

    fn predicate_interval(
        program: &dyn crate::CheckingInput,
        domain: &typed_trees::types::DomainConstraint,
    ) -> Option<PredicateInterval> {
        use typed_trees::types::DomainConstraintSubject;
        if domain.subject != DomainConstraintSubject::Declared {
            return None;
        }
        application_predicate_interval(
            program,
            domain.symbol,
            &domain.arguments,
            domain.semantic_id,
        )
    }

    pub(crate) fn membership_predicate_bounds(
        program: &dyn crate::CheckingInput,
        membership: &typed_trees::domain::ProofMembershipFact,
    ) -> Option<(
        Option<numerics::bignum::BigInt>,
        Option<numerics::bignum::BigInt>,
    )> {
        let arguments = program
            .type_reference_table
            .type_reference_handles(membership.domain_arguments);
        if arguments.len() != membership.domain_arguments.len() {
            return None;
        }
        let interval = application_predicate_interval(
            program,
            membership.domain_symbol,
            arguments,
            membership.semantic_domain,
        )?;
        Some((interval.minimum, interval.maximum))
    }

    fn application_predicate_interval(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        arguments: &[typed_trees::types::TypeReferenceHandle],
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> Option<PredicateInterval> {
        let definition = program.domain_definition(symbol)?;
        if definition.alias.is_some() {
            return None;
        }
        let indices = crate::proof_contracts::domains::closed_integer_index_bindings(
            program,
            definition,
            arguments,
            semantic_domain,
        )?;
        let mut interval = PredicateInterval {
            minimum: None,
            maximum: None,
            exact: true,
        };
        for fact in program.proof_facts(definition) {
            let Some(expression) = fact.as_expression() else {
                interval.exact = false;
                continue;
            };
            interval.exact &= self_predicate_bounds(
                program,
                definition,
                *expression,
                &indices,
                &mut |minimum, maximum| {
                    if let Some(minimum) = minimum {
                        interval.minimum = Some(match interval.minimum.take() {
                            Some(prior) => minimum.max(prior),
                            None => minimum,
                        });
                    }
                    if let Some(maximum) = maximum {
                        interval.maximum = Some(match interval.maximum.take() {
                            Some(prior) => maximum.min(prior),
                            None => maximum,
                        });
                    }
                },
            );
        }
        Some(interval)
    }

    fn self_predicate_bounds(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::domain::DomainDefinition,
        expression: typed_trees::expression::ExpressionHandle,
        indices: &[(symbols::SymbolHandle, numerics::bignum::BigInt)],
        bound: &mut impl FnMut(Option<numerics::bignum::BigInt>, Option<numerics::bignum::BigInt>),
    ) -> bool {
        use typed_trees::expression::BinaryOperator;
        if !crate::has_builtin_domain_decomposed_guard_meaning(program, definition, expression) {
            return false;
        }
        let ExpressionNode::Binary(binary) = expression_table(program).expression(expression)
        else {
            return false;
        };
        if binary.operator == BinaryOperator::And {
            let left = self_predicate_bounds(program, definition, binary.left, indices, bound);
            let right = self_predicate_bounds(program, definition, binary.right, indices, bound);
            return left && right;
        }
        let left_is_self =
            crate::exact_domain_subject_type(program, definition, binary.left).is_some();
        let right_is_self =
            crate::exact_domain_subject_type(program, definition, binary.right).is_some();
        let (literal, subject_on_left) = match (left_is_self, right_is_self) {
            (true, false) => (binary.right, true),
            (false, true) => (binary.left, false),
            _ => return false,
        };
        let Some(value) = crate::closed_integer_range_bound(program, literal).or_else(|| {
            let ExpressionNode::Name(path) = expression_table(program).expression(literal) else {
                return None;
            };
            (expression_table(program)
                .name_path_members(path.members)
                .len()
                == 1)
                .then(|| indices.iter().find(|(symbol, _)| *symbol == path.symbol))
                .flatten()
                .map(|(_, value)| value.clone())
        }) else {
            return false;
        };
        let Some(interval) =
            super::BigIntInterval::from_comparison(binary.operator, subject_on_left, value)
        else {
            return false;
        };
        bound(interval.low, interval.high);
        true
    }
}

pub(crate) mod integer_carriers {
    use numerics::bignum::BigInt;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use typed_trees::types::PrimitiveType;

    pub(crate) fn integer_type(primitive: PrimitiveType) -> Option<IntegerType> {
        let (sign, bits) = primitive.integer_sign_bits()?;
        IntegerType::new(sign, bits).ok()
    }

    pub(crate) fn admitted_integer(
        scalar_type: IntegerType,
        value: &BigInt,
    ) -> Option<IntegerValue> {
        let value = match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(value.to_i64()?)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value.to_u64()?)),
        };
        scalar_type.admits(value).then_some(value)
    }

    pub(crate) fn integer_magnitude(value: IntegerValue) -> BigInt {
        match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        }
    }
}
pub(crate) mod integer_ranges {
    use super::Interval;
    use typed_trees::types::PrimitiveType;

    pub fn integer_widen_is_total(source: PrimitiveType, target: PrimitiveType) -> bool {
        fn shape(primitive: PrimitiveType) -> Option<(bool, u8)> {
            primitive
                .integer_sign_bits()
                .map(|(sign, bits)| (sign == semantic_vocabulary::IntegerSign::Signed, bits as u8))
        }
        let Some((source_signed, source_bits)) = shape(source) else {
            return false;
        };
        let Some((target_signed, target_bits)) = shape(target) else {
            return false;
        };
        source_bits < target_bits && (!source_signed || target_signed)
    }

    pub(crate) fn primitive_range(primitive: PrimitiveType) -> Option<Interval> {
        let (low, high): (Option<i64>, Option<i64>) = match primitive {
            PrimitiveType::I8 => (Some(i8::MIN as i64), Some(i8::MAX as i64)),
            PrimitiveType::U8 => (Some(0), Some(u8::MAX as i64)),
            PrimitiveType::I16 => (Some(i16::MIN as i64), Some(i16::MAX as i64)),
            PrimitiveType::U16 => (Some(0), Some(u16::MAX as i64)),
            PrimitiveType::I32 => (Some(i32::MIN as i64), Some(i32::MAX as i64)),
            PrimitiveType::U32 => (Some(0), Some(u32::MAX as i64)),
            PrimitiveType::I64 => (Some(i64::MIN), Some(i64::MAX)),
            PrimitiveType::U64 | PrimitiveType::Addr => (Some(0), None),
            PrimitiveType::Selector
            | PrimitiveType::Bool
            | PrimitiveType::F32
            | PrimitiveType::F64 => {
                return None;
            }
        };
        Some(Interval { low, high })
    }

    pub(crate) fn integer_bit_width(primitive: PrimitiveType) -> Option<i64> {
        match primitive {
            PrimitiveType::I8 | PrimitiveType::U8 => Some(8),
            PrimitiveType::I16 | PrimitiveType::U16 => Some(16),
            PrimitiveType::I32 | PrimitiveType::U32 => Some(32),
            PrimitiveType::I64 | PrimitiveType::U64 => Some(64),
            PrimitiveType::Addr
            | PrimitiveType::Selector
            | PrimitiveType::Bool
            | PrimitiveType::F32
            | PrimitiveType::F64 => None,
        }
    }
}

pub(crate) use integer_carriers::{admitted_integer, integer_magnitude, integer_type};
pub(crate) mod expression_intervals {
    use facts::IntegerRange;
    use numerics::bignum::BigInt;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::PrimitiveType;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ExpressionInterval {
        pub expression: ExpressionHandle,
        pub low: Option<i64>,
        pub high: Option<i64>,
    }

    impl ExpressionInterval {
        pub fn carrier_range(&self, primitive: PrimitiveType) -> Option<IntegerRange> {
            let mut range = super::carrier_integer_range(primitive)?;
            if let Some(low) = self.low {
                range.minimum = range.minimum.max(BigInt::from_i64(low));
            }
            if let Some(high) = self.high {
                range.maximum = range.maximum.min(BigInt::from_i64(high));
            }
            (range.minimum <= range.maximum).then_some(range)
        }
    }

    pub fn expression_interval(
        intervals: &[ExpressionInterval],
        expression: ExpressionHandle,
    ) -> Option<&ExpressionInterval> {
        let position = checked_trees::operators::handle_position(expression);
        intervals
            .binary_search_by_key(&position, |interval| {
                checked_trees::operators::handle_position(interval.expression)
            })
            .ok()
            .map(|found| &intervals[found])
    }

    #[derive(Debug, Clone)]
    pub struct PredicateInterval {
        pub argument: ExpressionHandle,
        pub extent: bool,
        pub low: i128,
        pub high: i128,
    }
}
pub(crate) mod interval {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Interval {
        pub low: Option<i64>,
        pub high: Option<i64>,
    }

    impl Interval {
        pub const UNBOUNDED: Interval = Interval {
            low: None,
            high: None,
        };

        pub fn low(&self) -> Option<i64> {
            self.low
        }

        pub fn high(&self) -> Option<i64> {
            self.high
        }

        pub fn constant(value: i64) -> Self {
            Self {
                low: Some(value),
                high: Some(value),
            }
        }

        pub(crate) fn excludes_zero(self) -> bool {
            self.low.is_some_and(|low| low >= 1) || self.high.is_some_and(|high| high <= -1)
        }

        pub(crate) fn is_exactly_zero(self) -> bool {
            self.low == Some(0) && self.high == Some(0)
        }

        pub(crate) fn add(self, other: Self) -> Self {
            Self {
                low: pair(self.low, other.low, i64::checked_add),
                high: pair(self.high, other.high, i64::checked_add),
            }
        }

        pub(crate) fn subtract(self, other: Self) -> Self {
            Self {
                low: pair(self.low, other.high, i64::checked_sub),
                high: pair(self.high, other.low, i64::checked_sub),
            }
        }

        pub(crate) fn multiply(self, other: Self) -> Self {
            if self.is_exactly_zero() || other.is_exactly_zero() {
                return Self::constant(0);
            }
            let (Some(a), Some(b), Some(c), Some(d)) = (self.low, self.high, other.low, other.high)
            else {
                return Interval::UNBOUNDED;
            };
            let corners = [
                a.checked_mul(c),
                a.checked_mul(d),
                b.checked_mul(c),
                b.checked_mul(d),
            ];
            if corners.iter().any(Option::is_none) {
                return Interval::UNBOUNDED;
            }
            let values: Vec<i64> = corners.into_iter().flatten().collect();
            Self {
                low: values.iter().min().copied(),
                high: values.iter().max().copied(),
            }
        }

        pub(crate) fn shift_left(self, count: Self) -> Self {
            let (Some(value_low), Some(value_high), Some(count_low), Some(count_high)) =
                (self.low, self.high, count.low, count.high)
            else {
                return Interval::UNBOUNDED;
            };
            let (Ok(count_low), Ok(count_high)) =
                (u32::try_from(count_low), u32::try_from(count_high))
            else {
                return Interval::UNBOUNDED;
            };
            let (Some(low_factor), Some(high_factor)) = (
                1_i128.checked_shl(count_low),
                1_i128.checked_shl(count_high),
            ) else {
                return Interval::UNBOUNDED;
            };
            let corners = [
                i128::from(value_low).checked_mul(low_factor),
                i128::from(value_low).checked_mul(high_factor),
                i128::from(value_high).checked_mul(low_factor),
                i128::from(value_high).checked_mul(high_factor),
            ];
            if corners.iter().any(Option::is_none) {
                return Interval::UNBOUNDED;
            }
            let values = corners.into_iter().flatten().collect::<Vec<_>>();
            let low = *values.iter().min().expect("four exact shift corners exist");
            let high = *values.iter().max().expect("four exact shift corners exist");
            let (Ok(low), Ok(high)) = (i64::try_from(low), i64::try_from(high)) else {
                return Interval::UNBOUNDED;
            };
            Self {
                low: Some(low),
                high: Some(high),
            }
        }

        pub(crate) fn shift_right(self, count: Self) -> Self {
            let (Some(value_low), Some(value_high), Some(count_low), Some(count_high)) =
                (self.low, self.high, count.low, count.high)
            else {
                return Interval::UNBOUNDED;
            };
            let (Ok(count_low), Ok(count_high)) =
                (u32::try_from(count_low), u32::try_from(count_high))
            else {
                return Interval::UNBOUNDED;
            };
            if count_high >= i64::BITS {
                return Interval::UNBOUNDED;
            }
            let values = [
                value_low >> count_low,
                value_low >> count_high,
                value_high >> count_low,
                value_high >> count_high,
            ];
            Self {
                low: values.iter().min().copied(),
                high: values.iter().max().copied(),
            }
        }

        pub(crate) fn modulo(self, divisor: Self) -> Self {
            if !divisor.excludes_zero() {
                return Interval::UNBOUNDED;
            }
            let dividend_bound = self.union(Self::constant(0));
            let Some(magnitude) = divisor.nonzero_magnitude_bound() else {
                return dividend_bound;
            };
            let Some(bound) = magnitude
                .checked_sub(1)
                .and_then(|bound| i64::try_from(bound).ok())
            else {
                return Interval::UNBOUNDED;
            };
            dividend_bound.intersect(Self {
                low: Some(-bound),
                high: Some(bound),
            })
        }

        pub(crate) fn divide(self, divisor: Self) -> Self {
            let (Some(low), Some(high)) = (self.low, self.high) else {
                return Interval::UNBOUNDED;
            };
            if low > high
                || divisor
                    .low
                    .zip(divisor.high)
                    .is_some_and(|(low, high)| low > high)
                || !divisor.excludes_zero()
            {
                return Interval::UNBOUNDED;
            }
            let quotient = |value: i64, endpoint: Option<i64>| match endpoint {
                Some(endpoint) => value.checked_div(endpoint),
                None => Some(0),
            };
            let [Some(first), Some(second), Some(third), Some(fourth)] = [
                quotient(low, divisor.low),
                quotient(low, divisor.high),
                quotient(high, divisor.low),
                quotient(high, divisor.high),
            ] else {
                return Interval::UNBOUNDED;
            };
            Self {
                low: Some(first.min(second).min(third).min(fourth)),
                high: Some(first.max(second).max(third).max(fourth)),
            }
        }

        pub(crate) fn min_with(self, other: Self) -> Self {
            Self {
                low: self.union(other).low,
                high: self.intersect(other).high,
            }
        }

        pub(crate) fn max_with(self, other: Self) -> Self {
            Self {
                low: self.intersect(other).low,
                high: self.union(other).high,
            }
        }

        pub(crate) fn nonzero_magnitude_bound(self) -> Option<u64> {
            let (low, high) = (self.low?, self.high?);
            if low >= 1 || high <= -1 {
                Some(low.unsigned_abs().max(high.unsigned_abs()))
            } else {
                None
            }
        }

        pub fn union(self, other: Self) -> Self {
            Self {
                low: match (self.low, other.low) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    _ => None,
                },
                high: match (self.high, other.high) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                },
            }
        }

        pub fn intersect(self, other: Self) -> Self {
            Self {
                low: match (self.low, other.low) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(v), None) | (None, Some(v)) => Some(v),
                    (None, None) => None,
                },
                high: match (self.high, other.high) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(v), None) | (None, Some(v)) => Some(v),
                    (None, None) => None,
                },
            }
        }

        pub(crate) fn contains(self, inner: Interval) -> bool {
            self.intersect(inner) == inner
        }
    }

    fn pair(left: Option<i64>, right: Option<i64>, op: fn(i64, i64) -> Option<i64>) -> Option<i64> {
        left.zip(right).and_then(|(left, right)| op(left, right))
    }
}
pub use expression_intervals::{ExpressionInterval, PredicateInterval, expression_interval};
pub(crate) mod integer_sets {
    use typed_trees::expression::BinaryOperator;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct IntegerSet {
        ranges: Vec<(i128, i128)>,
    }

    impl IntegerSet {
        pub(crate) fn empty() -> Self {
            Self { ranges: Vec::new() }
        }

        pub(crate) fn range(low: i128, high: i128) -> Self {
            if low > high {
                return Self::empty();
            }
            Self {
                ranges: vec![(low, high)],
            }
        }

        pub(crate) fn singleton(value: i128) -> Self {
            Self::range(value, value)
        }

        pub(crate) fn is_empty(&self) -> bool {
            self.ranges.is_empty()
        }

        pub(crate) fn single_value(&self) -> Option<i128> {
            match self.ranges.as_slice() {
                [(low, high)] if low == high => Some(*low),
                _ => None,
            }
        }

        pub(crate) fn bounds(&self) -> Option<(i128, i128)> {
            Some((self.ranges.first()?.0, self.ranges.last()?.1))
        }

        pub(crate) fn interval_hull(&self) -> Option<Self> {
            let (low, high) = self.bounds()?;
            Some(Self::range(low, high))
        }

        pub(crate) fn union(&self, other: &Self) -> Self {
            let mut ranges = self.ranges.clone();
            ranges.extend_from_slice(&other.ranges);
            Self::normalized(ranges)
        }

        pub(crate) fn intersect(&self, other: &Self) -> Self {
            let mut ranges = Vec::new();
            for (left_low, left_high) in &self.ranges {
                for (right_low, right_high) in &other.ranges {
                    let low = (*left_low).max(*right_low);
                    let high = (*left_high).min(*right_high);
                    if low <= high {
                        ranges.push((low, high));
                    }
                }
            }
            Self::normalized(ranges)
        }

        pub(crate) fn difference(&self, other: &Self) -> Self {
            let mut remaining = self.ranges.clone();
            for (cut_low, cut_high) in &other.ranges {
                let mut next = Vec::with_capacity(remaining.len() + 1);
                for (low, high) in remaining {
                    if high < *cut_low || *cut_high < low {
                        next.push((low, high));
                        continue;
                    }
                    if low < *cut_low {
                        next.push((low, cut_low - 1));
                    }
                    if *cut_high < high {
                        next.push((cut_high + 1, high));
                    }
                }
                remaining = next;
            }
            Self::normalized(remaining)
        }

        pub(crate) fn is_subset_of(&self, other: &Self) -> bool {
            self.difference(other).is_empty()
        }

        pub(crate) fn satisfying(operator: BinaryOperator, bound: i128) -> Option<Self> {
            Some(match operator {
                BinaryOperator::Equal => Self::singleton(bound),
                BinaryOperator::NotEqual => {
                    Self::range(i128::MIN, i128::MAX).difference(&Self::singleton(bound))
                }
                BinaryOperator::Less => match bound.checked_sub(1) {
                    Some(high) => Self::range(i128::MIN, high),
                    None => Self::empty(),
                },
                BinaryOperator::LessOrEqual => Self::range(i128::MIN, bound),
                BinaryOperator::Greater => match bound.checked_add(1) {
                    Some(low) => Self::range(low, i128::MAX),
                    None => Self::empty(),
                },
                BinaryOperator::GreaterOrEqual => Self::range(bound, i128::MAX),
                _ => return None,
            })
        }

        pub(crate) fn range_count(&self) -> usize {
            self.ranges.len()
        }

        fn normalized(mut ranges: Vec<(i128, i128)>) -> Self {
            ranges.retain(|(low, high)| low <= high);
            ranges.sort_unstable();
            let mut merged: Vec<(i128, i128)> = Vec::with_capacity(ranges.len());
            for (low, high) in ranges {
                if let Some(last) = merged.last_mut()
                    && low <= last.1.saturating_add(1)
                {
                    last.1 = last.1.max(high);
                    continue;
                }
                merged.push((low, high));
            }
            Self { ranges: merged }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn integer_interval_unions_preserve_holes_and_carrier_edges() {
            let separated = IntegerSet::singleton(0).union(&IntegerSet::singleton(2));
            assert_eq!(separated.bounds(), Some((0, 2)));
            assert!(separated.is_subset_of(&IntegerSet::range(0, 2)));
            assert!(!IntegerSet::range(0, 2).is_subset_of(&separated));
            assert!(separated.intersect(&IntegerSet::singleton(1)).is_empty());
            let hull = separated.interval_hull().unwrap();
            assert!(separated.is_subset_of(&hull));
            assert!(IntegerSet::singleton(1).is_subset_of(&hull));
            assert_eq!(IntegerSet::empty().interval_hull(), None);
            assert_eq!(
                IntegerSet::range(-1, 1).difference(&IntegerSet::singleton(0)),
                IntegerSet::singleton(-1).union(&IntegerSet::singleton(1))
            );
            let universe = IntegerSet::range(i128::MIN, i128::MAX);
            let without_minimum =
                IntegerSet::satisfying(BinaryOperator::NotEqual, i128::MIN).unwrap();
            assert!(
                without_minimum
                    .intersect(&IntegerSet::singleton(i128::MIN))
                    .is_empty()
            );
            assert_eq!(
                universe.difference(&without_minimum),
                IntegerSet::singleton(i128::MIN)
            );
        }
    }
}
pub(crate) use integer_sets::IntegerSet;
pub(crate) mod integer_operations {
    use super::BigIntInterval;
    use facts::IntegerRange;
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;

    pub fn integer_binary_range(
        operator: BinaryOperator,
        left: IntegerRange,
        right: IntegerRange,
    ) -> Option<IntegerRange> {
        if left.minimum > left.maximum || right.minimum > right.maximum {
            return None;
        }
        let first = BigIntInterval::from_closed(left.clone());
        let second = BigIntInterval::from_closed(right.clone());
        match operator {
            BinaryOperator::Add => first.add(&second).into_closed(),
            BinaryOperator::Subtract => first.subtract(&second).into_closed(),
            BinaryOperator::Multiply => first.multiply(&second).into_closed(),
            BinaryOperator::Divide if second.excludes_zero() => first.divide(&second).into_closed(),
            BinaryOperator::Modulo if second.excludes_zero() => {
                first.remainder(&second).into_closed()
            }
            BinaryOperator::ShiftRight => first
                .shift_right(right.minimum.to_u64()?, right.maximum.to_u64()?)
                .into_closed(),
            BinaryOperator::BitwiseAnd
                if !left.minimum.is_negative() && !right.minimum.is_negative() =>
            {
                Some(IntegerRange {
                    minimum: BigInt::zero(),
                    maximum: left.maximum.min(right.maximum),
                })
            }
            BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor
                if !left.minimum.is_negative() && !right.minimum.is_negative() =>
            {
                let bits = left.maximum.clone().max(right.maximum).bit_length();
                Some(IntegerRange {
                    minimum: if operator == BinaryOperator::BitwiseOr {
                        left.minimum.max(right.minimum)
                    } else {
                        BigInt::zero()
                    },
                    maximum: BigInt::from_i64(1).shl_bits(bits).sub(&BigInt::from_i64(1)),
                })
            }
            _ => None,
        }
    }

    pub fn folded_integer_binary(left: i64, operator: BinaryOperator, right: i64) -> Option<i64> {
        if !matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
        ) {
            return None;
        }
        let point = |value| IntegerRange {
            minimum: BigInt::from_i64(value),
            maximum: BigInt::from_i64(value),
        };
        let left_value = BigInt::from_i64(left);
        let right_value = BigInt::from_i64(right);
        if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo) {
            let (quotient, remainder) = left_value.div_rem(&right_value)?;
            quotient.to_i64()?;
            return if operator == BinaryOperator::Divide {
                quotient.to_i64()
            } else {
                remainder.to_i64()
            };
        }
        let result = integer_binary_range(operator, point(left), point(right))?;
        (result.minimum == result.maximum)
            .then(|| result.minimum.to_i64())
            .flatten()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn closed(low: i64, high: i64) -> IntegerRange {
            IntegerRange {
                minimum: BigInt::from_i64(low),
                maximum: BigInt::from_i64(high),
            }
        }

        #[test]
        fn integer_interval_operations_enclose_small_models() {
            for low in -3..=3 {
                for high in low..=3 {
                    for other_low in -3..=3 {
                        for other_high in other_low..=3 {
                            for operator in [
                                BinaryOperator::Add,
                                BinaryOperator::Subtract,
                                BinaryOperator::Multiply,
                                BinaryOperator::Divide,
                                BinaryOperator::Modulo,
                            ] {
                                if matches!(
                                    operator,
                                    BinaryOperator::Divide | BinaryOperator::Modulo
                                ) && other_low <= 0
                                    && other_high >= 0
                                {
                                    assert_eq!(
                                        integer_binary_range(
                                            operator,
                                            closed(low, high),
                                            closed(other_low, other_high)
                                        ),
                                        None
                                    );
                                    continue;
                                }
                                let bounds = integer_binary_range(
                                    operator,
                                    closed(low, high),
                                    closed(other_low, other_high),
                                )
                                .unwrap();
                                for left in low..=high {
                                    for right in other_low..=other_high {
                                        let value = match operator {
                                            BinaryOperator::Add => left + right,
                                            BinaryOperator::Subtract => left - right,
                                            BinaryOperator::Multiply => left * right,
                                            BinaryOperator::Divide => left / right,
                                            BinaryOperator::Modulo => left % right,
                                            _ => unreachable!("enumerated arithmetic operator"),
                                        };
                                        let value = BigInt::from_i64(value);
                                        assert!(
                                            bounds.minimum <= value && value <= bounds.maximum,
                                            "{operator:?} over [{low},{high}] and [{other_low},{other_high}] excludes {value:?}"
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
        fn integer_intervals_preserve_full_width_and_unbounded_edges() {
            let maximum = BigInt::from_u64(u64::MAX);
            let full_unsigned = BigIntInterval {
                low: Some(BigInt::zero()),
                high: Some(maximum.clone()),
            };
            assert_eq!(
                full_unsigned.divide(&BigIntInterval::constant(BigInt::from_i64(1))),
                full_unsigned
            );
            assert_eq!(
                full_unsigned
                    .divide(&BigIntInterval::constant(BigInt::from_i64(2)))
                    .high,
                Some(BigInt::from_u64(u64::MAX / 2))
            );
            assert_eq!(
                BigIntInterval::unbounded().multiply(&BigIntInterval::constant(BigInt::zero())),
                BigIntInterval::constant(BigInt::zero())
            );

            let signed = BigIntInterval::from_closed(closed(i64::MIN, i64::MAX));
            let remainder = signed.remainder(&BigIntInterval::constant(BigInt::from_i64(i64::MIN)));
            assert_eq!(remainder.into_closed(), Some(closed(-i64::MAX, i64::MAX)));

            assert_eq!(
                integer_binary_range(BinaryOperator::ShiftRight, closed(-3, -3), closed(1, 1)),
                Some(closed(-2, -2))
            );
            assert_eq!(
                super::super::floor_shift_right(&BigInt::from_i64(-3), u64::MAX),
                Some(BigInt::from_i64(-1))
            );
            assert_eq!(
                super::super::floor_shift_right(&maximum, u64::MAX),
                Some(BigInt::zero())
            );
            assert_eq!(
                folded_integer_binary(i64::MIN, BinaryOperator::Divide, -1),
                None
            );
            assert_eq!(
                folded_integer_binary(i64::MIN, BinaryOperator::Modulo, -1),
                None
            );
            assert_eq!(folded_integer_binary(1, BinaryOperator::Divide, 0), None);
        }
    }
}
pub use integer_operations::{folded_integer_binary, integer_binary_range};
pub(crate) mod declared_type_bounds {
    use super::integer_ranges::primitive_range;
    use super::{ArithmeticDomain, PrimitiveType, TypeReferenceHandle, enforced_declared_range};
    use crate::CheckingInput;

    pub fn enforced_integer_type_bounds(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(i64, i64)> {
        let primitive = exact_integer_primitive(program, type_reference)?;
        let carrier = primitive_range(primitive)?;
        let interval = enforced_declared_range(program, type_reference)
            .map_or(carrier, |range| range.intersect(carrier));
        let (low, high) = (interval.low?, interval.high?);
        (low <= high).then_some((low, high))
    }

    fn exact_integer_primitive(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        let carrier_type = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        let symbol = &program
            .type_reference_table
            .type_reference(carrier_type)
            .as_named_symbol()?;
        let primitive =
            PrimitiveType::from_builtin_atom(program.symbols.builtin_type_atom(*symbol)?)?;
        if program.arithmetic_domain_for_type_reference(type_reference) != ArithmeticDomain::Exact {
            return None;
        }
        Some(primitive)
    }
}
pub use declared_type_bounds::enforced_integer_type_bounds;
pub(crate) mod range_constraints {
    use super::Interval;
    use crate::CheckingInput;
    use typed_trees::types::{
        PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) fn range_constraint_interval(
        program: &dyn CheckingInput,
        handle: TypeReferenceHandle,
    ) -> Option<Interval> {
        if floating_range_carrier(program, handle) {
            return None;
        }
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => {
                range_constraint_interval(program, *referee)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) => {
                        super::domain_intervals::declared_domain_predicate_bounds(program, domain)
                            .map(|(minimum, maximum)| Interval {
                                low: minimum.to_i64(),
                                high: maximum.to_i64(),
                            })
                    }
                    _ => None,
                })
                .chain(range_constraint_interval(program, *base_type))
                .reduce(Interval::intersect),
            _ => None,
        }
    }

    fn floating_range_carrier(program: &dyn CheckingInput, reference: TypeReferenceHandle) -> bool {
        crate::unwrapped_type_reference(program, reference)
            .and_then(|carrier| program.primitive_type_reference(carrier))
            .is_some_and(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))
    }
}

pub(crate) mod return_ranges {
    use typed_trees::expression::TableCallExpression;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) fn call_return_type(
        program: &dyn crate::CheckingInput,
        call: &TableCallExpression,
    ) -> Option<TypeReferenceHandle> {
        if crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call) {
            return crate::proof_contracts::proof_embeddings::proof_int_type_reference(program);
        }
        if let Some(operator) = typed_trees::operator::resolve_named_expression_call(program, call)
            && operator.return_type.is_valid()
        {
            return Some(operator.return_type);
        }

        crate::machine_calls::calls::resolved_call_result_type(program, call)
    }
}

pub(crate) mod selected_scalar_intervals {
    use super::{admitted_integer, integer_magnitude, integer_type};
    use checked_trees::CheckedIntegerBinaryKind;
    use facts::IntegerRange;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;
    use typed_trees::types::PrimitiveType;

    pub(crate) fn binary(
        kind: CheckedIntegerBinaryKind,
        primitive: PrimitiveType,
        right_primitive: PrimitiveType,
        left: IntegerRange,
        right: IntegerRange,
    ) -> Option<IntegerRange> {
        use CheckedIntegerBinaryKind as Kind;
        if matches!(kind, Kind::ExactShiftLeft | Kind::ExactShiftRight) {
            let carrier = integer_type(primitive)?;
            let count_type = integer_type(right_primitive)?;
            let value = (
                admitted_integer(carrier, &left.minimum)?,
                admitted_integer(carrier, &left.maximum)?,
            );
            let count = (
                admitted_integer(count_type, &right.minimum)?,
                admitted_integer(count_type, &right.maximum)?,
            );
            let (low, high) = match kind {
                Kind::ExactShiftLeft => carrier.exact_shift_left_bounds(value, count_type, count),
                Kind::ExactShiftRight => carrier.exact_shift_right_bounds(value, count_type, count),
                _ => None,
            }?;
            return Some(IntegerRange {
                minimum: integer_magnitude(low),
                maximum: integer_magnitude(high),
            });
        }
        if matches!(kind, Kind::BitwiseAnd | Kind::BitwiseOr | Kind::BitwiseXor) {
            let carrier = integer_type(primitive)?;
            let left = (
                admitted_integer(carrier, &left.minimum)?,
                admitted_integer(carrier, &left.maximum)?,
            );
            let right = (
                admitted_integer(carrier, &right.minimum)?,
                admitted_integer(carrier, &right.maximum)?,
            );
            let (low, high) = match kind {
                Kind::BitwiseAnd => carrier.bitwise_and_bounds(left, right),
                Kind::BitwiseOr => carrier.bitwise_or_bounds(left, right),
                Kind::BitwiseXor => carrier.bitwise_xor_bounds(left, right),
                _ => None,
            }?;
            return Some(IntegerRange {
                minimum: integer_magnitude(low),
                maximum: integer_magnitude(high),
            });
        }
        if matches!(
            kind,
            Kind::ExactRemainder | Kind::WrappingRemainder | Kind::SaturatingRemainder
        ) {
            return remainder(&left, &right);
        }
        if matches!(
            kind,
            Kind::ExactDivide | Kind::WrappingDivide | Kind::SaturatingDivide
        ) {
            return quotient(&left, &right);
        }
        let operator = match kind {
            Kind::ExactAdd | Kind::WrappingAdd | Kind::SaturatingAdd => BinaryOperator::Add,
            Kind::ExactSubtract | Kind::WrappingSubtract | Kind::SaturatingSubtract => {
                BinaryOperator::Subtract
            }
            Kind::ExactMultiply | Kind::WrappingMultiply | Kind::SaturatingMultiply => {
                BinaryOperator::Multiply
            }
            _ => return None,
        };
        let mut bounds = super::integer_binary_range(operator, left, right)?;
        let carrier = carrier_integer_range(primitive)?;
        match kind {
            Kind::ExactAdd | Kind::ExactSubtract | Kind::ExactMultiply => {
                integer_range_contains(&carrier, &bounds).then_some(bounds)
            }
            Kind::WrappingAdd | Kind::WrappingSubtract | Kind::WrappingMultiply => {
                let modulus = carrier
                    .maximum
                    .sub(&carrier.minimum)
                    .add(&BigInt::from_u64(1));
                let reduce = |value: &BigInt| {
                    let shifted = value.sub(&carrier.minimum);
                    let (quotient, remainder) = shifted.div_rem(&modulus)?;
                    Some(if remainder.is_negative() {
                        (quotient.sub(&BigInt::from_u64(1)), remainder.add(&modulus))
                    } else {
                        (quotient, remainder)
                    })
                };
                let (low_period, low) = reduce(&bounds.minimum)?;
                let (high_period, high) = reduce(&bounds.maximum)?;
                Some(if low_period == high_period {
                    IntegerRange {
                        minimum: low.add(&carrier.minimum),
                        maximum: high.add(&carrier.minimum),
                    }
                } else {
                    carrier
                })
            }
            Kind::SaturatingAdd | Kind::SaturatingSubtract | Kind::SaturatingMultiply => {
                bounds.minimum = bounds
                    .minimum
                    .max(carrier.minimum.clone())
                    .min(carrier.maximum.clone());
                bounds.maximum = bounds.maximum.max(carrier.minimum).min(carrier.maximum);
                Some(bounds)
            }
            _ => None,
        }
    }

    fn excludes_zero(divisor: &IntegerRange) -> bool {
        divisor.minimum >= BigInt::from_u64(1) || divisor.maximum <= BigInt::from_i64(-1)
    }

    pub fn checked_integer_binary_kind(
        operator: BinaryOperator,
        domain: ArithmeticDomain,
    ) -> Option<CheckedIntegerBinaryKind> {
        match (operator, domain) {
            (BinaryOperator::BitwiseAnd, _) => Some(CheckedIntegerBinaryKind::BitwiseAnd),
            (BinaryOperator::BitwiseOr, _) => Some(CheckedIntegerBinaryKind::BitwiseOr),
            (BinaryOperator::BitwiseXor, _) => Some(CheckedIntegerBinaryKind::BitwiseXor),
            (BinaryOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingShiftLeft)
            }
            (BinaryOperator::ShiftRight, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingShiftRight)
            }
            (BinaryOperator::ShiftRight, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactShiftRight)
            }
            (BinaryOperator::ShiftLeft, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactShiftLeft)
            }
            (BinaryOperator::ShiftLeft, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingShiftLeft)
            }
            (BinaryOperator::ShiftRight, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingShiftRight)
            }
            (BinaryOperator::ShiftLeft, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingShiftLeft)
            }
            (BinaryOperator::ShiftRight, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingShiftRight)
            }
            (BinaryOperator::Add, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingAdd)
            }
            (BinaryOperator::Subtract, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingSubtract)
            }
            (BinaryOperator::Multiply, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingMultiply)
            }
            (BinaryOperator::Divide, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingDivide)
            }
            (BinaryOperator::Modulo, ArithmeticDomain::Trapping) => {
                Some(CheckedIntegerBinaryKind::TrappingRemainder)
            }
            (BinaryOperator::Add, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactAdd)
            }
            (BinaryOperator::Subtract, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactSubtract)
            }
            (BinaryOperator::Multiply, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactMultiply)
            }
            (BinaryOperator::Divide, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactDivide)
            }
            (BinaryOperator::Modulo, ArithmeticDomain::Exact) => {
                Some(CheckedIntegerBinaryKind::ExactRemainder)
            }
            (BinaryOperator::Divide, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingDivide)
            }
            (BinaryOperator::Modulo, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingRemainder)
            }
            (BinaryOperator::Divide, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingDivide)
            }
            (BinaryOperator::Modulo, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingRemainder)
            }
            (BinaryOperator::Add, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingAdd)
            }
            (BinaryOperator::Add, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingAdd)
            }
            (BinaryOperator::Subtract, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingSubtract)
            }
            (BinaryOperator::Subtract, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingSubtract)
            }
            (BinaryOperator::Multiply, ArithmeticDomain::Wrapping) => {
                Some(CheckedIntegerBinaryKind::WrappingMultiply)
            }
            (BinaryOperator::Multiply, ArithmeticDomain::Saturating) => {
                Some(CheckedIntegerBinaryKind::SaturatingMultiply)
            }
            _ => None,
        }
    }

    fn remainder(dividend: &IntegerRange, divisor: &IntegerRange) -> Option<IntegerRange> {
        if !excludes_zero(divisor) {
            return None;
        }
        super::BigIntInterval::from_closed(dividend.clone())
            .remainder(&super::BigIntInterval::from_closed(divisor.clone()))
            .into_closed()
    }

    fn quotient(dividend: &IntegerRange, divisor: &IntegerRange) -> Option<IntegerRange> {
        if !excludes_zero(divisor) {
            return None;
        }
        super::BigIntInterval::from_closed(dividend.clone())
            .divide(&super::BigIntInterval::from_closed(divisor.clone()))
            .into_closed()
    }

    pub fn integer_range_contains(outer: &IntegerRange, inner: &IntegerRange) -> bool {
        outer.minimum <= inner.minimum
            && inner.minimum <= inner.maximum
            && inner.maximum <= outer.maximum
    }

    pub fn carrier_integer_range(primitive: PrimitiveType) -> Option<IntegerRange> {
        let (minimum, maximum) = primitive.integer_bounds()?;
        Some(IntegerRange {
            minimum: BigInt::from_i128(minimum),
            maximum: BigInt::from_i128(maximum),
        })
    }
}

pub use bigint_interval::{BigIntInterval, bitwise_carrier_bounds, floor_shift_right};
pub use interval::Interval;
pub(crate) use return_ranges::call_return_type;
pub(crate) use selected_scalar_intervals::binary as selected_integer_binary_interval;
pub use selected_scalar_intervals::{
    carrier_integer_range, checked_integer_binary_kind, integer_range_contains,
};

pub use cast_ranges::{plain_exact_widening_cast, unannotated_exact_cast};
pub(crate) use declared_source_ranges::{
    enforced_declared_range, expression_reads_unresolved_local_initializer,
};
pub use integer_ranges::integer_widen_is_total;
pub(crate) use integer_ranges::{integer_bit_width, primitive_range};
pub(crate) use range_constraints::range_constraint_interval;

use numerics::arithmetic::ArithmeticDomain;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::value_custody::places::declared_place_type_raw;
