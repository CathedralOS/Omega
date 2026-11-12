/*
    Statement transfers: after a `let` or an assignment, the facts that held
    for the written value's source hold for the written place.

    The statement owner resolves its write target and narrows captured selectors
    before retiring destination facts, then shares that completed coordinate
    with `propagate_statement_transfers`. Its phases run in order:
    `context_transport` carries each active context's source facts onto it;
    `owned_qualifications`, `projected`, `constructed` and `byte_sequences`
    re-anchor evidence below the destination from a moved owner, a copied
    field, a reference result, a constructed record or a byte-sequence write;
    `scalar_values` records the written scalar value and bounds;
    `declared_domains` records the destination's declared domains; and the new
    facts publish separately for each exact place and normalized domain atom.
    A captured address survives RHS execution, but its symbolic coordinate may
    cross a reference slot that the RHS replaced. Every kind of destination
    evidence requires that coordinate correspondence, not just scalar values.
    Refusing it leaves the runtime store capture and invalidation unchanged.
*/

use crate::published::semantic::places::append_place;
use crate::published::semantic::places::canonical_place_to_fact_place_in_state;
use crate::values::is_assignment_literal;
use validation::CanonicalPlace;

use crate::flow::append_constraint_ref;
use crate::flow::reference_spans;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use crate::flow::{
    FlowBuildContext, active_context_handles, place_cases_are_selected, state_statements,
};
use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowFacts, FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use facts::{PlaceHandle, QualificationCorrespondence, QualificationPayloadIdentity};
use symbols::SymbolHandle;

mod byte_sequences {
    /*
        Byte operations derive predicates from their operands at assignment time.
        The destination retains a materialized-value fact, not expressions that
        would read the mutable operands again at a later contract boundary.
    */
    use super::PlaceHandle;
    use crate::flow::FlowBuildContext;
    use crate::flow::canonical_place_from_semantic_place;
    use crate::flow::expressions;
    use crate::flow::transfers::contextual_expression_place;
    use arena::HandleSpan;
    use checked_trees::FlowSemanticContextRef;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use facts::{Fact, FactPayload, FactPlace, FactPlan, ProgramPoint};
    use symbols::SymbolHandle;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_derived_predicates(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        expression: ExpressionHandle,
        destination: PlaceHandle,
        point: ProgramPoint,
        references: &mut HandleSpan<facts::FactRef>,
    ) {
        let window = validation::subslice_expression(program, expression);
        let operation = window.unwrap_or(expression);
        let byte_operation = window.is_some()
            || matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Binary(binary)
                    if binary.operator == typed_trees::expression::BinaryOperator::Add
            );
        if !byte_operation || !crate::values::operator_is_builtin(contexts.operators, operation) {
            return;
        }
        let active_facts: Vec<_> = crate::flow::active_context_facts(contexts, semantic, active);
        for domain in program.domain_definitions() {
            if !domain.establishment_routes.is_empty()
                || !typed_trees::domain::index_parameters(program, domain).is_empty()
                || domain.alias.is_some()
                || !domain.predicate_presence.is_present()
                || !value_proves_predicate(
                    program,
                    semantic,
                    contexts.operators,
                    &active_facts,
                    expression,
                    domain.symbol,
                    point,
                )
            {
                continue;
            }
            let fact = semantic.append_transfer_at_point(
                FactPlace::Place(destination),
                point,
                FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain: HandleSpan::empty(),
                    domain_symbol: domain.symbol,
                    semantic_domain: language_semantics::SemanticDomainId::NULL,
                },
            );
            semantic.append_ref(references, fact);
        }
    }

    fn value_proves_predicate(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        operators: &checked_trees::CheckedOperatorFacts,
        active: &[Fact],
        expression: ExpressionHandle,
        domain: SymbolHandle,
        point: ProgramPoint,
    ) -> bool {
        if validation::field_domains::string_literal_expression_grants_domain(
            program, expression, domain,
        ) {
            return true;
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && binary.operator == typed_trees::expression::BinaryOperator::Add
            && crate::values::operator_is_builtin(operators, expression)
        {
            return validation::field_domains::domain_is_concat_preserving(program, domain)
                && value_proves_predicate(
                    program,
                    semantic,
                    operators,
                    active,
                    binary.left,
                    domain,
                    point,
                )
                && value_proves_predicate(
                    program,
                    semantic,
                    operators,
                    active,
                    binary.right,
                    domain,
                    point,
                );
        }
        if let Some(window) = validation::subslice_expression(program, expression)
            && let Some(indexed) = program.expression_table.indexed_node(window)
            && crate::values::operator_is_builtin(operators, window)
        {
            return validation::field_domains::domain_is_subslice_preserving(program, domain)
                && value_proves_predicate(
                    program,
                    semantic,
                    operators,
                    active,
                    indexed.collection,
                    domain,
                    point,
                );
        }
        let ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } = point
        else {
            return false;
        };
        let Some(place) = contextual_expression_place(
            program,
            semantic,
            machine_symbol,
            state_symbol,
            statement_index,
            expression,
        ) else {
            return false;
        };
        active.iter().any(|fact| {
            let Some(source) = fact.place.as_place() else {
                return false;
            };
            if !semantic.places_equal(source, place) {
                return false;
            }
            match fact.payload {
                facts::domain_membership_pattern!(domain_symbol, semantic_domain) => {
                    index_free_membership(program, domain_symbol, semantic_domain)
                        && validation::field_domains::domain_byte_predicate(program, domain_symbol)
                            == validation::field_domains::domain_byte_predicate(program, domain)
                }
                FactPayload::AssignedValue { value } => {
                    validation::field_domains::string_literal_expression_grants_domain(
                        program, value, domain,
                    )
                }
                FactPayload::BytePredicate { predicate } => {
                    validation::field_domains::domain_byte_predicate(program, domain)
                        .is_some_and(|required| predicate.implies(required))
                }
                _ => false,
            }
        })
    }

    /*
        An indexed store replaces exactly ONE byte. A PER-BYTE character class
        (`no_nul`, `ascii_only`) survives that replacement when the stored byte is
        itself in the class, so whole-carrier evidence outlives a write that retires
        the exact `AssignedValue` snapshot the carrier had before it.

        `valid_utf8` is deliberately NOT preserved here: an ASCII byte can split a
        multi-byte scalar, so UTF-8 is reachable only as a CONSEQUENCE of the
        surviving ASCII class (`ByteSequencePredicate::implies`), never as a class
        preserved in its own right. The premise is read from the PRE-mutation
        contexts, so consecutive stores chain: each one re-proves the class from the
        class the previous store left behind.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_element_replacement_predicates(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &mut FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        target: ExpressionHandle,
        target_place: PlaceHandle,
        source_expression: ExpressionHandle,
        scalar_value: Option<&facts::ScalarValue>,
        integer_bounds: Option<&facts::IntegerRange>,
        point: ProgramPoint,
        references: &mut HandleSpan<facts::FactRef>,
    ) {
        if program.expression_table.indexed_node(target).is_none() {
            return;
        }
        let active_facts: Vec<_> = crate::flow::active_context_facts(contexts, semantic, active);
        let written = *semantic.places.get(target_place);
        let segments = semantic.place_segments.span_or_empty(written.segments);
        let Some((
            facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
            parent,
        )) = segments.split_last()
        else {
            return;
        };
        let parent = parent.to_vec();
        let carrier =
            crate::published::semantic::places::append_place(semantic, written.root, &parent);
        if !semantic
            .place_segments
            .span_or_empty(semantic.places.get(carrier).segments)
            .iter()
            .all(crate::flow::fixed_selection_segment)
        {
            return;
        }
        let byte = match scalar_value {
            Some(facts::ScalarValue::Integer(value)) => {
                value.to_i64().and_then(|value| u8::try_from(value).ok())
            }
            _ => None,
        };
        let byte = byte.or_else(|| {
            replacement_byte(
                program,
                semantic,
                contexts,
                active,
                machine_symbol,
                state_symbol,
                statement_index,
                source_expression,
            )
        });
        let bounds = byte.map(|byte| (byte, byte)).or_else(|| {
            let bounds = integer_bounds?;
            let minimum = u8::try_from(bounds.minimum.to_i64()?).ok()?;
            let maximum = u8::try_from(bounds.maximum.to_i64()?).ok()?;
            (minimum <= maximum).then_some((minimum, maximum))
        });
        let source_place = contextual_expression_place(
            program,
            semantic,
            machine_symbol,
            state_symbol,
            statement_index,
            source_expression,
        );

        let source_element_facts = expressions(program)
            .indexed_node(source_expression)
            .filter(|_| crate::values::operator_is_builtin(contexts.operators, source_expression))
            .and_then(|indexed| {
                contextual_expression_place(
                    program,
                    semantic,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    indexed.collection,
                )
            })
            .map(|place| facts_about_place(program, semantic, &active_facts, place));
        let mut source_facts = None;
        let stored_classes: Vec<_> = validation::field_domains::ByteSequencePredicate::ALL
            .into_iter()
            .map(|predicate| {
                let stored = predicate.is_subslice_preserving()
                    && source_element_facts
                        .as_ref()
                        .is_some_and(|facts| carrier_proves_predicate(program, facts, predicate))
                    || bounds
                        .is_some_and(|(minimum, maximum)| predicate.holds_for(&[minimum, maximum]))
                    || source_place.is_some_and(|place| {
                        let facts = source_facts.get_or_insert_with(|| {
                            facts_about_place(program, semantic, &active_facts, place)
                        });
                        carrier_proves_predicate(program, facts, predicate)
                    });
                (predicate, stored)
            })
            .collect();
        let stored_in = |predicate: validation::field_domains::ByteSequencePredicate| {
            stored_classes
                .iter()
                .any(|(class, stored)| *class == predicate && *stored)
        };
        let ascii_stored = stored_in(validation::field_domains::ByteSequencePredicate::AsciiOnly);
        let mut carrier_facts = None;
        let mut carrier_memberships = None;
        for predicate in validation::field_domains::ByteSequencePredicate::ALL {
            let byte_in_class = if predicate.is_subslice_preserving() {
                stored_in(predicate)
            } else if predicate == validation::field_domains::ByteSequencePredicate::ValidUtf8 {
                ascii_stored && {
                    let facts = carrier_facts.get_or_insert_with(|| {
                        facts_about_place(program, semantic, &active_facts, carrier)
                    });
                    carrier_proves_predicate(
                        program,
                        facts,
                        validation::field_domains::ByteSequencePredicate::AsciiOnly,
                    )
                }
            } else {
                false
            };
            if !byte_in_class {
                continue;
            }
            let facts = carrier_facts.get_or_insert_with(|| {
                facts_about_place(program, semantic, &active_facts, carrier)
            });
            if !carrier_proves_predicate(program, facts, predicate) {
                continue;
            }
            let fact = semantic.append_transfer_at_point(
                FactPlace::Place(carrier),
                point,
                FactPayload::BytePredicate { predicate },
            );
            semantic.append_ref(references, fact);
            /*
                The store just re-proved `predicate` for the whole carrier, so a
                domain that is exactly that predicate still holds. Keep the
                carrier's own membership fact live instead of leaving only the
                class: call restoration and the lender check accept nothing but
                the membership's exact instance. This reactivates the fact the
                carrier already had; a carrier without one gains none, and a store
                that reaches no class above retires it as every other write does.
            */
            let memberships = carrier_memberships.get_or_insert_with(|| {
                memberships_about_place(program, semantic, contexts, active, carrier)
            });
            for (membership, domain_symbol) in memberships.iter() {
                if validation::field_domains::domain_byte_predicate(program, *domain_symbol)
                    == Some(predicate)
                {
                    semantic.append_ref(references, *membership);
                }
            }
        }
    }

    /*
        The live index-free memberships recorded on `carrier`'s own storage, with
        their fact identities so a store can keep one without issuing another.
    */
    fn memberships_about_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        carrier: PlaceHandle,
    ) -> Vec<(facts::FactHandle, symbols::SymbolHandle)> {
        let canonical = |place| {
            canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
        };
        let Some(carrier) = canonical(carrier) else {
            return Vec::new();
        };
        let active = crate::flow::active_context_handles(&contexts.contexts, active);
        semantic
            .unique_context_facts(active)
            .into_iter()
            .filter_map(|handle| {
                let fact = semantic.facts.get(handle);
                let facts::domain_membership_pattern!(domain_symbol, semantic_domain) =
                    fact.payload
                else {
                    return None;
                };
                let FactPlace::Place(place) = fact.place else {
                    return None;
                };
                (index_free_membership(program, domain_symbol, semantic_domain)
                    && canonical(place).as_ref() == Some(&carrier))
                .then_some((handle, domain_symbol))
            })
            .collect()
    }

    /* Read captured scalar values without replaying mutable operand expressions. */
    #[allow(clippy::too_many_arguments)]
    fn replacement_byte(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        source_expression: ExpressionHandle,
    ) -> Option<u8> {
        let literal = match program.expression_table.expression(source_expression) {
            ExpressionNode::Integer(value) => value.value_i64(),
            _ => None,
        };
        let value = literal.or_else(|| {
            let place = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                source_expression,
            )?;
            let subject =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))?;
            let facts::ScalarValue::Integer(value) = crate::values::scalar_value_at_place(
                program,
                semantic,
                contexts.semantic_context_views(semantic, active),
                &subject,
            )?
            else {
                return None;
            };
            value.to_i64()
        })?;
        u8::try_from(value).ok()
    }

    /*
        Whether the carrier provably satisfies `predicate` BEFORE the store, from an
        exact literal snapshot, an already-proved per-byte class, or a declared
        domain whose own predicate implies it.
    */
    pub(in crate::flow) fn carrier_proves_predicate(
        program: &dyn ::validation::CheckingInput,
        carrier_facts: &[Fact],
        predicate: validation::field_domains::ByteSequencePredicate,
    ) -> bool {
        carrier_facts.iter().any(|fact| match fact.payload {
            FactPayload::AssignedValue { value } => {
                typed_trees::byte_predicates::literal_bytes(program, value)
                    .is_some_and(|bytes| predicate.holds_for(&bytes))
            }
            FactPayload::BytePredicate { predicate: proved } => proved.implies(predicate),
            facts::domain_membership_pattern!(domain_symbol, semantic_domain) => {
                index_free_membership(program, domain_symbol, semantic_domain)
                    && validation::field_domains::domain_byte_predicate(program, domain_symbol)
                        .is_some_and(|proved| proved.implies(predicate))
            }
            _ => false,
        })
    }

    /*
        The active facts about `carrier`'s canonical storage, resolved once per
        carrier so each per-predicate query reads only them instead of
        re-canonicalizing every active fact per predicate. State inputs use one
        attached storage root; authored accesses may still use the current self
        parameter or an inherited field root.
    */
    fn facts_about_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        active: &[Fact],
        carrier: PlaceHandle,
    ) -> Vec<Fact> {
        let canonical = |place| {
            let place =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))?;

            Some(place)
        };
        let Some(carrier) = canonical(carrier) else {
            return Vec::new();
        };
        active
            .iter()
            .filter(|fact| {
                matches!(fact.place, FactPlace::Place(place) if canonical(place).as_ref() == Some(&carrier))
            })
            .copied()
            .collect()
    }

    fn index_free_membership(
        program: &dyn ::validation::CheckingInput,
        symbol: symbols::SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> bool {
        program.domain_definition(symbol).is_some_and(|domain| {
            typed_trees::domain::index_parameters(program, domain).is_empty()
                && (!semantic_domain.is_valid() || semantic_domain == domain.semantic_id)
        })
    }
}
pub(super) use byte_sequences::carrier_proves_predicate;
mod constructed {
    /*
        Constructor fields retain value evidence at the new storage coordinate.
        Owned children transfer their completed observations, not the constructor's
        final context after later children have written their sources. Live views
        retain store-time evidence instead. Declarations establish neither value.
    */
    use super::PlaceHandle;
    use crate::flow::FlowBuildContext;
    use crate::flow::literal_expression_projections;
    use crate::flow::transfers::contextual_expression_place;
    use crate::flow::transfers::projected;
    use arena::HandleSpan;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::{FlowConstraintKind, FlowOperandFact, FlowSemanticContextRef};
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};

    /*
        A routed membership may re-anchor onto the constructed field only when the
        source fact carries real issuance evidence — an authorized route, a checked
        validation, an admitted boundary receipt, or an already-conserved or
        propagated claim. `None` and vacuous evidence name no provenance, so a
        store cannot forward them into a domain that `established by` reserves.
    */
    fn routed_membership_transferable(evidence: facts::QualificationEvidence) -> bool {
        use language_semantics::QualificationEvidenceOrigin::*;
        matches!(
            evidence.origin,
            AuthorizedRouteEstablishment
                | CheckedValidation
                | AdmittedReceipt
                | CheckedTransformation
                | Propagated
        )
    }

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
        if capture.constructed_children().is_empty() {
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
                    FactPayload::AssignedValue { .. }
                    | FactPayload::AssignedScalarValue { .. }
                    | FactPayload::AssignedValueCapture { .. }
                    | FactPayload::FloatPredicate { .. } => fact.payload,
                    facts::domain_membership_pattern!(domain, domain_symbol, semantic_domain)
                        if program
                            .domain_definition(domain_symbol)
                            .is_some_and(|definition| {
                                definition.alias.is_none()
                                    && if definition.establishment_routes.is_empty() {
                                        definition.predicate_presence.is_present()
                                    } else {
                                        routed_membership_transferable(fact.evidence)
                                    }
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
        }
    }
}
mod context_transport {
    /*
        Carrying each active context's facts about the written value's source
        onto the written place: assigned cases and values, byte predicates and
        qualifications, each with its source correspondence retained.
    */
    use super::{
        ExpressionHandle, Fact, FactOrigin, FactPayload, FactPlace, FactPlan, HandleSpan,
        ProgramPoint, SymbolHandle, retain_qualification_correspondence,
    };
    use crate::flow::canonical_place_from_semantic_place;
    use crate::flow::canonical_places_agree;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn transport(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        write: &super::write_target::StatementWrite,
        completed_source: Option<typed_trees::types::TypeReferenceHandle>,
        stable_value_target: bool,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        context_handles: Vec<facts::FactContextHandle>,
        refs: &mut HandleSpan<facts::FactRef>,
    ) {
        if context_handles.is_empty() {
            return;
        }
        let target_place = write.target_place;
        let source_place = write.source_place;
        let source_value_place = source_place.and_then(|source| {
            canonical_place_from_semantic_place(program, semantic, semantic.places.get(source))
        });
        let mut value_carriers_agree: Option<bool> = None;
        for context_handle in context_handles {
            let context = semantic.contexts.get(context_handle);
            let facts_to_transfer: Vec<_> = semantic
                .refs
                .span_or_empty(context.facts)
                .iter()
                .filter_map(|reference| {
                    let fact = *semantic.facts.get(reference.fact);
                    let matches_source = |fact_place| {
                        source_value_place.as_ref().is_some_and(|source| {
                            canonical_place_from_semantic_place(
                                program,
                                semantic,
                                semantic.places.get(fact_place),
                            )
                            .is_some_and(|place| canonical_places_agree(program, &place, source))
                        })
                    };
                    /*
                        A zero-formed array element read as the source moves the
                        one tag its `ZeroCaseElements` fact states for that element.
                    */
                    let (payload, exact_source) = match fact.payload {
                        FactPayload::ZeroCaseElements { .. } => {
                            let variant = source_value_place.as_ref().and_then(|source| {
                                crate::flow::zero_case_at(program, semantic, &fact, source)
                            })?;
                            (
                                FactPayload::CaseValue {
                                    variant,
                                    value: true,
                                },
                                true,
                            )
                        }
                        payload => (payload, false),
                    };
                    match payload {
                        FactPayload::CaseValue { .. }
                        | FactPayload::AssignedValue { .. }
                        | FactPayload::AssignedScalarValue { .. }
                        | FactPayload::AssignedIntegerBounds { .. }
                        | FactPayload::AssignedValueCapture { .. }
                        | FactPayload::BytePredicate { .. }
                        | FactPayload::FloatPredicate { .. } => {
                            if !stable_value_target {
                                return None;
                            }
                            let carriers_agree = *value_carriers_agree.get_or_insert_with(|| {
                                source_place.is_some_and(|source| {
                                    let source = canonical_place_from_semantic_place(
                                        program,
                                        semantic,
                                        semantic.places.get(source),
                                    );
                                    let target = canonical_place_from_semantic_place(
                                        program,
                                        semantic,
                                        semantic.places.get(target_place),
                                    );
                                    source.zip(target).is_some_and(|(source, target)| {
                                        super::completed_value_carriers_agree(
                                            program,
                                            state_symbol,
                                            statement_index,
                                            &source,
                                            &target,
                                            completed_source,
                                        )
                                    })
                                })
                            });
                            if !carriers_agree {
                                return None;
                            }
                            let fact_place = fact.place.as_place()?;
                            (exact_source || matches_source(fact_place)).then_some((
                                payload,
                                fact.evidence,
                                None,
                            ))
                        }
                        facts::domain_membership_pattern!(
                            domain,
                            domain_symbol,
                            semantic_domain
                        ) => {
                            let fact_place = fact.place.as_place()?;
                            let whole_value = matches_source(fact_place);
                            if whole_value
                                && super::owned_qualifications::explicitly_erased(
                                    program,
                                    write.source_expression,
                                    domain_symbol,
                                    semantic_domain,
                                )
                            {
                                return None;
                            }
                            matches_source(fact_place).then_some((
                                FactPayload::DomainMembership {
                                    value: ExpressionHandle::invalid(),
                                    domain,
                                    domain_symbol,
                                    semantic_domain,
                                },
                                fact.evidence,
                                Some((reference.fact, fact_place)),
                            ))
                        }
                        facts::carry_permission_pair_pattern!(permission, ..) => {
                            let fact_place = fact.place.as_place()?;
                            matches_source(fact_place).then_some((
                                FactPayload::CarryPermission {
                                    value: ExpressionHandle::invalid(),
                                    permission,
                                },
                                fact.evidence,
                                Some((reference.fact, fact_place)),
                            ))
                        }
                        FactPayload::CarryOrigin { .. } => {
                            let fact_place = fact.place.as_place()?;
                            matches_source(fact_place).then_some((
                                FactPayload::CarryOrigin {
                                    value: ExpressionHandle::invalid(),
                                },
                                fact.evidence,
                                Some((reference.fact, fact_place)),
                            ))
                        }
                        FactPayload::BooleanExpression(expression) => {
                            (expression == write.source_expression).then_some((
                                FactPayload::BooleanExpression(expression),
                                fact.evidence,
                                None,
                            ))
                        }
                        FactPayload::ContractBooleanExpression {
                            expression,
                            instantiated,
                            ..
                        } if !instantiated.is_valid() => (expression == write.source_expression)
                            .then_some((
                                FactPayload::BooleanExpression(expression),
                                fact.evidence,
                                None,
                            )),
                        _ => None,
                    }
                })
                .collect();

            for (payload, evidence, source) in facts_to_transfer {
                let fact = semantic.append_fact(Fact {
                    place: FactPlace::Place(target_place),
                    point: ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    },
                    origin: FactOrigin::StatementTransfer,
                    evidence,
                    payload,
                });
                semantic.append_ref(refs, fact);
                if let Some((source_fact, source_fact_place)) = source
                    && let Some(source_occurrence_place) = source_place
                {
                    retain_qualification_correspondence(
                        program,
                        semantic,
                        source_fact,
                        fact,
                        source_fact_place,
                        source_occurrence_place,
                        target_place,
                        ProgramPoint::Statement {
                            machine_symbol,
                            state_symbol,
                            statement_index,
                        },
                        payload,
                        evidence,
                    );
                }
            }
        }
    }
}
mod declared_domains {
    /*
        Declared domains a write establishes: initializing or assigning a
        domain-refined place records membership in each declared domain that
        needs no provenance, and a write through a reference local keeps each
        candidate's membership exactly when it held before the write.
    */
    use facts::FactRef;
    use facts::PlaceSegment;
    use language_semantics::SemanticDomainId;
    use validation::field_domains::assignment_target_domain_identities;
    use validation::field_domains::assignment_target_type_reference;
    use validation::field_domains::declared_type_domain_rows;
    use validation::field_domains::domain_constraint_identities;
    use validation::field_domains::domain_requires_provenance;
    use validation::{CanonicalPlace, find_state_in_machine};

    use super::{
        ExpressionHandle, Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FlowBuildContext,
        FlowSemanticContextRef, HandleSpan, PlaceHandle, ProgramPoint, StatementNode, SymbolHandle,
    };

    #[allow(clippy::too_many_arguments)]
    pub(super) fn establish(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &FlowBuildContext,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        target_place: PlaceHandle,
        target_is_exact: bool,
        mut candidate_targets: Vec<CanonicalPlace>,
        assignment_source_contexts: HandleSpan<FlowSemanticContextRef>,
        refs: &mut HandleSpan<FactRef>,
    ) {
        let declared_target_domains = match statement {
            StatementNode::Assignment(assignment) => {
                match (
                    program.machine_by_symbol(machine_symbol),
                    find_state_in_machine(program, machine_symbol, state_symbol),
                ) {
                    (Some(machine), Some(state)) => assignment_target_domain_identities(
                        program,
                        machine,
                        state,
                        assignment.target,
                    ),
                    _ => Vec::new(),
                }
            }
            StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                domain_constraint_identities(program, local.type_reference)
            }
            _ => Vec::new(),
        };
        let declared_target_domains: Vec<_> = declared_target_domains
            .into_iter()
            .filter(|(symbol, _)| !domain_requires_provenance(program, *symbol))
            .collect();
        let membership_fact =
            |place: PlaceHandle, domain_symbol: SymbolHandle, semantic_domain: SemanticDomainId| {
                Fact {
                    place: FactPlace::Place(place),
                    point: ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    },
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::checked_validation_evidence(state_symbol),
                    payload: FactPayload::DomainMembership {
                        value: ExpressionHandle::invalid(),
                        domain: HandleSpan::empty(),
                        domain_symbol,
                        semantic_domain,
                    },
                }
            };
        for (domain_symbol, semantic_domain) in
            declared_target_domains.iter().filter(|_| target_is_exact)
        {
            let fact = semantic.append_fact(membership_fact(
                target_place,
                *domain_symbol,
                *semantic_domain,
            ));
            semantic.append_ref(refs, fact);
        }
        if target_is_exact {
            let declared_type = match statement {
                StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                    Some(local.type_reference)
                }
                StatementNode::Assignment(assignment) => program
                    .machine_by_symbol(machine_symbol)
                    .zip(find_state_in_machine(program, machine_symbol, state_symbol))
                    .and_then(|(machine, state)| {
                        assignment_target_type_reference(program, machine, state, assignment.target)
                    }),
                _ => None,
            };
            if let Some(reference) = declared_type {
                let rows = declared_type_domain_rows(program, reference);
                if !rows.is_empty() {
                    let target = semantic.places.get(target_place);
                    let root = target.root;
                    let prefix = semantic
                        .place_segments
                        .span_or_empty(target.segments)
                        .to_vec();
                    for row in rows {
                        if row.path.is_empty()
                            || row.transfer
                            || domain_requires_provenance(program, row.domain_symbol)
                        {
                            continue;
                        }
                        let mut path = prefix.clone();
                        path.extend_from_slice(&row.path);
                        let place =
                            crate::published::semantic::places::append_place(semantic, root, &path);
                        let fact = semantic.append_fact(membership_fact(
                            place,
                            row.domain_symbol,
                            row.semantic_domain,
                        ));
                        semantic.append_ref(refs, fact);
                    }
                }
            }
        }
        let target = semantic.places.get(target_place);
        let target_segments = semantic.place_segments.span_or_empty(target.segments);
        if target_is_exact
            && target_segments.iter().any(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Index { .. } | PlaceSegment::FixedIndex { .. }
                )
            })
        {
            for reference in build
                .contexts
                .semantic_context_refs
                .span_or_empty(assignment_source_contexts)
            {
                let context = semantic.contexts.get(reference.context);
                for reference in semantic.refs.span_or_empty(context.facts) {
                    let fact = semantic.facts.get(reference.fact);
                    let Some((_, _, symbol, domain)) = fact.payload.domain_membership() else {
                        continue;
                    };
                    if !declared_target_domains.contains(&(symbol, domain)) {
                        continue;
                    }
                    let Some(place) = fact.place.as_place() else {
                        continue;
                    };
                    let candidate = semantic.places.get(place);
                    let segments = semantic.place_segments.span_or_empty(candidate.segments);
                    if candidate.root != target.root
                        || segments.len() != target_segments.len()
                        || !segments.iter().all(crate::flow::fixed_selection_segment)
                        || !target_segments
                            .iter()
                            .zip(segments)
                            .all(|(target, candidate)| {
                                matches!(
                                    (target, candidate),
                                    (
                                        PlaceSegment::Index { .. }
                                            | PlaceSegment::FixedIndex { .. },
                                        PlaceSegment::FixedIndex { .. }
                                    )
                                ) || target == candidate
                            })
                    {
                        continue;
                    }
                    let candidate = CanonicalPlace {
                        root: candidate.root,
                        segments: segments.to_vec(),
                    };
                    crate::shared::collections::push_unique(&mut candidate_targets, candidate);
                }
            }
        }
        for candidate in candidate_targets {
            let candidate_place = crate::published::semantic::places::append_place(
                semantic,
                candidate.root,
                &candidate.segments,
            );
            for (domain_symbol, semantic_domain) in &declared_target_domains {
                let was_live = build
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(assignment_source_contexts)
                    .iter()
                    .any(|reference| {
                        semantic
                            .context_view(semantic.contexts.get(reference.context))
                            .proves_place_domain_membership_in_program(
                                program,
                                candidate_place,
                                *domain_symbol,
                            )
                    });
                if !was_live {
                    continue;
                }
                let fact = semantic.append_fact(membership_fact(
                    candidate_place,
                    *domain_symbol,
                    *semantic_domain,
                ));
                semantic.append_ref(refs, fact);
            }
        }
    }
}
mod owned_qualifications {

    /*
        Owned value transfers preserve existing qualifications and IEEE predicates.
        No declaration, equal carrier geometry, or matching label creates a fact.
        Reference copies do not copy referent predicates. Call facts remain provisional
        until the call/custody checker validates their exact producing invocation.
    */
    use arena::HandleSpan;
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceHandle, ProgramPoint};

    pub(in crate::flow) fn append_owned_qualification_transfer(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        active: &[facts::FactContextHandle],
        source_expression: typed_trees::expression::ExpressionHandle,
        source: PlaceHandle,
        destination: PlaceHandle,
        destination_type: typed_trees::types::TypeReferenceHandle,
        point: ProgramPoint,
        references: &mut HandleSpan<facts::FactRef>,
    ) {
        let ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } = point
        else {
            return;
        };
        let Some((machine, state)) =
            validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
        else {
            return;
        };
        let source = *semantic.places.get(source);
        // A completed expression's own facts name that exact result, independent
        // of whether it came from a call or a constructor. Reusing storage facts
        // still requires the canonical builtin coordinate owner; a selected call
        // cannot borrow its arguments' or a same-spelled place's memberships.
        let exact_result = source.root == facts::PlaceRoot::Expression(source_expression)
            && source.segments.is_empty();
        if !exact_result
            && !validation::value_has_builtin_coordinates(
                program,
                machine,
                state,
                source_expression,
            )
        {
            return;
        }
        let destination = *semantic.places.get(destination);
        let source_segments = semantic
            .place_segments
            .span_or_empty(source.segments)
            .to_vec();
        let destination_segments = semantic
            .place_segments
            .span_or_empty(destination.segments)
            .to_vec();
        if !crate::flow::place_cases_are_selected(
            program,
            semantic,
            active,
            machine_symbol,
            state_symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: source.root,
                segments: source_segments.clone(),
            },
        ) {
            return;
        }
        let candidates = active
            .iter()
            .flat_map(|context| {
                semantic
                    .context_view(semantic.contexts.get(*context))
                    .facts()
            })
            .filter(|fact| {
                (fact.payload.domain_membership().is_some()
                    || matches!(fact.payload, FactPayload::FloatPredicate { .. }))
                    && fact
                        .place
                        .as_place()
                        .is_some_and(|place| semantic.places.get(place).root == source.root)
            })
            .copied()
            .collect::<Vec<_>>();
        let mut transfers = Vec::new();
        for fact in candidates {
            let mut erased = false;
            let payload = if let Some((_, domain, domain_symbol, semantic_domain)) =
                fact.payload.domain_membership()
            {
                if !semantic_domain.is_valid()
                    && validation::field_domains::domain_requires_provenance(program, domain_symbol)
                {
                    continue;
                }
                erased =
                    explicitly_erased(program, source_expression, domain_symbol, semantic_domain);
                FactPayload::DomainMembership {
                    value: typed_trees::expression::ExpressionHandle::invalid(),
                    domain,
                    domain_symbol,
                    semantic_domain,
                }
            } else if matches!(fact.payload, FactPayload::FloatPredicate { .. }) {
                fact.payload
            } else {
                continue;
            };
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let place = semantic.places.get(place);
            let segments = semantic.place_segments.span_or_empty(place.segments);
            if place.root != source.root
                || segments.len() < source_segments.len()
                || !crate::flow::segments_cover_subject(
                    program,
                    &segments[..source_segments.len()],
                    &source_segments,
                )
            {
                continue;
            }
            let relative_path = &segments[source_segments.len()..];
            if erased && relative_path.is_empty() {
                continue;
            }
            if !(0..=relative_path.len()).all(|length| {
                let Some(mut reference) = validation::project_type_reference_from_segments(
                    program,
                    destination_type,
                    &relative_path[..length],
                ) else {
                    return false;
                };
                while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    reference = *base_type;
                }
                !matches!(
                    program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                )
            }) {
                continue;
            }
            if !relative_path.iter().chain(&destination_segments).all(|segment| {
                matches!(segment,
                facts::PlaceSegment::Field { symbol } if symbol.is_valid())
                    || matches!(segment, facts::PlaceSegment::Case { variant } if variant.is_valid())
                    || matches!(segment, facts::PlaceSegment::FixedIndex { .. })
            }) {
                continue;
            }
            let mut rebased = destination_segments.clone();
            rebased.extend_from_slice(relative_path);
            let place = crate::published::semantic::places::append_place(
                semantic,
                destination.root,
                &rebased,
            );
            transfers.push(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: fact.evidence,
                payload,
            });
        }
        if transfers.is_empty() {
            return;
        }
        // Assignment transport can already have published the first occurrence.
        // Include that prefix only after a transfer survives all eligibility checks.
        let prefix = semantic.refs.span_or_empty(*references);
        let existing = prefix.len();
        let mut batch = Vec::with_capacity(existing + transfers.len());
        batch.extend(
            prefix
                .iter()
                .map(|reference| *semantic.facts.get(reference.fact)),
        );
        batch.extend(transfers);
        // Check each place's records together, then publish new records in the
        // original order, leaving the entire existing prefix unchanged.
        let first = semantic.first_owned_qualification_positions(&batch, existing);
        for (position, fact) in batch.into_iter().enumerate().skip(existing) {
            if first[position] == position {
                let transferred = semantic.append_fact(fact);
                semantic.append_ref(references, transferred);
            }
        }
    }

    /*
        Whether an erasing cast in the source's cast chain drops this membership
        before the value reaches its new place.
    */
    pub(super) fn explicitly_erased(
        program: &dyn ::validation::CheckingInput,
        source_expression: typed_trees::expression::ExpressionHandle,
        domain_symbol: symbols::SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> bool {
        validation::field_domains::expression_erases_membership(
            program,
            source_expression,
            domain_symbol,
            semantic_domain,
        )
    }
}
pub(super) use owned_qualifications::append_owned_qualification_transfer;
mod projected {
    /*
        Whole-place assignment transports live evidence on its contained fields.
        Predicates, literal leaf values, folded scalar snapshots, and per-byte
        predicate classes all describe the copied VALUE, so each stored fact below
        the source place reappears at the destination under the same relative path:
        `let copy = rows` carries `rows[i].bytes` evidence onto `copy[i].bytes`, and
        `let r = rows[0]` carries `rows[0].bytes` onto `r.bytes`. This is the
        below-source sibling of the exact-place lane in
        `propagate_statement_transfers`, which transports the same payloads at the
        source place itself. Runtime-indexed segments still stop the transport -- a
        copy cannot promise which element supplied the evidence.
    */
    use super::super::operand_contexts::operand_binds_stable_copy;
    use super::PlaceHandle;
    use crate::flow;
    use crate::flow::FlowBuildContext;
    use crate::flow::origin_place;
    use crate::values::is_assignment_literal;
    use arena::HandleSpan;
    use checked_trees::FlowSemanticContextRef;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::statement::StatementNode;
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};
    use symbols::SymbolHandle;

    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_copied_field_predicates(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        source: PlaceHandle,
        destination: PlaceHandle,
        point: ProgramPoint,
        references: &mut HandleSpan<facts::FactRef>,
    ) {
        append_value_predicates(
            program,
            semantic,
            contexts,
            active,
            source,
            destination,
            point,
            references,
            CopiedEvidence::ContainedStorage,
        );
    }

    /*
        Completed owned copies have their own expression storage. Only observations
        of owned contents cross this boundary; reference descriptors retain live
        referent epochs and routed qualifications keep their independent custody.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn append_completed_copy_predicates(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        source: PlaceHandle,
        destination: PlaceHandle,
        point: ProgramPoint,
        references: &mut HandleSpan<facts::FactRef>,
    ) {
        append_value_predicates(
            program,
            semantic,
            contexts,
            active,
            source,
            destination,
            point,
            references,
            CopiedEvidence::CompletedValue,
        );
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CopiedEvidence {
        ContainedStorage,
        CompletedValue,
    }

    #[allow(clippy::too_many_arguments)]
    fn append_value_predicates(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &FlowBuildContext,
        active: HandleSpan<FlowSemanticContextRef>,
        source: PlaceHandle,
        destination: PlaceHandle,
        point: ProgramPoint,
        references: &mut HandleSpan<facts::FactRef>,
        evidence: CopiedEvidence,
    ) {
        let source_place = *semantic.places.get(source);
        let destination_place = *semantic.places.get(destination);
        let valid_source_root = match source_place.root {
            facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
            facts::PlaceRoot::Expression(expression) => {
                program.expression_table.expression_is_valid(expression)
            }
            _ => false,
        };
        if !valid_source_root
            || !(matches!(destination_place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
                || (evidence == CopiedEvidence::CompletedValue
                    && matches!(destination_place.root, facts::PlaceRoot::Expression(expression)
                        if program.expression_table.expression_is_valid(expression))))
        {
            return;
        }
        let source_segments = semantic
            .place_segments
            .span_or_empty(source_place.segments)
            .to_vec();
        let destination_segments = semantic
            .place_segments
            .span_or_empty(destination_place.segments)
            .to_vec();
        if !flow::source_transfer_cases_selected(
            program,
            semantic,
            contexts,
            active,
            point,
            source_place.root,
            source_segments.clone(),
        ) {
            return;
        }
        if !source_segments
            .iter()
            .chain(&destination_segments)
            .all(flow::place_segment_has_static_identity)
        {
            return;
        }
        let facts: Vec<_> = contexts
            .contexts
            .semantic_context_refs
            .span_or_empty(active)
            .iter()
            .flat_map(|reference| {
                semantic
                    .refs
                    .span_or_empty(semantic.contexts.get(reference.context).facts)
            })
            .map(|reference| *semantic.facts.get(reference.fact))
            .collect();
        for fact in facts {
            /*
                A `ZeroCaseElements` fact reads below the source either as the
                whole array, which carries the fact itself, or as one element's
                tag. `elements` keeps the array's path and the fact's own payload
                for the first; both are judged on the tagged place they stand for.
            */
            let mut elements = None;
            let mut zero_case_segments = None;
            let fact = match fact.payload {
                FactPayload::ZeroCaseElements {
                    first,
                    end,
                    element,
                    variant,
                } => {
                    let Some(place) = fact.place.as_place() else {
                        continue;
                    };
                    let place = semantic.places.get(place);
                    if place.root != source_place.root || first >= end {
                        continue;
                    }
                    let array = semantic.place_segments.span_or_empty(place.segments);
                    let element_segments = semantic.place_segments.span_or_empty(element);
                    let tagged = match flow::zero_case_below(
                        program,
                        array,
                        first..end,
                        element_segments,
                        &source_segments,
                    ) {
                        Some(flow::ZeroCaseBelow::Elements { .. }) => {
                            elements = Some((array.len(), fact.payload));
                            array
                                .iter()
                                .copied()
                                .chain([facts::PlaceSegment::FixedIndex { index: first }])
                                .chain(element_segments.iter().copied())
                                .collect::<Vec<_>>()
                        }
                        Some(flow::ZeroCaseBelow::Element { place }) => {
                            source_segments.iter().copied().chain(place).collect()
                        }
                        None => continue,
                    };
                    zero_case_segments = Some(tagged);
                    Fact {
                        payload: FactPayload::CaseValue {
                            variant,
                            value: true,
                        },
                        ..fact
                    }
                }
                _ => fact,
            };
            let payload = match fact.payload {
                facts::domain_membership_pattern!(domain, domain_symbol, semantic_domain) => {
                    if !program
                        .domain_definition(domain_symbol)
                        .is_some_and(|definition| {
                            definition.establishment_routes.is_empty()
                                && definition.alias.is_none()
                                && definition.predicate_presence.is_present()
                        })
                    {
                        continue;
                    }
                    FactPayload::DomainMembership {
                        value: ExpressionHandle::invalid(),
                        domain,
                        domain_symbol,
                        semantic_domain,
                    }
                }
                FactPayload::AssignedValue { value } if is_assignment_literal(program, value) => {
                    fact.payload
                }
                FactPayload::CaseValue { .. }
                | FactPayload::AssignedScalarValue { .. }
                | FactPayload::AssignedValueCapture { .. }
                | FactPayload::BytePredicate { .. }
                | FactPayload::FloatPredicate { .. } => fact.payload,
                _ => continue,
            };
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let place = semantic.places.get(place);
            let segments = match &zero_case_segments {
                Some(tagged) => tagged.as_slice(),
                None => semantic.place_segments.span_or_empty(place.segments),
            };
            if place.root != source_place.root
                || segments.len() < source_segments.len()
                || (evidence == CopiedEvidence::ContainedStorage
                    && segments.len() == source_segments.len())
                || !segments.starts_with(&source_segments)
                || !segments.iter().all(flow::place_segment_has_static_identity)
            {
                continue;
            }
            let suffix = segments[source_segments.len()..].to_vec();
            let ProgramPoint::Statement {
                state_symbol,
                statement_index,
                ..
            } = point
            else {
                continue;
            };
            let source_value = validation::CanonicalPlace {
                root: place.root,
                segments: segments.to_vec(),
            };
            let target_value = validation::CanonicalPlace {
                root: destination_place.root,
                segments: destination_segments
                    .iter()
                    .chain(&suffix)
                    .copied()
                    .collect(),
            };
            if evidence == CopiedEvidence::CompletedValue {
                let Some(source_type) = validation::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &validation::CanonicalPlace {
                        root: source_place.root,
                        segments: source_segments.clone(),
                    },
                ) else {
                    continue;
                };
                let Some(projected_type) =
                    validation::owned_projection_type_reference(program, source_type, &suffix)
                else {
                    continue;
                };
                if validation::reference_shell(program, projected_type).is_some()
                    || (!matches!(payload, FactPayload::CaseValue { .. })
                        && !operand_binds_stable_copy(program, projected_type))
                {
                    continue;
                }
            }
            if !super::completed_value_carriers_agree(
                program,
                state_symbol,
                statement_index,
                &source_value,
                &target_value,
                None,
            ) {
                continue;
            }
            let (suffix, payload) = match elements {
                Some((array_length, elements)) => (
                    segments[source_segments.len()..array_length].to_vec(),
                    elements,
                ),
                None => (suffix, payload),
            };
            let copied_place = semantic.append_place(facts::Place {
                root: destination_place.root,
                segments: HandleSpan::empty(),
            });
            for segment in destination_segments.iter().chain(&suffix) {
                semantic.push_place_segment(copied_place, *segment);
            }
            let copied_fact = semantic.append_fact(Fact {
                place: FactPlace::Place(copied_place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: fact.evidence,
                payload,
            });
            semantic.append_ref(references, copied_fact);
        }
    }

    /*
        The caller storage a freshly bound local REFERENCE refers to, or `None`
        when the statement does not bind a bare reference local or the caller
        prefix cannot prove one unique referent. A returned reference has no
        result storage of its own (flow/calls.rs), so its evidence stays below
        the referent place this recovers -- the same local write origin the
        mutation side already derives for `let room = room_mut(level)`, projected
        back into a canonical place. Only an exactly resolved origin (no runtime
        index truncation, no coarse-only path) may lend evidence to the binding.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bound_reference_referent_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        contexts: &FlowBuildContext,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        destination: PlaceHandle,
    ) -> Option<PlaceHandle> {
        let destination = *semantic.places.get(destination);
        if !destination.segments.is_empty() {
            return None;
        }
        let local_symbol = destination.root.as_symbol()?;
        if !local_symbol.is_valid() {
            return None;
        }
        let type_reference = match statement {
            StatementNode::LocalData(local) if local.symbol == local_symbol => local.type_reference,
            StatementNode::Assignment(_) => {
                declared_local_type_reference(program, state_symbol, statement_index, local_symbol)?
            }
            _ => return None,
        };
        let peeled = ::validation::peel_declared_constraints(program, type_reference);
        if !matches!(
            program.type_reference_table.type_reference(peeled),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            return None;
        }
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = program.state_by_symbol(state_symbol)?;
        let before =
            flow::state_statements(program, state.statement_nodes).get(statement_index + 1)?;
        let mut owned = None;
        let resolver = flow::shared_call_frames_or(contexts.call_frames, program, &mut owned)?;
        let origin = flow::sole_local_referent_origin(resolver, machine, before, local_symbol)?;
        let (place, precision) = origin_place(program, state, statement_index, &origin)?;
        if precision != flow::OriginPrecision::Exact {
            return None;
        }
        Some(crate::published::semantic::places::append_place(
            semantic,
            place.root,
            &place.segments,
        ))
    }

    fn declared_local_type_reference(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        local_symbol: SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let state = program.state_by_symbol(state_symbol)?;
        program
            .state_local_declaration(state, local_symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)
            .map(|(_, local)| local.type_reference)
    }
}
pub(super) use projected::append_completed_copy_predicates;
mod replacement {
    /*
        A selected owned replacement can preserve a collection's field contracts.

        Conservative overlap retires every possible selected element's membership.
        Each exact old leaf nevertheless keeps its qualification if its unchanged
        value had that qualification and the completed replacement independently
        satisfies the same conditional leaf contract. This reactivates an existing
        fact; it does not distribute the replacement's value, tag, issuer or equality
        among the candidates. Previously inactive leaves have no membership to
        reactivate and are not manufactured from their declarations here.
    */
    use super::write_target::StatementWrite;
    use crate::flow;
    use crate::flow::{CaseEvidence, FlowBuildContext, MembershipEvidence};
    use arena::{Handle, HandleSpan};
    use checked_trees::{FlowSemanticContextRef, FlowStatementFact};
    use facts::{FactPlan, FactRef, PlaceSegment, ProgramPoint};

    pub(super) fn preserve_owned_memberships(
        program: &dyn validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &FlowBuildContext,
        write: &StatementWrite,
        statement: Handle<FlowStatementFact>,
        before_store: HandleSpan<FlowSemanticContextRef>,
        point: ProgramPoint,
        references: &mut HandleSpan<FactRef>,
    ) {
        if !write.target_is_exact {
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
        let Some((machine, state)) =
            validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
        else {
            return;
        };
        if !validation::value_has_builtin_coordinates(
            program,
            machine,
            state,
            write.source_expression,
        ) {
            return;
        }
        let target = semantic.places.get(write.target_place);
        let target_segments = semantic.place_segments.span_or_empty(target.segments);
        if !target_segments.iter().any(|segment| matches!(segment, PlaceSegment::Index { .. }))
            || !target_segments.iter().all(|segment| {
                flow::place_segment_has_static_identity(segment)
                    || matches!(segment, PlaceSegment::Index { expression }
                        if expression.is_valid() && program.expression_table.range_node(*expression).is_none())
            })
        {
            return;
        }
        let stored = build.control.statements.get(statement).stored_value;
        if !build.control.operands.is_valid(stored) {
            return;
        }
        let replacement = build.control.operands.get(stored);
        if replacement.expression != write.source_expression
            || !validation::has_contents(
                program,
                replacement.type_reference,
                &[],
                validation::ContentsRequirement::BorrowFree,
            )
        {
            return;
        }
        let Some(source) = write.source_place.and_then(|source| {
            flow::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(source),
            )
        }) else {
            return;
        };
        let completed = flow::operand_contexts(
            program,
            &build.contexts,
            &build.control,
            semantic,
            replacement,
            replacement.type_reference,
            replacement.constraints,
        );
        let memberships = MembershipEvidence::prepare(program, semantic, &completed);
        let Some(cases) = CaseEvidence::prepare(
            program,
            semantic,
            &completed,
            machine_symbol,
            state_symbol,
            statement_index,
        ) else {
            return;
        };
        let contexts = build
            .contexts
            .semantic_context_refs
            .span_or_empty(before_store)
            .iter()
            .map(|reference| reference.context);
        let mut preserved = Vec::new();
        for handle in semantic.unique_context_facts(contexts) {
            let fact = semantic.facts.get(handle);
            let Some((_, _, domain, identity)) = fact.payload.domain_membership() else {
                continue;
            };
            let Some(place) = fact.place.as_place() else {
                continue;
            };
            let candidate = semantic.places.get(place);
            let segments = semantic.place_segments.span_or_empty(candidate.segments);
            if !identity.is_valid()
                || candidate.root != target.root
                || segments.len() <= target_segments.len()
                || !segments.iter().all(flow::place_segment_has_static_identity)
                || !target_segments
                    .iter()
                    .zip(segments)
                    .all(|(target, candidate)| {
                        matches!(
                            (target, candidate),
                            (PlaceSegment::Index { .. }, PlaceSegment::FixedIndex { .. })
                        ) || flow::static_place_segment_pair_equal(*target, *candidate)
                            == Some(true)
                    })
            {
                continue;
            }
            let relative = &segments[target_segments.len()..];
            let mut projected = source.clone();
            projected.segments.extend_from_slice(relative);
            if cases.path_is_inactive(&projected)
                || flow::literal_value_path_is_inactive(
                    program,
                    replacement.expression,
                    replacement.type_reference,
                    relative,
                )
                || memberships.contains(&projected, domain, identity)
            {
                preserved.push(handle);
            }
        }
        drop(memberships);
        for fact in preserved {
            semantic.append_ref(references, fact);
        }
    }
}
mod scalar_values {
    /*
        Capture a selected initializer or assignment while its operand facts are live.
    */
    use super::super::call_phases::call_storage_writes;
    use validation::CanonicalPlace;

    use crate::flow::FlowBuildContext;
    use crate::published::places::place_at;
    use arena::HandleSpan;
    use checked_trees::CheckedScalarExpressionRole;
    use checked_trees::CheckedStructuralPredicatePathSegment;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use checked_trees::statement::StatementNode;
    use checked_trees::{BorrowFacts, FlowSemanticContextRef};
    use facts::FactPlan;
    use facts::ScalarValue;
    use symbols::SymbolHandle;

    pub(crate) mod calls {
        use super::{CapturedValue, CheckedScalarExpressionRole};
        use crate::flow::FlowBuildContext;
        use crate::published::places::place_at;

        use crate::flow::transfers::scalar_values::CallValues;
        use crate::flow::transfers::scalar_values::LiveValues;
        use crate::flow::transfers::scalar_values::retains_values_across_unit_call;
        use arena::HandleSpan;

        use checked_trees::statement::StatementNode;
        use checked_trees::{BorrowFacts, FlowSemanticContextRef};
        use facts::FactPlan;
        use symbols::SymbolHandle;

        mod normal_returns {
            /*
                Capture normal values through selected scalar state paths.

                Each local, guard, return and delivered argument rejoins its checked source
                binding and selected pure expression. Unknown guards retain both paths;
                impossible guards skip a path, and explicit crashes contribute no normal
                value. State transfers deliver copied values into a fresh scalar namespace.
                A cycle requiring induction or a missing selected operation withholds the
                whole capture, rather than trusting a partially inspected path.
            */
            use super::{
                CallValues, CapturedValue, CheckedScalarExpressionRole as ExpressionRole,
                FlowBuildContext,
            };
            use crate::flow::state_statements;
            use checked_trees::{BorrowFacts, CheckedScalarExpression};
            use symbols::SymbolHandle;
            use typed_trees::expression::ExpressionHandle;
            use typed_trees::machine::Machine;
            use typed_trees::state::State;
            use typed_trees::statement::{
                StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
                TransitionTargetNode,
            };
            use validation::activation_target_state_index;

            enum CapturedReturns<Value> {
                NoNormalValue,
                Value(Value),
            }

            impl<Value: CapturedValue> CapturedReturns<Value> {
                fn join(self, other: Self) -> Option<Self> {
                    match (self, other) {
                        (Self::NoNormalValue, other) | (other, Self::NoNormalValue) => Some(other),
                        (Self::Value(left), Self::Value(right)) => {
                            left.join(right).map(Self::Value)
                        }
                    }
                }
            }

            #[allow(clippy::too_many_arguments)]
            pub(super) fn capture<Value: CapturedValue>(
                program: &dyn ::validation::CheckingInput,
                borrow: &BorrowFacts,
                context: &mut FlowBuildContext,
                machine: &Machine,
                state: &State,
                symbols: Vec<SymbolHandle>,
                values: CallValues<Value>,
            ) -> Option<Value> {
                let mut capture = NormalReturnCapture {
                    program,
                    borrow,
                    context,
                    machine,
                    visiting: Vec::new(),
                };
                match capture.state(state, symbols, values)? {
                    CapturedReturns::Value(value) => Some(value),
                    CapturedReturns::NoNormalValue => None,
                }
            }

            struct NormalReturnCapture<'capture, 'plans> {
                program: &'capture dyn ::validation::CheckingInput,
                borrow: &'capture BorrowFacts,
                context: &'capture mut FlowBuildContext<'plans>,
                machine: &'capture Machine,
                visiting: Vec<SymbolHandle>,
            }

            impl NormalReturnCapture<'_, '_> {
                fn state<Value: CapturedValue>(
                    &mut self,
                    state: &State,
                    mut symbols: Vec<SymbolHandle>,
                    mut values: CallValues<Value>,
                ) -> Option<CapturedReturns<Value>> {
                    if self.visiting.contains(&state.symbol) {
                        return None;
                    }
                    self.visiting.push(state.symbol);
                    let result = self.statements(state, &mut symbols, &mut values);
                    self.visiting.pop();
                    result
                }

                fn statements<Value: CapturedValue>(
                    &mut self,
                    state: &State,
                    symbols: &mut Vec<SymbolHandle>,
                    values: &mut CallValues<Value>,
                ) -> Option<CapturedReturns<Value>> {
                    let statements = state_statements(self.program, state.statement_nodes);
                    let mut returned = CapturedReturns::NoNormalValue;
                    let mut immutable_local_count = 0_u32;
                    for (statement_index, statement) in statements.iter().enumerate() {
                        if let Some(call) = statement.as_call() {
                            super::retains_values_across_unit_call(
                                self.program,
                                self.borrow,
                                self.context,
                                self.machine,
                                state,
                                statement_index,
                                call,
                                symbols,
                                values,
                            )?;
                            continue;
                        }
                        let ordinal = u32::try_from(statement_index).ok()?;
                        if let Some(transition) = statement.as_transition() {
                            let condition = match transition.guard {
                                TransitionGuardNode::Always => Some(true),
                                TransitionGuardNode::When(source) => {
                                    let expression = selected(
                                        self.context,
                                        state.symbol,
                                        ordinal,
                                        ExpressionRole::Guard,
                                        source,
                                        SymbolHandle::invalid(),
                                        symbols,
                                    )?;
                                    Value::boolean(expression, values)
                                }
                            };
                            if condition != Some(false) {
                                let primary = if matches!(transition.exit, TransitionExit::Crash(_))
                                {
                                    CapturedReturns::NoNormalValue
                                } else {
                                    self.target(
                                        state,
                                        ordinal,
                                        transition.target,
                                        false,
                                        symbols,
                                        values,
                                    )?
                                };
                                returned = returned.join(primary)?;
                            }
                            if condition != Some(true) {
                                if transition.continuation.is_valid() {
                                    let continuation = self.target(
                                        state,
                                        ordinal,
                                        transition.continuation,
                                        true,
                                        symbols,
                                        values,
                                    )?;
                                    returned = returned.join(continuation)?;
                                } else {
                                    continue;
                                }
                            }
                            return Some(returned);
                        }
                        let (source, destination, role) = match statement {
                            StatementNode::LocalData(local) => {
                                if !local.symbol.is_valid()
                                    || symbols.contains(&local.symbol)
                                    || values
                                        .storage
                                        .iter()
                                        .any(|(symbol, _)| *symbol == local.symbol)
                                    || self
                                        .program
                                        .primitive_type_reference(local.type_reference)
                                        .is_none()
                                {
                                    return None;
                                }
                                (
                                    local.initial_value,
                                    local.symbol,
                                    if local.is_mutable {
                                        ExpressionRole::StorageInitializer
                                    } else {
                                        ExpressionRole::LocalInitializer {
                                            binding_ordinal: immutable_local_count,
                                        }
                                    },
                                )
                            }
                            StatementNode::Assignment(assignment) => {
                                let path = self
                                    .program
                                    .expression_table
                                    .name_path_node(assignment.target)?;
                                if !values
                                    .storage
                                    .iter()
                                    .any(|(symbol, _)| *symbol == path.symbol)
                                {
                                    return None;
                                }
                                (
                                    assignment.value,
                                    path.symbol,
                                    ExpressionRole::AssignmentValue,
                                )
                            }
                            StatementNode::Expression(expression)
                                if statement_index + 1 == statements.len() =>
                            {
                                (*expression, SymbolHandle::invalid(), ExpressionRole::Return)
                            }
                            _ => return None,
                        };
                        let expression = selected(
                            self.context,
                            state.symbol,
                            ordinal,
                            role,
                            source,
                            destination,
                            symbols,
                        )?;
                        let value = Value::evaluate_local(expression, values);
                        match role {
                            ExpressionRole::Return => {
                                return returned.join(CapturedReturns::Value(value?));
                            }
                            ExpressionRole::LocalInitializer { .. } => {
                                symbols.push(destination);
                                values.bindings.push(value);
                                immutable_local_count = immutable_local_count.checked_add(1)?;
                            }
                            ExpressionRole::StorageInitializer => {
                                values.storage.push((destination, value?))
                            }
                            ExpressionRole::AssignmentValue => {
                                let (_, current) = values
                                    .storage
                                    .iter_mut()
                                    .find(|(symbol, _)| *symbol == destination)?;
                                *current = value?;
                            }
                            _ => return None,
                        }
                    }
                    None
                }

                #[allow(clippy::too_many_arguments)]
                fn target<Value: CapturedValue>(
                    &mut self,
                    state: &State,
                    ordinal: u32,
                    target: TransitionTargetHandle,
                    continuation: bool,
                    symbols: &[SymbolHandle],
                    values: &mut CallValues<Value>,
                ) -> Option<CapturedReturns<Value>> {
                    if !target.is_valid() {
                        return None;
                    }
                    match self.program.statement_table.transition_target(target) {
                        TransitionTargetNode::Value(source) => {
                            if let Some(returned) =
                                self.returned_machine_call(*source, symbols, values)
                            {
                                return returned;
                            }
                            let role = if continuation {
                                ExpressionRole::ContinuationReturn
                            } else {
                                ExpressionRole::Return
                            };
                            let expression = selected(
                                self.context,
                                state.symbol,
                                ordinal,
                                role,
                                *source,
                                SymbolHandle::invalid(),
                                symbols,
                            )?;
                            Some(CapturedReturns::Value(Value::evaluate_local(
                                expression, values,
                            )?))
                        }
                        TransitionTargetNode::Named {
                            path,
                            arguments,
                            static_machine_parameter,
                            evidence_arguments,
                            ..
                        } => {
                            if static_machine_parameter.is_valid() || !evidence_arguments.is_empty()
                            {
                                return None;
                            }
                            let target_index = activation_target_state_index(
                                self.program,
                                self.machine,
                                path.symbol,
                            )?;
                            let target_state = self
                                .program
                                .machine_states(self.machine)
                                .get(target_index)?;
                            let arguments =
                                self.program.statement_table.expression_handles(*arguments);
                            let parameters = self.program.state_parameters(target_state);
                            if parameters.len() != arguments.len() {
                                return None;
                            }
                            let mut delivered = CallValues {
                                bindings: Vec::new(),
                                storage: Vec::new(),
                                fields: Vec::new(),
                            };
                            let mut target_symbols = Vec::new();
                            for (position, (parameter, source)) in
                                parameters.iter().zip(arguments).enumerate()
                            {
                                if parameter.relevance.is_erased() {
                                    continue;
                                }
                                if parameter.is_const
                                    || !parameter.symbol.is_valid()
                                    || self
                                        .program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_none()
                                    || target_symbols.contains(&parameter.symbol)
                                {
                                    return None;
                                }
                                let argument_ordinal = u32::try_from(position).ok()?;
                                let role = if continuation {
                                    ExpressionRole::TransitionContinuationArgument {
                                        argument_ordinal,
                                    }
                                } else {
                                    ExpressionRole::TransitionArgument { argument_ordinal }
                                };
                                let expression = selected(
                                    self.context,
                                    state.symbol,
                                    ordinal,
                                    role,
                                    *source,
                                    parameter.symbol,
                                    symbols,
                                )?;
                                let value = Value::evaluate_local(expression, values);
                                target_symbols.push(parameter.symbol);
                                if parameter.is_mutable {
                                    delivered.bindings.push(None);
                                    delivered.storage.push((parameter.symbol, value?));
                                } else {
                                    delivered.bindings.push(value);
                                }
                            }
                            self.state(target_state, target_symbols, delivered)
                        }
                        TransitionTargetNode::Terminal => None,
                    }
                }
            }

            impl<'capture> NormalReturnCapture<'capture, '_> {
                /*
                    An arm that returns a call to another checked machine, as
                    `true -> outside()` or `_ -> inside(value)` returning a
                    nested machine's result, returns that machine's normal
                    value. Its scalar arguments are literals or values already
                    bound on this path, so the callee is captured under them.
                    `None` means the arm is not such a call; `Some(None)`
                    withholds the capture, as an unlowered return does.
                */
                fn returned_machine_call<Value: CapturedValue>(
                    &mut self,
                    source: ExpressionHandle,
                    symbols: &[SymbolHandle],
                    values: &CallValues<Value>,
                ) -> Option<Option<CapturedReturns<Value>>> {
                    let program = self.program;
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(source)
                    else {
                        return None;
                    };
                    Some((|| {
                        if call.receiver.is_valid()
                            || !call.machine_arguments.is_empty()
                            || !call.selects_only_nominal_route()
                        {
                            return None;
                        }
                        let entry = validation::call_target_entry_symbol(
                            program,
                            validation::call_target_symbol(program, call),
                        );
                        let machine = program.machine_holding_state(entry)?;
                        if !machine.body_is_present
                            || machine.supply_mode
                                != language_semantics::MachineSupplyMode::CheckedBody
                            || !machine.owned_data.is_empty()
                        {
                            return None;
                        }
                        let state = program.machine_states(machine).first()?;
                        if state.symbol != entry {
                            return None;
                        }
                        let parameters = program.state_parameters(state);
                        let arguments = program.expression_table.expression_handles(call.arguments);
                        if parameters.len() != arguments.len() {
                            return None;
                        }
                        let mut target_symbols = Vec::with_capacity(parameters.len());
                        let mut delivered = CallValues {
                            bindings: Vec::with_capacity(parameters.len()),
                            storage: Vec::new(),
                            fields: Vec::new(),
                        };
                        for (parameter, argument) in parameters.iter().zip(arguments) {
                            if parameter.relevance.is_erased()
                                || parameter.is_const
                                || parameter.is_mutable
                                || !parameter.symbol.is_valid()
                                || program.primitive_type_reference(parameter.type_reference).is_none()
                                || target_symbols.contains(&parameter.symbol)
                            {
                                return None;
                            }
                            let node = program.expression_table.expression(*argument);
                            let value = match Value::literal(node) {
                                Some(value) => Some(value),
                                None => {
                                    let typed_trees::expression::ExpressionNode::Name(path) = node
                                    else {
                                        return None;
                                    };
                                    let position = symbols
                                        .iter()
                                        .position(|symbol| *symbol == path.symbol)?;
                                    values.bindings.get(position)?.clone()
                                }
                            };
                            target_symbols.push(parameter.symbol);
                            delivered.bindings.push(value);
                        }
                        let caller = std::mem::replace(&mut self.machine, machine);
                        let returned = self.state(state, target_symbols, delivered);
                        self.machine = caller;
                        returned
                    })())
                }
            }

            fn selected<'plans>(
                context: &FlowBuildContext<'plans>,
                state: SymbolHandle,
                ordinal: u32,
                role: ExpressionRole,
                source: ExpressionHandle,
                destination: SymbolHandle,
                symbols: &[SymbolHandle],
            ) -> Option<&'plans CheckedScalarExpression> {
                let (binding, expression) = context
                    .selected_scalar_at(state, ordinal, |binding| {
                        binding.role == role
                            && binding.expression == source
                            && binding.destination == destination
                    })
                    .flatten()?;
                if context
                    .scalar_expressions
                    .binding_symbols
                    .span_or_empty(binding.symbols)
                    != symbols
                {
                    return None;
                }
                Some(expression)
            }
        }

        /*
            Capture the selected scalar state graph under copied caller actuals.
            Intervening Unit calls preserve these normal-return facts only when their
            complete storage footprint is empty. This never proves that a call returns.
        */
        pub(super) fn capture_call<Value: CapturedValue>(
            program: &dyn ::validation::CheckingInput,
            borrow: &BorrowFacts,
            semantic: &FactPlan,
            context: &mut FlowBuildContext,
            caller_state: SymbolHandle,
            statement_index: usize,
            source: typed_trees::expression::ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            active: HandleSpan<FlowSemanticContextRef>,
        ) -> Option<Value> {
            if !call.machine_arguments.is_empty() || !call.selects_only_nominal_route() {
                return None;
            }
            let (caller_machine_index, caller_state_index) =
                context.state_location(program, caller_state)?;
            let caller = &program.machine_states(&program.machines()[caller_machine_index])
                [caller_state_index];
            let caller_statements = program.statement_table.statements(caller.statement_nodes);
            let caller_statement = caller_statements.get(statement_index)?;
            let local_ordinal = u32::try_from(caller_statements[..statement_index].iter().filter(|statement| {
                matches!(statement, StatementNode::LocalData(local)
                    if !local.is_mutable && program.primitive_type_reference(local.type_reference).is_some())
            }).count()).ok()?;
            let receiver_place = if call.receiver.is_valid() {
                let place = place_at(program, caller_state, statement_index, call.receiver)?;
                if !matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
                    || !place
                        .segments
                        .iter()
                        .all(crate::flow::fixed_selection_segment)
                {
                    return None;
                }
                Some(place)
            } else {
                None
            };
            let occurrence = exact_call_occurrence(
                program,
                borrow,
                context,
                caller_state,
                statement_index,
                source,
                call,
                receiver_place.as_ref().and_then(|place| match place.root {
                    facts::PlaceRoot::Symbol(symbol) => Some(symbol),
                    _ => None,
                }),
            )?;
            let entered_target = validation::call_target_entry_symbol(
                program,
                validation::call_target_symbol(program, call),
            );
            let (machine_index, entry_state_index) =
                context.state_location(program, entered_target)?;
            let machine = &program.machines()[machine_index];
            if !machine.body_is_present
                || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !machine.owned_data.is_empty()
            {
                return None;
            }
            let state = program.machine_states(machine).get(entry_state_index)?;
            let parameters = program.state_parameters(state);
            let arguments = program.expression_table.expression_handles(call.arguments);
            if receiver_place.is_some() {
                return None;
            }
            let explicit_parameters: Vec<_> = parameters.iter().collect();
            if explicit_parameters.iter().any(|parameter| {
                crate::execution::terminal_unit::strips_erased_parameter(parameter).is_none()
            }) {
                return None;
            }
            let scalar_parameters: Vec<_> = explicit_parameters
                .iter()
                .copied()
                .filter(|parameter| !parameter.relevance.is_erased())
                .collect();
            if arguments.len() != explicit_parameters.len()
                || scalar_parameters.iter().any(|parameter| {
                    parameter.is_const
                        || (parameter.is_mutable
                            && crate::values::mutable_scalar_parameter_type(program, parameter)
                                .is_none())
                        || program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
            {
                return None;
            }
            let live = LiveValues {
                program,
                semantic,
                context,
                state: caller_state,
                statement_index,
                active,
            };
            let call_ordinal = u32::try_from(occurrence.call_ordinal).ok()?;
            let mut next_argument_ordinal = 0u32;
            let argument_values = arguments
                .iter()
                .enumerate()
                .filter(|(argument_index, _)| {
                    explicit_parameters
                        .get(*argument_index)
                        .is_some_and(|parameter| !parameter.relevance.is_erased())
                })
                .map(|(argument_index, argument)| {
                    let statement_ordinal = u32::try_from(statement_index).ok()?;
                    let argument_ordinal = next_argument_ordinal;
                    next_argument_ordinal = next_argument_ordinal.checked_add(1)?;
                    let role = match caller_statement {
                        StatementNode::LocalData(_) => CheckedScalarExpressionRole::CallArgument {
                            binding_ordinal: local_ordinal,
                            argument_ordinal,
                        },
                        StatementNode::Assignment(_) => {
                            CheckedScalarExpressionRole::UnitCallArgument {
                                call_ordinal,
                                argument_ordinal,
                            }
                        }
                        _ => return None,
                    };
                    match context.selected_scalar_at(caller_state, statement_ordinal, |binding| {
                        binding.role == role
                    }) {
                        None => {}
                        Some(None) => return None,
                        Some(Some((binding, expression))) => {
                            if binding.expression != *argument || binding.destination.is_valid() {
                                return None;
                            }
                            return Value::evaluate_live(
                                expression,
                                context
                                    .scalar_expressions
                                    .binding_symbols
                                    .span_or_empty(binding.symbols),
                                &live,
                            );
                        }
                    }
                    if let Some(value) =
                        Value::literal(program.expression_table.expression(*argument))
                    {
                        return Some(value);
                    }
                    let parameter = parameters.get(argument_index)?;
                    if Value::REQUIRES_SELECTED_ARGUMENTS && !parameter.is_mutable {
                        return Value::formal_fallback(program, parameter);
                    }
                    if let Some(place) = place_at(program, caller_state, statement_index, *argument)
                        && place
                            .segments
                            .iter()
                            .all(crate::flow::fixed_selection_segment)
                        && let Some(value) = Value::at_place(&place, &live)
                    {
                        return Some(value);
                    }
                    Value::formal_fallback(program, parameter)
                })
                .collect::<Option<Vec<_>>>()?;
            let symbols: Vec<_> = scalar_parameters
                .iter()
                .map(|parameter| parameter.symbol)
                .collect();
            let mut values = CallValues {
                bindings: Vec::with_capacity(scalar_parameters.len()),
                storage: Vec::new(),
                fields: Vec::new(),
            };
            for (parameter, value) in scalar_parameters.iter().zip(argument_values) {
                if !parameter.symbol.is_valid()
                    || parameters
                        .iter()
                        .filter(|candidate| candidate.symbol == parameter.symbol)
                        .count()
                        != 1
                {
                    return None;
                }
                if parameter.is_mutable {
                    values.bindings.push(None);
                    values.storage.push((parameter.symbol, value));
                } else {
                    values.bindings.push(Some(value));
                }
            }
            normal_returns::capture(program, borrow, context, machine, state, symbols, values)
        }

        /*
            The borrow row for this exact call occurrence. A statement can carry
            further call occurrences beside the captured one — an index selector on
            the target is itself a call — so the row is selected by target, receiver
            presence, and receiver root, and its `call_ordinal` must resolve back to
            this authored call expression: the ordinal is the occurrence key, not a
            trusted field on the row.
        */
        fn exact_call_occurrence<'facts>(
            program: &dyn ::validation::CheckingInput,
            borrow: &'facts BorrowFacts,
            context: &mut FlowBuildContext,
            caller_state: SymbolHandle,
            statement_index: usize,
            source: typed_trees::expression::ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            receiver_root: Option<SymbolHandle>,
        ) -> Option<&'facts checked_trees::BorrowCallFact> {
            let (machine_index, caller_state_index) =
                context.state_location(program, caller_state)?;
            let owner = &program.machines()[machine_index];
            let caller = &program.machine_states(owner)[caller_state_index];
            let state = borrow.unique_state_borrow(owner.symbol, caller_state)?;
            let receiver_root = receiver_root.unwrap_or_default();
            let mut calls = borrow
                .calls
                .span_or_empty(state.calls)
                .iter()
                .filter(|candidate| {
                    candidate.statement_index == statement_index
                        && candidate.target_symbol
                            == validation::call_target_entry_symbol(
                                program,
                                validation::call_target_symbol(program, call),
                            )
                        && candidate.has_receiver == call.receiver.is_valid()
                        && candidate.receiver_symbol == receiver_root
                        && matches!(
                            validation::find_call_site_in_state(
                                program,
                                owner,
                                caller,
                                statement_index,
                                candidate.call_ordinal,
                            ),
                            Some(validation::CallSite::Expression { expression, .. })
                                if expression == source
                        )
                });
            let captured = calls.next()?;
            if calls.next().is_some() {
                return None;
            }
            Some(captured)
        }
    }
    mod captured {
        /*
            Payload evaluation for the shared selected-call custody walk.
        */
        use crate::values::CurrentIntegerBounds;
        use crate::values::LivePlaceIntervals;
        use crate::values::PlaceScalarValues;
        use crate::values::admitted_integer;
        use crate::values::evaluate_checked_scalar;
        use crate::values::integer_bounds_at_place;
        use crate::values::integer_magnitude;
        use crate::values::integer_type;
        use crate::values::scalar_value_at_place;
        use typed_trees::signature::StateParameter;
        use typed_trees::types::PrimitiveType;
        use validation::{
            CanonicalPlace, SelectedScalarIntervalSource, carrier_integer_range,
            expression_type_reference_in_state, integer_range_contains, selected_boolean_decision,
            selected_scalar_interval,
        };

        use super::ScalarValue;
        use super::conversions::SelectedConversion;

        use crate::flow::transfers::scalar_values::CallValues;
        use crate::flow::{FlowBuildContext, active_context_handles};
        use crate::published::places::place_at;
        use arena::HandleSpan;
        use checked_trees::expression::ExpressionHandle;
        use checked_trees::expression::ExpressionNode;
        use checked_trees::{
            CheckedScalarExpression as ScalarExpression, CheckedStructuralPredicatePathSegment,
        };
        use checked_trees::{
            CheckedScalarReadProjection, CheckedScalarSelector, CheckedStorageRoot,
            FlowSemanticContextRef,
        };
        use facts::IntegerRange;
        use facts::{FactContext, FactPlan};
        use symbols::SymbolHandle;

        pub(super) struct LiveValues<'a, 'plans> {
            pub program: &'a dyn ::validation::CheckingInput,
            pub semantic: &'a FactPlan,
            pub context: &'a FlowBuildContext<'plans>,
            pub state: SymbolHandle,
            pub statement_index: usize,
            pub active: HandleSpan<FlowSemanticContextRef>,
        }

        impl LiveValues<'_, '_> {
            /* Each active semantic context, resolved against the plan's arena. */
            fn active_contexts(&self) -> impl Iterator<Item = &FactContext> + '_ {
                self.context
                    .semantic_context_views(self.semantic, self.active)
            }
        }

        pub(super) trait CapturedValue: Sized + Clone {
            const REQUIRES_SELECTED_ARGUMENTS: bool;
            fn literal(expression: &ExpressionNode) -> Option<Self>;
            fn at_place(place: &CanonicalPlace, live: &LiveValues<'_, '_>) -> Option<Self>;
            fn evaluate_live(
                expression: &ScalarExpression,
                symbols: &[SymbolHandle],
                live: &LiveValues<'_, '_>,
            ) -> Option<Self>;
            fn evaluate_local(
                expression: &ScalarExpression,
                values: &mut CallValues<Self>,
            ) -> Option<Self>;
            fn boolean(
                expression: &ScalarExpression,
                values: &mut CallValues<Self>,
            ) -> Option<bool>;
            fn join(self, other: Self) -> Option<Self>;
            /*
                Fallback for an argument with no live snapshot: its runtime value must
                still satisfy the formal's declared scalar type, so that declaration —
                at worst the raw carrier — bounds the incoming value.
            */
            fn formal_fallback(
                _program: &dyn ::validation::CheckingInput,
                _parameter: &StateParameter,
            ) -> Option<Self> {
                None
            }
            /* The value held at the place `expression` names in `live`'s state. */
            fn value_at_expression(
                program: &dyn ::validation::CheckingInput,
                live: &LiveValues<'_, '_>,
                statement_index: usize,
                expression: ExpressionHandle,
            ) -> Option<Self> {
                let place = place_at(program, live.state, statement_index, expression)?;
                Self::at_place(&place, live)
            }
            /*
                Evaluate a non-call operand below authored conversions: a literal or a
                caller place with a live snapshot or declared storage invariant. Never
                a source replay of nested computation the plan did not retain.
            */
            fn operand(
                _program: &dyn ::validation::CheckingInput,
                _live: &LiveValues<'_, '_>,
                _statement_index: usize,
                _expression: ExpressionHandle,
            ) -> Option<Self> {
                None
            }
            /*
                Apply one authored conversion step to a captured call result. A step
                that cannot describe a normal-return value fails the capture, never a
                guess at the destination carrier.
            */
            fn convert(self, conversion: &SelectedConversion) -> Option<Self>;
        }

        impl CapturedValue for ScalarValue {
            const REQUIRES_SELECTED_ARGUMENTS: bool = false;
            fn literal(expression: &ExpressionNode) -> Option<Self> {
                match expression {
                    ExpressionNode::Integer(value) => value.value_bignum().map(Self::Integer),
                    ExpressionNode::Boolean(value) => Some(Self::Boolean(*value)),
                    _ => None,
                }
            }

            fn at_place(place: &CanonicalPlace, live: &LiveValues<'_, '_>) -> Option<Self> {
                scalar_value_at_place(live.program, live.semantic, live.active_contexts(), place)
            }

            fn evaluate_live(
                expression: &ScalarExpression,
                symbols: &[SymbolHandle],
                live: &LiveValues<'_, '_>,
            ) -> Option<Self> {
                evaluate_checked_scalar(
                    expression,
                    &mut PlaceScalarValues {
                        program: live.program,
                        parameters: live
                            .program
                            .state_parameters(live.program.state_by_symbol(live.state)?),
                        symbols,
                        value_at_place: |place: &CanonicalPlace, _observation| {
                            Self::at_place(place, live)
                        },
                    },
                )
            }

            fn evaluate_local(
                expression: &ScalarExpression,
                values: &mut CallValues<Self>,
            ) -> Option<Self> {
                evaluate_checked_scalar(expression, values)
            }

            fn boolean(
                expression: &ScalarExpression,
                values: &mut CallValues<Self>,
            ) -> Option<bool> {
                match Self::evaluate_local(expression, values)? {
                    Self::Boolean(value) => Some(value),
                    _ => None,
                }
            }

            fn join(self, other: Self) -> Option<Self> {
                (self == other).then_some(self)
            }

            fn operand(
                program: &dyn ::validation::CheckingInput,
                live: &LiveValues<'_, '_>,
                statement_index: usize,
                expression: ExpressionHandle,
            ) -> Option<Self> {
                if let Some(value) = Self::literal(program.expression_table.expression(expression))
                {
                    return Some(value);
                }
                Self::value_at_expression(program, live, statement_index, expression)
            }

            fn convert(self, conversion: &SelectedConversion) -> Option<Self> {
                let ScalarValue::Integer(value) = self else {
                    return None;
                };
                if conversion.source == conversion.target {
                    return Some(Self::Integer(value));
                }
                let source = integer_type(conversion.source)?;
                let target = integer_type(conversion.target)?;
                let admitted = admitted_integer(source, &value)?;
                let converted = source.widen_value_to(target, admitted)?;
                Some(Self::Integer(integer_magnitude(converted)))
            }
        }

        impl CapturedValue for IntegerRange {
            const REQUIRES_SELECTED_ARGUMENTS: bool = true;
            fn literal(expression: &ExpressionNode) -> Option<Self> {
                let ExpressionNode::Integer(literal) = expression else {
                    return None;
                };
                let value = literal.value_bignum()?;
                Some(Self {
                    minimum: value.clone(),
                    maximum: value,
                })
            }

            fn at_place(place: &CanonicalPlace, live: &LiveValues<'_, '_>) -> Option<Self> {
                integer_bounds_at_place(live.program, live.semantic, live.active_contexts(), place)
            }

            fn evaluate_live(
                expression: &ScalarExpression,
                symbols: &[SymbolHandle],
                live: &LiveValues<'_, '_>,
            ) -> Option<Self> {
                let contexts = active_context_handles(&live.context.contexts, live.active);
                selected_scalar_interval(
                    expression,
                    &mut LivePlaceIntervals {
                        program: live.program,
                        semantic: live.semantic,
                        contexts: &contexts,
                        parameters: live
                            .program
                            .state_parameters(live.program.state_by_symbol(live.state)?),
                        symbols,
                        state: live.state,
                        contents: CurrentIntegerBounds::at_statement(
                            live.program,
                            live.context.operators,
                            live.semantic,
                            &contexts,
                            live.state,
                            live.statement_index,
                        )?,
                    },
                )
            }

            fn evaluate_local(
                expression: &ScalarExpression,
                values: &mut CallValues<Self>,
            ) -> Option<Self> {
                selected_scalar_interval(expression, values)
            }

            fn boolean(
                expression: &ScalarExpression,
                values: &mut CallValues<Self>,
            ) -> Option<bool> {
                let ScalarExpression::Boolean(expression) = expression else {
                    return None;
                };
                selected_boolean_decision(expression, values)
            }

            fn join(self, other: Self) -> Option<Self> {
                Some(Self {
                    minimum: self.minimum.min(other.minimum),
                    maximum: self.maximum.max(other.maximum),
                })
            }

            fn formal_fallback(
                program: &dyn ::validation::CheckingInput,
                parameter: &StateParameter,
            ) -> Option<Self> {
                let primitive = program.primitive_type_reference(parameter.type_reference)?;
                carrier_integer_range(primitive)
            }

            fn operand(
                program: &dyn ::validation::CheckingInput,
                live: &LiveValues<'_, '_>,
                statement_index: usize,
                expression: ExpressionHandle,
            ) -> Option<Self> {
                if let Some(value) = Self::literal(program.expression_table.expression(expression))
                {
                    return Some(value);
                }
                let place = place_at(program, live.state, statement_index, expression)?;
                let primitive =
                    program.primitive_type_reference(expression_type_reference_in_state(
                        program,
                        live.state,
                        statement_index,
                        expression,
                    )?)?;
                let contexts = active_context_handles(&live.context.contexts, live.active);
                LivePlaceIntervals {
                    program,
                    semantic: live.semantic,
                    contexts: &contexts,
                    parameters: program.state_parameters(program.state_by_symbol(live.state)?),
                    symbols: &[],
                    state: live.state,
                    contents: CurrentIntegerBounds::at_statement(
                        program,
                        live.context.operators,
                        live.semantic,
                        &contexts,
                        live.state,
                        statement_index,
                    )?,
                }
                .interval_at_place(&place, primitive)
            }

            fn convert(self, conversion: &SelectedConversion) -> Option<Self> {
                let carrier = carrier_integer_range(conversion.target)?;
                integer_range_contains(&carrier, &self).then_some(self)
            }
        }

        impl SelectedScalarIntervalSource for CallValues<IntegerRange> {
            fn binding(&mut self, position: usize, _: PrimitiveType) -> Option<IntegerRange> {
                self.bindings.get(position)?.clone()
            }

            fn storage(&mut self, symbol: SymbolHandle, _: PrimitiveType) -> Option<IntegerRange> {
                self.storage
                    .iter()
                    .find(|(candidate, _)| *candidate == symbol)
                    .map(|(_, bounds)| bounds.clone())
            }

            fn structural_field(
                &mut self,
                position: u32,
                path: &[CheckedStructuralPredicatePathSegment],
            ) -> Option<IntegerRange> {
                self.fields
                    .iter()
                    .find(|(candidate_position, candidate, _)| {
                        *candidate_position == position && candidate.as_slice() == path
                    })
                    .map(|(_, _, bounds)| bounds.clone())
            }

            fn indexed_read(
                &mut self,
                _: CheckedStorageRoot,
                _: &[CheckedStructuralPredicatePathSegment],
                _: &CheckedScalarSelector,
                _: &[CheckedScalarReadProjection],
                _: PrimitiveType,
            ) -> Option<IntegerRange> {
                None
            }
        }
    }
    mod conversions {
        /*
            A selected call result can still be a selected call when the statement
            wraps it in authored value conversions before landing it. Each conversion
            is classified once against the callee's declared carrier. A same-carrier
            retag or total inclusion keeps the captured result; a partial conversion
            is the arithmetic engine's decision, which flow does not read, so it fails
            the capture, never a widened guess.
        */
        use super::captured::CapturedValue;
        use checked_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;

        /*
            A call nested under authored integer conversions: the call itself plus the
            conversion steps applied to its result, innermost first. `expression` is
            the authored call node that identifies this exact call occurrence.
        */
        pub(super) struct SelectedCall<'a> {
            pub call: &'a typed_trees::expression::TableCallExpression,
            pub expression: ExpressionHandle,
            conversions: Vec<SelectedConversion>,
        }

        /*
            One conversion step over the running result carrier: a same-carrier retag
            or a total carrier inclusion, matching `IntegerWiden`'s lowering selection.
        */
        pub(super) struct SelectedConversion {
            pub source: PrimitiveType,
            pub target: PrimitiveType,
        }

        impl SelectedCall<'_> {
            /* Fold the captured call result through each authored conversion. */
            pub(super) fn convert<Value: CapturedValue>(&self, value: Value) -> Option<Value> {
                convert_through(&self.conversions, value)
            }
        }

        /*
            The authored-conversion fold both selected operands share: peel each
            conversion in order, failing on the first that cannot apply.
        */
        fn convert_through<Value: CapturedValue>(
            conversions: &[SelectedConversion],
            mut value: Value,
        ) -> Option<Value> {
            for conversion in conversions {
                value = value.convert(conversion)?;
            }
            Some(value)
        }

        /*
            A non-call operand under authored value conversions: the operand resolves
            against live caller facts or its declared storage instead of a callee body.
            This is the same conversion fold as `SelectedCall` with a different
            operand source, reached only when the selected scalar plan retained no
            form for the statement's source — today that is exactly the policies
            `construct_integer_cast` cannot express, like `Saturating`.
        */
        pub(super) struct SelectedOperand {
            pub operand: ExpressionHandle,
            conversions: Vec<SelectedConversion>,
        }

        impl SelectedOperand {
            pub(super) fn convert<Value: CapturedValue>(&self, value: Value) -> Option<Value> {
                convert_through(&self.conversions, value)
            }
        }

        /*
            Peel value casts around a non-call operand. The operand must resolve to a
            caller place or a literal — a nested computation or a call stays with the
            selected plan/`selected_call` routes that own it. The operand's own
            carrier starts the conversion fold, mirroring the callee return type in
            `selected_call`.
        */

        /*
            Peel the transparent-cast prefix of `source`: non-recast, non-domain casts
            accumulate until the first non-cast expression, which is returned with the
            accumulated chain.
        */
        fn peel_transparent_casts(
            program: &dyn ::validation::CheckingInput,
            source: ExpressionHandle,
        ) -> Option<(
            ExpressionHandle,
            Vec<(
                ExpressionHandle,
                &typed_trees::expression::TableCastExpression,
            )>,
        )> {
            let mut casts = Vec::new();
            let mut expression = source;
            loop {
                if !program.expression_table.expression_is_valid(expression) {
                    return None;
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Cast(cast)
                        if !cast.form.is_recast() && cast.semantic_domain.is_empty() =>
                    {
                        casts.push((expression, cast));
                        expression = cast.value;
                    }
                    _ => return Some((expression, casts)),
                }
            }
        }

        /*
            Classify every cast in a peeled chain, advancing the running carrier
            through each conversion's target.
        */
        fn classify_cast_chain(
            program: &dyn ::validation::CheckingInput,
            casts: &[(
                ExpressionHandle,
                &typed_trees::expression::TableCastExpression,
            )],
            mut source_type: PrimitiveType,
        ) -> Option<Vec<SelectedConversion>> {
            validation::carrier_integer_range(source_type)?;
            let mut conversions = Vec::with_capacity(casts.len());
            for (_, cast) in casts.iter().rev() {
                let target_type = program.primitive_type_reference(cast.target_type)?;
                let conversion = classify(source_type, target_type)?;
                source_type = conversion.target;
                conversions.push(conversion);
            }
            Some(conversions)
        }

        pub(super) fn selected_operand(
            program: &dyn ::validation::CheckingInput,
            state: symbols::SymbolHandle,
            statement_index: usize,
            source: ExpressionHandle,
        ) -> Option<SelectedOperand> {
            let (operand, casts) = peel_transparent_casts(program, source)?;
            if casts.is_empty()
                || matches!(
                    program.expression_table.expression(operand),
                    ExpressionNode::Call(_)
                )
            {
                return None;
            }
            let source_type = match program.expression_table.expression(operand) {
                ExpressionNode::Integer(literal) => {
                    PrimitiveType::non_address_landed_integer(literal.landing()?.landed_type)?
                }
                _ => program.primitive_type_reference(
                    validation::expression_type_reference_in_state(
                        program,
                        state,
                        statement_index,
                        operand,
                    )?,
                )?,
            };
            let conversions = classify_cast_chain(program, &casts, source_type)?;
            Some(SelectedOperand {
                operand,
                conversions,
            })
        }

        /*
            Peel value casts around a selected call. A borrow recast or a
            semantic-domain suffix retires the slice outright; a remaining non-cast,
            non-call operand is simply not a qualified call. Every cast between the
            call and the landing must classify against the running carrier — this
            keeps `scalar_qualified_call_expression`'s same-carrier transparency and
            extends it to occurrence-proved fixed-integer carrier conversions.
        */
        pub(super) fn selected_call<'a>(
            program: &'a dyn ::validation::CheckingInput,
            source: ExpressionHandle,
        ) -> Option<SelectedCall<'a>> {
            let (expression, casts) = peel_transparent_casts(program, source)?;
            let call = match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => call,
                _ => return None,
            };
            if casts.is_empty() {
                return Some(SelectedCall {
                    call,
                    expression,
                    conversions: Vec::new(),
                });
            }
            let callee = program.state_by_symbol(call.target_symbol)?;
            let source_type = program.primitive_type_reference(callee.return_type)?;
            let conversions = classify_cast_chain(program, &casts, source_type)?;
            Some(SelectedCall {
                call,
                expression,
                conversions,
            })
        }

        /*
            Classify one authored cast over the running result carrier, in
            `construct_integer_cast`'s order: a same-carrier retag or a total inclusion
            is a widening. A partial conversion has no flow-side value.
        */
        fn classify(source: PrimitiveType, target: PrimitiveType) -> Option<SelectedConversion> {
            validation::carrier_integer_range(target)?;
            (source == target || validation::integer_widen_is_total(source, target))
                .then_some(SelectedConversion { source, target })
        }
    }
    use calls::capture_call;
    use captured::{CapturedValue, LiveValues};
    use conversions::{selected_call, selected_operand};

    /* A statement's written value source, when it carries one. */
    fn statement_value_source(
        statement: &StatementNode,
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        Some(match statement {
            StatementNode::LocalData(local) => local.ordinary_value_initializer()?,
            StatementNode::Assignment(assignment) => assignment.value,
            _ => return None,
        })
    }

    /*
        The value `source`'s selected call captures, when the cast context
        selects one. `None` means no call was selected.
    */
    #[allow(clippy::too_many_arguments)]
    fn selected_call_capture<Value: CapturedValue>(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        semantic: &FactPlan,
        context: &mut FlowBuildContext,
        state: SymbolHandle,
        statement_index: usize,
        source: typed_trees::expression::ExpressionHandle,
        active: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<Option<Value>> {
        let selected = selected_call(program, source)?;
        Some(
            capture_call(
                program,
                borrow,
                semantic,
                context,
                state,
                statement_index,
                selected.expression,
                selected.call,
                active,
            )
            .and_then(|value| selected.convert(value)),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_statement(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        semantic: &FactPlan,
        context: &mut FlowBuildContext,
        machine_symbol: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        active: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<ScalarValue> {
        let source = statement_value_source(statement)?;
        if !program.expression_table.expression_is_valid(source) {
            return None;
        }
        if let Some(value) = completed_statement_scalar(
            program,
            semantic,
            context,
            state,
            statement_index,
            statement,
            source,
            active,
        ) {
            return Some(value);
        }
        if let Some(captured) = selected_call_capture(
            program,
            borrow,
            semantic,
            context,
            state,
            statement_index,
            source,
            active,
        ) {
            return captured;
        }
        let builtin = builtin_bound_meaning_source(program, context, machine_symbol, state, source);
        if let Some((expression, symbols)) =
            selected_statement(program, context, state, statement_index, statement)
            && builtin
            && let Some(value) = crate::values::evaluate_checked_scalar(
                expression,
                &mut crate::values::PlaceScalarValues {
                    program,
                    parameters: program.state_parameters(program.state_by_symbol(state)?),
                    symbols,
                    value_at_place: |place: &CanonicalPlace, _observation| {
                        crate::values::scalar_value_at_place(
                            program,
                            semantic,
                            context.semantic_context_views(semantic, active),
                            place,
                        )
                    },
                },
            )
        {
            return Some(value);
        }
        let live = LiveValues {
            program,
            semantic,
            context,
            state,
            statement_index,
            active,
        };
        if let Some(value) =
            ScalarValue::value_at_expression(program, &live, statement_index, source)
        {
            return Some(value);
        }
        selected_live_capture(
            program,
            semantic,
            context,
            state,
            active,
            statement_index,
            source,
        )
    }

    /*
        A completed scalar expression can carry its value before it has a named
        source carrier, as with an anonymous-literal match. Consume that snapshot,
        then use the existing explicit destination admission. This is not source
        replay, a carrier guess, or a way to turn a reference result into a value.
    */
    #[allow(clippy::too_many_arguments)]
    fn completed_statement_scalar(
        program: &dyn validation::CheckingInput,
        semantic: &FactPlan,
        context: &FlowBuildContext<'_>,
        state: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        source: ExpressionHandle,
        active: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<ScalarValue> {
        let (place, primitive) =
            completed_statement_place(program, state, statement_index, statement, source)?;
        let value = crate::values::scalar_value_at_place(
            program,
            semantic,
            context.semantic_context_views(semantic, active),
            &place,
        )?;
        match &value {
            ScalarValue::Integer(value) => {
                crate::values::admitted_integer(crate::values::integer_type(primitive)?, value)?;
            }
            ScalarValue::Boolean(_) if primitive == typed_trees::types::PrimitiveType::Bool => {}
            _ => return None,
        }
        Some(value)
    }

    fn completed_statement_place(
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        source: ExpressionHandle,
    ) -> Option<(CanonicalPlace, typed_trees::types::PrimitiveType)> {
        if !program.expression_table.expression_is_valid(source) {
            return None;
        }
        let place = CanonicalPlace {
            root: facts::PlaceRoot::Expression(source),
            segments: Vec::new(),
        };
        let destination = match statement {
            StatementNode::LocalData(local) => local.type_reference,
            StatementNode::Assignment(assignment) => {
                validation::expression_type_reference_in_state(
                    program,
                    state,
                    statement_index,
                    assignment.target,
                )?
            }
            _ => return None,
        };
        let destination = validation::peel_declared_constraints(program, destination);
        if matches!(
            program.type_reference_table.type_reference(destination),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            return None;
        }
        let primitive = program.primitive_type_reference(destination)?;
        Some((place, primitive))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_bounds(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        semantic: &FactPlan,
        context: &mut FlowBuildContext,
        machine_symbol: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        active: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<facts::IntegerRange> {
        let source = statement_value_source(statement)?;
        if let Some((place, primitive)) =
            completed_statement_place(program, state, statement_index, statement, source)
            && crate::values::integer_type(primitive).is_some()
            && let Some(bounds) = crate::values::integer_bounds_at_place(
                program,
                semantic,
                context.semantic_context_views(semantic, active),
                &place,
            )
        {
            return Some(bounds);
        }

        if let Some(captured) = selected_call_capture(
            program,
            borrow,
            semantic,
            context,
            state,
            statement_index,
            source,
            active,
        ) {
            return captured.or_else(|| {
                let selected = selected_call(program, source)?;
                let place = place_at(program, state, statement_index, selected.expression)?;
                selected.convert(crate::values::integer_bounds_at_place(
                    program,
                    semantic,
                    context.semantic_context_views(semantic, active),
                    &place,
                )?)
            });
        }
        let builtin = builtin_bound_meaning_source(program, context, machine_symbol, state, source);
        let located = context.state_location(program, state);
        if let Some((expression, symbols)) =
            selected_statement(program, context, state, statement_index, statement)
            && builtin
        {
            let contexts = crate::flow::active_context_handles(&context.contexts, active);
            let (machine_index, state_index) = located?;
            return validation::selected_scalar_interval(
                expression,
                &mut crate::values::LivePlaceIntervals {
                    program,
                    semantic,
                    contexts: &contexts,
                    parameters: program.state_parameters(
                        &program.machine_states(&program.machines()[machine_index])[state_index],
                    ),
                    symbols,
                    state,
                    contents: crate::values::CurrentIntegerBounds::at_statement(
                        program,
                        context.operators,
                        semantic,
                        &contexts,
                        state,
                        statement_index,
                    )?,
                },
            );
        }
        selected_live_capture(
            program,
            semantic,
            context,
            state,
            active,
            statement_index,
            source,
        )
    }

    /*
        The captured value one authored-conversion source yields: a literal or a
        caller place under the live snapshot, converted when the selection spelled
        conversions. The selected plan has no form for every cast policy.
    */
    fn selected_live_capture<Value: CapturedValue>(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        context: &FlowBuildContext<'_>,
        state: SymbolHandle,
        active: HandleSpan<FlowSemanticContextRef>,
        statement_index: usize,
        source: ExpressionHandle,
    ) -> Option<Value> {
        let operand = selected_operand(program, state, statement_index, source)?;
        let live = LiveValues {
            program,
            semantic,
            context,
            state,
            statement_index,
            active,
        };
        Value::operand(program, &live, statement_index, operand.operand)
            .and_then(|value| operand.convert(value))
    }

    /*
        A selected scalar plan folds every retained operation under builtin
        meaning. The checked operator row's BuiltinFallback is not builtin
        authority while a declared or selected authored candidate still matches
        the operands, so only a source whose complete bound subtree retains
        builtin meaning may be captured as this statement's recorded evidence.
    */
    fn builtin_bound_meaning_source(
        program: &dyn ::validation::CheckingInput,
        context: &mut FlowBuildContext,
        machine_symbol: SymbolHandle,
        state: SymbolHandle,
        source: ExpressionHandle,
    ) -> bool {
        let Some(machine_index) = context.machine_index(program, machine_symbol) else {
            return false;
        };
        let machine = &program.machines()[machine_index];
        let state = context
            .state_index_in_machine(program, machine_symbol, state)
            .and_then(|index| program.machine_states(machine).get(index));
        validation::has_builtin_bound_expression_meaning(program, machine, state, source)
    }

    fn selected_statement<'plans>(
        program: &dyn ::validation::CheckingInput,
        context: &'plans FlowBuildContext<'plans>,
        state: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
    ) -> Option<(
        &'plans checked_trees::CheckedScalarExpression,
        &'plans [SymbolHandle],
    )> {
        let (source, destination) = match statement {
            StatementNode::LocalData(local) => (local.ordinary_value_initializer()?, local.symbol),
            StatementNode::Assignment(assignment) => (
                assignment.value,
                match program.expression_table.expression(assignment.target) {
                    ExpressionNode::Name(path) => path.symbol,
                    _ => SymbolHandle::invalid(),
                },
            ),
            _ => return None,
        };
        if !program.expression_table.expression_is_valid(source) {
            return None;
        }
        let statement_ordinal = u32::try_from(statement_index).ok()?;
        let (binding, expression) = context
            .selected_scalar_at(state, statement_ordinal, |binding| {
                binding.expression == source
                    && binding.destination == destination
                    && match statement {
                        StatementNode::LocalData(local) if local.is_mutable => {
                            binding.role == CheckedScalarExpressionRole::StorageInitializer
                        }
                        StatementNode::LocalData(_) => matches!(
                            binding.role,
                            CheckedScalarExpressionRole::LocalInitializer { .. }
                        ),
                        StatementNode::Assignment(_) => {
                            binding.role == CheckedScalarExpressionRole::AssignmentValue
                        }
                        _ => false,
                    }
            })
            .flatten()?;
        Some((
            expression,
            context
                .scalar_expressions
                .binding_symbols
                .span_or_empty(binding.symbols),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn retains_values_across_unit_call<Value>(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        context: &mut FlowBuildContext,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        call: &typed_trees::statement::TableCall,
        symbols: &[SymbolHandle],
        values: &CallValues<Value>,
    ) -> Option<()> {
        if call.receiver_symbol.is_valid()
            || call.receiver_root_symbol.is_valid()
            || !call.receiver.is_empty()
            || !call.machine_arguments.is_empty()
            || call.static_requirement_dispatch.is_some()
            || call.discards_result
        {
            return None;
        }
        let entered_target = validation::call_target_entry_symbol(program, call.target_symbol);
        let (machine_index, state_index) = context.state_location(program, entered_target)?;
        let callee = &program.machines()[machine_index];
        let entry = &program.machine_states(callee)[state_index];
        if state_index != 0 {
            return None;
        }
        if !callee.body_is_present
            || callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !callee.owned_data.is_empty()
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(entry.return_type),
                typed_trees::types::TypeReferenceNode::Unit
            )
        {
            return None;
        }
        let arguments = program.statement_table.expression_handles(call.arguments);
        let parameters = program.state_parameters(entry);
        if arguments.len() != parameters.len()
            || parameters.iter().any(|parameter| {
                (parameter.is_const)
                    || program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            || arguments
                .iter()
                .enumerate()
                .any(|(argument_ordinal, argument)| {
                    match program.expression_table.expression(*argument) {
                        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => return false,
                        ExpressionNode::Name(path) => {
                            return !symbols.contains(&path.symbol)
                                && !values
                                    .storage
                                    .iter()
                                    .any(|(symbol, _)| *symbol == path.symbol);
                        }
                        _ => {}
                    }
                    !selected_scalar_argument(
                        context,
                        state.symbol,
                        statement_index,
                        argument_ordinal,
                        *argument,
                    )
                })
        {
            return None;
        }
        let borrowed_state = borrow.unique_state_borrow(machine.symbol, state.symbol)?;
        let mut calls = borrow
            .calls
            .span_or_empty(borrowed_state.calls)
            .iter()
            .filter(|candidate| candidate.statement_index == statement_index);
        let borrowed_call = calls.next()?;
        if calls.next().is_some()
            || borrowed_call.call_ordinal != 0
            || borrowed_call.target_symbol != entered_target
            || borrowed_call.has_receiver
            || borrowed_call.receiver_symbol.is_valid()
        {
            return None;
        }
        call_storage_writes(program, borrow, context, machine, state, borrowed_call)?
            .observation_places
            .is_empty()
            .then_some(())
    }

    /*
        Whether the computation plan retained `argument` as the selected scalar
        operand of the statement's only call. The binding and its checked form must
        each occur exactly once at the coordinate; selected scalars cannot express
        nested calls, borrows, or storage writes.
    */
    fn selected_scalar_argument(
        context: &FlowBuildContext,
        state: SymbolHandle,
        statement_index: usize,
        argument_ordinal: usize,
        argument: ExpressionHandle,
    ) -> bool {
        let (Ok(statement_ordinal), Ok(argument_ordinal)) = (
            u32::try_from(statement_index),
            u32::try_from(argument_ordinal),
        ) else {
            return false;
        };
        context
            .selected_scalar_at(state, statement_ordinal, |binding| {
                binding.expression == argument
                    && !binding.destination.is_valid()
                    && match binding.role {
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal: 0,
                            argument_ordinal: ordinal,
                        }
                        | CheckedScalarExpressionRole::BoundaryCallArgument {
                            call_ordinal: 0,
                            argument_ordinal: ordinal,
                        } => ordinal == argument_ordinal,
                        _ => false,
                    }
            })
            .flatten()
            .is_some()
    }

    /*
        Call-local scratch: immutable bindings use their selected ordinal namespace;
        mutable locals and owned formals use exact storage symbols and cannot alias
        caller storage. Mutable formals leave holes in the immutable namespace.
        `fields` carries each retained self-field read of a receiver call, already
        resolved against the caller's receiver place.
    */
    struct CallValues<Value = ScalarValue> {
        bindings: Vec<Option<Value>>,
        storage: Vec<(SymbolHandle, Value)>,
        fields: Vec<(u32, Vec<CheckedStructuralPredicatePathSegment>, Value)>,
    }

    impl crate::values::ScalarValueSource for CallValues {
        fn binding(&mut self, position: usize) -> Option<ScalarValue> {
            self.bindings.get(position)?.clone()
        }

        fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
            self.storage
                .iter()
                .find(|(candidate, _)| *candidate == symbol)
                .map(|(_, value)| value.clone())
        }

        fn structural_field(
            &mut self,
            _observation: typed_trees::expression::ExpressionHandle,
            parameter_position: u32,
            path: &[CheckedStructuralPredicatePathSegment],
        ) -> Option<ScalarValue> {
            self.fields
                .iter()
                .find(|(position, candidate, _)| {
                    *position == parameter_position && candidate.as_slice() == path
                })
                .map(|(_, _, value)| value.clone())
        }
    }
}
mod write_target {
    /*
        Where one statement writes: the place a `let` binds or an assignment
        stores to, the value's source, and the finite candidates a write through
        a reference local may reach. A selector whose value is already pinned
        narrows to the one element it names.
    */
    use validation::CanonicalPlace;

    use super::{
        ExpressionHandle, FactPlan, FlowBuildContext, PlaceHandle, StatementNode, SymbolHandle,
        contextual_expression_place,
    };

    /* The place one statement writes and the value it writes there. */
    pub(in crate::flow) struct StatementWrite {
        pub(in crate::flow) target_place: PlaceHandle,
        pub(in crate::flow) source_expression: ExpressionHandle,
        pub(in crate::flow) source_place: Option<PlaceHandle>,
        /* The finite origins a write through a reference local may reach. */
        pub(in crate::flow) candidate_targets: Vec<CanonicalPlace>,
        pub(in crate::flow) target_is_exact: bool,
        pub(in crate::flow) stable_value_target: bool,
        /*
            The captured address still has this semantic coordinate after the RHS.
            Geometry/exactness alone cannot freeze intermediate reference slots.
        */
        pub(in crate::flow) destination_is_current: bool,
    }

    pub(in crate::flow) fn statement_write(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        statement: &StatementNode,
        store_target: arena::Handle<checked_trees::FlowOperandFact>,
    ) -> Option<StatementWrite> {
        let mut candidate_targets: Vec<CanonicalPlace> = Vec::new();
        let mut target_is_exact = true;
        let (target_place, source_expression, source_place) = match statement {
            StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => return None,
            StatementNode::LocalData(local) if !local.role.has_ordinary_initializer() => {
                return None;
            }
            StatementNode::LocalData(local_data) => (
                semantic.append_symbol_place(local_data.symbol),
                local_data.initial_value,
                contextual_expression_place(
                    program,
                    semantic,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    validation::storage_operand(program, local_data.initial_value),
                ),
            ),
            StatementNode::Assignment(assignment) => {
                if !build.control.operands.is_valid(store_target) {
                    return None;
                }
                let captured = build.control.operands.get(store_target);
                let targets = build
                    .control
                    .operand_referents
                    .span_or_empty(captured.referents)
                    .iter()
                    .map(|referent| CanonicalPlace {
                        root: referent.root,
                        segments: build
                            .control
                            .operand_referent_segments
                            .span_or_empty(referent.segments)
                            .to_vec(),
                    })
                    .collect::<Vec<_>>();
                let target = targets.first()?;
                let target_place = crate::published::semantic::places::append_place(
                    semantic,
                    target.root,
                    &target.segments,
                );
                if targets.len() != 1 {
                    target_is_exact = false;
                    candidate_targets = targets;
                }
                let source_place = contextual_expression_place(
                    program,
                    semantic,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    validation::storage_operand(program, assignment.value),
                );
                (target_place, assignment.value, source_place)
            }
            StatementNode::Call(_)
            | StatementNode::Expression(_)
            | StatementNode::Transition(_) => {
                return None;
            }
        };
        Some(StatementWrite {
            target_place,
            source_expression,
            source_place,
            candidate_targets,
            target_is_exact,
            stable_value_target: false,
            destination_is_current: true,
        })
    }

    /*
        Narrow each runtime `Index` segment of `target_place` whose selector
        value is pinned in the active contexts, and report whether the narrowed
        place is stable enough to carry value facts.
    */
    pub(in crate::flow) fn narrow_pinned_selectors(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        state_symbol: SymbolHandle,
        statement_index: usize,
        context_handles: &[facts::FactContextHandle],
        mut target_place: PlaceHandle,
    ) -> (PlaceHandle, bool) {
        let existing_segments = semantic
            .place_segments
            .span_or_empty(semantic.places.get(target_place).segments);
        if existing_segments.is_empty() {
            return (target_place, true);
        }
        let mut target_segments: Vec<facts::PlaceSegment> = existing_segments.to_vec();
        if target_segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
        {
            crate::flow::narrow_pinned_index_segments(
                program,
                semantic,
                state_symbol,
                statement_index,
                context_handles,
                &mut target_segments,
            );
        }
        let stable_value_target = crate::flow::compose_captured_view_segments(&mut target_segments)
            .is_some()
            && target_segments
                .iter()
                .all(crate::flow::fixed_selection_segment);
        if stable_value_target
            && target_segments
                != semantic
                    .place_segments
                    .span_or_empty(semantic.places.get(target_place).segments)
        {
            let root = semantic.places.get(target_place).root;
            target_place =
                crate::published::semantic::places::append_place(semantic, root, &target_segments);
        }
        (target_place, stable_value_target)
    }
}
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
    if !write.destination_is_current {
        return;
    }
    let source_expression = write.source_expression;
    let source_place = write.source_place;

    let mut refs = HandleSpan::empty();
    let context_handles = active_context_handles(&build.contexts, *active_contexts);
    let target_place = write.target_place;
    let point = ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    };
    let stable_value_target = write.stable_value_target;

    if let Some(source) = source_place {
        let source = semantic.places.get(source);
        let source = CanonicalPlace {
            root: source.root,
            segments: semantic
                .place_segments
                .span_or_empty(source.segments)
                .to_vec(),
        };
        if !place_cases_are_selected(
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
        let candidates = if write.target_is_exact {
            vec![target_place]
        } else {
            write
                .candidate_targets
                .iter()
                .map(|candidate| append_place(semantic, candidate.root, &candidate.segments))
                .collect()
        };
        for candidate in candidates {
            byte_sequences::append_element_replacement_predicates(
                program,
                semantic,
                build,
                assignment_source_contexts,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.target,
                candidate,
                source_expression,
                scalar_value.as_ref(),
                integer_bounds.as_ref(),
                point,
                &mut refs,
            );
        }
        replacement::preserve_owned_memberships(
            program,
            semantic,
            build,
            &write,
            statement_fact,
            assignment_source_contexts,
            point,
            &mut refs,
        );
    }

    if !write.target_is_exact {
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

    let completed_source = build.control.statements.get(statement_fact).stored_value;
    let completed_source_type = build
        .control
        .operands
        .is_valid(completed_source)
        .then(|| build.control.operands.get(completed_source))
        .filter(|operand| operand.expression == source_expression)
        .map(|operand| operand.type_reference)
        .filter(|reference| reference.is_valid());
    context_transport::transport(
        program,
        semantic,
        &write,
        completed_source_type,
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
        let stored_value = build.control.statements.get(statement_fact).stored_value;
        let destination_type = build
            .control
            .operands
            .is_valid(stored_value)
            .then(|| build.control.operands.get(stored_value).type_reference)
            .filter(|reference| reference.is_valid());
        if let Some(destination_type) = destination_type {
            let source_contexts =
                super::active_context_handles(&build.contexts, assignment_source_contexts);
            owned_qualifications::append_owned_qualification_transfer(
                program,
                semantic,
                &source_contexts,
                source_expression,
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

    if stable_value_target
        && (is_assignment_literal(program, source_expression)
            || (matches!(
                program.expression_table.expression(source_expression),
                ExpressionNode::Call(_)
            )))
    {
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

/*
    Publish one statement's transferred facts as new active contexts, one per
    exact place and observation kind. A tag survives a payload write even
    when the full value at that same place no longer has its old contents.
*/
pub(super) fn publish_statement_contexts(
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    refs: HandleSpan<facts::FactRef>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let point = ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    };
    let references = semantic.refs.span_or_empty(refs);
    let subjects: Vec<_> = references
        .iter()
        .map(|reference| {
            let fact = semantic.facts.get(reference.fact);
            (
                fact.place,
                matches!(fact.payload, FactPayload::CaseValue { .. }),
                fact.payload
                    .domain_membership()
                    .map(|(_, _, symbol, identity)| (symbol, identity)),
            )
        })
        .collect();
    let buckets = facts::places::PlaceBucketIndex::new(subjects.iter().map(
        |(place, observes_tag, domain)| {
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            semantic.fact_place_bucket_hash(*place).hash(&mut hash);
            observes_tag.hash(&mut hash);
            domain.hash(&mut hash);
            hash.finish()
        },
    ));
    let first = buckets.group_equal_positions(|left, right| {
        let (left_place, left_tag, left_domain) = subjects[left];
        let (right_place, right_tag, right_domain) = subjects[right];
        left_tag == right_tag
            && left_domain == right_domain
            && semantic.fact_places_equal(left_place, right_place)
    });
    let mut groups: Vec<Vec<facts::FactRef>> = (0..references.len()).map(|_| Vec::new()).collect();
    for (position, reference) in references.iter().enumerate() {
        groups[first[position]].push(*reference);
    }
    let mut next_contexts =
        retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
    let mut next_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
    for group in groups.into_iter().filter(|group| !group.is_empty()) {
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
            semantic,
            source_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !exact_structural_symbol_place(
            program,
            semantic,
            source_occurrence_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !exact_structural_symbol_place(
            program,
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
    let Some(mut current) = correspondence_root_type_reference(
        program,
        machine_symbol,
        state_symbol,
        formation_statement_index,
        root,
    ) else {
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
                    < state_statements(program, state.statement_nodes).len() =>
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
    canonical_place_to_fact_place_in_state(
        program,
        semantic,
        state_symbol,
        statement_index,
        expression,
    )
}

/*
    A completed value is interpreted in its source and installed destination
    declarations. An alias or recast alone cannot equate different carriers.
*/
fn completed_value_carriers_agree(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    source: &CanonicalPlace,
    target: &CanonicalPlace,
    completed_source: Option<typed_trees::types::TypeReferenceHandle>,
) -> bool {
    let carrier_identity = |reference| {
        let reference = validation::reference_shell(program, reference)
            .map_or(reference, |shell| shell.referee);
        program.normalized_type_identity(reference)
    };
    completed_source
        .or_else(|| {
            validation::canonical_place_type_reference(program, state, statement_index, source)
        })
        .zip(validation::canonical_place_type_reference(
            program,
            state,
            statement_index.saturating_add(1),
            target,
        ))
        .map(|(source, target)| (carrier_identity(source), carrier_identity(target)))
        .is_some_and(|(source, target)| source == target)
}
