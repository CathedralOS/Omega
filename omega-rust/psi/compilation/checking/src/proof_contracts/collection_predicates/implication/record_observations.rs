//! Exact immutable record observations inside the symbolic predicate judgment.
//! A record names one captured value and a complete declaration-relative path.
//! Fields are read lazily so an unrelated unsupported field cannot erase a
//! supported observation. Stored references are separate epochs, not owned
//! field paths; this owner never follows them or manufactures caller syntax.

use super::*;

pub(super) struct Observation {
    identity: usize,
    projection: Vec<facts::PlaceSegment>,
    value: Value,
}

impl Implication<'_> {
    pub(super) fn record_member(
        &mut self,
        mut path: Path,
        receiver: Value,
        symbol: SymbolHandle,
    ) -> Option<(Path, Value)> {
        self.spend()?;
        let Value::Record {
            identity,
            reference,
            mut projection,
        } = receiver
        else {
            return None;
        };
        let definition =
            crate::value_custody::places::data_definition_for_type(self.program, reference)?;
        // Common fields belong to this exact declaration. A payload field
        // needs its own checked case path, never a same-named common leaf.
        if !self.program.data_members(definition).iter().any(|member| {
            matches!(member,
            typed_trees::data::DataMember::Field(field) if field.symbol == symbol && field.relevance == language_core::BindingRelevance::Relevant)
        }) {
            return None;
        }
        let segment = facts::PlaceSegment::Field { symbol };
        let field_type = crate::project_type_reference_from_segments(
            self.program,
            reference,
            std::slice::from_ref(&segment),
        )?;
        if crate::reference_shell(self.program, field_type).is_some() {
            return None;
        }
        projection.push(segment);
        if let Some(value) = self
            .records
            .iter()
            .find(|observation| {
                observation.identity == identity && observation.projection == projection
            })
            .map(|observation| observation.value.clone())
        {
            if let Value::Integer {
                value,
                carrier: Some(carrier),
            } = &value
            {
                let (low, high) = self
                    .program
                    .primitive_type_reference(*carrier)?
                    .integer_bounds()?;
                path.comparisons.push(compare(
                    BinaryOperator::GreaterOrEqual,
                    value.clone(),
                    number(low),
                ));
                path.comparisons.push(compare(
                    BinaryOperator::LessOrEqual,
                    value.clone(),
                    number(high),
                ));
            }
            return Some((path, value));
        }
        let value = match value_shape(self.program, field_type)? {
            ValueShape::Record(reference) => Value::Record {
                identity,
                reference,
                projection: projection.clone(),
            },
            ValueShape::Integer(carrier) => self.integer(carrier, &mut path)?,
            ValueShape::Collection { .. } => self.parameter(field_type, &mut path)?,
            ValueShape::Boolean => return None,
        };
        self.records.push(Observation {
            identity,
            projection,
            value: value.clone(),
        });
        Some((path, value))
    }

    pub(super) fn integer_subjects(
        &mut self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        value: Value,
        path: &mut Path,
        subjects: &mut Vec<(super::super::PredicateIntegerSubject, Integer)>,
        depth: usize,
    ) -> Option<()> {
        self.spend()?;
        if depth >= 32 {
            return None;
        }
        match &value {
            Value::Integer { value, .. } => subjects.push((
                super::super::PredicateIntegerSubject {
                    expression,
                    projection: Vec::new(),
                },
                value.clone(),
            )),
            Value::Record { reference, .. } => {
                let definition = crate::value_custody::places::data_definition_for_type(
                    self.program,
                    *reference,
                )?;
                let fields = self
                    .program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        typed_trees::data::DataMember::Field(field)
                            if field.relevance == language_core::BindingRelevance::Relevant =>
                        {
                            Some(field.symbol)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                for symbol in fields {
                    let field_type = crate::project_type_reference_from_segments(
                        self.program,
                        *reference,
                        &[facts::PlaceSegment::Field { symbol }],
                    )?;
                    if crate::reference_shell(self.program, field_type).is_some()
                        || !matches!(
                            value_shape(self.program, field_type),
                            Some(
                                ValueShape::Integer(_)
                                    | ValueShape::Record(_)
                                    | ValueShape::Collection { .. }
                            )
                        )
                    {
                        continue;
                    }
                    let (next, field) = self.record_member(path.clone(), value.clone(), symbol)?;
                    *path = next;
                    let before = subjects.len();
                    self.integer_subjects(
                        expression,
                        field_type,
                        field,
                        path,
                        subjects,
                        depth + 1,
                    )?;
                    // Each recursive read's path is relative to the original
                    // actual. Preserve every enclosing field in declaration order.
                    for (subject, _) in &mut subjects[before..] {
                        subject
                            .projection
                            .insert(0, facts::PlaceSegment::Field { symbol });
                    }
                }
            }
            Value::Collection {
                identity,
                length,
                element,
            } => {
                let Some(ValueShape::Collection {
                    length: Some(extent),
                    element_type,
                }) = value_shape(self.program, reference)
                else {
                    // A runtime descriptor is not a finite element roster.
                    return Some(());
                };
                if element_type != *element {
                    return None;
                }
                for index in 0..extent {
                    self.spend()?;
                    let selector = number(i128::try_from(index).ok()?);
                    if !self.implies(
                        path,
                        &compare(BinaryOperator::GreaterOrEqual, selector.clone(), number(0)),
                    ) || !self.implies(
                        path,
                        &compare(BinaryOperator::Less, selector.clone(), length.clone()),
                    ) {
                        return None;
                    }
                    // Reuse the collection reader's exact value identity and
                    // branch assumptions. A split cannot be silently selected;
                    // these fresh fixed formals need one complete read path.
                    let mut outcomes = self.read(path.clone(), *identity, selector, *element, 0)?;
                    if outcomes.len() != 1 {
                        return None;
                    }
                    let (next, value) = outcomes.pop()?;
                    *path = next;
                    let before = subjects.len();
                    self.integer_subjects(
                        expression,
                        element_type,
                        value,
                        path,
                        subjects,
                        depth + 1,
                    )?;
                    for (subject, _) in &mut subjects[before..] {
                        subject
                            .projection
                            .insert(0, facts::PlaceSegment::FixedIndex { index });
                    }
                }
            }
            _ => {}
        }
        Some(())
    }
}
