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
use typed_trees::data::DataDefinition;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn stored_data_predicate_holds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    receiver: ExpressionHandle,
    definition: &DataDefinition,
    active_case: Option<symbols::SymbolHandle>,
    predicate: ExpressionHandle,
) -> bool {
    let Some(place) = crate::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        receiver,
    ) else {
        return false;
    };
    let Some(next_statement) = statement_index.checked_add(1) else {
        return false;
    };
    let mut visitor = StoredPredicate {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index: next_statement,
            in_transition_target: false,
        },
        place,
        definition,
        active_case,
        predicate,
        observed: false,
        proven: true,
    };
    let complete = observe_runtime_state_prefix(program, machine, state, &mut visitor).is_some();
    complete && visitor.observed && visitor.proven
}

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
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        for field in program
            .data_members(self.definition)
            .iter()
            .filter_map(|member| member.as_field())
        {
            let mut place = self.place.clone();
            crate::value_custody::places::push_field_place_segments(
                program,
                &mut place.segments,
                field.symbol,
            );
            if let Some(value) = judge.captured_place_projection(
                &place,
                language_semantics::content::ContentPlaceVersion::Current,
            ) {
                parameters.push(StateParameter {
                    symbol: field.symbol,
                    name: field.name.clone(),
                    type_reference: field.type_reference,
                    ..Default::default()
                });
                bindings.push((field.name.to_string(), value));
            }
        }
        let subject_demanded = self.definition.predicate_subject_symbol.is_valid()
            && crate::expression_traversal::visit_pre_order(
                program,
                vec![self.predicate],
                |_, node| {
                    matches!(node, typed_trees::expression::ExpressionNode::Name(path)
                    if path.symbol == self.definition.predicate_subject_symbol)
                },
            );
        if subject_demanded {
            let variant = match self.active_case {
                Some(symbol) => program
                    .data_members(self.definition)
                    .iter()
                    .find_map(|member| {
                        let variant = member.as_variant()?;
                        (variant.symbol == symbol).then_some(variant)
                    }),
                None => None,
            };
            if self.active_case.is_some() && variant.is_none()
                || variant.is_none()
                    && program
                        .data_members(self.definition)
                        .iter()
                        .any(|member| member.as_variant().is_some())
            {
                self.proven = false;
                return;
            }
            let subject = super::structural_terms::captured_constructor_term(
                program,
                self.definition,
                variant,
                |field| {
                    let mut place = self.place.clone();
                    crate::value_custody::places::push_field_place_segments(
                        program,
                        &mut place.segments,
                        field.symbol,
                    );
                    judge.captured_place_projection(
                        &place,
                        language_semantics::content::ContentPlaceVersion::Current,
                    )
                },
            );
            let Some(subject) = subject else {
                self.proven = false;
                return;
            };
            if !self.definition.predicate_subject_type.is_valid() {
                self.proven = false;
                return;
            }
            let name = format!(
                "\0stored_subject_{}_{}",
                self.definition.predicate_subject_symbol.arena_index(),
                self.definition.predicate_subject_symbol.generation()
            );
            parameters.push(StateParameter {
                symbol: self.definition.predicate_subject_symbol,
                name: name.as_str().into(),
                type_reference: self.definition.predicate_subject_type,
                ..Default::default()
            });
            bindings.push((name, subject));
        }
        let judgment = instantiated_selected_fact_judgment(
            program,
            &judge,
            self.definition.symbol,
            &parameters,
            self.predicate,
            &bindings,
            &[],
            Some((machine, state)),
        );
        self.proven &= matches!(judgment, StructuralJudgment::Proven);
    }
}
