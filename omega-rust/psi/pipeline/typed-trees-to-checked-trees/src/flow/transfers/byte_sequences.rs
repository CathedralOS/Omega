//! Concatenation derives predicates from the operands at assignment time.
//! The destination retains a materialized-value fact, not expressions that
//! would read the mutable operands again at a later contract boundary.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_concatenated_predicates(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    expression: ExpressionHandle,
    destination: PlaceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Add
    ) {
        return;
    }
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
    for domain in program.domain_definitions() {
        // Concatenation proves a byte predicate, never a routed qualification.
        if !domain.establishment_routes.is_empty()
            || domain.alias.is_some()
            || !domain.predicate_body.is_present()
            || !crate::field_domain::domain_is_concat_preserving(program, domain.symbol)
            || !value_proves_predicate(
                program,
                semantic,
                &active_facts,
                expression,
                domain.symbol,
                point,
            )
        {
            continue;
        }
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(destination),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: domain.symbol,
            },
        });
        semantic.append_ref(references, fact);
    }
}

fn value_proves_predicate(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    active: &[Fact],
    expression: ExpressionHandle,
    domain: SymbolHandle,
    point: ProgramPoint,
) -> bool {
    if crate::field_domain::string_literal_expression_grants_domain(program, expression, domain) {
        return true;
    }
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == typed_trees::expression::BinaryOperator::Add
    {
        return value_proves_predicate(program, semantic, active, binary.left, domain, point)
            && value_proves_predicate(program, semantic, active, binary.right, domain, point);
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
        let FactPlace::Place(source) = fact.place else {
            return false;
        };
        if !semantic.places_equal(source, place) {
            return false;
        }
        match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                // The materialized bytes retain the same primitive predicate
                // across bounded carriers. This is not carrier/domain identity.
                crate::field_domain::domain_byte_predicate(program, domain_symbol)
                    == crate::field_domain::domain_byte_predicate(program, domain)
            }
            FactPayload::AssignedValue { value } => {
                crate::field_domain::string_literal_expression_grants_domain(program, value, domain)
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
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: ExpressionHandle,
    source_expression: ExpressionHandle,
    scalar_value: Option<&facts::ScalarValue>,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(target) else {
        return;
    };
    let collection = indexed.collection;
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
    let Some(carrier) = contextual_expression_place(
        program,
        semantic,
        machine_symbol,
        state_symbol,
        statement_index,
        collection,
    ) else {
        return;
    };
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
    let Some(byte) = byte.or_else(|| {
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
    }) else {
        return;
    };

    for predicate in crate::field_domain::ByteSequencePredicate::ALL
        .into_iter()
        .filter(|predicate| predicate.is_subslice_preserving())
    {
        if !predicate.holds_for(&[byte])
            || !carrier_proves_predicate(program, semantic, &active_facts, carrier, predicate)
        {
            continue;
        }
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(carrier),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::BytePredicate { predicate },
        });
        semantic.append_ref(references, fact);
    }
}

// Read captured scalar values without replaying mutable operand expressions.
#[allow(clippy::too_many_arguments)]
fn replacement_byte(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    active: &[Fact],
    carrier: PlaceHandle,
    predicate: crate::field_domain::ByteSequencePredicate,
) -> bool {
    active.iter().any(|fact| {
        let FactPlace::Place(place) = fact.place else {
            return false;
        };
        if !semantic.places_equal(place, carrier) {
            return false;
        }
        match fact.payload {
            FactPayload::AssignedValue { value } => matches!(
                program.expression_table.expression(value),
                ExpressionNode::String(literal) if predicate.holds_for(literal)
            ),
            FactPayload::BytePredicate { predicate: proved } => proved.implies(predicate),
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                crate::field_domain::domain_byte_predicate(program, domain_symbol)
                    .is_some_and(|proved| proved.implies(predicate))
            }
            _ => false,
        }
    })
}
