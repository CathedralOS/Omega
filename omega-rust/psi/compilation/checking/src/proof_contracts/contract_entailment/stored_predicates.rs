//! Default-domain restoration observes the contents after a completed store.
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
use typed_trees::{machine::Machine, state::State};

pub(crate) fn stored_data_predicate_holds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    place: &crate::CanonicalPlace,
    definition: &DataDefinition,
    active_case: Option<symbols::SymbolHandle>,
    predicate: ExpressionHandle,
) -> bool {
    let Some(next_statement) = statement_index.checked_add(1) else {
        return false;
    };
    let mut visitor = StoredPredicate {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index: next_statement,
            in_transition_target: false,
        },
        place: place.clone(),
        definition,
        active_case,
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
    // and completed operands; every actual arrival owes this post-store fact.
    visitor.observed = false;
    visitor.proven = true;
    super::structural_prefix::observe_private_arrivals(program, machine, &mut visitor, false)
        == Some(true)
        && visitor.has_positive_proof()
}

#[derive(Clone)]
struct StoredPredicate<'definition> {
    site: StructuralPrefixSite,
    place: crate::CanonicalPlace,
    definition: &'definition DataDefinition,
    active_case: Option<symbols::SymbolHandle>,
    predicate: ExpressionHandle,
    observed: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for StoredPredicate<'_> {
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
        let (program, machine, state, judge, _) = observation;
        self.observed = true;
        self.proven &= predicate_holds_on_current_storage(
            program,
            machine,
            state,
            &judge,
            &self.place,
            self.definition,
            self.active_case,
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
                    program, machine, state, judge, place, definition, None, predicate,
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
    definition: &DataDefinition,
    active_case: Option<symbols::SymbolHandle>,
    predicate: ExpressionHandle,
) -> bool {
    let mut parameters = Vec::new();
    let mut bindings = Vec::new();
    for field in program
        .data_members(definition)
        .iter()
        .filter_map(|member| member.as_field())
    {
        let mut place = place.clone();
        crate::value_custody::places::push_field_place_segments(
            program,
            &mut place.segments,
            field.symbol,
        );
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
    let subject_demanded = definition.predicate_subject_symbol.is_valid()
        && crate::expression_traversal::visit_pre_order(program, vec![predicate], |_, node| {
            matches!(node, typed_trees::expression::ExpressionNode::Name(path)
                    if path.symbol == definition.predicate_subject_symbol)
        });
    if subject_demanded {
        let variant = match active_case {
            Some(symbol) => program.data_members(definition).iter().find_map(|member| {
                let variant = member.as_variant()?;
                (variant.symbol == symbol).then_some(variant)
            }),
            None => None,
        };
        if active_case.is_some() && variant.is_none()
            || variant.is_none()
                && program
                    .data_members(definition)
                    .iter()
                    .any(|member| member.as_variant().is_some())
        {
            return false;
        }
        let subject = super::structural_terms::captured_constructor_term(
            program,
            definition,
            variant,
            |field| {
                let mut place = place.clone();
                crate::value_custody::places::push_field_place_segments(
                    program,
                    &mut place.segments,
                    field.symbol,
                );
                judge.captured_place_projection(&place, ContentPlaceVersion::Current)
            },
        );
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
            type_reference: definition.predicate_subject_type,
            ..Default::default()
        });
        bindings.push((name, subject));
    }
    let judgment = instantiated_selected_fact_judgment(
        program,
        &judge,
        definition.symbol,
        &parameters,
        predicate,
        &bindings,
        &[],
        &[],
        Some((machine, state)),
    );
    matches!(judgment, StructuralJudgment::Proven)
}
