//! Handling assignments: preserving proven establishment, refusing open
//! windows and the case facts an assignment mentions.
//!
//! A runtime-selected field write can preserve an already valid region without
//! identifying the selected element. It cannot restore an invalid region: two
//! writes with the same wildcard spelling may select different elements. Keep
//! preservation separate from re-establishment, including debt inherited from
//! predecessor states and debt on overlapping concrete places.

use crate::proof_contracts::default_domains::data_reads::range_gates_hold;
use crate::proof_contracts::default_domains::place_queries::{
    data_definition_for_expression, data_has_case_where_facts, declared_root_established,
    expression_root_symbol, fact_span_mentions_field, field_is_where_mentioned, is_data_rooted,
    is_subplace, membership_field, place_spelling_covers, place_spellings_may_overlap,
    write_place_spelling,
};
use crate::proof_contracts::default_domains::state_flow::PlaceValuation;
use crate::proof_contracts::default_domains::symbolic_values::{
    LiteralValue, expression_sequence_measures, expression_symbolic_value, fold, literal_value,
};
use crate::proof_contracts::default_domains::{InvariantWindow, TrackedPlace};
use diagnostics::Diagnostic;
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
            .filter(|place| place.established)
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
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    inherited_windows: &[InvariantWindow],
    poisoned_all: bool,
    poisoned_paths: &mut Vec<String>,
    born_zero: bool,
    statement_index: usize,
) {
    // The binding naming the record this state's places are rooted at; see
    // `place_queries::is_data_rooted`.
    let data_binding = program
        .state_data_binding(machine, state)
        .map(|name| name.as_str().to_owned());
    let data_binding = data_binding.as_deref();
    // Every accepted whole data-value store establishes its exact destination
    // type, whether supplied by construction, a local, or a normal call result.
    // Literal syntax contributes optional snapshots only. The containing
    // receiver below still owes its independent coupled predicates.
    if let Some(spelling) = write_place_spelling(program, target)
        && crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            target,
        )
        .is_none_or(|reference| crate::reference_shell(program, reference).is_none())
        && let Some(definition) =
            data_definition_for_expression(program, machine, Some(state), target)
    {
        if spelling.contains("[*]") {
            // A checked whole-value replacement preserves an established
            // region, but its unknown selector establishes no additional
            // element. Keep every existing establishment/window row: even a
            // valid replacement cannot close debt on a different element.
            // Only exact current-value snapshots beneath the possible write
            // lapse. In particular, missing fields must not become zero again.
            if !poisoned_paths.contains(&spelling) {
                poisoned_paths.push(spelling.clone());
            }
            for place in tracked.iter_mut() {
                if place_spellings_may_overlap(&spelling, &place.spelling) {
                    place.fields.clear();
                    place.established_fields.clear();
                    place.symbols.clear();
                    place.measures.clear();
                    place.elements.clear();
                    place.born_zero = false;
                    place.active_case = None;
                }
            }
        } else {
            let literal = match program.expression_table.expression(value) {
                ExpressionNode::StructLiteral(literal) => Some(literal),
                _ => None,
            };
            let literal_fields = literal
                .map(|literal| program.expression_table.struct_fields(literal.fields))
                .unwrap_or(&[]);
            let fields = literal_fields
                .iter()
                .map(|field| {
                    (
                        field.name.as_str().to_string(),
                        literal_value(program, field.value),
                    )
                })
                .collect();
            let symbols = literal_fields
                .iter()
                .filter_map(|field| {
                    expression_symbolic_value(program, field.value)
                        .map(|symbol| (field.name.as_str().to_string(), symbol))
                })
                .collect();
            let measures = literal_fields
                .iter()
                .filter_map(|field| {
                    expression_sequence_measures(program, field.value).map(|(length, capacity)| {
                        (field.name.as_str().to_string(), length, capacity)
                    })
                })
                .collect();
            tracked.retain(|place| {
                place.spelling != spelling && !is_subplace(&place.spelling, &spelling)
            });
            let place_born_zero = born_zero && is_data_rooted(data_binding, &spelling);
            tracked.push(TrackedPlace {
                spelling,
                definition,
                fields,
                established_fields: Vec::new(),
                symbols,
                measures,
                // Assignment checking proved the exact destination type.
                established: true,
                born_zero: place_born_zero,
                window_open: false,
                elements: Vec::new(),
                element_windows: Vec::new(),
                // Whole replacement transports the same completed case row as
                // initialization. Syntax cannot stand in for an emitted case;
                // unknown or disagreeing normal outcomes retain no case fact.
                active_case: crate::value_custody::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    target,
                )
                .and_then(|reference| {
                    crate::proof_contracts::contract_entailment::completed_value_case_at_occurrence(
                        program,
                        machine,
                        state,
                        statement_index,
                        value,
                        reference,
                    )
                }),
            });
        }
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
        statement_index,
    ) {
        return;
    }

    // A FIELD store: `<self-place>.field = value` where the receiver's type
    // carries where facts.
    let Some(member) = program.expression_table.member_node(target) else {
        return;
    };
    let Some(receiver_spelling) = write_place_spelling(program, member.receiver) else {
        return;
    };
    // A whole-region reseed drops every tracked place strictly nested under
    // the target; a wildcard receiver's write may hit any covered concrete
    // place, so covered siblings lose this field's valuation too.
    let target_spelling = write_place_spelling(program, target);
    if let Some(target_spelling) = target_spelling.as_deref()
        && !target_spelling.contains("[*]")
    {
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
                sibling.established_fields.retain(|symbol| {
                    !program
                        .data_members(sibling.definition)
                        .iter()
                        .any(|member| {
                            member.as_field().is_some_and(|field| {
                                field.symbol == *symbol && field.name.as_str() == field_name
                            })
                        })
                });
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
        state,
        expression_root_symbol(program, member.receiver),
        tracked,
        receiver_spelling,
        definition,
        entry_valuations,
        poisoned_all,
        poisoned_paths,
        born_zero,
        data_binding,
    );
    let region_spelling = tracked[position].spelling.as_str();
    let region_was_established = tracked[position].established
        && !tracked.iter().any(|place| {
            place.any_window_open()
                && (place_spelling_covers(region_spelling, &place.spelling)
                    || place_spelling_covers(&place.spelling, region_spelling))
        })
        && !inherited_windows.iter().any(|(spelling, _, _)| {
            place_spelling_covers(region_spelling, spelling)
                || place_spelling_covers(spelling, region_spelling)
        });
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
    let selected_symbol =
        crate::value_custody::places::effective_member_symbol(program, member.receiver, member);
    let selected_field = crate::value_custody::places::exact_data_member_field(
        program,
        place.definition,
        selected_symbol,
        &field_name,
        member.case_variant.as_ref().map(|case| case.as_str()),
    )
    .filter(|field| {
        let parent = program.symbols.get(field.symbol).parent;
        program.symbols.get(parent).kind != symbols::SymbolKind::Variant
            || place.active_case.is_none_or(|active| active == parent)
    });
    let field_type = selected_field.map(|field| field.type_reference);
    if let Some(field) = selected_field
        && !place.spelling.contains("[*]")
        && !place.established_fields.contains(&field.symbol)
    {
        place.established_fields.push(field.symbol);
    }
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
    // An accepted replacement satisfies its declared field type. It preserves
    // an established owner's implicit containment/qualification gates without
    // inventing literal valuations for untouched siblings. Authored cross-field
    // predicates still require the post-write proof below. A fresh invalid
    // root must establish its gates; scalar writes cannot mint child membership.
    let mut all_hold = region_was_established || range_gates_hold(program, place);
    // Interval-only facts each bound one field, and every store to that field
    // is range-checked against it, so they hold after any accepted store.
    let store_checked = super::data_where_field_intervals(program, place.definition).is_some();
    // Const-only clauses settle at application. A tag-only clause is also
    // preserved by a scalar field store; whole-value replacement owns its
    // tag and complete default-domain construction obligation separately.
    let definition = place.definition;
    // A collection-predicate fact over other fields is untouched by this
    // store; its element windows are tracked separately.
    let written_field = selected_field.map(|field| field.symbol);
    // Conjunction owns independent obligations regardless of their arithmetic
    // spelling. Keep the exact original conjuncts, then let the declaration's
    // read-set owner below identify what this write can invalidate. The closed
    // region check is what preserves an untouched sibling, not recognition as
    // an interval (which misses, for example, the bound `0 - 10 <= delta`).
    // Fresh roots and open windows still owe every conjunct. Disjunctions and
    // other predicates remain whole; no necessary bound replaces their converse.
    let mut pending = if store_checked {
        Vec::new()
    } else {
        program
            .proof_facts
            .span_or_empty(definition.where_facts)
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut declared_facts = Vec::new();
    while let Some(fact) = pending.pop() {
        if let Some(expression) = fact.as_expression()
            && let Some(binary) = program.expression_table.binary_node(*expression)
            && binary.operator == typed_trees::expression::BinaryOperator::And
        {
            pending.extend(
                [binary.right, binary.left].map(typed_trees::domain::ProofFact::Expression),
            );
        } else {
            declared_facts.push(fact);
        }
    }
    let type_facts = declared_facts.iter().filter(|fact| {
        if !region_was_established {
            return true;
        }
        if !super::place_queries::fact_names_field(program, definition, fact, &field_name) {
            return false;
        }
        let Some(expression) = fact.as_expression() else {
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
                match fold(
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
                    None if written_field.is_some_and(|written| {
                        super::post_store_predicates::post_store_predicate_holds(
                            program,
                            machine,
                            state,
                            statement_index,
                            member.receiver,
                            place,
                            written,
                            value,
                            *expression,
                        )
                    }) => {}
                    None => {
                        all_hold = false;
                        // A named runtime value may be written into multiple
                        // correlated fields inside one invariant window. Its
                        // stable symbol lets a later write prove equality; do
                        // not reject before that closing write arrives.
                        // An unknown intermediate value remains unproved;
                        // the next consumption must establish the complete
                        // domain. Later stores may close the same window.
                    }
                }
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let mentioned = membership_field(program, place.definition, membership.value);
                if mentioned.is_some_and(|field| Some(field.symbol) == written_field) {
                    if !crate::proof_contracts::proof_facts::string_literal_grants_domain(
                        program,
                        value,
                        membership.domain_symbol,
                    ) {
                        all_hold = false;
                    }
                } else if mentioned.is_none() {
                    all_hold &= crate::proof_contracts::proof_facts::string_literal_grants_domain(
                        program,
                        membership.value,
                        membership.domain_symbol,
                    );
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
    // consumption points to police (ch11). A wildcard write preserves a
    // previously established, debt-free region when its post-write facts hold,
    // but cannot close an existing region window. Its selected position is not
    // an identity shared with earlier writes, even in a predecessor state.
    if all_hold && (!place.spelling.contains("[*]") || region_was_established) {
        place.established = true;
        place.window_open = false;
    } else {
        // Failed initial establishment leaves reserved storage. Only a
        // previously established value owns restoration debt; reads of an
        // unfinished whole value still fail the establishment gate.
        place.window_open = place.established || place.window_open;
    }
}

/// The tracked place a store's receiver names, created on first write.
pub(super) fn tracked_receiver<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    receiver_root: symbols::SymbolHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    receiver_spelling: String,
    definition: &'program typed_trees::data::DataDefinition,
    entry_valuations: &[PlaceValuation],
    poisoned_all: bool,
    poisoned_paths: &[String],
    born_zero: bool,
    data_binding: Option<&str>,
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
        || poisoned_paths
            .iter()
            .any(|written| place_spellings_may_overlap(&receiver_spelling, written));
    let (seeded_fields, seeded_case) = if poisoned {
        (Vec::new(), None)
    } else {
        entry_valuations
            .iter()
            .find(|(name, _, _)| *name == receiver_spelling)
            .map(|(_, fields, active_case)| (fields.clone(), *active_case))
            .unwrap_or_default()
    };
    let data_rooted = is_data_rooted(data_binding, &receiver_spelling);
    let established = tracked.iter().any(|place| {
        place.established && place_spelling_covers(&place.spelling, &receiver_spelling)
    }) || declared_root_established(program, state, receiver_root)
        || (data_rooted
            && !crate::value_custody::data::data_requires_establishment(program, definition));
    tracked.push(TrackedPlace {
        spelling: receiver_spelling,
        definition,
        fields: seeded_fields,
        established_fields: Vec::new(),
        symbols: Vec::new(),
        measures: Vec::new(),
        // Zero-satisfying data is born established; gated data must
        // earn it (the accepted write below does, since it re-proves
        // the whole domain). A parameter place arrives ALREADY VALID
        // (the caller's net enforced its domain), so it counts as
        // established for the access gate; its VALUATION stays unknown.
        established,
        // Deferred local storage has zero bits even while its domain is
        // unestablished. An already established ancestor or a possible write
        // removes that initial valuation; incoming parameters never acquire it.
        born_zero: !poisoned
            && ((born_zero && data_rooted)
                || (!established
                    && program
                        .state_local_declaration(state, receiver_root)
                        .is_some_and(|(_, local)| !local.initial_value.is_valid()))),
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
    program: &dyn crate::CheckingInput,
    place: &TrackedPlace<'_>,
    field_name: &str,
) -> Vec<arena::HandleSpan<typed_trees::domain::ProofFact>> {
    program
        .data_members(place.definition)
        .iter()
        .filter_map(|member| {
            let variant = member.as_variant()?;
            if variant.where_facts.is_empty()
                || !fact_span_mentions_field(
                    program,
                    place.definition,
                    variant.where_facts,
                    field_name,
                )
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
