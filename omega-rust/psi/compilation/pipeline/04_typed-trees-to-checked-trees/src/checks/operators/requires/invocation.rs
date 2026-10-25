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
    CheckFacts, CheckedNamedOperatorUseFact, CheckedOperatorUseFact, FlowCallFact, FlowFacts,
    FlowOperatorInvocationFact,
};
use facts::{FactContextHandle, FactPlan};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

pub(crate) struct InvocationContexts<'facts> {
    flow: &'facts FlowFacts,
    semantic: &'facts FactPlan,
    invocation: Option<CapturedInvocation<'facts>>,
}

enum CapturedInvocation<'facts> {
    Operator(&'facts FlowOperatorInvocationFact),
    Call {
        call: &'facts FlowCallFact,
        receiver_offset: usize,
    },
}

impl<'facts> InvocationContexts<'facts> {
    /// Ordinary state and machine calls share the scalar snapshot law with
    /// operator calls. The caller supplies the authenticated explicit telescope;
    /// a namespace receiver is evaluated but is not a formal parameter.
    pub(crate) fn from_call(
        facts: &'facts CheckFacts,
        call: &'facts FlowCallFact,
        arguments: &[ExpressionHandle],
    ) -> Self {
        let operands = facts.flow.control.operands.span_or_empty(call.operands);
        let receiver_offset = operands.len().checked_sub(arguments.len());
        let invocation = receiver_offset
            .filter(|offset| *offset <= 1)
            .filter(|offset| {
                call.authored_source_custody_valid
                    && operands[*offset..]
                        .iter()
                        .map(|operand| operand.expression)
                        .eq(arguments.iter().copied())
            })
            .map(|receiver_offset| CapturedInvocation::Call {
                call,
                receiver_offset,
            });
        Self {
            flow: &facts.flow,
            semantic: &facts.semantic,
            invocation,
        }
    }

    pub(crate) fn new(
        facts: &'facts CheckFacts,
        operator_use: Handle<CheckedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        Self::from_flow(&facts.flow, &facts.semantic, operator_use, operands)
    }

    pub(crate) fn from_flow(
        flow: &'facts FlowFacts,
        semantic: &'facts FactPlan,
        operator_use: Handle<CheckedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        Self::from_matching_invocation(flow, semantic, operands, |invocation| {
            invocation.operator_use == operator_use
        })
    }

    fn from_matching_invocation(
        flow: &'facts FlowFacts,
        semantic: &'facts FactPlan,
        operands: &[ExpressionHandle],
        matches: impl Fn(&FlowOperatorInvocationFact) -> bool,
    ) -> Self {
        let mut matching = flow
            .control
            .operator_invocations
            .iter()
            .filter_map(|(_, invocation)| matches(invocation).then_some(invocation));
        let invocation = match (matching.next(), matching.next()) {
            (Some(invocation), None) if operand_expressions_match(flow, invocation, operands) => {
                Some(CapturedInvocation::Operator(invocation))
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
    pub(crate) fn from_named_use(
        flow: &'facts FlowFacts,
        semantic: &'facts FactPlan,
        named_use: Handle<CheckedNamedOperatorUseFact>,
        operands: &[ExpressionHandle],
    ) -> Self {
        Self::from_matching_invocation(flow, semantic, operands, |invocation| {
            invocation.named_use == named_use
        })
    }

    /// Select the exact capture of a resolved expression occurrence. Spelled,
    /// named operator and ordinary requirement calls retain their own source
    /// identities while sharing the same detached-value/live-view context law.
    pub(crate) fn for_selected_expression(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        expression: ExpressionHandle,
        selected: symbols::SymbolHandle,
        operands: &[ExpressionHandle],
    ) -> Self {
        Self::for_selected_expression_in_flow(
            program,
            &facts.operators,
            &facts.flow,
            &facts.semantic,
            expression,
            selected,
            operands,
        )
    }

    /// The same exact occurrence binding while the checked facts are being
    /// assembled. Readers borrow the published owners independently; they do
    /// not manufacture a temporary aggregate or re-evaluate source operands.
    pub(crate) fn for_selected_expression_in_flow(
        program: &dyn ::validation::CheckingInput,
        operators: &'facts checked_trees::CheckedOperatorFacts,
        flow: &'facts FlowFacts,
        semantic: &'facts FactPlan,
        expression: ExpressionHandle,
        selected: symbols::SymbolHandle,
        operands: &[ExpressionHandle],
    ) -> Self {
        let mut spelled = Vec::new();
        operators.for_each_expression_use(expression, |handle, fact| {
            if fact.selected_operator_symbol == selected
                && fact.status == checked_trees::CheckedOperatorResolutionStatus::Resolved
                && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
            {
                spelled.push((handle, fact));
            }
        });
        let named = operators
            .named_uses
            .iter()
            .filter(|(_, fact)| {
                fact.expression == expression && fact.selected_operator_symbol == selected
            })
            .collect::<Vec<_>>();
        if let ([(handle, _)], []) = (spelled.as_slice(), named.as_slice()) {
            return Self::from_flow(flow, semantic, *handle, operands);
        }
        if let ([], [(handle, _)]) = (spelled.as_slice(), named.as_slice()) {
            return Self::from_named_use(flow, semantic, *handle, operands);
        }
        // Ordinary calls retain their entry-state target; boundary requirement
        // selections name the declaration. Compare that established owner
        // identity without weakening occurrence or operand custody.
        let mut calls = flow.control.calls.iter().filter_map(|(_, call)| {
            (call.authored_expression == expression
                && validation::find_machine_by_entry_state(program, call.authored_target)
                    .map(|(machine, _)| machine.symbol)
                    .unwrap_or(call.authored_target)
                    == selected
                && call.authored_source_custody_valid
                && flow
                    .control
                    .operands
                    .span_or_empty(call.operands)
                    .iter()
                    .map(|operand| operand.expression)
                    .eq(operands.iter().copied()))
            .then_some(call)
        });
        let invocation = match (
            spelled.is_empty(),
            named.is_empty(),
            calls.next(),
            calls.next(),
        ) {
            (true, true, Some(call), None) => Some(CapturedInvocation::Call {
                call,
                receiver_offset: 0,
            }),
            _ => None,
        };
        Self {
            flow,
            semantic,
            invocation,
        }
    }

    pub(crate) fn entry_constraints(
        &self,
    ) -> Option<arena::HandleSpan<checked_trees::FlowConstraintRef>> {
        Some(match self.invocation.as_ref()? {
            CapturedInvocation::Operator(invocation) => invocation.requires_constraints,
            CapturedInvocation::Call { call, .. } => call.entry_constraints,
        })
    }

    pub(crate) fn for_expressions(
        &self,
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> Vec<FactContextHandle> {
        let Some(invocation) = &self.invocation else {
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
        let (operands, requires_constraints, receiver_offset) = match invocation {
            CapturedInvocation::Operator(invocation) => {
                (invocation.operands, invocation.requires_constraints, 0)
            }
            CapturedInvocation::Call {
                call,
                receiver_offset,
            } => (call.operands, call.entry_constraints, *receiver_offset),
        };
        let operands = self.flow.control.operands.span_or_empty(operands);
        let mut selected = None::<Vec<FactContextHandle>>;
        for (ordinal, parameter) in parameters.iter().enumerate() {
            if !occurrences
                .iter()
                .any(|expression| occurrence_root_symbol(program, *expression) == parameter.symbol)
            {
                continue;
            }
            let Some(operand) = operands.get(ordinal + receiver_offset) else {
                return Vec::new();
            };
            let operand_contexts = crate::flow::operand_contexts(
                program,
                &self.flow.contexts,
                &self.flow.control,
                self.semantic,
                operand,
                parameter.type_reference,
                requires_constraints,
            );
            if let Some(selected) = &mut selected {
                selected.retain(|candidate| operand_contexts.contains(candidate));
            } else {
                selected = Some(operand_contexts);
            }
        }
        selected.unwrap_or_else(|| {
            self.flow
                .semantic_constraint_contexts(requires_constraints)
                .collect()
        })
    }

    pub(crate) fn flow(&self) -> &'facts FlowFacts {
        self.flow
    }

    pub(crate) fn operand_at(
        &self,
        ordinal: usize,
    ) -> Option<&'facts checked_trees::FlowOperandFact> {
        let invocation = self.invocation.as_ref()?;
        let (operands, receiver_offset) = match invocation {
            CapturedInvocation::Operator(invocation) => (invocation.operands, 0),
            CapturedInvocation::Call {
                call,
                receiver_offset,
            } => (call.operands, *receiver_offset),
        };
        self.flow
            .control
            .operands
            .span_or_empty(operands)
            .get(ordinal + receiver_offset)
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
    program: &dyn ::validation::CheckingInput,
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
