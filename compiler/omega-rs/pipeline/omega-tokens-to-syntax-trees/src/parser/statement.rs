use crate::parser::expression::{parse_expression_handle, parse_spawn_block_call_handle};
use crate::parser::input::{Input, ParseResult};
use crate::parser::transition::parse_transition_block_target_handle;
use crate::parser::type_reference::parse_type_reference_handle_allowing_borrow;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableCallExpression,
    TableIndexedExpression, TableMemberExpression,
};
use omega_syntax_trees::statement::{
    StatementHandle, StatementNode, TableAssignment, TableCall, TableLocalData, TableRelax,
    TableTransition, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    if input.at_keyword(KeywordKind::Let) {
        let input = input.take_keyword(KeywordKind::Let, "let")?;
        return parse_local_data_statement_handle(syntax_trees, input);
    }

    if input.at_keyword(KeywordKind::If) {
        return parse_if_transition_statement_handle(syntax_trees, input);
    }

    if input.at_contextual("relax") {
        return parse_relax_statement_handle(syntax_trees, input);
    }

    if input.at_contextual("asm") {
        return parse_asm_statement_handle(syntax_trees, input);
    }

    // CONCURRENCY STAGE 1: a statement-position `spawn { call; }` is the
    // fire-and-forget form. Under the synchronous-spawn desugar (see
    // `expression/spawn.rs`) the call simply RUNS HERE as an ordinary call
    // statement; frozen decision 9's strict-result rule still governs a
    // discarded non-unit result. `spawn` stays contextual: without a `{` it
    // falls through and parses as a plain identifier.
    if input.at_contextual("spawn") {
        let after_spawn = input.take_contextual("spawn")?;
        if after_spawn.at_punctuation(PunctuationKind::LeftBrace) {
            return parse_spawn_statement_handle(syntax_trees, after_spawn);
        }
    }

    if input.at_contextual("_") {
        let after_underscore = input.take_contextual("_")?;
        if after_underscore.at_punctuation(PunctuationKind::Equal) {
            return parse_discard_statement_handle(syntax_trees, after_underscore);
        }
    }

    if input.at_contextual("trap") {
        let input = input.take_contextual("trap")?;
        let input = if input.at_punctuation(PunctuationKind::Semicolon) {
            input.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            input
        };
        let target = syntax_trees
            .statements
            .insert_transition_target(TransitionTargetNode::Terminal);
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Transition(TableTransition {
                    target,
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                })),
            input,
        ));
    }

    let (expression, input) = parse_expression_handle(syntax_trees, input)?;

    // ATOMICS STAGE 1 (ch17, M2): `atomic_place.store(value, ordering);` is
    // desugared here into `atomic_place = value;`. The postfix parser keeps
    // the Call node intact (target="store", 2 arguments) so we can detect it.
    // On x86_64 all orderings currently lower to a plain aligned `mov` -- see
    // the postfix.rs comment for the SeqCst / mfence frontier.
    if let Some(assignment) = try_desugar_atomic_store(syntax_trees, expression) {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(assignment)),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) = parse_expression_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
    }

    for (punctuation, label, operator) in [
        (PunctuationKind::PlusEqual, "+=", BinaryOperator::Add),
        (PunctuationKind::MinusEqual, "-=", BinaryOperator::Subtract),
        (
            PunctuationKind::AsteriskEqual,
            "*=",
            BinaryOperator::Multiply,
        ),
        (PunctuationKind::SlashEqual, "/=", BinaryOperator::Divide),
        (PunctuationKind::PercentEqual, "%=", BinaryOperator::Modulo),
    ] {
        if !input.at_punctuation(punctuation) {
            continue;
        }
        let read_target = copy_compound_assignment_target(syntax_trees, expression)
            .ok_or_else(|| input.error_here("compound assignment target must be a place"))?;
        let input = input.take_punctuation(punctuation, label)?;
        let (right, input) = parse_expression_handle(syntax_trees, input)?;
        let value =
            syntax_trees
                .expressions
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: read_target,
                    operator,
                    right,
                }));
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Assignment(TableAssignment {
                    target: expression,
                    value,
                })),
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::RightBrace) {
        return Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ));
    }

    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression) {
        Ok((
            syntax_trees.statements.insert(StatementNode::Call(call)),
            input,
        ))
    } else {
        Ok((
            syntax_trees
                .statements
                .insert(StatementNode::Expression(expression)),
            input,
        ))
    }
}

/// CONCURRENCY STAGE 1 fire-and-forget: `spawn { call(); }` as a statement
/// desugars to the call statement itself (synchronous execution -- see
/// `expression/spawn.rs` for the full desugar contract). The optional
/// trailing `;` after the closing brace is accepted but not required,
/// matching the chapter-17 spelling.
fn parse_spawn_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let (expression, input) = parse_spawn_block_call_handle(syntax_trees, input)?;
    let input = if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")?
    } else {
        input
    };

    // The spawn body is guaranteed to be a call expression; statement-call
    // conversion only fails for call shapes (e.g. indexed receivers) that the
    // ordinary call-statement path also leaves as expression statements.
    let statement = if let Some(call) = expression_handle_to_statement_call(syntax_trees, expression)
    {
        StatementNode::Call(call)
    } else {
        StatementNode::Expression(expression)
    };

    Ok((syntax_trees.statements.insert(statement), input))
}

/// `_ = call();` -- an explicit-discard statement. The call executes and its
/// non-unit result is intentionally dropped (frozen decision 9: discarding a
/// non-unit result silently is a compile error; `_ =` is the spelling for an
/// intentional discard).
fn parse_discard_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
    let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;

    let Some(mut call) = expression_handle_to_statement_call(syntax_trees, expression) else {
        return Err(input.error_here("`_ =` discards a call result; only a call can follow `_ =`"));
    };
    call.discards_result = true;

    Ok((
        syntax_trees.statements.insert(StatementNode::Call(call)),
        rest,
    ))
}

fn parse_asm_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_contextual("asm")?;
    if input.at_contextual("where") {
        return Err(input.error_here("asm where contracts are not implemented yet"));
    }

    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let input = input.take_contextual("jmp").map_err(|_| {
        input.error_here("asm blocks currently support only `jmp` transition statements")
    })?;
    let (target, input) = parse_transition_block_target_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::Transition(TableTransition {
                target,
                continuation: TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::Always,
            })),
        input,
    ))
}

fn parse_relax_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_contextual("relax")?;
    let (target, input) = parse_expression_handle(syntax_trees, input)?;
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut statement_start = Handle::invalid();
    let mut statement_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (statement, rest) = parse_statement_handle(syntax_trees, input)?;
        let handle = syntax_trees.items.append_statement_handle(statement);
        if statement_count == 0 {
            statement_start = handle;
        }
        statement_count = statement_count
            .checked_add(1)
            .expect("relax statement span count overflow");
        input = rest;
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let statements = if statement_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(statement_start, statement_count)
    };

    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::Relax(TableRelax { target, statements })),
        input,
    ))
}

/// RETIRED (settled 2026-07-02: "if isn't a thing"). The `if` STATEMENT had
/// no `else` and never set a continuation, so its dispatch could always fall
/// through -- unwritable since the no-silent-fall-through rule, and used
/// exactly once in the whole corpus. Dispatch is `transition`. (The pattern
/// guard `Type::Case { x } if x > 3 ->` inside a transition arm is a
/// DIFFERENT surface and stays.)
fn parse_if_transition_statement_handle<'tokens, 'source>(
    _syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    Err(input.error_here(
        "the `if` statement is retired; dispatch is `transition <guard> { true -> ... _ -> ... }` \
         (every arm set must provably cover all cases)",
    ))
}

fn copy_compound_assignment_target(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => {
            let path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(path)
        }
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(member) => {
            let receiver = copy_compound_assignment_target(syntax_trees, member.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver,
                member: member.member,
                case_variant: member.case_variant.clone(),
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = copy_compound_assignment_target(syntax_trees, indexed.collection)?;
            let index = copy_stable_compound_assignment_index(syntax_trees, indexed.index)?;
            ExpressionNode::Indexed(TableIndexedExpression { collection, index })
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
}

fn copy_stable_compound_assignment_index(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let copy = match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Boolean(value) => ExpressionNode::Boolean(value),
        ExpressionNode::Integer(value) => ExpressionNode::Integer(value),
        ExpressionNode::Name(path) => {
            let path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(path)
        }
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(member) => {
            let receiver = copy_compound_assignment_target(syntax_trees, member.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver,
                member: member.member,
                case_variant: member.case_variant.clone(),
            })
        }
        _ => return None,
    };

    Some(syntax_trees.expressions.insert(copy))
}

fn parse_local_data_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
    let (initial_value, input) = if input.at_punctuation(PunctuationKind::Equal) {
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (expression, input) = parse_expression_handle(syntax_trees, input)?;
        (expression, input)
    } else {
        (ExpressionHandle::invalid(), input)
    };
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                name,
                type_reference,
                initial_value,
            })),
        input,
    ))
}

/// ATOMICS STAGE 1 (ch17, M2): Recognise `atomic_place.store(value, ordering)`
/// -- a Call expression with target name `"store"` and exactly two arguments
/// (the value to write and the ordering identifier) -- and desugar it into an
/// Assignment of the receiver place to the first argument. Returns `None` for
/// any other expression, leaving it to the normal statement paths.
/// ATOMICS STAGE 1 (ch17, M3): Try to parse and expand
/// `let name: type = place.fetch_add(delta, ordering);` as TWO statements:
///   1. `let name: type = place;`       -- captures the PRIOR value
///   2. `place = place + delta;`        -- increments the place
///
/// On x86_64 both desugar steps lower to ordinary reads/writes in stage 1;
/// a future pass will replace them with a single `LOCK xadd` RMW instruction
/// when the threading scheduler lands.  Returns `None` if the input does not
/// match the `let ... = ...fetch_add(...)` form, leaving the caller to fall
/// back to `parse_statement_handle`.
///
/// The returned span covers exactly two statement entries that are already
/// appended to `syntax_trees.items`; callers must advance their span
/// accounting by 2.
/// ATOMICS STAGE 1 (ch17, M4): Try to parse and expand
/// `let name: type = place.compare_exchange(expected, new_val, succ_ord, fail_ord);`
/// as TWO statements:
///   1. `let name: type = place;`
///      -- captures the PRIOR value (returned regardless of success/failure,
///         matching Rust's `Err` branch shape and x86 CMPXCHG register contract)
///   2. `place = prior + (prior == expected) * (new_val - prior);`
///      -- arithmetically conditional swap: when `prior == expected` evaluates
///         to 1 this simplifies to `place = new_val`; when 0, `place = prior`
///         (no-op). Under stage-1 single-threaded execution this is semantically
///         equivalent to a single `LOCK cmpxchg` instruction.
///
/// Return-shape choice: the PRIOR value (before the potential swap), not a
/// bool.  This mirrors x86 CMPXCHG's RAX contract and lets callers check
/// success with `prior == expected`.  A future pass will lower to a single
/// `LOCK cmpxchg` RMW instruction.
///
/// Returns `None` if the input does not match the form (wrong name or arity),
/// leaving the caller to fall back to `parse_statement_handle`.
/// The returned span covers exactly two statement entries already appended to
/// `syntax_trees.items`; callers must advance their span accounting by 2.
pub(super) fn try_parse_atomic_compare_exchange_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(HandleSpan<omega_syntax_trees::statement::StatementHandle>, Input<'tokens, 'source>)>
{
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
    let (place_expr, expected_expr, new_val_expr) = {
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
        (place, arg_handles[0], arg_handles[1])
    };

    // Statement 1: `let name: type = place;`
    let local_stmt = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name: name.clone(),
            type_reference,
            initial_value: place_expr,
        }));
    let first_handle = syntax_trees.items.append_statement_handle(local_stmt);

    // Build a Name expression referring to the freshly-bound local `name`.
    // This appears twice in the RHS arithmetic so we build it twice.
    let make_prior_name = |syntax_trees: &mut SyntaxTrees| {
        let id = omega_syntax_trees::identifier::Identifier::generated(name.as_str());
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

pub(super) fn try_parse_atomic_fetch_add_let<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Option<(HandleSpan<omega_syntax_trees::statement::StatementHandle>, Input<'tokens, 'source>)>
{
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

    // Check: is rhs a Call with target "fetch_add" and exactly 2 args?
    let (place_expr, delta_expr) = {
        let ExpressionNode::Call(ref call) = *syntax_trees.expressions.expression(rhs) else {
            return None;
        };
        if call.target.as_str() != "fetch_add" {
            return None;
        }
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
        (place, arg_handles[0])
    };

    // Duplicate the place expression (needed for `place = place + delta`).
    let place_copy = copy_expression_as_place(syntax_trees, place_expr)?;

    // Statement 1: `let name: type = place;`
    let local_stmt = syntax_trees
        .statements
        .insert(StatementNode::LocalData(TableLocalData {
            name,
            type_reference,
            initial_value: place_expr,
        }));
    let first_handle = syntax_trees.items.append_statement_handle(local_stmt);

    // Statement 2: `place = place + delta;`
    let add_expr = syntax_trees
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: place_copy,
            operator: BinaryOperator::Add,
            right: delta_expr,
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

/// Deep-copy an expression that is a valid place (member / name / indexed /
/// self), returning a fresh handle with the same structure.  Returns `None`
/// for non-place expression shapes (binary, call, etc.) since those cannot
/// appear on the left-hand side of an assignment.
fn copy_expression_as_place(
    syntax_trees: &mut SyntaxTrees,
    expr: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let node = syntax_trees.expressions.expression(expr).clone();
    let copied = match node {
        ExpressionNode::SelfValue => ExpressionNode::SelfValue,
        ExpressionNode::Member(m) => {
            let recv = copy_expression_as_place(syntax_trees, m.receiver)?;
            ExpressionNode::Member(TableMemberExpression {
                receiver: recv,
                member: m.member,
                case_variant: m.case_variant.clone(),
            })
        }
        ExpressionNode::Name(path) => {
            let new_path = syntax_trees
                .expressions
                .copy_identifier_path_prefix(path, path.len());
            ExpressionNode::Name(new_path)
        }
        ExpressionNode::Indexed(idx) => {
            let coll = copy_expression_as_place(syntax_trees, idx.collection)?;
            ExpressionNode::Indexed(TableIndexedExpression {
                collection: coll,
                index: idx.index,
            })
        }
        ExpressionNode::Mutable(inner) => {
            let inner_copy = copy_expression_as_place(syntax_trees, inner)?;
            ExpressionNode::Mutable(inner_copy)
        }
        _ => return None,
    };
    Some(syntax_trees.expressions.insert(copied))
}

fn try_desugar_atomic_store(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableAssignment> {
    let ExpressionNode::Call(ref call) = *syntax_trees.expressions.expression(expression) else {
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
    // The first argument is the value to store; the second is the ordering
    // (accepted syntactically, ignored in codegen for now).
    let value = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments)[0];
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

fn expression_handle_to_statement_call(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableCall> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };

    let (receiver, target) = split_expression_call_handle(syntax_trees, &call)?;
    Some(TableCall {
        receiver: receiver.members,
        receiver_starts_at_self: receiver.starts_at_self,
        target,
        arguments: copy_expression_handles_to_statement_table(syntax_trees, call.arguments),
        discards_result: false,
    })
}

struct StatementIdentifierPath {
    members: HandleSpan<omega_syntax_trees::identifier::Identifier>,
    starts_at_self: bool,
}

fn split_expression_call_handle(
    syntax_trees: &mut SyntaxTrees,
    call: &TableCallExpression,
) -> Option<(
    StatementIdentifierPath,
    omega_syntax_trees::identifier::Identifier,
)> {
    let receiver = if call.receiver.is_valid() {
        expression_handle_to_identifier_path_span(syntax_trees, call.receiver)?
    } else {
        StatementIdentifierPath {
            members: HandleSpan::empty(),
            starts_at_self: false,
        }
    };

    Some((receiver, call.target.clone()))
}

fn expression_handle_to_identifier_path_span(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<StatementIdentifierPath> {
    match syntax_trees.expressions.expression(expression).clone() {
        ExpressionNode::Name(path) => Some(StatementIdentifierPath {
            members: copy_expression_identifier_path_to_statement_table(syntax_trees, path),
            starts_at_self: false,
        }),
        ExpressionNode::SelfValue => {
            let self_member = syntax_trees.statements.append_identifier_path_member(
                omega_syntax_trees::identifier::Identifier::generated("self"),
            );
            Some(StatementIdentifierPath {
                members: HandleSpan::from_parts(self_member, 1),
                starts_at_self: true,
            })
        }
        ExpressionNode::Member(member) => {
            let mut receiver =
                expression_handle_to_identifier_path_span(syntax_trees, member.receiver)?;
            receiver.members = append_statement_identifier_path_member(
                syntax_trees,
                receiver.members,
                member.member,
            );
            Some(receiver)
        }
        _ => None,
    }
}

fn copy_expression_identifier_path_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<omega_syntax_trees::identifier::Identifier>,
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let member_count = syntax_trees.expressions.identifier_path_members(path).len();

    for index in 0..member_count {
        let member = syntax_trees.expressions.identifier_path_members(path)[index].clone();
        let handle = syntax_trees
            .statements
            .append_identifier_path_member(member);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement identifier path span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_statement_identifier_path_member(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<omega_syntax_trees::identifier::Identifier>,
    member: omega_syntax_trees::identifier::Identifier,
) -> HandleSpan<omega_syntax_trees::identifier::Identifier> {
    let handle = syntax_trees
        .statements
        .append_identifier_path_member(member);

    if path.is_empty() {
        HandleSpan::from_parts(handle, 1)
    } else {
        HandleSpan::from_parts(
            path.start(),
            path.count()
                .checked_add(1)
                .expect("statement identifier path span count overflow"),
        )
    }
}

fn copy_expression_handles_to_statement_table(
    syntax_trees: &mut SyntaxTrees,
    arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(arguments)
        .to_vec();

    for argument in arguments {
        let handle = syntax_trees
            .tables
            .statements
            .append_expression_handle(argument);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("statement call argument span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}
