//! A contextual primitive read detaches contents from its evaluated descriptor.
//! The descriptor supplies geometry and authority; only post-receiver facts
//! supply contents. Reading through a call, a saved reference or a projection
//! uses this same cut, without recovering an initializer or replaying a body.

use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowConstraintRef, FlowOperandFact, FlowSemanticContextRef};
use facts::{FactPayload, IntegerRange, ScalarValue};
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

impl Execution<'_, '_, '_> {
    pub(super) fn retain_primitive_read(
        &mut self,
        expression: ExpressionHandle,
        descriptor: Handle<FlowOperandFact>,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let descriptor_value = *self.context.control.operands.get(descriptor);
        let Some(primitive) = self.program.primitive_type_reference(destination) else {
            return;
        };
        let Some(shell) =
            validation::reference_shell(self.program, descriptor_value.type_reference)
        else {
            return;
        };
        if !shell.access.is_readable()
            || self.program.primitive_type_reference(shell.referee) != Some(primitive)
        {
            return;
        }
        let sources = self
            .context
            .control
            .operand_referents
            .span_or_empty(descriptor_value.referents);
        if sources.is_empty() {
            return;
        }
        let mut scalar: Option<ScalarValue> = None;
        let mut bounds: Option<IntegerRange> = None;
        let mut scalar_complete = true;
        let mut bounds_complete = true;
        for source in sources {
            let place = validation::CanonicalPlace {
                root: source.root,
                segments: self
                    .context
                    .control
                    .operand_referent_segments
                    .span_or_empty(source.segments)
                    .to_vec(),
            };
            let source_type = validation::canonical_place_type_reference(
                self.program,
                self.state.symbol,
                self.statement_index,
                &place,
            )
            .and_then(|reference| {
                validation::field_domains::readable_type_reference(self.program, reference)
            });
            // Equal-width representation views need not have the same scalar
            // meaning. Such a view needs its own interpretation evidence;
            // neither representability nor a result annotation grants equality.
            if source_type.and_then(|reference| self.program.primitive_type_reference(reference))
                != Some(primitive)
            {
                return;
            }
            let value = crate::values::scalar_value_at_place(
                self.program,
                self.semantic,
                self.context
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(*contexts)
                    .iter()
                    .map(|reference| self.semantic.contexts.get(reference.context)),
                &place,
            );
            match (scalar.as_ref(), value) {
                (_, None) => scalar_complete = false,
                (Some(previous), Some(value)) if *previous != value => scalar_complete = false,
                (_, Some(value)) => scalar = Some(value),
            }
            let range = crate::values::integer_bounds_at_place(
                self.program,
                self.semantic,
                self.context
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(*contexts)
                    .iter()
                    .map(|reference| self.semantic.contexts.get(reference.context)),
                &place,
            );
            match (bounds.take(), range) {
                (_, None) => bounds_complete = false,
                (Some(previous), Some(value)) => {
                    bounds = Some(IntegerRange {
                        minimum: previous.minimum.min(value.minimum),
                        maximum: previous.maximum.max(value.maximum),
                    })
                }
                (None, Some(value)) => bounds = Some(value),
            }
        }
        // The completed operand already retains this descriptor as `backing`.
        // These owned scalar facts carry no new referent authority or lifetime.
        if let Some(value) = scalar.filter(|_| scalar_complete) {
            let value = self.semantic.scalar_values.append(value);
            self.append_constructed_observation(
                expression,
                &[],
                FactPayload::AssignedScalarValue { value },
                contexts,
                constraints,
            );
        }
        if let Some(bounds) = bounds.filter(|_| bounds_complete) {
            let bounds = self.semantic.integer_ranges.append(bounds);
            self.append_constructed_observation(
                expression,
                &[],
                FactPayload::AssignedIntegerBounds { bounds },
                contexts,
                constraints,
            );
        }
    }
}
