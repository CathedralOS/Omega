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
            place.segments.push(facts::PlaceSegment::Field {
                symbol: field.symbol,
            });
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
