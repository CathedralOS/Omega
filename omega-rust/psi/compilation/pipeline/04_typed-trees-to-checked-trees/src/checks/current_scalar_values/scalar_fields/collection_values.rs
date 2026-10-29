//! Finite collection observations travel with their captured values. Array
//! construction records element coordinates; a formed window translates those
//! coordinates, never the backing capacity. Reading an unknown selector can
//! join a contiguous equal-value run only after independently proving inclusion.

use super::super::{CurrentScalarValues, ScopedOccurrence};
use super::{CapturedScalarField, ScalarField, ScalarProjection, integer_carrier, same_projection};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;
use validation::ScopedArithmeticValue;

fn element(index: u64) -> Option<ScalarProjection> {
    Some(ScalarProjection::Element {
        source: facts::PlaceSegment::FixedIndex {
            index: usize::try_from(index).ok()?,
        },
        selector: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(index)),
    })
}

impl CurrentScalarValues<'_> {
    pub(super) fn capture_array_elements(
        &self,
        elements: arena::HandleSpan<ExpressionHandle>,
        reference: typed_trees::types::TypeReferenceHandle,
        depth: usize,
    ) -> Vec<CapturedScalarField> {
        let Some((element_type, _)) =
            validation::readable_fixed_array_elements(self.program, reference)
        else {
            return Vec::new();
        };
        let mut fields = Vec::new();
        for (index, expression) in self
            .program
            .expression_table
            .expression_handles(elements)
            .iter()
            .enumerate()
        {
            let Some(index) = u64::try_from(index).ok().and_then(element) else {
                return Vec::new();
            };
            for mut field in
                self.captured_scalar_fields_of_type(*expression, element_type, depth + 1)
            {
                field.path.insert(0, index.clone());
                fields.push(field);
            }
            let carrier = integer_carrier(self.program, element_type);
            if let Some(carrier) = carrier
                && let Some(value) =
                    self.capture(self.program, self.machine, self.state, *expression)
            {
                fields.push(CapturedScalarField {
                    path: vec![index],
                    carrier,
                    value,
                    read_dependencies: Vec::new(),
                    source: None,
                });
            }
        }
        fields
    }

    pub(super) fn capture_join_fields(
        &self,
        expression: ExpressionHandle,
        dispatch: &typed_trees::expression::TableMatchExpression,
        depth: usize,
    ) -> Vec<CapturedScalarField> {
        let arms = self.program.expression_table.match_arms(dispatch.arms);
        let alternatives = arms
            .iter()
            .map(|arm| self.captured_scalar_fields(arm.value, depth + 1))
            .collect::<Vec<_>>();
        let Some(first) = alternatives.first() else {
            return Vec::new();
        };
        first
            .iter()
            .filter_map(|field| {
                let selected = alternatives
                    .iter()
                    .map(|fields| {
                        fields.iter().find(|candidate| {
                            candidate.carrier == field.carrier
                                && same_projection(self.occurrence(), &candidate.path, &field.path)
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let value = super::super::joins::joined_values(
                    self.program,
                    self.machine,
                    &self.hypotheses,
                    format!(
                        "state:{:?}:join-result:{expression:?}:field:{:?}",
                        self.state.symbol, field.path
                    ),
                    selected.iter().map(|field| field.value.clone()).collect(),
                )?;
                let mut read_dependencies = Vec::new();
                for candidate in selected {
                    for dependency in &candidate.read_dependencies {
                        crate::collections::push_unique(&mut read_dependencies, dependency.clone());
                    }
                    if let Some(source) = &candidate.source {
                        crate::collections::push_unique(&mut read_dependencies, source.clone());
                    }
                }
                Some(CapturedScalarField {
                    path: field.path.clone(),
                    carrier: field.carrier,
                    value,
                    read_dependencies,
                    source: None,
                })
            })
            .collect()
    }

    pub(super) fn capture_window_elements(
        &self,
        expression: ExpressionHandle,
        indexed: &typed_trees::expression::TableIndexedExpression,
        depth: usize,
    ) -> Option<Vec<CapturedScalarField>> {
        let (start, end) = self.window_bounds(expression, indexed, depth, None, None)?;
        // A finite retained coordinate needs a proved exact displacement. A
        // symbolic range still carries its descriptor extent through the
        // ordinary length roster, but invents no static element coordinates.
        let start = self.exact_length(&start)?.to_u64()?;
        let end = self.exact_length(&end)?.to_u64()?;
        Some(
            self.captured_scalar_fields(indexed.collection, depth + 1)
                .into_iter()
                .filter_map(|mut field| {
                    let ScalarProjection::Element { selector, .. } = field.path.first()? else {
                        return None;
                    };
                    let index = self.exact_length(selector)?.to_u64()?;
                    if index < start || index >= end {
                        return None;
                    }
                    field.path[0] = element(index.checked_sub(start)?)?;
                    Some(field)
                })
                .collect(),
        )
    }
}

fn proves(
    occurrence: ScopedOccurrence<'_, '_>,
    operator: BinaryOperator,
    left: &ScopedArithmeticValue,
    right: &ScopedArithmeticValue,
) -> bool {
    validation::scoped_arithmetic_values_relation(
        occurrence.program,
        occurrence.machine,
        occurrence.hypotheses,
        operator,
        left,
        right,
    ) == validation::StrictArithmeticImplicationJudgment::Proven
}

pub(super) fn selected_value(
    occurrence: ScopedOccurrence<'_, '_>,
    owner: SymbolHandle,
    path: &[ScalarProjection],
    carrier: PrimitiveType,
    fields: &[ScalarField],
) -> Option<ScopedArithmeticValue> {
    if let Some(field) = fields.iter().find(|field| {
        field.owner == owner
            && field.carrier == carrier
            && same_projection(occurrence, &field.path, path)
    }) {
        return field.value.clone();
    }
    let (ScalarProjection::Element { selector, .. }, prefix) = path.split_last()? else {
        return None;
    };
    let mut elements = fields
        .iter()
        .filter_map(|field| {
            if field.owner != owner
                || field.carrier != carrier
                || field.path.len() != path.len()
                || !same_projection(occurrence, &field.path[..prefix.len()], prefix)
            {
                return None;
            }
            let ScalarProjection::Element {
                selector: ScopedArithmeticValue::Integer(index),
                ..
            } = field.path.last()?
            else {
                return None;
            };
            Some((index.to_u64()?, field.value.as_ref()?))
        })
        .collect::<Vec<_>>();
    elements.sort_by_key(|(index, _)| *index);
    // Duplicate coordinates do not establish a finite partition.
    if elements.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    let mut start = 0;
    while start < elements.len() {
        let mut end = start;
        while end + 1 < elements.len()
            && elements[end].0.checked_add(1) == Some(elements[end + 1].0)
            && elements[start].1 == elements[end + 1].1
        {
            end += 1;
        }
        let low =
            ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(elements[start].0));
        let high =
            ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(elements[end].0));
        if proves(occurrence, BinaryOperator::GreaterOrEqual, selector, &low)
            && proves(occurrence, BinaryOperator::LessOrEqual, selector, &high)
        {
            return Some(elements[start].1.clone());
        }
        start = end + 1;
    }
    // Distinct elements need not be equal to share an independently proved
    // bound. A complete contiguous partition supplies one captured read whose
    // interval is the union of every possible element's interval.
    if let (Some(first), Some(last)) = (elements.first(), elements.last())
        && elements
            .windows(2)
            .all(|pair| pair[0].0.checked_add(1) == Some(pair[1].0))
    {
        let low = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(first.0));
        let high = ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_u64(last.0));
        if proves(occurrence, BinaryOperator::GreaterOrEqual, selector, &low)
            && proves(occurrence, BinaryOperator::LessOrEqual, selector, &high)
        {
            return super::super::joins::joined_values(
                occurrence.program,
                occurrence.machine,
                occurrence.hypotheses,
                format!("selected:{owner:?}:{path:?}:{elements:?}"),
                elements.iter().map(|(_, value)| (*value).clone()).collect(),
            );
        }
    }
    None
}
