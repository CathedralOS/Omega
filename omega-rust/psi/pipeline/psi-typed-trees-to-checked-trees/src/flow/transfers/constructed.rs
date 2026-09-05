//! Constructor fields retain value evidence at the new storage coordinate.
//! Only current literal values or live predicate facts transfer; declarations
//! alone do not establish the contents of a newly constructed field.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_constructed_field_values(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    statement: &StatementNode,
    expression: ExpressionHandle,
    destination: PlaceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<psi_facts::FactRef>,
) {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
    ) {
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
    let reference = match statement {
        StatementNode::LocalData(local) => Some(local.type_reference),
        StatementNode::Assignment(assignment) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            assignment.target,
        ),
        _ => None,
    };
    let Some(mut reference) = reference else {
        return;
    };
    loop {
        match program.type_reference_table.type_reference(reference) {
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                reference = *referee
            }
            _ => break,
        }
    }
    let Some(projections) = literal_value_projections(program, expression, reference, &[], true)
    else {
        return;
    };
    let active_facts: Vec<_> = contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .flat_map(|reference| {
            semantic
                .context_view(semantic.contexts.get(reference.context))
                .facts()
        })
        .copied()
        .collect();
    let destination_root = *semantic.places.get(destination);
    let destination_segments = semantic
        .place_segments
        .span_or_empty(destination_root.segments)
        .to_vec();
    for projection in projections {
        if !projection.remaining.is_empty() {
            continue;
        }
        let place = semantic.append_place(psi_facts::Place {
            root: destination_root.root,
            segments: HandleSpan::empty(),
        });
        for segment in destination_segments.iter().chain(&projection.destination) {
            semantic.push_place_segment(place, *segment);
        }
        if matches!(
            program.expression_table.expression(projection.expression),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::String(_)
        ) {
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::default(),
                payload: FactPayload::AssignedValue {
                    value: projection.expression,
                },
            });
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
        for fact in &active_facts {
            let payload = match fact.payload {
                FactPayload::AssignedValue { .. } => fact.payload,
                FactPayload::DomainMembership {
                    domain,
                    domain_symbol,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain,
                    domain_symbol,
                    ..
                } if program.domain_definitions().iter().any(|definition| {
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
                    }
                }
                _ => continue,
            };
            let FactPlace::Place(source_fact) = fact.place else {
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
    }
}
