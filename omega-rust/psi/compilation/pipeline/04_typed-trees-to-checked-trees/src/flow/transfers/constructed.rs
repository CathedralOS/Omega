//! Constructor fields retain value evidence at the new storage coordinate.
//! Owned children transfer their completed observations, not the constructor's
//! final context after later children have written their sources. Live views
//! retain store-time evidence instead. Declarations establish neither value.
use super::PlaceHandle;
use crate::flow::FlowBuildContext;
use crate::flow::literal_expression_projections;
use crate::flow::transfers::contextual_expression_place;
use crate::flow::transfers::projected;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{FlowConstraintKind, FlowOperandFact, FlowSemanticContextRef};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};

#[allow(clippy::too_many_arguments)]
pub(super) fn append_constructed_field_values(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    contexts: &mut FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    stored_value: arena::Handle<FlowOperandFact>,
    destination: PlaceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    if !contexts.control.operands.is_valid(stored_value) {
        return;
    }
    let capture = *contexts.control.operands.get(stored_value);
    if capture.children.is_empty() {
        // Whole stored values already use ordinary context transport. Only
        // separately evaluated children need distinct completion contexts.
        return;
    }
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = point
    else {
        return;
    };
    let reference = capture.type_reference;
    if !reference.is_valid() {
        return;
    }
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let Some(projections) =
        literal_expression_projections(program, capture.expression, reference, &[], true)
    else {
        return;
    };
    let destination_root = *semantic.places.get(destination);
    let destination_segments = semantic
        .place_segments
        .span_or_empty(destination_root.segments)
        .to_vec();
    for projection in projections {
        if !projection.remaining.is_empty() {
            continue;
        }
        let Some((child, child_type)) = crate::flow::operand_values::captured_operand_at_path(
            program,
            &contexts.control,
            &capture,
            reference,
            &projection.destination,
        ) else {
            continue;
        };
        if child.expression != projection.expression {
            continue;
        }
        let active = if crate::flow::operand_binds_live_view(program, child_type) {
            active
        } else {
            let captured_contexts = contexts
                .contexts
                .constraint_refs
                .span_or_empty(child.constraints)
                .iter()
                .filter_map(|reference| match reference.kind {
                    FlowConstraintKind::SemanticContext { context } => {
                        Some(FlowSemanticContextRef { context })
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            contexts
                .contexts
                .semantic_context_refs
                .insert_many(captured_contexts)
        };
        let active_facts = crate::flow::active_context_facts(contexts, semantic, active);
        let place = semantic.append_place(facts::Place {
            root: destination_root.root,
            segments: HandleSpan::empty(),
        });
        for segment in destination_segments.iter().chain(&projection.destination) {
            semantic.push_place_segment(place, *segment);
        }
        if crate::values::is_assignment_literal(program, projection.expression) {
            let fact = semantic.append_transfer_at_point(
                FactPlace::Place(place),
                point,
                FactPayload::AssignedValue {
                    value: projection.expression,
                },
            );
            semantic.append_ref(references, fact);
            continue;
        }
        let Some(source) = contextual_expression_place(
            program,
            semantic,
            machine_symbol,
            state_symbol,
            statement_index,
            projection.expression,
        ) else {
            continue;
        };
        let source_place = semantic.places.get(source);
        if !crate::flow::source_transfer_cases_selected(
            program,
            semantic,
            contexts,
            active,
            point,
            source_place.root,
            semantic
                .place_segments
                .span_or_empty(source_place.segments)
                .to_vec(),
        ) {
            continue;
        }
        for fact in &active_facts {
            let payload = match fact.payload {
                FactPayload::AssignedValue { .. } | FactPayload::AssignedScalarValue { .. } => {
                    fact.payload
                }
                facts::domain_membership_fields_pattern!(
                    domain,
                    domain_symbol,
                    semantic_domain
                ) if program.domain_definitions().iter().any(|definition| {
                    definition.symbol == domain_symbol
                        && definition.establishment_routes.is_empty()
                        && definition.alias.is_none()
                        && definition.predicate_body.is_present()
                }) =>
                {
                    FactPayload::DomainMembership {
                        value: ExpressionHandle::invalid(),
                        domain,
                        domain_symbol,
                        semantic_domain,
                    }
                }
                _ => continue,
            };
            let Some(source_fact) = fact.place.as_place() else {
                continue;
            };
            if !semantic.places_equal(source_fact, source) {
                continue;
            }
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: fact.evidence,
                payload,
            });
            semantic.append_ref(references, fact);
        }
        projected::append_copied_field_predicates(
            program, semantic, contexts, active, source, place, point, references,
        );
        if let Some(destination_type) = validation::project_type_reference_from_segments(
            program,
            reference,
            &projection.destination,
        ) {
            super::owned_qualifications::append_owned_qualification_transfer(
                program,
                semantic,
                contexts,
                active,
                source,
                place,
                destination_type,
                point,
                references,
            );
        }
    }
}
