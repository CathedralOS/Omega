//! A completed result keeps its checked scalar guarantees at evaluation.
//!
//! Reuse validation's destination-independent result query: it joins all Match
//! arms by normalized type identity and does not copy operand refinements onto
//! arithmetic results. Publish after child effects and branch meets, never into
//! a callee's requires context or an unselected arm. The subject is this result
//! occurrence, not storage read while producing it.
//!
//! Checked domain qualifications and enforced floating ranges describe the
//! completed result, not its operands or a receiving annotation. Range
//! predicates join the same captured occurrence as tags; invocation checking
//! need not reconstruct a source call
//! or skip its preconditions. This is transport, not establishment authority.
//! The normalized instance travels with the declaration through every fact copy.
//! Equality of exact instances is not a license to equate different open index
//! expressions; those still require their selected normalization/equality proof.
//! Normal-return integer bounds use the selected declaration's contract, not
//! body evaluation or a later read of mutable actuals. Publishing them at the
//! expression occurrence lets constructors and assignments consume one common
//! snapshot; a later sibling write invalidates source storage, not this result.
use crate::flow::append_constraint_ref;
use crate::flow::expression::Execution;
use crate::flow::reference_spans;
use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint, QualificationEvidence};

impl Execution<'_, '_, '_> {
    pub(super) fn append_result_facts(
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
        // Source validation has discharged qualification predicates before
        // flow construction. The completed result owns every declared atom,
        // not just predicate-free tags. Omitting predicate domains here made
        // a checked cast lose its membership at an inline call or return.
        // This does not establish a new domain on a bare operand or storage.
        // A structural carrier is no different: a checked record
        // qualification describes its completed value exactly as a scalar
        // one does.
        let domains =
            validation::field_domains::domain_constraint_identities(self.program, reference);
        let mut predicates = Vec::new();
        crate::values::float_predicates::declared_type_predicates(
            self.program,
            reference,
            &mut predicates,
        );
        if let Some(call) = self.program.expression_table.call_node(expression) {
            validation::normal_return_float_predicates(self.program, call, &mut predicates);
        }
        let integer_bounds = self
            .program
            .expression_table
            .call_node(expression)
            .and_then(|call| validation::normal_return_integer_bounds(self.program, call))
            .and_then(|(minimum, maximum)| {
                Some(facts::IntegerRange {
                    minimum: numerics::bignum::BigInt::from_i64(minimum?),
                    maximum: numerics::bignum::BigInt::from_i64(maximum?),
                })
            });
        if domains.is_empty() && predicates.is_empty() && integer_bounds.is_none() {
            return;
        }
        let place = self.semantic.append_expression_place(expression);
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        let mut refs = HandleSpan::empty();
        if let Some(bounds) = integer_bounds {
            let bounds = self.semantic.integer_ranges.append(bounds);
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::AssignedIntegerBounds { bounds },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        for (domain_symbol, semantic_domain) in domains {
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
                    semantic_domain,
                },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        for predicate in predicates {
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::FloatPredicate { predicate },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        let context = self.semantic.append_context(point, refs);
        reference_spans::append_flow_reference(
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
