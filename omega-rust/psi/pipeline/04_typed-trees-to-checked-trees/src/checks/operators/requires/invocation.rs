//! A copied scalar's contract refers to its operand-time value, whereas a
//! reference still denotes its captured referent's live storage at invocation.
//! A by-value formal whose type has stable observable contents is an equally
//! detached carrier: the bound operand is a copy no loan or interior authority
//! can retarget, so its operand-time facts describe the invocation value
//! outright and need no invocation-time liveness. Reference and slice formals
//! are views onto shared storage: the operand's evaluated value denotes a
//! fixed referent at its own completion, so exact referent custody transports
//! the facts live on that storage at invocation — including newer facts a
//! later operand established — instead of intersecting context identities.
//! Operands without exact custody keep the capture ∩ invocation intersection.
//! In particular, rebinding a source reference must not retarget an earlier
//! copy. Select facts per clause
//! leaf, intersecting exact context identities when a relation uses several
//! operands. Unioning their snapshots could combine facts about different
//! versions of the same source place. Conjunctions are proved leaf by leaf by
//! the ordinary requires checker; they do not need all premises at one time.

use arena::Handle;
use checked_trees::{
    CheckFacts, CheckedNamedOperatorUseFact, CheckedOperatorUseFact, FlowFacts,
    FlowOperatorInvocationFact, FlowOperatorOperandFact,
};
use facts::{FactContextHandle, FactPlan};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) struct InvocationContexts<'facts> {
    flow: &'facts FlowFacts,
    semantic: &'facts FactPlan,
    invocation: Option<&'facts FlowOperatorInvocationFact>,
}

impl<'facts> InvocationContexts<'facts> {
    pub(super) fn new(
        facts: &'facts CheckFacts,
        operator_use: Handle<CheckedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        Self::from_flow(&facts.flow, &facts.semantic, operator_use, operands)
    }

    pub(super) fn from_flow(
        flow: &'facts FlowFacts,
        semantic: &'facts FactPlan,
        operator_use: Handle<CheckedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        let mut matching =
            flow.control
                .operator_invocations
                .iter()
                .filter_map(|(_, invocation)| {
                    (invocation.operator_use == operator_use).then_some(invocation)
                });
        let invocation = match (matching.next(), matching.next()) {
            (Some(invocation), None) if operand_expressions_match(flow, invocation, operands) => {
                Some(invocation)
            }
            // Missing, duplicate, or substituted operand custody cannot fall
            // back to statement-entry facts. Only context-free truths remain.
            _ => None,
        };
        Self {
            flow,
            semantic,
            invocation,
        }
    }

    /// The operand-time capture of one named `Namespace::requirement(...)`
    /// call. Named uses produce no `uses` row, so their capture rows key on
    /// the `named_use` handle instead. The caller distinguishes "no capture
    /// was emitted" (entry-context fallback may apply) from present-but-bad
    /// custody: a mismatched or duplicated row selects no contexts, leaving
    /// only context-free truths.
    pub(super) fn from_named_use(
        flow: &'facts FlowFacts,
        semantic: &'facts FactPlan,
        named_use: Handle<CheckedNamedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        let mut matching =
            flow.control
                .operator_invocations
                .iter()
                .filter_map(|(_, invocation)| {
                    (invocation.named_use == named_use).then_some(invocation)
                });
        let invocation = match (matching.next(), matching.next()) {
            (Some(invocation), None) if operand_expressions_match(flow, invocation, operands) => {
                Some(invocation)
            }
            _ => None,
        };
        Self {
            flow,
            semantic,
            invocation,
        }
    }

    pub(super) fn for_expressions(
        &self,
        program: &TypedTrees,
        parameters: &[StateParameter],
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> Vec<FactContextHandle> {
        let Some(invocation) = self.invocation else {
            return Vec::new();
        };
        let mut occurrences = Vec::new();
        for expression in expressions {
            crate::facts::contract_occurrences::append_expression_occurrences(
                program,
                expression,
                &mut occurrences,
            );
        }
        let operands = self
            .flow
            .control
            .operator_operands
            .span_or_empty(invocation.operands);
        let mut selected = None::<Vec<FactContextHandle>>;
        for (ordinal, parameter) in parameters.iter().enumerate() {
            if !occurrences
                .iter()
                .any(|expression| occurrence_root_symbol(program, *expression) == parameter.symbol)
            {
                continue;
            }
            let Some(operand) = operands.get(ordinal) else {
                return Vec::new();
            };
            let mut operand_contexts: Vec<_> = self
                .flow
                .semantic_constraint_contexts(operand.constraints)
                .collect();
            if !operand_binds_stable_copy(program, parameter.type_reference) {
                operand_contexts = if operand.referents.is_empty() {
                    operand_contexts.retain(|candidate| {
                        self.flow
                            .semantic_constraint_contexts(invocation.requires_constraints)
                            .any(|context| context == *candidate)
                    });
                    operand_contexts
                } else {
                    self.invocation_contexts_at_referents(program, operand)
                };
            }
            if let Some(selected) = &mut selected {
                selected.retain(|candidate| operand_contexts.contains(candidate));
            } else {
                selected = Some(operand_contexts);
            }
        }
        selected.unwrap_or_else(|| {
            self.flow
                .semantic_constraint_contexts(invocation.requires_constraints)
                .collect()
        })
    }

    /// Invocation-live contexts carrying at least one fact about a referent
    /// the operand exactly denotes. A bound view reads that storage at
    /// invocation, so its current facts — including ones a later operand
    /// established — are the clause's evidence in place of the captured
    /// snapshot's context identities.
    fn invocation_contexts_at_referents(
        &self,
        program: &TypedTrees,
        operand: &FlowOperatorOperandFact,
    ) -> Vec<FactContextHandle> {
        let Some(invocation) = self.invocation else {
            return Vec::new();
        };
        let referents = self
            .flow
            .control
            .operand_referents
            .span_or_empty(operand.referents);
        self.flow
            .semantic_constraint_contexts(invocation.requires_constraints)
            .filter(|context| {
                self.semantic
                    .refs
                    .span_or_empty(self.semantic.contexts.get(*context).facts)
                    .iter()
                    .any(|fact_ref| {
                        let fact = self.semantic.facts.get(fact_ref.fact);
                        let facts::FactPlace::Place(place) = fact.place else {
                            return false;
                        };
                        let Some(place) = crate::flow::canonical_place_from_semantic_place(
                            program,
                            self.semantic,
                            self.semantic.places.get(place),
                        ) else {
                            return false;
                        };
                        referents.iter().any(|referent| {
                            crate::flow::normalized_event_place_root(program, place.root)
                                == crate::flow::normalized_event_place_root(program, referent.root)
                                && crate::flow::canonical_place_segments_may_overlap(
                                    program,
                                    &place.segments,
                                    self.flow
                                        .control
                                        .operand_referent_segments
                                        .span_or_empty(referent.segments),
                                )
                        })
                    })
            })
            .collect()
    }
}

/// The capture row names its exact operand expressions in evaluation order;
/// a row whose operands differ is substituted custody, not this invocation's.
fn operand_expressions_match(
    flow: &FlowFacts,
    invocation: &FlowOperatorInvocationFact,
    operands: &[ExpressionHandle],
) -> bool {
    flow.control
        .operator_operands
        .span_or_empty(invocation.operands)
        .iter()
        .map(|operand| operand.expression)
        .eq(operands.iter().copied())
}

/// Whether the operand bound to this formal is a detached copy of stable
/// contents, so its operand-time facts describe the invocation value outright.
/// A copied scalar is the narrow case; a by-value record, sum, or fixed array
/// with stable observable contents is equally detached — a later operand's
/// write to the source storage cannot reach the copy, and no loan or interior
/// authority inside it can retarget what the operator received. Reference and
/// slice formals are views onto shared storage: a view operand's recorded
/// referents transport the facts live on that storage at invocation; anything
/// the contents classifier cannot prove stable, or whose referent could not
/// be resolved exactly, keeps the capture ∩ invocation intersection.
fn operand_binds_stable_copy(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { .. } => {
                return program.primitive_type_reference(reference).is_some()
                    || validation::has_stable_observable_contents(program, reference);
            }
            TypeReferenceNode::Generic { .. } | TypeReferenceNode::FixedArray { .. } => {
                return validation::has_stable_observable_contents(program, reference);
            }
            _ => return false,
        }
    }
}

fn occurrence_root_symbol(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> symbols::SymbolHandle {
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => return path.symbol,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            _ => return symbols::SymbolHandle::invalid(),
        }
    }
}
