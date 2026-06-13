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

fn parse_if_transition_statement_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StatementHandle> {
    let input = input.take_keyword(KeywordKind::If, "if")?;
    let (condition, input) = parse_expression_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (target, input) = if input.at_punctuation(PunctuationKind::RightBrace) {
        (
            syntax_trees
                .statements
                .insert_transition_target(TransitionTargetNode::Terminal),
            input,
        )
    } else {
        parse_transition_block_target_handle(syntax_trees, input)?
    };
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        syntax_trees
            .statements
            .insert(StatementNode::Transition(TableTransition {
                target,
                continuation: TransitionTargetHandle::invalid(),
                guard: TransitionGuardNode::When(condition),
            })),
        input,
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
