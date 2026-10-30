//! Normal invocation promises belong to the completed value, with its live
//! backing dependencies. A footprint describes invalidation, not a field path
//! on the selected result. Neither a receiving type nor a loan issues a domain.

use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowCallFact, FlowConstraintRef, FlowOperandFact, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint, QualificationEvidence,
};

impl Execution<'_, '_, '_> {
    pub(super) fn append_invocation_memberships(
        &mut self,
        invocation: Handle<FlowCallFact>,
        value: &mut FlowOperandFact,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if !self.context.control.calls.is_valid(invocation) {
            return;
        }
        let call = self.context.control.calls.get(invocation);
        let target = call.target_symbol;
        let ordinal = call.call_ordinal;
        if call.authored_expression != value.expression
            || call.statement_index != self.statement_index
        {
            return;
        }
        let Some(source) = self.program.expression_table.call_node(value.expression) else {
            return;
        };
        let Some(produced) = self.context.expression_result_type_at(
            self.program,
            self.machine,
            self.state,
            value.expression,
        ) else {
            return;
        };
        if validation::reference_shell(self.program, produced).is_none() {
            return;
        }
        let selected = self
            .program
            .requirement_call_signature(self.machine.symbol, source);
        let rows = crate::flow::calls::call_result_qualification_identities_indexed(
            self.program,
            self.context,
            target,
            selected,
            produced,
        );
        let realization = crate::flow::calls::checked_realization_result_identities(
            self.program,
            self.context,
            source,
        );
        let point = ProgramPoint::CallEnsures {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
            call_ordinal: ordinal,
        };
        for (path, domain, identity) in rows {
            // A checked adapter cannot borrow the boundary requirement's
            // inherited issuer promise. Preserve the ordinary result fence.
            if !crate::flow::calls::result_identity_is_available(
                self.program,
                realization.as_deref().map(Vec::as_slice),
                &(path.clone(), domain, identity),
            ) {
                continue;
            }
            let dependencies = self
                .membership_backing_dependencies(value, &path)
                .unwrap_or_else(|| vec![FactPlace::Unknown]);
            let place = crate::semantic::places::append_place(
                self.semantic,
                PlaceRoot::Expression(value.expression),
                &path,
            );
            let context = crate::semantic::facts::append_observation_context(
                self.semantic,
                Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::CallEnsures,
                    evidence: QualificationEvidence::from_origin(
                        language_semantics::QualificationEvidenceOrigin::Propagated,
                        target,
                    ),
                    payload: FactPayload::DomainMembership {
                        value: value.expression,
                        domain: HandleSpan::empty(),
                        domain_symbol: domain,
                        semantic_domain: identity,
                    },
                },
                &dependencies,
            );
            crate::flow::reference_spans::append_flow_reference(
                &mut self.context.contexts.semantic_context_refs,
                contexts,
                FlowSemanticContextRef { context },
            );
            crate::flow::append_constraint_ref(
                &mut self.context.contexts.constraint_refs,
                constraints,
                checked_trees::FlowConstraintKind::SemanticContext { context },
            );
        }
        value.constraints = *constraints;
    }
    fn membership_backing_dependencies(
        &mut self,
        value: &FlowOperandFact,
        path: &[facts::PlaceSegment],
    ) -> Option<Vec<FactPlace>> {
        let shell = validation::reference_shell(self.program, value.type_reference)?;
        let selected =
            validation::owned_projection_type_reference(self.program, shell.referee, path)?;
        if validation::reference_shell(self.program, selected).is_some()
            || validation::borrow_contracts::is_borrow_carrying_data(self.program, selected)
        {
            return None;
        }
        let mut dependencies = Vec::new();
        for backing in self.completed_backing_places(value)? {
            if !backing.root.is_valid() || backing.root == PlaceRoot::Expression(value.expression) {
                return None;
            }
            let place = crate::flow::project_captured_view_place(backing, path)?;
            let dependency = FactPlace::Place(crate::semantic::places::append_place(
                self.semantic,
                place.root,
                &place.segments,
            ));
            crate::collections::push_unique(&mut dependencies, dependency);
        }
        (!dependencies.is_empty()).then_some(dependencies)
    }
}
