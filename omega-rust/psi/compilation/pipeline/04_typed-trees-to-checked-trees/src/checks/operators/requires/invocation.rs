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
    FlowOperatorInvocationFact,
};
use facts::{FactContextHandle, FactPlan};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

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
            .operands
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
            let operand_contexts = crate::flow::operand_contexts(
                program,
                self.flow,
                self.semantic,
                operand,
                parameter.type_reference,
                invocation.requires_constraints,
            );
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
}

/// The capture row names its exact operand expressions in evaluation order;
/// a row whose operands differ is substituted custody, not this invocation's.
fn operand_expressions_match(
    flow: &FlowFacts,
    invocation: &FlowOperatorInvocationFact,
    operands: &[ExpressionHandle],
) -> bool {
    flow.control
        .operands
        .span_or_empty(invocation.operands)
        .iter()
        .map(|operand| operand.expression)
        .eq(operands.iter().copied())
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
