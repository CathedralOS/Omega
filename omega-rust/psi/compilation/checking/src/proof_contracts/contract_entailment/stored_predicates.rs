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
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn stored_data_predicate_holds(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
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
    application: TypeReferenceHandle,
    definition: &'definition DataDefinition,
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
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        // Restoring a value judges the same applied predicate as constructing
        // it. Its static arguments are part of that complete telescope, even
        // when the field captures themselves are runtime scalars.
        let type_bindings = judge.data_application_type_bindings(self.application);
        for (parameter, value) in judge.data_application_static_bindings(self.application) {
            bindings.push((parameter.name.to_string(), value));
            parameters.push(parameter);
        }
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
            if let Some(value) =
                judge.captured_place_projection(&place, ContentPlaceVersion::Current)
            {
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
            let Some(subject) = judge.captured_data_subject(&self.place, self.definition) else {
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
                // The declaration subject can name its generic origin's field
                // view. The performed store retains this exact receiving
                // carrier, whose current constructor supplies these fields.
                type_reference: self.application,
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
            &type_bindings,
            Some((machine, state)),
        );
        if super::struct_trace() {
            eprintln!(
                "STRUCT stored-predicate data={} predicate={} parameters={parameters:?} bindings={bindings:?} judgment={judgment:?}",
                self.definition.name,
                program.expression_table.display_name(self.predicate),
            );
        }
        self.proven &= matches!(judgment, StructuralJudgment::Proven);
    }
}
