//! Construction publishes scalar snapshots when each child finishes evaluating.
//! Later siblings may write the child's former source; the established member
//! already holds its own value. Normal calls invalidate this destination through
//! their ordinary write frontier. Cleanup reads the surviving facts at its edge,
//! never recovers an initializer or assumes a hook prerequisite.

use super::{EvaluatedOperand, Execution};
use arena::HandleSpan;
use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, PlaceSegment, ProgramPoint, QualificationEvidence,
    ScalarValue,
};
use typed_trees::expression::ExpressionHandle;

impl Execution<'_, '_, '_> {
    pub(super) fn append_constructed_values(
        &mut self,
        parent: ExpressionHandle,
        destination: &[PlaceSegment],
        type_reference: typed_trees::types::TypeReferenceHandle,
        evaluated: &EvaluatedOperand,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        // Only owned contents transport member values. A reference/slice
        // carrier retains live referents, not snapshots below its descriptor.
        // Nominal cleanup is a later disposition and does not change the
        // scalar storage already established by this child evaluation.
        if !type_reference.is_valid()
            || !validation::has_borrow_free_contents(self.program, type_reference)
        {
            return;
        }
        let captured_contexts = self
            .context
            .contexts
            .constraint_refs
            .span_or_empty(evaluated.capture.constraints)
            .iter()
            .filter_map(|reference| {
                if let FlowConstraintKind::SemanticContext { context } = reference.kind {
                    Some(context)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut values = Vec::new();
        let primitive = self.program.primitive_type_reference(type_reference);
        let scalar = evaluated.boolean.map(ScalarValue::Boolean).or_else(|| {
            let primitive = primitive?;
            let expression = evaluated.capture.expression;
            if !validation::has_builtin_bound_expression_meaning(
                self.program,
                self.machine,
                Some(self.state),
                expression,
            ) {
                return None;
            }
            let expression = crate::values::lower_unit_scalar_argument(
                self.program,
                self.context.operators,
                self.state,
                self.statement_index,
                expression,
                primitive,
            )?;
            let symbols = crate::values::unit_scalar_binding_symbols(
                self.program,
                self.state,
                self.statement_index,
            )?;
            crate::values::evaluate_checked_scalar(
                &expression,
                &mut crate::values::PlaceScalarValues {
                    program: self.program,
                    parameters: self.program.state_parameters(self.state),
                    symbols: &symbols,
                    value_at_place: |place: &crate::flow::CanonicalPlace| {
                        crate::values::scalar_value_at_place(
                            self.program,
                            self.semantic,
                            captured_contexts
                                .iter()
                                .map(|context| self.semantic.contexts.get(*context)),
                            place,
                        )
                    },
                },
            )
        });
        if let Some(value) = scalar {
            values.push((destination.to_vec(), value));
        }
        // Scalar children supplied their completed observation. Only
        // aggregate children need the captured context's nested member facts.
        let is_aggregate = primitive.is_none();
        if is_aggregate
            && let Some(source) = self.context.canonical_place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                evaluated.capture.expression,
            )
        {
            // Aggregate children can themselves be constructions or moved
            // storage. Rebase their current scalar observations, including
            // nested fields and array members, onto the new destination.
            for context in &captured_contexts {
                for fact in self
                    .semantic
                    .context_view(self.semantic.contexts.get(*context))
                    .facts()
                {
                    let FactPlace::Place(place) = fact.place else {
                        continue;
                    };
                    let Some(place) = crate::flow::canonical_place_from_semantic_place(
                        self.program,
                        self.semantic,
                        self.semantic.places.get(place),
                    ) else {
                        continue;
                    };
                    if place.root != source.root {
                        continue;
                    }
                    let Some(relative) = place.segments.strip_prefix(source.segments.as_slice())
                    else {
                        continue;
                    };
                    if !crate::flow::place_cases_are_selected(
                        self.program,
                        self.semantic,
                        &captured_contexts,
                        self.machine.symbol,
                        self.state.symbol,
                        self.statement_index,
                        &place,
                    ) {
                        continue;
                    }
                    let value = crate::checks::contracts::boolean_value_at_place(
                        self.program,
                        self.semantic,
                        &captured_contexts,
                        &place,
                    )
                    .map(ScalarValue::Boolean)
                    .or_else(|| {
                        crate::values::scalar_value_at_place(
                            self.program,
                            self.semantic,
                            captured_contexts
                                .iter()
                                .map(|context| self.semantic.contexts.get(*context)),
                            &place,
                        )
                    });
                    let Some(value) = value else {
                        continue;
                    };
                    let mut path = destination.to_vec();
                    path.extend_from_slice(relative);
                    if !values.iter().any(|(existing, _)| *existing == path) {
                        values.push((path, value));
                    }
                }
            }
        }
        if values.is_empty() {
            return;
        }
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        let mut references = HandleSpan::empty();
        for (path, value) in values {
            let place = self.semantic.append_expression_place(parent);
            for segment in path {
                self.semantic.push_place_segment(place, segment);
            }
            let value = self.semantic.scalar_values.append(value);
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::AssignedScalarValue { value },
            });
            self.semantic.append_ref(&mut references, fact);
        }
        let context = self.semantic.append_context(point, references);
        crate::flow::reference_spans::append_flow_reference(
            &mut self.context.contexts.semantic_context_refs,
            contexts,
            FlowSemanticContextRef { context },
        );
        crate::flow::append_constraint_ref(
            &mut self.context.contexts.constraint_refs,
            constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
}
