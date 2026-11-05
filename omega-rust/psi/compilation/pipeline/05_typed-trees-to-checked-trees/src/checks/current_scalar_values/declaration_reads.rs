//! Rejoin a demanded declaration read with one admitted invocation observation.
//! The declaration's selector uses its supplying telescope, while the selected
//! field keeps its own storage, entry-bound and scalar-value receipts. Resolving
//! a selector cannot invent an observation or grant access to its record.

use super::scalar_fields::{CapturedScalarField, ScalarField, capture_path, same_projection, source_path};
use super::{CurrentScalarValues, ScalarProjection};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use validation::{DeclaredInputRead, RankSubjectCoordinate, ScopedArithmeticValue};

impl CurrentScalarValues<'_> {
    /// Entry rank observation may demand indexed declaration reads before any
    /// statement executes. Seed any missing scalar leaf the declared projection
    /// already resolves from entry storage and selector receipts.
    pub(crate) fn seed_declared_read_entry_fields(
        &mut self,
        root: &RankSubjectCoordinate,
        reads: &[DeclaredInputRead],
    ) {
        for read in reads {
            let Some(projection) =
                self.declared_projection(root, read, &read.coordinate, 0)
            else {
                continue;
            };
            if self.fields.iter().any(|field| {
                field.owner == root.parameter
                    && field.carrier == read.primitive_type
                    && field.value.is_some()
                    && same_projection(self.occurrence(), &field.path, &projection)
            }) {
                continue;
            }
            self.fields.push(ScalarField::for_owner(
                root.parameter,
                CapturedScalarField {
                    path: projection,
                    carrier: read.primitive_type,
                    value: ScopedArithmeticValue::Atom {
                        identity: format!(
                            "declared-read:parameter:{:?}:calls:{:?}",
                            root.parameter,
                            read.calls
                        ),
                        unsigned: super::unsigned(read.primitive_type),
                    },
                },
            ));
        }
    }

    pub(crate) fn declared_read_path(
        &self,
        root: &RankSubjectCoordinate,
        read: &DeclaredInputRead,
    ) -> Option<Vec<facts::PlaceSegment>> {
        let selected = self.declared_projection(root, read, &read.coordinate, 0)?;
        self.fields
            .iter()
            .find(|field| {
                field.owner == root.parameter
                    && field.carrier == read.primitive_type
                    && field.value.is_some()
                    && same_projection(self.occurrence(), &field.path, &selected)
            })
            .map(|field| source_path(&field.path))
    }

    fn declared_projection(
        &self,
        root: &RankSubjectCoordinate,
        read: &DeclaredInputRead,
        coordinate: &RankSubjectCoordinate,
        depth: usize,
    ) -> Option<Vec<ScalarProjection>> {
        if depth >= 64 {
            return None;
        }
        // The read's own projection owner is checked at the outer call. Selector
        // telescopes may name outer formals with different parameter symbols.
        if depth == 0 && coordinate.parameter != read.coordinate.parameter {
            return None;
        }
        let mut prefix = capture_path(&root.path, |selector| {
            super::scalar_fields::coordinates::selector_value(
                self.occurrence(),
                selector,
                &self.current,
                &self.fields,
            )
        })?;
        prefix.extend(capture_path(&coordinate.path, |selector| {
            self.declared_read_selector_value(read, root, depth, selector)
        })?);
        Some(prefix)
    }

    fn declared_read_selector_value(
        &self,
        read: &DeclaredInputRead,
        root: &RankSubjectCoordinate,
        depth: usize,
        selector: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        let supplying = match self.program.expression_table.expression(selector) {
            ExpressionNode::Name(name) => read
                .selector_arguments
                .iter()
                .find(|(formal, _)| *formal == name.symbol)
                .map(|(_, actual)| *actual)
                .unwrap_or(selector),
            _ => selector,
        };
        for candidate in [supplying, selector] {
            if let Some(value) =
                self.telescoped_selector_leaf_value_from_expression(read, root, depth, candidate)
            {
                return Some(value);
            }
            if let Some(value) = read.selector_value(self.program, candidate, &mut |input| {
                self.telescoped_selector_leaf_value_from_coordinate(read, root, depth, input)
            }) {
                return Some(value);
            }
            if let Some(value) = super::scalar_fields::coordinates::entry_selector_value(
                self.occurrence(),
                candidate,
                &self.current,
                &self.fields,
            ) {
                return Some(value);
            }
        }
        read.selector_value(self.program, selector, &mut |input| {
            self.telescoped_selector_leaf_value_from_coordinate(read, root, depth, input)
        })
        .or_else(|| {
            let field_symbol = read.coordinate.path.first().and_then(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => Some(*symbol),
                _ => None,
            })?;
            self.fields.iter().find_map(|field| {
                if field.owner != root.parameter {
                    return None;
                }
                match field.path.as_slice() {
                    [
                        ScalarProjection::Field(symbol),
                        ScalarProjection::Element { selector, .. },
                    ] if *symbol == field_symbol => Some(selector.clone()),
                    _ => None,
                }
            })
        })
        .or_else(|| {
            self.fields.iter().find_map(|field| {
                if field.owner != root.parameter {
                    return None;
                }
                let ScalarProjection::Element {
                    selector: observed, ..
                } = field.path.last()?
                else {
                    return None;
                };
                let demanded =
                    self.telescoped_selector_leaf_value_from_expression(read, root, depth, supplying)?;
                (validation::scoped_arithmetic_values_relation(
                    self.occurrence().program,
                    self.occurrence().machine,
                    self.occurrence().hypotheses,
                    typed_trees::expression::BinaryOperator::Equal,
                    &demanded,
                    observed,
                ) == validation::StrictArithmeticImplicationJudgment::Proven)
                .then(|| observed.clone())
            })
        })
    }

    fn telescoped_selector_leaf_value_from_expression(
        &self,
        read: &DeclaredInputRead,
        root: &RankSubjectCoordinate,
        depth: usize,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        let place = validation::canonical_place_from_expression(self.program, expression)
            .or_else(|| {
                validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                )
            })?;
        let formal_root = place.root.as_symbol()?;
        let (_, input) = read
            .selector_inputs
            .iter()
            .find(|(formal, _)| *formal == formal_root)?;
        let mut coordinate = input.clone();
        coordinate.path.extend(place.segments);
        self.telescoped_selector_leaf_value_from_coordinate(read, root, depth, &coordinate)
    }

    fn telescoped_selector_leaf_value_from_coordinate(
        &self,
        read: &DeclaredInputRead,
        root: &RankSubjectCoordinate,
        depth: usize,
        input: &RankSubjectCoordinate,
    ) -> Option<ScopedArithmeticValue> {
        let selected = self.declared_projection(root, read, input, depth + 1)?;
        self.fields
            .iter()
            .find(|field| {
                field.owner == root.parameter
                    && same_projection(self.occurrence(), &field.path, &selected)
            })
            .and_then(|field| field.value.clone())
    }
}
