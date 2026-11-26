//! Select a dot operation only when the receiver's bound type identifies core atomic storage.
//! Until this point a callable field named `store` or `load` is an ordinary member call.
use crate::expressions::call_results::peel;
use crate::expressions::expression::table::member_binding::declared_type;
use crate::expressions::statement::arguments::lower_statement_expression;
use crate::lowerer::Lowerer;
use crate::source_resolved as resolved;
use crate::source_state::expression::{
    BinaryOperator, ExpressionHandle as TypedHandle, ExpressionNode as TypedNode,
    ExpressionTable as TypedExpressions, TableAtomicExpression, TableBinaryExpression,
    TableNamePath,
};
use crate::source_state::statement::{StatementNode, TableAssignment, TableLocalData};
use crate::tables::{resolved_expressions, resolved_statements};
use diagnostics::Diagnostic;
use language_core::atomic::{
    AtomicCompareExchangeOnceResultCustody, AtomicExpressionResultCustody, AtomicOrderingPlan,
    MemoryOrdering,
};
use resolved::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableCallExpression,
};

use crate::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic as Intrinsic, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionLateBinding as Late, AuthoredDeclarationSelectionTarget as Target,
};
use crate::source_selections::AuthoredDeclarationSelectionOccurrenceId as Occurrence;
use language_core::atomic::AtomicCellMember as Operation;

pub(crate) struct AtomicCall<'a> {
    operation: Operation,
    pub(crate) receiver: ExpressionHandle,
    pub(crate) operands: &'a [ExpressionHandle],
    ordering: AtomicOrderingPlan,
    span: source_text::SourceSpan,
}

fn receiver(
    program: &resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    callee: ExpressionHandle,
) -> Option<ExpressionHandle> {
    if !expressions.expression_is_valid(callee) {
        return None;
    }
    let ExpressionNode::Member(member) = expressions.expression(callee) else {
        return None;
    };
    let ty = declared_type(program, expressions, member.receiver, 0)?;
    let resolved::types::TypeReference::Named { symbol, .. } = peel(program, ty) else {
        return None;
    };
    matches!(
        program.symbols.builtin_type_atom(*symbol),
        Some(
            symbols::BuiltinTypeAtom::AtomicBool
                | symbols::BuiltinTypeAtom::AtomicU32
                | symbols::BuiltinTypeAtom::AtomicU64
        )
    )
    .then_some(member.receiver)
}

pub(crate) fn select_expression<'a>(
    program: &resolved::SymbolResolvedTrees,
    expressions: &'a ExpressionTable,
    call: &TableCallExpression,
) -> Result<Option<AtomicCall<'a>>, Diagnostic> {
    let Some(operation) = Operation::from_name(call.target.as_str()) else {
        return Ok(None);
    };
    let Some(receiver) = receiver(program, expressions, call.callee) else {
        return Ok(None);
    };
    if !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement != Default::default()
    {
        return Err(Diagnostic::error(
            "an atomic cell operation takes only its value and ordering operands",
        )
        .with_source_span(call.target.source_span()));
    }
    select(
        operation,
        receiver,
        expressions,
        expressions.expression_handles(call.arguments),
        &call.target,
    )
    .map(Some)
}

fn select<'a>(
    operation: Operation,
    receiver: ExpressionHandle,
    expressions: &ExpressionTable,
    arguments: &'a [ExpressionHandle],
    target: &resolved::name::DiagnosticName,
) -> Result<AtomicCall<'a>, Diagnostic> {
    let span = target.source_span();
    let ordering = |handle| {
        if let ExpressionNode::Name(path) = expressions.expression(handle)
            && !path.static_application.is_valid()
            && let [name] = expressions.name_path_members(path.members)
            && let Some(order) = MemoryOrdering::from_name(name.as_str())
        {
            return Ok(order);
        }
        Err(Diagnostic::error("atomic ordering must be one of `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`")
            .with_source_span(expressions.source_span(handle)))
    };
    let (operand_count, plan) = match (operation, arguments) {
        (Operation::Load, &[argument]) => {
            let value = ordering(argument)?;
            if !value.valid_for_load() {
                return Err(Diagnostic::error(format!(
                    "atomic load cannot use `{}` ordering; use `NoOrdering`, `Receive`, or `GlobalOrder`",
                    value.name()
                ))
                .with_source_span(span));
            }
            (0, AtomicOrderingPlan::Load(value))
        }
        (Operation::Store, &[_, argument]) => {
            let value = ordering(argument)?;
            if !value.valid_for_store() {
                return Err(Diagnostic::error(format!(
                    "atomic store cannot use `{}` ordering; use `NoOrdering`, `Publish`, or `GlobalOrder`",
                    value.name()
                ))
                .with_source_span(span));
            }
            (1, AtomicOrderingPlan::Store(value))
        }
        (
            Operation::FetchAdd
            | Operation::FetchSub
            | Operation::FetchXor
            | Operation::FetchOr
            | Operation::FetchAnd,
            &[_, order],
        ) => (1, AtomicOrderingPlan::ReadModifyWrite(ordering(order)?)),
        (Operation::Swap, &[_, order]) => (1, AtomicOrderingPlan::Swap(ordering(order)?)),
        (Operation::Exchange | Operation::Once(_), &[_, _, success, failure]) => {
            let success = ordering(success)?;
            let failure = ordering(failure)?;
            if !failure.valid_compare_exchange_failure(success) {
                return Err(Diagnostic::error(format!("atomic compare_exchange failure ordering `{}` is invalid for success ordering `{}`; failure cannot publish or be stronger than success", failure.name(), success.name())).with_source_span(span));
            }
            let plan = match operation {
                Operation::Once(_) => AtomicOrderingPlan::CompareExchangeOnce { success, failure },
                _ => AtomicOrderingPlan::CompareExchange { success, failure },
            };
            (2, plan)
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "incorrect argument count for atomic `{}`",
                target.as_str()
            ))
            .with_source_span(span));
        }
    };
    Ok(AtomicCall {
        operation,
        receiver,
        operands: &arguments[..operand_count],
        ordering: plan,
        span,
    })
}

impl AtomicCall<'_> {
    /// Complete the question recorded at this call's source coordinate and keep
    /// it on the typed event even when a local binding expands to an assignment.
    pub(crate) fn publish(
        &self,
        typed: &mut crate::source_state::TypedTrees,
        event: TypedHandle,
        occurrences: impl IntoIterator<Item = Occurrence>,
    ) -> Result<(), Diagnostic> {
        let intrinsic = Intrinsic::AtomicCellOperation(self.operation);
        for occurrence in occurrences {
            let row = typed
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| Diagnostic::error("atomic call lost its authored selection"))?;
            if row.kind() != Kind::Call || row.source_span() != self.span {
                continue;
            }
            match row.target() {
                Target::Intrinsic(selected) if selected == intrinsic => {}
                Target::LateBound(Late::CheckedCall) => {
                    typed
                        .finalize_authored_intrinsic_selection(
                            occurrence,
                            Late::CheckedCall,
                            intrinsic,
                        )
                        .map_err(|error| {
                            Diagnostic::error(format!(
                                "atomic call selection could not be completed: {error:?}"
                            ))
                            .with_source_span(self.span)
                        })?;
                }
                _ => {
                    return Err(Diagnostic::error(
                        "atomic cell operation conflicts with its selected declaration",
                    )
                    .with_source_span(self.span));
                }
            }
            typed
                .expression_table
                .attach_authored_selection_occurrences(event, [occurrence]);
        }
        Ok(())
    }

    pub(crate) fn value(
        &self,
        table: &mut TypedExpressions,
        receiver: TypedHandle,
        operands: Vec<TypedHandle>,
    ) -> Result<TypedHandle, Diagnostic> {
        let (value, result_custody) = match self.operation {
            Operation::Load => (receiver, AtomicExpressionResultCustody::Scalar),
            Operation::Once(observation) => {
                let arguments = table.insert_expression_handles(operands);
                let call = table.insert(TypedNode::Call(
                    crate::source_state::expression::TableCallExpression {
                        receiver,
                        arguments,
                        target: crate::source_state::name::Identifier::generated_static(
                            observation.single_attempt_member_name(),
                        ),
                        ..Default::default()
                    },
                ));
                (
                    call,
                    AtomicExpressionResultCustody::CompareExchangeOnce(
                        AtomicCompareExchangeOnceResultCustody::for_observation(observation),
                    ),
                )
            }
            Operation::Store => {
                return Err(Diagnostic::error("atomic store does not return a value")
                    .with_source_span(self.span));
            }
            _ => {
                return Err(
                    Diagnostic::error("bind the observed result of the atomic update")
                        .with_source_span(self.span),
                );
            }
        };
        Ok(table.insert(TypedNode::Atomic(TableAtomicExpression {
            value,
            result: TypedHandle::invalid(),
            ordering: self.ordering,
            result_custody,
        })))
    }

    pub(crate) fn assignment(
        &self,
        lowerer: &mut Lowerer,
        local: &TableLocalData,
        source: ExpressionHandle,
    ) -> Result<StatementNode, Diagnostic> {
        let target = lower_statement_expression(lowerer, self.receiver)?;
        let mut operands = self
            .operands
            .iter()
            .map(|&operand| lower_statement_expression(lowerer, operand));
        let first = operands.next().expect("selected update has an operand")?;
        let replacement = operands.next().transpose()?;
        let table = &mut lowerer.typed_trees.expression_table;
        let mut names = arena::HandleSpan::empty();
        table.push_name_path_member(&mut names, local.name.clone());
        let mut symbols = arena::HandleSpan::empty();
        table.push_name_path_member_symbol(&mut symbols, local.symbol);
        let prior = table.insert(TypedNode::Name(TableNamePath::new(
            names,
            symbols,
            local.symbol,
            local.symbol,
        )));
        let mut binary = |left, operator, right| {
            table.insert(TypedNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
                policy: numerics::arithmetic::ArithmeticDomain::Exact,
                negation: false,
            }))
        };
        let value = match self.operation {
            Operation::FetchAdd => binary(prior, BinaryOperator::Add, first),
            Operation::FetchSub => binary(prior, BinaryOperator::Subtract, first),
            Operation::FetchXor => binary(prior, BinaryOperator::BitwiseXor, first),
            Operation::FetchOr => binary(prior, BinaryOperator::BitwiseOr, first),
            Operation::FetchAnd => binary(prior, BinaryOperator::BitwiseAnd, first),
            Operation::Swap => first,
            Operation::Exchange => {
                // Keep the expected and replacement operands in the conditional-update
                // shape consumed by Terminal lowering for a decisive compare-exchange.
                let delta = binary(
                    replacement.expect("selected compare_exchange has a replacement"),
                    BinaryOperator::Subtract,
                    prior,
                );
                let matched = binary(prior, BinaryOperator::Equal, first);
                let update = binary(matched, BinaryOperator::Multiply, delta);
                binary(prior, BinaryOperator::Add, update)
            }
            _ => unreachable!("only observed updates have result bindings"),
        };
        let value = table.insert(TypedNode::Atomic(TableAtomicExpression {
            value,
            result: prior,
            ordering: self.ordering,
            result_custody: AtomicExpressionResultCustody::Scalar,
        }));
        table.set_source_span(value, self.span);
        self.publish(
            &mut lowerer.typed_trees,
            value,
            resolved_expressions(lowerer.source_trees).authored_selection_occurrences(source),
        )?;
        Ok(StatementNode::Assignment(TableAssignment { target, value }))
    }
}

pub(crate) fn update_initializer(
    program: &resolved::SymbolResolvedTrees,
    value: ExpressionHandle,
) -> Result<Option<AtomicCall<'_>>, Diagnostic> {
    let expressions = resolved_expressions(program);
    if !expressions.expression_is_valid(value) {
        return Ok(None);
    }
    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return Ok(None);
    };
    let Some(
        Operation::Swap
        | Operation::FetchAdd
        | Operation::FetchSub
        | Operation::FetchXor
        | Operation::FetchOr
        | Operation::FetchAnd
        | Operation::Exchange,
    ) = Operation::from_name(call.target.as_str())
    else {
        return Ok(None);
    };
    select_expression(program, expressions, call)
}

pub(crate) fn store(
    lowerer: &mut Lowerer,
    call: &resolved::statement::TableCall,
) -> Result<Option<StatementNode>, Diagnostic> {
    if call.target.as_str() != "store" {
        return Ok(None);
    }
    let program = lowerer.source_trees;
    let expressions = resolved_expressions(program);
    let Some(cell) = receiver(program, expressions, call.callee) else {
        return Ok(None);
    };
    if call.discards_result
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.operational_acknowledgement != Default::default()
    {
        return Err(Diagnostic::error(
            "atomic store takes only value and ordering operands and returns no result",
        )
        .with_source_span(call.target.source_span()));
    }
    let operation = select(
        Operation::Store,
        cell,
        expressions,
        resolved_statements(program).expression_handles(call.arguments),
        &call.target,
    )?;
    let target = lower_statement_expression(lowerer, cell)?;
    let value = lower_statement_expression(lowerer, operation.operands[0])?;
    let table = &mut lowerer.typed_trees.expression_table;
    let value = table.insert(TypedNode::Atomic(TableAtomicExpression {
        value,
        result: TypedHandle::invalid(),
        ordering: operation.ordering,
        result_custody: AtomicExpressionResultCustody::Scalar,
    }));
    table.set_source_span(value, operation.span);
    operation.publish(
        &mut lowerer.typed_trees,
        value,
        call.authored_call_selection,
    )?;
    Ok(Some(StatementNode::Assignment(TableAssignment {
        target,
        value,
    })))
}

#[cfg(test)]
mod tests;
