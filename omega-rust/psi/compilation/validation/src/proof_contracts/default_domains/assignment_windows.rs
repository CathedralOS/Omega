//! Track which record requirements an assignment must restore.
//!
//! Two writes through a computed array index may select different elements.
//! A valid replacement preserves an already valid array, but cannot prove that
//! an earlier invalid element was restored. Requirements left unfinished by an
//! earlier state or an overlapping write still need proof.

use crate::proof_contracts::default_domains::data_reads::range_gates_hold;
use crate::proof_contracts::default_domains::place_queries::{
    data_definition_for_expression, data_has_case_where_facts, declared_root_established,
    is_data_rooted, is_subplace, membership_field, place_spelling_covers,
    place_spellings_may_overlap, write_place_spelling,
};
use crate::proof_contracts::default_domains::state_flow::PlaceValuation;
use crate::proof_contracts::default_domains::symbolic_values::{
    LiteralValue, expression_sequence_measures, expression_symbolic_value, fold, literal_value,
};
use crate::proof_contracts::default_domains::{DomainRequirement, InvariantWindow, TrackedPlace};
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// A call may change field contents, but a checked callee must restore the
/// record's requirements before returning. Retain that validity after checking
/// open windows, then discard any precise values the call may change.
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

/// Report a record that still needs restoration when a call or return would
/// expose it. Point at the write that opened the window when it is available.
pub(crate) fn refuse_open_windows(
    tracked: &[TrackedPlace<'_>],
    inherited_windows: &[InvariantWindow],
    machine: &Machine,
    state: &State,
    statement_index: Option<usize>,
    consumption_point: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !tracked.iter().any(TrackedPlace::any_window_open) && inherited_windows.is_empty() {
        return;
    }
    let point = match statement_index {
        Some(index) => format!(
            "{consumption_point} at statement {} in machine `{}` state `{}`",
            index + 1,
            machine.name,
            state.name
        ),
        None => format!(
            "{consumption_point} from machine `{}` state `{}`",
            machine.name, state.name
        ),
    };
    for place in tracked.iter().filter(|place| place.any_window_open()) {
        let mut diagnostic = Diagnostic::error(format!(
            "cannot complete {point}: this write left the `requires` facts of `{}` (data `{}`) unproved; restore them first",
            place.spelling, place.definition.name,
        ));
        if let Some(opening) = place.opening_write {
            diagnostic = diagnostic.with_source_span(opening);
        }
        diagnostics.push(diagnostic);
    }
    for (spelling, data_name, _) in inherited_windows {
        if tracked.iter().any(|place| place.spelling == *spelling) {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "cannot complete {point}: an earlier state left the `requires` facts of `{spelling}` (data `{data_name}`) unproved; restore them first"
        )));
    }
}

pub(crate) fn handle_assignment<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    inherited_windows: &[InvariantWindow],
    poisoned_all: bool,
    poisoned_paths: &mut Vec<String>,
    born_zero: bool,
    statement_index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(assignment) = statement.as_assignment() else {
        return;
    };
    let resident_requirement =
        crate::atomic_assignment_carrier(program, assignment).and_then(|_| {
            super::where_fact_intervals::stored_field_where_interval(
                program,
                machine,
                state,
                assignment.target,
            )
        });
    let target = assignment.target;
    let value = assignment.value;
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
                established_fields: super::established_common_fields(program, definition),
                pending_requirements: Vec::new(),
                symbols,
                measures,
                // Assignment checking proved the exact destination type.
                established: true,
                born_zero: place_born_zero,
                window_open: false,
                opening_write: None,
                elements: Vec::new(),
                element_windows: Vec::new(),
                // Whole replacement transports the same completed case row as
                // initialization. Syntax cannot stand in for an emitted case;
                // unknown or disagreeing normal outcomes retain no case fact.
                alternatives: Vec::new(),
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
    let element_store = super::element_stores::handle_element_store(
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
    );

    // Direct names, stored aliases and representation views all publish their
    // performed destinations through the frame owner. No source-shape path may
    // independently decide which parent's predicate a store must restore.
    let Some(frames) = call_frames else {
        return;
    }; // Symbol diagnostics already prevent admission.
    let origins = match frames.assignment_storage_origins(machine, statement) {
        Ok(origins) => origins,
        Err(reason) => {
            diagnostics.push(
                Diagnostic::error(format!(
                    "cannot establish assignment invariants: performed write has unknown destination origins in machine `{}` state `{}` at statement {statement_index}: {reason}",
                    machine.name.as_str(), state.name.as_str(),
                ))
                .with_source_span(program.expression_table.source_span(target)),
            );
            return;
        }
    };
    if crate::proof_contracts::contract_entailment::struct_trace() {
        eprintln!(
            "DEFAULT assignment-origins machine={} destinations={:?}",
            machine.name, origins.destinations,
        );
    }
    let definite = origins.destinations.len() == 1;
    for origin in origins.destinations {
        let coordinate = match origin.storage_coordinate(program, state) {
            Ok(coordinate) => coordinate,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot establish assignment invariants: source storage interpretation is unavailable: {reason} (machine `{}` state `{}` statement {statement_index}, source `{}`, path {:?}, view {:?})",
                    machine.name, state.name, program.symbols.name(origin.root), origin.segments,
                    origin.representation.as_ref().map(|(reference, path)|
                        (program.display_type_reference(*reference), path)),
                )));
                continue;
            }
        };
        let destination = coordinate.place;
        let preserves_scalar_value = definite && coordinate.preserves_scalar_value;
        let stores = match field_stores(
            program,
            state,
            statement_index,
            destination,
            preserves_scalar_value,
        ) {
            Ok(stores) => stores,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot establish assignment invariants: {reason}"
                )));
                continue;
            }
        };
        // Innermost owners settle first. Every ancestor still owns its own
        // predicates; a leaf value is never a replacement for its aggregate field.
        for store in stores {
            // The element ledger owns only the field it actually analyzed.
            // A lexical alias/recast cannot suppress the lender's independent
            // predicate merely because that lender is the nearest owner.
            if element_store.as_ref() == Some(&store.field_place) {
                continue;
            }
            handle_field_store(
                program,
                machine,
                state,
                store,
                value,
                tracked,
                entry_valuations,
                inherited_windows,
                poisoned_all,
                poisoned_paths,
                born_zero,
                statement_index,
                data_binding,
                resident_requirement,
            );
        }
    }
}

struct FieldStore<'program> {
    destination: crate::CanonicalPlace,
    receiver: crate::CanonicalPlace,
    field_place: crate::CanonicalPlace,
    receiver_type: typed_trees::types::TypeReferenceHandle,
    definition: &'program typed_trees::data::DataDefinition,
    field: &'program typed_trees::data::DataField,
    preserves_scalar_value: bool,
}

/// The performed destination supplies the complete enclosing-owner roster.
/// Each field along that coordinate may carry an independent parent predicate.
fn field_stores<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &State,
    statement_index: usize,
    destination: crate::CanonicalPlace,
    preserves_scalar_value: bool,
) -> Result<Vec<FieldStore<'program>>, &'static str> {
    let mut stores = Vec::new();
    for index in (0..destination.segments.len()).rev() {
        let facts::PlaceSegment::Field { symbol } = destination.segments[index] else {
            continue;
        };
        if !symbol.is_valid() {
            return Err("source field identity is unavailable");
        }
        let mut receiver = destination.clone();
        receiver.segments.truncate(index);
        if matches!(
            receiver.segments.last(),
            Some(facts::PlaceSegment::Case { .. })
        ) {
            receiver.segments.pop();
        }
        let receiver_type = crate::value_custody::places::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &receiver,
        )
        .ok_or("source parent type is unavailable")?;
        let definition =
            crate::value_custody::places::data_definition_for_type(program, receiver_type)
                .ok_or("source parent declaration is unavailable")?;
        let parent = program.symbols.get(symbol).parent;
        let case_name = (program.symbols.get(parent).kind == symbols::SymbolKind::Variant)
            .then(|| program.symbols.name(parent));
        let field = crate::value_custody::places::exact_data_member_field(
            program,
            definition,
            symbol,
            program.symbols.name(symbol),
            case_name,
        )
        .ok_or("source field does not belong to its parent declaration")?;
        let mut field_place = destination.clone();
        field_place.segments.truncate(index + 1);
        stores.push(FieldStore {
            destination: destination.clone(),
            receiver,
            receiver_type,
            field_place,
            definition,
            field,
            preserves_scalar_value,
        });
    }
    Ok(stores)
}

/// Labels adapt already resolved storage to the legacy valuation table. A
/// runtime selector stays a wildcard; it never becomes a persistent identity.
fn invariant_place_spelling(
    program: &dyn crate::CheckingInput,
    place: &crate::CanonicalPlace,
) -> Option<String> {
    use std::fmt::Write;
    let root = place.root.as_symbol()?;
    let mut spelling = program.symbols.name(root).to_owned();
    for segment in &place.segments {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                spelling.push('.');
                spelling.push_str(program.symbols.name(*symbol));
            }
            facts::PlaceSegment::Case { .. } => {}
            facts::PlaceSegment::FixedIndex { index } => {
                write!(&mut spelling, "[{index}]").ok()?;
            }
            facts::PlaceSegment::Index { .. } => spelling.push_str("[*]"),
            _ => return None,
        }
    }
    Some(spelling)
}

#[allow(clippy::too_many_arguments)]
fn handle_field_store<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    store: FieldStore<'program>,
    value: ExpressionHandle,
    tracked: &mut Vec<TrackedPlace<'program>>,
    entry_valuations: &[PlaceValuation],
    inherited_windows: &[InvariantWindow],
    poisoned_all: bool,
    poisoned_paths: &[String],
    born_zero: bool,
    statement_index: usize,
    data_binding: Option<&str>,
    resident_requirement: Option<crate::proof_contracts::arithmetic_domains::Interval>,
) {
    // Replacing this field retires tracked descendants of its actual backing,
    // independently of the alias that named the performed store.
    if let Some(spelling) = invariant_place_spelling(program, &store.destination)
        && !spelling.contains("[*]")
    {
        tracked.retain(|place| !is_subplace(&place.spelling, &spelling));
    }
    let definition = store.definition;
    let Some(receiver_spelling) = invariant_place_spelling(program, &store.receiver) else {
        return;
    };
    if definition.where_facts.is_empty()
        && !data_has_case_where_facts(program, definition)
        && !crate::value_custody::data::data_requires_establishment(program, definition)
    {
        return;
    }
    let whole_field = store.destination == store.field_place;
    let field_name = store.field.name.to_string();
    let written = (store.preserves_scalar_value && whole_field)
        .then(|| literal_value(program, value))
        .flatten();

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
        store
            .receiver
            .root
            .as_symbol()
            .unwrap_or_else(symbols::SymbolHandle::invalid),
        tracked,
        receiver_spelling,
        definition,
        entry_valuations,
        inherited_windows,
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
    if let Some(symbol) = (store.preserves_scalar_value && whole_field)
        .then(|| expression_symbolic_value(program, value))
        .flatten()
    {
        place.symbols.push((field_name.clone(), symbol));
    }
    place.measures.retain(|(name, _, _)| *name != field_name);
    if let Some((length, capacity)) = (store.preserves_scalar_value && whole_field)
        .then(|| expression_sequence_measures(program, value))
        .flatten()
    {
        place.measures.push((field_name.clone(), length, capacity));
    }
    if !place.alternatives.is_empty() {
        let written = (store.preserves_scalar_value && whole_field)
            .then(|| literal_value(program, value))
            .flatten();
        let definition = place.definition;
        let case = place.active_case;
        let holds = place.alternatives.iter().all(|fields| {
            let mut fields = fields.clone();
            if let Some(slot) = fields.iter_mut().find(|(name, _)| name == &field_name) {
                slot.1 = written;
            } else {
                fields.push((field_name.clone(), written));
            }
            super::symbolic_values::predicate_holds(program, definition, &fields, case)
        });
        if holds {
            for fields in &mut place.alternatives {
                if let Some(slot) = fields.iter_mut().find(|(name, _)| name == &field_name) {
                    slot.1 = written;
                } else {
                    fields.push((field_name.clone(), written));
                }
            }
            place.established = true;
            place.window_open = false;
            place.pending_requirements.clear();
            if !place.any_window_open() {
                place.opening_write = None;
            }
            return;
        }
        // A symbolic or representation-changing store may lack a literal
        // valuation. The complete prefix proof below still gets to establish
        // the actual post-store predicate across every incoming alternative.
        place.window_open = place.established || place.window_open;
    }

    // Obligation: a field participating in either an authored `where` fact or
    // an implicit range/containment gate must help re-establish the whole
    // value. Unrelated writes preserve the current establishment state.
    let selected_field = Some(store.field).filter(|field| {
        let parent = program.symbols.get(field.symbol).parent;
        program.symbols.get(parent).kind != symbols::SymbolKind::Variant
            || place.active_case.is_none_or(|active| active == parent)
    });
    let field_type = selected_field.map(|field| field.type_reference);
    if let Some(field) = selected_field
        && whole_field
        && !place.spelling.contains("[*]")
        && !place.established_fields.contains(&field.symbol)
    {
        place.established_fields.push(field.symbol);
    }
    // The atomic checker already proved that the replacement preserves the
    // resident bound. Reuse that proof only for the same field, numeric
    // interpretation and bound. Plain stores update the requirements below.
    let resident_field = resident_requirement
        .filter(|checked| {
            whole_field
                && store.preserves_scalar_value
                && super::where_fact_intervals::atomic_field_interval(
                    program,
                    place.definition,
                    store.field,
                ) == Some(*checked)
        })
        .map(|_| store.field.symbol);
    let requirements = domain_requirements(program, place.definition, place.active_case);
    if crate::proof_contracts::contract_entailment::struct_trace() {
        eprintln!(
            "DEFAULT owner-store machine={} data={} receiver={:?} destination={:?} field={:?} requirements={}",
            machine.name,
            place.definition.name,
            store.receiver,
            store.destination,
            store.field.symbol,
            requirements.len(),
        );
    }
    if !requirements.iter().any(|requirement| {
        super::place_queries::fact_names_field(
            program,
            place.definition,
            &requirement.fact,
            &field_name,
        )
    }) && !field_type.is_some_and(|field_type| {
        crate::value_custody::data::type_requires_establishment(program, field_type)
    }) {
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
    let definition = place.definition;
    let written_field = selected_field.map(|field| field.symbol);
    // A case change replaces the value and its obligations. A known current
    // tag also makes the other cases' conditional requirements inapplicable.
    place.pending_requirements.retain(|requirement| {
        !requirement.case.is_valid()
            || place
                .active_case
                .is_none_or(|active| active == requirement.case)
    });
    let requirements = requirements
        .into_iter()
        .filter(|requirement| {
            if !place.established || place.pending_requirements.contains(requirement) {
                return true;
            }
            if !super::place_queries::fact_names_field(
                program,
                definition,
                &requirement.fact,
                &field_name,
            ) {
                return false;
            }
            let disjoint = store
                .destination
                .segments
                .strip_prefix(store.receiver.segments.as_slice())
                .is_some_and(|path| {
                    super::place_queries::fact_is_disjoint_from_store(
                        program,
                        definition,
                        &requirement.fact,
                        path,
                    )
                });
            if crate::proof_contracts::contract_entailment::struct_trace() {
                eprintln!(
                    "DEFAULT owner-requirement machine={} data={} fact={:?} disjoint={disjoint}",
                    machine.name, definition.name, requirement.fact,
                );
            }
            if disjoint {
                return false;
            }
            let Some(expression) = requirement.fact.as_expression() else {
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
        })
        .collect::<Vec<_>>();
    for requirement in requirements {
        let fact = &requirement.fact;
        let resident_proven = !requirement.case.is_valid()
            && resident_field.is_some_and(|field| {
                super::place_queries::data_predicate_fields(program, place.definition, fact)
                    == [field]
            });
        let proven = resident_proven
            || match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    match fold(
                        program,
                        place.definition,
                        &valuation,
                        &place.symbols,
                        &place.measures,
                        place.born_zero,
                        place.active_case,
                        *expression,
                    ) {
                        Some(value) => value != 0,
                        None => written_field.is_some_and(|written| {
                            super::post_store_predicates::post_store_predicate_holds(
                                program,
                                machine,
                                state,
                                statement_index,
                                &store.receiver,
                                store.receiver_type,
                                store.preserves_scalar_value,
                                &store.destination,
                                place,
                                written,
                                value,
                                *expression,
                            )
                        }),
                    }
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    let mentioned = membership_field(program, place.definition, membership.value);
                    if mentioned.is_some_and(|field| Some(field.symbol) == written_field) {
                        crate::proof_contracts::proof_facts::string_literal_grants_domain(
                            program,
                            value,
                            membership.domain_symbol,
                        )
                    } else if mentioned.is_none() {
                        crate::proof_contracts::proof_facts::string_literal_grants_domain(
                            program,
                            membership.value,
                            membership.domain_symbol,
                        )
                    } else {
                        place.established && !place.any_window_open()
                    }
                }
                // An unproved proposition remains owed; it is not a Boolean value.
                typed_trees::domain::ProofFact::Proposition(_) => false,
            };
        if crate::proof_contracts::contract_entailment::struct_trace() {
            eprintln!(
                "DEFAULT owner-proof machine={} data={} fact={fact:?} proven={proven}",
                machine.name, definition.name,
            );
        }
        if proven {
            place
                .pending_requirements
                .retain(|pending| *pending != requirement);
        } else if !place.pending_requirements.contains(&requirement) {
            place.pending_requirements.push(requirement);
        }
    }
    all_hold &= place.pending_requirements.is_empty();
    // Close the window only after every requirement holds. A runtime-selected
    // element may differ between writes, so it can preserve an already valid
    // region but cannot establish that an earlier invalid element was restored.
    if all_hold && (!place.spelling.contains("[*]") || region_was_established) {
        place.established = true;
        place.window_open = false;
    } else {
        // Failed initial establishment leaves reserved storage. Only a
        // previously established value owns restoration debt; reads of an
        // unfinished whole value still fail the establishment gate.
        place.window_open = place.established || place.window_open;
    }
    if place.any_window_open() {
        place
            .opening_write
            .get_or_insert(program.expression_table.source_span(value));
    } else {
        place.opening_write = None;
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
    inherited_windows: &[InvariantWindow],
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
    // Reuse incoming field values only when no call or overlapping write has
    // invalidated them. Unknown contents remain unknown.
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
    let pending_requirements = if !established
        || inherited_windows.iter().any(|(spelling, _, _)| {
            place_spelling_covers(&receiver_spelling, spelling)
                || place_spelling_covers(spelling, &receiver_spelling)
        }) {
        domain_requirements(program, definition, seeded_case)
    } else {
        Vec::new()
    };
    tracked.push(TrackedPlace {
        spelling: receiver_spelling,
        definition,
        fields: seeded_fields,
        // A valid incoming record already establishes each child's declared
        // type. Changing another field does not require reconstructing that
        // child. Any nested write still has its own tracked restoration window.
        established_fields: if established {
            super::established_common_fields(program, definition)
        } else {
            Vec::new()
        },
        pending_requirements,
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
        opening_write: None,
        elements: Vec::new(),
        element_windows: Vec::new(),
        alternatives: Vec::new(),
        active_case: seeded_case,
    });
    tracked.len() - 1
}

/// List every declaration requirement that must hold before exposing this
/// value. Keep case identities distinct when its tag is not known.
fn domain_requirements(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    active_case: Option<symbols::SymbolHandle>,
) -> Vec<DomainRequirement> {
    let mut requirements = program
        .conjunctive_proof_facts(definition.where_facts)
        .map(|fact| DomainRequirement {
            case: symbols::SymbolHandle::invalid(),
            fact,
        })
        .collect::<Vec<_>>();
    for member in program.data_members(definition) {
        let Some(case) = member.as_variant() else {
            continue;
        };
        if active_case.is_some_and(|active| active != case.symbol) {
            continue;
        }
        requirements.extend(
            program
                .conjunctive_proof_facts(case.where_facts)
                .map(|fact| DomainRequirement {
                    case: case.symbol,
                    fact,
                }),
        );
    }
    requirements
}
