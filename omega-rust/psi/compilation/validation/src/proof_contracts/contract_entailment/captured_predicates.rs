//! Declaration predicates observe the exact contents at a completed prefix.
//!
//! The ordinary prefix observer owns operand order, call guarantees and storage
//! revisions. Bind the declaration's fields to those current captured values,
//! not to its standing default-domain hypothesis. Missing or ambiguous storage
//! observations cannot close a window. This proves a predicate only; ownership
//! restoration and qualification provenance remain separate obligations.

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::StructuralJudgment;
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use language_semantics::content::ContentPlaceVersion;
use typed_trees::data::DataDefinition;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

#[derive(Clone, Copy)]
pub(crate) enum CapturedPredicatePoint {
    Prefix(usize),
    StatementReturn(usize),
    ExpressionReturn {
        statement_index: usize,
        expression: ExpressionHandle,
    },
}

pub(crate) fn captured_data_predicate_holds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    point: CapturedPredicatePoint,
    place: &crate::CanonicalPlace,
    receiver_type: TypeReferenceHandle,
    definition: &DataDefinition,
    predicate: ExpressionHandle,
) -> bool {
    let Some(application) =
        crate::value_custody::places::unwrapped_type_reference(program, receiver_type)
    else {
        return false;
    };
    if program
        .data_constructor_definition(application, definition.symbol)
        .is_none()
    {
        return false;
    }
    let statement_index = match point {
        CapturedPredicatePoint::Prefix(index) | CapturedPredicatePoint::StatementReturn(index) => {
            index
        }
        CapturedPredicatePoint::ExpressionReturn {
            statement_index, ..
        } => statement_index,
    };
    let in_transition_target = match point {
        CapturedPredicatePoint::ExpressionReturn { expression, .. } => {
            super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            )
        }
        _ => false,
    };
    let mut visitor = CapturedPredicate {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target,
        },
        point,
        place: place.clone(),
        application,
        definition,
        predicate,
        observed: false,
        proven: true,
    };
    let complete = observe_runtime_state_prefix(program, machine, state, &mut visitor).is_some();
    if complete && visitor.has_positive_proof() {
        return true;
    }
    // The declaration-only entry may not know a private invocation's exact
    // incoming guard. Reuse the same closed-arrival owner as call requirements
    // and completed operands; every actual arrival owes this captured-prefix fact.
    visitor.observed = false;
    visitor.proven = true;
    super::structural_prefix::observe_private_arrivals(program, machine, &mut visitor, false)
        == Some(true)
        && visitor.has_positive_proof()
}

#[derive(Clone)]
struct CapturedPredicate<'definition> {
    site: StructuralPrefixSite,
    point: CapturedPredicatePoint,
    place: crate::CanonicalPlace,
    application: TypeReferenceHandle,
    definition: &'definition DataDefinition,
    predicate: ExpressionHandle,
    observed: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for CapturedPredicate<'_> {
    fn has_positive_proof(&self) -> bool {
        self.observed && self.proven
    }

    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.observed = true;
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        self.observed = true;
        let scope = (machine, state, state.return_type);
        let completed = match self.point {
            CapturedPredicatePoint::Prefix(_) => {
                self.judge_predicate(program, machine, state, judge);
                return;
            }
            CapturedPredicatePoint::StatementReturn(index) => program
                .statement_table
                .statement_at(state, index)
                .and_then(|statement| statement.as_call())
                .and_then(|call| {
                    judge.normal_statement_completion_paths(call, index, environment, scope)
                }),
            CapturedPredicatePoint::ExpressionReturn {
                statement_index,
                expression,
            } => {
                judge.normal_operation_completions(statement_index, expression, environment, scope)
            }
        };
        let Some(completed) = completed else {
            self.proven = false;
            return;
        };
        for judge in completed {
            self.judge_predicate(program, machine, state, judge);
        }
    }
}

impl CapturedPredicate<'_> {
    fn judge_predicate(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        judge: super::structural_judgment::StructuralJudge<'_>,
    ) {
        self.proven &= predicate_holds_on_current_storage(
            program,
            machine,
            state,
            &judge,
            &self.place,
            self.application,
            self.definition,
            self.predicate,
        );
    }
}

/// Establish a predicate at this completed operand occurrence, including its
/// selected guard and earlier sibling effects. A descriptor supplies no value
/// premise: the fields are captured from current storage by the prefix owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn completed_data_predicate_holds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    place: &crate::CanonicalPlace,
    definition: &DataDefinition,
    predicate: ExpressionHandle,
) -> bool {
    let Some(reference) =
        crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    let Some(application) = predicate_application(program, place, definition) else {
        return false;
    };
    let result = super::completed_observations::fold_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        true,
        |proven| *proven,
        |proven, observation| {
            *proven &= observation.is_some_and(|(judge, _)| {
                predicate_holds_on_current_storage(
                    program,
                    machine,
                    state,
                    judge,
                    place,
                    application,
                    definition,
                    predicate,
                )
            });
            if *proven {
                std::ops::ControlFlow::Continue(())
            } else {
                std::ops::ControlFlow::Break(())
            }
        },
    );
    result.is_some_and(|result| result.complete && result.state)
}

#[allow(clippy::too_many_arguments)]
fn predicate_holds_on_current_storage(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    judge: &super::structural_judgment::StructuralJudge<'_>,
    place: &crate::CanonicalPlace,
    application: TypeReferenceHandle,
    definition: &DataDefinition,
    predicate: ExpressionHandle,
) -> bool {
    let mut parameters = Vec::new();
    let mut bindings = Vec::new();
    // Static actuals belong to this exact applied predicate, for both stored
    // restoration and consumption of a completed operand.
    let type_bindings = judge.data_application_type_bindings(application);
    for (parameter, value) in judge.data_application_static_bindings(application) {
        bindings.push((parameter.name.to_string(), value));
        parameters.push(parameter);
    }
    for field in program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
    {
        let Some(place) = predicate_field_place(program, place, definition, field.symbol) else {
            return false;
        };
        if let Some(value) = judge.captured_place_projection(&place, ContentPlaceVersion::Current) {
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: field.type_reference,
                ..Default::default()
            });
            bindings.push((field.name.to_string(), value));
        }
    }
    // A requirement may depend only on the field just written. Prove it from
    // captured current fields even when write-only authority prevents reading
    // the rest of the record. Do not invent a complete record or assume the
    // requirement being checked.
    if let Some(term) = current_projection_predicate(
        program,
        judge,
        place,
        application,
        definition,
        predicate,
        (&parameters, &bindings),
    ) && matches!(
        judge.judge_observed_predicate(term, 0),
        StructuralJudgment::Proven
    ) {
        return true;
    }
    let subject_demanded = definition.predicate_subject_symbol.is_valid()
        && crate::queries::expression_traversal::visit_pre_order(
            program,
            vec![predicate],
            |_, node| {
                matches!(node, typed_trees::expression::ExpressionNode::Name(path)
                    if path.symbol == definition.predicate_subject_symbol)
            },
        );
    if subject_demanded {
        let subject = judge.captured_data_subject(place, definition, |field| {
            predicate_field_place(program, place, definition, field)
        });
        let Some(subject) = subject else {
            return false;
        };
        if !definition.predicate_subject_type.is_valid() {
            return false;
        }
        let name = format!(
            "\0stored_subject_{}_{}",
            definition.predicate_subject_symbol.arena_index(),
            definition.predicate_subject_symbol.generation()
        );
        parameters.push(StateParameter {
            symbol: definition.predicate_subject_symbol,
            name: name.as_str().into(),
            type_reference: application,
            ..Default::default()
        });
        bindings.push((name, subject));
    }
    let judgment = instantiated_selected_fact_judgment(
        program,
        judge,
        definition.symbol,
        &parameters,
        predicate,
        &bindings,
        &[],
        &type_bindings,
        Some((machine, state)),
    );
    matches!(judgment, StructuralJudgment::Proven)
}

/// Prove a field-only requirement using captured current fields. If it needs
/// the whole record, crosses a reference, or uses an unavailable field, fall
/// back to the complete-record check. Missing fields never become assumed values.
fn current_projection_predicate(
    program: &dyn crate::CheckingInput,
    judge: &super::structural_judgment::StructuralJudge<'_>,
    place: &crate::CanonicalPlace,
    application: TypeReferenceHandle,
    definition: &DataDefinition,
    predicate: ExpressionHandle,
    environment: (
        &[StateParameter],
        &[(String, super::structural_judgment::StructuralTerm)],
    ),
) -> Option<super::structural_judgment::StructuralTerm> {
    let (parameters, bindings) = environment;
    // Field declarations type the question; only captured contents supply values.
    let mut parameters = parameters.to_vec();
    for field in program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
    {
        if !parameters
            .iter()
            .any(|parameter| parameter.symbol == field.symbol)
        {
            let path = [facts::PlaceSegment::Field {
                symbol: field.symbol,
            }];
            parameters.push(StateParameter {
                symbol: field.symbol,
                name: field.name.clone(),
                type_reference: crate::project_type_reference_from_segments(
                    program,
                    application,
                    &path,
                )?,
                ..Default::default()
            });
        }
    }
    if definition.predicate_subject_symbol.is_valid() {
        parameters.push(StateParameter {
            symbol: definition.predicate_subject_symbol,
            name: "\0current_predicate_subject".into(),
            type_reference: application,
            ..Default::default()
        });
    }
    let observed_projection = std::cell::Cell::new(false);
    let unavailable_projection = std::cell::Cell::new(false);
    let term = super::structural_terms::structural_term(
        program,
        None,
        Some((definition.symbol, &parameters)),
        predicate,
        Some(&|expression| {
            if let Some(path) = super::super::default_domains::predicate_projection(
                program,
                definition,
                application,
                expression,
            ) {
                let selected = (!path.is_empty())
                    .then(|| predicate_projection_place(program, place, application, &path))
                    .flatten();
                let value = selected.as_ref().and_then(|selected| {
                    judge.captured_place_projection(selected, ContentPlaceVersion::Current)
                });
                if super::struct_trace() {
                    eprintln!(
                        "STRUCT current-predicate-field data={} expression={expression:?} selected={selected:?} value={value:?}",
                        definition.name,
                    );
                }
                if value.is_some() {
                    observed_projection.set(true);
                } else {
                    unavailable_projection.set(true);
                }
                return value;
            }
            // A refused projection must not become a symbolic field or subject
            // through the ordinary term reader's fallback. This also covers
            // capture expressions reached through static or mathematical calls.
            if predicate_storage_root(program, definition, expression) {
                unavailable_projection.set(true);
                return None;
            }
            let name = program.expression_table.name_path_node(expression)?;
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == name.symbol)?;
            bindings
                .iter()
                .find(|(name, _)| *name == parameter.name.as_str())
                .map(|(_, value)| value.clone())
        }),
    )?;
    // An opaque call can omit its operands from term construction. It has no
    // identity bound to the observed arguments, so leave it to the full check.
    (observed_projection.get() && !unavailable_projection.get() && term.has_observation_identity())
        .then_some(term)
}

fn predicate_storage_root(
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> bool {
    let Some(root) = crate::canonical_place_from_expression(program, expression)
        .and_then(|place| place.root.as_symbol())
    else {
        return false;
    };
    root == definition.predicate_subject_symbol
        || program
            .data_members(definition)
            .iter()
            .any(|member| match member {
                typed_trees::data::DataMember::Field(field) => field.symbol == root,
                typed_trees::data::DataMember::Variant(case) => program
                    .data_payload_fields(case)
                    .iter()
                    .any(|field| field.symbol == root),
            })
}

/// Rejoin semantic fields to their actual storage at every nested projection.
/// Only fixed coordinates are available here; a runtime selector still needs
/// its own captured value and cannot be replaced by a chosen array element.
fn predicate_projection_place(
    program: &dyn crate::CheckingInput,
    place: &crate::CanonicalPlace,
    mut reference: TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> Option<crate::CanonicalPlace> {
    let mut selected = place.clone();
    for segment in path {
        match *segment {
            facts::PlaceSegment::Field { symbol } => {
                let definition =
                    crate::value_custody::places::data_definition_for_type(program, reference)?;
                selected = predicate_field_place(program, &selected, definition, symbol)?;
            }
            facts::PlaceSegment::FixedIndex { .. } => selected.segments.push(*segment),
            _ => return None,
        }
        reference = crate::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        )?;
    }
    Some(selected)
}

/// Predicates name semantic fields; a selected layout owns distinct physical
/// field symbols. Join them through that exact plan before capturing contents,
/// retaining semantic binders in the predicate instantiation itself.
fn predicate_field_place(
    program: &dyn crate::CheckingInput,
    place: &crate::CanonicalPlace,
    definition: &DataDefinition,
    semantic_field: symbols::SymbolHandle,
) -> Option<crate::CanonicalPlace> {
    let mut symbol = semantic_field;
    if let Some(reference) = place
        .root
        .as_symbol()
        .and_then(|root| crate::declaration_type_reference(program, root))
        .and_then(|reference| {
            crate::project_type_reference_from_segments(program, reference, &place.segments)
        })
    {
        let carrier = crate::value_custody::places::assignment_value_type(program, reference);
        if let Some(physical) = program.closed_data_field_definition(carrier)
            && let Some(plan) = program.plan_laid_layout_for_data(physical)
        {
            let semantic = program.closed_data_field_definition(plan.semantic_type)?;
            if (semantic.symbol, semantic.generic_instance)
                != (definition.symbol, definition.generic_instance)
            {
                return None;
            }
            let mut matches = plan
                .schema_field_symbols
                .iter()
                .enumerate()
                .filter(|(_, candidate)| **candidate == semantic_field);
            let (index, _) = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            symbol = *plan.field_symbols.get(index)?;
        }
    }
    let mut place = place.clone();
    crate::value_custody::places::push_field_place_segments(program, &mut place.segments, symbol);
    Some(place)
}

/// Preserve the complete semantic application when storage is presented by a
/// named layout. No receiving annotation supplies a value or predicate premise.
fn predicate_application(
    program: &dyn crate::CheckingInput,
    place: &crate::CanonicalPlace,
    definition: &DataDefinition,
) -> Option<TypeReferenceHandle> {
    let reference = place
        .root
        .as_symbol()
        .and_then(|root| crate::declaration_type_reference(program, root))?;
    let projected =
        crate::project_type_reference_from_segments(program, reference, &place.segments)?;
    let carrier = crate::value_custody::places::assignment_value_type(program, projected);
    let application = match program
        .closed_data_field_definition(carrier)
        .and_then(|physical| program.plan_laid_layout_for_data(physical))
    {
        Some(plan) => plan.semantic_type,
        None => carrier,
    };
    program.data_constructor_definition(application, definition.symbol)?;
    Some(application)
}
