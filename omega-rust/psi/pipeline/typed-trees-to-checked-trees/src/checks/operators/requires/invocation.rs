//! A copied scalar's contract refers to its operand-time value, whereas a
//! reference still denotes its captured referent's live storage at invocation.
//! Non-scalar carriers require the same facts at capture and invocation until
//! exact payload/referent custody supports transporting newer facts. In
//! particular, rebinding a source reference must not retarget an earlier copy.
//! Select facts per clause
//! leaf, intersecting exact context identities when a relation uses several
//! operands. Unioning their snapshots could combine facts about different
//! versions of the same source place. Conjunctions are proved leaf by leaf by
//! the ordinary requires checker; they do not need all premises at one time.

use arena::Handle;
use checked_trees::{CheckFacts, CheckedOperatorUseFact, FlowFacts, FlowOperatorInvocationFact};
use facts::FactContextHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) struct InvocationContexts<'facts> {
    flow: &'facts FlowFacts,
    invocation: Option<&'facts FlowOperatorInvocationFact>,
}

impl<'facts> InvocationContexts<'facts> {
    pub(super) fn new(
        facts: &'facts CheckFacts,
        operator_use: Handle<CheckedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        Self::from_flow(&facts.flow, operator_use, operands)
    }

    pub(super) fn from_flow(
        flow: &'facts FlowFacts,
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
            (Some(invocation), None)
                if flow
                    .control
                    .operator_operands
                    .span_or_empty(invocation.operands)
                    .iter()
                    .map(|operand| operand.expression)
                    .eq(operands.iter().copied()) =>
            {
                Some(invocation)
            }
            // Missing, duplicate, or substituted operand custody cannot fall
            // back to statement-entry facts. Only context-free truths remain.
            _ => None,
        };
        Self { flow, invocation }
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
            crate::contract_occurrences::append_expression_occurrences(
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
            if !is_copied_scalar(program, parameter.type_reference) {
                operand_contexts.retain(|candidate| {
                    self.flow
                        .semantic_constraint_contexts(invocation.requires_constraints)
                        .any(|context| context == *candidate)
                });
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
}

fn is_copied_scalar(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { .. } => {
                return program.primitive_type_reference(reference).is_some();
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
