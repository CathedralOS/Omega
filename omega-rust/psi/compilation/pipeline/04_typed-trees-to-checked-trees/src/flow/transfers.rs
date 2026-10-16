//! Statement transfers: after a `let` or an assignment, the facts that held
//! for the written value's source hold for the written place.
//!
//! The statement owner resolves its write target and narrows captured selectors
//! before retiring destination facts, then shares that completed coordinate
//! with `propagate_statement_transfers`. Its phases run in order:
//! `context_transport` carries each active context's source facts onto it;
//! `owned_qualifications`, `projected`, `constructed` and `byte_sequences`
//! re-anchor evidence below the destination from a moved owner, a copied
//! field, a reference result, a constructed record or a byte-sequence write;
//! `scalar_values` records the written scalar value and bounds;
//! `declared_domains` records the destination's declared domains; and the new
//! facts publish as one context per exact place.

use validation::CanonicalPlace;

use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::reference_spans;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowFacts, FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use facts::{PlaceHandle, QualificationCorrespondence, QualificationPayloadIdentity};
use symbols::SymbolHandle;

mod byte_sequences;
mod constructed;
mod context_transport;
mod declared_domains;
mod owned_qualifications;
mod projected;
mod scalar_values;
mod write_target;
pub(super) use write_target::{StatementWrite, narrow_pinned_selectors, statement_write};

pub(super) fn propagate_statement_transfers(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    statement_fact: arena::Handle<checked_trees::FlowStatementFact>,
    write: Option<StatementWrite>,
    assignment_source_contexts: HandleSpan<FlowSemanticContextRef>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some(write) = write else {
        return;
    };
    let source_expression = write.source_expression;
    let source_label = program.expression_table.display_name(source_expression);
    let source_place = write.source_place;

    let mut refs = HandleSpan::empty();
    let context_handles = crate::flow::active_context_handles(build, *active_contexts);
    let target_place = write.target_place;
    let point = ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    };
    let stable_value_target = write.stable_value_target;

    if !write.target_is_exact {
        // A weak store selects one captured alternative. Only membership
        // already live on a candidate can survive a proved domain-preserving
        // replacement; no candidate receives the new scalar value or a new
        // qualification merely because it was one possible destination.
        declared_domains::establish(
            program,
            semantic,
            build,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            target_place,
            false,
            write.candidate_targets,
            assignment_source_contexts,
            &mut refs,
        );
        if !refs.is_empty() {
            publish_statement_contexts(
                semantic,
                build,
                machine_symbol,
                state_symbol,
                statement_index,
                refs,
                active_contexts,
                active_constraints,
            );
        }
        return;
    }

    if stable_value_target {
        build
            .control
            .statements
            .get_mut(statement_fact)
            .stored_destination = target_place;
    }

    if let Some(source) = source_place {
        let source = semantic.places.get(source);
        let source = CanonicalPlace {
            root: source.root,
            segments: semantic
                .place_segments
                .span_or_empty(source.segments)
                .to_vec(),
        };
        if !crate::flow::place_cases_are_selected(
            program,
            semantic,
            &context_handles,
            machine_symbol,
            state_symbol,
            statement_index,
            &source,
        ) {
            return;
        }
    }

    context_transport::transport(
        program,
        semantic,
        build,
        &write,
        &source_label,
        stable_value_target,
        machine_symbol,
        state_symbol,
        statement_index,
        context_handles,
        &mut refs,
    );
    let write_target::StatementWrite {
        target_place,
        candidate_targets,
        ..
    } = write;

    if let Some(source_place) = source_place {
        // Expression flow already captured the declaration of the installed
        // value, distinguishing a referent store from reference rebinding.
        // Qualification transport uses that same type rather than rebuilding
        // the destination's access carrier after evaluation.
        let stored_value = build.control.statements.get(statement_fact).stored_value;
        let destination_type = build
            .control
            .operands
            .is_valid(stored_value)
            .then(|| build.control.operands.get(stored_value).type_reference)
            .filter(|reference| reference.is_valid());
        if let Some(destination_type) = destination_type {
            owned_qualifications::append_owned_qualification_transfer(
                program,
                semantic,
                build,
                assignment_source_contexts,
                source_place,
                target_place,
                destination_type,
                point,
                &mut refs,
            );
        }
        projected::append_copied_field_predicates(
            program,
            semantic,
            build,
            *active_contexts,
            source_place,
            target_place,
            point,
            &mut refs,
        );
    }

    // A reference result lends its proven referent to the binding:
    // `let room = room_mut(level)` makes `room` the storage the callee
    // selected, and the local write origins already track exactly which
    // caller place that is. An expression-rooted source has no canonical
    // storage of its own, so evidence below the referent re-anchors below the
    // binding; a symbol-rooted source was already transported above.
    if source_place.is_some_and(|place| {
        matches!(
            semantic.places.get(place).root,
            facts::PlaceRoot::Expression(_)
        )
    }) && let Some(referent_place) = projected::bound_reference_referent_place(
        program,
        semantic,
        build,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
        target_place,
    ) {
        projected::append_copied_field_predicates(
            program,
            semantic,
            build,
            *active_contexts,
            referent_place,
            target_place,
            point,
            &mut refs,
        );
    }

    if stable_value_target {
        let stored_value = build.control.statements.get(statement_fact).stored_value;
        constructed::append_constructed_field_values(
            program,
            semantic,
            build,
            assignment_source_contexts,
            stored_value,
            target_place,
            point,
            &mut refs,
        );
        byte_sequences::append_derived_predicates(
            program,
            semantic,
            build,
            assignment_source_contexts,
            source_expression,
            target_place,
            point,
            &mut refs,
        );
    }

    let scalar_value = scalar_values::capture_statement(
        program,
        borrow,
        semantic,
        build,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
        assignment_source_contexts,
    );
    let integer_bounds = if scalar_value.is_none() {
        scalar_values::capture_bounds(
            program,
            borrow,
            semantic,
            build,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            assignment_source_contexts,
        )
    } else {
        None
    };
    if let Some(assignment) = statement.as_assignment() {
        byte_sequences::append_element_replacement_predicates(
            program,
            semantic,
            build,
            assignment_source_contexts,
            machine_symbol,
            state_symbol,
            statement_index,
            assignment.target,
            source_expression,
            scalar_value.as_ref(),
            integer_bounds.as_ref(),
            point,
            &mut refs,
        );
    }

    if stable_value_target
        && (crate::values::is_assignment_literal(program, source_expression)
            || (matches!(
                program.expression_table.expression(source_expression),
                ExpressionNode::Call(_)
            )))
    {
        // A call is retained only as the exact source occurrence of this
        // completed assignment. It is not evaluated here. Consumers need its
        // checked call/argument/guarantee joins before it supplies a value;
        // ordinary assignment invalidation still retires this provenance.
        let fact = semantic.append_statement_transfer(
            FactPlace::Place(target_place),
            machine_symbol,
            state_symbol,
            statement_index,
            FactPayload::AssignedValue {
                value: source_expression,
            },
        );
        semantic.append_ref(&mut refs, fact);
    }

    if stable_value_target && let Some(bounds) = integer_bounds {
        let bounds = semantic.integer_ranges.append(bounds);
        let fact = semantic.append_statement_transfer(
            FactPlace::Place(target_place),
            machine_symbol,
            state_symbol,
            statement_index,
            FactPayload::AssignedIntegerBounds { bounds },
        );
        semantic.append_ref(&mut refs, fact);
    }

    if stable_value_target && let Some(value) = scalar_value {
        let value = semantic.scalar_values.append(value);
        let fact = semantic.append_statement_transfer(
            FactPlace::Place(target_place),
            machine_symbol,
            state_symbol,
            statement_index,
            FactPayload::AssignedScalarValue { value },
        );
        semantic.append_ref(&mut refs, fact);
    }

    declared_domains::establish(
        program,
        semantic,
        build,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
        target_place,
        true,
        candidate_targets,
        assignment_source_contexts,
        &mut refs,
    );

    if refs.is_empty() {
        return;
    }

    publish_statement_contexts(
        semantic,
        build,
        machine_symbol,
        state_symbol,
        statement_index,
        refs,
        active_contexts,
        active_constraints,
    );
}

/// Publish one statement's transferred facts as new active contexts, one per
/// exact place and observation kind. A tag survives a payload write even
/// when the full value at that same place no longer has its old contents.
fn publish_statement_contexts(
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    refs: HandleSpan<facts::FactRef>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    // One statement transports evidence for many independent storage
    // coordinates: a view binding re-anchors every element's declared fields
    // below the view. Invalidation drops a whole context once any of its
    // facts overlaps a write, so facts over distinct places take separate
    // contexts, as entry seeding already does. Discriminator observations
    // have an independent lifetime from contents at the same place.
    let point = ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    };
    let mut groups: Vec<(FactPlace, bool, Vec<facts::FactRef>)> = Vec::new();
    for reference in semantic.refs.span_or_empty(refs) {
        let fact = semantic.facts.get(reference.fact);
        let place = fact.place;
        let observes_tag = matches!(fact.payload, FactPayload::AssignedCase { .. });
        if let Some((_, _, group)) =
            groups
                .iter_mut()
                .find(|(candidate, candidate_observes_tag, _)| {
                    *candidate_observes_tag == observes_tag
                        && match (*candidate, place) {
                            (FactPlace::Place(left), FactPlace::Place(right)) => {
                                semantic.places_equal(left, right)
                            }
                            _ => *candidate == place,
                        }
                })
        {
            group.push(*reference);
        } else {
            groups.push((place, observes_tag, vec![*reference]));
        }
    }
    let mut next_contexts =
        retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
    let mut next_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
    for (_, _, group) in groups {
        let mut group_refs = HandleSpan::empty();
        for reference in group {
            semantic.refs.append_to_span(&mut group_refs, reference);
        }
        let context = semantic.append_context(point, group_refs);
        reference_spans::append_flow_reference(
            &mut build.contexts.semantic_context_refs,
            &mut next_contexts,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            &mut build.contexts.constraint_refs,
            &mut next_constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
    *active_contexts = next_contexts;
    *active_constraints = next_constraints;
}

#[allow(clippy::too_many_arguments)]
fn retain_qualification_correspondence(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    semantic: &mut FactPlan,
    source_fact: facts::FactHandle,
    destination_fact: facts::FactHandle,
    source_place: PlaceHandle,
    source_occurrence_place: PlaceHandle,
    destination_place: PlaceHandle,
    formation: ProgramPoint,
    destination_payload: FactPayload,
    evidence: QualificationEvidence,
) {
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = formation
    else {
        return;
    };
    if evidence.origin != language_semantics::QualificationEvidenceOrigin::CheckedTransformation
        || !exact_evidence_source(program, evidence)
        || !exact_statement_owner(program, machine_symbol, state_symbol)
        || !exact_structural_symbol_place(
            program,
            build,
            semantic,
            source_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !exact_structural_symbol_place(
            program,
            build,
            semantic,
            source_occurrence_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !exact_structural_symbol_place(
            program,
            build,
            semantic,
            destination_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !semantic.facts.is_valid(source_fact)
        || !semantic.facts.is_valid(destination_fact)
        || source_fact == destination_fact
        || source_fact.arena_index() >= destination_fact.arena_index()
        || source_place == destination_place
        || !semantic.places_equal(source_place, source_occurrence_place)
    {
        return;
    }
    let source = semantic.facts.get(source_fact);
    let destination = semantic.facts.get(destination_fact);
    if source.place != FactPlace::Place(source_place)
        || source.evidence != evidence
        || destination.place != FactPlace::Place(destination_place)
        || destination.point != formation
        || destination.origin != FactOrigin::StatementTransfer
        || destination.evidence != evidence
        || destination.payload != destination_payload
    {
        return;
    }
    let Some(payload) = QualificationPayloadIdentity::from_fact_payload(source.payload) else {
        return;
    };
    if QualificationPayloadIdentity::from_fact_payload(destination_payload) != Some(payload)
        || !exact_qualification_payload(program, payload)
    {
        return;
    }
    semantic.append_qualification_correspondence(QualificationCorrespondence {
        source_fact,
        destination_fact,
        source_occurrence_place,
        source_place,
        destination_place,
        formation,
        payload,
        evidence,
    });
}

fn exact_qualification_payload(
    program: &dyn ::validation::CheckingInput,
    payload: QualificationPayloadIdentity,
) -> bool {
    match payload {
        QualificationPayloadIdentity::DomainMembership {
            domain,
            domain_symbol,
            semantic_domain,
        } => {
            (!semantic_domain.is_valid()
                || program.semantic_domains.name(semantic_domain).is_some())
                && domain_symbol.is_valid()
                && program.symbols.get(domain_symbol).kind == symbols::SymbolKind::Domain
                && program.domain_path_members.span(domain).is_some()
        }
        QualificationPayloadIdentity::CarryPermission { .. }
        | QualificationPayloadIdentity::CarryOrigin => true,
    }
}

fn exact_evidence_source(
    program: &dyn ::validation::CheckingInput,
    evidence: QualificationEvidence,
) -> bool {
    evidence.source_symbol.is_valid()
        && evidence.requirement_symbol == SymbolHandle::invalid()
        && matches!(
            program.symbols.get(evidence.source_symbol).kind,
            symbols::SymbolKind::Machine | symbols::SymbolKind::Operator
        )
}

fn exact_structural_symbol_place(
    program: &dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext,
    semantic: &FactPlan,
    handle: PlaceHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
) -> bool {
    if !semantic.places.is_valid(handle) {
        return false;
    }
    let place = semantic.places.get(handle);
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    if !root.is_valid() {
        return false;
    }
    let Some(segments) = semantic.place_segments.span(place.segments) else {
        return false;
    };
    let Some(mut current) = *build
        .correspondence_root_types
        .entry((
            machine_symbol,
            state_symbol,
            formation_statement_index,
            root,
        ))
        .or_insert_with(|| {
            correspondence_root_type_reference(
                program,
                machine_symbol,
                state_symbol,
                formation_statement_index,
                root,
            )
        })
    else {
        return false;
    };
    let mut selected_variant = None;
    for segment in segments {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                if !symbol.is_valid()
                    || program.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                {
                    return false;
                }
                let Some(data) = validation::declared_data_type(program, current) else {
                    return false;
                };
                let field = if let Some(variant_symbol) = selected_variant.take() {
                    program.data_members(data).iter().find_map(|member| {
                        let variant = member.as_variant()?;
                        (variant.symbol == variant_symbol)
                            .then(|| program.data_payload_field_by_symbol(variant, *symbol))?
                    })
                } else {
                    program.data_field(data, *symbol)
                };
                let Some(field) = field else {
                    return false;
                };
                current = field.type_reference;
            }
            facts::PlaceSegment::Case { variant } => {
                if selected_variant.is_some()
                    || !variant.is_valid()
                    || program.symbols.get(*variant).kind != symbols::SymbolKind::Variant
                {
                    return false;
                }
                let Some(data) = validation::declared_data_type(program, current) else {
                    return false;
                };
                if !program.data_members(data).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Variant(candidate)
                        if candidate.symbol == *variant)
                }) {
                    return false;
                }
                selected_variant = Some(*variant);
            }
            facts::PlaceSegment::FixedIndex { index } => {
                if selected_variant.is_some() {
                    return false;
                }
                current = program.type_reference_table.peeled_type_reference(current);
                match program.type_reference_table.type_reference(current) {
                    typed_trees::types::TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(length),
                    } if *index < *length => {
                        current = *element_type;
                    }
                    _ => return false,
                }
            }
            facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::OpaqueRange
            | facts::PlaceSegment::Index { .. } => {
                return false;
            }
        }
    }
    true
}

fn correspondence_root_type_reference(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
    root: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_in_machine(machine, state_symbol)?;
    match program.symbols.get(root).kind {
        symbols::SymbolKind::Parameter
            if matches!(
                program.symbols.get(root).parent,
                parent if parent == machine_symbol || parent == state_symbol
            ) =>
        {
            program
                .state_parameter_by_symbol(state, root)
                .map(|parameter| parameter.type_reference)
        }
        symbols::SymbolKind::Local
            if program.symbols.get(root).parent == state_symbol
                && formation_statement_index
                    < program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len() =>
        {
            program
                .state_local_declaration(state, root)
                .filter(|&(ordinal, _)| ordinal < formation_statement_index)
                .map(|(_, local)| local.type_reference)
        }
        _ => None,
    }
}

fn exact_statement_owner(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    machine_symbol.is_valid()
        && state_symbol.is_valid()
        && program.symbols.get(machine_symbol).kind == symbols::SymbolKind::Machine
        && program.symbols.get(state_symbol).kind == symbols::SymbolKind::State
        && program.symbols.get(state_symbol).parent == machine_symbol
}

fn contextual_expression_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    let _ = machine_symbol;
    crate::semantic::places::canonical_place_to_fact_place_in_state(
        program,
        semantic,
        state_symbol,
        statement_index,
        expression,
    )
}

/// A completed value is interpreted in its source and installed destination
/// declarations. An alias or recast alone cannot equate different carriers.
fn completed_value_carriers_agree(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    source: &CanonicalPlace,
    target: &CanonicalPlace,
) -> bool {
    let type_at = |place, position| {
        let reference =
            validation::canonical_place_type_reference(program, state, position, place)?;
        let reference = validation::reference_shell(program, reference)
            .map_or(reference, |shell| shell.referee);
        Some(program.normalized_type_identity(reference))
    };
    // Only the installed destination's current declaration becomes visible.
    type_at(source, statement_index)
        .zip(type_at(target, statement_index.saturating_add(1)))
        .is_some_and(|(source, target)| source == target)
}
