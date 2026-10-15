//! Byte operations derive predicates from their operands at assignment time.
//! The destination retains a materialized-value fact, not expressions that
//! would read the mutable operands again at a later contract boundary.
use super::PlaceHandle;
use crate::flow::FlowBuildContext;
use crate::flow::canonical_place_from_semantic_place;
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
    // A window `base[a..b]` and its explicit borrows `&base[a..b]` /
    // `&mut base[a..b]` form one descriptor over the same bytes: the borrow
    // spelling sets the view's access, not its contents, and under
    // `bare-range-reference-access` it is the only mutable spelling.
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
        // Primitive operations prove byte predicates, never routed qualifications.
        if !domain.establishment_routes.is_empty()
            || !typed_trees::domain::index_parameters(program, domain).is_empty()
            || domain.alias.is_some()
            || !domain.predicate_body.is_present()
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
    // A window `base[a..b]` and its explicit borrows `&base[a..b]` /
    // `&mut base[a..b]` form the same descriptor over the same bytes; the
    // borrow spelling sets the view's access, not its contents. Under
    // `bare-range-reference-access` the borrow is the only mutable spelling.
    if let Some(window) = validation::subslice_expression(program, expression)
        && let Some(indexed) = program.expression_table.indexed_node(window)
        && crate::values::operator_is_builtin(operators, window)
    {
        // Preserve the class at descriptor formation. Replaying an initializer
        // later would observe a source binding that may have been replaced.
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
            facts::domain_membership_symbol_and_instance_pattern!(
                domain_symbol,
                semantic_domain
            ) => {
                // The materialized bytes retain the same primitive predicate
                // across bounded carriers. This is not carrier/domain identity.
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

/// An indexed store replaces exactly ONE byte. A PER-BYTE character class
/// (`no_nul`, `ascii_only`) survives that replacement when the stored byte is
/// itself in the class, so whole-carrier evidence outlives a write that retires
/// the exact `AssignedValue` snapshot the carrier had before it.
///
/// `valid_utf8` is deliberately NOT preserved here: an ASCII byte can split a
/// multi-byte scalar, so UTF-8 is reachable only as a CONSEQUENCE of the
/// surviving ASCII class (`ByteSequencePredicate::implies`), never as a class
/// preserved in its own right. The premise is read from the PRE-mutation
/// contexts, so consecutive stores chain: each one re-proves the class from the
/// class the previous store left behind.
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
    // Invalidation and reconstruction must address the same captured storage.
    // Replaying the authored alias expression here leaves the class on a local
    // descriptor instead of the exact owner selected by a returned reference.
    let written = *semantic.places.get(target_place);
    let segments = semantic.place_segments.span_or_empty(written.segments);
    let Some((facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }, parent)) =
        segments.split_last()
    else {
        return;
    };
    let parent = parent.to_vec();
    let carrier = crate::semantic::places::append_place(semantic, written.root, &parent);
    // The element selector may vary; the retained whole-carrier fact must
    // still name storage independently of mutable selectors.
    if !semantic
        .place_segments
        .span_or_empty(semantic.places.get(carrier).segments)
        .iter()
        .all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        })
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
    // A stored place is in a per-byte class when its captured bounds sit
    // inside it, or when the source place itself proves the class -- a field
    // byte carried across a state edge can keep its class through a join that
    // dropped the differing exact bounds each arm left behind.
    let source_place = contextual_expression_place(
        program,
        semantic,
        machine_symbol,
        state_symbol,
        statement_index,
        source_expression,
    );

    // The carrier's classes are loop invariants: an element store re-proves
    // them under the co-inductive hypothesis that the greatest claim the
    // state's incoming edges could jointly sustain still holds. A weak
    // mid-fixpoint delivery (a bound still unconstrained on one pass) shrinks
    // the stored join below the invariant and the premise channel can never
    // re-derive it, so the hypothesis is the recorded ceiling; the join
    // itself still removes what no current edge can deliver, so a byte or
    // propagation path that genuinely cannot carry a class refutes the
    // candidate by itself.
    let canonical_carrier =
        canonical_place_from_semantic_place(program, semantic, semantic.places.get(carrier));
    let carrier_ceiling: Vec<validation::field_domains::ByteSequencePredicate> = canonical_carrier
        .as_ref()
        .and_then(|place| {
            (place.root == facts::PlaceRoot::Symbol(machine_symbol)).then(|| {
                crate::flow::state_values::field_predicate_ceiling(
                    contexts,
                    state_symbol,
                    place.root,
                    &place.segments,
                )
            })
        })
        .map(<[_]>::to_vec)
        .unwrap_or_default();
    // A read of one element inherits only per-byte classes from its live
    // container. UTF-8 membership alone says nothing about one selected byte.
    let source_element_facts = program
        .expression_table
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
                    .is_some_and(|facts| carrier_proves_predicate(program, facts, predicate, &[]))
                || bounds
                    .is_some_and(|(minimum, maximum)| predicate.holds_for(&[minimum, maximum]))
                || source_place.is_some_and(|place| {
                    let facts = source_facts.get_or_insert_with(|| {
                        facts_about_place(program, semantic, &active_facts, place)
                    });
                    carrier_proves_predicate(program, facts, predicate, &[])
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
    if let Some(carrier_place) = canonical_carrier
        .as_ref()
        .filter(|place| place.root == facts::PlaceRoot::Symbol(machine_symbol))
    {
        // The byte side of this store alone -- no carrier premise -- is what
        // the outgoing edges could deliver for the carrier if the state's
        // predicate candidate holds. `valid_utf8` on the byte side is only
        // `ascii_stored`: whether the carrier held ASCII (and so could not
        // split a scalar) is the premise the candidate answers.
        let byteok: Vec<_> = validation::field_domains::ByteSequencePredicate::ALL
            .into_iter()
            .filter(|predicate| {
                if predicate.is_subslice_preserving() {
                    stored_in(*predicate)
                } else {
                    *predicate == validation::field_domains::ByteSequencePredicate::ValidUtf8
                        && ascii_stored
                }
            })
            .collect();
        if let Some((_, _, joined)) =
            contexts
                .element_store_potentials
                .iter_mut()
                .find(|(state, segments, _)| {
                    *state == state_symbol && *segments == carrier_place.segments
                })
        {
            // Every store into the carrier must keep the class, so the
            // deliverable set is the intersection over the state's stores.
            joined.retain(|predicate| byteok.contains(predicate));
        } else {
            contexts.element_store_potentials.push((
                state_symbol,
                carrier_place.segments.clone(),
                byteok,
            ));
        }
    }
    let mut carrier_facts = None;
    for predicate in validation::field_domains::ByteSequencePredicate::ALL {
        // Both supported per-byte classes are intervals. Their endpoints
        // establish every possible replacement byte, not just one witness.
        // `ValidUtf8` is not subslice preserving in general -- an ASCII byte
        // can split a multi-byte scalar -- but an all-ASCII carrier holds no
        // multi-byte scalars, so an ASCII replacement keeps it decodable.
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
                    &carrier_ceiling,
                )
            }
        } else {
            false
        };
        if !byte_in_class {
            continue;
        }
        let facts = carrier_facts
            .get_or_insert_with(|| facts_about_place(program, semantic, &active_facts, carrier));
        if !carrier_proves_predicate(program, facts, predicate, &carrier_ceiling) {
            continue;
        }
        let fact = semantic.append_transfer_at_point(
            FactPlace::Place(carrier),
            point,
            FactPayload::BytePredicate { predicate },
        );
        semantic.append_ref(references, fact);
    }
}

// Read captured scalar values without replaying mutable operand expressions.
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
    // Compound source syntax carries no selected arithmetic or operator authority.
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
            contexts
                .contexts
                .semantic_context_refs
                .span_or_empty(active)
                .iter()
                .map(|reference| semantic.contexts.get(reference.context)),
            &subject,
        )?
        else {
            return None;
        };
        value.to_i64()
    })?;
    u8::try_from(value).ok()
}

/// Whether the carrier provably satisfies `predicate` BEFORE the store, from an
/// exact literal snapshot, an already-proved per-byte class, or a declared
/// domain whose own predicate implies it.
fn carrier_proves_predicate(
    program: &dyn ::validation::CheckingInput,
    carrier_facts: &[Fact],
    predicate: validation::field_domains::ByteSequencePredicate,
    ceiling: &[validation::field_domains::ByteSequencePredicate],
) -> bool {
    if ceiling.contains(&predicate) {
        return true;
    }
    carrier_facts.iter().any(|fact| match fact.payload {
        FactPayload::AssignedValue { value } => {
            typed_trees::byte_predicates::literal_bytes(program, value)
                .is_some_and(|bytes| predicate.holds_for(&bytes))
        }
        FactPayload::BytePredicate { predicate: proved } => proved.implies(predicate),
        facts::domain_membership_symbol_and_instance_pattern!(domain_symbol, semantic_domain) => {
            index_free_membership(program, domain_symbol, semantic_domain)
                && validation::field_domains::domain_byte_predicate(program, domain_symbol)
                    .is_some_and(|proved| proved.implies(predicate))
        }
        _ => false,
    })
}

/// The active facts about `carrier`'s canonical storage, resolved once per
/// carrier so each per-predicate query reads only them instead of
/// re-canonicalizing every active fact per predicate. State inputs use one
/// attached storage root; authored accesses may still use the current self
/// parameter or an inherited field root.
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
    // Byte-predicate extraction has no substitution environment for domain indices.
    program.domain_definition(symbol).is_some_and(|domain| {
        typed_trees::domain::index_parameters(program, domain).is_empty()
            && (!semantic_domain.is_valid() || semantic_domain == domain.semantic_id)
    })
}
