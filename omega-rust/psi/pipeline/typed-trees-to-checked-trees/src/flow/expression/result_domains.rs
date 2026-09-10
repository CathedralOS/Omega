//! A computed result keeps its declared scalar tags at its evaluation point.
//!
//! Reuse validation's destination-independent result query: it joins all Match
//! arms by normalized type identity and does not copy operand refinements onto
//! arithmetic results. Publish after child effects and branch meets, never into
//! a callee's requires context or an unselected arm. The subject is this result
//! occurrence, not storage read while producing it.
//!
//! This is tag transport, not predicate proof or establishment authority. The
//! current membership payload identifies a domain declaration but cannot retain
//! index arguments, so indexed families are excluded even when an instance's
//! predicate is vacuous. Those need exact-instance proof vocabulary first.

use super::*;

impl Execution<'_, '_, '_> {
    pub(super) fn append_result_domains(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if !self.program.expression_table.expression_is_valid(expression)
            // Storage-backed values already have invalidation-aware facts. A
            // declared place type cannot restore a promise lost to a write.
            || matches!(self.program.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) | ExpressionNode::Borrow(_))
        {
            return;
        }
        let Some(reference) = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            expression,
        ) else {
            return;
        };
        let domains = validation::scalar_type_index_free_tags(self.program, reference);
        if domains.is_empty() {
            return;
        }
        let place = self.semantic.append_expression_place(expression);
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        let mut refs = HandleSpan::empty();
        for domain_symbol in domains {
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::DomainMembership {
                    value: expression,
                    domain: HandleSpan::empty(),
                    domain_symbol,
                },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        let context = self.semantic.append_context(point, refs);
        common::append_flow_reference(
            &mut self.context.contexts.semantic_context_refs,
            contexts,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            &mut self.context.contexts.constraint_refs,
            constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
}
