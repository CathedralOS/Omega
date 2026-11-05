//! Describe a value after its operands and own effects completed.
//!
//! Failure to represent this value does not undo those effects or erase a
//! separately requested operand observation. The caller owns that availability
//! distinction; this operation still uses the ordinary typed value owners.

use super::super::super::structural_terms;
use super::*;

impl<'program> StructuralJudge<'program> {
    pub(super) fn completed_value_path(
        self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        inputs: &[(ExpressionHandle, NormalOperandResult)],
        call_values: Option<Vec<StructuralTerm>>,
        receiver_changed: bool,
        scalar_storage_read: bool,
        reference_contents: Option<StructuralTerm>,
    ) -> Option<NormalReturnPath<'program>> {
        let program = self.program;
        let node = expression_table(program).expression(expression);
        let mut judge = self;
        // Complete scalar contents through the common storage owner before
        // descriptor projection can hide them. The captured operands select
        // this read; an intervening receiver write forbids address replay.
        let completed_reference = if let Some(contents) = reference_contents {
            let declared = judge.scalar_reference_read_type(scope.0, scope.1, expression)?;
            let result_type =
                crate::value_custody::expression_types::readable_observation_type_reference(
                    program, declared,
                )?;
            let mut observed = judge.capture_unknown_value(expression, scope)?;
            if let StructuralTerm::CapturedValue {
                result_type: carrier,
                integer_range,
                ..
            } = &mut observed
            {
                // This is a performed scalar read, not a descriptor cast.
                // The read owner supplied every current-content alternative;
                // each normal path proves its own independently captured value.
                *carrier = result_type;
                *integer_range = judge
                    .completed_integer_range_recipe(&contents)
                    .map(Arc::new);
            }
            judge.assume_completed_value_equation(result_type, observed.clone(), contents);
            if super::super::super::struct_trace() {
                eprintln!(
                    "STRUCT completed-reference-contents expression={expression:?} descriptor_type={declared:?} scalar_type={result_type:?} observed={observed:?}"
                );
            }
            Some(
                structural_terms::shared_inline_observation(program, declared, observed.clone())
                    .unwrap_or(observed),
            )
        } else if !receiver_changed
            && let Some(reference) = judge.scalar_reference_read_type(scope.0, scope.1, expression)
        {
            judge
                .stored_value_observation(Some((scope.0, scope.1)), expression, |selector| {
                    inputs
                        .iter()
                        .find(|(input, _)| *input == selector)
                        .and_then(|(_, input)| input.observed_value().cloned())
                })
                .filter(|value| {
                    structural_terms::scalar_operand_is_observed(program, reference, value)
                })
        } else {
            None
        };
        let stable_descriptor = (!receiver_changed)
            .then(|| judge.completed_descriptor_place(Some(scope), expression, environment))
            .flatten()
            .map(StructuralTerm::into_collection_descriptor);
        let descriptor_projection = stable_descriptor.or_else(|| {
            let reference =
                crate::expression_result_type_reference(program, scope.0, scope.1, expression)?;
            crate::reference_shell(program, reference)?;
            let (receiver, selector) = match node {
                ExpressionNode::Member(member) => (member.receiver, None),
                ExpressionNode::Indexed(indexed)
                    if program.expression_table.range_node(indexed.index).is_none() =>
                {
                    let selector = inputs
                        .iter()
                        .find(|(source, _)| *source == indexed.index)?
                        .1
                        .observed_value()?
                        .clone();
                    (indexed.collection, Some(selector))
                }
                _ => return None,
            };
            let receiver = inputs
                .iter()
                .find(|(source, _)| *source == receiver)?
                .1
                .observed_value()?
                .clone();
            super::super::super::observed_projections::completed_reference_descriptor(
                program, &judge, scope.0, scope.1, expression, receiver, selector,
            )
        });
        let descriptor_completed = descriptor_projection.is_some() || completed_reference.is_some();
        let completed_descriptor = if let Some(descriptor) = descriptor_projection {
            // The event remains opaque. Only its separately retained address
            // can be rejoined with frame-authorized current referent storage.
            let value = judge.capture_unknown_value(expression, scope)?;
            Some(StructuralTerm::ObservedValue {
                observed_type: crate::expression_result_type_reference(
                    program, scope.0, scope.1, expression,
                )?,
                value: Box::new(value),
                descriptor: Some(Box::new(descriptor)),
            })
        } else {
            None
        };
        let completed_field = if let ExpressionNode::Member(member) = node {
            let receiver_type = crate::expression_result_type_reference(
                program,
                scope.0,
                scope.1,
                member.receiver,
            )?;
            // An owned receiver has already completed at this operand.
            // Project that captured value without replaying its former
            // source home. Reference descriptors still cannot supply
            // observations of their referent's fields.
            if crate::reference_shell(program, receiver_type).is_none() {
                inputs
                    .iter()
                    .find(|(source, _)| *source == member.receiver)
                    .and_then(|(_, value)| {
                        judge.observed_field_term(
                            value.observed_value()?.clone(),
                            member.member.as_str(),
                            0,
                        )
                    })
            } else {
                None
            }
        } else {
            None
        };
        // Metadata observes the descriptor child that just completed,
        // not a replay of its source place. A backing write may retire
        // content observations without replacing this descriptor.
        let completed_measure = if let ExpressionNode::Member(member) = node {
            crate::expression_result_type_reference(program, scope.0, scope.1, member.receiver)
                .and_then(|reference| {
                    structural_terms::collection_measure_term(program, member, reference, || {
                        inputs
                            .iter()
                            .find(|(source, _)| *source == member.receiver)
                            .and_then(|(_, value)| value.observed_value().cloned())
                    })
                })
        } else {
            None
        };
        // A borrow uses its target's completed observation before any
        // source-place fallback. The old home can be retired while the
        // captured value remains valid; manufacturing a placeholder
        // first can fail typing and discard this normal path entirely.
        let completed_borrow = if let ExpressionNode::Borrow(borrow) = node {
            inputs
                .iter()
                .find(|(input, _)| *input == borrow.target)
                .and_then(|(_, referent)| referent.observed_value())
                .map(|observed| {
                    crate::expression_result_type_reference(program, scope.0, scope.1, expression)
                        .and_then(|declared| {
                            structural_terms::shared_inline_observation(
                                program,
                                declared,
                                observed.clone(),
                            )
                        })
                        .unwrap_or_else(|| observed.clone())
                })
        } else {
            None
        };
        if let ExpressionNode::Cast(cast) = node
            && let [(source_expression, source)] = inputs
            && let Some(value) = source.observed_value()
            && let Some(reference) = crate::expression_result_type_reference(
                program,
                scope.0,
                scope.1,
                *source_expression,
            )
            && program.primitive_type_reference(reference)
                != program.primitive_type_reference(cast.target_type)
            && structural_terms::denotation_preserving_conversion(program, cast, reference)
        {
            judge.intake_completed_scalar_bounds(reference, value);
        }
        // A value-preserving conversion still completes in its target
        // carrier. Retain that exact occurrence instead of publishing
        // the source carrier under the receiver's primitive operators.
        // Its equation and interval come from the completed operand;
        // the conversion's own obligation uses the pre-operation cut.
        let completed_cast = if judge.runtime_body_values
            && let ExpressionNode::Cast(cast) = node
            && let Some(source) =
                crate::expression_result_type_reference(program, scope.0, scope.1, cast.value)
            && crate::reference_shell(program, source).is_none()
            && let Some(source_primitive) = program.primitive_type_reference(source)
            && source_primitive.is_fixed_integer()
            && program
                .primitive_type_reference(cast.target_type)
                .is_some_and(|primitive| primitive.is_fixed_integer())
            && structural_terms::denotation_preserving_conversion(program, cast, source)
        {
            let operand = inputs
                .iter()
                .find(|(input, _)| *input == cast.value)
                .and_then(|(_, path)| path.observed_value())?
                .clone();
            let observed_range = judge.completed_integer_range_recipe(&operand);
            // The completed source's physical carrier is independent of
            // the receiving declaration and of conversion success. Keep
            // it when a formal has no narrower occurrence recipe.
            let carrier_range = source_primitive.integer_bounds().map(|(low, high)| {
                checked_trees::IntegerRangeRecipe::bounds(
                    numerics::bignum::BigInt::from_i128(low),
                    numerics::bignum::BigInt::from_i128(high),
                )
            });
            let range = match (observed_range, carrier_range) {
                (Some(observed), Some(carrier)) => Some(observed.intersect(&carrier)),
                (observed, carrier) => observed.or(carrier),
            };
            let mut converted = judge.capture_unknown_value(expression, scope)?;
            if let Some(range) = range
                && let StructuralTerm::CapturedValue { integer_range, .. } = &mut converted
            {
                let range = integer_range
                    .as_deref()
                    .map_or_else(|| range.clone(), |target| range.intersect(target));
                *integer_range = Some(std::sync::Arc::new(range));
            }
            // The denotation is the completed source integer, including
            // its authenticated carrier. A bare formal would erase that
            // carrier when exact arithmetic follows this equation.
            judge.assume_normal_equation(
                converted.clone(),
                StructuralTerm::IntegerEmbedding {
                    reference: source,
                    value: Box::new(operand),
                },
            );
            Some(converted)
        } else {
            None
        };
        let mut value = if receiver_changed {
            // The receiver selected its base before a later operand
            // wrote storage. Reconstructing it from the live place now
            // could silently select a replacement descriptor.
            match completed_descriptor
                .or(completed_field)
                .filter(|value| value.observation_identity_key().is_some())
            {
                Some(value) => value,
                None => judge.capture_unknown_value(expression, scope)?,
            }
        } else if let Some(value) = completed_reference
            .or(completed_descriptor)
            .or(completed_borrow)
            .or(completed_measure)
            .or(completed_field)
            .or(completed_cast)
        {
            value
        } else if matches!(node, ExpressionNode::Atomic(_)) {
            // The atomic owner distinguishes an observed load from
            // update carriers. Its value is a fresh event, not the
            // place's prior initializer or the modeled new resident.
            if crate::value_custody::atomic_operations::atomic_load_carrier(program, expression)
                .is_none()
                && crate::value_custody::atomic_operations::atomic_once_carrier(program, expression)
                    .is_none()
            {
                return None;
            }
            judge.capture_unknown_value(expression, scope)?
        } else if let Some(value) =
            judge.callee_term_in_scope(Some(scope), expression, environment, 0)
        {
            value
        } else if judge.runtime_body_values
            && (matches!(node, ExpressionNode::Call(_))
                || matches!(node, ExpressionNode::Indexed(indexed)
                            if program.expression_table.range_node(indexed.index).is_some()
                                && judge.completed_builtin_coordinates(expression, scope))
                || judge.storage_observation_invalidated(expression)
                || discarded_value_has_no_own_effects(program, scope, expression)
                || matches!(node, ExpressionNode::Cast(cast)
                            if !cast.form.is_recast()
                                && program.primitive_type_reference(cast.target_type).is_some()))
        {
            // Completion does not require a mathematical denotation.
            // Every child has already run, and the own-effect owner
            // above handled calls/atomics. Pure construction and
            // primitive conversion may therefore retain a fresh value
            // with no equations. In particular an earlier address cast
            // must not erase unrelated premises for a later cast.
            judge.capture_unknown_value(expression, scope)?
        } else {
            return None;
        };
        if let (StructuralTerm::Application { arguments, .. }, Some(inputs)) =
            (&mut value, call_values)
        {
            *arguments = inputs;
        }
        if let ExpressionNode::StructLiteral(literal) = node {
            let fields = expression_table(program).struct_fields(literal.fields);
            let constructed = structural_terms::constructor_literal_term(
                program,
                expression,
                literal,
                |declared, supplied| {
                    if let Some(field) = supplied {
                        let position = fields
                            .iter()
                            .position(|source| source.field_symbol == field.field_symbol)?;
                        return inputs
                            .get(position)
                            .and_then(|(_, value)| value.observed_value().cloned());
                    }
                    if !judge.runtime_body_values
                        || !crate::has_contents(
                            program,
                            declared.type_reference,
                            &[],
                            crate::value_custody::storage_contents::ContentsRequirement::BorrowFree,
                        )
                    {
                        return None;
                    }
                    // Default-domain admission is not a loan origin.
                    // Only finite borrow-free runtime zeros receive an
                    // opaque identity; erased terms and reference-bearing
                    // defaults still need their independent producers.
                    judge.capture_sequence += 1;
                    Some(StructuralTerm::CapturedValue {
                        statement: arena::Handle::invalid(),
                        expression,
                        callable: SymbolHandle::invalid(),
                        occurrence: judge.capture_sequence,
                        result_type: declared.type_reference,
                        integer_range: None,
                    })
                },
            );
            value = match constructed {
                Some(constructed) => constructed,
                None if judge.runtime_body_values => {
                    // A completed construction can contain an omitted
                    // established value with no mathematical denotation.
                    // Preserve its executed effects and a fresh result;
                    // an unrelated later predicate still sees its prefix.
                    judge.capture_unknown_value(expression, scope)?
                }
                None => return None,
            };
            if judge.runtime_body_values
                && let Some(reference) = judge.normal_value_type_reference(expression, scope)
                && crate::reference_shell(program, reference).is_none()
                && crate::field_domains::readable_nominal_definition(program, reference).is_some()
            {
                // This operation has completed: its children and the complete
                // construction obligation precede this cut. Retain validity
                // on a fresh value, never on a reconstructed constructor with
                // equal fields. Before-operation judgment cannot see this row.
                value =
                    judge.retain_completed_nominal_value(expression, reference, value, scope)?;
            }
        }
        if let ExpressionNode::Cast(cast) = node {
            let [(_, operand)] = inputs else {
                return None;
            };
            let source =
                crate::expression_result_type_reference(program, scope.0, scope.1, cast.value);
            // The operand has completed once. Carry its exact observed
            // value through the conversion, never reread its old home.
            value = match structural_terms::conversion_term(
                program,
                cast,
                source,
                operand.observed_value()?.clone(),
            ) {
                Some(converted) => converted,
                None if judge.runtime_body_values
                    && (cast.form.is_recast()
                        || program.primitive_type_reference(cast.target_type).is_some()) =>
                {
                    // All operands and effects have completed. A formed
                    // reference or conversion result may have no symbolic
                    // denotation. Keep its opaque descriptor and the
                    // executed prefix, without asserting referent contents.
                    // The storage-origin owner independently carries the
                    // backing used by subsequent reads and writes.
                    judge.capture_unknown_value(expression, scope)?
                }
                None => return None,
            };
        }
        if let ExpressionNode::ArrayLiteral(elements) = node {
            judge.intake_completed_array_elements(&value, elements.count(), inputs);
        }
        if let ExpressionNode::Binary(_) = node {
            let [(_, left), (_, right)] = inputs else {
                return None;
            };
            if let Some(term) = structural_terms::binary_term_in_scope(
                program,
                Some(scope),
                None,
                expression,
                left.observed_value()?.clone(),
                right.observed_value()?.clone(),
            ) {
                value = term;
            } else if !judge.runtime_body_values
                || !discarded_value_has_no_own_effects(program, scope, expression)
            {
                return None;
            }
            // A completed builtin may have no structural denotation
            // (for example nominal equality). Its fresh typed result
            // carries no operand equation, but cannot erase unrelated
            // prefix premises. Selected call effects retain their
            // separate operation owner and do not use this path.
        }
        if let ExpressionNode::Unary(_) = node {
            let [(_, operand)] = inputs else {
                return None;
            };
            if let Some(negated) = structural_terms::logical_not_term_in_scope(
                program,
                Some(scope),
                None,
                expression,
                operand.observed_value()?.clone(),
            ) {
                value = negated;
            }
        }
        if let ExpressionNode::Indexed(indexed) = node
            && program.expression_table.range_node(indexed.index).is_none()
            && !descriptor_completed
        {
            let [(_, collection), (_, index)] = inputs else {
                return None;
            };
            if let (Some(collection), Some(index)) =
                (collection.observed_value(), index.observed_value())
                && let Some(observed) = structural_terms::indexed_term_in_scope(
                    program,
                    Some(scope),
                    None,
                    expression,
                    collection.clone(),
                    index.clone(),
                )
                .filter(|value| value.observation_identity_key().is_some())
            {
                value = observed;
            } else if !judge.runtime_body_values {
                return None;
            }
            // A completed runtime read still has its captured scalar
            // value when its collection's contents have no structural
            // denotation. A stable descriptor alone cannot reconstruct
            // an element identity from an earlier storage epoch.
            // A location-only collection is an evaluation placeholder,
            // not a replacement for that captured scalar.
            if let Some(reference) =
                crate::expression_result_type_reference(program, scope.0, scope.1, expression)
            {
                judge.intake_admitted_value_predicates(reference, value.clone());
            }
        }
        judge.observe_completed_window(expression, inputs, scope, &value)?;
        if scalar_storage_read {
            // Completing a typed read observes its actual value even
            // when no mathematical entry value is known. Integers,
            // addresses, and IEEE floats share that coordinate, so an
            // exclusive loan retires the read only when it overlaps.
            // Source text is never its identity.
            // Reading a field through a pointer observes its contents now.
            // The pointer's projection may have an identity, but it is not a
            // snapshot of those contents and cannot carry their checked bounds.
            if value.observation_identity_key().is_none()
                || structural_terms::term_is_unobserved_descriptor(program, &value)
            {
                value = judge.capture_unknown_value(expression, scope)?;
            }
            if !receiver_changed {
                judge.intake_completed_storage_bounds(scope.0, scope.1, expression, &value);
                judge.observe_completed_storage_read(scope.0, scope.1, expression, value.clone());
            }
        }
        // A whole owned-value read captures its revision before a
        // named state can inspect its fields. A reference descriptor
        // does not capture the contents of its referent.
        if matches!(node, ExpressionNode::Name(_) | ExpressionNode::Member(_))
            && let Some(reference) =
                crate::expression_result_type_reference(program, scope.0, scope.1, expression)
            && program.primitive_type_reference(reference).is_none()
            && crate::reference_shell(program, reference).is_none()
        {
            if !receiver_changed
                && judge.runtime_body_values
                && crate::field_domains::readable_nominal_definition(program, reference).is_some()
                && judge.has_admitted_nominal_subject(expression, &value, reference)
            {
                // The actual read acquired this admitted value before any
                // synthetic transition/local binding. That new home must carry
                // the completed observation, rather than re-admit its type.
                value =
                    judge.retain_completed_nominal_value(expression, reference, value, scope)?;
            }
            judge.observe_completed_storage_read(scope.0, scope.1, expression, value.clone());
        }
        if let Some(actual_type) =
            crate::expression_result_type_reference(program, scope.0, scope.1, expression)
            && crate::fixed_array_type_extent(program, actual_type).is_some()
            && let Some(extent) = structural_terms::observed_collection_measure(
                program,
                actual_type,
                CollectionMeasure::Length,
                || None,
            )
        {
            // Capture immutable geometry on the completed actual before
            // a receiving slice formal attenuates its declared carrier.
            // This exact descriptor identity supplies no element facts,
            // initializer replay, or additional reference authority.
            let descriptor = value.clone().into_collection_descriptor();
            if descriptor.observation_identity_key().is_some() {
                judge.assume_normal_equation(
                    StructuralTerm::CollectionMeasure {
                        measure: CollectionMeasure::Length,
                        collection: Box::new(descriptor),
                    },
                    extent,
                );
            }
        }
        Some(NormalReturnPath::observed(value, judge))
    }
}
