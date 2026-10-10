//! Addresses are formed from completed operands, not the final selector
//! variables. A bound can change while a later bound evaluates; each coordinate
//! therefore reads its own capture. Stored descriptors retain these addresses
//! independently of later writes to their backing contents.

use super::Execution;
use arena::HandleSpan;
use checked_trees::{FlowConstraintKind, FlowOperandFact, FlowOperandReferent};
use facts::{PlaceRoot, PlaceSegment, ScalarValue};
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

impl Execution<'_, '_, '_> {
    /// Read the completed value installed by a reaching store. A matching
    /// unresolved capture or an intervening replacement is authoritative:
    /// neither may fall back to the declaration's initializer.
    pub(super) fn stored_view_referents(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<HandleSpan<FlowOperandReferent>> {
        let source = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )?;
        if !super::super::operand_contexts::binding_origin_is_stable(
            self.program,
            &self.context.control,
            self.entry_origin_writes,
            self.entry_origin_unknown_write,
            &source,
        ) {
            return Some(HandleSpan::empty());
        }
        let statements = super::super::reference_spans::appended_span_since(
            &self.context.control.statements,
            self.context.state_statements_start,
        );
        for statement in self
            .context
            .control
            .statements
            .span_or_empty(statements)
            .iter()
            .rev()
            .filter(|statement| statement.statement_index < self.statement_index)
        {
            if self.semantic.places.is_valid(statement.stored_destination)
                && self
                    .context
                    .control
                    .operands
                    .is_valid(statement.stored_value)
            {
                let destination = super::super::canonical_place_from_semantic_place(
                    self.program,
                    self.semantic,
                    self.semantic.places.get(statement.stored_destination),
                )?;
                if destination.root == source.root
                    && source.segments.starts_with(&destination.segments)
                {
                    let stored = self.context.control.operands.get(statement.stored_value);
                    let path = &source.segments[destination.segments.len()..];
                    if self.is_view_type(stored.type_reference) {
                        let places = self
                            .context
                            .control
                            .operand_referents
                            .span_or_empty(stored.referents)
                            .iter()
                            .map(|referent| {
                                super::super::operand_values::project_captured_view_place(
                                    CanonicalPlace {
                                        root: referent.root,
                                        segments: self
                                            .context
                                            .control
                                            .operand_referent_segments
                                            .span_or_empty(referent.segments)
                                            .to_vec(),
                                    },
                                    path,
                                )
                            })
                            .collect::<Option<Vec<_>>>();
                        return Some(places.map_or_else(HandleSpan::empty, |places| {
                            self.append_referent_places(&places)
                        }));
                    }
                    let Some((captured, reference)) =
                        super::super::operand_values::captured_operand_at_path(
                            self.program,
                            &self.context.control,
                            stored,
                            stored.type_reference,
                            path,
                        )
                    else {
                        return Some(HandleSpan::empty());
                    };
                    if self.is_view_type(reference) {
                        return Some(captured.referents);
                    }
                    return None;
                }
            }
            if !super::super::operand_contexts::binding_origin_is_stable(
                self.program,
                &self.context.control,
                statement.completed_writes,
                statement.completed_unknown_write,
                &source,
            ) {
                return Some(HandleSpan::empty());
            }
        }
        // A local view without a reaching capture is unresolved. Historical
        // source declarations are not a replacement for produced storage.
        source
            .root
            .as_symbol()
            .and_then(|root| self.program.state_local_declaration(self.state, root))
            .filter(|(_, local)| self.is_view_type(local.type_reference))
            .map(|_| HandleSpan::empty())
    }

    pub(super) fn capture_indexed_referents(
        &mut self,
        expression: ExpressionHandle,
        collection: &FlowOperandFact,
        start: &FlowOperandFact,
        end: Option<&FlowOperandFact>,
    ) -> HandleSpan<FlowOperandReferent> {
        if !validation::place_has_builtin_coordinates(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        ) {
            return HandleSpan::empty();
        }
        let indexed = self.program.expression_table.indexed_node(expression);
        let Some(indexed) = indexed else {
            return HandleSpan::empty();
        };
        let mut places = self.captured_collection_places(collection);
        let geometry = if let Some(end) = end {
            let Some(range) = self.program.expression_table.range_node(indexed.index) else {
                return HandleSpan::empty();
            };
            let start_coordinate = if start.expression.is_valid() {
                self.captured_coordinate(start)
            } else {
                Some(0)
            };
            // The whole interval denotes the captured collection itself,
            // even when its runtime extent remains symbolic. This equality
            // uses selected builtin meaning, not a declaration's initializer.
            if start_coordinate == Some(0) && !end.expression.is_valid() && !range.end_inclusive {
                return self.append_referent_places(&places);
            }
            let end_coordinate = if end.expression.is_valid() {
                self.captured_coordinate(end).and_then(|end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                })
            } else {
                let mut extent = None;
                for place in &places {
                    let Some(current) = crate::checks::collection_extents::at_formed_place(
                        self.program,
                        self.state.symbol,
                        self.statement_index,
                        place,
                    ) else {
                        return HandleSpan::empty();
                    };
                    if extent.is_some_and(|previous| previous != current) {
                        return HandleSpan::empty();
                    }
                    extent = Some(current);
                }
                extent
            };
            match (start_coordinate, end_coordinate) {
                (Some(start), Some(end)) if start <= end => PlaceSegment::FixedRange { start, end },
                _ => PlaceSegment::Index {
                    expression: indexed.index,
                },
            }
        } else {
            match self.captured_coordinate(start) {
                Some(index) => PlaceSegment::FixedIndex { index },
                None => PlaceSegment::Index {
                    expression: indexed.index,
                },
            }
        };
        for place in &mut places {
            place.segments.push(geometry);
            if super::super::compose_captured_view_segments(&mut place.segments).is_none() {
                return HandleSpan::empty();
            }
        }
        self.append_referent_places(&places)
    }

    fn captured_coordinate(&self, operand: &FlowOperandFact) -> Option<usize> {
        validation::captured_integer_value(
            self.program,
            self.machine,
            self.state,
            operand.expression,
            |place| {
                // Authored literals belong to the arithmetic engine. Only
                // actual storage observations can bind its captured places.
                if !matches!(place.root, PlaceRoot::Symbol(_)) {
                    return None;
                }
                let value = crate::values::scalar_value_at_place(
                    self.program,
                    self.semantic,
                    self.context
                        .contexts
                        .constraint_refs
                        .span_or_empty(operand.constraints)
                        .iter()
                        .filter_map(|reference| {
                            let FlowConstraintKind::SemanticContext { context } = reference.kind
                            else {
                                return None;
                            };
                            Some(self.semantic.contexts.get(context))
                        }),
                    place,
                )?;
                match value {
                    ScalarValue::Integer(value) => Some(value),
                    _ => None,
                }
            },
        )?
        .to_u64()
        .and_then(|value| usize::try_from(value).ok())
    }

    fn captured_collection_places(&mut self, operand: &FlowOperandFact) -> Vec<CanonicalPlace> {
        if !operand.referents.is_empty() {
            return self
                .context
                .control
                .operand_referents
                .span_or_empty(operand.referents)
                .iter()
                .map(|referent| CanonicalPlace {
                    root: referent.root,
                    segments: self
                        .context
                        .control
                        .operand_referent_segments
                        .span_or_empty(referent.segments)
                        .to_vec(),
                })
                .collect();
        }
        if self
            .context
            .expression_result_type_at(self.program, self.machine, self.state, operand.expression)
            .is_some_and(|reference| self.is_view_type(reference))
        {
            return Vec::new();
        }
        validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            operand.expression,
        )
        .filter(|place| matches!(place.root, PlaceRoot::Symbol(_)))
        .into_iter()
        .collect()
    }

    pub(super) fn append_referent_places(
        &mut self,
        places: &[CanonicalPlace],
    ) -> HandleSpan<FlowOperandReferent> {
        let rows = places
            .iter()
            .map(|place| FlowOperandReferent {
                root: place.root,
                segments: self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(place.segments.iter().copied()),
            })
            .collect::<Vec<_>>();
        self.context.control.operand_referents.insert_many(rows)
    }
}
