//! Handling assignments: preserving proven establishment, refusing open
//! windows and the case facts an assignment mentions.

use crate::proof_contracts::default_domains::data_reads::range_gates_hold;
use crate::proof_contracts::default_domains::place_queries::{
    data_definition_for_expression, data_has_case_where_facts, domain_definition_by_name,
    fact_mentions_any_field, fact_span_mentions_field, field_is_where_mentioned, is_self_rooted,
    is_subplace, membership_field_name, place_spelling_covers, write_place_spelling,
};
use crate::proof_contracts::default_domains::state_flow::PlaceValuation;
use crate::proof_contracts::default_domains::symbolic_values::{
    LiteralValue, expression_sequence_measures, expression_symbol, expression_symbolic_value,
    fold_with_valuation, literal_value, mentions_float,
};
use crate::proof_contracts::default_domains::{InvariantWindow, TrackedPlace};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// A call invalidates exact field valuations through aliasing, but it cannot
/// invalidate establishment: every accepted write in every checked callee must
/// leave the default domain true. Preserve that monotone fact before clearing
/// the more precise tracked valuation.
pub(crate) fn preserve_proven_establishment(
    tracked: &[TrackedPlace<'_>],
    established: &mut Vec<String>,
) {
    established.extend(
        tracked
            .iter()
            .filter(|place| place.established && is_self_rooted(&place.spelling))
            .map(|place| place.spelling.clone()),
    );
}

/// Ch11 (slice 8): refuse every open invariant window at a consumption
/// point, naming the place and the point -- both this state's own open
/// windows and the ones transported from predecessor states.
pub(crate) fn refuse_open_windows(
    tracked: &[TrackedPlace<'_>],
    inherited_windows: &[InvariantWindow],
    consumption_point: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for place in tracked.iter().filter(|place| place.any_window_open()) {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}`'s default domain is still FALSE at {consumption_point}: the \
             invariant window opened on `{}` must close first -- restore the \
             `where` facts before this consumption point (ch11)",
            place.definition.name.as_str(),
            place.spelling
        )));
    }
    for (spelling, data_name, _) in inherited_windows {
        if tracked.iter().any(|place| place.spelling == *spelling) {
            // The tracked entry already reported (open) or closed it.
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "data `{data_name}`'s default domain is still FALSE at {consumption_point}: \
             the invariant window opened on `{spelling}` in a predecessor state must \
             close first -- restore the `where` facts before this consumption point \
             (ch11 window transport)"
        )));
    }
}

pub(crate) fn handle_assignment<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    inherited_windows: &[InvariantWindow],
    poisoned_all: bool,
    poisoned_paths: &[String],
    born_zero: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A whole-place store of a struct literal reseeds the valuation (the
    // literal itself was proven at construction, rung 2b). A wildcard
    // position (`self.maps[i] = ...`) is different: the literal proves only
    // the one element the runtime index hits, so the wildcard place cannot
    // be marked established -- its window opens as ch11's conservative
    // ceiling on unrepresentable origins.
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(value)
        && let Some(spelling) = write_place_spelling(program, target)
        && let Some(definition) = domain_definition_by_name(program, literal.type_name.as_str())
    {
        let wildcard = spelling.contains("[*]");
        let fields = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .map(|field| {
                (
                    field.name.as_str().to_string(),
                    literal_value(program, field.value),
                )
            })
            .collect();
        let symbols = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter_map(|field| {
                expression_symbolic_value(program, field.value)
                    .map(|symbol| (field.name.as_str().to_string(), symbol))
            })
            .collect();
        let measures = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter_map(|field| {
                expression_sequence_measures(program, field.value)
                    .map(|(length, capacity)| (field.name.as_str().to_string(), length, capacity))
            })
            .collect();
        tracked
            .retain(|place| place.spelling != spelling && !is_subplace(&place.spelling, &spelling));
        let place_born_zero = born_zero && is_self_rooted(&spelling);
        tracked.push(TrackedPlace {
            spelling,
            definition,
            fields: if wildcard { Vec::new() } else { fields },
            symbols: if wildcard { Vec::new() } else { symbols },
            measures: if wildcard { Vec::new() } else { measures },
            // Rung 2b proved this literal against the domain -- for the one
            // representable position a literal spelling names.
            established: !wildcard,
            born_zero: place_born_zero,
            window_open: wildcard,
            elements: Vec::new(),
            element_windows: Vec::new(),
            // The literal's selected case replaces the place's active case:
            // the previous case's facts lapse with its payload, and the new
            // case's facts were already proven at construction. A wildcard
            // position cannot claim a case for the whole covered region.
            active_case: if wildcard { None } else { literal.case_symbol },
        });
        return;
    }

    // An ELEMENT store: `<place>.field[index] = value`. Facts reading the
    // field's contents owe re-proof; nothing else changes.
    if super::element_stores::handle_element_store(
        program,
        machine,
        state,
        target,
        value,
        tracked,
        entry_valuations,
        inherited_windows,
        poisoned_all,
        poisoned_paths,
        born_zero,
    ) {
        return;
    }

    // A FIELD store: `<self-place>.field = value` where the receiver's type
    // carries where facts.
    let ExpressionNode::Member(member) = program.expression_table.expression(target) else {
        return;
    };
    let Some(receiver_spelling) = write_place_spelling(program, member.receiver) else {
        return;
    };
    // A whole-region reseed drops every tracked place strictly nested under
    // the target; a wildcard receiver's write may hit any covered concrete
    // place, so covered siblings lose this field's valuation too.
    let target_spelling = write_place_spelling(program, target);
    if let Some(target_spelling) = target_spelling.as_deref() {
        tracked.retain(|place| !is_subplace(&place.spelling, target_spelling));
    }
    let Some(definition) =
        data_definition_for_expression(program, machine, Some(state), member.receiver)
    else {
        return;
    };
    if definition.where_facts.is_empty()
        && !data_has_case_where_facts(program, definition)
        && !crate::value_custody::data::data_requires_establishment(program, definition)
    {
        return;
    }
    let field_name = member.member.as_str().to_string();
    let written = literal_value(program, value);

    // A wildcard receiver (`self.maps[*]`) may hit any covered concrete
    // element: every tracked sibling under it loses this field's known
    // valuation before the write proceeds. The siblings' own window state is
    // untouched -- the write can neither prove nor disprove what it did not
    // positionally name.
    if receiver_spelling.contains("[*]") {
        for sibling in tracked.iter_mut() {
            if place_spelling_covers(&receiver_spelling, &sibling.spelling) {
                sibling.fields.retain(|(name, _)| *name != field_name);
                // Missing born-zero fields are known zero; an overlapping
                // unknown write is instead an explicit unknown snapshot.
                sibling.fields.push((field_name.clone(), None));
                sibling.symbols.retain(|(name, _)| *name != field_name);
                sibling.measures.retain(|(name, _, _)| *name != field_name);
            }
        }
    }

    let position = tracked_receiver(
        program,
        tracked,
        receiver_spelling,
        definition,
        entry_valuations,
        poisoned_all,
        poisoned_paths,
        born_zero,
    );
    let place = &mut tracked[position];
    place.fields.retain(|(name, _)| *name != field_name);
    place.fields.push((field_name.clone(), written));
    place.symbols.retain(|(name, _)| *name != field_name);
    if let Some(symbol) = expression_symbolic_value(program, value) {
        place.symbols.push((field_name.clone(), symbol));
    }
    place.measures.retain(|(name, _, _)| *name != field_name);
    if let Some((length, capacity)) = expression_sequence_measures(program, value) {
        place.measures.push((field_name.clone(), length, capacity));
    }

    // Obligation: a field participating in either an authored `where` fact or
    // an implicit range/containment gate must help re-establish the whole
    // value. Unrelated writes preserve the current establishment state.
    let field_type =
        program
            .data_members(place.definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == field_name =>
                {
                    Some(field.type_reference)
                }
                _ => None,
            });
    // CASE-CONSTRAINTS (ch12): the facts a field write must re-prove include
    // the ACTIVE case's `where` facts. A known active case contributes its
    // own fact set only when the write touches a fact-mentioned name; an
    // unknown active case joins the facts of every case whose fact set
    // mentions the field (a payload write is only meaningful under a case
    // that owns the name, so demanding each candidate's facts is the
    // conservative meet).
    let case_fact_spans = case_fact_spans_mentioning(program, place, &field_name);
    if !field_is_where_mentioned(program, place.definition, &field_name)
        && case_fact_spans.is_empty()
        && !field_type.is_some_and(|field_type| {
            crate::value_custody::data::type_requires_establishment(program, field_type)
        })
    {
        return;
    }
    let valuation: Vec<(&str, Option<LiteralValue>)> = place
        .fields
        .iter()
        .map(|(name, value)| (name.as_str(), *value))
        .collect();
    let mut all_hold = range_gates_hold(program, place);
    // Interval-only facts each bound one field, and every store to that field
    // is range-checked against it, so they hold after any accepted store.
    let store_checked = super::data_where_field_intervals(program, place.definition).is_some();
    // A TYPE-WIDE fact naming no field constrains the declaration's CONST
    // PARAMETERS alone, and the specialization decided it once:
    // `FixedBuffer<const N: u64> where N > 0 && N <= 8` is settled when 4 is
    // substituted, not re-proved at every write to `used`. The valuation
    // below carries field values, so folding such a fact can only answer
    // "unknown" -- which would open an invariant window no later store could
    // close. A CASE fact is not filtered this way: its names are the
    // variant's payload bindings, which are not members of the data.
    let definition = place.definition;
    // A collection-predicate fact over other fields is untouched by this
    // store; its element windows are tracked separately.
    let written_field = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                Some(field.symbol)
            }
            _ => None,
        });
    // When every fact bounds one field by a literal, a store can change only
    // the written field's own conjuncts. A place that was established with no
    // open window still satisfies every other field's bounds, so folding them
    // against a valuation that may not know those fields would only invent
    // unknowns; the written field's conjuncts are re-proved below.
    let written_field_conjuncts = written_field
        .filter(|_| !store_checked && place.established && !place.any_window_open())
        .and_then(|written| {
            Some(
                super::data_where_field_literal_bounds(program, definition)?
                    .into_iter()
                    .filter(|bound| bound.field == written)
                    .map(|bound| typed_trees::domain::ProofFact::Expression(bound.conjunct))
                    .collect::<Vec<_>>(),
            )
        });
    let declared_facts = match &written_field_conjuncts {
        Some(conjuncts) => conjuncts.as_slice(),
        None if store_checked => &[],
        None => program.proof_facts.span_or_empty(definition.where_facts),
    };
    let type_facts = declared_facts
        .iter()
        .filter(|fact| fact_mentions_any_field(program, definition, fact))
        .filter(|fact| {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                return true;
            };
            written_field.is_none_or(|written| {
                crate::proof_contracts::collection_predicates::data_fact_applies_to_field(
                    program,
                    definition,
                    *expression,
                    written,
                ) != Some(false)
            })
        });
    let case_facts = case_fact_spans
        .iter()
        .copied()
        .flat_map(|span| program.proof_facts.span_or_empty(span));
    for fact in type_facts.chain(case_facts) {
        match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                match fold_with_valuation(
                    program,
                    place.definition,
                    &valuation,
                    &place.symbols,
                    &place.measures,
                    place.born_zero,
                    *expression,
                ) {
                    Some(value) if value != 0 => {}
                    // Ch11 (slice 8): a checkable violation OPENS a window instead
                    // of refusing -- the consumption points demand closure.
                    Some(_) => all_hold = false,
                    None => {
                        all_hold = false;
                        // A named runtime value may be written into multiple
                        // correlated fields inside one invariant window. Its
                        // stable symbol lets a later write prove equality; do
                        // not reject before that closing write arrives.
                        // A float computation need not be folded to permit
                        // a temporary write: it poisons the exact field until
                        // a later store proves the predicate again. Reads and
                        // all other consumption points still demand closure.
                        if expression_symbol(program, value).is_none()
                            && !mentions_float(program, place.definition, *expression)
                        {
                            diagnostics.push(Diagnostic::error(format!(
                                "write to `{}.{field_name}` cannot PROVE data `{}`'s default domain: \
                                 a `where`-mentioned field's value is not a literal known here (a \
                                 runtime value, or a co-field last written in another state) -- \
                                 restructure with literal stores in one state for now (the \
                                 entailment integration and cross-state valuation transport relax \
                                 this)",
                                place.spelling,
                                place.definition.name.as_str()
                            )));
                        }
                    }
                }
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let mentioned = membership_field_name(program, membership.value);
                if mentioned == Some(field_name.as_str()) {
                    if !crate::proof_contracts::proof_facts::string_literal_grants_domain(
                        program,
                        value,
                        membership.domain_symbol,
                    ) {
                        all_hold = false;
                    }
                } else if !place.established || place.any_window_open() {
                    // A write to another field preserves a previously true
                    // membership, but cannot manufacture a missing one.
                    all_hold = false;
                }
            }
            typed_trees::domain::ProofFact::Proposition(_) => {
                // Proposition entailment is proof-layer work. The default-domain
                // interval checker must not pretend a proposition is Boolean.
                all_hold = false;
            }
        }
    }
    // Every fact re-proven at the post-write valuation: the place
    // satisfies its domain again (any open window CLOSES; a gated place
    // establishes). A checkable violation leaves the window OPEN for the
    // consumption points to police (ch11). A wildcard place never closes
    // here: one dynamic position's re-proof cannot discharge the covered
    // region -- only a whole-place reseed drops it.
    if all_hold && !place.spelling.contains("[*]") {
        place.established = true;
        place.window_open = false;
    } else {
        place.window_open = true;
    }
}

/// The tracked place a store's receiver names, created on first write.
pub(super) fn tracked_receiver<'program>(
    program: &'program TypedTrees,
    tracked: &mut Vec<TrackedPlace<'program>>,
    receiver_spelling: String,
    definition: &'program typed_trees::data::DataDefinition,
    entry_valuations: &[PlaceValuation],
    poisoned_all: bool,
    poisoned_paths: &[String],
    born_zero: bool,
) -> usize {
    if let Some(position) = tracked
        .iter()
        .position(|place| place.spelling == receiver_spelling)
    {
        return position;
    }
    // R2 rung 3 slice 5: seed the fresh place from its transported entry
    // valuation unless an opaque call, or a known overlapping write,
    // poisoned this place's view.
    let poisoned = poisoned_all
        || poisoned_paths.iter().any(|written| {
            crate::machine_calls::calls::frame_paths_overlap(&receiver_spelling, written)
        });
    let (seeded_fields, seeded_case) = if poisoned {
        (Vec::new(), None)
    } else {
        entry_valuations
            .iter()
            .find(|(name, _, _)| *name == receiver_spelling)
            .map(|(_, fields, active_case)| (fields.clone(), *active_case))
            .unwrap_or_default()
    };
    let self_rooted = is_self_rooted(&receiver_spelling);
    tracked.push(TrackedPlace {
        spelling: receiver_spelling,
        definition,
        fields: seeded_fields,
        symbols: Vec::new(),
        measures: Vec::new(),
        // Zero-satisfying data is born established; gated data must
        // earn it (the accepted write below does, since it re-proves
        // the whole domain). A parameter place arrives ALREADY VALID
        // (the caller's net enforced its domain), so it counts as
        // established for the access gate; its VALUATION stays unknown.
        established: !crate::value_custody::data::data_requires_establishment(program, definition)
            || !self_rooted,
        born_zero: born_zero && self_rooted && !poisoned,
        window_open: false,
        elements: Vec::new(),
        element_windows: Vec::new(),
        active_case: seeded_case,
    });
    tracked.len() - 1
}

/// CASE-CONSTRAINTS (ch12): the case-local `where` fact spans a field write
/// must re-prove. When the place's active case is known, only that case's
/// facts apply -- and only when the written field is one the facts name
/// (an unrelated write preserves the current window state, exactly as an
/// unrelated common-field write does). When the active case is unknown, the
/// write joins every case whose fact set names the field: a payload write is
/// only meaningful under a case owning that binding, so requiring each
/// candidate's facts is the conservative meet. A known case without facts
/// (or whose facts ignore the field) contributes nothing -- an omitted case
/// clause contributes true.
fn case_fact_spans_mentioning(
    program: &TypedTrees,
    place: &TrackedPlace<'_>,
    field_name: &str,
) -> Vec<arena::HandleSpan<typed_trees::domain::ProofFact>> {
    program
        .data_members(place.definition)
        .iter()
        .filter_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            if variant.where_facts.is_empty()
                || !fact_span_mentions_field(program, variant.where_facts, field_name)
                || place
                    .active_case
                    .is_some_and(|active| active != variant.symbol)
            {
                return None;
            }
            Some(variant.where_facts)
        })
        .collect()
}
