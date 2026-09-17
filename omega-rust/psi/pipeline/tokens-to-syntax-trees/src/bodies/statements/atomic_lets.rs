//! Atomic `let` forms: compare-exchange, fetch, swap and desugared stores.

use crate::bodies::statements::statement_tables::copy_expression_as_place;
use crate::expressions::parse_expression::parse_expression_handle;
use crate::expressions::parse_postfix::memory_ordering_from_expression;
use crate::input::token_cursor::Input;
use crate::type_syntax::parse_type::parse_type_reference_handle_allowing_borrow;
use arena::HandleSpan;
use numerics::literals::IntegerLiteral;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression, TableBinaryExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{StatementNode, TableAssignment, TableLocalData};
use tokens::{KeywordKind, PunctuationKind};

/// ATOMICS STAGE 1 (ch17, M2): Recognise `atomic_place.store(value, ordering)`
/// -- a Call expression with target name `"store"` and exactly two arguments
/// (the value to write and a validated ordering identifier) -- and desugar it into an
/// Assignment of the receiver place to the first argument. Returns `None` for
/// any other expression, leaving it to the normal statement paths.
/// ATOMICS (ch17): Try to parse and carry
/// `let name: type = place.fetch_add(delta, ordering);` as TWO statements:
///   1. reserve the result local without reading the atomic place;
///   2. attach an opaque atomic carrier to the arithmetic-shaped interpreter
///      model. Native selection replaces the pair with one RMW instruction and
///      stores that instruction's observed prior into the result local.
///
/// Returns `None` if the input does not
/// match the `let ... = ...fetch_add(...)` form, leaving the caller to fall
/// back to `parse_statement_handle`.
///
/// The returned span covers exactly two statement entries that are already
/// appended to `syntax_trees.items`; callers must advance their span
/// accounting by 2.
/// ATOMICS (ch17): Try to parse and carry
/// `let name: type = place.compare_exchange(expected, new_val, succ_ord, fail_ord);`
/// as two statements. The first reserves the result local without reading the
/// atomic place. The second carries
/// `prior + (prior == expected) * (new_val - prior)` as the interpreter model
/// inside an opaque CAS carrier. Arithmetically, when `prior == expected`
/// evaluates to 1 this simplifies to `place = new_val`; when 0, `place = prior`
/// (no-op). Native selection replaces the carrier with one CAS and writes its
/// observed prior into the result local.
///
/// Return-shape choice: the PRIOR value (before the potential swap), not a
/// bool.  This mirrors x86 CMPXCHG's RAX contract and lets callers check
/// success with `prior == expected`.
///
/// Returns `None` if the input does not match the form (wrong name or arity),
/// leaving the caller to fall back to `parse_statement_handle`.
/// The returned span covers exactly two statement entries already appended to
/// `syntax_trees.items`; callers must advance their span accounting by 2.
pub(crate) fn try_parse_atomic_compare_exchange_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    // Must start with `let`.
    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    let (name, after_name) = after_let.take_identifier().ok()?;
    let after_colon = after_name
        .take_punctuation(PunctuationKind::Colon, ":")
        .ok()?;
    let (type_reference, after_type) =
        parse_type_reference_handle_allowing_borrow(syntax_trees, after_colon).ok()?;
    let after_eq = after_type
        .take_punctuation(PunctuationKind::Equal, "=")
        .ok()?;

    // Parse the right-hand expression.
    let (rhs, after_rhs) = parse_expression_handle(syntax_trees, after_eq).ok()?;
    let after_semi = after_rhs
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    // Check: is rhs a Call with target "compare_exchange" and exactly 4 args?
    let (place_expr, expected_expr, new_val_expr, success_ordering, failure_ordering) = {
        let ExpressionNode::Call(ref call) = *syntax_trees.expressions.expression(rhs) else {
            return None;
        };
        if call.target.as_str() != "compare_exchange" {
            return None;
        }
        let arg_handles = syntax_trees
            .tables
            .expressions
            .expression_handles(call.arguments)
            .to_vec();
        if arg_handles.len() != 4 {
            return None;
        }
        let place = call.receiver;
        if !place.is_valid() {
            return None;
        }
        // arg 0 = expected, arg 1 = new_val, arg 2 = success_ord, arg 3 = fail_ord
        let success = memory_ordering_from_expression(syntax_trees, arg_handles[2]).ok()?;
        let failure = memory_ordering_from_expression(syntax_trees, arg_handles[3]).ok()?;
        (place, arg_handles[0], arg_handles[1], success, failure)
    };

    // Reserve the result slot without reading the atomic place. The atomic
    // instruction writes its observed prior value into this local; a separate
    // ordinary read would race and could disagree with the RMW observation.
    let zero = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let local_stmt = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: zero,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        }));
    let first_handle = syntax_trees.items.append_statement_handle(local_stmt);

    // Build a Name expression referring to the freshly-bound local `name`.
    // This appears twice in the RHS arithmetic so we build it twice.
    let make_prior_name = |syntax_trees: &mut SyntaxTrees| {
        let id = syntax_trees::identifier::Identifier::generated(name.as_str());
        let member = syntax_trees.expressions.append_identifier_path_member(id);
        let path = HandleSpan::from_parts(member, 1);
        syntax_trees.expressions.insert(ExpressionNode::Name(path))
    };

    // Statement 2: `place = prior + (prior == expected) * (new_val - prior);`
    //
    //  sub_expr  = new_val - prior
    //  eq_expr   = prior == expected
    //  mul_expr  = eq_expr * sub_expr
    //  add_expr  = prior + mul_expr
    let prior_for_sub = make_prior_name(syntax_trees);
    let sub_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: new_val_expr,
            operator: BinaryOperator::Subtract,
            right: prior_for_sub,
        }));

    let prior_for_eq = make_prior_name(syntax_trees);
    let eq_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: prior_for_eq,
            operator: BinaryOperator::Equal,
            right: expected_expr,
        }));

    let mul_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: eq_expr,
            operator: BinaryOperator::Multiply,
            right: sub_expr,
        }));

    let prior_for_add = make_prior_name(syntax_trees);
    let add_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: prior_for_add,
            operator: BinaryOperator::Add,
            right: mul_expr,
        }));
    let add_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value: add_expr,
            result: prior_for_add,
            ordering: language_core::atomic::AtomicOrderingPlan::CompareExchange {
                success: success_ordering,
                failure: failure_ordering,
            },
            result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
        }));

    let place_for_assign = copy_expression_as_place(syntax_trees, place_expr)?;
    let assign_stmt = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target: place_for_assign,
            value: add_expr,
        }));
    syntax_trees.items.append_statement_handle(assign_stmt);

    let span = HandleSpan::from_parts(first_handle, 2);
    Some((span, after_semi))
}

pub(crate) fn try_parse_atomic_fetch_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    // Must start with `let`.
    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    let (name, after_name) = after_let.take_identifier().ok()?;
    let after_colon = after_name
        .take_punctuation(PunctuationKind::Colon, ":")
        .ok()?;
    let (type_reference, after_type) =
        parse_type_reference_handle_allowing_borrow(syntax_trees, after_colon).ok()?;
    let after_eq = after_type
        .take_punctuation(PunctuationKind::Equal, "=")
        .ok()?;

    // Parse the right-hand expression.
    let (rhs, after_rhs) = parse_expression_handle(syntax_trees, after_eq).ok()?;
    let after_semi = after_rhs
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    // Check: is rhs a supported fetch arithmetic call with exactly 2 args?
    let (place_expr, operand_expr, operator, ordering) = {
        let ExpressionNode::Call(ref call) = *syntax_trees.expressions.expression(rhs) else {
            return None;
        };
        let operator = match call.target.as_str() {
            "fetch_add" => BinaryOperator::Add,
            "fetch_sub" => BinaryOperator::Subtract,
            "fetch_xor" => BinaryOperator::BitwiseXor,
            "fetch_or" => BinaryOperator::BitwiseOr,
            "fetch_and" => BinaryOperator::BitwiseAnd,
            _ => return None,
        };
        let arg_handles = syntax_trees
            .tables
            .expressions
            .expression_handles(call.arguments)
            .to_vec();
        if arg_handles.len() != 2 {
            return None;
        }
        let place = call.receiver;
        if !place.is_valid() {
            return None;
        }
        let ordering = memory_ordering_from_expression(syntax_trees, arg_handles[1]).ok()?;
        (place, arg_handles[0], operator, ordering)
    };

    // Reserve the result slot without reading the atomic place. The atomic
    // instruction writes its observed prior value into this local.
    let zero = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let local_stmt = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: zero,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        }));
    let first_handle = syntax_trees.items.append_statement_handle(local_stmt);

    // The wrapper makes the binary's left operand the instruction-result
    // destination rather than an arithmetic source.
    let result_name = {
        let id = syntax_trees::identifier::Identifier::generated(name.as_str());
        let member = syntax_trees.expressions.append_identifier_path_member(id);
        let path = HandleSpan::from_parts(member, 1);
        syntax_trees.expressions.insert(ExpressionNode::Name(path))
    };
    let update_expr =
        syntax_trees
            .expressions
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: result_name,
                operator,
                right: operand_expr,
            }));
    let update_expr =
        syntax_trees
            .expressions
            .insert(ExpressionNode::Atomic(TableAtomicExpression {
                value: update_expr,
                result: result_name,
                ordering: language_core::atomic::AtomicOrderingPlan::ReadModifyWrite(ordering),
                result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
            }));
    let place_for_assign = copy_expression_as_place(syntax_trees, place_expr)?;
    let assign_stmt = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment {
            target: place_for_assign,
            value: update_expr,
        }));
    syntax_trees.items.append_statement_handle(assign_stmt);

    let span = HandleSpan::from_parts(first_handle, 2);
    Some((span, after_semi))
}

/// Parse `let prior: T = place.swap(replacement, ordering);`. The result local
/// is reserved without reading `place`; the atomic carrier names it as the
/// destination for the instruction-observed prior value.
pub(crate) fn try_parse_atomic_swap_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(
    HandleSpan<syntax_trees::statement::StatementHandle>,
    Input<'tokens, 'source>,
)> {
    if !input.at_keyword(KeywordKind::Let) {
        return None;
    }
    let after_let = input.take_keyword(KeywordKind::Let, "let").ok()?;
    let (name, after_name) = after_let.take_identifier().ok()?;
    let after_colon = after_name
        .take_punctuation(PunctuationKind::Colon, ":")
        .ok()?;
    let (type_reference, after_type) =
        parse_type_reference_handle_allowing_borrow(syntax_trees, after_colon).ok()?;
    let after_eq = after_type
        .take_punctuation(PunctuationKind::Equal, "=")
        .ok()?;
    let (rhs, after_rhs) = parse_expression_handle(syntax_trees, after_eq).ok()?;
    let after_semi = after_rhs
        .take_punctuation(PunctuationKind::Semicolon, ";")
        .ok()?;

    let (place, replacement, ordering) = {
        let ExpressionNode::Call(call) = syntax_trees.expressions.expression(rhs).clone() else {
            return None;
        };
        if call.target.as_str() != "swap" || !call.receiver.is_valid() {
            return None;
        }
        let arguments = syntax_trees
            .tables
            .expressions
            .expression_handles(call.arguments);
        let [replacement, ordering] = arguments else {
            return None;
        };
        let ordering = memory_ordering_from_expression(syntax_trees, *ordering).ok()?;
        (call.receiver, *replacement, ordering)
    };

    let zero = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let local = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: zero,
            is_mutable: false,
            relevance: language_core::BindingRelevance::Relevant,
        }));
    let first = syntax_trees.items.append_statement_handle(local);

    let result = {
        let identifier = Identifier::generated(name.as_str());
        let member = syntax_trees
            .expressions
            .append_identifier_path_member(identifier);
        let path = HandleSpan::from_parts(member, 1);
        syntax_trees.expressions.insert(ExpressionNode::Name(path))
    };
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value: replacement,
            result,
            ordering: language_core::atomic::AtomicOrderingPlan::Swap(ordering),
            result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
        }));
    let target = copy_expression_as_place(syntax_trees, place)?;
    let assignment = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment { target, value }));
    syntax_trees.items.append_statement_handle(assignment);

    Some((HandleSpan::from_parts(first, 2), after_semi))
}

pub(crate) fn try_desugar_atomic_store(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableAssignment> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };
    if call.target.as_str() != "store" {
        return None;
    }
    let argument_count = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments)
        .len();
    if argument_count != 2 {
        // Not the atomic store shape (wrong arity); fall through to normal
        // call-statement or error path.
        return None;
    }
    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments);
    let value = arguments[0];
    let ordering = memory_ordering_from_expression(syntax_trees, arguments[1]).ok()?;
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value,
            result: ExpressionHandle::invalid(),
            ordering: language_core::atomic::AtomicOrderingPlan::Store(ordering),
            result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
        }));
    let receiver = call.receiver;
    // receiver must be a valid place expression (member/indexed path). If it
    // is not, `None` lets the statement parser continue normally.
    if !receiver.is_valid() {
        return None;
    }
    Some(TableAssignment {
        target: receiver,
        value,
    })
}
